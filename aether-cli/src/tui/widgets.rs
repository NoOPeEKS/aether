use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Widget},
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
    pub fn max_line_len(&self) -> Option<usize> {
        if self.cfg.profiles.len() < 1 {
            return None;
        }
        self.cfg
            .profiles
            .iter()
            .map(|(name, prof)| {
                format!("{name}: {}:{}", prof.broker_ip, prof.broker_api_port).len()
            })
            .max()
    }
    pub fn num_profiles(&self) -> u16 {
        self.cfg.profiles.len() as u16
    }
}

impl<'a> Widget for ProfileSelectionPopup<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let block = Block::bordered()
            .style(Style::default())
            .title("Select profile")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().light_green());

        let lines: Vec<Line<'_>> = self
            .cfg
            .profiles
            .iter()
            .map(|(name, pinfo)| {
                Line::from(vec![
                    name.as_str().yellow(),
                    Span::from(format!(": {}:{}", pinfo.broker_ip, pinfo.broker_api_port)),
                ])
            })
            .collect();
        Paragraph::new(lines).block(block).render(area, buf);
    }
}
