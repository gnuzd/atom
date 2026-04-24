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

    /// Returns a one-line blame summary for `line` (0-indexed): "abc12345 Author • X ago • message"
    pub fn get_blame_line(&self, path: &Path, line: usize) -> Option<String> {
        let repo_lock = self.repo.lock().unwrap();
        let repo = repo_lock.as_ref()?;
        let workdir = repo.workdir()?;
        let rel = path.strip_prefix(workdir).ok()?;
        let blame = repo.blame_file(rel, None).ok()?;
        let hunk = blame.get_line(line + 1)?;
        let commit_id = hunk.final_commit_id();
        if commit_id == git2::Oid::zero() {
            return Some("Not committed yet".to_string());
        }
        let commit = repo.find_commit(commit_id).ok()?;
        let author = commit.author();
        let name = author.name().unwrap_or("Unknown");
        let summary = commit.summary().unwrap_or("No message");
        let short_id = &commit_id.to_string()[..8];

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let ago = now_secs - commit.time().seconds();
        let time_str = if ago < 60 {
            "just now".to_string()
        } else if ago < 3600 {
            format!("{} min ago", ago / 60)
        } else if ago < 86400 {
            format!("{} hr ago", ago / 3600)
        } else {
            format!("{} days ago", ago / 86400)
        };

        Some(format!("{} {} • {} • {}", short_id, name, time_str, summary))
    }

    /// Returns an inline diff string with line numbers, markers, context, and hunk separators.
    /// Format per line: "{num:>4} {marker} {content}" where marker is +, -, or space.
    /// Hunks are separated by "~~~".
    pub fn get_hunk_diff(&self, path: &Path, content: &str) -> Option<String> {
        let repo_lock = self.repo.lock().unwrap();
        let repo = repo_lock.as_ref()?;
        let workdir = repo.workdir()?;
        let rel = path.strip_prefix(workdir).ok()?;
        let blob = repo.revparse_single("HEAD:").ok()
            .and_then(|obj| obj.as_tree().unwrap().get_path(rel).ok().map(|e| (obj, e)))
            .and_then(|(_, entry)| repo.find_blob(entry.id()).ok())?;
        let old_content = String::from_utf8_lossy(blob.content());
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = content.lines().collect();

        let m = old_lines.len();
        let n = new_lines.len();

        let mut dp = vec![vec![0usize; n + 1]; m + 1];
        for ii in (0..m).rev() {
            for jj in (0..n).rev() {
                dp[ii][jj] = if old_lines[ii] == new_lines[jj] {
                    dp[ii + 1][jj + 1] + 1
                } else {
                    dp[ii + 1][jj].max(dp[ii][jj + 1])
                };
            }
        }

        // ops: (display_line_num, marker, content)
        // display_num = new_line for context/add, old_line for remove
        let mut ops: Vec<(usize, char, String)> = Vec::new();
        let mut oi = 0usize;
        let mut ni = 0usize;
        let mut guard = 0usize;

        while (oi < m || ni < n) && guard < 2000 {
            guard += 1;
            if oi < m && ni < n && old_lines[oi] == new_lines[ni] {
                ops.push((ni + 1, ' ', new_lines[ni].to_string()));
                oi += 1; ni += 1;
            } else if ni < n && (oi >= m || dp[oi][ni + 1] >= dp[oi + 1][ni]) {
                ops.push((ni + 1, '+', new_lines[ni].to_string()));
                ni += 1;
            } else {
                ops.push((oi + 1, '-', old_lines[oi].to_string()));
                oi += 1;
            }
        }

        let changed: Vec<usize> = ops.iter().enumerate()
            .filter(|(_, op)| op.1 != ' ')
            .map(|(i, _)| i)
            .collect();

        if changed.is_empty() { return None; }

        const CTX: usize = 3;
        let mut hunks: Vec<(usize, usize)> = Vec::new();

        for &ci in &changed {
            let s = ci.saturating_sub(CTX);
            let e = (ci + CTX).min(ops.len().saturating_sub(1));
            if let Some(last) = hunks.last_mut() {
                if s <= last.1 + 1 {
                    last.1 = last.1.max(e);
                    continue;
                }
            }
            hunks.push((s, e));
        }

        let mut out = String::new();
        for (h_idx, (start, end)) in hunks.iter().enumerate() {
            if h_idx > 0 {
                out.push_str("~~~\n");
            }
            for (num, marker, line_content) in &ops[*start..=*end] {
                out.push_str(&format!("{:>4} {} {}\n", num, marker, line_content));
            }
        }

        if out.is_empty() { None } else { Some(out.trim_end().to_string()) }
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
