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
}
