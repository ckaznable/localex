use std::{
    collections::{BTreeMap, HashMap}, path::PathBuf, sync::Arc
};

use async_trait::async_trait;
use bimap::BiHashMap;
use common::{auth::LocalExAuthResponse, event::DaemonEvent, peer::DaemonPeer};
use futures::StreamExt;
use network::{new_swarm, LocalExBehaviour};
use protocol::{
    auth::AuthHandler,
    client::ClientHandler,
    file::{
        FileChunk, FileReaderClient, FileTransferClientProtocol, FilesRegisterCenter,
        RegistFileDatabase,
    },
    message::{GossipTopic, GossipTopicManager, GossipsubHandler, SyncOfferCollector, SyncRequestItem},
    AbortListener, EventEmitter, LocalExContentProvider, LocalExProtocol, LocalExProtocolAction,
    LocalExSwarm, PeersManager,
};
use protocol::{
    database::LocalExDb,
    libp2p::{
        bytes::Bytes, gossipsub::TopicHash, identity::Keypair, request_response::ResponseChannel,
        swarm::SwarmEvent, PeerId, Swarm,
    },
};
use tokio::{runtime::Runtime, sync::{mpsc::{self, Receiver, Sender}, Mutex, RwLock}, task::JoinHandle};

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
        fs_dir: PathBuf,
    ) -> anyhow::Result<Self> {
        let service = Service::new(local_keypair, hostname, daemon_tx.clone(), fs_dir)?;

        Ok(Self {
            service: Arc::new(Mutex::new(service)),
            daemon_tx,
        })
    }

    pub async fn listen(&mut self) -> anyhow::Result<()> {
        let service = self.service.clone();
        let daemon_tx = self.daemon_tx.clone();
        let mut guard = service.lock().await;

        if guard.prepare().is_err() {
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
    topics: BiHashMap<TopicHash, GossipTopic>,
    auth_channels: HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>,
    hostname: String,
    peers: BTreeMap<PeerId, DaemonPeer>,
    daemon_tx: mpsc::Sender<FFIDaemonEvent>,
    files_register_store: HashMap<String, Bytes>,
    sync_offer_collector: Arc<RwLock<Option<SyncOfferCollector>>>,
    sync_offer_tx: Sender<Vec<SyncRequestItem>>,
    sync_offer_rx: Option<Receiver<Vec<SyncRequestItem>>>,
    sync_offer_timer_handler: Option<JoinHandle<()>>,
    db: LocalExDb,
}

impl Service {
    pub fn new(
        local_keypair: Keypair,
        hostname: String,
        daemon_tx: mpsc::Sender<FFIDaemonEvent>,
        fs_dir: PathBuf,
    ) -> anyhow::Result<Self> {
        let (sync_offer_tx, sync_offer_rx) = mpsc::channel(1);

        let rt  = Runtime::new()?;
        let db = rt.block_on(async {
            LocalExDb::new(Some(fs_dir)).await
        })?;
        drop(rt);

        Ok(Self {
            swarm: new_swarm(local_keypair)?,
            hostname,
            daemon_tx,
            auth_channels: HashMap::new(),
            topics: BiHashMap::new(),
            peers: BTreeMap::new(),
            files_register_store: HashMap::new(),
            sync_offer_collector: Arc::new(RwLock::new(None)),
            sync_offer_tx,
            sync_offer_rx: Some(sync_offer_rx),
            sync_offer_timer_handler: None,
            db,
        })
    }

    pub async fn run(&mut self) {
        let rx = get_client_event_receiver().unwrap();
        let mut client_rx = rx.lock().await;
        let mut quit_rx = get_quit_rx().unwrap();
        let mut sync_offer_rx = self.sync_offer_rx.take().unwrap();

        loop {
            tokio::select! {
                _ = async {
                    if let SwarmEvent::Behaviour(event) = self.swarm.select_next_some().await {
                        let _ = self.handle_event(event).await;
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
                sync_offer = sync_offer_rx.recv() => if let Some(sync_offer) = sync_offer {
                    self.handle_sync_offers(sync_offer).await
                }
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
impl EventEmitter<DaemonEvent> for Service {
    async fn emit_event(&mut self, event: DaemonEvent) -> anyhow::Result<()> {
        self.daemon_tx
            .send(event.into())
            .await
            .map_err(anyhow::Error::from)
    }
}

impl LocalExContentProvider for Service {
    fn hostname(&self) -> String {
        self.hostname.clone()
    }
}

impl PeersManager for Service {
    fn peers_mut(&mut self) -> &mut BTreeMap<PeerId, DaemonPeer> {
        &mut self.peers
    }

    fn peers(&self) -> &BTreeMap<PeerId, DaemonPeer> {
        &self.peers
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
}

impl AuthHandler for Service {
    fn auth_channels_mut(&mut self) -> &mut HashMap<PeerId, ResponseChannel<LocalExAuthResponse>> {
        &mut self.auth_channels
    }
}

impl GossipTopicManager for Service {
    fn topics_mut(&mut self) -> &mut BiHashMap<TopicHash, GossipTopic> {
        &mut self.topics
    }

    fn topics(&self) -> &BiHashMap<TopicHash, GossipTopic> {
        &self.topics
    }
}

impl GossipsubHandler for Service {
    fn sync_offer_sender(&self) -> Sender<Vec<SyncRequestItem>> {
        self.sync_offer_tx.clone()
    }

    fn sync_offer_timer_handler(&mut self) -> &mut Option<JoinHandle<()>> {
        &mut self.sync_offer_timer_handler
    }

    fn sync_offer_collector(&self) -> Arc<RwLock<Option<SyncOfferCollector>>> {
        self.sync_offer_collector.clone()
    }
}

impl ClientHandler for Service {}
impl LocalExProtocolAction for Service {}

impl RegistFileDatabase for Service {
    fn db(&self) -> &LocalExDb {
        &self.db
    }
}

#[async_trait]
impl LocalExProtocol for Service {}

#[async_trait]
impl FileReaderClient for Service {
    async fn read(
        &mut self,
        session: &str,
        app_id: &str,
        file_id: &str,
        chunk: FileChunk,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn ready(
        &mut self,
        session: &str,
        app_id: &str,
        file_id: &str,
        size: usize,
        chunk_size: usize,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn done(&mut self, session: &str, app_id: &str, file_id: &str) -> anyhow::Result<()> {
        todo!()
    }
}

impl AbortListener for Service {
    fn abort_rx(&self) -> tokio::sync::broadcast::Receiver<()> {
        get_quit_rx().unwrap()
    }
}

impl FilesRegisterCenter for Service {
    fn raw_store(&self) -> &HashMap<String, Bytes> {
        &self.files_register_store
    }

    fn raw_store_mut(&mut self) -> &mut HashMap<String, Bytes> {
        &mut self.files_register_store
    }
}

#[async_trait]
impl FileTransferClientProtocol for Service {}
