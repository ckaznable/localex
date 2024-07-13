use anyhow::Result;
use crossterm::{
    self, cursor,
    event::{DisableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, List, ListItem, ListState},
    Frame, Terminal,
};
use tokio::sync::mpsc;
use tracing::info;

use crate::protocol::{ClientEvent, DaemonEvent, PeerVerifyState};

use self::{incomming_verify::InCommingVerify, peers::Peer};

mod incomming_verify;
mod peers;

#[derive(Default)]
enum TuiUiState {
    InCommingVerify(Peer),
    #[default]
    List,
}

#[derive(Default)]
pub struct TuiState {
    list_state: ListState,
    ui_state: TuiUiState,
    peers: Vec<Peer>,
}

pub struct Tui {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    state: TuiState,
    should_quit: bool,
    quit_tx: Option<tokio::sync::oneshot::Sender<()>>,
    client_tx: mpsc::Sender<ClientEvent>,
    daemon_rx: mpsc::Receiver<DaemonEvent>,
}

impl Tui {
    pub fn new(
        quit_tx: tokio::sync::oneshot::Sender<()>,
        client_tx: mpsc::Sender<ClientEvent>,
        daemon_rx: mpsc::Receiver<DaemonEvent>,
    ) -> Result<Self> {
        // setup terminal
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, DisableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            state: TuiState::default(),
            should_quit: false,
            quit_tx: Some(quit_tx),
            client_tx,
            daemon_rx,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut reader = crossterm::event::EventStream::new();

        loop {
            self.terminal.draw(|f| Self::ui(f, &mut self.state))?;

            let tui_event = reader.next().fuse();
            tokio::select! {
                event = tui_event => if let Some(Ok(Event::Key(KeyEvent { code, modifiers: KeyModifiers::NONE, .. }))) = event {
                    self.handle_input(code).await;
                },
                daemon_event = self.daemon_rx.recv() => if let Some(event) = daemon_event {
                    self.handle_daemon(event).await;
                },
            }

            if self.should_quit {
                break;
            }
        }

        let _ = self.quit_tx.take().map(|t| t.send(()));
        Ok(())
    }

    #[inline]
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    async fn handle_daemon(&mut self, event: DaemonEvent) {
        match event {
            DaemonEvent::VerifyResult(peer_id, result) => {
                if let Some(p) = self.state.peers.iter_mut().find(|p| p.id() == peer_id) {
                    p.0.state = if result {
                        PeerVerifyState::Verified
                    } else {
                        PeerVerifyState::Blocked
                    };
                }
            }
            DaemonEvent::InCommingVerify(peer) => {
                self.state.ui_state = self
                    .state
                    .peers
                    .iter()
                    .find(|p| p.id() == peer.peer_id)
                    .map(|p| TuiUiState::InCommingVerify(p.clone()))
                    .unwrap_or_else(|| TuiUiState::InCommingVerify(Peer(peer)));
            }
            DaemonEvent::PeerList(peers) => {
                self.state.peers = peers.into_iter().map(Peer).collect();
                if self.state.list_state.selected().is_none() && !self.state.peers.is_empty() {
                    self.state.list_state.select(Some(0))
                }
            }
        }
    }

    async fn handle_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit(),
            KeyCode::Char('j') => self.state.list_state.select_next(),
            KeyCode::Char('k') => self.state.list_state.select_previous(),
            KeyCode::Enter => {
                if let Some(index) = self.state.list_state.selected() {
                    if let Some(peer) = self.state.peers.get(index) {
                        info!("request verication to remote peer {}", peer.id());
                        self.client_tx
                            .send(ClientEvent::RequestVerify(peer.id()))
                            .await;
                    }
                }
            }
            KeyCode::Char('d') => {
                if let Some(index) = self.state.list_state.selected() {
                    if let Some(peer) = self.state.peers.get(index) {
                        self.client_tx
                            .send(ClientEvent::DisconnectPeer(peer.id()))
                            .await;
                    }
                }
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let TuiUiState::InCommingVerify(p) = &self.state.ui_state {
                    self.client_tx
                        .send(ClientEvent::VerifyConfirm(p.id(), true))
                        .await;

                    if let Some(peer) = self.state.peers.iter_mut().find(|_p| p.id() == _p.id()) {
                        peer.0.state = PeerVerifyState::Verified;
                    }
                }

                self.state.ui_state = TuiUiState::List;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if let TuiUiState::InCommingVerify(p) = &self.state.ui_state {
                    self.client_tx
                        .send(ClientEvent::VerifyConfirm(p.id(), false))
                        .await;
                }

                self.state.ui_state = TuiUiState::List;
            }
            _ => {}
        }
    }

    fn ui(f: &mut Frame, state: &mut TuiState) {
        let area = f.size();
        let items: Vec<ListItem> = state.peers.iter().map(ListItem::from).collect();

        let list = List::new(items)
            .block(
                Block::bordered()
                    .title("Localex Agent")
                    .title_alignment(Alignment::Center),
            )
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">")
            .repeat_highlight_symbol(true);

        f.render_stateful_widget(list, area, &mut state.list_state);

        if let TuiUiState::InCommingVerify(peer) = &state.ui_state {
            let carea = Self::centered_rect(100, 5, area);
            f.render_widget(InCommingVerify(peer), carea);
        }
    }

    fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
        let padding = (r.height - height) / 2;
        let [_, v_center] =
            Layout::vertical(Constraint::from_lengths([padding, height])).areas(r);

        let padding = (r.width - width) / 2;
        let [_, center] =
            Layout::horizontal(Constraint::from_lengths([padding, width]))
                .areas(v_center);
        center
    }
}

impl Drop for Tui {
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
