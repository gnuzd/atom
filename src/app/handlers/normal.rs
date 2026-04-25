use super::*;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub fn handle_normal_mode(&mut self, key: KeyEvent) {
        let mut action = Action::Unbound;
        let is_in_sequence = !self.vim.input_buffer.is_empty();

        if !is_in_sequence {
            action = match self.vim.focus {
                Focus::Editor => self.keymap_normal.resolve(&key),
                Focus::Explorer => self.keymap_explorer.resolve(&key),
                Focus::Trouble => self.keymap_normal.resolve(&key),
            }
            .clone();
        }

        match action {
            Action::Unbound => self.handle_normal_unbound(key),
            action => {
                let count = self.consume_count_buffer();
                self.dispatch_action(action.clone(), count);
            }
        }
    }

    fn handle_normal_unbound(&mut self, key: KeyEvent) {
        match self.vim.focus {
            Focus::Editor | Focus::Trouble => self.handle_normal_editor_unbound(key),
            Focus::Explorer => self.handle_normal_explorer_unbound(key),
        }
    }

    fn handle_normal_editor_unbound(&mut self, key: KeyEvent) {
        if let KeyCode::Char(c) = key.code {
            // Accumulate digit counts (e.g. "3j" moves down 3 lines).
            if c.is_ascii_digit()
                && (self.vim.input_buffer.is_empty()
                    || self.vim.input_buffer.chars().all(|d| d.is_ascii_digit()))
            {
                self.vim.input_buffer.push(c);
                return; // wait for the motion key
            }

            let count = self.consume_count_buffer();
            self.vim.input_buffer.push(c);
            let seq = self.vim.input_buffer.clone();
            let mut matched = true;

            match seq.as_str() {
                " ff" => self.dispatch_action(Action::TelescopeFiles, count),
                " fg" => self.dispatch_action(Action::TelescopeLiveGrep, count),
                " fb" => self.dispatch_action(Action::TelescopeBuffers, count),
                " th" | "th" => self.dispatch_action(Action::TelescopeThemes, count),
                " n" => self.dispatch_action(Action::ToggleRelativeNumber, count),
                " /" => self.dispatch_action(Action::ToggleComment, count),
                " tt" => self.dispatch_action(Action::ToggleTrouble, count),
                " bb" => self.dispatch_action(Action::ToggleAutoformat, count),
                " bl" => self.dispatch_action(Action::GitBlame, count),
                " x" => self.dispatch_action(Action::CloseBuffer, count),
                "gg" | "[[" => self.dispatch_action(Action::JumpToFirstLine, count),
                "]]" => self.dispatch_action(Action::JumpToLastLine, count),
                "dd" => self.dispatch_action(Action::DeleteLine, count),
                "yy" => self.dispatch_action(Action::YankLine, count),
                "gd" => self.dispatch_action(Action::LspDefinition, count),
                "zc" | "za" => self.dispatch_action(Action::ToggleFold, count),
                "]g" => self.dispatch_action(Action::NextHunk, count),
                "[g" => self.dispatch_action(Action::PrevHunk, count),
                "ZZ" => self.dispatch_action(Action::SaveAndQuit, 1),
                "ZQ" => self.dispatch_action(Action::QuitWithoutSaving, 1),
                _ => {
                    matched = false;
                }
            }

            if matched {
                self.vim.input_buffer.clear();
            } else {
                let is_partial = matches!(
                    seq.as_str(),
                    " " | " f" | " t" | " g" | " b" | "[" | "]" | "z" | "d" | "y" | "g" | "Z"
                );
                if !is_partial {
                    self.vim.input_buffer.clear();
                    let fallback = self.keymap_normal.resolve(&key).clone();
                    if fallback != Action::Unbound {
                        self.dispatch_action(fallback, count);
                    }
                }
            }
        } else {
            self.vim.input_buffer.clear();
            if key.code == KeyCode::Esc {
                self.vim.selection_start = None;
            }
        }
    }

    fn handle_normal_explorer_unbound(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('<') => self.explorer.decrease_width(),
            KeyCode::Char('>') => self.explorer.increase_width(),
            KeyCode::Char('y') => {
                if let Some(entry) = self.explorer.selected_entry() {
                    self.vim.register = entry.path.to_string_lossy().to_string();
                    self.vim.set_message("Path copied to register".to_string());
                }
            }
            _ => {}
        }
    }

    /// Parses the digit prefix in `input_buffer` as a repeat count, clears the buffer, and
    /// returns the count (defaulting to 1 if the buffer is empty or non-numeric).
    fn consume_count_buffer(&mut self) -> usize {
        if !self.vim.input_buffer.is_empty()
            && self.vim.input_buffer.chars().all(|c| c.is_ascii_digit())
        {
            let count = self.vim.input_buffer.parse::<usize>().unwrap_or(1);
            self.vim.input_buffer.clear();
            count
        } else {
            1
        }
    }
}
