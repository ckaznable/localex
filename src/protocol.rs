use std::collections::HashMap;

use libp2p::PeerId;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PeerVerifyState {
    Verified,
    Blocked,
    #[default]
    WaitingVerification,
}

impl PeerVerifyState {
    pub fn should_skip_verify(&self) -> bool {
        *self == Self::Verified || *self == Self::Blocked
    }
}

#[derive(Clone)]
pub struct RemotePeer {
    pub peer_id: PeerId,
    pub state: PeerVerifyState,
    pub hostname: Option<String>,
}

impl RemotePeer {
    fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            state: PeerVerifyState::default(),
            hostname: None,
        }
    }
}

pub struct LocalExProtocol {
    peers: HashMap<PeerId, RemotePeer>,
}

impl LocalExProtocol {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, peer_id: PeerId) {
        if self.peers.contains_key(&peer_id) {
            return;
        }

        self.peers.insert(peer_id, RemotePeer::new(peer_id));
    }

    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id);
    }

    pub async fn handle_auth(&mut self, peer_id: PeerId) -> bool {
        let Some(peer) = self.peers.get(&peer_id) else {
            return false;
        };

        if peer.state.should_skip_verify() {
            return false;
        }

        self.verify(peer).await
    }

    pub async fn verify(&self, _: &RemotePeer) -> bool {
        true
    }

    pub fn verified(&mut self, peer_id: &PeerId, hostname: String) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.state = PeerVerifyState::Verified;
            peer.hostname = Some(hostname);
        }
    }
}
