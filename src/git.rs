use git2::Repository;
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
pub enum GitSign {
    Add,
    Change,
    Delete,
    TopDelete,
    ChangeDelete,
}

#[derive(Clone)]
pub struct GitManager {
    repo: Arc<Mutex<Option<Repository>>>,
}

impl GitManager {
    pub fn new(project_root: &Path) -> Self {
        let repo = Repository::discover(project_root).ok();
        Self { repo: Arc::new(Mutex::new(repo)) }
    }

    pub fn get_signs(&self, path: &Path, content: &str) -> Vec<(usize, GitSign)> {
        let mut signs = Vec::new();
        let repo_lock = self.repo.lock().unwrap();
        if let Some(repo) = repo_lock.as_ref() {
            if let Ok(rel_path) = path.strip_prefix(repo.workdir().unwrap_or(Path::new(""))) {
                let blob_res = repo.revparse_single("HEAD:").and_then(|obj| {
                    obj.as_tree().unwrap().get_path(rel_path).and_then(|entry| repo.find_blob(entry.id()))
                });

                if let Ok(blob) = blob_res {
                    let old_content = String::from_utf8_lossy(blob.content());
                    let old_lines: Vec<&str> = old_content.lines().collect();
                    let new_lines: Vec<&str> = content.lines().collect();
                    
                    for (i, line) in new_lines.iter().enumerate() {
                        if i >= old_lines.len() {
                            signs.push((i, GitSign::Add));
                        } else if line != &old_lines[i] {
                            signs.push((i, GitSign::Change));
                        }
                    }
                }
            }
        }
        signs
    }
}
