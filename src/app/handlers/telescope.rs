use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub fn handle_telescope_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.dispatch_action(Action::ExitMode, 1),
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.dispatch_action(Action::SelectNext, 1)
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.dispatch_action(Action::SelectPrev, 1)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.vim.telescope.scroll_preview_up(5)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.vim.telescope.scroll_preview_down(5)
            }
            KeyCode::Char(c) => {
                self.vim.telescope.query.push(c);
                self.vim.telescope.update_results(&self.editor);
            }
            KeyCode::Backspace => {
                self.vim.telescope.query.pop();
                self.vim.telescope.update_results(&self.editor);
            }
            KeyCode::Enter => self.dispatch_action(Action::Confirm, 1),
            _ => {}
        }
    }
}
