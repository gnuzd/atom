pub mod colorscheme;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier},
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

pub struct TerminalUi;

impl TerminalUi {
    pub fn new() -> Self {
        Self
    }

    pub fn draw(&self, frame: &mut Frame, editor: &crate::editor::Editor, vim: &crate::vim::VimState) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // Editor
                Constraint::Length(1), // Status Line
                Constraint::Length(1), // Command/Search Line
            ])
            .split(frame.area());

        let buffer = editor.buffer();
        let cursor = editor.cursor();

        // Editor Area
        let mut text = Text::default();
        let search_query = &vim.search_query;
        
        for (y, line) in buffer.lines.iter().enumerate() {
            let mut spans = Vec::new();
            let syntax_styles = editor.highlighter.highlight_line(line);
            
            // Collect matches if search query is not empty
            let mut search_matches = Vec::new();
            if !search_query.is_empty() {
                let mut start = 0;
                while let Some(pos) = line[start..].find(search_query) {
                    let absolute_pos = start + pos;
                    search_matches.push(absolute_pos..absolute_pos + search_query.len());
                    start = absolute_pos + 1;
                }
            }

            for (x, c) in line.chars().enumerate() {
                let mut style = syntax_styles.get(x).copied().unwrap_or(editor.highlighter.colors.normal);
                
                // Visual Mode Selection
                if let Some(start) = vim.selection_start {
                    let cur = crate::vim::Position { x: cursor.x, y: cursor.y };
                    let (s_y, s_x, e_y, e_x) = if (start.y, start.x) < (cur.y, cur.x) {
                        (start.y, start.x, cur.y, cur.x)
                    } else {
                        (cur.y, cur.x, start.y, start.x)
                    };

                    let is_in_range = if y > s_y && y < e_y {
                        true
                    } else if y == s_y && y == e_y {
                        x >= s_x && x <= e_x
                    } else if y == s_y {
                        x >= s_x
                    } else if y == e_y {
                        x <= e_x
                    } else {
                        false
                    };

                    if is_in_range {
                        style = style.add_modifier(Modifier::REVERSED);
                    }
                }
                
                // Search Highlight
                for range in &search_matches {
                    if range.contains(&x) {
                        style = style.bg(Color::Yellow).fg(Color::Black);
                    }
                }
                
                spans.push(Span::styled(c.to_string(), style));
            }
            if line.is_empty() {
                 spans.push(Span::raw(" "));
            }
            text.lines.push(Line::from(spans));
        }

        let editor_paragraph = Paragraph::new(text);
        frame.render_widget(editor_paragraph, chunks[0]);

        // Status Line
        let mode_text = format!("{:?}", vim.mode).to_uppercase();
        let file_name = buffer.file_path.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[No Name]");
        let status_text = format!(" {} | {} | {}:{} (Buffer {}/{})", mode_text, file_name, cursor.y + 1, cursor.x + 1, editor.active_idx + 1, editor.buffers.len());
        let status_bar = Paragraph::new(status_text);
        frame.render_widget(status_bar, chunks[1]);

        // Command/Search Line
        if vim.mode == crate::vim::mode::Mode::Command {
            let command_text = format!(":{}", vim.command_buffer);
            let command_bar = Paragraph::new(command_text);
            frame.render_widget(command_bar, chunks[2]);
            
            frame.set_cursor_position((
                chunks[2].x + vim.command_buffer.len() as u16 + 1,
                chunks[2].y,
            ));
        } else if vim.mode == crate::vim::mode::Mode::Search {
            let search_text = format!("/{}", vim.search_query);
            let search_bar = Paragraph::new(search_text);
            frame.render_widget(search_bar, chunks[2]);

            frame.set_cursor_position((
                chunks[2].x + vim.search_query.len() as u16 + 1,
                chunks[2].y,
            ));
        } else {
            frame.render_widget(Paragraph::new(""), chunks[2]);
            frame.set_cursor_position((
                chunks[0].x + cursor.x as u16,
                chunks[0].y + cursor.y as u16,
            ));
        }
    }
}
