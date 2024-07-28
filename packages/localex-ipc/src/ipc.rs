use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    net::{
        unix::{OwnedReadHalf, OwnedWriteHalf},
        UnixListener, UnixStream,
    },
    sync::{mpsc, Mutex},
    task::JoinHandle,
};
use tracing::error;

use crate::sock;

thread_local! {
    static DATA_BUF: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::with_capacity(1024 * 256)));
    static WRITER_BUF: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::with_capacity(1024 * 256)));
    static READER_BUF: Arc<Mutex<[u8; 1024 * 128]>> = Arc::new(Mutex::new([0u8; 1024 * 128]));
}

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
        read.readable().await?;

        let buffer = DATA_BUF.with(|d| d.clone());
        let mut buffer = buffer.lock().await;

        let read_buf = READER_BUF.with(|r| r.clone());
        let mut read_buf = read_buf.lock().await;

        match read.try_read_buf(&mut *buffer) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                let data = &buffer[..n];
                let request: I = ciborium::from_reader_with_buffer(data, &mut *read_buf)?;
                tx.send((write, request)).await?;
            }
            Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(anyhow::Error::from(e)),
        }

        Ok(())
    }

    async fn send_to_stream(&self, stream: &OwnedWriteHalf, msg: &O) -> Result<()> {
        let writer_buf = WRITER_BUF.with(|w| w.clone());
        let mut writer_buf = writer_buf.lock().await;

        ciborium::ser::into_writer(msg, &mut *writer_buf)?;
        stream.writable().await?;
        stream.try_write(&writer_buf)?;
        Ok(())
    }

    async fn connect() -> Result<UnixStream> {
        UnixStream::connect(sock::get_sock_connect_path())
            .await
            .map_err(anyhow::Error::from)
    }

    async fn wait_for_stream(
        &self,
        read: Arc<OwnedReadHalf>,
        write: Arc<OwnedWriteHalf>,
    ) -> Result<JoinHandle<Result<()>>> {
        let tx = self.ipc_tx();

        let handle = tokio::spawn(async move {
            loop {
                if let Err(err) = Self::handle_stream(tx.clone(), read.clone(), write.clone()).await {
                    error!("{err:?}");
                    continue;
                }
            }
        });

        Ok(handle)
    }

    async fn listen(&self) -> Result<JoinHandle<Result<()>>> {
        let tx = self.ipc_tx();

        let handle = tokio::spawn(async move {
            let listener = UnixListener::bind(sock::get_sock_mount_path())?;
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    let (read, write) = stream.into_split();
                    if let Err(err) = Self::handle_stream(tx.clone(), Arc::new(read), Arc::new(write)).await {
                        error!("{err:?}");
                        continue;
                    }
                }
            }
        });

        Ok(handle)
    }
}
