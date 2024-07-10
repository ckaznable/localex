use anyhow::Result;
use crossterm::{
    self, cursor,
    event::{DisableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend, layout::Alignment, style::{Modifier, Style}, widgets::{Block, List, ListItem, ListState}, Frame, Terminal
};
use tokio::sync::mpsc;

use crate::protocol::{ClientEvent, DaemonEvent};

use self::peers::Peer;

mod peers;

#[derive(Default)]
pub struct TuiState {
    list_state: ListState,
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
    pub fn new(quit_tx: tokio::sync::oneshot::Sender<()>, client_tx: mpsc::Sender<ClientEvent>, daemon_rx: mpsc::Receiver<DaemonEvent>) -> Result<Self> {
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

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    async fn handle_daemon(&mut self, event: DaemonEvent) {
        match event {
            DaemonEvent::VerifyResult(_, _) => todo!(),
            DaemonEvent::InCommingVerify(_) => todo!(),
            DaemonEvent::PeerList(_) => todo!(),
        }
    }

    async fn handle_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit(),
            KeyCode::Char('j') => self.state.list_state.select_next(),
            KeyCode::Char('k') => self.state.list_state.select_previous(),
            KeyCode::Enter => {
                let Some(index) = self.state.list_state.selected() else {
                    return;
                };

                if let Some(peer) = self.state.peers.get(index) {
                    self.client_tx.send(ClientEvent::RequestVerify(peer.id())).await;
                }
            }
            _ => {}
        }
    }

    fn ui(f: &mut Frame, state: &mut TuiState) {
        let items: Vec<ListItem> = state.peers
            .iter()
            .map(ListItem::from)
            .collect();

        let list = List::new(items)
            .block(Block::bordered().title("Localex Agent").title_alignment(Alignment::Center))
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">")
            .repeat_highlight_symbol(true);

        f.render_stateful_widget(list, f.size(), &mut state.list_state);
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
