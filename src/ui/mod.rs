use ratatui::{
    layout::{Constraint, Direction, Layout},
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
                Constraint::Length(1), // Command Line
            ])
            .split(frame.area());

        // Main Editor Area - No Border
        let lines: Vec<String> = editor.buffer.lines.clone();
        let editor_paragraph = Paragraph::new(lines.join("\n"));
        frame.render_widget(editor_paragraph, chunks[0]);

        // Status Line
        let mode_text = format!("{:?}", vim.mode).to_uppercase();
        let file_name = editor.buffer.file_path.as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[No Name]");
        let status_text = format!(" {} | {} | {}:{} ", mode_text, file_name, editor.cursor.y + 1, editor.cursor.x + 1);
        let status_bar = Paragraph::new(status_text);
        frame.render_widget(status_bar, chunks[1]);

        // Command Line
        if vim.mode == crate::vim::mode::Mode::Command {
            let command_text = format!(":{}", vim.command_buffer);
            let command_bar = Paragraph::new(command_text);
            frame.render_widget(command_bar, chunks[2]);
            
            frame.set_cursor_position((
                chunks[2].x + vim.command_buffer.len() as u16 + 1,
                chunks[2].y,
            ));
        } else {
            // Clear command line area when not in command mode
            frame.render_widget(Paragraph::new(""), chunks[2]);

            frame.set_cursor_position((
                chunks[0].x + editor.cursor.x as u16,
                chunks[0].y + editor.cursor.y as u16,
            ));
        }
    }
}
