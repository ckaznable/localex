#![allow(clippy::single_match)]

use std::{fs::File, io::{BufReader, Read}, path::PathBuf};

use anyhow::Result;
use common::{
    event::{ClientEvent, ClientFileId, DaemonEvent},
    peer::DaemonPeer,
};
use crossterm::{
    cursor,
    event::{DisableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use localex_ipc::IPCClient;
use rand::RngCore;
use ratatui::{
    backend::CrosstermBackend, layout::{Constraint, Layout}, style::{Modifier, Style}, text::{Line, Span}, widgets::{Block, List, ListItem, ListState, Paragraph}, Frame, Terminal
};
use tokio::sync::broadcast;

const FILE_ID: &str = "localex-example1-file-id-xxxx";

#[tokio::main]
async fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, DisableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    let mut app = App::new(terminal).await?;
    app.run().await?;

    Ok(())
}

#[derive(Default)]
struct AppState {
    md5_checksum: Option<String>,
    remote_md5_checksum: Option<String>,
    raw: Vec<u8>,
    list: Vec<DaemonPeer>,
    list_state: ListState,
}

struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    should_quit: bool,
    client: IPCClient,
    quit_tx: broadcast::Sender<()>,
    state: AppState,
}

impl App {
    async fn new(terminal: Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<Self> {
        let (quit_tx, rx) = broadcast::channel(0);
        Ok(Self {
            quit_tx,
            terminal,
            state: AppState::default(),
            should_quit: false,
            client: IPCClient::new(None, rx).await?,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut reader = crossterm::event::EventStream::new();
        self.client.prepare().await?;

        self.gen_test_raw_data();
        self.client
            .send(ClientEvent::RegistFileId(
                FILE_ID.to_string(),
                ClientFileId::Raw(self.state.raw.clone()),
            ))
            .await;

        loop {
            self.terminal.draw(|f| Self::ui(f, &mut self.state))?;

            let tui_event = reader.next().fuse();
            tokio::select! {
                event = tui_event => if let Some(Ok(Event::Key(KeyEvent { code, modifiers: KeyModifiers::NONE, .. }))) = event {
                    self.handle_input(code).await;
                },
                event = self.client.recv() => {
                    self.handle_deamon(event).await?;
                },
            }

            if self.should_quit {
                let _ = self.quit_tx.send(());
                return Ok(());
            }
        }
    }

    fn gen_test_raw_data(&mut self) {
        let mut rng = rand::thread_rng();
        let len: usize = 1024 * 1024 * 10; // 10 MB
        let mut raw = Vec::with_capacity(len);
        rng.fill_bytes(&mut raw);
        self.state.md5_checksum = Some(format!("{:x}", md5::compute(&raw)));
        self.state.raw = raw;
    }

    async fn handle_deamon(&mut self, event: DaemonEvent) -> Result<()> {
        match event {
            DaemonEvent::PeerList(list) => {
                self.state.list = list;
            },
            DaemonEvent::FileUpdated(_, path) => {
                let path = PathBuf::from(path);
                let file = File::open(path)?;
                let mut reader = BufReader::new(file);

                let mut hasher = md5::Context::new();

                let mut buffer = [0; 1024];
                loop {
                    let bytes_read = reader.read(&mut buffer)?;
                    if bytes_read == 0 {
                        break;
                    }
                    hasher.consume(&buffer[..bytes_read]);
                }

                let result = hasher.compute();
                let md5_string = format!("{:x}", result);
                self.state.remote_md5_checksum = Some(md5_string);
            }
            _ => {},
        }

        Ok(())
    }

    async fn handle_input(&mut self, code: KeyCode) {
        use KeyCode::*;
        match code {
            Char('q') => self.should_quit = true,
            Char('j') => self.state.list_state.select_next(),
            Char('k') => self.state.list_state.select_previous(),
            Enter => {
                let i = self.state.list_state.selected().unwrap_or(0);
                if let Some(peer) = self.state.list.get(i) {
                    self.client.send(ClientEvent::SendFile(peer.peer_id, FILE_ID.to_string())).await;
                }
            }
            _ => {}
        }
    }

    fn ui(f: &mut Frame, state: &mut AppState) {
        let [local_area, remote_area, list_area] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .areas(f.area());

        let local_checksum = state.md5_checksum.clone().unwrap_or("".to_string());
        let local_block = Paragraph::new(vec![Line::from(Span::raw(local_checksum))])
            .block(Block::bordered().title("local"));
        f.render_widget(local_block, local_area);

        let remote_checksum = state.remote_md5_checksum.clone().unwrap_or("".to_string());
        let remote_block = Paragraph::new(vec![Line::from(Span::raw(remote_checksum))])
            .block(Block::bordered().title("local"));
        f.render_widget(remote_block, remote_area);

        let items: Vec<ListItem> = state.list.iter().map(|p| ListItem::from(p.peer_id.to_string())).collect();
        let list = List::new(items)
            .block(Block::bordered())
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">")
            .repeat_highlight_symbol(true);
        f.render_stateful_widget(list, list_area, &mut state.list_state);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if crossterm::terminal::is_raw_mode_enabled().unwrap() {
            let _ = disable_raw_mode();
            let _ = execute!(
                self.terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                cursor::Show,
            );
        }
    }
}
