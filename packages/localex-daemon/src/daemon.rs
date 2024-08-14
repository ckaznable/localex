use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use anyhow::Result;
use async_trait::async_trait;
use common::{auth::LocalExAuthResponse, event::DaemonEvent, peer::DaemonPeer};
use futures::StreamExt;
use libp2p::{
    gossipsub::TopicHash, identity::Keypair, request_response::ResponseChannel, swarm::SwarmEvent,
    PeerId, Swarm,
};
use localex_ipc::IPCServer;
use network::LocalExBehaviour;
use protocol::{file::{FileChunk, FileReaderClient, FileTransferClientProtocol}, AbortListener, GossipTopic, LocalExProtocol, LocalExSwarm};
use tokio::sync::broadcast;
use tracing::error;

use crate::store::DaemonDataStore;

pub struct Daemon {
    swarm: Swarm<LocalExBehaviour>,
    server: IPCServer,
    topics: HashMap<GossipTopic, TopicHash>,
    auth_channels: HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>,
    hostname: String,
    ctrlc_rx: broadcast::Receiver<()>,
    store: Box<dyn DaemonDataStore + Send + Sync>,
}

impl Daemon {
    pub fn new(
        local_keypair: Keypair,
        hostname: String,
        sock: Option<PathBuf>,
        store: Box<dyn DaemonDataStore + Send + Sync>,
    ) -> Result<Self> {
        let (tx, rx) = broadcast::channel(0);
        let _ = ctrlc::set_handler(move || {
            tx.send(()).expect("close application error");
        });

        let server = IPCServer::new(sock, rx.resubscribe())?;
        let swarm = network::new_swarm(local_keypair)?;

        Ok(Self {
            swarm,
            server,
            hostname,
            store,
            topics: HashMap::new(),
            auth_channels: HashMap::new(),
            ctrlc_rx: rx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.server.prepare().await?;

        loop {
            tokio::select! {
                event = self.server.recv() => {
                    if let Err(e) = self.handle_client_event(event).await {
                        error!("client event error: {e:?}");
                    }
                },
                _ = self.ctrlc_rx.recv() => {
                    self.server.release();
                    return Ok(())
                },
                swarm_event = self.swarm.select_next_some() => if let SwarmEvent::Behaviour(event) = swarm_event {
                    if let Err(e) = self.handle_event(event).await {
                        error!("swarm event error: {e:?}");
                    }
                }
            }
        }
    }
}

impl LocalExSwarm for Daemon {
    fn swarm(&self) -> &Swarm<LocalExBehaviour> {
        &self.swarm
    }

    fn swarm_mut(&mut self) -> &mut Swarm<LocalExBehaviour> {
        &mut self.swarm
    }
}

#[async_trait]
impl LocalExProtocol for Daemon {
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
        self.store.get_peers_mut()
    }

    fn get_peers(&mut self) -> Vec<DaemonPeer> {
        self.store.get_peers().values().cloned().collect()
    }

    fn save_peers(&mut self) -> Result<()> {
        self.store.save_peers()
    }

    fn on_remove_peer(&mut self, _: &PeerId) { }

    fn on_add_peer(&mut self, _: PeerId) { }

    async fn send_daemon_event(&mut self, event: DaemonEvent) -> Result<()> {
        self.server.broadcast(event).await;
        Ok(())
    }
}


#[async_trait]
impl FileReaderClient for Daemon {
    async fn read(&mut self, session: &str, chunk: FileChunk) -> anyhow::Result<()> {
        todo!()
    }

    async fn ready(&mut self, session: &str, id: &str, filename: &str, size: usize, chunks: usize, chunk_size: usize) -> anyhow::Result<()> {
        todo!()
    }

    async fn done(&mut self, session: &str) {
        todo!()
    }
}

impl AbortListener for Daemon {
    fn abort_rx(&self) -> broadcast::Receiver<()> {
        self.ctrlc_rx.resubscribe()
    }
}

#[async_trait]
impl FileTransferClientProtocol for Daemon {
    fn get_file_path_with_id(&self, id: &str) -> PathBuf {
        todo!()
    }
}
