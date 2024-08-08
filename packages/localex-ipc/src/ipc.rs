use std::{
    path::{Path, PathBuf},
    sync::Arc, time::Duration,
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
    sync::{broadcast, mpsc},
    task::JoinHandle,
};
use tracing::{error, info};

pub type IPCMsgPack<I> = (Arc<OwnedWriteHalf>, I);

#[async_trait]
pub trait IPC<I, O>
where
    I: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
    O: Serialize + DeserializeOwned + Send + Sync,
{
    fn ipc_tx(&self) -> mpsc::Sender<IPCMsgPack<I>>;
    fn ipc_rx(&mut self) -> &mut mpsc::Receiver<IPCMsgPack<I>>;
    fn ctrlc_rx(&self) -> broadcast::Receiver<()>;

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

        let mut attempts = 0;
        let max_attempts = 5;
        let mut sent = 0;

        while sent < buffer.len() {
            stream.writable().await?;
            match stream.try_write(&buffer[sent..]) {
                Ok(n) => {
                    info!("write {} bytes to stream", n);
                    sent += n;
                    attempts = 0;
                }
                Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(anyhow!("Max retry attempts reached"));
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }

    #[inline]
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
                    break;
                }
            }
            Ok(())
        });

        Ok(handle)
    }

    async fn listen(&self, sock_path: PathBuf) -> Result<JoinHandle<Result<()>>> {
        let tx = self.ipc_tx();
        let crx = self.ctrlc_rx();

        let handle = tokio::spawn(async move {
            let listener = UnixListener::bind(sock_path)?;
            let mut ctrlc_rx_out = crx.resubscribe();
            let ctrlc_rx_inner = crx.resubscribe();

            loop {
                tokio::select! {
                    _ = ctrlc_rx_out.recv() => break,
                    r = listener.accept() => if let Ok((stream, _)) = r {
                        let crx = ctrlc_rx_inner.resubscribe();
                        let tx = tx.clone();
                        info!("new accept stream incoming");
                        tokio::spawn(async move {
                            Self::handle_accept_connection(stream, tx, crx).await;
                        });
                    }
                }
            }

            Ok(())
        });

        Ok(handle)
    }

    async fn handle_accept_connection(stream: UnixStream, tx: mpsc::Sender<IPCMsgPack<I>>, mut ctrlc_rx: broadcast::Receiver<()>) {
        let (read, write) = stream.into_split();
        let mut reader = StreamReader::new(tx, Arc::new(read), Arc::new(write));
        loop {
            tokio::select! {
                _ = ctrlc_rx.recv() => break,
                r = reader.read() => if let Err(err) = r {
                    error!("handle connection error: {err:?}");
                    break;
                },
            }
        }
    }
}

struct StreamReader<I> {
    tx: mpsc::Sender<IPCMsgPack<I>>,
    read: Arc<OwnedReadHalf>,
    write: Arc<OwnedWriteHalf>,
    read_buf: BytesMut,
    buf: Vec<u8>,
    reading_size: usize,
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
            reading_size: 0,
        }
    }

    async fn read_chunk(&mut self, packet_size: usize) -> Result<()> {
        if self.read_buf.len() < 4 {
            return Err(anyhow!("reading chunk error"));
        }

        let mut read = 0usize;
        while read < packet_size {
            if self.buf.is_empty() {
                let size_byte = &self.read_buf[read..read+4];
                self.reading_size = u32::from_le_bytes((*size_byte).try_into()?) as usize;
                info!("reading chunk size: {}", self.reading_size);
                read += 4;
            }

            let remaining = self.reading_size - self.buf.len();
            let remaining = if packet_size < read + remaining {
                packet_size - read + 1
            } else {
                remaining
            };

            if remaining > 0 {
                self.buf.extend_from_slice(&self.read_buf[read..read + remaining]);
                read += remaining;
            }

            if self.buf.len() == self.reading_size {
                self.send(&self.buf).await?;
                self.buf.clear();
                self.reading_size = 0;
            }
        }

        Ok(())
    }

    async fn read(&mut self) -> Result<()> {
        self.read.readable().await?;

        match self.read.try_read_buf(&mut self.read_buf) {
            Ok(0) => {},
            Ok(n) => {
                info!("read {} bytes from stream", n);
                self.read_chunk(n).await?;
                self.read_buf.clear();
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
