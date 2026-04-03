use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
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
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(frame.area());

        // Main Editor Area - No Border
        let lines: Vec<String> = editor.buffer.lines.clone();
        let editor_paragraph = Paragraph::new(lines.join("\n"));
        frame.render_widget(editor_paragraph, chunks[0]);

        // Status Line
        let mode_text = format!("{:?}", vim.mode).to_uppercase();
        let status_text = format!(" {} | {}:{} ", mode_text, editor.cursor.y + 1, editor.cursor.x + 1);
        let status_bar = Paragraph::new(status_text);
        frame.render_widget(status_bar, chunks[1]);

        // Set Cursor (No offset since border is gone)
        frame.set_cursor_position((
            chunks[0].x + editor.cursor.x as u16,
            chunks[0].y + editor.cursor.y as u16,
        ));
    }
}
