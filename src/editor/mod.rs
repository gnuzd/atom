use std::{io, path::PathBuf};

pub mod buffer;
pub mod cursor;
pub mod highlighter;

pub struct Editor {
    pub buffers: Vec<buffer::Buffer>,
    pub cursors: Vec<cursor::Cursor>,
    pub active_idx: usize,
    pub highlighter: highlighter::Highlighter,
}

impl Editor {
    pub fn new() -> Self {
        let colors = crate::ui::colorscheme::ColorScheme::default_dark();
        Self {
            buffers: vec![buffer::Buffer::new()],
            cursors: vec![cursor::Cursor::new()],
            active_idx: 0,
            highlighter: highlighter::Highlighter::new(colors),
        }
    }

    pub fn buffer(&self) -> &buffer::Buffer {
        &self.buffers[self.active_idx]
    }

    pub fn buffer_mut(&mut self) -> &mut buffer::Buffer {
        &mut self.buffers[self.active_idx]
    }

    pub fn cursor(&self) -> &cursor::Cursor {
        &self.cursors[self.active_idx]
    }

    pub fn cursor_mut(&mut self) -> &mut cursor::Cursor {
        &mut self.cursors[self.active_idx]
    }

    pub fn open_file(&mut self, path: PathBuf) -> io::Result<()> {
        let new_buffer = buffer::Buffer::load(path)?;
        self.buffers.push(new_buffer);
        self.cursors.push(cursor::Cursor::new());
        self.active_idx = self.buffers.len() - 1;
        Ok(())
    }

    pub fn next_buffer(&mut self) {
        if !self.buffers.is_empty() {
            self.active_idx = (self.active_idx + 1) % self.buffers.len();
        }
    }

    pub fn prev_buffer(&mut self) {
        if !self.buffers.is_empty() {
            if self.active_idx == 0 {
                self.active_idx = self.buffers.len() - 1;
            } else {
                self.active_idx -= 1;
            }
        }
    }

    pub fn close_current_buffer(&mut self) {
        if self.buffers.len() > 1 {
            self.buffers.remove(self.active_idx);
            self.cursors.remove(self.active_idx);
            if self.active_idx >= self.buffers.len() {
                self.active_idx = self.buffers.len() - 1;
            }
        } else {
            self.buffers[0] = buffer::Buffer::new();
            self.cursors[0] = cursor::Cursor::new();
        }
    }

    pub fn save_file(&self) -> io::Result<()> {
        self.buffer().save()
    }

    pub fn save_file_as(&mut self, path: PathBuf) -> io::Result<()> {
        self.buffer_mut().save_as(path)
    }

    pub fn undo(&mut self) -> bool {
        self.buffer_mut().undo()
    }

    pub fn redo(&mut self) -> bool {
        self.buffer_mut().redo()
    }

