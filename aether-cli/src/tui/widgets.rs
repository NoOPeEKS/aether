use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Widget, Wrap},
};

use crate::config::AetherConfig;

pub struct ProfileSection<'a> {
    cfg: &'a AetherConfig,
    active: bool,
}

impl<'a> ProfileSection<'a> {
    pub fn new(cfg: &'a AetherConfig, active: bool) -> Self {
        Self { cfg, active }
    }
}

impl<'a> Widget for ProfileSection<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.active {
            Style::new().light_green()
        } else {
            Style::default()
        };
        let block = Block::bordered()
            .style(Style::default())
            .title("Profile")
            .border_style(border_style)
            .border_type(BorderType::Rounded);
        if let Some(active) = &self.cfg.active {
            if let Some(prof) = self.cfg.profiles.get(active) {
                let port = format!("{}", prof.broker_api_port);
                let lines = vec![
                    Line::from(vec!["Name: ".yellow(), active.into()]),
                    Line::from(vec![
                        "Broker IP: ".yellow(),
                        Span::from(prof.broker_ip.clone()),
                    ]),
                    Line::from(vec!["Broker Port: ".yellow(), Span::from(port)]),
                ];

                Paragraph::new(lines).block(block).render(area, buf);
            } else {
                Paragraph::new("No active profile.\nPlease select one.")
                    .block(block)
                    .render(area, buf);
            }
        } else {
            Paragraph::new("No active profile.\nPlease select one.")
                .block(block)
                .render(area, buf);
        }
    }
}

pub struct MenuSection {
    active: bool,
}

impl MenuSection {
    pub fn new(active: bool) -> Self {
        Self { active }
    }
}

impl Widget for MenuSection {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.active {
            Style::new().light_green()
        } else {
            Style::default()
        };
        Block::bordered()
            .style(Style::default())
            .title("Menu")
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .render(area, buf);
    }
}

pub struct ProfileSelectionPopup<'a> {
    cfg: &'a AetherConfig,
}

impl<'a> ProfileSelectionPopup<'a> {
    pub fn new(cfg: &'a AetherConfig) -> Self {
        Self { cfg }
    }
    pub fn token_len(&self) -> Option<usize> {
        if self.cfg.profiles.is_empty() {
            return None;
        }
        if let Some(active) = &self.cfg.active
            && let Some(prof) = self.cfg.profiles.get(&active.clone())
            && let Some(token) = &prof.token
        {
            Some(token.len())
        } else {
            None
        }
    }
}

impl<'a> Widget for ProfileSelectionPopup<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .style(Style::default())
            .title("Profile JWT")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().light_green());
        if let Some(active) = &self.cfg.active {
            let token = self
                .cfg
                .profiles
                .get(&active.clone())
                .unwrap()
                .token
                .as_ref();
            if let Some(token) = token {
                Clear.render(area, buf);

                Paragraph::new(Line::from(vec!["Token: ".yellow(), Span::from(token)]))
                    .block(block)
                    .wrap(Wrap { trim: true })
                    .render(area, buf);
            } else {
                Clear.render(area, buf);
                Paragraph::new(Line::from(vec![
                    "Cannot get token for active profile".yellow(),
                ]))
                .block(block)
                .render(area, buf);
            }
        } else {
            Clear.render(area, buf);
            Paragraph::new(Line::from(vec![
                "Cannot get token without an active user!".light_red(),
            ]))
            .block(block)
            .render(area, buf);
        }
    }
}
