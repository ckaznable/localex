use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use bimap::BiHashMap;
use common::{auth::LocalExAuthResponse, event::DaemonEvent, peer::DaemonPeer};
use futures::StreamExt;
use localex_ipc::IPCServer;
use network::LocalExBehaviour;
use protocol::{
    auth::AuthHandler,
    client::ClientHandler,
    file::{
        FileChunk, FileReaderClient, FileTransferClientProtocol, FilesRegisterCenter,
        RegistFileDatabase,
    },
    message::{GossipTopic, GossipTopicManager, GossipsubHandler},
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
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::{reader::FileHandleManager, store::DaemonDataStore};

pub struct Daemon {
    swarm: Swarm<LocalExBehaviour>,
    server: IPCServer,
    db: LocalExDb,
    topics: BiHashMap<TopicHash, GossipTopic>,
    auth_channels: HashMap<PeerId, ResponseChannel<LocalExAuthResponse>>,
    hostname: String,
    ctrlc_rx: broadcast::Receiver<()>,
    store: Box<dyn DaemonDataStore + Send + Sync>,
    raw_data_register: HashMap<String, Bytes>,
    file_reader_manager: FileHandleManager,
}

impl Daemon {
    pub async fn new(
        local_keypair: Keypair,
        hostname: String,
        sock: Option<PathBuf>,
        store: Box<dyn DaemonDataStore + Send + Sync>,
    ) -> Result<Self> {
        let db = LocalExDb::new(None).await?;

        let (tx, rx) = broadcast::channel(1);
        let _ = ctrlc::set_handler(move || {
            tx.send(()).expect("close application error");
        });

        let server = IPCServer::new(sock, rx.resubscribe())?;
        let swarm = network::new_swarm(local_keypair)?;

        Ok(Self {
            swarm,
            server,
            db,
            hostname,
            store,
            topics: BiHashMap::new(),
            auth_channels: HashMap::new(),
            raw_data_register: HashMap::new(),
            ctrlc_rx: rx,
            file_reader_manager: FileHandleManager::default(),
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
impl EventEmitter<DaemonEvent> for Daemon {
    async fn emit_event(&mut self, event: DaemonEvent) -> Result<()> {
        self.server.broadcast(event).await;
        Ok(())
    }
}

impl LocalExContentProvider for Daemon {
    fn hostname(&self) -> String {
        self.hostname.clone()
    }
}

impl PeersManager for Daemon {
    fn peers_mut(&mut self) -> &mut BTreeMap<PeerId, DaemonPeer> {
        self.store.get_peers_mut()
    }

    fn get_peers(&mut self) -> Vec<DaemonPeer> {
        self.store.get_peers().values().cloned().collect()
    }

    fn save_peers(&mut self) -> Result<()> {
        self.store.save_peers()
    }

    fn on_remove_peer(&mut self, _: &PeerId) {}

    fn on_add_peer(&mut self, _: PeerId) {}
}

impl AuthHandler for Daemon {
    fn auth_channels_mut(&mut self) -> &mut HashMap<PeerId, ResponseChannel<LocalExAuthResponse>> {
        &mut self.auth_channels
    }
}

impl GossipTopicManager for Daemon {
    fn topics_mut(&mut self) -> &mut BiHashMap<TopicHash, GossipTopic> {
        &mut self.topics
    }

    fn topics(&self) -> &BiHashMap<TopicHash, GossipTopic> {
        &self.topics
    }
}

impl AbortListener for Daemon {
    fn abort_rx(&self) -> broadcast::Receiver<()> {
        self.ctrlc_rx.resubscribe()
    }
}

impl RegistFileDatabase for Daemon {
    fn db(&self) -> &LocalExDb {
        &self.db
    }
}

impl FilesRegisterCenter for Daemon {
    fn raw_store(&self) -> &HashMap<String, Bytes> {
        &self.raw_data_register
    }

    fn raw_store_mut(&mut self) -> &mut HashMap<String, Bytes> {
        &mut self.raw_data_register
    }
}

impl GossipsubHandler for Daemon {}
impl ClientHandler for Daemon {}
impl LocalExProtocolAction for Daemon {}

#[async_trait]
impl FileTransferClientProtocol for Daemon {}
#[async_trait]
impl LocalExProtocol for Daemon {}

#[async_trait]
impl FileReaderClient for Daemon {
    async fn read(
        &mut self,
        session: &str,
        app_id: &str,
        file_id: &str,
        chunk: FileChunk,
    ) -> Result<()> {
        let handler = self
            .file_reader_manager
            .get(session, &(app_id.to_string(), file_id.to_string()))
            .ok_or_else(|| anyhow!("session and id not found"))?;

        let mut writer = handler.write().await;
        writer.write(&chunk.chunk, chunk.offset).await
    }

    async fn ready(
        &mut self,
        session: &str,
        app_id: &str,
        file_id: &str,
        size: usize,
        chunk_size: usize,
    ) -> Result<()> {
        info!("file reader ready {session}:{app_id}:{file_id}");
        self.file_reader_manager
            .add(
                session.to_string(),
                (app_id.to_string(), file_id.to_string()),
                size,
                chunk_size,
            )
            .await
    }

    async fn done(&mut self, session: &str, app_id: &str, file_id: &str) -> Result<()> {
        let id = (app_id.to_string(), file_id.to_string());
        let handler = self
            .file_reader_manager
            .get(session, &id)
            .ok_or_else(|| anyhow!("session and id not found"))?;
        let mut handler = handler.write().await;
        let tmp_file_path = handler.get_file_path()?;

        self.file_reader_manager.remove(session, &id);
        self.emit_event(DaemonEvent::FileUpdated(
            app_id.to_string(),
            file_id.to_string(),
            tmp_file_path.to_string_lossy().to_string(),
        ))
        .await?;
        Ok(())
    }
}
