use std::process::{Child, Command, Stdio};
use std::io::{BufRead, BufReader, Write, Read};
use lsp_types::*;
use lsp_server::{Connection, Message, Request, RequestId};
use crossbeam_channel::{Sender, Receiver, unbounded};
use std::thread;

pub struct LspClient {
    pub connection: Connection,
    child: Child,
}

impl LspClient {
    pub fn start(command: &str, args: &[&str]) -> Result<Self, Box<dyn std::error::Error>> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let (writer_sender, writer_receiver): (Sender<Message>, Receiver<Message>) = unbounded();
        let (reader_sender, reader_receiver): (Sender<Message>, Receiver<Message>) = unbounded();

        // Writer Thread
        thread::spawn(move || {
            for msg in writer_receiver {
                if let Ok(json) = serde_json::to_string(&msg) {
                    let s = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
                    if stdin.write_all(s.as_bytes()).is_err() { break; }
                    if stdin.flush().is_err() { break; }
                }
            }
        });

        // Reader Thread
        let mut reader = BufReader::new(stdout);
        thread::spawn(move || {
            loop {
                let mut content_length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() || line.is_empty() { return; }
                    if line == "\r\n" || line == "\n" { break; }
                    if line.starts_with("Content-Length: ") {
                        if let Ok(len) = line.trim_start_matches("Content-Length: ").trim().parse::<usize>() {
                            content_length = len;
                        }
                    }
                }

                if content_length > 0 {
                    let mut buf = vec![0u8; content_length];
                    if reader.read_exact(&mut buf).is_ok() {
                        if let Ok(msg) = serde_json::from_slice::<Message>(&buf) {
                            let _ = reader_sender.send(msg);
                        }
                    }
                }
            }
        });

        let connection = Connection {
            sender: writer_sender,
            receiver: reader_receiver,
        };

        Ok(Self {
            connection,
            child,
        })
    }

    pub fn send_initialize(&self, root_uri: Url) -> Result<(), Box<dyn std::error::Error>> {
        let params = InitializeParams {
            root_uri: Some(root_uri),
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    completion: Some(CompletionClientCapabilities {
                        completion_item: Some(CompletionItemCapability {
                            snippet_support: Some(true),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let id = RequestId::from(1);
        let request = Request::new(id, "initialize".to_string(), params);
        self.connection.sender.send(Message::Request(request))?;
        Ok(())
    }

    pub fn send_request<P: serde::Serialize>(&self, id: i32, method: &str, params: P) -> Result<(), Box<dyn std::error::Error>> {
        let request = Request::new(RequestId::from(id), method.to_string(), params);
        self.connection.sender.send(Message::Request(request))?;
        Ok(())
    }

    pub fn send_notification<P: serde::Serialize>(&self, method: &str, params: P) -> Result<(), Box<dyn std::error::Error>> {
        let notification = lsp_server::Notification::new(method.to_string(), params);
        self.connection.sender.send(Message::Notification(notification))?;
        Ok(())
    }

    pub fn receiver(&self) -> &crossbeam_channel::Receiver<Message> {
        &self.connection.receiver
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
