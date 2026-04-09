use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use crate::ui::colorscheme::ColorScheme;

pub fn draw_intro(frame: &mut Frame, area: Rect, theme: &ColorScheme) {
    let logo = vec![
        "      ___           ___           ___           ___     ",
        "     /  /\\         /__/\\         /  /\\         /__/\\    ",
        "    /  /::\\        \\  \\:\\       /  /::\\        \\  \\:\\   ",
        "   /  /:/\\:\\        \\  \\:\\     /  /:/\\:\\        \\  \\:\\  ",
        "  /  /:/~/::\\   _____\\__\\:\\   /  /:/  \\:\\   _____\\__\\:\\ ",
        " /__/:/ /:/\\:\\ /__/::::::::\\ /__/:/ \\__\\:\\ /__/::::::::\\",
        " \\  \\:\\/:/__\\/ \\  \\:\\~~\\~~\\/ \\  \\:\\ /  /:/ \\  \\:\\~~\\~~\\/ ",
        "  \\  \\::/       \\  \\:\\  ~~~   \\  \\:\\  /:/   \\  \\:\\  ~~~  ",
        "   \\  \\:\\        \\  \\:\\        \\  \\:\\/:/     \\  \\:\\      ",
        "    \\  \\:\\        \\  \\:\\        \\  \\::/       \\  \\:\\     ",
        "     \\__\\/         \\__\\/         \\__\\/         \\__\\/     ",
    ];

    let mut content = Vec::new();
    content.push(Line::from(""));
    
    for line in logo {
        content.push(Line::from(Span::styled(line, theme.get("Keyword"))).alignment(Alignment::Center));
    }

    content.push(Line::from(""));
    content.push(Line::from(Span::styled("Atom IDE - High Performance Modal Editor", theme.get("String").add_modifier(Modifier::BOLD))).alignment(Alignment::Center));
    content.push(Line::from(Span::styled("version 0.1.0 (Rust 2024)", theme.get("Comment"))).alignment(Alignment::Center));
    content.push(Line::from(""));
    content.push(Line::from(""));

    let help_items = vec![
        ("type  :q<Enter> ", "to exit"),
        ("type  :help<Enter>", "for help"),
        ("type  \\           ", "toggle explorer"),
        ("type  <Space>ff   ", "find files"),
        ("type  <Space>th   ", "change theme"),
    ];

    for (cmd, desc) in help_items {
        content.push(Line::from(vec![
            Span::styled(cmd, theme.get("Function")),
            Span::styled(desc, theme.get("Normal")),
        ]).alignment(Alignment::Center));
    }

    let paragraph = Paragraph::new(content)
        .alignment(Alignment::Center)
        .block(Block::default());

    // Center vertically
    let vertical_center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(25), // Height of logo + text + help
            Constraint::Min(1),
        ])
        .split(area);

    frame.render_widget(paragraph, vertical_center[1]);
}
