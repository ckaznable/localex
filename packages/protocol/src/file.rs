use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use async_compression::tokio::write::{ZstdDecoder, ZstdEncoder};
use async_trait::async_trait;
use common::file::{
    ChunkResult, FileRequestPayload, FileResponsePayload, LocalExFileRequest, LocalExFileResponse,
};
use database::LocalExDb;
use futures::{executor::block_on, future::join_all};
use libp2p::{
    bytes::Bytes, request_response::{self, ResponseChannel}, PeerId
};
use tokio::{
    fs::File, io::{AsyncRead, AsyncReadExt, AsyncWriteExt}, sync::{
        broadcast::{self, error::TryRecvError}, mpsc, Mutex
    }, task::JoinHandle
};
use tracing::{error, info};

use crate::{AbortListener, LocalExSwarm};

const CHUNK_SIZE: usize = 1024 * 1024;
const MAX_CONCURRENT_CONNECTIONS: usize = 5;

pub struct FileChunk {
    pub chunk: Vec<u8>,
    pub offset: usize,
}

async fn compress_chunk(chunk: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZstdEncoder::new(Vec::new());
    encoder.write_all(chunk).await?;
    encoder.shutdown().await?;
    Ok(encoder.into_inner())
}

async fn decompress_chunk(chunk: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZstdDecoder::new(Vec::new());
    decoder.write_all(chunk).await?;
    decoder.shutdown().await?;
    Ok(decoder.into_inner())
}

pub trait RegistFileDatabase {
    fn db(&self) -> &LocalExDb;
}

#[async_trait]
pub trait FileReaderClient {
    async fn done(&mut self, session: &str, app_id: &str, file_id: &str) -> Result<()>;
    async fn read(&mut self, session: &str, app_id: &str, file_id: &str, chunk: FileChunk) -> Result<()>;
    async fn ready(
        &mut self,
        session: &str,
        app_id: &str,
        file_id: &str,
        size: usize,
        chunk_size: usize,
    ) -> Result<()>;
}

#[derive(Clone)]
pub enum ReadableItem {
    FilePath(PathBuf),
    Raw(Bytes),
}

impl ReadableItem {
    async fn chunk_and_compress(
        &mut self,
        tx: mpsc::Sender<(usize, Vec<u8>)>,
        mut abort_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut compress_tasks = Vec::new();
        let mut i = 0usize;

        use ReadableItem::*;
        let mut input: Box<dyn AsyncRead + Unpin + Send> = match self {
            FilePath(path) => Box::new(File::open(path).await?),
            Raw(raw) => Box::new(&raw[..]),
        };

        loop {
            let bytes_read = input.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }

            match abort_rx.try_recv() {
                Ok(()) => break,
                Err(e) => {
                    let TryRecvError::Empty = e else {
                        break;
                    };
                }
            }

            let chunk = buffer[..bytes_read].to_vec();
            let tx = tx.clone();

            let compress_task: JoinHandle<Result<()>> = tokio::spawn(async move {
                let compressed_data = compress_chunk(&chunk).await?;
                tx.send((i, compressed_data)).await?;
                Ok(())
            });

            compress_tasks.push(compress_task);
            i += 1;
        }

        for task in join_all(compress_tasks).await {
            task??;
        }

        Ok(())
    }
}

pub trait FilesRegisterCenter: RegistFileDatabase {
    fn raw_store(&self) -> &HashMap<String, Bytes>;
    fn raw_store_mut(&mut self) -> &mut HashMap<String, Bytes>;

    fn regist_path(&self, app_id: &str, file_id: &str, path: &str) -> Result<()> {
        block_on(async {
            self.db().regist_file(app_id, file_id, path).await
        })
    }

    fn regist_raw(&mut self, id: String, item: Vec<u8>) {
        self.raw_store_mut().insert(id, Bytes::from(item));
    }

    fn unregist_file(&self, app_id: &str, file_id: &str) -> Result<()> {
        block_on(async {
            self.db().unregist_file(app_id, file_id).await
        })
    }

    fn unregist_app(&self, app_id: &str) -> Result<()> {
        block_on(async {
            self.db().unregist_app(app_id).await
        })
    }

    fn unregist_raw(&mut self, id: &str) {
        self.raw_store_mut().remove(id);
    }

    fn get_regist_item<'a>(&self, app_id: &'a str, file_id: &'a str) -> Option<ReadableItem> {
        self
            .raw_store()
            .get(file_id)
            .cloned()
            .map(ReadableItem::Raw)
            .or_else(|| block_on(async {
                self.db()
                    .get_file_path(app_id, file_id)
                    .await
                    .map(PathBuf::from)
                    .map(ReadableItem::FilePath)
            }))
    }
}

#[async_trait]
pub trait FileTransferClientProtocol: LocalExSwarm + FileReaderClient + AbortListener + FilesRegisterCenter {
    fn send_file_rr_response(
        &mut self,
        session: String,
        app_id: String,
        file_id: String,
        channel: ResponseChannel<LocalExFileResponse>,
        payload: FileResponsePayload,
    ) -> Result<()> {
        if self
            .swarm_mut()
            .behaviour_mut()
            .rr_file
            .send_response(channel, LocalExFileResponse { session, app_id, file_id, payload })
            .is_err()
        {
            Err(anyhow!("send file response error"))
        } else {
            Ok(())
        }
    }

