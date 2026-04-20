use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::ui::colorscheme::ColorScheme;

pub fn draw_intro(frame: &mut Frame, area: Rect, theme: &ColorScheme) {
    let logo = vec![
        "      █████╗ ████████╗ ██████╗ ███╗   ███╗",
        "     ██╔══██╗╚══██╔══╝██╔═══██╗████╗ ████║",
        "     ███████║   ██║   ██║   ██║██╔████╔██║",
        "     ██╔══██║   ██║   ██║   ██║██║╚██╔╝██║",
        "     ██║  ██║   ██║   ╚██████╔╝██║ ╚═╝ ██║",
        "     ╚═╝  ╚═╝   ╚═╝    ╚═════╝ ╚═╝     ╚═╝",
    ];

    let mut content = Vec::new();
    
    // 1. Logo
    for line in logo {
        content.push(Line::from(Span::styled(line, theme.get("Keyword"))).alignment(Alignment::Center));
    }

    content.push(Line::from(""));
    content.push(Line::from(Span::styled("ATOM IDE v0.1.9", theme.get("String").add_modifier(Modifier::BOLD))).alignment(Alignment::Center));

    // 2. Horizontal separator
    let separator = "──────────────────────────────────────────────────────────────────";
    let sep_style = theme.get("Comment").add_modifier(Modifier::DIM);
    content.push(Line::from(Span::styled(separator, sep_style)).alignment(Alignment::Center));

    // 3. Tagline
    content.push(Line::from(Span::styled("Atom is open source and freely distributable", theme.get("Normal"))).alignment(Alignment::Center));
    content.push(Line::from(Span::styled("https://github.com/gnuzd/atom", theme.get("String"))).alignment(Alignment::Center));

    content.push(Line::from(Span::styled(separator, sep_style)).alignment(Alignment::Center));

    // 4. Main Menu
    let main_items = vec![
        ("type  :help<Enter> ", "for help"),
        ("type  :q<Enter> ", "to exit"),
    ];

    for (cmd, desc) in main_items {
        content.push(Line::from(vec![
            Span::styled(format!("{:<26}", cmd), theme.get("Function")),
            Span::styled(format!("{:<20}", desc), theme.get("Normal")),
        ]).alignment(Alignment::Center));
    }

    content.push(Line::from(Span::styled(separator, sep_style)).alignment(Alignment::Center));

    // 5. Sub Menu
    content.push(Line::from(vec![
        Span::styled(format!("{:<26}", "type  :help news<Enter> "), theme.get("Function")),
        Span::styled(format!("{:<20}", "for v0.1.9 notes"), theme.get("Normal")),
    ]).alignment(Alignment::Center));

    let paragraph = Paragraph::new(content)
        .alignment(Alignment::Center);

    // Center vertically
    let vertical_center = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(22), // Adjusted for shorter logo
            Constraint::Min(1),
        ])
        .split(area);

    frame.render_widget(paragraph, vertical_center[1]);
}
