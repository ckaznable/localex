use common::peer::{DaemonPeer, PeerVerifyState};
use libp2p::PeerId;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{ListItem, Paragraph, Widget},
};

const VERIFIED_COLOR: Color = Color::Green;
const BLOCKED_COLOR: Color = Color::Red;
const WAITING_VERIFICATION_COLOR: Color = Color::LightBlue;

#[derive(Clone)]
pub struct Peer(pub DaemonPeer);

impl Peer {
    #[inline]
    pub fn id(&self) -> PeerId {
        self.0.peer_id
    }
}

impl<'a> From<&Peer> for ListItem<'a> {
    fn from(peer: &Peer) -> Self {
        let hostname = peer.0.hostname.clone().unwrap_or_else(|| "unknown".into());
        Self::from(Line::from(Span::styled(
            hostname,
            match peer.0.state {
                PeerVerifyState::Verified => Style::default().fg(VERIFIED_COLOR),
                PeerVerifyState::Blocked => Style::default().fg(BLOCKED_COLOR),
                PeerVerifyState::WaitingVerification => Style::default().fg(WAITING_VERIFICATION_COLOR),
            },
        )))
    }
}

impl Widget for &Peer {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw(format!("ID: {}", self.0.peer_id)),
            ]),
            Line::from(vec![
                Span::raw("state: "),
                match self.0.state {
                    PeerVerifyState::Verified => Span::styled("Verified", Style::default().fg(VERIFIED_COLOR)),
                    PeerVerifyState::Blocked => Span::styled("Blocked", Style::default().fg(BLOCKED_COLOR)),
                    PeerVerifyState::WaitingVerification => Span::styled("Waiting For Verification", Style::default().fg(WAITING_VERIFICATION_COLOR)),
                }
            ])
        ]).render(area, buf);
    }
}
