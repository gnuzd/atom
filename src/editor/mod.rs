use std::{io, path::PathBuf};

pub mod buffer;
pub mod cursor;

pub struct Editor {
    pub buffer: buffer::Buffer,
    pub cursor: cursor::Cursor,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: buffer::Buffer::new(),
            cursor: cursor::Cursor::new(),
        }
    }

    pub fn open_file(&mut self, path: PathBuf) -> io::Result<()> {
        self.buffer = buffer::Buffer::load(path)?;
        self.cursor.x = 0;
        self.cursor.y = 0;
        Ok(())
    }

    pub fn save_file(&self) -> io::Result<()> {
        self.buffer.save()
    }

    pub fn save_file_as(&mut self, path: PathBuf) -> io::Result<()> {
        self.buffer.save_as(path)
    }

    pub fn undo(&mut self) -> bool {
        self.buffer.undo()
    }

    pub fn redo(&mut self) -> bool {
        self.buffer.redo()
    }

    pub fn move_up(&mut self) {
        if self.cursor.y > 0 {
            self.cursor.y -= 1;
            let line_len = self.buffer.lines[self.cursor.y].len();
            if self.cursor.x > line_len {
                self.cursor.x = line_len;
            }
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor.y < self.buffer.lines.len() - 1 {
            self.cursor.y += 1;
            let line_len = self.buffer.lines[self.cursor.y].len();
            if self.cursor.x > line_len {
                self.cursor.x = line_len;
            }
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor.x > 0 {
            self.cursor.x -= 1;
        }
    }

    pub fn move_right(&mut self) {
        if let Some(line) = self.buffer.lines.get(self.cursor.y) {
            if self.cursor.x < line.len() {
                self.cursor.x += 1;
            }
        }
    }

    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    pub fn move_word_forward(&mut self) {
        let lines = &self.buffer.lines;
        let y = self.cursor.y;
        let x = self.cursor.x;

        if y >= lines.len() { return; }
        let line = &lines[y];
        
        if x >= line.len() {
            if y < lines.len() - 1 {
                self.cursor.y += 1;
                self.cursor.x = 0;
                let next_line = &lines[self.cursor.y];
                let mut j = 0;
                while j < next_line.len() && next_line.chars().nth(j).unwrap().is_whitespace() {
                    j += 1;
                }
                self.cursor.x = j;
            }
            return;
        }

        let chars: Vec<char> = line.chars().collect();
        let mut i = x;

        // Vim-like 'w' logic: 
        // 1. If on word char, skip word chars, then skip whitespace.
        // 2. If on punctuation, skip punctuation, then skip whitespace.
        // 3. If on whitespace, skip whitespace.
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
            self.cursor.x = i;
        } else if y < lines.len() - 1 {
            self.cursor.y += 1;
            self.cursor.x = 0;
            let next_line = &lines[self.cursor.y];
            let mut j = 0;
            while j < next_line.len() && next_line.chars().nth(j).unwrap().is_whitespace() {
                j += 1;
            }
            self.cursor.x = j;
        } else {
            self.cursor.x = line.len();
        }
    }

    pub fn move_word_backward(&mut self) {
        let lines = &self.buffer.lines;
        let y = self.cursor.y;
        let x = self.cursor.x;

        if x == 0 {
            if y > 0 {
                self.cursor.y -= 1;
                self.cursor.x = lines[self.cursor.y].len();
                self.move_word_backward();
            }
            return;
        }

        let line = &lines[y];
        let chars: Vec<char> = line.chars().collect();
        let mut i = x.saturating_sub(1);

        while i > 0 && chars[i].is_whitespace() {
            i -= 1;
        }

        if chars[i].is_whitespace() {
            self.cursor.x = i;
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

        self.cursor.x = i;
    }

    pub fn move_word_end(&mut self) {
        let lines = &self.buffer.lines;
        let y = self.cursor.y;
        let x = self.cursor.x;

        if y >= lines.len() { return; }
        let line = &lines[y];
        let chars: Vec<char> = line.chars().collect();
        
        if x >= line.len().saturating_sub(1) {
            if y < lines.len() - 1 {
                self.cursor.y += 1;
                self.cursor.x = 0;
                self.move_word_end();
            }
            return;
        }

        let mut i = x + 1;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        if i >= chars.len() {
            if y < lines.len() - 1 {
                self.cursor.y += 1;
                self.cursor.x = 0;
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
        self.cursor.x = i;
    }

    pub fn yank(&self, start_x: usize, start_y: usize, end_x: usize, end_y: usize) -> String {
        let (s_y, s_x, e_y, e_x) = if (start_y, start_x) < (end_y, end_x) {
            (start_y, start_x, end_y, end_x)
        } else {
            (end_y, end_x, start_y, start_x)
        };

        let mut result = Vec::new();
        for y in s_y..=e_y {
            if let Some(line) = self.buffer.lines.get(y) {
                let start = if y == s_y { s_x } else { 0 };
                let end = if y == e_y { e_x + 1 } else { line.len() };
                
                if start < line.len() {
                    let end = end.min(line.len());
                    result.push(line[start..end].to_string());
                } else if y == s_y && y == e_y && start == e_x && line.is_empty() {
                    // Empty line
                }
            }
        }
        result.join("\n")
    }

    pub fn paste(&mut self, text: &str) {
        if text.is_empty() { return; }
        self.buffer.push_history();

        let lines_to_paste: Vec<&str> = text.split('\n').collect();
        let current_line = &mut self.buffer.lines[self.cursor.y];
        
        if lines_to_paste.len() == 1 {
            current_line.insert_str(self.cursor.x, lines_to_paste[0]);
            self.cursor.x += lines_to_paste[0].len();
        } else {
            let suffix = current_line.split_off(self.cursor.x);
            current_line.push_str(lines_to_paste[0]);
            
            for i in 1..lines_to_paste.len() - 1 {
                self.buffer.lines.insert(self.cursor.y + i, lines_to_paste[i].to_string());
            }
            
            let last_line_idx = self.cursor.y + lines_to_paste.len() - 1;
            let mut last_line = lines_to_paste.last().unwrap().to_string();
            let new_x = last_line.len();
            last_line.push_str(&suffix);
            self.buffer.lines.insert(last_line_idx, last_line);
            
            self.cursor.y = last_line_idx;
            self.cursor.x = new_x;
        }
    }

    pub fn delete_selection(&mut self, start_x: usize, start_y: usize, end_x: usize, end_y: usize) -> String {
        let (s_y, s_x, e_y, e_x) = if (start_y, start_x) < (end_y, end_x) {
            (start_y, start_x, end_y, end_x)
        } else {
            (end_y, end_x, start_y, start_x)
        };

        self.buffer.push_history();
        let yanked = self.yank(start_x, start_y, end_x, end_y);

        if s_y == e_y {
            let line = &mut self.buffer.lines[s_y];
            let suffix = line.split_off(e_x + 1);
            line.truncate(s_x);
            line.push_str(&suffix);
        } else {
            let first_line = self.buffer.lines[s_y].clone();
            let last_line = self.buffer.lines[e_y].clone();
            
            let prefix = &first_line[..s_x];
            let suffix = if e_x + 1 < last_line.len() { &last_line[e_x+1..] } else { "" };
            
            self.buffer.lines[s_y] = format!("{}{}", prefix, suffix);
            
            for _ in s_y+1..=e_y {
                self.buffer.lines.remove(s_y + 1);
            }
        }

        self.cursor.x = s_x;
        self.cursor.y = s_y;
        yanked
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_new() {
        let editor = Editor::new();
        assert_eq!(editor.buffer.lines.len(), 1);
        assert_eq!(editor.cursor.x, 0);
        assert_eq!(editor.cursor.y, 0);
    }

    #[test]
    fn test_editor_movement() {
        let mut editor = Editor::new();
        editor.buffer.lines = vec!["abc".to_string(), "de".to_string()];
        editor.move_right();
        assert_eq!(editor.cursor.x, 1);
        editor.move_down();
        assert_eq!(editor.cursor.y, 1);
        assert_eq!(editor.cursor.x, 1);
    }

    #[test]
    fn test_editor_word_movement() {
        let mut editor = Editor::new();
        editor.buffer.lines = vec!["hello, world rust".to_string()];
        
        editor.move_word_forward();
        assert_eq!(editor.cursor.x, 5); // start of ','
        
        editor.move_word_forward();
        assert_eq!(editor.cursor.x, 7); // start of 'world'
        
        editor.move_word_end();
        assert_eq!(editor.cursor.x, 11); // end of 'world'
        
        editor.move_word_backward();
        assert_eq!(editor.cursor.x, 7); // start of 'world'
    }

    #[test]
    fn test_editor_delete_selection() {
        let mut editor = Editor::new();
        editor.buffer.lines = vec!["hello world".to_string()];
        editor.delete_selection(0, 0, 5, 0); // delete "hello "
        assert_eq!(editor.buffer.lines[0], "world");
    }
}
