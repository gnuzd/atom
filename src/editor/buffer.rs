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

    #[test]
    fn test_buffer_new() {
        let buffer = Buffer::new();
        assert_eq!(buffer.lines.len(), 1);
        assert_eq!(buffer.lines[0], "");
        assert!(buffer.file_path.is_none());
    }
}
