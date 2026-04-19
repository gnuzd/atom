use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub fn handle_nucleus_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if self.vim.nucleus_pending_delete.is_some() {
                    // Cancel pending delete instead of closing Nucleus
                    self.vim.nucleus_pending_delete = None;
                } else {
                    self.dispatch_action(Action::ExitMode, 1);
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                // Moving to a different item cancels any pending delete
                self.vim.nucleus_pending_delete = None;
                self.dispatch_action(Action::SelectNext, 1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.vim.nucleus_pending_delete = None;
                self.dispatch_action(Action::SelectPrev, 1);
            }
            KeyCode::Char('1') => self.set_nucleus_tab(0),
            KeyCode::Char('2') => self.set_nucleus_tab(1),
            KeyCode::Char('3') => self.set_nucleus_tab(2),
            KeyCode::Char('4') => self.set_nucleus_tab(3),
            KeyCode::Char('5') => self.set_nucleus_tab(4),
            KeyCode::Char('6') => self.set_nucleus_tab(5),
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.vim.nucleus_pending_delete = None;
                self.vim.mode = Mode::NucleusFilter;
                self.vim.nucleus_filter.clear();
            }
            KeyCode::Char(' ')
            | KeyCode::Char('i')
            | KeyCode::Char('u')
            | KeyCode::Char('d')
            | KeyCode::Char('x') => self.install_selected_package(key),
            _ => {}
        }
    }

    pub fn handle_nucleus_filter_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.vim.mode = Mode::Nucleus;
            }
            KeyCode::Char(c) => {
                self.vim.nucleus_filter.push(c);
                self.vim.nucleus_state.select(Some(0));
            }
            KeyCode::Backspace => {
                self.vim.nucleus_filter.pop();
                self.vim.nucleus_state.select(Some(0));
            }
            _ => {}
        }
    }

    pub fn handle_keymaps_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') => self.dispatch_action(Action::ExitMode, 1),
            KeyCode::Char('j') | KeyCode::Down => self.dispatch_action(Action::SelectNext, 1),
            KeyCode::Char('k') | KeyCode::Up => self.dispatch_action(Action::SelectPrev, 1),
            KeyCode::Char(c) => {
                self.vim.keymap_filter.push(c);
                self.vim.keymap_state.select(Some(0));
            }
            KeyCode::Backspace => {
                self.vim.keymap_filter.pop();
                self.vim.keymap_state.select(Some(0));
            }
            _ => {}
        }
    }

    fn set_nucleus_tab(&mut self, tab: usize) {
        self.vim.nucleus_tab = tab;
        self.vim.nucleus_state.select(Some(0));
    }
}
