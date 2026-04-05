pub mod client;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;
use std::sync::{Arc, Mutex};
use crate::lsp::client::LspClient;
use lsp_types::*;
use std::process::Command;

#[derive(PartialEq, Clone, Copy)]
pub enum ClientState {
    Starting,
    Ready,
}

#[derive(Clone)]
pub struct LspManager {
    pub clients: Arc<Mutex<HashMap<String, (LspClient, ClientState)>>>,
    pub failed_exts: Arc<Mutex<HashSet<String>>>,
    pub installed_cache: Arc<Mutex<HashMap<String, bool>>>,
    pub installing: Arc<Mutex<HashSet<String>>>,
    pub formatter_cache: Arc<Mutex<HashMap<String, String>>>,
    pub not_found_cache: Arc<Mutex<HashSet<String>>>,
    pub root_cache: Arc<Mutex<HashMap<String, std::path::PathBuf>>>,
    pub bin_cache: Arc<Mutex<HashMap<String, std::path::PathBuf>>>,
    pub versions: Arc<Mutex<HashMap<String, i32>>>,
    pub last_change: Option<Instant>,
    pub pending_change: bool,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            failed_exts: Arc::new(Mutex::new(HashSet::new())),
            installed_cache: Arc::new(Mutex::new(HashMap::new())),
            installing: Arc::new(Mutex::new(HashSet::new())),
            formatter_cache: Arc::new(Mutex::new(HashMap::new())),
            not_found_cache: Arc::new(Mutex::new(HashSet::new())),
            root_cache: Arc::new(Mutex::new(HashMap::new())),
            bin_cache: Arc::new(Mutex::new(HashMap::new())),
            versions: Arc::new(Mutex::new(HashMap::new())),
            last_change: None,
            pending_change: false,
        }
    }

    pub fn is_ready(&self, ext: &str) -> bool {
        self.clients.lock().unwrap().get(ext).map(|(_, s)| *s == ClientState::Ready).unwrap_or(false)
    }

    pub fn is_installed(&self, server_cmd: &str) -> bool {
        {
            let cache = self.installed_cache.lock().unwrap();
            if let Some(&status) = cache.get(server_cmd) { return status; }
        }

        let status = Command::new(server_cmd)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        
        self.installed_cache.lock().unwrap().insert(server_cmd.to_string(), status);
        status
    }

    pub fn is_any_installing(&self) -> bool {
        !self.installing.lock().unwrap().is_empty()
    }

    pub fn get_server_command(ext: &str) -> Option<(&'static str, &'static [&'static str])> {
        match ext {
            "rs" => Some(("rust-analyzer", &[])),
            "py" => Some(("pyright-langserver", &["--stdio"])),
            "js" | "ts" | "jsx" | "tsx" => Some(("typescript-language-server", &["--stdio"])),
            "svelte" => Some(("svelteserver", &["--stdio"])),
            _ => None,
        }
    }

    pub fn get_install_command(server_cmd: &str) -> Option<(&'static str, &'static [&'static str])> {
        match server_cmd {
            "rust-analyzer" => Some(("rustup", &["component", "add", "rust-analyzer"])),
            "pyright-langserver" => Some(("npm", &["install", "-g", "pyright"])),
            "typescript-language-server" => Some(("npm", &["install", "-g", "typescript-language-server", "typescript"])),
            "svelteserver" => Some(("npm", &["install", "-g", "svelte-language-server"])),
            _ => None,
        }
    }

    pub fn install_server(&self, server_cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.installing.lock().unwrap().insert(server_cmd.to_string());
        let result = (|| {
            if let Some((cmd, args)) = Self::get_install_command(server_cmd) {
                // Run in home directory to avoid npm local project detection
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let status = Command::new(cmd)
                    .args(args)
                    .current_dir(home)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()?;
                if status.success() {
                    self.installed_cache.lock().unwrap().insert(server_cmd.to_string(), true);
                    return Ok(());
                } else {
                    return Err(format!("Installation failed with status: {}", status).into());
                }
            }
            Err("No install command known for this server".into())
        })();
        self.installing.lock().unwrap().remove(server_cmd);
        result
    }

    pub fn start_client(&mut self, ext: &str, root_path: std::path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if self.clients.lock().unwrap().contains_key(ext) { return Ok(()); }
        if self.failed_exts.lock().unwrap().contains(ext) { return Err("Already failed".into()); }

        if let Some((cmd, args)) = Self::get_server_command(ext) {
            match LspClient::start(cmd, args) {
                Ok(client) => {
                    let root_uri = Url::from_directory_path(root_path).unwrap();
                    client.send_initialize(root_uri)?;
                    self.clients.lock().unwrap().insert(ext.to_string(), (client, ClientState::Starting));
                }
                Err(e) => {
                    self.failed_exts.lock().unwrap().insert(ext.to_string());
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub fn did_open(&self, ext: &str, path: &Path, text: String) -> Result<(), Box<dyn std::error::Error>> {
        let clients = self.clients.lock().unwrap();
        if let Some((client, _)) = clients.get(ext) {
            let params = DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: Url::from_file_path(path).unwrap(),
                    language_id: match ext {
                        "rs" => "rust",
                        "py" => "python",
                        "js" => "javascript",
                        "ts" => "typescript",
                        "jsx" => "javascriptreact",
                        "tsx" => "typescriptreact",
                        "svelte" => "svelte",
                        _ => ext,
                    }.to_string(),
                    version: 0,
                    text,
                },
            };
            client.send_notification("textDocument/didOpen", params)?;
        }
        Ok(())
    }

    pub fn did_change(&self, ext: &str, path: &Path, text: String) -> Result<(), Box<dyn std::error::Error>> {
        let clients = self.clients.lock().unwrap();
        if let Some((client, state)) = clients.get(ext) {
            if *state != ClientState::Ready { return Ok(()); }
            
            let mut versions = self.versions.lock().unwrap();
            let version = versions.entry(path.to_string_lossy().to_string()).or_insert(0);
            *version += 1;

            let params = DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: Url::from_file_path(path).unwrap(),
                    version: *version,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text,
                }],
            };
            client.send_notification("textDocument/didChange", params)?;
        }
        Ok(())
    }

    pub fn format_document(&self, ext: &str, path: &Path, text: String) -> Option<Result<String, String>> {
        let mut formatters: Vec<(String, Vec<String>)> = match ext {
            "rs" | "rust" => vec![("rustfmt".to_string(), vec!["--emit".to_string(), "stdout".to_string(), "--edition".to_string(), "2021".to_string()])],
            "lua" => vec![("stylua".to_string(), vec!["-".to_string()])],
            "css" | "html" | "graphql" | "js" | "ts" | "jsx" | "tsx" | "svelte" | "javascript" | "typescript" | "json" | "jsonc" => {
                let file_path = path.to_str().unwrap_or("");
                let is_svelte = ext == "svelte";
                let mut base_args = vec!["--stdin-filepath".to_string(), file_path.to_string(), "--tab-width".to_string(), "2".to_string(), "--use-tabs".to_string(), "false".to_string()];
                if is_svelte {
                    base_args.push("--plugin".to_string());
                    base_args.push("prettier-plugin-svelte".to_string());
                    base_args.push("--parser".to_string());
                    base_args.push("svelte".to_string());
                }

                let mut npx_args = vec!["--yes".to_string(), "prettier".to_string()];
                npx_args.extend(base_args.clone());

                let mut candidates = vec![
                    ("prettierd".to_string(), base_args.clone()),
                ];

                let parent_str = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                let local_key = format!("{}:prettier", parent_str);
                let cached_bin = self.bin_cache.lock().unwrap().get(&local_key).cloned();
                
                if let Some(bin) = cached_bin {
                    candidates.push((bin.to_string_lossy().to_string(), base_args.clone()));
                } else if let Some(local_prettier) = find_local_bin(path, "prettier") {
                    self.bin_cache.lock().unwrap().insert(local_key, local_prettier.clone());
                    candidates.push((local_prettier.to_string_lossy().to_string(), base_args.clone()));
                }

                candidates.push(("prettier".to_string(), base_args));
                candidates.push(("npx".to_string(), npx_args));
                candidates
            },
            _ => return None,
        };

        {
            let cache = self.formatter_cache.lock().unwrap();
            if let Some(cached_cmd) = cache.get(ext) {
                if let Some(pos) = formatters.iter().position(|(cmd, _)| cmd == cached_cmd) {
                    let cached_item = formatters.remove(pos);
                    formatters.insert(0, cached_item);
                }
            }
        }

        let mut last_err = String::from("No formatter succeeded");
        
        let root_dir = {
            let path_str = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
            let mut cache = self.root_cache.lock().unwrap();
            if let Some(root) = cache.get(&path_str) {
                root.clone()
            } else {
                let root = find_project_root_static(path);
                cache.insert(path_str, root.clone());
                root
            }
        };

        for (cmd, args) in formatters {
            {
                let nf = self.not_found_cache.lock().unwrap();
                if nf.contains(&cmd) { continue; }
            }

            use std::io::Write;
            let mut child_cmd = Command::new(&cmd);
            child_cmd.args(&args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            child_cmd.current_dir(&root_dir);

            match child_cmd.spawn() {
                Ok(mut child) => {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(text.as_bytes());
                        drop(stdin);
                    }

                    match child.wait_with_output() {
                        Ok(output) => {
                            if output.status.success() && !output.stdout.is_empty() {
                                let formatted = String::from_utf8_lossy(&output.stdout).to_string();
                                self.formatter_cache.lock().unwrap().insert(ext.to_string(), cmd);
                                return Some(Ok(formatted));
                            } else {
                                let stderr = String::from_utf8_lossy(&output.stderr);
                                last_err = format!("{}: {}", cmd, stderr.trim());
                            }
                        }
                        Err(e) => {
                            last_err = format!("{}: failed to wait: {}", cmd, e);
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    self.not_found_cache.lock().unwrap().insert(cmd.to_string());
                    continue;
                }
                Err(e) => {
                    last_err = format!("{}: failed to spawn: {}", cmd, e);
                }
            }
        }
        
        Some(Err(last_err))
    }

    pub fn request_completions(&self, ext: &str, path: &Path, line: usize, character: usize) -> Result<i32, Box<dyn std::error::Error>> {
        let clients = self.clients.lock().unwrap();
        if let Some((client, state)) = clients.get(ext) {
            if *state != ClientState::Ready { return Err("LSP not ready".into()); }
            let id = 100;
            let params = CompletionParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: Url::from_file_path(path).unwrap(),
                    },
                    position: lsp_types::Position {
                        line: line as u32,
                        character: character as u32,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: Some(CompletionContext {
                    trigger_kind: CompletionTriggerKind::INVOKED,
                    trigger_character: None,
                }),
            };
            client.send_request(id, "textDocument/completion", params)?;
            return Ok(id);
        }
        Err("No LSP client".into())
    }
}

pub fn char_to_utf16_offset(s: &str, char_idx: usize) -> usize {
    s.chars().take(char_idx).map(|c| c.len_utf16()).sum()
}

fn find_project_root_static(path: &Path) -> std::path::PathBuf {
    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        if parent.join("package.json").exists() || parent.join("Cargo.toml").exists() || parent.join(".git").exists() {
            return parent.to_path_buf();
        }
        current = parent.to_path_buf();
    }
    path.parent().unwrap_or(path).to_path_buf()
}

fn find_local_bin(path: &Path, name: &str) -> Option<std::path::PathBuf> {
    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        let bin = parent.join("node_modules").join(".bin").join(name);
        if bin.exists() {
            return Some(bin);
        }
        if parent.join("package.json").exists() { break; }
        current = parent.to_path_buf();
    }
    None
}
