use git2::Repository;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub enum GitSign {
    Add,
    Change,
    Delete,
    TopDelete,
    ChangeDelete,
}

pub struct GitManager {
    repo: Option<Repository>,
}

impl GitManager {
    pub fn new(project_root: &Path) -> Self {
        let repo = Repository::discover(project_root).ok();
        Self { repo }
    }

    pub fn get_signs(&self, file_path: &Path, _content: &str) -> Vec<(usize, GitSign)> {
        use std::process::Command;

        let Some(repo) = &self.repo else { return Vec::new(); };
        let workdir = repo.workdir().unwrap_or(Path::new("."));
        
        let relative_path = if let Ok(rel) = file_path.strip_prefix(workdir) {
            rel
        } else {
            file_path
        };

        let output = Command::new("git")
            .args(&["diff", "--unified=0", relative_path.to_str().unwrap()])
            .current_dir(workdir)
            .output();

        let mut signs = Vec::new();
        match output {
            Ok(o) => {
                let diff_str = String::from_utf8_lossy(&o.stdout);
                for line in diff_str.lines() {
                    if line.starts_with("@@") {
                        // Parse @@ -old_start,old_lines +new_start,new_lines @@
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 3 {
                            let new_info = &parts[2][1..]; // Skip '+'
                            let new_parts: Vec<&str> = new_info.split(',').collect();
                            let new_start: usize = new_parts[0].parse().unwrap_or(0);
                            let new_lines: usize = if new_parts.len() > 1 { new_parts[1].parse().unwrap_or(1) } else { 1 };

                            let old_info = &parts[1][1..]; // Skip '-'
                            let old_parts: Vec<&str> = old_info.split(',').collect();
                            let old_lines: usize = if old_parts.len() > 1 { old_parts[1].parse().unwrap_or(1) } else { 1 };

                            if old_lines > 0 && new_lines > 0 {
                                // Changed
                                for i in 0..new_lines {
                                    signs.push((new_start + i - 1, GitSign::Change));
                                }
                            } else if old_lines > 0 && new_lines == 0 {
                                // Deleted
                                if new_start == 0 {
                                    signs.push((0, GitSign::TopDelete));
                                } else {
                                    signs.push((new_start - 1, GitSign::Delete));
                                }
                            } else if old_lines == 0 && new_lines > 0 {
                                // Added
                                for i in 0..new_lines {
                                    signs.push((new_start + i - 1, GitSign::Add));
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {}
        }

        signs
    }

    pub fn get_blame(&self, file_path: &Path, line: usize) -> Option<String> {
        let repo = self.repo.as_ref()?;
        let workdir = repo.workdir().unwrap_or(Path::new(""));
        let relative_path = file_path.strip_prefix(workdir).ok()?;
        
        let mut opts = git2::BlameOptions::new();
        opts.track_copies_same_file(true);
        
        let blame = repo.blame_file(relative_path, Some(&mut opts)).ok()?;
        let hunk = blame.get_line(line + 1)?; // git2 blame is 1-indexed
        
        let commit_id = hunk.final_commit_id();
        let commit = repo.find_commit(commit_id).ok()?;
        let author = commit.author();
        let name = author.name().unwrap_or("Unknown");
        let time = commit.time();
        
        // Format time (rough relative time or just date)
        let datetime = std::time::UNIX_EPOCH + std::time::Duration::from_secs(time.seconds() as u64);
        let now = std::time::SystemTime::now();
        let ago = now.duration_since(datetime).ok()?.as_secs();
        
        let time_str = if ago < 60 {
            format!("{}s ago", ago)
        } else if ago < 3600 {
            format!("{}m ago", ago / 60)
        } else if ago < 86400 {
            format!("{}h ago", ago / 3600)
        } else {
            format!("{}d ago", ago / 86400)
        };

        let short_id = &commit_id.to_string()[..7];
        Some(format!("{} • {} • {}", short_id, name, time_str))
    }
}
