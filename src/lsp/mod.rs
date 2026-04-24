pub mod client;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;
use std::sync::{Arc, Mutex};
use crate::lsp::client::LspClient;
use lsp_types::*;
use std::process::Command;

#[derive(serde::Deserialize)]
struct EslintMessage {
    line: u32,
    column: u32,
    #[serde(rename = "endLine")]
    end_line: Option<u32>,
    #[serde(rename = "endColumn")]
    end_column: Option<u32>,
    severity: i32,
    message: String,
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct EslintResult {
    messages: Vec<EslintMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Lsp,
    Dap,
    Linter,
    Formatter,
}

pub struct Package {
    pub name: &'static str,
    pub cmd: &'static str,
    pub kind: PackageKind,
    pub description: &'static str,
    pub install_cmd: &'static str,
    pub install_args: &'static [&'static str],
}

impl Package {
    pub fn needs_npm(&self) -> bool {
        self.install_cmd == "npm"
    }
}

pub const PACKAGES: &[Package] = &[
    Package {
        name: "rust-analyzer",
        cmd: "rust-analyzer",
        kind: PackageKind::Lsp,
        description: "Rust Language Server",
        install_cmd: "rustup",
        install_args: &["component", "add", "rust-analyzer"],
    },
    Package {
        name: "pyright",
        cmd: "pyright-langserver",
        kind: PackageKind::Lsp,
        description: "Static type checker for Python",
        install_cmd: "npm",
        install_args: &["install", "pyright"],
    },
    Package {
        name: "typescript-language-server",
        cmd: "typescript-language-server",
        kind: PackageKind::Lsp,
        description: "LSP for TypeScript & JavaScript",
        install_cmd: "npm",
        install_args: &["install", "typescript-language-server", "typescript"],
    },
    Package {
        name: "svelte-language-server",
        cmd: "svelteserver",
        kind: PackageKind::Lsp,
        description: "LSP for Svelte",
        install_cmd: "npm",
        install_args: &["install", "svelte-language-server"],
    },
    Package {
        name: "prettierd",
        cmd: "prettierd",
        kind: PackageKind::Formatter,
        description: "Prettier daemon",
        install_cmd: "npm",
        install_args: &["install", "@fsouza/prettierd"],
    },
    Package {
        name: "stylua",
        cmd: "stylua",
        kind: PackageKind::Formatter,
        description: "Opinionated Lua code formatter",
        install_cmd: "cargo",
        install_args: &["install", "stylua"],
    },
    Package {
        name: "eslint_d",
        cmd: "eslint_d",
        kind: PackageKind::Linter,
        description: "Fast ESLint daemon",
        install_cmd: "npm",
        install_args: &["install", "eslint_d"],
    },
    Package {
        name: "eslint-lsp",
        cmd: "vscode-eslint-language-server",
        kind: PackageKind::Lsp,
        description: "ESLint Language Server",
        install_cmd: "npm",
        install_args: &["install", "vscode-langservers-extracted"],
    },
    Package {
        name: "tailwindcss-language-server",
        cmd: "tailwindcss-language-server",
        kind: PackageKind::Lsp,
        description: "Tailwind CSS Language Server",
        install_cmd: "npm",
        install_args: &["install", "@tailwindcss/language-server"],
    },
    Package {
        name: "vtsls",
        cmd: "vtsls",
        kind: PackageKind::Lsp,
        description: "Visual Studio Code TypeScript Language Server",
        install_cmd: "npm",
        install_args: &["install", "@vtsls/language-server"],
    },
    Package {
        name: "css-lsp",
        cmd: "vscode-css-language-server",
        kind: PackageKind::Lsp,
        description: "CSS/LESS/SCSS Language Server",
        install_cmd: "npm",
        install_args: &["install", "vscode-langservers-extracted"],
    },
    Package {
        name: "json-lsp",
        cmd: "vscode-json-language-server",
        kind: PackageKind::Lsp,
        description: "JSON Language Server",
        install_cmd: "npm",
        install_args: &["install", "vscode-langservers-extracted"],
    },
    Package {
        name: "lua-language-server",
        cmd: "lua-language-server",
        kind: PackageKind::Lsp,
        description: "LSP for Lua",
        install_cmd: "npm",
        install_args: &["install", "lua-language-server"],
    },
    Package {
        name: "tree-sitter-cli",
        cmd: "tree-sitter",
        kind: PackageKind::Linter,
        description: "Tree-sitter CLI",
        install_cmd: "cargo",
        install_args: &["install", "tree-sitter-cli"],
    },
    Package {
        name: "actionlint",
        cmd: "actionlint",
        kind: PackageKind::Linter,
        description: "GitHub Actions workflow linter",
        install_cmd: "go",
        install_args: &["install", "github.com/rhysd/actionlint/cmd/actionlint@latest"],
    },
    Package {
        name: "ansible-language-server",
        cmd: "ansible-language-server",
        kind: PackageKind::Lsp,
        description: "Ansible Language Server",
        install_cmd: "npm",
        install_args: &["install", "@ansible/ansible-language-server"],
    },
    Package {
        name: "bash-language-server",
        cmd: "bash-language-server",
        kind: PackageKind::Lsp,
        description: "Bash Language Server",
        install_cmd: "npm",
        install_args: &["install", "bash-language-server"],
    },
];

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ClientState {
    Starting,
    Ready,
}

