mod app;
mod ui;
mod widgets;

use crate::tui::app::App;

pub async fn run_tui() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new();

    app.run(&mut terminal).await?;
    ratatui::restore();
    Ok(())
}

