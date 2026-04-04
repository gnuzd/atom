pub mod config;
pub mod editor;
pub mod lsp;
pub mod ui;
pub mod vim;

use std::{env, error::Error, io, path::PathBuf, time::Duration};

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use editor::Editor;
use ui::TerminalUi;
use vim::{mode::{Mode, YankType, Focus, ExplorerInputType}, VimState, Position, LspStatus};
use ui::explorer::FileExplorer;
use lsp::{LspManager, char_to_utf16_offset};
use lsp_server::Message;

fn find_project_root(path: &PathBuf) -> PathBuf {
    let mut current = path.clone();
    if current.is_file() {
        current.pop();
    }
    while current.parent().is_some() {
        if current.join("Cargo.toml").exists() || current.join(".git").exists() || current.join("package.json").exists() {
            return current;
        }
        current.pop();
    }
    path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| env::current_dir().unwrap_or_default())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut editor = Editor::new();
    let mut vim = VimState::new();
    let ui = TerminalUi::new();
    let mut explorer = FileExplorer::new();
    let mut lsp_manager = LspManager::new();

    // Handle CLI arguments
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        editor.buffers.clear();
        editor.cursors.clear();
        for arg in &args[1..] {
            let path = PathBuf::from(arg).canonicalize().unwrap_or(PathBuf::from(arg));
            if path.is_dir() {
                explorer.root = path;
                explorer.refresh();
            } else if path.is_file() || !path.exists() {
                let _ = editor.open_file(path.clone());
            }
        }
        if editor.buffers.is_empty() {
            editor.buffers.push(editor::buffer::Buffer::new());
            editor.cursors.push(editor::cursor::Cursor::new());
        }
        editor.active_idx = 0;
    } else {
        let active_buffer = editor.buffer_mut();
        active_buffer.lines = vec![
            "Welcome to Atom IDE!".to_string(),
            "Press 'i' for Insert mode, 'v' for Visual mode.".to_string(),
            "Press '\\' to toggle/focus File Explorer.".to_string(),
            "LSP: Type std:: in a Rust file or Ctrl+Space for completion.".to_string(),
        ];
    }

    let mut flash_counter = 0;

    loop {
        // 0. Ensure LSP is active for current buffer
        if let Some(path) = editor.buffer().file_path.clone() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let has_client = lsp_manager.clients.lock().unwrap().contains_key(ext);
                let is_failed = lsp_manager.failed_exts.lock().unwrap().contains(ext);
                if !has_client && !is_failed {
                    if let Some((cmd, _)) = LspManager::get_server_command(ext) {
                        if lsp_manager.is_installed(cmd) {
                            vim.lsp_status = LspStatus::Loading;
                            let root = find_project_root(&path);
                            if let Ok(_) = lsp_manager.start_client(ext, root) {
                                // did_open is called when client becomes Ready in message loop
                            } else {
                                vim.lsp_status = LspStatus::Error("Start Fail".into());
                            }
                        } else {
                            vim.lsp_to_install = Some(cmd.to_string());
                        }
                    }
                }
            }
        }

        // 0.5 Process debounced LSP changes
        if lsp_manager.pending_change {
            if let Some(last) = lsp_manager.last_change {
                if last.elapsed() > std::time::Duration::from_millis(150) {
                    if let Some(path) = editor.buffer().file_path.clone() {
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            if lsp_manager.is_ready(ext) {
                                let _ = lsp_manager.did_change(ext, &path, editor.buffer().lines.join("\n"));
                                let (y, x) = (editor.cursor().y, editor.cursor().x);
                                let line = &editor.buffer().lines[y];
                                
                                // Check prefix
                                let mut start_x = x;
                                let chars: Vec<char> = line.chars().collect();
                                while start_x > 0 && (chars[start_x-1].is_alphanumeric() || chars[start_x-1] == '_' || chars[start_x-1] == '$') {
                                    start_x -= 1;
                                }
                                let prefix = if start_x < x { line[start_x..x].to_string() } else { String::new() };

                                if prefix.is_empty() {
                                    vim.show_suggestions = false;
                                } else {
                                    let utf16_x = char_to_utf16_offset(&editor.buffer().lines[y], editor.cursor().x);
                                    let _ = lsp_manager.request_completions(ext, &path, y, utf16_x);
                                }
                                lsp_manager.pending_change = false;
                            }
                        }
                    }
                }
            }
        }

        // 1. Process LSP messages
        let mut ready_exts = Vec::new();
        {
            let clients = lsp_manager.clients.lock().unwrap();
            for (ext, (client, state)) in clients.iter() {
                while let Ok(msg) = client.receiver().try_recv() {
                    match msg {
                        Message::Response(resp) => {
                            if resp.id == lsp_server::RequestId::from(1) {
                                let notification = lsp_server::Message::Notification(lsp_server::Notification::new("initialized".to_string(), serde_json::json!({})));
                                let _ = client.connection.sender.send(notification);
                                ready_exts.push(ext.clone());
                                vim.lsp_status = LspStatus::Ready;
                            } else if resp.id == lsp_server::RequestId::from(100) {
                                if let Some(result) = resp.result {
                                    if let Ok(completions) = serde_json::from_value::<lsp_types::CompletionResponse>(result) {
                                        match completions {
                                            lsp_types::CompletionResponse::Array(items) => {
                                                vim.suggestions = items;
                                                vim.show_suggestions = !vim.suggestions.is_empty();
                                                vim.selected_suggestion = 0;
                                                vim.suggestion_state.select(Some(0));
                                            }
                                            lsp_types::CompletionResponse::List(list) => {
                                                vim.suggestions = list.items;
                                                vim.show_suggestions = !vim.suggestions.is_empty();
                                                vim.selected_suggestion = 0;
                                                vim.suggestion_state.select(Some(0));
                                            }

                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        for ext in ready_exts {
            let mut set_ready = false;
            if let Some((_, state)) = lsp_manager.clients.lock().unwrap().get_mut(&ext) {
                *state = lsp::ClientState::Ready;
                set_ready = true;
            }
            if set_ready {
                // Send didOpen for the current buffer now that it's ready
                if let Some(path) = editor.buffer().file_path.clone() {
                    if path.extension().and_then(|s| s.to_str()) == Some(&ext) {
                        let text = editor.buffer().lines.join("\n");
                        let _ = lsp_manager.did_open(&ext, &path, text);
                    }
                }
            }
        }

        // 2. Render
        match vim.mode {
            Mode::Insert | Mode::ExplorerInput(_) => execute!(terminal.backend_mut(), SetCursorStyle::SteadyBar)?,
            _ => execute!(terminal.backend_mut(), SetCursorStyle::SteadyBlock)?,
        }
        terminal.draw(|f| ui.draw(f, &editor, &mut vim, &explorer, &lsp_manager))?;

        // 3. Handle Events
        if vim.yank_highlight_line.is_some() {
            if flash_counter > 5 { vim.yank_highlight_line = None; flash_counter = 0; }
            else { flash_counter += 1; }
        }

        if event::poll(Duration::from_millis(20))? {
            let event = event::read()?;
            if let Event::Mouse(mouse) = &event {
                match mouse.kind {
                    MouseEventKind::ScrollUp => { editor.move_up(); }
                    MouseEventKind::ScrollDown => { editor.move_down(); }
                    _ => {}
                }
            }

            if let Event::Key(key) = event {
                vim.yank_highlight_line = None;
                flash_counter = 0;

                // Global Ctrl-Space for completion
                if key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if let Some(path) = &editor.buffer().file_path {
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            let y = editor.cursor().y;
                            let utf16_x = char_to_utf16_offset(&editor.buffer().lines[y], editor.cursor().x);
                            let _ = lsp_manager.request_completions(ext, path, y, utf16_x);
                        }
                    }
                    continue;
                }

                // Handle LSP Install Prompt
                if let Some(lsp_cmd) = vim.lsp_to_install.clone() {
                    match key.code {
                        KeyCode::Char('y') => {
                            vim.lsp_status = LspStatus::Installing;
                            let lsp_cmd_clone = lsp_cmd.clone();
                            let lsp_manager_clone = lsp_manager.clone();
                            std::thread::spawn(move || {
                                let _ = lsp_manager_clone.install_server(&lsp_cmd_clone);
                            });
                            vim.lsp_to_install = None;
                            vim.lsp_status = LspStatus::None;
                        }
                        KeyCode::Char('n') | KeyCode::Esc => { vim.lsp_to_install = None; }
                        _ => {}
                    }
                    continue;
                }

                // Handle Suggestions (CMP) Navigation
                if vim.show_suggestions {
                    match key.code {
                        KeyCode::Esc => { vim.show_suggestions = false; continue; }
                        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if !vim.suggestions.is_empty() { 
                                vim.selected_suggestion = (vim.selected_suggestion + 1) % vim.suggestions.len();
                                vim.suggestion_state.select(Some(vim.selected_suggestion));
                            }
                            continue;
                        }
                        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if !vim.suggestions.is_empty() {
                                if vim.selected_suggestion == 0 { vim.selected_suggestion = vim.suggestions.len() - 1; }
                                else { vim.selected_suggestion -= 1; }
                                vim.suggestion_state.select(Some(vim.selected_suggestion));
                            }
                            continue;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            if !vim.suggestions.is_empty() { 
                                vim.selected_suggestion = (vim.selected_suggestion + 1) % vim.suggestions.len();
                                vim.suggestion_state.select(Some(vim.selected_suggestion));
                            }
                            continue;
                        }
                        KeyCode::Up => {
                            if !vim.suggestions.is_empty() {
                                if vim.selected_suggestion == 0 { vim.selected_suggestion = vim.suggestions.len() - 1; }
                                else { vim.selected_suggestion -= 1; }
                                vim.suggestion_state.select(Some(vim.selected_suggestion));
                            }
                            continue;
                        }
                        KeyCode::Enter => {
                            let (y, x) = (editor.cursor().y, editor.cursor().x);
                            let line = &editor.buffer().lines[y];
                            
                            // Calculate prefix to filter exactly like the UI does
                            let mut start_x = x;
                            let chars: Vec<char> = line.chars().collect();
                            while start_x > 0 && (chars[start_x-1].is_alphanumeric() || chars[start_x-1] == '_' || chars[start_x-1] == '$') {
                                start_x -= 1;
                            }
                            let prefix = if start_x < x { line[start_x..x].to_lowercase() } else { String::new() };

                            let mut unique_items = std::collections::HashSet::new();
                            let filtered: Vec<&lsp_types::CompletionItem> = vim.suggestions.iter()
                                .filter(|item| {
                                    let key = format!("{}:{:?}", item.label, item.kind);
                                    if unique_items.contains(&key) { return false; }
                                    if item.label.to_lowercase().contains(&prefix) {
                                        unique_items.insert(key);
                                        true
                                    } else { false }
                                })
                                .collect();

                            if let Some(item) = filtered.get(vim.selected_suggestion % filtered.len().max(1)) {
                                let insert_text = item.insert_text.as_ref().unwrap_or(&item.label);
                                let line_mut = &mut editor.buffer_mut().lines[y];
                                
                                // Replace prefix
                                for _ in start_x..x {
                                    line_mut.remove(start_x);
                                }
                                line_mut.insert_str(start_x, insert_text);
                                editor.cursor_mut().x = start_x + insert_text.len();
                            }
                            vim.show_suggestions = false;
                            continue;
                        }
                        KeyCode::Char(_) | KeyCode::Backspace => { /* Allow to fall through to Insert mode without hiding */ }
                        _ => { vim.show_suggestions = false; }
                    }
                }

                if let Mode::ExplorerInput(input_type) = vim.mode {
                    match key.code {
                        KeyCode::Esc => { vim.mode = Mode::Normal; vim.input_buffer.clear(); }
                        KeyCode::Enter => {
                            let input = vim.input_buffer.clone();
                            match input_type {
                                ExplorerInputType::Add => { let _ = explorer.create_file(&input); }
                                ExplorerInputType::Rename => { let _ = explorer.rename_selected(&input); }
                                ExplorerInputType::Move => { let _ = explorer.move_selected(PathBuf::from(&input).as_path()); }
                                ExplorerInputType::Filter => { explorer.filter = input; explorer.refresh(); }
                                ExplorerInputType::DeleteConfirm => { if input.to_lowercase() == "y" { let _ = explorer.delete_selected(); } }
                            }
                            vim.mode = Mode::Normal;
                            vim.input_buffer.clear();
                        }
                        KeyCode::Char(c) => { vim.input_buffer.push(c); if input_type == ExplorerInputType::Filter { explorer.filter = vim.input_buffer.clone(); explorer.refresh(); } }
                        KeyCode::Backspace => { vim.input_buffer.pop(); if input_type == ExplorerInputType::Filter { explorer.filter = vim.input_buffer.clone(); explorer.refresh(); } }
                        _ => {}
                    }
                    continue;
                }

                if key.code == KeyCode::Char('\\') {
                    if !explorer.visible { explorer.toggle(); vim.focus = Focus::Explorer; }
                    else if vim.focus == Focus::Editor { vim.focus = Focus::Explorer; }
                    else { explorer.toggle(); vim.focus = Focus::Editor; }
                    continue;
                }

                if vim.focus == Focus::Explorer {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => { vim.focus = Focus::Editor; }
                        KeyCode::Char('j') | KeyCode::Down => explorer.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => explorer.move_up(),
                        KeyCode::Char('h') | KeyCode::Left => explorer.collapse(),
                        KeyCode::Char('l') | KeyCode::Right => explorer.expand(),
                        KeyCode::Char('a') => { vim.mode = Mode::ExplorerInput(ExplorerInputType::Add); vim.input_buffer.clear(); }
                        KeyCode::Char('r') => {
                            vim.mode = Mode::ExplorerInput(ExplorerInputType::Rename);
                            if let Some(entry) = explorer.selected_entry() {
                                vim.input_buffer = entry.path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                            }
                        }
                        KeyCode::Char('d') => { vim.mode = Mode::ExplorerInput(ExplorerInputType::DeleteConfirm); vim.input_buffer.clear(); }
                        KeyCode::Char('m') => { vim.mode = Mode::ExplorerInput(ExplorerInputType::Move); vim.input_buffer.clear(); }
                        KeyCode::Char('/') => { vim.mode = Mode::ExplorerInput(ExplorerInputType::Filter); vim.input_buffer = explorer.filter.clone(); }
                        KeyCode::Char('Z') => { explorer.close_all(); }
                        KeyCode::Char('H') => { explorer.show_hidden = !explorer.show_hidden; explorer.refresh(); }
                        KeyCode::Char('I') => { explorer.show_ignored = !explorer.show_ignored; explorer.refresh(); }
                        KeyCode::Enter => {
                            if let Some(entry) = explorer.selected_entry() {
                                let path = entry.path.clone();
                                if entry.is_dir { explorer.toggle_expand(); }
                                else { let _ = editor.open_file(path.clone()); vim.focus = Focus::Editor; }
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                match vim.mode {
                    Mode::Keymaps => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => { vim.mode = Mode::Normal; }
                        KeyCode::Char('j') | KeyCode::Down => {
                            let i = match vim.keymap_state.selected() {
                                Some(i) => (i + 1).min(35), // Approximating item count for now, list is static
                                None => 0,
                            };
                            vim.keymap_state.select(Some(i));
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            let i = match vim.keymap_state.selected() {
                                Some(i) => i.saturating_sub(1),
                                None => 0,
                            };
                            vim.keymap_state.select(Some(i));
                        }
                        _ => {}
                    },
                    Mode::Mason => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => { vim.mode = Mode::Normal; }
                        _ => {}
                    },
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('?') => { 
                            vim.mode = Mode::Keymaps;
                            vim.keymap_state.select(Some(0));
                        }
                        KeyCode::Char('i') => { editor.buffer_mut().push_history(); vim.mode = Mode::Insert; },
                        KeyCode::Char('v') => { vim.mode = Mode::Visual; let cursor = editor.cursor(); vim.selection_start = Some(Position { x: cursor.x, y: cursor.y }); }
                        KeyCode::Char(':') => { vim.mode = Mode::Command; vim.command_buffer.clear(); }
                        KeyCode::Char('/') => { vim.mode = Mode::Search; vim.search_query.clear(); }
                        KeyCode::Char('u') => { editor.undo(); }
                        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => { editor.redo(); }
                        KeyCode::Char('w') => editor.move_word_forward(),
                        KeyCode::Char('b') => editor.move_word_backward(),
                        KeyCode::Char('e') => editor.move_word_end(),
                        KeyCode::Char('o') => { editor.open_line_below(); vim.mode = Mode::Insert; }
                        KeyCode::Char('O') => { editor.open_line_above(); vim.mode = Mode::Insert; }
                        KeyCode::Char('p') => editor.paste_after(&vim.register, vim.yank_type),
                        KeyCode::Char('P') => editor.paste_before(&vim.register, vim.yank_type),
                        KeyCode::Char('y') => {
                            if vim.pending_op == Some('y') {
                                let y = editor.cursor().y;
                                vim.register = editor.buffer().lines[y].clone();
                                vim.yank_type = YankType::Line;
                                vim.pending_op = None;
                                vim.yank_highlight_line = Some(y);
                                flash_counter = 0;
                            } else { vim.pending_op = Some('y'); }
                        }
                        KeyCode::Char('d') => {
                            if vim.pending_op == Some('d') {
                                let y = editor.cursor().y;
                                vim.register = editor.delete_line(y);
                                vim.yank_type = YankType::Line;
                                vim.pending_op = None;
                            } else { vim.pending_op = Some('d'); }
                        }
                        KeyCode::PageUp | KeyCode::Home => { vim.pending_op = None; editor.move_to_line_start(); }
                        KeyCode::PageDown | KeyCode::End => { vim.pending_op = None; editor.move_to_line_end(); }
                        KeyCode::Char('j') | KeyCode::Down => { vim.pending_op = None; editor.move_down(); },
                        KeyCode::Char('k') | KeyCode::Up => { vim.pending_op = None; editor.move_up(); },
                        KeyCode::Char('h') | KeyCode::Left => { vim.pending_op = None; editor.move_left(); },
                        KeyCode::Char('l') | KeyCode::Right => { vim.pending_op = None; editor.move_right(); },
                        _ => { vim.pending_op = None; }
                    },
                    Mode::Visual => match key.code {
                        KeyCode::Esc => { vim.mode = Mode::Normal; vim.selection_start = None; }
                        KeyCode::Char('y') => { if let Some(start) = vim.selection_start { let cursor = editor.cursor(); vim.register = editor.yank(start.x, start.y, cursor.x, cursor.y); vim.yank_type = YankType::Char; } vim.mode = Mode::Normal; vim.selection_start = None; }
                        KeyCode::Char('d') | KeyCode::Char('x') => { if let Some(start) = vim.selection_start { let cursor = editor.cursor(); vim.register = editor.delete_selection(start.x, start.y, cursor.x, cursor.y); vim.yank_type = YankType::Char; } vim.mode = Mode::Normal; vim.selection_start = None; }
                        KeyCode::Char('w') => editor.move_word_forward(),
                        KeyCode::Char('b') => editor.move_word_backward(),
                        KeyCode::Char('e') => editor.move_word_end(),
                        KeyCode::PageUp | KeyCode::Home => editor.move_to_line_start(),
                        KeyCode::PageDown | KeyCode::End => editor.move_to_line_end(),
                        KeyCode::Char('j') | KeyCode::Down => editor.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => editor.move_up(),
                        KeyCode::Char('h') | KeyCode::Left => editor.move_left(),
                        KeyCode::Char('l') | KeyCode::Right => editor.move_right(),
                        _ => {}
                    },
                    Mode::Insert => match key.code {
                        KeyCode::Esc => { vim.mode = Mode::Normal; vim.show_suggestions = false; }
                        KeyCode::Up => editor.move_up(),
                        KeyCode::Down => editor.move_down(),
                        KeyCode::Left => editor.move_left(),
                        KeyCode::Right => editor.move_right(),
                        KeyCode::PageUp | KeyCode::Home => editor.move_to_line_start(),
                        KeyCode::PageDown | KeyCode::End => editor.move_to_line_end(),
                        KeyCode::Char(c) => {
                            let (y, x) = { let cur = editor.cursor(); (cur.y, cur.x) };
                            let line = &mut editor.buffer_mut().lines[y];
                            line.insert(x, c);
                            editor.cursor_mut().x += 1;
                            
                            lsp_manager.last_change = Some(std::time::Instant::now());
                            lsp_manager.pending_change = true;
                        }
                        KeyCode::Backspace => {
                            let (y, x) = { let cur = editor.cursor(); (cur.y, cur.x) };
                            if x > 0 {
                                let line = &mut editor.buffer_mut().lines[y];
                                line.remove(x - 1);
                                editor.cursor_mut().x -= 1;
                            } else if y > 0 {
                                let current_line = editor.buffer_mut().lines.remove(y);
                                editor.cursor_mut().y -= 1;
                                let prev_y = editor.cursor().y;
                                let prev_line = &mut editor.buffer_mut().lines[prev_y];
                                let new_x = prev_line.len();
                                prev_line.push_str(&current_line);
                                editor.cursor_mut().x = new_x;
                            }
                            lsp_manager.last_change = Some(std::time::Instant::now());
                            lsp_manager.pending_change = true;
                        }
                        KeyCode::Enter => {
                            let (y, x) = { let cur = editor.cursor(); (cur.y, cur.x) };
                            let line = &mut editor.buffer_mut().lines[y];
                            let new_line = line.split_off(x);
                            editor.buffer_mut().lines.insert(y + 1, new_line);
                            editor.cursor_mut().y += 1;
                            editor.cursor_mut().x = 0;
                            vim.show_suggestions = false;
                            lsp_manager.last_change = Some(std::time::Instant::now());
                            lsp_manager.pending_change = true;
                        }
                        KeyCode::Tab => {
                            let (y, x) = { let cur = editor.cursor(); (cur.y, cur.x) };
                            let line = &mut editor.buffer_mut().lines[y];
                            line.insert_str(x, "  ");
                            editor.cursor_mut().x += 2;
                            lsp_manager.last_change = Some(std::time::Instant::now());
                            lsp_manager.pending_change = true;
                        }
                        _ => {}
                    },
                    Mode::Command => match key.code {
                        KeyCode::Esc => { vim.mode = Mode::Normal; vim.command_buffer.clear(); }
                        KeyCode::Char(c) => { vim.command_buffer.push(c); }
                        KeyCode::Backspace => { if vim.command_buffer.is_empty() { vim.mode = Mode::Normal; } else { vim.command_buffer.pop(); } }
                        KeyCode::Enter => {
                            let cmd_parts: Vec<&str> = vim.command_buffer.split_whitespace().collect();
                            if !cmd_parts.is_empty() {
                                match cmd_parts[0] {
                                    "q" | "quit" => break,
                                    "w" | "write" => { 
                                        if !vim.disable_autoformat {
                                            let format_data = if let Some(path) = editor.buffer().file_path.clone() {
                                                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_string()) {
                                                    Some((path, ext, editor.buffer().lines.join("\n")))
                                                } else { None }
                                            } else { None };

                                            if let Some((path, ext, text)) = format_data {
                                                if let Some(formatted) = lsp_manager.format_document(&ext, &path, text) {
                                                    editor.buffer_mut().lines = formatted.lines().map(|s| s.to_string()).collect();
                                                    let _ = lsp_manager.did_change(&ext, &path, editor.buffer().lines.join("\n"));
                                                }
                                            }
                                        }
                                        if cmd_parts.len() > 1 { let path = PathBuf::from(cmd_parts[1]); let _ = editor.save_file_as(path); } else { let _ = editor.save_file(); } 
                                    }
                                    "wq" => { 
                                        if !vim.disable_autoformat {
                                            let format_data = if let Some(path) = editor.buffer().file_path.clone() {
                                                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_string()) {
                                                    Some((path, ext, editor.buffer().lines.join("\n")))
                                                } else { None }
                                            } else { None };

                                            if let Some((path, ext, text)) = format_data {
                                                if let Some(formatted) = lsp_manager.format_document(&ext, &path, text) {
                                                    editor.buffer_mut().lines = formatted.lines().map(|s| s.to_string()).collect();
                                                }
                                            }
                                        }
                                        let _ = editor.save_file(); break; 
                                    }
                                    "FormatDisable" => { vim.disable_autoformat = true; }
                                    "FormatEnable" => { vim.disable_autoformat = false; }
                                    "bn" | "bnext" => editor.next_buffer(),
                                    "bp" | "bprev" => editor.prev_buffer(),
                                    "bd" | "bdelete" => editor.close_current_buffer(),
                                    "e" | "edit" => { if cmd_parts.len() > 1 { let path = PathBuf::from(cmd_parts[1]); let _ = editor.open_file(path); } }
                                    "Mason" | "mason" => { vim.mode = Mode::Mason; }
                                    _ => {}
                                }
                            }
                            vim.mode = Mode::Normal;
                            vim.command_buffer.clear();
                        }
                        _ => {}
                    },
                    Mode::Search => match key.code {
                        KeyCode::Esc => { vim.mode = Mode::Normal; vim.search_query.clear(); }
                        KeyCode::Char(c) => { vim.search_query.push(c); }
                        KeyCode::Backspace => { if vim.search_query.is_empty() { vim.mode = Mode::Normal; } else { vim.search_query.pop(); } }
                        KeyCode::Enter => { vim.mode = Mode::Normal; }
                        _ => {}
                    },
                    _ => {}
                }
            }

            let area = terminal.size()?;
            let visible_height = area.height.saturating_sub(2) as usize;
            editor.scroll_into_view(visible_height);
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture, SetCursorStyle::DefaultUserShape)?;
    terminal.show_cursor()?;
    Ok(())
}
