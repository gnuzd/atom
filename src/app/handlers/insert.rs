use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lsp_types::CompletionTriggerKind;

impl App {
    pub fn handle_insert_mode(&mut self, key: KeyEvent) {
        let action = self.keymap_insert.resolve(&key).clone();
        match action {
            Action::ExitMode => self.dispatch_action(Action::ExitMode, 1),
            Action::Save => self.dispatch_action(Action::Save, 1),
            Action::Confirm => self.dispatch_action(Action::Confirm, 1),
            Action::PasteFromClipboard => self.dispatch_action(Action::PasteFromClipboard, 1),
            Action::SelectNext => self.dispatch_action(Action::SelectNext, 1),
            Action::SelectPrev => self.dispatch_action(Action::SelectPrev, 1),
            Action::Indent => self.dispatch_action(Action::Indent, 1),
            Action::MoveLineStart => self.dispatch_action(Action::MoveLineStart, 1),
            Action::MoveLineEnd => self.dispatch_action(Action::MoveLineEnd, 1),
            _ => self.handle_insert_raw_key(key),
        }
    }

    fn handle_insert_raw_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                    self.dispatch_action(Action::SelectPrev, 1);
                } else {
                    self.editor.move_up();
                }
            }
            KeyCode::Down => {
                if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                    self.dispatch_action(Action::SelectNext, 1);
                } else {
                    self.editor.move_down();
                }
            }
            KeyCode::Left => self.editor.move_left(),
            KeyCode::Right => self.editor.move_right(),
            // Ctrl+Space or NUL: manually trigger LSP completion.
            KeyCode::Char(' ') | KeyCode::Null
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.code == KeyCode::Null =>
            {
                self.trigger_lsp_completion(CompletionTriggerKind::INVOKED, None);
            }
            KeyCode::PageUp | KeyCode::Home => self.dispatch_action(Action::MoveLineStart, 1),
            KeyCode::PageDown | KeyCode::End => self.dispatch_action(Action::MoveLineEnd, 1),
            KeyCode::Char(c) => self.handle_insert_char(c),
            KeyCode::Backspace => self.handle_insert_backspace(),
            _ => {}
        }
    }

    fn handle_insert_char(&mut self, c: char) {
        let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
        let idx = self.safe_line_to_char(y) + x;

        // Auto-pair and auto-close HTML tags.
        let mut to_insert = c.to_string();
        match c {
            '(' => to_insert.push(')'),
            '[' => to_insert.push(']'),
            '{' => to_insert.push('}'),
            '\'' => to_insert.push('\''),
            '"' => to_insert.push('"'),
            '>' => {
                if let Some(line) = self.editor.buffer().line(y) {
                    let line_str = line.to_string();
                    let before_cursor = &line_str[..x.min(line_str.len())];
                    if let Some(tag_start) = before_cursor.rfind('<') {
                        let tag_content = &before_cursor[tag_start + 1..];
                        if !tag_content.is_empty()
                            && !tag_content.contains(' ')
                            && !tag_content.contains('/')
                        {
                            to_insert.push_str(&format!("</{}>", tag_content));
                        }
                    }
                }
            }
            _ => {}
        }

        self.editor.buffer_mut().apply_edit(|t| {
            t.insert(idx, &to_insert);
        });
        self.editor.cursor_mut().x += 1;

        // Notify LSP and request completions when appropriate.
        if let Some(path) = self.editor.buffer().file_path.clone() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                let is_trigger = c == '.' || c == ':' || c == '>';
                let is_alpha = c.is_alphanumeric() || c == '_';

                let text = self.editor.buffer().text.to_string();
                let _ = self.lsp_manager.did_change(&ext, &path, text);
                self.last_lsp_update = Some(std::time::Instant::now());

                if is_trigger || is_alpha {
                    let trigger_kind = if is_trigger {
                        CompletionTriggerKind::TRIGGER_CHARACTER
                    } else {
                        CompletionTriggerKind::INVOKED
                    };
                    let trigger_char = if is_trigger { Some(c.to_string()) } else { None };
                    let _ = self.lsp_manager.request_completions(
                        &ext, &path, y, x + 1, trigger_kind, trigger_char,
                    );
                } else {
                    self.vim.show_suggestions = false;
                    self.vim.suggestions.clear();
                    self.vim.filtered_suggestions.clear();
                }
            }
        }
        self.refresh_filtered_suggestions();
    }

    fn handle_insert_backspace(&mut self) {
        let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);

        if x > 0 {
            // Delete character before cursor on the same line.
            let idx = self.safe_line_to_char(y) + x;
            self.editor.buffer_mut().apply_edit(|t| {
                t.remove((idx - 1)..idx);
            });
            self.editor.cursor_mut().x -= 1;

            if let Some(path) = self.editor.buffer().file_path.clone() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                    let should_notify = self
                        .last_lsp_update
                        .map_or(true, |t| t.elapsed() > Duration::from_millis(200));
                    if should_notify {
                        let text = self.editor.buffer().text.to_string();
                        let _ = self.lsp_manager.did_change(&ext, &path, text);
                        self.last_lsp_update = Some(std::time::Instant::now());
                    }
                    if self.vim.suggestions.is_empty() {
                        self.vim.show_suggestions = false;
                    }
                }
            }
            self.refresh_filtered_suggestions();
        } else if y > 0 {
            // Merge current line into the previous line.
            let prev_line = self.editor.buffer().text.line(y - 1);
            let prev_len = prev_line.len_chars();
            let has_newline = prev_line.chars().last().is_some_and(|c| c == '\n' || c == '\r');
            let new_x = if has_newline { prev_len - 1 } else { prev_len };

            let char_idx = self.safe_line_to_char(y);
            self.editor.buffer_mut().apply_edit(|t| {
                t.remove((char_idx - 1)..char_idx);
            });
            self.editor.cursor_mut().y -= 1;
            self.editor.cursor_mut().x = new_x;

            if let Some(path) = self.editor.buffer().file_path.clone() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                    let text = self.editor.buffer().text.to_string();
                    let _ = self.lsp_manager.did_change(&ext, &path, text);
                }
            }
        }
    }

    /// Requests LSP completions at the current cursor position.
    fn trigger_lsp_completion(&mut self, kind: CompletionTriggerKind, trigger_char: Option<String>) {
        if let Some(path) = self.editor.buffer().file_path.clone() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                let _ = self.lsp_manager.request_completions(&ext, &path, y, x, kind, trigger_char);
            }
        }
    }
}