#[derive(Clone)]
pub struct LspManager {
    pub clients: Arc<Mutex<HashMap<String, Vec<(LspClient, ClientState, String)>>>>,
    pub diagnostics: Arc<Mutex<HashMap<Url, HashMap<String, Vec<Diagnostic>>>>>,
    pub failed_exts: Arc<Mutex<HashSet<String>>>,
    pub installed_cache: Arc<Mutex<HashMap<String, bool>>>,
    pub installing: Arc<Mutex<HashSet<String>>>,
    /// Maps package cmd → current phase label ("downloading", "installing", "updating", "uninstalling")
    pub op_status: Arc<Mutex<HashMap<String, String>>>,
    /// Completed operation messages: (display text, is_success)
    pub op_messages: Arc<Mutex<Vec<(String, bool)>>>,
    /// Language names whose parsers were just installed/updated — consumed by the main loop
    /// to invalidate highlight caches so new parsers take effect without a restart.
    pub installed_parsers: Arc<Mutex<Vec<String>>>,
    pub formatter_cache: Arc<Mutex<HashMap<String, String>>>,
    pub not_found_cache: Arc<Mutex<HashSet<String>>>,
    pub root_cache: Arc<Mutex<HashMap<String, std::path::PathBuf>>>,
    pub bin_cache: Arc<Mutex<HashMap<String, std::path::PathBuf>>>,
    pub versions: Arc<Mutex<HashMap<String, i32>>>,
    pub id_counter: Arc<Mutex<i32>>,
    pub last_change: Option<Instant>,
    pub pending_change: bool,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            diagnostics: Arc::new(Mutex::new(HashMap::new())),
            failed_exts: Arc::new(Mutex::new(HashSet::new())),
            installed_cache: Arc::new(Mutex::new(HashMap::new())),
            installing: Arc::new(Mutex::new(HashSet::new())),
            op_status: Arc::new(Mutex::new(HashMap::new())),
            op_messages: Arc::new(Mutex::new(Vec::new())),
            installed_parsers: Arc::new(Mutex::new(Vec::new())),
            formatter_cache: Arc::new(Mutex::new(HashMap::new())),
            not_found_cache: Arc::new(Mutex::new(HashSet::new())),
            root_cache: Arc::new(Mutex::new(HashMap::new())),
            bin_cache: Arc::new(Mutex::new(HashMap::new())),
            versions: Arc::new(Mutex::new(HashMap::new())),
            id_counter: Arc::new(Mutex::new(100)),
            last_change: None,
            pending_change: false,
        }
    }

    pub fn get_local_bin_dir() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = std::path::PathBuf::from(home);
        path.push(".local");
        path.push("share");
        path.push("atom");
        path.push("nucleus");
        path
    }

    pub fn is_ready(&self, ext: &str) -> bool {
        self.clients.lock().unwrap().get(ext).map(|clients| clients.iter().any(|(_, s, _)| *s == ClientState::Ready)).unwrap_or(false)
    }

    pub fn is_managed(&self, server_cmd: &str) -> bool {
        // Check local bin directory for Nucleus-managed packages (npm)
        let npm_bin = Self::get_local_bin_dir().join("node_modules").join(".bin").join(server_cmd);
        if npm_bin.exists() {
            return true;
        }

        // Check top-level bin directory for other tools
        let direct_bin = Self::get_local_bin_dir().join("bin").join(server_cmd);
        if direct_bin.exists() {
            return true;
        }

        // For non-npm tools like rust-analyzer, we might need a marker or just check if it was explicitly installed.
        let marker = Self::get_local_bin_dir().join(format!("{}.managed", server_cmd));
        marker.exists()
    }

    pub fn is_installed(&self, server_cmd: &str) -> bool {
        {
            let cache = self.installed_cache.lock().unwrap();
            if let Some(&status) = cache.get(server_cmd) { return status; }
        }

        if self.is_managed(server_cmd) {
            self.installed_cache.lock().unwrap().insert(server_cmd.to_string(), true);
            return true;
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

    pub fn get_server_commands(&self, ext: &str) -> Vec<(&'static str, &'static [&'static str])> {
        let mut servers = Vec::new();
        match ext {
            "rs" => servers.push(("rust-analyzer", &[] as &[&str])),
            "py" => servers.push(("pyright-langserver", &["--stdio"])),
            "js" | "ts" | "jsx" | "tsx" => {
                if self.is_installed("vtsls") {
                    servers.push(("vtsls", &["--stdio"]));
                } else {
                    servers.push(("typescript-language-server", &["--stdio"]));
                }

                if self.is_installed("eslint-lsp") {
                    servers.push(("vscode-eslint-language-server", &["--stdio"]));
                }
            }
            "svelte" => servers.push(("svelteserver", &["--stdio"])),
            _ => {}
        }
        servers
    }

    pub fn get_install_command(server_cmd: &str) -> Option<(&'static str, Vec<String>)> {
        if let Some(pkg) = PACKAGES.iter().find(|p| p.cmd == server_cmd || p.name == server_cmd) {
            let mut args = Vec::new();
            if pkg.install_cmd == "npm" {
                args.push("install".to_string());
                args.push("--prefix".to_string());
                args.push(Self::get_local_bin_dir().to_string_lossy().to_string());
                for arg in pkg.install_args {
                    if *arg != "install" {
                        args.push(arg.to_string());
                    }
                }
            } else {
                args.extend(pkg.install_args.iter().map(|s| s.to_string()));
            }
            return Some((pkg.install_cmd, args));
        }
        None
    }

    pub fn uninstall_server(&self, server_cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.installing.lock().unwrap().insert(server_cmd.to_string());
        self.op_status.lock().unwrap().insert(server_cmd.to_string(), "uninstalling".to_string());

        let local_dir = Self::get_local_bin_dir();

        // Remove bin symlink
        let bin_dir = local_dir.join("bin");
        let target = bin_dir.join(server_cmd);
        if target.exists() {
            let _ = std::fs::remove_file(&target);
        }

        // Proper npm uninstall
        if let Some(pkg) = PACKAGES.iter().find(|p| p.cmd == server_cmd || p.name == server_cmd) {
            if pkg.install_cmd == "npm" {
                // Find the npm package name from install_args (skip "install")
                let npm_pkg_name = pkg.install_args.iter().find(|&&a| a != "install").copied().unwrap_or(server_cmd);
                let _ = Command::new("npm")
                    .args(&["uninstall", "--prefix", &local_dir.to_string_lossy(), npm_pkg_name])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
        }

        // Remove marker file
        let marker = local_dir.join(format!("{}.managed", server_cmd));
        if marker.exists() {
            let _ = std::fs::remove_file(marker);
        }

        self.installed_cache.lock().unwrap().remove(server_cmd);
        self.installing.lock().unwrap().remove(server_cmd);
        self.op_status.lock().unwrap().remove(server_cmd);
        self.op_messages.lock().unwrap().push((format!("{} uninstalled", server_cmd), true));
        Ok(())
    }

    pub fn update_server(&self, server_cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.installing.lock().unwrap().insert(server_cmd.to_string());
        self.op_status.lock().unwrap().insert(server_cmd.to_string(), "downloading".to_string());

        let result = self.run_install_command(server_cmd, "updating");

        self.installing.lock().unwrap().remove(server_cmd);
        self.op_status.lock().unwrap().remove(server_cmd);

        match &result {
            Ok(()) => self.op_messages.lock().unwrap().push((format!("{} updated successfully", server_cmd), true)),
            Err(e) => self.op_messages.lock().unwrap().push((format!("{} update failed: {}", server_cmd, e), false)),
        }
        result
    }

    pub fn install_server(&self, server_cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.installing.lock().unwrap().insert(server_cmd.to_string());
        self.op_status.lock().unwrap().insert(server_cmd.to_string(), "downloading".to_string());

        let result = self.run_install_command(server_cmd, "installing");

        self.installing.lock().unwrap().remove(server_cmd);
        self.op_status.lock().unwrap().remove(server_cmd);

        match &result {
            Ok(()) => self.op_messages.lock().unwrap().push((format!("{} installed successfully", server_cmd), true)),
            Err(e) => self.op_messages.lock().unwrap().push((format!("{} install failed: {}", server_cmd, e), false)),
        }
        result
    }

    fn run_install_command(&self, server_cmd: &str, post_phase: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some((cmd, args)) = Self::get_install_command(server_cmd) {
            let local_dir = Self::get_local_bin_dir();
            if !local_dir.exists() {
                std::fs::create_dir_all(&local_dir)?;
            }

            let status = Command::new(cmd)
                .args(&args)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()?;

            if status.success() {
                // Transition to post-processing phase
                self.op_status.lock().unwrap().insert(server_cmd.to_string(), post_phase.to_string());

                let bin_dir = local_dir.join("bin");
                if !bin_dir.exists() {
                    let _ = std::fs::create_dir_all(&bin_dir);
                }

                if cmd == "npm" {
                    // Symlink the npm .bin entry into our bin/ dir
                    let npm_bin = local_dir.join("node_modules").join(".bin").join(server_cmd);
                    if npm_bin.exists() {
                        let target = bin_dir.join(server_cmd);
                        if target.exists() { let _ = std::fs::remove_file(&target); }
                        #[cfg(unix)]
                        let _ = std::os::unix::fs::symlink(&npm_bin, &target);
                    }
                } else {
                    // For rustup/cargo/go installs, leave a marker so is_managed() returns true
                    let marker = local_dir.join(format!("{}.managed", server_cmd));
                    let _ = std::fs::File::create(marker);
                }
                self.installed_cache.lock().unwrap().insert(server_cmd.to_string(), true);
                return Ok(());
            } else {
                return Err(format!("command exited with status: {}", status).into());
            }
        }
        Err("no install command known for this server".into())
    }

    pub fn start_client(&mut self, ext: &str, root_path: std::path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if self.failed_exts.lock().unwrap().contains(ext) { return Err("Already failed".into()); }

        let commands = self.get_server_commands(ext);
        for (cmd, args) in commands {
            // Check if this specific command is already running for this extension
            {
                let clients = self.clients.lock().unwrap();
                if let Some(ext_clients) = clients.get(ext) {
                    if ext_clients.iter().any(|(_, _, c)| c == cmd) {
                        continue;
                    }
                }
            }

            let npm_bin = Self::get_local_bin_dir().join("node_modules").join(".bin").join(cmd);
            let direct_bin = Self::get_local_bin_dir().join("bin").join(cmd);
            
            let final_cmd = if npm_bin.exists() {
                npm_bin.to_string_lossy().to_string()
            } else if direct_bin.exists() {
                direct_bin.to_string_lossy().to_string()
            } else {
                cmd.to_string()
            };

            match LspClient::start(&final_cmd, args) {
                Ok(client) => {
                    let abs_root = if root_path.is_absolute() {
                        root_path.clone()
                    } else {
                        std::env::current_dir().unwrap_or_default().join(&root_path)
                    };
                    let root_uri = match Url::from_directory_path(&abs_root) {
                        Ok(u) => u,
                        Err(_) => {
                            eprintln!("LSP: could not build root URI for {:?}", abs_root);
                            continue;
                        }
                    };
                    if let Err(e) = client.send_initialize(root_uri) {
                        eprintln!("LSP init failed for {}: {}", cmd, e);
                        continue;
                    }
                    let mut clients = self.clients.lock().unwrap();
                    clients.entry(ext.to_string()).or_default().push((client, ClientState::Starting, cmd.to_string()));
                }
                Err(e) => {
                    eprintln!("Failed to start LSP {}: {}", cmd, e);
                    // If binary not found, mark ext as failed so we don't retry every frame
                    let msg = e.to_string();
                    if msg.contains("No such file") || msg.contains("os error 2") || msg.contains("program not found") {
                        self.failed_exts.lock().unwrap().insert(ext.to_string());
                    }
                }
            }
        }
        Ok(())
    }


    fn path_to_uri(path: &Path) -> Url {
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };
        Url::from_file_path(abs_path).unwrap_or_else(|_| Url::parse("file:///").unwrap())
    }

    pub fn did_open(&self, ext: &str, path: &Path, text: String, target_cmd: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let clients_lock = self.clients.lock().unwrap();
        if let Some(clients) = clients_lock.get(ext) {
            for (client, state, cmd) in clients {
                if *state != ClientState::Ready { continue; }
                if let Some(target) = target_cmd {
                    if target != cmd { continue; }
                }
                
                let params = DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: Self::path_to_uri(path),
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
                        text: text.clone(),
                    },
                };
                let _ = client.send_notification("textDocument/didOpen", params);
            }
        }
        self.refresh_linters(ext, path, &text);
        Ok(())
    }

    pub fn did_change(&self, ext: &str, path: &Path, text: String) -> Result<(), Box<dyn std::error::Error>> {
        let clients_lock = self.clients.lock().unwrap();
        if let Some(clients) = clients_lock.get(ext) {
            let mut versions = self.versions.lock().unwrap();
            let version = versions.entry(path.to_string_lossy().to_string()).or_insert(0);
            *version += 1;

            for (client, state, _) in clients {
                if *state != ClientState::Ready { continue; }
                
                let params = DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: Self::path_to_uri(path),
                        version: *version,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: text.clone(),
                    }],
                };
                let _ = client.send_notification("textDocument/didChange", params);
            }
        }
        self.refresh_linters(ext, path, &text);
        Ok(())
    }

    pub fn did_save(&self, ext: &str, path: &Path, text: String) -> Result<(), Box<dyn std::error::Error>> {
        let clients_lock = self.clients.lock().unwrap();
        if let Some(clients) = clients_lock.get(ext) {
            for (client, state, _) in clients {
                if *state != ClientState::Ready { continue; }

                let params = DidSaveTextDocumentParams {
                    text_document: TextDocumentIdentifier {
                        uri: Self::path_to_uri(path),
                    },
                    text: None,
                };
                let _ = client.send_notification("textDocument/didSave", params);
            }
        }
        self.refresh_linters(ext, path, &text);
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

    pub fn refresh_linters(&self, ext: &str, path: &Path, text: &str) {
        match ext {
            "js" | "ts" | "jsx" | "tsx" => {
                if self.is_installed("eslint_d") {
                    // Run eslint_d in a background thread so it never blocks the UI.
                    let diagnostics = Arc::clone(&self.diagnostics);
                    let path = path.to_path_buf();
                    let text = text.to_string();
                    std::thread::spawn(move || {
                        let diags = Self::run_eslint_d_bg(&path, &text);
                        let uri = Self::path_to_uri(&path);
                        let mut diag_map = diagnostics.lock().unwrap();
                        let file_diags = diag_map.entry(uri).or_default();
                        file_diags.insert("eslint_d".to_string(), diags);
                    });
                }
            }
            _ => {}
        }
    }

    fn run_eslint_d_bg(path: &Path, text: &str) -> Vec<Diagnostic> {
        let local_bin = Self::get_local_bin_dir().join("node_modules").join(".bin").join("eslint_d");
        let cmd = if local_bin.exists() {
            local_bin.to_string_lossy().to_string()
        } else {
            "eslint_d".to_string()
        };

        let mut child = if let Ok(c) = Command::new(cmd)
            .args(&["--stdin", "--stdin-filename", path.to_str().unwrap_or(""), "--format", "json"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn() { c } else { return Vec::new(); };

        if let Some(mut stdin) = child.stdin.take() {
            let _ = std::io::Write::write_all(&mut stdin, text.as_bytes());
        }

        let output = if let Ok(o) = child.wait_with_output() { o } else { return Vec::new(); };
        if !output.status.success() && output.stdout.is_empty() { return Vec::new(); }

        let results: Vec<EslintResult> = if let Ok(r) = serde_json::from_slice(&output.stdout) { r } else { return Vec::new(); };
        
        let mut diagnostics = Vec::new();
        for result in results {
            for msg in result.messages {
                let range = Range {
                    start: lsp_types::Position {
                        line: msg.line.saturating_sub(1),
                        character: msg.column.saturating_sub(1),
                    },
                    end: lsp_types::Position {
                        line: msg.end_line.unwrap_or(msg.line).saturating_sub(1),
                        character: msg.end_column.unwrap_or(msg.column + 1).saturating_sub(1),
                    },
                };

                let severity = match msg.severity {
                    1 => Some(DiagnosticSeverity::WARNING),
                    2 => Some(DiagnosticSeverity::ERROR),
                    _ => Some(DiagnosticSeverity::INFORMATION),
                };

                diagnostics.push(Diagnostic {
                    range,
                    severity,
                    code: msg.rule_id.map(|id| NumberOrString::String(id)),
                    source: Some("eslint".to_string()),
                    message: msg.message,
                    related_information: None,
                    tags: None,
                    data: None,
                    code_description: None,
                });
            }
        }
        diagnostics
    }

    pub fn request_definition(&self, ext: &str, path: &Path, line: usize, character: usize) -> Result<i32, Box<dyn std::error::Error>> {
        let clients_lock = self.clients.lock().unwrap();
        if let Some(clients) = clients_lock.get(ext) {
            if let Some((client, _, _)) = clients.iter().find(|(_, s, _)| *s == ClientState::Ready) {
                let id = {
                    let mut counter = self.id_counter.lock().unwrap();
                    let val = *counter;
                    *counter += 1;
                    val
                };
                let params = GotoDefinitionParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier {
                            uri: Self::path_to_uri(path),
                        },
                        position: lsp_types::Position {
                            line: line as u32,
                            character: character as u32,
                        },
                    },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                };
                client.send_request(id, "textDocument/definition", params)?;
                return Ok(id);
            }
        }
        Err("No ready LSP client".into())
    }

    pub fn request_hover(&self, ext: &str, path: &Path, line: usize, character: usize) -> Result<i32, Box<dyn std::error::Error>> {
        let clients_lock = self.clients.lock().unwrap();
        if let Some(clients) = clients_lock.get(ext) {
            if let Some((client, _, _)) = clients.iter().find(|(_, s, _)| *s == ClientState::Ready) {
                let id = {
                    let mut counter = self.id_counter.lock().unwrap();
                    *counter += 1;
                    *counter - 1
                };
                let params = HoverParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier { uri: Self::path_to_uri(path) },
                        position: lsp_types::Position { line: line as u32, character: character as u32 },
                    },
                    work_done_progress_params: Default::default(),
                };
                client.send_request(id, "textDocument/hover", params)?;
                return Ok(id);
            }
        }
        Err("No ready LSP client".into())
    }

    pub fn get_clients_info(&self) -> Vec<(String, String)> {
        let clients = self.clients.lock().unwrap();
        clients.iter().flat_map(|(ext, list)| {
            list.iter().map(move |(_, state, cmd)| {
                let status = if *state == ClientState::Ready { "ready" } else { "starting" };
                (format!("{} ({})", cmd, ext), status.to_string())
            })
        }).collect()
    }

    pub fn restart_clients_for_ext(&mut self, ext: &str) {
        self.clients.lock().unwrap().remove(ext);
        self.failed_exts.lock().unwrap().remove(ext);
    }

    pub fn request_folding_ranges(&self, ext: &str, path: &Path) -> Result<i32, Box<dyn std::error::Error>> {
        let clients_lock = self.clients.lock().unwrap();
        if let Some(clients) = clients_lock.get(ext) {
            if let Some((client, _, _)) = clients.iter().find(|(_, s, _)| *s == ClientState::Ready) {
                let id = {
                    let mut counter = self.id_counter.lock().unwrap();
                    let val = *counter;
                    *counter += 1;
                    val
                };
                let params = FoldingRangeParams {
                    text_document: TextDocumentIdentifier {
                        uri: Self::path_to_uri(path),
                    },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                };
                client.send_request(id, "textDocument/foldingRange", params)?;
                return Ok(id);
            }
        }
        Err("No ready LSP client".into())
    }

    pub fn request_completions(&self, ext: &str, path: &Path, line: usize, character: usize, trigger_kind: CompletionTriggerKind, trigger_char: Option<String>) -> Result<i32, Box<dyn std::error::Error>> {
        let clients_lock = self.clients.lock().unwrap();
        if let Some(clients) = clients_lock.get(ext) {
            if let Some((client, _, _)) = clients.iter().find(|(_, s, _)| *s == ClientState::Ready) {
                let id = {
                    let mut counter = self.id_counter.lock().unwrap();
                    let val = *counter;
                    *counter += 1;
                    val
                };
                let params = CompletionParams {
                    text_document_position: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier {
                            uri: Self::path_to_uri(path),
                        },
                        position: lsp_types::Position {
                            line: line as u32,
                            character: character as u32,
                        },
                    },
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                    context: Some(CompletionContext {
                        trigger_kind,
                        trigger_character: trigger_char,
                    }),
                };
                client.send_request(id, "textDocument/completion", params)?;
                return Ok(id);
            }
        }
        Err("No ready LSP client".into())
    }
}

pub fn byte_to_utf16_offset(s: &str, byte_idx: usize) -> usize {
    let mut utf16_offset = 0;
    for (idx, c) in s.char_indices() {
        if idx >= byte_idx {
            break;
        }
        utf16_offset += c.len_utf16();
    }
    utf16_offset
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
