use std::{fs, io, path::PathBuf};

pub struct Buffer {
    pub lines: Vec<String>,
    pub file_path: Option<PathBuf>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            file_path: None,
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
        })
    }

    pub fn save(&self) -> io::Result<()> {
        if let Some(path) = &self.file_path {
            let content = self.lines.join("\n");
            fs::write(path, content)?;
        }
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> io::Result<()> {
        let content = self.lines.join("\n");
        fs::write(&path, content)?;
        self.file_path = Some(path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_buffer_new() {
        let buffer = Buffer::new();
        assert_eq!(buffer.lines.len(), 1);
        assert_eq!(buffer.lines[0], "");
        assert!(buffer.file_path.is_none());
    }

    #[test]
    fn test_buffer_save_load() {
        let mut temp_path = env::temp_dir();
        temp_path.push("atom_test_buffer.txt");
        
        let mut buffer = Buffer::new();
        buffer.lines = vec!["Line 1".to_string(), "Line 2".to_string()];
        
        // Save As
        buffer.save_as(temp_path.clone()).expect("Failed to save as");
        assert_eq!(buffer.file_path, Some(temp_path.clone()));
        assert!(temp_path.exists());
        
        // Load
        let loaded_buffer = Buffer::load(temp_path.clone()).expect("Failed to load");
        assert_eq!(loaded_buffer.lines.len(), 2);
        assert_eq!(loaded_buffer.lines[0], "Line 1");
        assert_eq!(loaded_buffer.lines[1], "Line 2");
        assert_eq!(loaded_buffer.file_path, Some(temp_path.clone()));
        
        // Clean up
        let _ = fs::remove_file(temp_path);
    }
}
