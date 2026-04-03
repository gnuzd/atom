use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub is_last: bool, // Used for drawing tree guides
}

pub struct FileExplorer {
    pub root: PathBuf,
    pub entries: Vec<TreeEntry>,
    pub selected_idx: usize,
    pub visible: bool,
    pub filter: String,
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
        };
        explorer.init_root();
        explorer
    }

    fn init_root(&mut self) {
        self.entries.clear();
        if let Ok(entries) = fs::read_dir(&self.root) {
            let mut root_entries: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
            self.sort_paths(&mut root_entries);
            
            let len = root_entries.len();
            for (i, path) in root_entries.into_iter().enumerate() {
                self.entries.push(TreeEntry {
                    is_dir: path.is_dir(),
                    path,
                    depth: 0,
                    is_expanded: false,
                    is_last: i == len - 1,
                });
            }
        }
    }

    fn sort_paths(&self, paths: &mut Vec<PathBuf>) {
        paths.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            if a_is_dir != b_is_dir {
                b_is_dir.cmp(&a_is_dir)
            } else {
                a.cmp(b)
            }
        });
    }

    pub fn toggle_expand(&mut self) {
        if self.entries.is_empty() { return; }
        let entry = &self.entries[self.selected_idx];
        if !entry.is_dir { return; }

        let is_expanded = entry.is_expanded;
        let depth = entry.depth;
        let path = entry.path.clone();

        if is_expanded {
            let mut i = self.selected_idx + 1;
            while i < self.entries.len() && self.entries[i].depth > depth {
                self.entries.remove(i);
            }
            self.entries[self.selected_idx].is_expanded = false;
        } else {
            if let Ok(entries) = fs::read_dir(&path) {
                let mut new_paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
                self.sort_paths(&mut new_paths);
                
                let len = new_paths.len();
                for (offset, new_path) in new_paths.into_iter().enumerate() {
                    self.entries.insert(self.selected_idx + 1 + offset, TreeEntry {
                        is_dir: new_path.is_dir(),
                        path: new_path,
                        depth: depth + 1,
                        is_expanded: false,
                        is_last: offset == len - 1,
                    });
                }
                self.entries[self.selected_idx].is_expanded = true;
            }
        }
    }

    pub fn refresh(&mut self) {
        self.init_root();
        self.selected_idx = 0;
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
}
