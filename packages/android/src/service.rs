use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use async_trait::async_trait;
use common::{auth::LocalExAuthResponse, event::DaemonEvent, peer::DaemonPeer};
use futures::StreamExt;
use libp2p::{
    gossipsub::TopicHash, identity::Keypair, request_response::ResponseChannel, swarm::SwarmEvent,
    PeerId, Swarm,
};
use network::{new_swarm, LocalExBehaviour, LocalExBehaviourEvent};
use protocol::{file::{FileChunk, FileReaderClient, FileTransferClientProtocol}, GossipTopic, LocalExProtocol, LocalExSwarm};
use tokio::sync::{mpsc, Mutex};

use crate::{error::FFIError, ffi::FFIDaemonEvent, get_client_event_receiver, get_quit_rx};

pub struct ServiceManager {
    service: Arc<Mutex<Service>>,
    daemon_tx: mpsc::Sender<FFIDaemonEvent>,
}

impl ServiceManager {
    pub fn new(
        local_keypair: Keypair,
        hostname: String,
        daemon_tx: mpsc::Sender<FFIDaemonEvent>,
    ) -> anyhow::Result<Self> {
        let service = Service::new(local_keypair, hostname, daemon_tx.clone())?;

        Ok(Self {
            service: Arc::new(Mutex::new(service)),
            daemon_tx,
        })
    }

    pub async fn listen(&mut self) -> anyhow::Result<()> {
        let service = self.service.clone();
        let daemon_tx = self.daemon_tx.clone();
        let mut guard = service.lock().await;

        if guard.listen_on().is_err() {
            let _ = daemon_tx
                .send(FFIDaemonEvent::Error(FFIError::ListenLibP2PError))
                .await;
        }

        guard.run().await;
        Ok(())
    }
}

pub struct Service {
    swarm: Swarm<LocalExBehaviour>,
    topics: HashMap<GossipTopic, TopicHash>,
    auth_channels: HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>,
    hostname: String,
    peers: BTreeMap<PeerId, DaemonPeer>,
    daemon_tx: mpsc::Sender<FFIDaemonEvent>,
}

impl Service {
    pub fn new(
        local_keypair: Keypair,
        hostname: String,
        daemon_tx: mpsc::Sender<FFIDaemonEvent>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            swarm: new_swarm(local_keypair)?,
            hostname,
            daemon_tx,
            auth_channels: HashMap::new(),
            topics: HashMap::new(),
            peers: BTreeMap::new(),
        })
    }

    pub async fn run(&mut self) {
        let rx = get_client_event_receiver().unwrap();
        let mut client_rx = rx.lock().await;
        let mut quit_rx = get_quit_rx().unwrap();

        loop {
            tokio::select! {
                _ = async {
                    if let SwarmEvent::Behaviour(event) = self.swarm.select_next_some().await {
                        let _ = match event {
                            LocalExBehaviourEvent::RrAuth(event) => self.handle_auth(event).await,
                            LocalExBehaviourEvent::Gossipsub(event) => self.handle_gossipsub(event).await,
                            LocalExBehaviourEvent::Mdns(event) => self.handle_mdns(event).await,
                            LocalExBehaviourEvent::RrFile(event) => self.handle_file(event).await,
                        };
                    }
                } => {},
                event = client_rx.recv() => if let Some(event) = event {
                    if self.handle_client_event(event.try_into().unwrap()).await.is_err() {
                        let _ = self.daemon_tx.send(FFIDaemonEvent::Error(FFIError::FFIClientEventHandleError)).await;
                    }
                },
                rx = quit_rx.recv()  => if rx.is_ok() {
                    return;
                },
            }
        }
    }
}

impl LocalExSwarm for Service {
    fn swarm(&self) -> &Swarm<LocalExBehaviour> {
        &self.swarm
    }

    fn swarm_mut(&mut self) -> &mut Swarm<LocalExBehaviour> {
        &mut self.swarm
    }
}

#[async_trait]
impl LocalExProtocol for Service {
    fn hostname(&self) -> String {
        self.hostname.clone()
    }

    fn topics_mut(&mut self) -> &mut HashMap<protocol::GossipTopic, TopicHash> {
        &mut self.topics
    }

    fn topics(&self) -> &HashMap<protocol::GossipTopic, TopicHash> {
        &self.topics
    }

    fn auth_channels_mut(&mut self) -> &mut HashMap<PeerId, ResponseChannel<LocalExAuthResponse>> {
        &mut self.auth_channels
    }

    fn peers_mut(&mut self) -> &mut BTreeMap<PeerId, DaemonPeer> {
        &mut self.peers
    }

    fn get_peers(&mut self) -> Vec<DaemonPeer> {
        self.peers.values().cloned().collect()
    }

    /// implement in android side
    fn save_peers(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_remove_peer(&mut self, _: &PeerId) {}

    fn on_add_peer(&mut self, _: PeerId) {}

    async fn send_daemon_event(&mut self, event: DaemonEvent) -> anyhow::Result<()> {
        self.daemon_tx
            .send(event.into())
            .await
            .map_err(anyhow::Error::from)
    }
}

#[async_trait]
impl FileReaderClient for Service {
    async fn read(&mut self, chunk: FileChunk) -> anyhow::Result<()> {
        todo!()
    }

    async fn ready(&mut self, id: String, filename: String, size: usize, chunks: usize, chunk_size: usize) -> anyhow::Result<()> {
        todo!()
    }
}

#[async_trait]
impl FileTransferClientProtocol for Service {
    async fn recv_file(&mut self, chunk: &[u8]) -> anyhow::Result<()> {
        todo!()
    }
}
