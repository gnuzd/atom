use std::path::{Path, PathBuf};
use crate::ui::trouble::{TroubleItem, TroubleType};
use ignore::WalkBuilder;
use std::fs;
use ropey::Rope;

pub fn scan_todos(path: &PathBuf, text: &Rope) -> Vec<TroubleItem> {
    let mut todos = Vec::new();
    let todo_keywords = ["TODO", "FIXME", "BUG", "HACK", "NOTE"];

    for (y, line) in text.lines().enumerate() {
        let line_str = line.to_string();
        for keyword in todo_keywords {
            if let Some(x) = line_str.find(keyword) {
                if line_str.trim_start().starts_with("//") || (line_str.contains("//") && line_str.find("//").unwrap() < x) {
                     todos.push(TroubleItem {
                        path: path.clone(),
                        line: y,
                        col: x,
                        message: line_str[x..].trim().to_string(),
                        severity: None,
                        item_type: TroubleType::Todo,
                    });
                }
            }
        }
    }
    todos
}

pub fn scan_project_todos(root: &Path) -> Vec<TroubleItem> {
    let mut todos = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let valid_exts = ["rs", "js", "ts", "jsx", "tsx", "svelte", "py", "c", "cpp", "h", "hpp", "lua"];
                if valid_exts.contains(&ext) {
                    if let Ok(content) = fs::read_to_string(path) {
                        let text = Rope::from_str(&content);
                        todos.extend(scan_todos(&path.to_path_buf(), &text));
                    }
                }
            }
        }
    }
    todos
}
