pub mod ipc;
pub mod sock;
pub mod event;

use std::{collections::HashMap, fs};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use event::{IPCEventRequest, IPCEventResponse};
use ipc::IPC;
use protocol::event::{ClientEvent, DeamonEvent};
use tokio::{net::UnixStream, sync::mpsc, task::JoinHandle};
use tracing::error;

pub type RequestFromClient = IPCEventRequest<ClientEvent>;
pub type RequestFromServer = IPCEventResponse<DeamonEvent>;

pub struct IPCServer {
    tx: mpsc::Sender<(UnixStream, RequestFromClient)>,
    rx: mpsc::Receiver<(UnixStream, RequestFromClient)>,
    id_map: HashMap<String, UnixStream>,
    listen_handle: Option<JoinHandle<Result<()>>>
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

    pub async fn reply(&self, client_id: &str, msg: RequestFromServer) -> Result<()> {
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
    fn ipc_tx(&self) -> mpsc::Sender<(UnixStream, RequestFromClient)> {
        self.tx.clone()
    }

    fn ipc_rx(&mut self) -> &mut mpsc::Receiver<(UnixStream, RequestFromClient)> {
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
    tx: mpsc::Sender<(UnixStream, RequestFromServer)>,
    rx: mpsc::Receiver<(UnixStream, RequestFromServer)>,
    stream: Option<UnixStream>,
    id: String,
}

impl IPCClient {
    pub fn new() -> Result<Self> {
        if sock::is_sock_exist() {
            let (tx, rx) = Self::get_ipc_channel();
            Ok(Self {
                tx,
                rx,
                stream: None,
                id: uuid::Uuid::new_v4().to_string(),
            })
        } else {
            Err(anyhow!("daemon is not started"))
        }
    }

    pub async fn recv(&mut self) -> DeamonEvent {
        let (s, data) = self.recv_stream().await;
        self.stream = Some(s);
        data.event
    }

    pub async fn send(&mut self, event: ClientEvent) -> Result<()> {
        let Some(stream) = &self.stream else {
            let err: &'static str = "send fail, unix stream object not found";
            error!(err);
            return Err(anyhow!(err));
        };

        self.send_to_stream(stream, &IPCEventRequest {
            client_id: self.id.clone(),
            event,
        }).await
    }
}

#[async_trait]
impl IPC<RequestFromServer, RequestFromClient> for IPCClient {
    fn ipc_tx(&self) -> mpsc::Sender<(UnixStream, RequestFromServer)> {
        self.tx.clone()
    }

    fn ipc_rx(&mut self) -> &mut mpsc::Receiver<(UnixStream, RequestFromServer)> {
        &mut self.rx
    }
}
