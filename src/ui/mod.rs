pub mod colorscheme;
pub mod explorer;
pub mod icons;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, BorderType, List, ListItem, Padding, Paragraph},
    Frame,
};
use crate::vim::mode::{Mode, ExplorerInputType};

pub struct TerminalUi;

impl TerminalUi {
    pub fn new() -> Self {
        Self
    }

    fn get_file_icon(path: &std::path::Path) -> (&'static str, Color) {
        if path.is_dir() {
            return (icons::FOLDER, Color::Yellow);
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext {
            "rs" => (icons::STRUCT, Color::Red),
            "toml" => (icons::PACKAGE, Color::Cyan),
            "md" => (icons::TEXT, Color::Blue),
            "lock" => (icons::FILE, Color::Gray),
            _ => (icons::FILE, Color::White),
        }
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        editor: &crate::editor::Editor,
        vim: &crate::vim::VimState,
        explorer: &explorer::FileExplorer,
    ) {
        let area = frame.area();
        
        let root_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area);

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(if explorer.visible {
                [Constraint::Percentage(15), Constraint::Percentage(85)]
            } else {
                [Constraint::Percentage(0), Constraint::Percentage(100)]
            })
            .split(root_chunks[0]);

        // 1. File Explorer Area
        if explorer.visible {
            // Full height vertical separator
            let sidebar_divider = Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(Color::DarkGray));
            frame.render_widget(sidebar_divider, main_chunks[0]);

            let explorer_content_area = Rect {
                x: main_chunks[0].x,
                y: main_chunks[0].y,
                width: main_chunks[0].width.saturating_sub(1),
                height: main_chunks[0].height,
            };

            let explorer_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(explorer_content_area);

            // Explorer Header Box - Full Borders + Rounded Corners + Padding
            let header_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled(" Explorer ", Style::default().fg(Color::Yellow).bold()))
                .border_style(Style::default().fg(Color::DarkGray))
                .padding(Padding::horizontal(1)); // Horizontal padding for search text
            
            let header_inner = header_block.inner(explorer_layout[0]);
            frame.render_widget(header_block, explorer_layout[0]);

