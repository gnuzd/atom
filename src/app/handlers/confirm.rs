use super::*;
use crossterm::event::{KeyCode, KeyEvent};
use crate::vim::mode::ConfirmAction;

impl App {
    pub fn handle_confirm_mode(&mut self, key: KeyEvent, action: ConfirmAction) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => match action {
                ConfirmAction::Quit => {
                    self.save_and_format(None);
                    self.should_quit = true;
                }
                ConfirmAction::CloseBuffer => {
                    self.save_and_format(None);
                    if let Some(removed) = self.editor.close_current_buffer() {
                        self.vim.pane_layout.update_buffer_indices(removed);
                    }
                    self.vim.mode = Mode::Normal;
                }
            },
            KeyCode::Char('n') | KeyCode::Char('N') => match action {
                ConfirmAction::Quit => {
                    self.should_quit = true;
                }
                ConfirmAction::CloseBuffer => {
                    if let Some(removed) = self.editor.close_current_buffer() {
                        self.vim.pane_layout.update_buffer_indices(removed);
                    }
                    self.vim.mode = Mode::Normal;
                }
            },
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                self.vim.mode = Mode::Normal;
            }
            _ => {}
        }
    }
}
