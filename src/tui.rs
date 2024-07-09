use anyhow::Result;
use crossterm::{
    self, cursor,
    event::{DisableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend, Frame, Terminal
};


pub struct Tui {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    should_quit: bool,
    tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Tui {
    pub fn new(tx: tokio::sync::oneshot::Sender<()>) -> Result<Self> {
        // setup terminal
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            tx: Some(tx),
            should_quit: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut reader = crossterm::event::EventStream::new();

        loop {
            self.terminal.draw(Self::ui)?;

            let tui_event = reader.next().fuse();
            tokio::select! {
                event = tui_event => if let Some(Ok(Event::Key(KeyEvent { code, modifiers: KeyModifiers::NONE, .. }))) = event {
                    self.handle_input(code);
                },
            }

            if self.should_quit {
                break;
            }
        }

        let _ = self.tx.take().unwrap().send(());
        Ok(())
    }

    fn handle_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.should_quit = true,
            _ => {}
        }
    }

    fn ui(f: &mut Frame) {

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
