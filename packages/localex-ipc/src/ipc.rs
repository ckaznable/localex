use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bytes::{BufMut, BytesMut};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    net::{
        unix::{OwnedReadHalf, OwnedWriteHalf},
        UnixListener, UnixStream,
    },
    sync::mpsc,
    task::JoinHandle,
};
use tracing::error;

pub type IPCMsgPack<I> = (Arc<OwnedWriteHalf>, I);

#[async_trait]
pub trait IPC<I, O>
where
    I: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
    O: Serialize + DeserializeOwned + Send + Sync,
{
    fn ipc_tx(&self) -> mpsc::Sender<IPCMsgPack<I>>;
    fn ipc_rx(&mut self) -> &mut mpsc::Receiver<IPCMsgPack<I>>;

    #[allow(clippy::type_complexity)]
    fn get_ipc_channel() -> (mpsc::Sender<IPCMsgPack<I>>, mpsc::Receiver<IPCMsgPack<I>>) {
        mpsc::channel(16)
    }

    async fn recv_stream(&mut self) -> IPCMsgPack<I> {
        loop {
            if let Some(data) = self.ipc_rx().recv().await {
                return data;
            }
        }
    }

    async fn send_to_stream(&self, stream: &OwnedWriteHalf, msg: &O) -> Result<()> {
        let mut writer_buf = vec![];
        ciborium::ser::into_writer(msg, &mut writer_buf)?;
        let size_bytes = (writer_buf.len() as u32).to_le_bytes();

        let mut buffer = BytesMut::with_capacity(writer_buf.len() + size_bytes.len());
        buffer.put_slice(&size_bytes);
        buffer.put_slice(&writer_buf);

        loop {
            stream.writable().await?;
            match stream.try_write(&buffer) {
                Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
                Ok(_) => {
                    return Ok(())
                }
            }
        }
    }

    async fn connect(sock: &Path) -> Result<UnixStream> {
        UnixStream::connect(sock).await.map_err(anyhow::Error::from)
    }

    async fn wait_for_stream(
        &self,
        read: Arc<OwnedReadHalf>,
        write: Arc<OwnedWriteHalf>,
    ) -> Result<JoinHandle<Result<()>>> {
        let tx = self.ipc_tx();

        let handle = tokio::spawn(async move {
            let mut reader = StreamReader::new(tx.clone(), read.clone(), write.clone());
            loop {
                if let Err(err) = reader.read().await {
                    error!("{err:?}");
                }
            }
        });

        Ok(handle)
    }

    async fn listen(&self, sock_path: PathBuf) -> Result<JoinHandle<Result<()>>> {
        let tx = self.ipc_tx();

        let handle = tokio::spawn(async move {
            let listener = UnixListener::bind(sock_path)?;
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let (read, write) = stream.into_split();
                    let mut reader = StreamReader::new(tx.clone(), Arc::new(read), Arc::new(write));
                    if let Err(err) = reader.read().await {
                        error!("{err:?}");
                    }
                }
            }
        });

        Ok(handle)
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ReaderState {
    Reading,
    Idle,
}

struct StreamReader<I> {
    tx: mpsc::Sender<IPCMsgPack<I>>,
    read: Arc<OwnedReadHalf>,
    write: Arc<OwnedWriteHalf>,
    read_buf: BytesMut,
    buf: Vec<u8>,
    state: ReaderState,
    size: u32,
    remaining: usize,
}

impl<I> StreamReader<I>
where
    I: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
{
    fn new(
        tx: mpsc::Sender<IPCMsgPack<I>>,
        read: Arc<OwnedReadHalf>,
        write: Arc<OwnedWriteHalf>,
    ) -> Self {
        Self {
            tx,
            read,
            write,
            buf: vec![],
            read_buf: BytesMut::with_capacity(4096),
            state: ReaderState::Idle,
            size: 0,
            remaining: 0
        }
    }

    async fn read_chunk(&mut self, read_size: usize) -> Result<usize> {
        if read_size < 8 {
            return Err(anyhow!("reading data error"));
        }

        let mut offset_start = 0usize;
        if self.state == ReaderState::Idle {
            let (size, _) = &self.read_buf[..read_size].split_at(4);
            self.size = u32::from_le_bytes((*size).try_into()?);
            self.remaining = self.size as usize;
            offset_start = 4;
        }

        let remaining = self.remaining - self.buf.len();
        let offset = std::cmp::min(remaining, read_size);
        let (chunk, next_chunk) = self.read_buf[offset_start..].split_at(offset);
        self.remaining = self.remaining.saturating_sub(chunk.len());

        if self.remaining == 0 {
            self.buf.extend_from_slice(chunk);
            self.send(&self.buf).await?;
            self.buf.clear();
        }

        let remaining_chunk_size = next_chunk.len();
        if next_chunk.is_empty() {
            self.state = ReaderState::Idle;
        } else {
            self.read_buf = BytesMut::from(next_chunk);
            self.state = ReaderState::Reading;
        }

        Ok(remaining_chunk_size)
    }

    async fn read(&mut self) -> Result<()> {
        self.read.readable().await?;

        match self.read.try_read_buf(&mut self.read_buf) {
            Ok(0) => {
                self.size = 0;
                self.buf.clear();
                self.state = ReaderState::Idle;
            },
            Ok(n) => {
                loop {
                    if self.read_chunk(n).await? == 0 {
                        break;
                    }
                }
            },
            Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {},
            Err(e) => return Err(anyhow::Error::from(e)),
        }

        Ok(())
    }

    async fn send(&self, data: &[u8]) -> Result<()> {
        let request: I = ciborium::from_reader(data)?;
        self.tx.send((self.write.clone(), request)).await
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }
}
