use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    net::{
        unix::{OwnedReadHalf, OwnedWriteHalf},
        UnixListener, UnixStream,
    },
    sync::mpsc,
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

    async fn handle_stream(
        tx: mpsc::Sender<IPCMsgPack<I>>,
        read: Arc<OwnedReadHalf>,
        write: Arc<OwnedWriteHalf>,
    ) -> Result<()> {
        let mut data = vec![];
        let mut buffer = Vec::with_capacity(1024 * 4);

        loop {
            read.readable().await?;
            buffer.clear();

            match read.try_read_buf(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    info!("reading {n}byte data from stream");
                    data.extend_from_slice(&buffer[..n]);
                }
                Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(anyhow::Error::from(e)),
            }
        }

        info!("read msg size: {}", data.len());
        let data: &[u8] = &data;
        let request: I = ciborium::from_reader(data)?;
        tx.send((write, request)).await?;
        Ok(())
    }

    async fn send_to_stream(&self, stream: &OwnedWriteHalf, msg: &O) -> Result<()> {
        let mut writer_buf = vec![];
        ciborium::ser::into_writer(msg, &mut writer_buf)?;
        info!("try to write data to stream");

        loop {
            stream.writable().await?;
            match stream.try_write(&writer_buf) {
                Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
                Ok(n) => {
                    info!("send msg size: {}", n);
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
            loop {
                if let Err(err) = Self::handle_stream(tx.clone(), read.clone(), write.clone()).await
                {
                    error!("{err:?}");
                    continue;
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
                    if let Err(err) =
                        Self::handle_stream(tx.clone(), Arc::new(read), Arc::new(write)).await
                    {
                        error!("{err:?}");
                        continue;
                    }
                }
            }
        });

        Ok(handle)
    }
}
