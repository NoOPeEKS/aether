use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;

use crate::{config::AetherConfig, tui::ui::ui};

#[derive(PartialEq)]
pub enum Action {
    ProfileSelection,
    None,
}

#[derive(PartialEq)]
pub enum Panel {
    Menu,
    Profile,
    None,
}

pub enum PanelDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(PartialEq)]
pub enum Mode {
    Default,
}

pub struct AppState {
    pub should_quit: bool,
    pub http_client: reqwest::Client,
    pub config: AetherConfig,
    pub current_panel: Panel,
    pub mode: Mode,
    pub action: Action,
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
                current_panel: Panel::Menu,
                mode: Mode::Default,
                action: Action::None,
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

    async fn handle_events(&mut self, stream: &mut EventStream) {
        if let Some(Ok(event)) = stream.next().await
            && let Event::Key(kevent) = event
        {
            match (kevent.code, kevent.modifiers) {
                (KeyCode::Char('q'), KeyModifiers::CONTROL) => self.state.should_quit = true,
                (KeyCode::Esc, _) => {
                    self.state.mode = Mode::Default;
                    self.state.action = Action::None;
                    self.state.current_panel = Panel::None
                }
                (KeyCode::Left, KeyModifiers::CONTROL) => {
                    self.handle_panel_changes(PanelDirection::Left)
                }
                (KeyCode::Right, KeyModifiers::CONTROL) => {
                    self.handle_panel_changes(PanelDirection::Right)
                }
                (KeyCode::Down, KeyModifiers::CONTROL) => {
                    self.handle_panel_changes(PanelDirection::Down)
                }
                (KeyCode::Up, KeyModifiers::CONTROL) => {
                    self.handle_panel_changes(PanelDirection::Up)
                }
                (KeyCode::Enter, _) => self.handle_enter(),
                _ => {}
            }
        }
    }

    fn handle_panel_changes(&mut self, direction: PanelDirection) {
        if self.state.mode == Mode::Default {
            match direction {
                PanelDirection::Left => {
                    // TODO: Handle this whenever we implement more panels.
                }
                PanelDirection::Right => {
                    // TODO: Handle this whenever we implement more panels.
                }
                PanelDirection::Up => {
                    // TODO: Implement logic for intelligently changing panels.
                    self.state.current_panel = Panel::Profile
                }
                PanelDirection::Down => {
                    // TODO: Implement logic for intelligently changing panels.
                    self.state.current_panel = Panel::Menu
                }
            }
        }
    }

    fn handle_enter(&mut self) {
        if self.state.current_panel == Panel::Profile {
            self.state.action = Action::ProfileSelection;
        }
    }
}
