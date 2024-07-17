pub mod ipc;
pub mod sock;
pub mod event;

use std::{collections::HashMap, fs};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use event::{IPCEventRequest, IPCEventResponse};
use ipc::IPC;
use tokio::{net::UnixStream, sync::mpsc};

pub struct Server {
    tx: mpsc::Sender<IPCEventRequest>,
    id_map: HashMap<String, UnixStream>,
}

impl Server {
    pub fn new(tx: mpsc::Sender<IPCEventRequest>) -> Result<Self> {
        if sock::is_sock_exist() {
            Err(anyhow!("application has already running"))
        } else {
            Ok(Self {
                tx,
                id_map: HashMap::new(),
            })
        }
    }

    pub async fn reply(&self, client_id: &str, msg: IPCEventResponse) -> Result<()> {
        let stream = self.id_map.get(client_id).ok_or(anyhow!("reply error"))?;
        self.send(stream, &msg).await
    }
}

#[async_trait]
impl IPC<IPCEventRequest, IPCEventResponse> for Server {
    async fn handle_incomming_msg(&mut self, stream: UnixStream, msg: IPCEventRequest) -> Result<()> {
        self.id_map.insert(msg.client_id.clone(), stream);
        self.tx.send(msg).await?;
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.id_map.drain().for_each(|(_, s)| drop(s));
        let _ = fs::remove_file(sock::get_sock_path());
    }
}

pub struct Client {
    tx: mpsc::Sender<IPCEventResponse>,
    rx: mpsc::Receiver<IPCEventResponse>,
}

impl Client {
    pub fn new() -> Result<Self> {
        if sock::is_sock_exist() {
            let (tx, rx) = mpsc::channel(32);
            Ok(Self { tx, rx })
        } else {
            Err(anyhow!("daemon is not started"))
        }
    }

    pub async fn recv(&mut self) -> IPCEventResponse {
        loop {
            if let Some(data) = self.rx.recv().await {
                return data
            }
        }
    }
}

#[async_trait]
impl IPC<IPCEventResponse, IPCEventRequest> for Client {
    async fn handle_incomming_msg(&mut self, _: UnixStream, msg: IPCEventResponse) -> Result<()> {
        self.tx.send(msg).await?;
        Ok(())
    }
}
