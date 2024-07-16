use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    net::{UnixListener, UnixStream},
    sync::Mutex,
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
    I: Serialize + DeserializeOwned + Send + Sync,
    O: Serialize + DeserializeOwned + Send + Sync,
{
    async fn handle_incomming_msg(&mut self, stream: UnixStream, msg: I) -> Result<()>;

    async fn handle_stream(&mut self, stream: UnixStream) -> Result<()> {
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
                self.handle_incomming_msg(stream, request).await?;
            }
            _ => return Err(anyhow!("read buffer error")),
        }

        Ok(())
    }

    async fn send<'a>(&self, stream: &'a UnixStream, msg: &'a O) -> Result<()> {
        let writer_buf = WRITER_BUF.with(|w| w.clone());
        let mut writer_buf = writer_buf.lock().await;

        ciborium::ser::into_writer(msg, &mut *writer_buf)?;
        stream.writable().await?;
        stream.try_write(&writer_buf)?;
        Ok(())
    }

    async fn listen(&mut self) -> Result<()> {
        let listener = UnixListener::bind(sock::get_sock_path())?;
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                if let Err(err) = self.handle_stream(stream).await {
                    error!("{err:?}");
                    continue;
                }
            }
        }
    }
}
