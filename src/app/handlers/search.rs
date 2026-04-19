use super::*;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub fn handle_search_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => self.vim.mode = Mode::Normal,
            KeyCode::Char(c) => self.vim.search_query.push(c),
            KeyCode::Backspace => {
                self.vim.search_query.pop();
            }
            _ => {}
        }
    }
}
