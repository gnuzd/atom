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

    pub fn get_server_command(ext: &str) -> Option<(&'static str, &'static [&'static str])> {
        match ext {
            "rs" => Some(("rust-analyzer", &[])),
            "py" => Some(("pyright-langserver", &["--stdio"])),
            "js" | "ts" => Some(("typescript-language-server", &["--stdio"])),
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
                return Ok(());
            } else {
                return Err(format!("Installation failed with status: {}", status).into());
            }
        }
        Err("No install command known for this server".into())
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

    pub fn format_document(&self, ext: &str, path: &Path, text: String) -> Option<String> {
        // External formatters like conform.nvim
        // Try prettierd first if it's a JS/TS/Svelte file, then fallback to prettier or npx prettier
        let formatters = match ext {
            "lua" => vec![("stylua", vec!["-"])],
            "css" | "html" | "graphql" | "js" | "ts" | "jsx" | "tsx" | "svelte" => {
                vec![
                    ("prettierd", vec![path.to_str().unwrap()]),
                    ("prettier", vec!["--stdin-filepath", path.to_str().unwrap()]),
                    ("npx", vec!["prettier", "--stdin-filepath", path.to_str().unwrap()]),
                ]
            },
            _ => return None,
        };

        for (cmd, args) in formatters {
            use std::io::Write;
            let mut child_cmd = Command::new(cmd);
            child_cmd.args(&args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());

            // Try to set current_dir to project root
            if let Some(parent) = path.parent() {
                child_cmd.current_dir(parent);
            }

            if let Ok(mut child) = child_cmd.spawn() {
                if let Some(mut stdin) = child.stdin.take() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        drop(stdin);
                        if let Ok(output) = child.wait_with_output() {
                            if output.status.success() && !output.stdout.is_empty() {
                                return String::from_utf8(output.stdout).ok();
                            }
                        }
                    }
                }
            }
        }
        
        None
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
