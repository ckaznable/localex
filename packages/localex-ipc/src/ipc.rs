use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    net::{UnixListener, UnixStream},
    sync::{mpsc, Mutex}, task::JoinHandle,
};
use tracing::error;

use crate::sock;

thread_local! {
    static DATA_BUF: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::with_capacity(1024 * 256)));
    static WRITER_BUF: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::with_capacity(1024 * 256)));
    static READER_BUF: Arc<Mutex<[u8; 1024 * 128]>> = Arc::new(Mutex::new([0u8; 1024 * 128]));
}

#[async_trait]
pub trait IPC<I, O>
where
    I: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
    O: Serialize + DeserializeOwned + Send + Sync,
{
    fn ipc_tx(&self) -> mpsc::Sender<(UnixStream, I)>;
    fn ipc_rx(&mut self) -> &mut mpsc::Receiver<(UnixStream, I)>;

    #[allow(clippy::type_complexity)]
    fn get_ipc_channel() -> (mpsc::Sender<(UnixStream, I)>, mpsc::Receiver<(UnixStream, I)>) {
        mpsc::channel(16)
    }

    async fn recv_stream(&mut self) -> (UnixStream, I) {
        loop {
            if let Some(data) = self.ipc_rx().recv().await {
                return data
            }
        }
    }

    async fn handle_stream(tx: mpsc::Sender<(UnixStream, I)>, stream: UnixStream) -> Result<()> {
        stream.readable().await?;

        let buffer = DATA_BUF.with(|d| d.clone());
        let mut buffer = buffer.lock().await;

        let read_buf = READER_BUF.with(|r| r.clone());
        let mut read_buf = read_buf.lock().await;

        match stream.try_read_buf(&mut *buffer) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                let data = &buffer[..n];
                let request: I = ciborium::from_reader_with_buffer(data, &mut *read_buf)?;
                tx.send((stream, request)).await?;
            }
            _ => return Err(anyhow!("read buffer error")),
        }

        Ok(())
    }

    async fn send_to_stream(&self, stream: &UnixStream, msg: &O) -> Result<()> {
        let writer_buf = WRITER_BUF.with(|w| w.clone());
        let mut writer_buf = writer_buf.lock().await;

        ciborium::ser::into_writer(msg, &mut *writer_buf)?;
        stream.writable().await?;
        stream.try_write(&writer_buf)?;
        Ok(())
    }

    async fn listen(&self) -> Result<JoinHandle<Result<()>>> {
        let tx = self.ipc_tx();

        let handle = tokio::spawn(async move {
            let listener = UnixListener::bind(sock::get_sock_mount_path())?;
            loop {
                if let Ok((stream, _)) = listener.accept().await {
                    if let Err(err) = Self::handle_stream(tx.clone(), stream).await {
                        error!("{err:?}");
                        continue;
                    }
                }
            }
        });

        Ok(handle)
    }
}
