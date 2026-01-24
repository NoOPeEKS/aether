use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;

use crate::{config::AetherConfig, tui::ui::ui};

pub struct AppState {
    pub should_quit: bool,
    pub http_client: reqwest::Client,
    pub config: AetherConfig,
}

pub struct App {
    pub state: AppState,
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState {
                should_quit: false,
                http_client: reqwest::Client::new(),
                config: AetherConfig::get().expect("Must have at least a profile logged in!"),
            },
        }
    }
    pub async fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> anyhow::Result<()> {
        let mut events = EventStream::new();
        let mut tick_interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
        while !self.state.should_quit {
            terminal.draw(|frame| ui(frame, self))?;
            tokio::select! {
                _ = tick_interval.tick() => {}
                _ = self.handle_events(&mut events) => {}
            };
        }
        Ok(())
    }

    #[allow(clippy::all)]
    async fn handle_events(&mut self, stream: &mut EventStream) {
        if let Some(Ok(event)) = stream.next().await {
            if let Event::Key(kevent) = event {
                match (kevent.code, kevent.modifiers) {
                    (KeyCode::Char('q'), KeyModifiers::CONTROL) => self.state.should_quit = true,
                    _ => {}
                }
            }
        }
    }
}