    pub fn move_up(&mut self) {
        if self.cursor().y > 0 {
            self.cursor_mut().y -= 1;
            let y = self.cursor().y;
            let line_len = self.buffer().lines[y].len();
            if self.cursor().x > line_len {
                self.cursor_mut().x = line_len;
            }
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor().y < self.buffer().lines.len() - 1 {
            self.cursor_mut().y += 1;
            let y = self.cursor().y;
            let line_len = self.buffer().lines[y].len();
            if self.cursor().x > line_len {
                self.cursor_mut().x = line_len;
            }
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor().x > 0 {
            self.cursor_mut().x -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let y = self.cursor().y;
        let x = self.cursor().x;
        if let Some(line) = self.buffer().lines.get(y) {
            if x < line.len() {
                self.cursor_mut().x += 1;
            }
        }
    }

    pub fn jump_to_first_line(&mut self) {
        self.cursor_mut().y = 0;
        self.cursor_mut().x = 0;
    }

    pub fn jump_to_last_line(&mut self) {
        let last_y = self.buffer().lines.len().saturating_sub(1);
        self.cursor_mut().y = last_y;
        self.cursor_mut().x = 0;
    }

    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    pub fn move_word_forward(&mut self) {
        let y = self.cursor().y;
        let x = self.cursor().x;
        let num_lines = self.buffer().lines.len();

        if y >= num_lines { return; }
        let line = &self.buffer().lines[y];
        
        if x >= line.len() {
            if y < num_lines - 1 {
                self.cursor_mut().y += 1;
                self.cursor_mut().x = 0;
                self.move_word_forward();
            }
            return;
        }

        let chars: Vec<char> = line.chars().collect();
        let mut i = x;

        if Self::is_word_char(chars[i]) {
            while i < chars.len() && Self::is_word_char(chars[i]) {
                i += 1;
            }
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
        } else if chars[i].is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
        } else {
            while i < chars.len() && !chars[i].is_whitespace() && !Self::is_word_char(chars[i]) {
                i += 1;
            }
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
        }

        if i < chars.len() {
            self.cursor_mut().x = i;
        } else if y < num_lines - 1 {
            self.cursor_mut().y += 1;
            self.cursor_mut().x = 0;
            let y_new = self.cursor().y;
            let next_line_text = &self.buffer().lines[y_new];
            let mut j = 0;
            while j < next_line_text.len() && next_line_text.chars().nth(j).unwrap().is_whitespace() {
                j += 1;
            }
            self.cursor_mut().x = j;
        } else {
            self.cursor_mut().x = line.len();
        }
    }

    pub fn move_word_backward(&mut self) {
        let y = self.cursor().y;
        let x = self.cursor().x;

        if x == 0 {
            if y > 0 {
                self.cursor_mut().y -= 1;
                let y_new = self.cursor().y;
                self.cursor_mut().x = self.buffer().lines[y_new].len();
                self.move_word_backward();
            }
            return;
        }

        let line = &self.buffer().lines[y];
        let chars: Vec<char> = line.chars().collect();
        let mut i = x.saturating_sub(1);

        while i > 0 && chars[i].is_whitespace() {
            i -= 1;
        }

        if chars[i].is_whitespace() {
            self.cursor_mut().x = i;
            return;
        }

        if Self::is_word_char(chars[i]) {
            while i > 0 && Self::is_word_char(chars[i-1]) {
                i -= 1;
            }
        } else {
            while i > 0 && !chars[i-1].is_whitespace() && !Self::is_word_char(chars[i-1]) {
                i -= 1;
            }
        }

        self.cursor_mut().x = i;
    }

    pub fn move_word_end(&mut self) {
        let y = self.cursor().y;
        let x = self.cursor().x;
        let num_lines = self.buffer().lines.len();

        if y >= num_lines { return; }
        let line = &self.buffer().lines[y];
        let chars: Vec<char> = line.chars().collect();
        
        if x >= line.len().saturating_sub(1) {
            if y < num_lines - 1 {
                self.cursor_mut().y += 1;
                self.cursor_mut().x = 0;
                self.move_word_end();
            }
            return;
        }

        let mut i = x + 1;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        if i >= chars.len() {
            if y < num_lines - 1 {
                self.cursor_mut().y += 1;
                self.cursor_mut().x = 0;
                self.move_word_end();
            }
            return;
        }

        if Self::is_word_char(chars[i]) {
            while i + 1 < chars.len() && Self::is_word_char(chars[i+1]) {
                i += 1;
            }
        } else {
            while i + 1 < chars.len() && !chars[i+1].is_whitespace() && !Self::is_word_char(chars[i+1]) {
                i += 1;
            }
        }
        self.cursor_mut().x = i;
    }

    pub fn open_line_below(&mut self) {
        self.buffer_mut().push_history();
        let y = self.cursor().y;
        self.buffer_mut().lines.insert(y + 1, String::new());
        self.cursor_mut().y = y + 1;
        self.cursor_mut().x = 0;
    }

    pub fn open_line_above(&mut self) {
        self.buffer_mut().push_history();
        let y = self.cursor().y;
        self.buffer_mut().lines.insert(y, String::new());
        self.cursor_mut().y = y;
        self.cursor_mut().x = 0;
    }

    pub fn yank(&self, start_x: usize, start_y: usize, end_x: usize, end_y: usize) -> String {
        let (s_y, s_x, e_y, e_x) = if (start_y, start_x) < (end_y, end_x) {
            (start_y, start_x, end_y, end_x)
        } else {
            (end_y, end_x, start_y, start_x)
        };

        let mut result = Vec::new();
        for y in s_y..=e_y {
            if let Some(line) = self.buffer().lines.get(y) {
                let start = if y == s_y { s_x } else { 0 };
                let end = if y == e_y { e_x + 1 } else { line.len() };
                
                if start < line.len() {
                    let end = end.min(line.len());
                    result.push(line[start..end].to_string());
                }
            }
        }
        result.join("\n")
    }

    pub fn paste_before(&mut self, text: &str, yank_type: crate::vim::mode::YankType) {
        if text.is_empty() { return; }
        self.buffer_mut().push_history();

        let cursor_y = self.cursor().y;
        let cursor_x = self.cursor().x;

        if yank_type == crate::vim::mode::YankType::Line {
            let lines_to_paste: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
            for (i, line) in lines_to_paste.into_iter().enumerate() {
                self.buffer_mut().lines.insert(cursor_y + i, line);
            }
            self.cursor_mut().y = cursor_y;
            self.cursor_mut().x = 0;
        } else {
            let lines_to_paste: Vec<&str> = text.split('\n').collect();
            if lines_to_paste.len() == 1 {
                let current_line = &mut self.buffer_mut().lines[cursor_y];
                current_line.insert_str(cursor_x, lines_to_paste[0]);
                self.cursor_mut().x += lines_to_paste[0].len();
            } else {
                let current_line = &mut self.buffer_mut().lines[cursor_y];
                let suffix = current_line.split_off(cursor_x);
                current_line.push_str(lines_to_paste[0]);
                
                for i in 1..lines_to_paste.len() - 1 {
                    self.buffer_mut().lines.insert(cursor_y + i, lines_to_paste[i].to_string());
                }
                
                let last_line_idx = cursor_y + lines_to_paste.len() - 1;
                let mut last_line = lines_to_paste.last().unwrap().to_string();
                let new_x = last_line.len();
                last_line.push_str(&suffix);
                self.buffer_mut().lines.insert(last_line_idx, last_line);
                
                self.cursor_mut().y = last_line_idx;
                self.cursor_mut().x = new_x;
            }
        }
    }

    pub fn paste_after(&mut self, text: &str, yank_type: crate::vim::mode::YankType) {
        if text.is_empty() { return; }
        
        if yank_type == crate::vim::mode::YankType::Line {
            self.buffer_mut().push_history();
            let cursor_y = self.cursor().y;
            let lines_to_paste: Vec<String> = text.split('\n').map(|s| s.to_string()).collect();
            for (i, line) in lines_to_paste.into_iter().enumerate() {
                self.buffer_mut().lines.insert(cursor_y + 1 + i, line);
            }
            self.cursor_mut().y = cursor_y + 1;
            self.cursor_mut().x = 0;
        } else {
            let cursor_x = self.cursor().x;
            let line_len = self.buffer().lines[self.cursor().y].len();
            if cursor_x < line_len {
                self.cursor_mut().x += 1;
            }
            self.paste_before(text, yank_type);
        }
    }

    pub fn delete_selection(&mut self, start_x: usize, start_y: usize, end_x: usize, end_y: usize) -> String {
        let (s_y, s_x, e_y, e_x) = if (start_y, start_x) < (end_y, end_x) {
            (start_y, start_x, end_y, end_x)
        } else {
            (end_y, end_x, start_y, start_x)
        };

        let yanked = self.yank(start_x, start_y, end_x, end_y);
        self.buffer_mut().push_history();

        if s_y == e_y {
            let line = &mut self.buffer_mut().lines[s_y];
            let suffix = line.split_off(e_x + 1);
            line.truncate(s_x);
            line.push_str(&suffix);
        } else {
            let first_line = self.buffer_mut().lines[s_y].clone();
            let last_line = self.buffer_mut().lines[e_y].clone();
            
            let prefix = &first_line[..s_x];
            let suffix = if e_x + 1 < last_line.len() { &last_line[e_x+1..] } else { "" };
            
            self.buffer_mut().lines[s_y] = format!("{}{}", prefix, suffix);
            
            for _ in s_y+1..=e_y {
                self.buffer_mut().lines.remove(s_y + 1);
            }
        }

        self.cursor_mut().x = s_x;
        self.cursor_mut().y = s_y;
        yanked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_new() {
        let editor = Editor::new();
        assert_eq!(editor.buffers.len(), 1);
        assert_eq!(editor.cursors.len(), 1);
    }

    #[test]
    fn test_editor_multi_buffer() {
        let mut editor = Editor::new();
        editor.buffers[0].lines = vec!["Buffer 1".to_string()];
        
        editor.buffers.push(buffer::Buffer::new());
        editor.cursors.push(cursor::Cursor::new());
        editor.buffers[1].lines = vec!["Buffer 2".to_string()];
        
        editor.next_buffer();
        assert_eq!(editor.active_idx, 1);
        assert_eq!(editor.buffer().lines[0], "Buffer 2");
        
        editor.prev_buffer();
        assert_eq!(editor.active_idx, 0);
        assert_eq!(editor.buffer().lines[0], "Buffer 1");
    }

    #[test]
    fn test_editor_movement() {
        let mut editor = Editor::new();
        editor.buffer_mut().lines = vec!["abc".to_string(), "de".to_string()];
        editor.move_right();
        assert_eq!(editor.cursor().x, 1);
        editor.move_down();
        assert_eq!(editor.cursor().y, 1);
        assert_eq!(editor.cursor().x, 1);
    }

    #[test]
    fn test_editor_word_movement() {
        let mut editor = Editor::new();
        editor.buffer_mut().lines = vec!["hello, world rust".to_string()];
        
        editor.move_word_forward();
        assert_eq!(editor.cursor().x, 5); // start of ','
        
        editor.move_word_forward();
        assert_eq!(editor.cursor().x, 7); // start of 'world'
        
        editor.move_word_end();
        assert_eq!(editor.cursor().x, 11); // end of 'world'
        
        editor.move_word_backward();
        assert_eq!(editor.cursor().x, 7); // start of 'world'
    }

    #[test]
    fn test_editor_delete_selection() {
        let mut editor = Editor::new();
        editor.buffer_mut().lines = vec!["hello world".to_string()];
        editor.delete_selection(0, 0, 5, 0); // delete "hello "
        assert_eq!(editor.buffer().lines[0], "world");
    }

    #[test]
    fn test_editor_open_line() {
        let mut editor = Editor::new();
        editor.buffer_mut().lines = vec!["line 1".to_string()];
        
        editor.open_line_below();
        assert_eq!(editor.buffer().lines.len(), 2);
        assert_eq!(editor.cursor().y, 1);
        
        editor.open_line_above();
        assert_eq!(editor.buffer().lines.len(), 3);
        assert_eq!(editor.cursor().y, 1);
        assert_eq!(editor.buffer().lines[1], "");
    }

    #[test]
    fn test_editor_paste() {
        let mut editor = Editor::new();
        editor.buffer_mut().lines = vec!["ab".to_string()];
        editor.cursor_mut().x = 1; // On 'b'
        
        editor.paste_after("X", crate::vim::mode::YankType::Char);
        assert_eq!(editor.buffer().lines[0], "abX");
        
        editor.cursor_mut().x = 1; // On 'b'
        editor.paste_before("Y", crate::vim::mode::YankType::Char);
        assert_eq!(editor.buffer().lines[0], "aYbX");
    }
}
