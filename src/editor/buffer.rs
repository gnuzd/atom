use std::{fs, io, path::PathBuf, time::SystemTime};
use crate::git::GitSign;
use ropey::Rope;

#[derive(Clone)]
pub struct Buffer {
    pub text: Rope,
    pub file_path: Option<PathBuf>,
    pub history: Vec<Rope>,
    pub redo_stack: Vec<Rope>,
    pub modified: bool,
    pub folded_ranges: Vec<(usize, usize)>,
    pub git_signs: Vec<(usize, GitSign)>,
    pub last_modified: Option<SystemTime>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            file_path: None,
            history: Vec::new(),
            redo_stack: Vec::new(),
            modified: false,
            folded_ranges: Vec::new(),
            git_signs: Vec::new(),
            last_modified: None,
        }
    }

    pub fn load(path: PathBuf) -> io::Result<Self> {
        let content = fs::read_to_string(&path)?;
        let last_modified = fs::metadata(&path)?.modified().ok();
        let text = Rope::from_str(&content);
        Ok(Self {
            text,
            file_path: Some(path),
            history: Vec::new(),
            redo_stack: Vec::new(),
            modified: false,
            folded_ranges: Vec::new(),
            git_signs: Vec::new(),
            last_modified,
        })
    }

    pub fn reload(&mut self) -> io::Result<()> {
        if let Some(path) = &self.file_path {
            let content = fs::read_to_string(path)?;
            self.last_modified = fs::metadata(path)?.modified().ok();
            self.text = Rope::from_str(&content);
            self.modified = false;
            self.history.clear();
            self.redo_stack.clear();
        }
        Ok(())
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(path) = &self.file_path {
            let file = fs::File::create(path)?;
            self.text.write_to(io::BufWriter::new(file))?;
            self.modified = false;
            self.last_modified = fs::metadata(path)?.modified().ok();
        }
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        let file = fs::File::create(&path)?;
        self.text.write_to(io::BufWriter::new(file))?;
        self.file_path = Some(path.clone());
        self.modified = false;
        self.last_modified = fs::metadata(&path)?.modified().ok();
        Ok(())
    }

    pub fn push_history(&mut self) {
        self.history.push(self.text.clone());
        self.redo_stack.clear();
        self.modified = true;
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev_state) = self.history.pop() {
            self.redo_stack.push(self.text.clone());
            self.text = prev_state;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next_state) = self.redo_stack.pop() {
            self.history.push(self.text.clone());
            self.text = next_state;
            true
        } else {
            false
        }
    }

    pub fn len_lines(&self) -> usize {
        self.text.len_lines()
    }

    pub fn line(&self, line_idx: usize) -> Option<ropey::RopeSlice<'_>> {
        if line_idx < self.text.len_lines() {
            Some(self.text.line(line_idx))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_new() {
        let buffer = Buffer::new();
        assert_eq!(buffer.len_lines(), 1);
        assert!(buffer.file_path.is_none());
    }

    #[test]
    fn test_buffer_undo_redo() {
        let mut buffer = Buffer::new();
        buffer.text = Rope::from_str("State 1");
        buffer.push_history();
        
        buffer.text = Rope::from_str("State 2");
        assert_eq!(buffer.text.to_string(), "State 2");
        
        buffer.undo();
        assert_eq!(buffer.text.to_string(), "State 1");
        
        buffer.redo();
        assert_eq!(buffer.text.to_string(), "State 2");
    }
}
