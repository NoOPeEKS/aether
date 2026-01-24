use crate::tui::{
    app::{App, Panel},
    widgets::{MenuSection, ProfileSection},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Style, palette::tailwind},
    widgets::{Block, Paragraph},
};

pub fn ui(frame: &mut Frame, app: &App) {
    let background = Block::default().style(Style::default().bg(tailwind::NEUTRAL.c900));
    frame.render_widget(background, frame.area());

    let area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(2)])
        .split(frame.area());

    let info_area = area[1];
    let info = Paragraph::new("Info bar will go here")
        .style(Style::default())
        .alignment(Alignment::Center);
    frame.render_widget(info, info_area);

    let area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)])
        .split(area[0]);

    let title_area = area[0];

    let title_text = "Aether";
    let width = title_area.width as usize;
    let text_len = title_text.len();

    let total_dashes = width.saturating_sub(text_len + 2);
    let left_dashes = total_dashes / 2;
    let right_dashes = total_dashes - left_dashes;

    let title_string = format!(
        "{}{} {} {}{}",
        "─".repeat(left_dashes),
        "*",
        title_text,
        "*",
        "─".repeat(right_dashes)
    );

    let title = Paragraph::new(title_string)
        .style(Style::default())
        .alignment(Alignment::Left);
    frame.render_widget(title, title_area);

    let main_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(20), Constraint::Percentage(80)])
        .split(area[1]);

    let navbar_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Percentage(90)])
        .split(main_area[0]);

    let _rest_area = main_area[1];

    let profile = ProfileSection::new(&app.state.config, app.state.current_panel == Panel::Profile);
    frame.render_widget(profile, navbar_area[0]);

    let menu_section = MenuSection::new(app.state.current_panel == Panel::Menu);
    frame.render_widget(menu_section, navbar_area[1]);
}
