pub mod ipc;
pub mod sock;
pub mod event;

use std::{collections::HashMap, fs};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use event::{IPCEventRequest, IPCEventResponse};
use ipc::IPC;
use protocol::event::{ClientEvent, DeamonEvent};
use tokio::{net::UnixStream, sync::mpsc};

pub type RequestFromClient = IPCEventRequest<ClientEvent>;
pub type RequestFromServer = IPCEventResponse<DeamonEvent>;

pub struct IPCServer {
    tx: mpsc::Sender<RequestFromClient>,
    id_map: HashMap<String, UnixStream>,
}

impl IPCServer {
    pub fn new(tx: mpsc::Sender<RequestFromClient>) -> Result<Self> {
        if sock::is_sock_exist() {
            Err(anyhow!("application has already running"))
        } else {
            Ok(Self {
                tx,
                id_map: HashMap::new(),
            })
        }
    }

    pub async fn reply(&self, client_id: &str, msg: RequestFromServer) -> Result<()> {
        let stream = self.id_map.get(client_id).ok_or(anyhow!("reply error"))?;
        self.send(stream, &msg).await
    }

    pub async fn broadcast(&self, msg: DeamonEvent) {
        let msg = IPCEventResponse::from(msg);
        for s in self.id_map.values() {
            let _ = self.send(s, &msg).await;
        }
    }
}

#[async_trait]
impl IPC<RequestFromClient, RequestFromServer> for IPCServer {
    async fn handle_incomming_msg(&mut self, stream: UnixStream, msg: RequestFromClient) -> Result<()> {
        self.id_map.insert(msg.client_id.clone(), stream);
        self.tx.send(msg).await?;
        Ok(())
    }
}

impl Drop for IPCServer {
    fn drop(&mut self) {
        self.id_map.drain().for_each(|(_, s)| drop(s));
        let _ = fs::remove_file(sock::get_sock_path());
    }
}

pub struct IPCClient {
    tx: mpsc::Sender<RequestFromServer>,
    rx: mpsc::Receiver<RequestFromServer>,
}

impl IPCClient {
    pub fn new() -> Result<Self> {
        if sock::is_sock_exist() {
            let (tx, rx) = mpsc::channel(32);
            Ok(Self { tx, rx })
        } else {
            Err(anyhow!("daemon is not started"))
        }
    }

    pub async fn recv(&mut self) -> RequestFromServer {
        loop {
            if let Some(data) = self.rx.recv().await {
                return data
            }
        }
    }
}

#[async_trait]
impl IPC<RequestFromServer, RequestFromClient> for IPCClient {
    async fn handle_incomming_msg(&mut self, _: UnixStream, msg: RequestFromServer) -> Result<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
}
