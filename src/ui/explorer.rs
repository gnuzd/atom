use std::fs;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use std::collections::HashSet;

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

    pub fn init_root(&mut self) {
        self.entries.clear();
        if self.filter.is_empty() {
            self.load_dir(&self.root.clone(), 0, 0);
        } else {
            self.load_filtered();
        }
    }

    fn load_filtered(&mut self) {
        let mut visible_paths = HashSet::new();
        let walker = WalkBuilder::new(&self.root)
            .hidden(!self.show_hidden)
            .git_ignore(!self.show_ignored)
            .build();

        let filter_lower = self.filter.to_lowercase();

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.to_lowercase().contains(&filter_lower) {
                // Add this path and all its parents up to root
                let mut curr = path;
                while let Some(parent) = curr.parent() {
                    visible_paths.insert(curr.to_path_buf());
                    if curr == self.root { break; }
                    curr = parent;
                }
            }
        }

        self.load_dir_recursive(&self.root.clone(), 0, &visible_paths);
    }

    fn load_dir_recursive(&mut self, path: &Path, depth: usize, visible_paths: &HashSet<PathBuf>) {
        if let Ok(entries) = fs::read_dir(path) {
            let mut paths: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| visible_paths.contains(p))
                .collect();
            
            self.sort_paths(&mut paths);
            let len = paths.len();
            for (i, p) in paths.into_iter().enumerate() {
                let is_dir = p.is_dir();
                self.entries.push(TreeEntry {
                    path: p.clone(),
                    depth,
                    is_dir,
                    is_expanded: true, // Auto-expand when filtering
                    is_last: i == len - 1,
                    is_ignored: false, // Could check with ignore crate if needed
                });
                if is_dir {
                    self.load_dir_recursive(&p, depth + 1, visible_paths);
                }
            }
        }
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
            .filter(|e| e.path() != path)
            .map(|e| (e.path().to_path_buf(), e.file_type().unwrap().is_dir(), e.depth() > 0)) // simplistic ignore check
            .collect();

        paths.sort_by(|a, b| {
            if a.1 != b.1 { b.1.cmp(&a.1) } else { a.0.cmp(&b.0) }
        });

        let len = paths.len();
        for (i, (p, is_dir, _)) in paths.into_iter().enumerate() {
            self.entries.insert(insert_pos + added, TreeEntry {
                path: p,
                depth,
                is_dir,
                is_expanded: false,
                is_last: i == len - 1,
                is_ignored: false, // Simplified
            });
            added += 1;
        }
        added
    }

    fn sort_paths(&self, paths: &mut Vec<PathBuf>) {
        paths.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            if a_is_dir != b_is_dir { b_is_dir.cmp(&a_is_dir) } else { a.cmp(b) }
        });
    }

    pub fn expand(&mut self) {
        if self.entries.is_empty() { return; }
        let entry = self.entries[self.selected_idx].clone();
        if !entry.is_dir { return; }
        if !entry.is_expanded { self.toggle_expand(); }
        else if self.selected_idx + 1 < self.entries.len() { self.selected_idx += 1; }
    }

    pub fn collapse(&mut self) {
        if self.entries.is_empty() { return; }
        let entry = self.entries[self.selected_idx].clone();
        if entry.is_dir && entry.is_expanded { self.toggle_expand(); }
        else if entry.depth > 0 {
            let target_depth = entry.depth - 1;
            let mut i = self.selected_idx;
            while i > 0 {
                i -= 1;
                if self.entries[i].depth == target_depth { self.selected_idx = i; break; }
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
            while i < self.entries.len() && self.entries[i].depth > depth { self.entries.remove(i); }
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
        let selected_path = self.selected_entry().map(|e| e.path.clone());
        self.init_root();
        if let Some(path) = selected_path {
            if let Some(pos) = self.entries.iter().position(|e| e.path == path) { self.selected_idx = pos; }
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible && self.entries.is_empty() { self.init_root(); }
    }

    pub fn move_up(&mut self) { if self.selected_idx > 0 { self.selected_idx -= 1; } }
    pub fn move_down(&mut self) { if !self.entries.is_empty() && self.selected_idx < self.entries.len() - 1 { self.selected_idx += 1; } }
    pub fn selected_entry(&self) -> Option<&TreeEntry> { self.entries.get(self.selected_idx) }

    pub fn create_file(&mut self, name: &str) -> std::io::Result<()> {
        let parent = self.selected_entry().map(|e| if e.is_dir { e.path.clone() } else { e.path.parent().unwrap().to_path_buf() }).unwrap_or_else(|| self.root.clone());
        let path = parent.join(name);
        if name.ends_with('/') { fs::create_dir_all(path)?; } else { fs::File::create(path)?; }
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
            if entry.is_dir { fs::remove_dir_all(&entry.path)?; } else { fs::remove_file(&entry.path)?; }
            self.refresh();
        }
        Ok(())
    }

    pub fn move_selected(&mut self, target_path: &Path) -> std::io::Result<()> {
        if let Some(entry) = self.selected_entry() {
            let file_name = entry.path.file_name().unwrap();
            let new_path = if target_path.is_dir() { target_path.join(file_name) } else { target_path.to_path_buf() };
            fs::rename(&entry.path, new_path)?;
            self.refresh();
        }
        Ok(())
    }
}
