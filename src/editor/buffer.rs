use std::{fs, io, path::PathBuf};

#[derive(Clone)]
pub struct Buffer {
    pub lines: Vec<String>,
    pub file_path: Option<PathBuf>,
    pub history: Vec<Vec<String>>,
    pub redo_stack: Vec<Vec<String>>,
    pub modified: bool,
    pub folded_ranges: Vec<(usize, usize)>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            file_path: None,
            history: Vec::new(),
            redo_stack: Vec::new(),
            modified: false,
            folded_ranges: Vec::new(),
        }
    }

    pub fn load(path: PathBuf) -> io::Result<Self> {
        let content = fs::read_to_string(&path)?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        Ok(Self {
            lines,
            file_path: Some(path),
            history: Vec::new(),
            redo_stack: Vec::new(),
            modified: false,
            folded_ranges: Vec::new(),
        })
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(path) = &self.file_path {
            let content = self.lines.join("\n");
            fs::write(path, content)?;
            self.modified = false;
        }
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        let content = self.lines.join("\n");
        fs::write(&path, content)?;
        self.file_path = Some(path);
        self.modified = false;
        Ok(())
    }

    pub fn push_history(&mut self) {
        self.history.push(self.lines.clone());
        self.redo_stack.clear();
        self.modified = true;
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev_state) = self.history.pop() {
            self.redo_stack.push(self.lines.clone());
            self.lines = prev_state;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next_state) = self.redo_stack.pop() {
            self.history.push(self.lines.clone());
            self.lines = next_state;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_new() {
        let buffer = Buffer::new();
        assert_eq!(buffer.lines.len(), 1);
        assert_eq!(buffer.lines[0], "");
        assert!(buffer.file_path.is_none());
    }

    #[test]
    fn test_buffer_undo_redo() {
        let mut buffer = Buffer::new();
        buffer.lines = vec!["State 1".to_string()];
        buffer.push_history();
        
        buffer.lines = vec!["State 2".to_string()];
        assert_eq!(buffer.lines[0], "State 2");
        
        buffer.undo();
        assert_eq!(buffer.lines[0], "State 1");
        
        buffer.redo();
        assert_eq!(buffer.lines[0], "State 2");
    }
}
