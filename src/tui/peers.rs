use libp2p::PeerId;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::ListItem,
};

use crate::protocol::{PeerVerifyState, RemotePeer};

#[derive(Clone)]
pub struct Peer(pub RemotePeer);

impl Peer {
    pub fn id(&self) -> PeerId {
        self.0.peer_id
    }
}

impl<'a> From<&Peer> for ListItem<'a> {
    fn from(peer: &Peer) -> Self {
        let hostname = Span::raw(peer.0.hostname .clone().unwrap_or_else(|| String::from("unknown:")));
        let id = Span::raw(peer.0.peer_id.to_string());
        let state = match peer.0.state {
            PeerVerifyState::Verified => Span::styled("(verified) ", Style::default().fg(Color::Green)),
            PeerVerifyState::Blocked => Span::styled("(blocked) ", Style::default().fg(Color::Red)),
            PeerVerifyState::WaitingVerification => Span::styled("(waiting for verification) ", Style::default().fg(Color::LightBlue)),
        };

        Self::from(Line::from(vec![state, hostname, id]))
    }
}
