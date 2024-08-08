pub mod event;
pub mod ipc;
pub mod sock;

use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use event::{IPCEventRequest, IPCEventResponse};
use ipc::{IPCMsgPack, IPC};
use common::event::{ClientEvent, DaemonEvent};
use tokio::{
    net::unix::{OwnedReadHalf, OwnedWriteHalf},
    sync::mpsc,
    task::JoinHandle,
};
use tracing::{error, info};

pub type RequestFromClient = IPCEventRequest<ClientEvent>;
pub type RequestFromServer = IPCEventResponse<DaemonEvent>;

pub struct IPCServer {
    tx: mpsc::Sender<IPCMsgPack<RequestFromClient>>,
    rx: mpsc::Receiver<IPCMsgPack<RequestFromClient>>,
    id_map: HashMap<String, Arc<OwnedWriteHalf>>,
    listen_handle: Option<JoinHandle<Result<()>>>,
    sock: PathBuf,
}

impl IPCServer {
    pub fn new(sock_path: Option<PathBuf>) -> Result<Self> {
        let sock_path = sock_path.unwrap_or_else(|| sock::get_sock_mount_path().into());

        if sock_path.exists() {
            Err(anyhow!("application has already running"))
        } else {
            let (tx, rx) = Self::get_ipc_channel();
            Ok(Self {
                tx,
                rx,
                sock: sock_path,
                id_map: HashMap::new(),
                listen_handle: None,
            })
        }
    }

    pub async fn prepare(&mut self) -> Result<()> {
        self.listen_handle = Some(self.listen(self.sock.clone()).await?);
        Ok(())
    }

    pub async fn reply(&mut self, client_id: &str, msg: RequestFromServer) -> Result<()> {
        let stream = self.id_map.get(client_id).ok_or(anyhow!("reply error"))?;
        self.send_to_stream(stream, &msg).await
    }

    pub async fn broadcast(&mut self, msg: DaemonEvent) {
        let msg = IPCEventResponse::from(msg);
        let mut disconnect_stream = vec![];
        for (k, s) in self.id_map.iter() {
            if let Err(e) = self.send_to_stream(s, &msg).await {
                disconnect_stream.push(k.clone());
                error!("send msg error: {e:?}");
            }
        }

        if !disconnect_stream.is_empty() {
            info!("{} client disconnected", disconnect_stream.len());
        }

        disconnect_stream
            .iter()
            .for_each(|k| {
                self.id_map.remove(k);
            });
    }

    pub async fn recv(&mut self) -> ClientEvent {
        let (stream, data) = self.recv_stream().await;
        if !self.id_map.contains_key(&data.client_id) {
            info!("{} connected", &data.client_id);
        }

        self.id_map.insert(data.client_id, stream);
        data.event
    }

    pub fn release(&mut self) {
        self.id_map.drain().for_each(|(_, s)| drop(s));
        let _ = fs::remove_file(&self.sock);
        if let Some(handle) = self.listen_handle.take() {
            handle.abort();
        }
    }
}

#[async_trait]
impl IPC<RequestFromClient, RequestFromServer> for IPCServer {
    fn ipc_tx(&self) -> mpsc::Sender<IPCMsgPack<RequestFromClient>> {
        self.tx.clone()
    }

    fn ipc_rx(&mut self) -> &mut mpsc::Receiver<IPCMsgPack<RequestFromClient>> {
        &mut self.rx
    }
}

impl Drop for IPCServer {
    fn drop(&mut self) {
        self.release();
    }
}

pub struct IPCClient {
    tx: mpsc::Sender<IPCMsgPack<RequestFromServer>>,
    rx: mpsc::Receiver<IPCMsgPack<RequestFromServer>>,
    stream_read: Option<Arc<OwnedReadHalf>>,
    stream_write: Arc<OwnedWriteHalf>,
    stream_handle: Option<JoinHandle<Result<()>>>,
    id: String,
}

impl IPCClient {
    pub async fn new(sock_path: Option<PathBuf>) -> Result<Self> {
        let sock_path = sock_path.unwrap_or_else(|| sock::get_sock_mount_path().into());
        if sock_path.exists() {
            let (tx, rx) = Self::get_ipc_channel();
            let stream = Self::connect(&sock_path).await?;
            let (read, write) = stream.into_split();

            Ok(Self {
                tx,
                rx,
                stream_read: Some(Arc::new(read)),
                stream_write: Arc::new(write),
                stream_handle: None,
                id: uuid::Uuid::new_v4().to_string(),
            })
        } else {
            Err(anyhow!("daemon is not started"))
        }
    }

    pub async fn prepare(&mut self) -> Result<()> {
        if let Some(r) = self.stream_read.take() {
            self.stream_handle = self.wait_for_stream(r, self.stream_write.clone()).await.ok();
        }

        Ok(())
    }

    pub async fn recv(&mut self) -> DaemonEvent {
        let (_, data) = self.recv_stream().await;
        data.event
    }

    pub async fn send(&mut self, event: ClientEvent) {
        if let Err(e) = self.send_to_stream(
            &self.stream_write,
            &IPCEventRequest {
                client_id: self.id.clone(),
                event,
            },
        )
        .await {
            error!("{e:?}");
        }
    }
}

#[async_trait]
impl IPC<RequestFromServer, RequestFromClient> for IPCClient {
    fn ipc_tx(&self) -> mpsc::Sender<IPCMsgPack<RequestFromServer>> {
        self.tx.clone()
    }

    fn ipc_rx(&mut self) -> &mut mpsc::Receiver<IPCMsgPack<RequestFromServer>> {
        &mut self.rx
    }
}
