pub mod client;

use std::collections::HashMap;
use std::path::Path;
use crate::lsp::client::LspClient;
use lsp_types::*;
use std::process::Command;

pub struct LspManager {
    pub clients: HashMap<String, LspClient>,
}

impl LspManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    pub fn is_installed(&self, server_cmd: &str) -> bool {
        Command::new(server_cmd)
            .arg("--version")
            .output()
            .is_ok()
    }

    pub fn get_server_command(ext: &str) -> Option<(&'static str, &'static [&'static str])> {
        match ext {
            "rs" => Some(("rust-analyzer", &[])),
            "py" => Some(("pyright-langserver", &["--stdio"])),
            "js" | "ts" => Some(("typescript-language-server", &["--stdio"])),
            _ => None,
        }
    }

    pub fn start_client(&mut self, ext: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.clients.contains_key(ext) { return Ok(()); }

        if let Some((cmd, args)) = Self::get_server_command(ext) {
            let client = LspClient::start(cmd, args)?;
            let root_path = std::env::current_dir()?;
            let root_uri = Url::from_directory_path(root_path).unwrap();
            client.initialize(root_uri)?;
            self.clients.insert(ext.to_string(), client);
        }
        Ok(())
    }

    pub fn did_open(&self, ext: &str, path: &Path, text: String) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(client) = self.clients.get(ext) {
            let params = DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: Url::from_file_path(path).unwrap(),
                    language_id: match ext {
                        "rs" => "rust",
                        "py" => "python",
                        "js" => "javascript",
                        "ts" => "typescript",
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
        if let Some(client) = self.clients.get(ext) {
            let params = DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: Url::from_file_path(path).unwrap(),
                    version: 0,
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

    pub fn request_completions(&self, ext: &str, path: &Path, line: usize, character: usize) -> Result<i32, Box<dyn std::error::Error>> {
        if let Some(client) = self.clients.get(ext) {
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
