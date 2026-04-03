use ratatui::style::{Color, Style};

pub struct ColorScheme {
    pub normal: Style,
    pub keyword: Style,
    pub string: Style,
    pub comment: Style,
    pub function: Style,
    pub type_name: Style,
    pub number: Style,
}

impl ColorScheme {
    pub fn default_dark() -> Self {
        Self {
            normal: Style::default().fg(Color::White),
            keyword: Style::default().fg(Color::Magenta),
            string: Style::default().fg(Color::Green),
            comment: Style::default().fg(Color::Gray),
            function: Style::default().fg(Color::Blue),
            type_name: Style::default().fg(Color::Yellow),
            number: Style::default().fg(Color::Cyan),
        }
    }
}
