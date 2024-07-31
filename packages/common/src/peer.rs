use libp2p::PeerId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize)]
pub struct DaemonPeer {
    pub peer_id: PeerId,
    pub state: PeerVerifyState,
    pub hostname: Option<String>,
}

impl DaemonPeer {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            state: PeerVerifyState::default(),
            hostname: None,
        }
    }

    pub fn set_hostname(&mut self, hostname: String) -> &mut Self {
        self.hostname = Some(hostname);
        self
    }
}

impl Clone for DaemonPeer {
    fn clone(&self) -> Self {
        Self {
            peer_id: self.peer_id,
            state: self.state,
            hostname: self.hostname.clone(),
        }
    }
}
