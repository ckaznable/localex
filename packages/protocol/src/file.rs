use std::{path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use async_compression::tokio::write::ZstdEncoder;
use async_trait::async_trait;
use common::file::{ChunkResult, FileRequestPayload, FileResponsePayload, LocalExFileRequest, LocalExFileResponse};
use futures::future::join_all;
use libp2p::{request_response::{self, ResponseChannel}, PeerId};
use tokio::{fs::File, io::{AsyncReadExt, AsyncWriteExt}, sync::Semaphore, task::JoinHandle};
use tracing::error;

use crate::LocalExSwarm;

const CHUNK_SIZE: usize = 1024 * 1024;
const MAX_CONCURRENT_CONNECTIONS: usize = 5;

pub struct FileChunk {
    pub chunk: Vec<u8>,
    pub offset: usize,
}

#[async_trait]
pub trait FileReaderClient {
    async fn read(&mut self, chunk: FileChunk) -> Result<()>;
    async fn ready(&mut self, id: String, filename: String, size: usize, chunks: usize, chunk_size: usize) -> Result<()>;
}

#[async_trait]
pub trait FileTransferClientProtocol: LocalExSwarm + FileReaderClient {
    async fn recv_file(&mut self, chunk: &[u8]) -> Result<()>;

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
            Message {
                message: request_response::Message::Request { request, channel, .. },
                ..
            } => {
                self.handle_file_request(channel, request).await?;
            }
            Message {
                message: request_response::Message::Response { response, .. },
                ..
            } => {
                self.handle_file_response(response).await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn send_file_with_session(&mut self, id: String, peer: &PeerId, file_path: PathBuf) -> Result<()> {
        let session = uuid::Uuid::new_v4().to_string();
        self.send_file(session, id, peer, file_path).await
    }

    async fn send_file(&mut self, session: String, id: String, peer: &PeerId, file_path: PathBuf) -> Result<()> {
        let mut input_file = File::open(file_path).await?;

        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS));
        let (tx, mut rx) = tokio::sync::mpsc::channel(MAX_CONCURRENT_CONNECTIONS);
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut compress_tasks = Vec::new();
        let mut i = 0usize;

        loop {
            let bytes_read = input_file.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }

            let chunk = buffer[..bytes_read].to_vec();
            let tx = tx.clone();

            let compress_task: JoinHandle<Result<()>> = tokio::spawn(async move {
                let compressed_data = Self::compress_chunk(&chunk).await?;
                tx.send((i, compressed_data)).await.map_err(anyhow::Error::from)
            });

            compress_tasks.push(compress_task);
            i += 1;
        }

        drop(tx);

        while let Some((index, data)) = rx.recv().await {
            let _ = semaphore.acquire().await.unwrap();
            self.send_chunk(session.clone(), id.clone(), peer, CHUNK_SIZE * index, data);
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

    fn send_chunk(&mut self, session: String, id: String, peer: &PeerId, offset: usize, data: Vec<u8>) {
        self.swarm_mut()
            .behaviour_mut()
            .rr_file
            .send_request(peer, LocalExFileRequest {
                session,
                payload: FileRequestPayload::Chunk {
                    id,
                    offset,
                    data,
                }
            });
    }

    async fn handle_file_request(&mut self, channel: ResponseChannel<LocalExFileResponse>, request: LocalExFileRequest) -> Result<()> {
        let LocalExFileRequest { session, payload } = request;
        match payload {
            FileRequestPayload::Chunk { id, data, offset } => {
                let chunk = FileChunk {
                    chunk: data,
                    offset,
                };

                let result = match self.read(chunk).await {
                    Ok(_) => ChunkResult::Success,
                    Err(_) => ChunkResult::Fail,
                };

                if self.swarm_mut()
                    .behaviour_mut()
                    .rr_file
                    .send_response(channel, LocalExFileResponse {
                        session,
                        payload: FileResponsePayload::Checked { id, result, offset },
                    })
                    .is_err() {
                    return Err(anyhow!("send checked response error"));
                }
            },
            FileRequestPayload::Ready { id, filename, size, chunks, chunk_size } => {
                self.ready(id, filename, size, chunks, chunk_size).await?;
            },
        };

        Ok(())
    }

    async fn handle_file_response(&mut self, _response: LocalExFileResponse) -> Result<()> {
        todo!()
    }
}
