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

    pub fn move_word_forward(&mut self) {
        let lines = &self.buffer.lines;
        let y = self.cursor.y;
        let x = self.cursor.x;

        if y >= lines.len() { return; }

        let line = &lines[y];
        // If at end of line, go to start of next line
        if x >= line.len() {
            if y < lines.len() - 1 {
                self.cursor.y += 1;
                self.cursor.x = 0;
                self.move_word_forward();
            }
            return;
        }

        // Find next word start
        let chars: Vec<char> = line.chars().collect();
        
        // Skip current non-space characters
        let mut i = x;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        
        // Skip spaces
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        if i < chars.len() {
            self.cursor.x = i;
        } else if y < lines.len() - 1 {
            self.cursor.y += 1;
            self.cursor.x = 0;
            // Optionally continue to find word in next line
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

        // Skip preceding spaces
        while i > 0 && chars[i].is_whitespace() {
            i -= 1;
        }

        // Find start of word
        while i > 0 && !chars[i-1].is_whitespace() {
            i -= 1;
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
                    // Special case for empty line selection
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
        
        editor.move_right();
        assert_eq!(editor.cursor.x, 2);
        
        editor.move_right(); // At EOL
        assert_eq!(editor.cursor.x, 2);
        
        editor.move_left();
        assert_eq!(editor.cursor.x, 1);
        
        editor.move_up();
        assert_eq!(editor.cursor.y, 0);
    }

    #[test]
    fn test_editor_word_movement() {
        let mut editor = Editor::new();
        editor.buffer.lines = vec!["hello world rust".to_string()];
        
        editor.move_word_forward();
        assert_eq!(editor.cursor.x, 6); // start of 'world'
        
        editor.move_word_forward();
        assert_eq!(editor.cursor.x, 12); // start of 'rust'
        
        editor.move_word_backward();
        assert_eq!(editor.cursor.x, 6); // start of 'world'
    }

    #[test]
    fn test_editor_yank_paste() {
        let mut editor = Editor::new();
        editor.buffer.lines = vec!["hello".to_string()];
        
        let yanked = editor.yank(0, 0, 4, 0);
        assert_eq!(yanked, "hello");
        
        editor.cursor.x = 5;
        editor.paste(" world");
        assert_eq!(editor.buffer.lines[0], "hello world");
    }
}
