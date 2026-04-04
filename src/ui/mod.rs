pub mod colorscheme;
pub mod explorer;
pub mod icons;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, BorderType, List, ListItem, Padding, Paragraph},
    Frame,
};
use crate::vim::mode::{Mode, ExplorerInputType, Focus};

pub struct TerminalUi;

impl TerminalUi {
    pub fn new() -> Self {
        Self
    }

    fn get_file_icon(path: &std::path::Path) -> (&'static str, String) {
        if path.is_dir() {
            return (icons::FOLDER, "TreeExplorerFolderIcon".into());
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext {
            "rs" => (icons::STRUCT, "Identifier".into()),
            "toml" => (icons::PACKAGE, "Type".into()),
            "md" => (icons::TEXT, "Function".into()),
            "lock" => (icons::FILE, "Comment".into()),
            _ => (icons::FILE, "TreeExplorerFileIcon".into()),
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
        let theme = &editor.highlighter.theme;
        
        // Fill full screen background
        frame.render_widget(Block::default().style(theme.get("Normal")), area);

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
            let sidebar_divider = Block::default()
                .borders(Borders::RIGHT)
                .border_style(theme.get("TreeExplorerConnector"));
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

            // Explorer Header Box
            let header_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled(" Explorer ", theme.get("TreeExplorerRoot")))
                .border_style(theme.get("TreeExplorerConnector"))
                .padding(Padding::horizontal(1));
            
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
            frame.render_widget(Paragraph::new(filter_display).style(theme.get("Keyword")), header_chunks[0]);

            let count_text = format!("{}/{}", explorer.entries.len(), explorer.entries.len());
            frame.render_widget(Paragraph::new(count_text).alignment(Alignment::Right).style(theme.get("Comment")), header_chunks[1]);

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

                    let (icon, icon_group) = Self::get_file_icon(&entry.path);
                    let mut name_style = if entry.is_dir { theme.get("TreeExplorerFolderName") } else { theme.get("TreeExplorerFileName") };
                    let icon_style = if name.starts_with('.') || entry.is_ignored { theme.get("Comment") } else { theme.get(&icon_group) };

                    if name.starts_with('.') || entry.is_ignored {
                        name_style = theme.get("Comment");
                    }

                    let mut spans = vec![
                        Span::raw(" "),
                        Span::styled(guide, theme.get("TreeExplorerConnector")),
                        Span::styled(format!("{} ", icon), icon_style),
                        Span::styled(name, name_style),
                        Span::raw("    "),
                    ];

                    let mut line_style = Style::default();
                    if i == explorer.selected_idx {
                        line_style = theme.get("CursorLine");
                        spans[3] = spans[3].clone().add_modifier(Modifier::BOLD);
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

        // Line Numbers
        let mut line_numbers = Text::default();
        for i in 1..=buffer.lines.len() {
            let is_active = i - 1 == cursor.y;
            let style = if is_active { theme.get("CursorLineNr") } else { theme.get("LineNr") };
            line_numbers.lines.push(Line::from(vec![
                Span::styled(format!("{:>4} ", i), style)
            ]));
        }
        frame.render_widget(Paragraph::new(line_numbers).alignment(Alignment::Right).style(theme.get("Normal")), editor_layout[0]);

        // Code Content
        let mut text = Text::default();
        let search_query = &vim.search_query;

        for (y, line) in buffer.lines.iter().enumerate() {
            let mut spans = Vec::new();
            let syntax_styles = editor.highlighter.highlight_line(line);
            let is_current_line = y == cursor.y;

            for (x, c) in line.chars().enumerate() {
                let mut style = syntax_styles.get(x).copied().unwrap_or(theme.get("Normal"));
                
                if is_current_line {
                    style = style.bg(theme.palette.black2);
                }

                if let Some(start) = vim.selection_start {
                    let cur = crate::vim::Position { x: cursor.x, y: cursor.y };
                    let (s_y, s_x, e_y, e_x) = if (start.y, start.x) < (cur.y, cur.x) { (start.y, start.x, cur.y, cur.x) } else { (cur.y, cur.x, start.y, start.x) };
                    let is_in_range = if y > s_y && y < e_y { true } else if y == s_y && y == e_y { x >= s_x && x <= e_x } else if y == s_y { x >= s_x } else if y == e_y { x <= e_x } else { false };
                    if is_in_range { style = theme.get("Visual"); }
                }
                if !search_query.is_empty() {
                    if let Some(pos) = line.to_lowercase().find(&search_query.to_lowercase()) {
                        if x >= pos && x < pos + search_query.len() {
                            style = theme.get("Search");
                        }
                    }
                }
                if vim.yank_highlight_line == Some(y) { style = Style::default().bg(theme.palette.blue).fg(theme.palette.black); }
                spans.push(Span::styled(c.to_string(), style));
            }
            if line.is_empty() { 
                let style = if is_current_line { theme.get("CursorLine") } else { theme.get("Normal") };
                spans.push(Span::styled(" ", style)); 
            }
            
            let mut line_obj = Line::from(spans);
            if is_current_line {
                line_obj = line_obj.style(theme.get("CursorLine"));
            }
            text.lines.push(line_obj);
        }
        frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), editor_layout[1]);

        // 3. Status Line
        let (mode_group, mode_label) = match vim.mode {
            Mode::Normal => ("StatusLineNormal", " NORMAL "),
            Mode::Insert => ("StatusLineInsert", " INSERT "),
            Mode::Visual => ("StatusLineVisual", " VISUAL "),
            Mode::Command => ("StatusLineCommand", " COMMAND "),
            _ => ("StatusLine", " OTHER "),
        };

        let file_name = buffer.file_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("[No Name]");
        
        let status_line = Line::from(vec![
            Span::styled(mode_label, theme.get(mode_group)),
            Span::styled(format!(" {} ", file_name), theme.get("StatusLineFile")),
            Span::styled(" ", theme.get("StatusLine")), // Filler
            Span::styled(format!(" {}:{} (Buffer {}/{}) ", cursor.y + 1, cursor.x + 1, editor.active_idx + 1, editor.buffers.len()), theme.get("StatusLinePos")),
        ]);
        
        frame.render_widget(Paragraph::new(status_line).style(theme.get("StatusLine")), root_chunks[1]);

        // 4. Command/Search/Input Line
        match vim.mode {
            Mode::Command => {
                let text = format!(":{}", vim.command_buffer);
                frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]);
                frame.set_cursor_position((root_chunks[2].x + vim.command_buffer.len() as u16 + 1, root_chunks[2].y));
            }
            Mode::Search => {
                let text = format!("/{}", vim.search_query);
                frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]);
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
                    frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]);
                    frame.set_cursor_position((root_chunks[2].x + prompt.len() as u16 + vim.input_buffer.len() as u16, root_chunks[2].y));
                } else {
                    frame.render_widget(Paragraph::new("").style(theme.get("Normal")), root_chunks[2]);
                }
            }
            _ => {
                frame.render_widget(Paragraph::new("").style(theme.get("Normal")), root_chunks[2]);
                if vim.focus == Focus::Editor {
                    frame.set_cursor_position((editor_layout[1].x + cursor.x as u16, editor_layout[1].y + cursor.y as u16));
                }
            }
        }
    }
}
