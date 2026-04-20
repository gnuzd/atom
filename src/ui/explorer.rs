use std::fs;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use std::collections::HashSet;
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem};
use ratatui::text::{Line, Span};

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
    pub width: u16,
    pub scroll_y: usize,
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
            width: 30,
            scroll_y: 0,
        };
        explorer.init_root();
        explorer
    }

    pub fn scroll_into_view(&mut self, height: usize) {
        if height == 0 { return; }
        
        if self.entries.is_empty() {
            self.selected_idx = 0;
            self.scroll_y = 0;
            return;
        }

        // Ensure selected_idx is within bounds
        self.selected_idx = self.selected_idx.min(self.entries.len() - 1);

        if self.selected_idx < self.scroll_y {
            self.scroll_y = self.selected_idx;
        } else if self.selected_idx >= self.scroll_y + height {
            self.scroll_y = self.selected_idx - height + 1;
        }

        // Clamp scroll_y in case entries list shrunk
        let max_scroll = self.entries.len().saturating_sub(1);
        if self.scroll_y > max_scroll {
            self.scroll_y = max_scroll;
        }
    }

    pub fn move_page_up(&mut self, height: usize) {
        self.selected_idx = self.selected_idx.saturating_sub(height);
    }

    pub fn move_page_down(&mut self, height: usize) {
        if !self.entries.is_empty() {
            self.selected_idx = (self.selected_idx + height).min(self.entries.len() - 1);
        }
    }

    pub fn increase_width(&mut self) { self.width = self.width.saturating_add(2).min(80); }
    pub fn decrease_width(&mut self) { self.width = self.width.saturating_sub(2).max(10); }

    pub fn init_root(&mut self) {
        self.entries.clear();
        self.selected_idx = 0;
        if self.filter.is_empty() {
            // Add root entry
            self.entries.push(TreeEntry {
                path: self.root.clone(),
                depth: 0,
                is_dir: true,
                is_expanded: true,
                is_last: true,
                is_ignored: false,
            });
            self.load_dir(&self.root.clone(), 1, 1);
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

        // Add root entry first
        self.entries.push(TreeEntry {
            path: self.root.clone(),
            depth: 0,
            is_dir: true,
            is_expanded: true,
            is_last: true,
            is_ignored: false,
        });

        self.load_dir_recursive(&self.root.clone(), 1, &visible_paths);
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

    pub fn reveal_path(&mut self, target: &Path) {
        if !target.starts_with(&self.root) { return; }
        
        let relative = target.strip_prefix(&self.root).unwrap_or(Path::new(""));
        let mut current_path = self.root.clone();
        
        // Root is always at index 0 and expanded by default in init_root
        for component in relative.components() {
            let name = component.as_os_str();
            current_path.push(name);
            
            // Find current_path in entries
            if let Some(pos) = self.entries.iter().position(|e| e.path == current_path) {
                self.selected_idx = pos;
                if self.entries[pos].is_dir && !self.entries[pos].is_expanded {
                    self.toggle_expand();
                }
            } else {
                // If not found, it might be inside a collapsed dir we just expanded? 
                // But we iterate components, so we should find it if we expand as we go.
                break;
            }
        }
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
        let mut expanded_paths: Vec<PathBuf> = self.entries.iter()
            .filter(|e| e.is_dir && e.is_expanded)
            .map(|e| e.path.clone())
            .collect();

        // Sort by length (depth) to expand parents before children
        expanded_paths.sort_by_key(|p| p.as_os_str().len());

        self.init_root();
        
        for path in expanded_paths {
            if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
                // Temporarily set selected_idx to toggle_expand target
                let old_idx = self.selected_idx;
                self.selected_idx = pos;
                if !self.entries[pos].is_expanded {
                    self.toggle_expand();
                }
                self.selected_idx = old_idx;
            }
        }

        if let Some(path) = selected_path {
            if let Some(pos) = self.entries.iter().position(|e| e.path == path) {
                self.selected_idx = pos;
            }
        }
        
        // Final safety clamp
        if !self.entries.is_empty() && self.selected_idx >= self.entries.len() {
            self.selected_idx = self.entries.len() - 1;
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

    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    pub fn filtered_count(&self) -> usize {
        self.entries.len()
    }

    pub fn open_in_system_explorer(&self) {
        if let Some(entry) = self.selected_entry() {
            let path = if entry.is_dir { 
                &entry.path 
            } else { 
                entry.path.parent().unwrap_or(&entry.path) 
            };
            
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(path).spawn();
            
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
            
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("explorer").arg(path).spawn();
        }
    }

    pub fn draw(
        &mut self,
        frame: &mut ratatui::Frame,
        area: Rect,
        vim: &crate::vim::VimState,
        theme: &crate::ui::colorscheme::ColorScheme,
    ) {
        let height = area.height as usize;
        self.scroll_into_view(height);

        // Apply horizontal padding (1 cell each side) matching the header block
        let padded_area = ratatui::layout::Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        };

        let mut list_items = Vec::new();
        for i in self.scroll_y..std::cmp::min(self.scroll_y + height, self.entries.len()) {
            let entry = &self.entries[i];
            let mut name = entry.path.file_name().and_then(|s| s.to_str()).unwrap_or("/").to_string();
            if entry.is_dir && !name.ends_with('/') {
                name.push('/');
            }

            // Build proper tree connectors using is_last flags
            let mut guide = String::new();
            if entry.depth > 0 {
                for k in 1..entry.depth {
                    let ancestor_is_last = (0..i).rev()
                        .find(|&j| self.entries[j].depth == k)
                        .map(|j| self.entries[j].is_last)
                        .unwrap_or(false);
                    if ancestor_is_last {
                        guide.push_str("  ");
                    } else {
                        guide.push_str("│ ");
                    }
                }
                if entry.is_last {
                    guide.push_str("└ ");
                } else {
                    guide.push_str("├ ");
                }
            }

            let (icon, icon_style): (&str, ratatui::style::Style) = if entry.is_dir {
                (if entry.is_expanded { "󰉖" } else { "󰉋" }, theme.get("TreeExplorerFolderIcon"))
            } else {
                let ext = entry.path.extension().and_then(|s| s.to_str()).unwrap_or("");
                let (icon, style_name) = match ext {
                    "rs" => ("", "TreeExplorerFileIcon"),
                    "ts" | "tsx" => (" ", "Type"),
                    "js" | "jsx" => (" ", "Constant"),
                    "py" => ("", "Function"),
                    "go" => ("", "Type"),
                    "lua" => ("", "Constant"),
                    "json" => ("", "String"),
                    "toml" => ("", "Keyword"),
                    "md" => ("", "Comment"),
                    "html" => ("", "Tag"),
                    "css" => ("", "Attribute"),
                    _ => (crate::ui::icons::FILE, "TreeExplorerFileIcon"),
                };
                (icon, theme.get(style_name))
            };

            let name_style = if i == self.selected_idx && vim.focus == crate::vim::mode::Focus::Explorer {
                theme.get("Visual")
            } else if entry.is_dir {
                theme.get("TreeExplorerFolderName")
            } else {
                theme.get("TreeExplorerFileName")
            };

            list_items.push(ListItem::new(Line::from(vec![
                Span::styled(guide, theme.get("TreeExplorerConnector")),
                Span::styled(format!("{} ", icon), icon_style),
                Span::styled(name, name_style),
            ])));
        }

        frame.render_widget(List::new(list_items), padded_area);
    }
}
