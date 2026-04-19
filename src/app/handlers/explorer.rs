use super::*;
use crossterm::event::{KeyCode, KeyEvent};
use crate::vim::mode::ExplorerInputType;

impl App {
    pub fn handle_explorer_input_mode(&mut self, key: KeyEvent, input_type: ExplorerInputType) {
        match key.code {
            KeyCode::Esc => {
                if let ExplorerInputType::Filter = input_type {
                    self.explorer.filter.clear();
                    self.explorer.refresh();
                }
                self.vim.mode = Mode::Normal;
            }
            KeyCode::Char(c) => {
                self.vim.input_buffer.push(c);
                if let ExplorerInputType::Filter = input_type {
                    self.explorer.filter = self.vim.input_buffer.clone();
                    self.explorer.refresh();
                }
            }
            KeyCode::Backspace => {
                self.vim.input_buffer.pop();
                if let ExplorerInputType::Filter = input_type {
                    self.explorer.filter = self.vim.input_buffer.clone();
                    self.explorer.refresh();
                }
            }
            KeyCode::Enter => {
                let input = self.vim.input_buffer.clone();
                self.vim.input_buffer.clear();
                self.vim.mode = Mode::Normal;
                match input_type {
                    ExplorerInputType::Add => {
                        if let Err(e) = self.explorer.create_file(&input) {
                            self.vim.set_message(format!("Error: {}", e));
                        }
                    }
                    ExplorerInputType::Rename => {
                        if let Err(e) = self.explorer.rename_selected(&input) {
                            self.vim.set_message(format!("Error: {}", e));
                        }
                    }
                    ExplorerInputType::Move => {
                        if let Err(e) = self.explorer.move_selected(Path::new(&input)) {
                            self.vim.set_message(format!("Error: {}", e));
                        }
                    }
                    ExplorerInputType::DeleteConfirm => {
                        if input.to_lowercase() == "y" {
                            if let Err(e) = self.explorer.delete_selected() {
                                self.vim.set_message(format!("Error: {}", e));
                            }
                        }
                    }
                    ExplorerInputType::Filter => {
                        self.explorer.filter = input;
                        self.explorer.refresh();
                    }
                }
            }
            _ => {}
        }
    }
}
