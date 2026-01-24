use crate::tui::app::App;
use ratatui::Frame;

pub fn ui(frame: &mut Frame, app: &App) {
    frame.render_widget("Hello world!", frame.area());
}
