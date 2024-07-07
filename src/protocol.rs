use std::collections::HashMap;

use libp2p::PeerId;

const RETRY_LIMIT: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PeerVerifyState {
    Verified,
    Blocked,
    WaitingVerification(u8),
}

pub struct RemotePeer {
    peer_id: PeerId,
    state: PeerVerifyState,
}

impl RemotePeer {
    fn new(peer_id: PeerId) -> Self {
        let state = PeerVerifyState::WaitingVerification(0);

        Self {
            peer_id,
            state,
        }
    }

    fn retry(&mut self) {
        self.state = match self.state {
            PeerVerifyState::WaitingVerification(c) => {
                if c >= RETRY_LIMIT {
                    PeerVerifyState::Blocked
                } else {
                    PeerVerifyState::WaitingVerification(c.saturating_add(1))
                }
            },
            s => s
        };
    }
}

pub struct LocalExProtocol {
    peers: HashMap<PeerId, RemotePeer>,
}

impl LocalExProtocol {
    pub fn new(local_peer_id: PeerId) -> Self {
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
        if let Some(true) = self.is_blocked(peer_id) {
            return;
        }

        self.peers.remove(peer_id);
    }

    pub fn verify(&mut self, peer_id: &PeerId, topic: &str, password: &[u8]) -> bool {
        todo!()
    }

    pub fn is_verified(&self, peer_id: &PeerId) -> Option<bool> {
        let peer = self.peers.get(peer_id)?;
        Some(peer.state == PeerVerifyState::Verified)
    }

    pub fn is_blocked(&self, peer_id: &PeerId) -> Option<bool> {
        let peer = self.peers.get(peer_id)?;
        Some(peer.state == PeerVerifyState::Blocked)
    }
}
