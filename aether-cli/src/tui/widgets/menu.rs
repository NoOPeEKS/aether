use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Widget},
};

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
