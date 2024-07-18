pub mod event;
pub mod ipc;
pub mod sock;

use std::{collections::HashMap, fs, sync::Arc};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use event::{IPCEventRequest, IPCEventResponse};
use ipc::{IPCMsgPack, IPC};
use protocol::event::{ClientEvent, DeamonEvent};
use tokio::{
    net::unix::{OwnedReadHalf, OwnedWriteHalf},
    sync::mpsc,
    task::JoinHandle,
};
use tracing::error;

pub type RequestFromClient = IPCEventRequest<ClientEvent>;
pub type RequestFromServer = IPCEventResponse<DeamonEvent>;

pub struct IPCServer {
    tx: mpsc::Sender<IPCMsgPack<RequestFromClient>>,
    rx: mpsc::Receiver<IPCMsgPack<RequestFromClient>>,
    id_map: HashMap<String, Arc<OwnedWriteHalf>>,
    listen_handle: Option<JoinHandle<Result<()>>>,
}

impl IPCServer {
    pub fn new() -> Result<Self> {
        if sock::is_sock_exist() {
            Err(anyhow!("application has already running"))
        } else {
            let (tx, rx) = Self::get_ipc_channel();
            Ok(Self {
                tx,
                rx,
                id_map: HashMap::new(),
                listen_handle: None,
            })
        }
    }

    pub async fn prepare(&mut self) -> Result<()> {
        self.listen_handle = Some(self.listen().await?);
        Ok(())
    }

    pub async fn reply(&mut self, client_id: &str, msg: RequestFromServer) -> Result<()> {
        let stream = self.id_map.get(client_id).ok_or(anyhow!("reply error"))?;
        self.send_to_stream(stream, &msg).await
    }

    pub async fn broadcast(&self, msg: DeamonEvent) {
        let msg = IPCEventResponse::from(msg);
        for s in self.id_map.values() {
            if let Err(e) = self.send_to_stream(s, &msg).await {
                error!("send msg error: {e:?}");
            }
        }
    }

    pub async fn recv(&mut self) -> ClientEvent {
        let (stream, data) = self.recv_stream().await;
        self.id_map.insert(data.client_id, stream);
        data.event
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
        self.id_map.drain().for_each(|(_, s)| drop(s));
        let _ = fs::remove_file(sock::get_sock_mount_path());
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
    pub async fn new() -> Result<Self> {
        if sock::is_sock_exist() {
            let (tx, rx) = Self::get_ipc_channel();
            let stream = Self::connect().await?;
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

    pub async fn recv(&mut self) -> DeamonEvent {
        let (_, data) = self.recv_stream().await;
        data.event
    }

    pub async fn send(&mut self, event: ClientEvent) -> Result<()> {
        self.send_to_stream(
            &self.stream_write,
            &IPCEventRequest {
                client_id: self.id.clone(),
                event,
            },
        )
        .await
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