    fn send_file_rr_request(
        &mut self,
        peer: &PeerId,
        session: String,
        app_id: String,
        file_id: String,
        payload: FileRequestPayload,
    ) {
        self.swarm_mut()
            .behaviour_mut()
            .rr_file
            .send_request(peer, LocalExFileRequest { session, app_id, file_id, payload });
    }

    async fn handle_file_event(
        &mut self,
        event: request_response::Event<LocalExFileRequest, LocalExFileResponse>,
    ) -> Result<()> {
        use request_response::Event::*;
        match event {
            InboundFailure { error, .. } => {
                error!("rr_file inbound failure: {error}");
            }
            OutboundFailure { error, .. } => {
                error!("rr_file outbound failure: {error}");
            }
            Message { peer, message } => {
                use request_response::Message::*;
                match message {
                    Request { request, channel, .. } => {
                        self.handle_file_reciver(channel, request).await?;
                    }
                    Response { response, .. } => {
                        self.handle_file_sender(peer, response).await?;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    async fn do_send_file(
        &mut self,
        session: String,
        app_id: String,
        file_id: String,
        peer: PeerId,
        input: ReadableItem,
    ) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(MAX_CONCURRENT_CONNECTIONS);

        let _input = Arc::new(Mutex::new(input));
        let input = _input.clone();

        let mut abort_rx = self.abort_rx();
        let _abort_rx = abort_rx.resubscribe();
        let handle = tokio::spawn(async move {
            let mut input = input.lock().await;
            if let Err(e) = input.chunk_and_compress(tx, _abort_rx).await {
                error!("{e:?}");
            };
        });

        while let Some((index, data)) = rx.recv().await {
            match abort_rx.try_recv() {
                Ok(()) => break,
                Err(e) => {
                    let TryRecvError::Empty = e else {
                        break;
                    };
                }
            }

            self.send_chunk(session.clone(), app_id.clone(), file_id.clone(), &peer, CHUNK_SIZE * index, data);
        }

        handle.await?;
        self.send_file_rr_request(&peer, session, app_id, file_id, FileRequestPayload::Done);
        Ok(())
    }

    fn send_chunk(
        &mut self,
        session: String,
        app_id: String,
        file_id: String,
        peer: &PeerId,
        offset: usize,
        data: Vec<u8>,
    ) {
        self.send_file_rr_request(
            peer,
            session,
            app_id,
            file_id,
            FileRequestPayload::Chunk { offset, data },
        );
    }

    async fn send_file(&mut self, peer: &PeerId, app_id: String, file_id: String) -> Result<()> {
        let item = self
            .get_regist_item(&app_id, &file_id)
            .ok_or_else(|| anyhow!("regist item not found"))?;
        let payload: FileRequestPayload = match item {
            ReadableItem::FilePath(path) => {
                let metadata = std::fs::metadata(path)?;
                FileRequestPayload::Ready {
                    size: metadata.len() as usize,
                    chunk_size: CHUNK_SIZE,
                }
            },
            ReadableItem::Raw(body) => {
                FileRequestPayload::Ready {
                    size: body.len(),
                    chunk_size: CHUNK_SIZE,
                }
            },
        };

        let session = uuid::Uuid::new_v4().to_string();
        self.send_file_rr_request(peer, session, app_id, file_id , payload);
        Ok(())
    }

    async fn handle_file_reciver(
        &mut self,
        channel: ResponseChannel<LocalExFileResponse>,
        request: LocalExFileRequest,
    ) -> Result<()> {
        let LocalExFileRequest { session, app_id, file_id, payload } = request;

        use FileRequestPayload::*;
        match payload {
            Done => {
                info!("session: {session} id: {app_id}:{file_id} transfer file done");
                self.done(&session, &app_id, &file_id).await?;
            }
            Chunk {
                data,
                offset,
            } => {
                let chunk = decompress_chunk(&data).await?;
                let chunk = FileChunk { chunk, offset };
                let result = match self.read(&session, &app_id, &file_id, chunk).await {
                    Ok(_) => ChunkResult::Success,
                    Err(e) => {
                        error!("{e:?}");
                        ChunkResult::Fail
                    },
                };

                self.send_file_rr_response(
                    session,
                    app_id,
                    file_id,
                    channel,
                    FileResponsePayload::Checked { result, offset },
                )?;
            }
            Ready {
                size,
                chunk_size,
            } => {
                info!("file is ready, size: {size}, chunk size: {chunk_size}");
                self.ready(&session, &app_id, &file_id, size, chunk_size).await?;
                self.send_file_rr_response(session, app_id, file_id, channel, FileResponsePayload::Ready)?;
            }
        };

        Ok(())
    }

    async fn handle_file_sender(
        &mut self,
        peer: PeerId,
        response: LocalExFileResponse,
    ) -> Result<()> {
        let LocalExFileResponse { session, app_id, file_id, payload } = response;

        use FileResponsePayload::*;
        match payload {
            Ready | RequestFile => {
                if let Some(item) = self.get_regist_item(&app_id, &file_id) {
                    self.do_send_file(session, app_id, file_id, peer, item.clone()).await?;
                }
            }
            RequestChunk { .. } => {
                todo!()
            }
            Checked { result, offset } => {
                info!("transfer chunk offset {} {}", offset, match result {
                    ChunkResult::Success => "success",
                    ChunkResult::Fail => "fail",
                })
            }
        }

        Ok(())
    }
}
