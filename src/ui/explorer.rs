use std::fs;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;

#[derive(Clone)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub is_last: bool,
    pub is_ignored: bool,
}

pub struct FileExplorer {
    pub root: PathBuf,
    pub entries: Vec<TreeEntry>,
    pub selected_idx: usize,
    pub visible: bool,
    pub filter: String,
    pub show_hidden: bool,
    pub show_ignored: bool,
}

impl FileExplorer {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut explorer = Self {
            root: root.clone(),
            entries: Vec::new(),
            selected_idx: 0,
            visible: false,
            filter: String::new(),
            show_hidden: false,
            show_ignored: false,
        };
        explorer.init_root();
        explorer
    }

    fn init_root(&mut self) {
        self.entries.clear();
        self.load_dir(&self.root.clone(), 0, 0);
    }

    fn load_dir(&mut self, path: &Path, depth: usize, insert_pos: usize) -> usize {
        let mut added = 0;
        let walker = WalkBuilder::new(path)
            .max_depth(Some(1))
            .hidden(!self.show_hidden)
            .git_ignore(!self.show_ignored)
            .build();

        let mut paths: Vec<(PathBuf, bool, bool)> = walker
            .filter_map(|e| e.ok())
            .filter(|e| e.path() != path) // skip the directory itself
            .map(|e| (e.path().to_path_buf(), e.file_type().unwrap().is_dir(), false)) // TODO: track ignored
            .collect();

        // Manual sorting: dirs first, then name
        paths.sort_by(|a, b| {
            if a.1 != b.1 { b.1.cmp(&a.1) } else { a.0.cmp(&b.0) }
        });

        let len = paths.len();
        for (i, (p, is_dir, is_ignored)) in paths.into_iter().enumerate() {
            // Apply filter if depth 0 (or optionally nested)
            if !self.filter.is_empty() {
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if !name.to_lowercase().contains(&self.filter.to_lowercase()) {
                    continue;
                }
            }

            self.entries.insert(insert_pos + added, TreeEntry {
                path: p,
                depth,
                is_dir,
                is_expanded: false,
                is_last: i == len - 1,
                is_ignored,
            });
            added += 1;
        }
        added
    }

    pub fn expand(&mut self) {
        if self.entries.is_empty() { return; }
        let entry = &self.entries[self.selected_idx];
        if !entry.is_dir { return; }
        
        if !entry.is_expanded {
            self.toggle_expand();
        } else if self.selected_idx + 1 < self.entries.len() {
            self.selected_idx += 1;
        }
    }

    pub fn collapse(&mut self) {
        if self.entries.is_empty() { return; }
        let entry = self.entries[self.selected_idx].clone();
        
        if entry.is_dir && entry.is_expanded {
            self.toggle_expand();
        } else if entry.depth > 0 {
            let target_depth = entry.depth - 1;
            let mut i = self.selected_idx;
            while i > 0 {
                i -= 1;
                if self.entries[i].depth == target_depth {
                    self.selected_idx = i;
                    break;
                }
            }
        }
    }

    pub fn toggle_expand(&mut self) {
        if self.entries.is_empty() { return; }
        let entry = self.entries[self.selected_idx].clone();
        if !entry.is_dir { return; }

        if entry.is_expanded {
            let depth = entry.depth;
            let i = self.selected_idx + 1;
            while i < self.entries.len() && self.entries[i].depth > depth {
                self.entries.remove(i);
            }
            self.entries[self.selected_idx].is_expanded = false;
        } else {
            self.load_dir(&entry.path, entry.depth + 1, self.selected_idx + 1);
            self.entries[self.selected_idx].is_expanded = true;
        }
    }

    pub fn close_all(&mut self) {
        self.init_root();
        self.selected_idx = 0;
    }

    pub fn refresh(&mut self) {
        // Keep selected path if possible
        let selected_path = self.selected_entry().map(|e| e.path.clone());
        
        // This is a naive refresh. Real tree refresh would need to remember expanded nodes.
        // For now, let's just re-init.
        self.init_root();
        
        if let Some(path) = selected_path {
            if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
                self.selected_idx = pos;
            }
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible && self.entries.is_empty() {
            self.init_root();
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.entries.is_empty() && self.selected_idx < self.entries.len() - 1 {
            self.selected_idx += 1;
        }
    }

    pub fn selected_entry(&self) -> Option<&TreeEntry> {
        self.entries.get(self.selected_idx)
    }

    // File Operations
    pub fn create_file(&mut self, name: &str) -> std::io::Result<()> {
        let parent = self.selected_entry()
            .map(|e| if e.is_dir { e.path.clone() } else { e.path.parent().unwrap().to_path_buf() })
            .unwrap_or_else(|| self.root.clone());
        
        let path = parent.join(name);
        if name.ends_with('/') {
            fs::create_dir_all(path)?;
        } else {
            fs::File::create(path)?;
        }
        self.refresh();
        Ok(())
    }

    pub fn rename_selected(&mut self, new_name: &str) -> std::io::Result<()> {
        if let Some(entry) = self.selected_entry() {
            let parent = entry.path.parent().unwrap();
            let new_path = parent.join(new_name);
            fs::rename(&entry.path, new_path)?;
            self.refresh();
        }
        Ok(())
    }

    pub fn delete_selected(&mut self) -> std::io::Result<()> {
        if let Some(entry) = self.selected_entry() {
            if entry.is_dir {
                fs::remove_dir_all(&entry.path)?;
            } else {
                fs::remove_file(&entry.path)?;
            }
            self.refresh();
        }
        Ok(())
    }

    pub fn move_selected(&mut self, target_dir: &Path) -> std::io::Result<()> {
        if let Some(entry) = self.selected_entry() {
            let file_name = entry.path.file_name().unwrap();
            let new_path = target_dir.join(file_name);
            fs::rename(&entry.path, new_path)?;
            self.refresh();
        }
        Ok(())
    }
}
