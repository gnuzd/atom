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
                    signs = diff_signs(&old_lines, &new_lines);
                }
            }
        }
        signs
    }
}

/// Compute git-style line signs by diffing old vs new lines.
/// Uses a simple patience-like LCS approach to detect Add, Change, Delete, TopDelete.
fn diff_signs(old: &[&str], new: &[&str]) -> Vec<(usize, GitSign)> {
    let mut signs = Vec::new();

    // Build LCS table
    let m = old.len();
    let n = new.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            dp[i][j] = if old[i] == new[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    // Walk the edit script
    let mut i = 0;
    let mut j = 0;
    let mut pending_deletes = 0usize;

    while i < m || j < n {
        if i < m && j < n && old[i] == new[j] {
            // Unchanged line — flush any pending deletes before this line
            if pending_deletes > 0 {
                if j == 0 {
                    signs.push((0, GitSign::TopDelete));
                } else {
                    // Check if the line before was already marked as Change;
                    // if so use ChangeDelete, else Delete
                    let prev_sign = signs.iter().rev().find(|(l, _)| *l == j - 1);
                    match prev_sign.map(|(_, s)| s) {
                        Some(GitSign::Change) | Some(GitSign::ChangeDelete) => {
                            // upgrade to ChangeDelete
                            if let Some(entry) = signs.iter_mut().rfind(|(l, _)| *l == j - 1) {
                                entry.1 = GitSign::ChangeDelete;
                            }
                        }
                        _ => {
                            signs.push((j - 1, GitSign::Delete));
                        }
                    }
                }
                pending_deletes = 0;
            }
            i += 1;
            j += 1;
        } else if j < n && (i >= m || dp[i][j + 1] >= dp[i + 1][j]) {
            // Added line
            if pending_deletes > 0 {
                // delete followed by add → Change
                signs.push((j, GitSign::Change));
                pending_deletes -= 1;
            } else {
                signs.push((j, GitSign::Add));
            }
            j += 1;
        } else {
            // Deleted line
            pending_deletes += 1;
            i += 1;
        }
    }

    // Trailing deletes (lines removed from the end)
    if pending_deletes > 0 {
        if j == 0 {
            signs.push((0, GitSign::TopDelete));
        } else {
            signs.push((j - 1, GitSign::Delete));
        }
    }

    // Deduplicate: keep the most informative sign per line
    signs.sort_by_key(|(l, _)| *l);
    signs.dedup_by(|b, a| {
        if a.0 == b.0 {
            // Keep ChangeDelete > Change > Delete > Add, else keep first
            let priority = |s: &GitSign| match s {
                GitSign::ChangeDelete => 4,
                GitSign::Change => 3,
                GitSign::Delete | GitSign::TopDelete => 2,
                GitSign::Add => 1,
            };
            if priority(&b.1) > priority(&a.1) {
                a.1 = b.1.clone();
            }
            true
        } else {
            false
        }
    });

    signs
}
