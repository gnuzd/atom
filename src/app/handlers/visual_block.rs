use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub fn handle_visual_block_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.vim.mode = Mode::Normal;
                self.vim.selection_start = None;
            }
            KeyCode::Char('j') | KeyCode::Down => self.dispatch_action(Action::MoveDown, 1),
            KeyCode::Char('k') | KeyCode::Up => self.dispatch_action(Action::MoveUp, 1),
            KeyCode::Char('h') | KeyCode::Left => self.dispatch_action(Action::MoveLeft, 1),
            KeyCode::Char('l') | KeyCode::Right => self.dispatch_action(Action::MoveRight, 1),
            KeyCode::Char('w') => self.dispatch_action(Action::MoveWordForward, 1),
            KeyCode::Char('b') => self.dispatch_action(Action::MoveWordBackward, 1),
            KeyCode::End => self.dispatch_action(Action::MoveLineEnd, 1),
            KeyCode::Home => self.dispatch_action(Action::MoveLineStart, 1),
            KeyCode::Char('I') => self.enter_block_insert(),
            KeyCode::Char('d') | KeyCode::Char('x') => self.delete_visual_block(),
            _ => {}
        }
    }

    pub fn handle_block_insert_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.exit_block_insert(),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.block_insert_char(c);
            }
            KeyCode::Backspace => self.block_insert_backspace(),
            _ => {}
        }
    }

    fn enter_block_insert(&mut self) {
        let anchor = match self.vim.selection_start {
            Some(p) => p,
            None => return,
        };
        let cur = crate::vim::Position { x: self.editor.cursor().x, y: self.editor.cursor().y };

        let top_y = anchor.y.min(cur.y);
        let bottom_y = anchor.y.max(cur.y);
        let left_col = anchor.x.min(cur.x);

        // Move cursor to top line, insert column
        self.editor.cursor_mut().y = top_y;
        self.editor.cursor_mut().x = left_col;

        // Store bottom_y in selection_start so exit_block_insert can recover the full range.
        // (cursor is moved to top_y, so we'd lose bottom_y otherwise.)
        self.vim.selection_start = Some(crate::vim::Position { x: left_col, y: bottom_y });
        self.vim.block_insert_col = left_col;
        self.vim.block_insert_text.clear();
        self.vim.mode = Mode::BlockInsert;
    }

    fn block_insert_char(&mut self, c: char) {
        let y = self.editor.cursor().y;
        let x = self.editor.cursor().x;
        let idx = self.safe_line_to_char(y) + x;

        self.editor.buffer_mut().apply_edit(|t| {
            t.insert_char(idx, c);
        });
        self.editor.cursor_mut().x += 1;
        self.vim.block_insert_text.push(c);
    }

    fn block_insert_backspace(&mut self) {
        if self.vim.block_insert_text.is_empty() {
            return;
        }
        let y = self.editor.cursor().y;
        let x = self.editor.cursor().x;
        if x > self.vim.block_insert_col {
            let idx = self.safe_line_to_char(y) + x;
            self.editor.buffer_mut().apply_edit(|t| {
                t.remove((idx - 1)..idx);
            });
            self.editor.cursor_mut().x -= 1;
            self.vim.block_insert_text.pop();
        }
    }

    fn exit_block_insert(&mut self) {
        let text = self.vim.block_insert_text.clone();
        if text.is_empty() {
            self.vim.mode = Mode::Normal;
            self.vim.selection_start = None;
            return;
        }

        let anchor = match self.vim.selection_start {
            Some(p) => p,
            None => {
                self.vim.mode = Mode::Normal;
                return;
            }
        };
        // cursor is at top_y (set in enter_block_insert); anchor.y holds bottom_y.
        let top_y = self.editor.cursor().y;
        let bottom_y = anchor.y;
        let col = self.vim.block_insert_col;

        // Apply text to all other lines in the block (top_y is already modified)
        self.editor.buffer_mut().push_history();
        for line_y in top_y + 1..=bottom_y {
            let line_len = self.editor.buffer().line(line_y)
                .map(|l| l.chars().filter(|&c| c != '\n' && c != '\r').count())
                .unwrap_or(0);
            let insert_col = col.min(line_len);
            let idx = self.safe_line_to_char(line_y) + insert_col;
            let text_clone = text.clone();
            self.editor.buffer_mut().apply_edit(|t| {
                t.insert(idx, &text_clone);
            });
        }

        self.vim.block_insert_text.clear();
        self.vim.mode = Mode::Normal;
        self.vim.selection_start = None;
        self.editor.clamp_cursor();
    }

    fn delete_visual_block(&mut self) {
        let anchor = match self.vim.selection_start {
            Some(p) => p,
            None => {
                self.vim.mode = Mode::Normal;
                return;
            }
        };
        let cur = crate::vim::Position { x: self.editor.cursor().x, y: self.editor.cursor().y };

        let top_y = anchor.y.min(cur.y);
        let bottom_y = anchor.y.max(cur.y);
        let left_col = anchor.x.min(cur.x);
        let right_col = anchor.x.max(cur.x);

        self.editor.buffer_mut().push_history();
        // Delete from bottom up to avoid index shifting
        for line_y in (top_y..=bottom_y).rev() {
            let line_len = self.editor.buffer().line(line_y)
                .map(|l| l.chars().filter(|&c| c != '\n' && c != '\r').count())
                .unwrap_or(0);
            if left_col >= line_len {
                continue;
            }
            let actual_right = (right_col + 1).min(line_len);
            let line_start = self.safe_line_to_char(line_y);
            let start_idx = line_start + left_col;
            let end_idx = line_start + actual_right;
            self.editor.buffer_mut().apply_edit(|t| {
                t.remove(start_idx..end_idx);
            });
        }

        self.vim.mode = Mode::Normal;
        self.vim.selection_start = None;
        self.editor.clamp_cursor();
    }
}
