use std::collections::HashMap;

use libp2p::{request_response::ResponseChannel, PeerId};

use crate::behaviour::LocalExAuthResponse;

#[derive(Clone)]
pub enum ClientEvent {
    RequestVerify(PeerId),
    DisconnectPeer(PeerId),
    VerifyConfirm(PeerId, bool),
}

#[derive(Clone)]
pub enum DaemonEvent {
    VerifyResult(PeerId, bool),
    InCommingVerify(RemotePeer),
    PeerList(Vec<RemotePeer>),
    LocalInfo(String, PeerId),
}

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

pub struct RemotePeer {
    pub peer_id: PeerId,
    pub state: PeerVerifyState,
    pub hostname: Option<String>,
    pub channel: Option<ResponseChannel<LocalExAuthResponse>>,
}

impl RemotePeer {
    fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            state: PeerVerifyState::default(),
            hostname: None,
            channel: None,
        }
    }

    pub fn set_channel(&mut self, channel: ResponseChannel<LocalExAuthResponse>) -> &mut Self {
        self.channel = Some(channel);
        self
    }

    pub fn set_hostname(&mut self, hostname: String) -> &mut Self {
        self.hostname = Some(hostname);
        self
    }
}

impl Clone for RemotePeer {
    fn clone(&self) -> Self {
        Self {
            peer_id: self.peer_id,
            state: self.state,
            hostname: self.hostname.clone(),
            channel: None,
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

    #[inline]
    pub fn add_peer(&mut self, peer_id: PeerId) {
        if self.peers.contains_key(&peer_id) {
            return;
        }

        self.peers.insert(peer_id, RemotePeer::new(peer_id));
    }

    #[inline]
    pub fn remove_peer(&mut self, peer_id: &PeerId) {
        self.peers.remove(peer_id);
    }

    #[inline]
    pub fn get_peer_mut(&mut self, peer_id: &PeerId) -> Option<&mut RemotePeer> {
        self.peers.get_mut(peer_id)
    }

    #[inline]
    pub fn verified(&mut self, peer_id: &PeerId) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.state = PeerVerifyState::Verified;
        }
    }

    #[inline]
    pub fn get_all_peers(&self) -> Vec<RemotePeer> {
        self.peers.values().cloned().collect()
    }
}
