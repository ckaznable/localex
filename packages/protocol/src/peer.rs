use libp2p::{request_response::ResponseChannel, PeerId};

use crate::auth::LocalExAuthResponse;

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

pub struct DeamonPeer {
    pub peer_id: PeerId,
    pub state: PeerVerifyState,
    pub hostname: Option<String>,
    pub channel: Option<ResponseChannel<LocalExAuthResponse>>,
}

impl DeamonPeer {
    pub fn new(peer_id: PeerId) -> Self {
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

impl Clone for DeamonPeer {
    fn clone(&self) -> Self {
        Self {
            peer_id: self.peer_id,
            state: self.state,
            hostname: self.hostname.clone(),
            channel: None,
        }
    }
}
