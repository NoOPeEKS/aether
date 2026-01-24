use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Widget},
};

use crate::config::AetherConfig;

pub struct ProfileSection<'a> {
    cfg: &'a AetherConfig,
}

impl<'a> ProfileSection<'a> {
    pub fn new(cfg: &'a AetherConfig) -> Self {
        Self { cfg }
    }
}

impl<'a> Widget for ProfileSection<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .style(Style::default())
            .title("Profile")
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
