use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use ciborium::{from_reader, into_writer};
use futures::executor::block_on;
use libp2p::{identity::Keypair, PeerId};
use common::peer::DaemonPeer;
use secret_service::{EncryptionType, SecretService};

const SECRET_LABEL: &str = "LocalEx";
const ATTR_KEY: &str = "localex";
const PRIVATE_KEY_ATTR: (&str, &str) = (ATTR_KEY, "keypair");
const PAIR_PEERS_ATTR: (&str, &str) = (ATTR_KEY, "piar_peers");

pub trait LocalKeyStore {
    fn get_local_key(&self) -> Option<Keypair> { None }
    fn save_local_key(&self, _: &Keypair) -> Result<()> { Ok(()) }
}

pub trait PiarPeersStore {
    fn get_peers(&mut self) -> &BTreeMap<PeerId, DaemonPeer>;
    fn get_peers_mut(&mut self) -> &mut BTreeMap<PeerId, DaemonPeer>;
    fn save_peers(&mut self) -> Result<()>;
    fn add_peer(&mut self, peer: DaemonPeer);
    fn remove_peer(&mut self, peer: &PeerId);
}

pub trait DaemonDataStore: LocalKeyStore + PiarPeersStore {}

#[derive(Default)]
pub struct DefaultStore(pub BTreeMap<PeerId, DaemonPeer>);
impl DaemonDataStore for DefaultStore {}
impl LocalKeyStore for DefaultStore {}

impl PiarPeersStore for DefaultStore {
    #[inline]
    fn get_peers(&mut self) -> &BTreeMap<PeerId, DaemonPeer> {
        &self.0
    }

    #[inline]
    fn get_peers_mut(&mut self) -> &mut BTreeMap<PeerId, DaemonPeer> {
        &mut self.0
    }

    #[inline]
    fn save_peers(&mut self) -> Result<()> {
        Ok(())
    }

    #[inline]
    fn add_peer(&mut self, peer: DaemonPeer) {
        self.0.insert(peer.peer_id, peer);
    }

    #[inline]
    fn remove_peer(&mut self, peer: &PeerId) {
        self.0.remove(peer);
    }
}

pub struct SecretStore<'a> {
    service: SecretService<'a>,
    peers: BTreeMap<PeerId, DaemonPeer>,
    was_peers_loaded: bool,
}

impl<'a> SecretStore<'a> {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            service: SecretService::connect(EncryptionType::Dh).await?,
            peers: BTreeMap::new(),
            was_peers_loaded: false,
        })
    }

    async fn get_secret(&self, attr: HashMap<&str, &str>) -> Option<Vec<u8>> {
        let search_items = self
            .service
            .search_items(attr)
            .await
            .ok()?;

        let item = search_items.unlocked.first()?;
        item.get_secret().await.ok()
    }

    async fn save_secret(&self, attr: HashMap<&str, &str>, secret: &[u8]) -> Result<()> {
        let collection = self.service.get_default_collection().await?;
        collection
            .create_item(
                SECRET_LABEL,
                attr,
                secret,
                true,
                "text/plain",
            )
            .await?;

        Ok(())
    }

    #[inline]
    async fn get_secret_with_one_attr(&self, attr: (&str, &str)) -> Option<Vec<u8>> {
        self.get_secret(HashMap::from([attr])).await
    }

    #[inline]
    async fn save_secret_with_one_attr(&self, attr: (&str, &str), secret: &[u8]) -> Result<()> {
        self.save_secret(HashMap::from([attr]), secret).await
    }

    async fn peers_init(&mut self) {
        if self.was_peers_loaded {
            return;
        }

        self.was_peers_loaded = true;
        self.peers = self
            .get_secret_with_one_attr(PAIR_PEERS_ATTR)
            .await
            .and_then(|data| {
                let data: &[u8] = data.as_ref();
                let peers: BTreeMap<PeerId, DaemonPeer> = from_reader(data).ok()?;
                Some(peers)
            })
            .unwrap_or_else(BTreeMap::new);
    }
}

impl<'a> DaemonDataStore for SecretStore<'a> {}

impl<'a> LocalKeyStore for SecretStore<'a> {
    #[inline]
    fn get_local_key(&self) -> Option<Keypair> {
        block_on(async {
            self.get_secret_with_one_attr(PRIVATE_KEY_ATTR).await
                .and_then(|secret| Keypair::from_protobuf_encoding(&secret).ok())
                .or_else(|| Some(Keypair::generate_ed25519()))
        })
    }

    #[inline]
    fn save_local_key(&self, keypair: &Keypair) -> Result<()> {
        block_on(async {
            let key = keypair.to_protobuf_encoding()?;
            self.save_secret_with_one_attr(PRIVATE_KEY_ATTR, &key).await
        })
    }
}

impl<'a> PiarPeersStore for SecretStore<'a> {
    #[inline]
    fn get_peers_mut(&mut self) -> &mut BTreeMap<PeerId, DaemonPeer> {
        block_on(async {
            self.peers_init().await;
        });

        &mut self.peers
    }

    #[inline]
    fn get_peers(&mut self) -> &BTreeMap<PeerId, DaemonPeer> {
        block_on(async {
            self.peers_init().await;
        });

        &self.peers
    }

    #[inline]
    fn save_peers(&mut self) -> Result<()> {
        block_on(async {
            let mut data = Vec::with_capacity(1024);
            into_writer(&self.peers, &mut data)?;
            self.save_secret_with_one_attr(PAIR_PEERS_ATTR, &data).await
        })
    }

    #[inline]
    fn add_peer(&mut self, peer: DaemonPeer) {
        self.peers.insert(peer.peer_id, peer);
    }

    #[inline]
    fn remove_peer(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id);
    }
}
