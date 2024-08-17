use anyhow::Result;
use common::event::DaemonEvent;
use crossterm::{
    cursor,
    event::{DisableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use localex_ipc::IPCClient;
use ratatui::{backend::CrosstermBackend, Frame, Terminal};
use tokio::sync::broadcast;

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
struct AppState {}

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

    async fn handle_deamon(&mut self, event: DaemonEvent) {
        todo!()
    }

    async fn handle_input(&mut self, code: KeyCode) {
        todo!()
    }

    fn ui(f: &mut Frame, state: &mut AppState) {
        todo!()
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