            let header_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Length(8)])
                .split(header_inner);

            let filter_display = if let Mode::ExplorerInput(ExplorerInputType::Filter) = vim.mode {
                format!("> {}", vim.input_buffer)
            } else {
                format!("> {}", explorer.filter)
            };
            frame.render_widget(Paragraph::new(filter_display).fg(Color::Yellow), header_chunks[0]);

            let count_text = format!("{}/{}", explorer.entries.len(), explorer.entries.len());
            frame.render_widget(Paragraph::new(count_text).alignment(Alignment::Right).fg(Color::DarkGray), header_chunks[1]);

            if let Mode::ExplorerInput(ExplorerInputType::Filter) = vim.mode {
                frame.set_cursor_position((
                    header_chunks[0].x + vim.input_buffer.len() as u16 + 2,
                    header_chunks[0].y,
                ));
            }

            let items: Vec<ListItem> = explorer
                .entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let name = entry.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    let mut guide = String::new();
                    for _ in 0..entry.depth { guide.push_str("│ "); }
                    if entry.depth > 0 {
                        guide.pop(); guide.pop();
                        if entry.is_last { guide.push_str("└─"); } else { guide.push_str("├─"); }
                    }

                    let (icon, icon_color) = Self::get_file_icon(&entry.path);
                    let mut name_style = Style::default();
                    let icon_style = Style::default().fg(icon_color);

                    if name.starts_with('.') || entry.is_ignored {
                        name_style = name_style.fg(Color::DarkGray);
                    } else if entry.is_dir {
                        name_style = name_style.fg(Color::LightBlue);
                    }

                    let mut spans = vec![
                        Span::raw(" "),
                        Span::styled(guide, Style::default().fg(Color::DarkGray)),
                        Span::styled(format!("{} ", icon), icon_style),
                        Span::styled(name, name_style),
                        Span::raw("    "),
                    ];

                    let mut line_style = Style::default();
                    if i == explorer.selected_idx {
                        line_style = line_style.bg(Color::Rgb(40, 40, 40));
                        spans[3] = spans[3].clone().bold();
                    }

                    ListItem::new(Line::from(spans)).style(line_style)
                })
                .collect();

            frame.render_widget(List::new(items), explorer_layout[1]);
        }

        // 2. Editor Area
        let buffer = editor.buffer();
        let cursor = editor.cursor();
        
        let editor_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(6), Constraint::Min(1)])
            .split(main_chunks[1]);

        let mut line_numbers = Text::default();
        for i in 1..=buffer.lines.len() {
            line_numbers.lines.push(Line::from(vec![
                Span::styled(format!("{:>4} ", i), Style::default().fg(Color::DarkGray))
            ]));
        }
        frame.render_widget(Paragraph::new(line_numbers).alignment(Alignment::Right), editor_layout[0]);

        let mut text = Text::default();
        let search_query = &vim.search_query;

        for (y, line) in buffer.lines.iter().enumerate() {
            let mut spans = Vec::new();
            let syntax_styles = editor.highlighter.highlight_line(line);
            for (x, c) in line.chars().enumerate() {
                let mut style = syntax_styles.get(x).copied().unwrap_or(editor.highlighter.colors.normal);
                if let Some(start) = vim.selection_start {
                    let cur = crate::vim::Position { x: cursor.x, y: cursor.y };
                    let (s_y, s_x, e_y, e_x) = if (start.y, start.x) < (cur.y, cur.x) { (start.y, start.x, cur.y, cur.x) } else { (cur.y, cur.x, start.y, start.x) };
                    let is_in_range = if y > s_y && y < e_y { true } else if y == s_y && y == e_y { x >= s_x && x <= e_x } else if y == s_y { x >= s_x } else if y == e_y { x <= e_x } else { false };
                    if is_in_range { style = style.add_modifier(Modifier::REVERSED); }
                }
                if !search_query.is_empty() {
                    if let Some(pos) = line.to_lowercase().find(&search_query.to_lowercase()) {
                        if x >= pos && x < pos + search_query.len() {
                            style = style.bg(Color::Yellow).fg(Color::Black);
                        }
                    }
                }
                if vim.yank_highlight_line == Some(y) { style = style.bg(Color::Blue).fg(Color::White); }
                spans.push(Span::styled(c.to_string(), style));
            }
            if line.is_empty() { spans.push(Span::raw(" ")); }
            text.lines.push(Line::from(spans));
        }
        frame.render_widget(Paragraph::new(text), editor_layout[1]);

        // 3. Status Line
        let mode_text = format!("{:?}", vim.mode).to_uppercase();
        let file_name = buffer.file_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("[No Name]");
        let status_text = format!(" {} | {} | {}:{} (Buffer {}/{}) ", mode_text, file_name, cursor.y + 1, cursor.x + 1, editor.active_idx + 1, editor.buffers.len());
        frame.render_widget(Paragraph::new(status_text), root_chunks[1]);

        // 4. Command/Search/Input Line
        match vim.mode {
            Mode::Command => {
                let text = format!(":{}", vim.command_buffer);
                frame.render_widget(Paragraph::new(text), root_chunks[2]);
                frame.set_cursor_position((root_chunks[2].x + vim.command_buffer.len() as u16 + 1, root_chunks[2].y));
            }
            Mode::Search => {
                let text = format!("/{}", vim.search_query);
                frame.render_widget(Paragraph::new(text), root_chunks[2]);
                frame.set_cursor_position((root_chunks[2].x + vim.search_query.len() as u16 + 1, root_chunks[2].y));
            }
            Mode::ExplorerInput(input_type) => {
                let prompt = match input_type {
                    ExplorerInputType::Add => "Add a new file or directory (directories end with a \"/\"): ",
                    ExplorerInputType::Rename => "New File Name: ",
                    ExplorerInputType::Move => "Move To: ",
                    ExplorerInputType::DeleteConfirm => "Delete selected? (y/n): ",
                    ExplorerInputType::Filter => "", 
                };
                if input_type != ExplorerInputType::Filter {
                    let text = format!("{}{}", prompt, vim.input_buffer);
                    frame.render_widget(Paragraph::new(text), root_chunks[2]);
                    frame.set_cursor_position((root_chunks[2].x + prompt.len() as u16 + vim.input_buffer.len() as u16, root_chunks[2].y));
                } else {
                    frame.render_widget(Paragraph::new(""), root_chunks[2]);
                }
            }
            _ => {
                frame.render_widget(Paragraph::new(""), root_chunks[2]);
                if vim.focus == crate::vim::mode::Focus::Editor {
                    frame.set_cursor_position((editor_layout[1].x + cursor.x as u16, editor_layout[1].y + cursor.y as u16));
                }
            }
        }
    }
}
