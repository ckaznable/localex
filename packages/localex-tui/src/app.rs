use std::path::PathBuf;

use anyhow::Result;
use common::{
    event::{ClientEvent, DaemonEvent},
    peer::PeerVerifyState,
};
use crossterm::{
    self, cursor,
    event::{DisableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use libp2p::PeerId;
use localex_ipc::IPCClient;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Padding, Paragraph},
    Frame, Terminal,
};
use tokio::sync::broadcast;
use tracing::info;

use crate::components::{incomming_verify::InCommingVerify, peers::Peer};

#[derive(Default)]
enum AppUIState {
    InCommingVerify(Peer),
    #[default]
    List,
}

#[derive(Default)]
pub struct AppState {
    list_state: ListState,
    ui_state: AppUIState,
    peers: Vec<Peer>,
    hostname: String,
    local_peer: Option<PeerId>,
}

pub struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    state: AppState,
    should_quit: bool,
    client: IPCClient,
    quit_tx: broadcast::Sender<()>,
}

impl App {
    pub async fn new(sock: Option<PathBuf>) -> Result<Self> {
        let (quit_tx, rx) = broadcast::channel(10);
        let client = IPCClient::new(sock, rx).await?;

        // setup terminal
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, DisableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            quit_tx,
            terminal,
            client,
            state: AppState::default(),
            should_quit: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut reader = crossterm::event::EventStream::new();

        self.client.prepare().await?;
        self.client.send(ClientEvent::RequestPeerList).await;
        self.client.send(ClientEvent::RequestLocalInfo).await;

        loop {
            self.terminal.draw(|f| Self::ui(f, &mut self.state))?;

            let tui_event = reader.next().fuse();
            tokio::select! {
                event = tui_event => if let Some(Ok(Event::Key(KeyEvent { code, modifiers: KeyModifiers::NONE, .. }))) = event {
                    self.handle_input(code).await;
                },
                event = self.client.recv() => {
                    self.handle_deamon(event).await;
                },
            }

            if self.should_quit {
                let _ = self.quit_tx.send(());
                return Ok(());
            }
        }
    }

    #[inline]
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    async fn handle_deamon(&mut self, event: DaemonEvent) {
        use DaemonEvent::*;
        match event {
            LocalInfo(hostname, peer) => {
                info!("local info {}:{}", hostname.clone(), peer);
                self.state.hostname = hostname;
                self.state.local_peer = Some(peer);
            }
            VerifyResult(peer_id, result) => {
                if let Some(p) = self.state.peers.iter_mut().find(|p| p.id() == peer_id) {
                    p.0.state = if result {
                        PeerVerifyState::Verified
                    } else {
                        PeerVerifyState::Blocked
                    };
                }
            }
            InComingVerify(peer) => {
                self.state.ui_state = self
                    .state
                    .peers
                    .iter()
                    .find(|p| p.id() == peer.peer_id)
                    .map(|p| AppUIState::InCommingVerify(p.clone()))
                    .unwrap_or_else(|| AppUIState::InCommingVerify(Peer(peer)));
            }
            PeerList(peers) => {
                if peers.len() > self.state.peers.len() {
                    info!(
                        "received {} new peers",
                        peers.len() - self.state.peers.len()
                    );
                }

                self.state.peers = peers.into_iter().map(Peer).collect();
                if self.state.list_state.selected().is_none() && !self.state.peers.is_empty() {
                    self.state.list_state.select(Some(0))
                }
            }
            _ => {}
        }
    }

    async fn handle_input(&mut self, code: KeyCode) {
        use KeyCode::*;
        match code {
            Char('q') | Char('Q') => self.quit(),
            Char('j') => self.state.list_state.select_next(),
            Char('k') => self.state.list_state.select_previous(),
            Enter => {
                if let Some(index) = self.state.list_state.selected() {
                    if let Some(peer) = self.state.peers.get(index) {
                        info!("request verification to remote peer {}", peer.id());
                        self.client
                            .send(ClientEvent::RequestVerify(peer.id()))
                            .await;
                    }
                }
            }
            Char('d') => {
                if let Some(index) = self.state.list_state.selected() {
                    if let Some(peer) = self.state.peers.get(index) {
                        self.client
                            .send(ClientEvent::DisconnectPeer(peer.id()))
                            .await;
                    }
                }
            }
            Char('y') | Char('Y') => {
                if let AppUIState::InCommingVerify(p) = &self.state.ui_state {
                    self.client
                        .send(ClientEvent::VerifyConfirm(p.id(), true))
                        .await;

                    if let Some(peer) = self.state.peers.iter_mut().find(|_p| p.id() == _p.id()) {
                        peer.0.state = PeerVerifyState::Verified;
                    }
                }

                self.state.ui_state = AppUIState::List;
            }
            Char('n') | Char('N') => {
                if let AppUIState::InCommingVerify(p) = &self.state.ui_state {
                    self.client
                        .send(ClientEvent::VerifyConfirm(p.id(), false))
                        .await;
                }

                self.state.ui_state = AppUIState::List;
            }
            Char('r') | Char('R') => {
                self.client
                    .send(ClientEvent::RequestPeerList)
                    .await;
            }
            _ => {}
        }
    }

    fn ui(f: &mut Frame, state: &mut AppState) {
        let area = f.size();
        let items: Vec<ListItem> = state.peers.iter().map(ListItem::from).collect();

        let [name_area, id_area, list_area, bottom_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

        let name = Paragraph::new(vec![Line::from(Span::raw(&state.hostname))])
            .block(Block::bordered().title("Name"));
        f.render_widget(name, name_area);

        let peer_id = state
            .local_peer
            .map(|p| p.to_string())
            .unwrap_or_else(|| "unknown".into());
        let id = Paragraph::new(vec![Line::from(Span::raw(&peer_id))])
            .block(Block::bordered().title("ID"));
        f.render_widget(id, id_area);

        let list = List::new(items)
            .block(
                Block::bordered()
                    .title("Remote Device List")
                    .title_alignment(Alignment::Left),
            )
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">")
            .repeat_highlight_symbol(true);
        f.render_stateful_widget(list, list_area, &mut state.list_state);

        let bottom = Paragraph::new(vec![Line::from(vec![
            Span::styled("R", Style::default().fg(Color::Red)),
            Span::raw("efresh"),
        ])])
            .block(Block::default().padding(Padding::horizontal(2)));
        f.render_widget(bottom, bottom_area);

        if let AppUIState::InCommingVerify(peer) = &state.ui_state {
            let carea = Self::centered_rect(100, 5, area);
            f.render_widget(InCommingVerify(peer), carea);
        }
    }

    fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
        let padding = (r.height - height) / 2;
        let [_, v_center] = Layout::vertical(Constraint::from_lengths([padding, height])).areas(r);

        let padding = (r.width - width) / 2;
        let [_, center] =
            Layout::horizontal(Constraint::from_lengths([padding, width])).areas(v_center);
        center
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // restore terminal
        if crossterm::terminal::is_raw_mode_enabled().unwrap() {
            let _ = disable_raw_mode();
            let _ = execute!(
                self.terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                cursor::Show
            );
        }
    }
}
