use std::{collections::{BTreeMap, HashMap}, sync::Arc};

use async_trait::async_trait;
use common::{
    auth::LocalExAuthResponse,
    event::{ClientEvent, DaemonEvent},
    peer::DaemonPeer,
};
use futures::StreamExt;
use libp2p::{
    gossipsub::TopicHash,
    identity::Keypair,
    request_response::ResponseChannel,
    swarm::SwarmEvent,
    PeerId, Swarm,
};
use network::{new_swarm, LocalExBehaviour, LocalExBehaviourEvent};
use protocol::{GossipTopic, LocalExProtocol};
use tokio::sync::{mpsc, Mutex};

use crate::{error::FFIError, ffi::FFIDaemonEvent};

pub struct ServiceManager {
    service: Arc<Mutex<Service>>,
    client_tx: mpsc::Sender<ClientEvent>,
    client_rx: Option<mpsc::Receiver<ClientEvent>>,
    quit_tx: mpsc::Sender<bool>,
    quit_rx: Option<mpsc::Receiver<bool>>,
    daemon_tx: mpsc::Sender<FFIDaemonEvent>,
}

impl ServiceManager {
    pub fn new(
        local_keypair: Keypair,
        hostname: String,
        daemon_tx: mpsc::Sender<FFIDaemonEvent>,
    ) -> anyhow::Result<Self> {
        let service = Service::new(local_keypair, hostname, daemon_tx.clone())?;
        let (client_tx, client_rx) = mpsc::channel(16);
        let (quit_tx, quit_rx) = mpsc::channel(1);

        Ok(Self {
            service: Arc::new(Mutex::new(service)),
            client_tx,
            client_rx: Some(client_rx),
            quit_tx,
            quit_rx: Some(quit_rx),
            daemon_tx,
        })
    }

    pub async fn dispatch(&self, event: ClientEvent) -> anyhow::Result<()> {
        self.client_tx
            .send(event)
            .await
            .map(|_| ())
            .map_err(|_| anyhow::anyhow!("dispatch error"))
    }

    pub async fn listen(&mut self) -> anyhow::Result<()> {
        if let (Some(client_rx), Some(quit_rx)) = (self.client_rx.take(), self.quit_rx.take()) {
            let service = self.service.clone();
            let daemon_tx = self.daemon_tx.clone();
            let mut guard = service.lock().await;

            if guard.listen_on().is_err() {
                let _ = daemon_tx
                    .send(FFIDaemonEvent::Error(FFIError::ListenLibP2PError))
                    .await;
            }

            guard.run(client_rx, quit_rx).await;
        }

        Ok(())
    }

    pub async fn quit(&self) {
        let _ = self.quit_tx.send(true).await;
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

    pub async fn run(
        &mut self,
        mut client_rx: mpsc::Receiver<ClientEvent>,
        mut quit_rx: mpsc::Receiver<bool>,
    ) {
        loop {
            tokio::select! {
                _ = async {
                    if let SwarmEvent::Behaviour(event) = self.swarm.select_next_some().await {
                        let _ = match event {
                            LocalExBehaviourEvent::RrAuth(event) => self.handle_auth(event).await,
                            LocalExBehaviourEvent::Gossipsub(event) => self.handle_gossipsub(event).await,
                            LocalExBehaviourEvent::Mdns(event) => self.handle_mdns(event).await,
                        };
                    }
                } => {},
                event = client_rx.recv() => if let Some(event) = event {
                    let _ = self.handle_client_event(event).await;
                },
                rx = quit_rx.recv()  => if rx.is_some() {
                    return;
                },
            }
        }
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

    fn swarm(&self) -> &Swarm<LocalExBehaviour> {
        &self.swarm
    }

    fn swarm_mut(&mut self) -> &mut Swarm<LocalExBehaviour> {
        &mut self.swarm
    }

    fn get_peers(&mut self) -> Vec<DaemonPeer> {
        self.peers.values().cloned().collect()
    }

    fn save_peers(&mut self) -> anyhow::Result<()> {
        todo!()
    }

    fn on_remove_peer(&mut self, _: &PeerId) {}

    fn on_add_peer(&mut self, _: PeerId) {}

    async fn send_daemon_event(&mut self, event: DaemonEvent) -> anyhow::Result<()> {
        self.daemon_tx.send(event.into()).await.map_err(anyhow::Error::from)
    }
}
