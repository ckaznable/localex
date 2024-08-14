use std::path::PathBuf;

use anyhow::{anyhow, Result};
use async_compression::tokio::write::ZstdEncoder;
use async_trait::async_trait;
use common::file::{
    ChunkResult, FileRequestPayload, FileResponsePayload, LocalExFileRequest, LocalExFileResponse,
};
use futures::future::join_all;
use libp2p::{
    request_response::{self, ResponseChannel},
    PeerId,
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{
        broadcast::{self, error::TryRecvError},
        mpsc,
    },
    task::JoinHandle,
};
use tracing::error;

use crate::{AbortListener, LocalExSwarm};

const CHUNK_SIZE: usize = 1024 * 1024;
const MAX_CONCURRENT_CONNECTIONS: usize = 5;

pub struct FileChunk {
    pub chunk: Vec<u8>,
    pub offset: usize,
}

struct FileSender;
impl FileSender {
    async fn send(
        file_path: PathBuf,
        tx: mpsc::Sender<(usize, Vec<u8>)>,
        mut abort_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        let mut input_file = File::open(file_path).await?;

        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut compress_tasks = Vec::new();
        let mut i = 0usize;

        loop {
            let bytes_read = input_file.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }

            match abort_rx.try_recv() {
                Ok(()) => break,
                Err(e) => {
                    if let TryRecvError::Empty = e {
                    } else {
                        break;
                    }
                }
            }

            let chunk = buffer[..bytes_read].to_vec();
            let tx = tx.clone();

            let compress_task: JoinHandle<Result<()>> = tokio::spawn(async move {
                let compressed_data = Self::compress_chunk(&chunk).await?;
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

    async fn compress_chunk(chunk: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = ZstdEncoder::new(Vec::new());
        encoder.write_all(chunk).await?;
        encoder.shutdown().await?;
        Ok(encoder.into_inner())
    }
}

#[async_trait]
pub trait FileReaderClient {
    async fn done(&mut self, session: &str);
    async fn read(&mut self, session: &str, chunk: FileChunk) -> Result<()>;
    async fn ready(
        &mut self,
        session: &str,
        id: &str,
        filename: &str,
        size: usize,
        chunks: usize,
        chunk_size: usize,
    ) -> Result<()>;
}

#[async_trait]
pub trait FileTransferClientProtocol: LocalExSwarm + FileReaderClient + AbortListener {
    fn get_file_path_with_id(&self, id: &str) -> PathBuf;

    fn send_file_rr_response(
        &mut self,
        session: String,
        channel: ResponseChannel<LocalExFileResponse>,
        payload: FileResponsePayload,
    ) -> Result<()> {
        if self
            .swarm_mut()
            .behaviour_mut()
            .rr_file
            .send_response(channel, LocalExFileResponse { session, payload })
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
        payload: FileRequestPayload,
    ) {
        self.swarm_mut()
            .behaviour_mut()
            .rr_file
            .send_request(peer, LocalExFileRequest { session, payload });
    }

    async fn handle_file(
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
                    Request {
                        request, channel, ..
                    } => {
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

    async fn send_file(
        &mut self,
        session: String,
        id: String,
        peer: &PeerId,
        file_path: PathBuf,
    ) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(MAX_CONCURRENT_CONNECTIONS);

        let mut abort_rx = self.abort_rx();
        let _abort_rx = abort_rx.resubscribe();
        tokio::spawn(async move {
            if let Err(e) = FileSender::send(file_path, tx, _abort_rx).await {
                error!("{e:?}");
            };
        });

        while let Some((index, data)) = rx.recv().await {
            match abort_rx.try_recv() {
                Ok(()) => break,
                Err(e) => {
                    if let TryRecvError::Empty = e {
                    } else {
                        break;
                    }
                }
            }

            self.send_chunk(session.clone(), id.clone(), peer, CHUNK_SIZE * index, data);
        }

        self.send_file_rr_request(peer, session, FileRequestPayload::Done);
        Ok(())
    }

    fn send_chunk(
        &mut self,
        session: String,
        id: String,
        peer: &PeerId,
        offset: usize,
        data: Vec<u8>,
    ) {
        self.send_file_rr_request(
            peer,
            session,
            FileRequestPayload::Chunk { id, offset, data },
        );
    }

    async fn handle_file_reciver(
        &mut self,
        channel: ResponseChannel<LocalExFileResponse>,
        request: LocalExFileRequest,
    ) -> Result<()> {
        let LocalExFileRequest { session, payload } = request;

        use FileRequestPayload::*;
        match payload {
            Done => {
                self.done(&session).await;
            }
            Chunk {
                id,
                data: chunk,
                offset,
            } => {
                let chunk = FileChunk { chunk, offset };
                let result = match self.read(&session, chunk).await {
                    Ok(_) => ChunkResult::Success,
                    Err(_) => ChunkResult::Fail,
                };

                self.send_file_rr_response(
                    session,
                    channel,
                    FileResponsePayload::Checked { id, result, offset },
                )?;
            }
            Ready {
                id,
                filename,
                size,
                chunks,
                chunk_size,
            } => {
                self.ready(&session, &id, &filename, size, chunks, chunk_size)
                    .await?;
                self.send_file_rr_response(session, channel, FileResponsePayload::Ready { id })?;
            }
        };

        Ok(())
    }

    async fn handle_file_sender(
        &mut self,
        peer: PeerId,
        response: LocalExFileResponse,
    ) -> Result<()> {
        let LocalExFileResponse { session, payload } = response;

        use FileResponsePayload::*;
        match payload {
            Ready { id } | RequestFile { id } => {
                let file_path = self.get_file_path_with_id(&id);
                self.send_file(session, id, &peer, file_path).await?;
            }
            RequestChunk { .. } => {
                todo!()
            }
            Checked { result, .. } => {
                match result {
                    ChunkResult::Success => todo!(),
                    ChunkResult::Fail => todo!(),
                }
            }
        }

        Ok(())
    }
}
