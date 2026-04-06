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
use ui::trouble::{TroubleType, TroubleItem};
use lsp::{LspManager, char_to_utf16_offset};
use lsp_types::CompletionTriggerKind;
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

fn update_git_info(project_root: &PathBuf) -> Option<vim::GitInfo> {
    use std::process::Command;
    
    let branch = Command::new("git")
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_root)
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })?;

    let status = Command::new("git")
        .args(&["status", "--porcelain"])
        .current_dir(project_root)
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).to_string()) } else { None })
        .unwrap_or_default();

    let mut info = vim::GitInfo {
        branch,
        added: 0,
        modified: 0,
        removed: 0,
    };

    for line in status.lines() {
        if line.starts_with('A') || line.starts_with("??") { info.added += 1; }
        else if line.starts_with('M') || line.starts_with(" M") { info.modified += 1; }
        else if line.starts_with('D') || line.starts_with(" D") { info.removed += 1; }
    }

    Some(info)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let config = crate::config::Config::load();
    let mut editor = Editor::new(&config.colorscheme);
    let project_root = find_project_root(&env::current_dir().unwrap_or_default());
    let mut vim = VimState::new(config, project_root);
    let ui = TerminalUi::new();
    let mut explorer = FileExplorer::new();
    let mut trouble = ui::trouble::TroubleList::new();
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
        // 0. Update Message status
        if let Some(time) = vim.message_time {
            if time.elapsed().as_secs() >= 3 {
                vim.message = None;
                vim.message_time = None;
            }
        }

        // Update Git Info (every 5 seconds)
        if vim.last_git_update.is_none() || vim.last_git_update.unwrap().elapsed() > Duration::from_secs(5) {
            vim.git_info = update_git_info(&vim.project_root);
            vim.last_git_update = Some(std::time::Instant::now());
        }

        // Update LSP Status for installation
        if lsp_manager.is_any_installing() {
            vim.lsp_status = LspStatus::Installing;
        } else if let LspStatus::Installing = vim.lsp_status {
            vim.lsp_status = LspStatus::None;
        }

        // 0. Ensure LSP is active for current buffer
        if let Some(path) = editor.buffer().file_path.clone() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let is_failed = lsp_manager.failed_exts.lock().unwrap().contains(ext);
                if !is_failed {
                    let commands = lsp_manager.get_server_commands(ext);
                    for (cmd, _) in commands {
                        let is_running = {
                            let clients = lsp_manager.clients.lock().unwrap();
                            clients.get(ext).map(|cs| cs.iter().any(|(_, _, c)| c == cmd)).unwrap_or(false)
                        };

                        if !is_running {
                            if lsp_manager.is_installed(cmd) {
                                vim.lsp_status = LspStatus::Loading;
                                let root = find_project_root(&path);
                                let _ = lsp_manager.start_client(ext, root);
                            } else {
                                // Don't auto-prompt, just set a status message once
                                if vim.lsp_to_install.is_none() || vim.lsp_to_install.as_ref() != Some(&cmd.to_string()) {
                                    vim.lsp_to_install = Some(cmd.to_string());
                                    vim.set_message(format!("LSP '{}' not found. Use :Mason to install.", cmd));
                                }
                            }
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
                                    // Only hide if the character before cursor is NOT a trigger character
                                    let is_trigger = x > 0 && {
                                        let c = chars[x-1];
                                        c == '.' || c == ':'
                                    };
                                    if !is_trigger {
                                        vim.show_suggestions = false;
                                    }
                                } else {
                                    let text = editor.buffer().lines.join("\n");
                                    let _ = lsp_manager.did_change(ext, &path, text);
                                    let utf16_x = char_to_utf16_offset(&editor.buffer().lines[y], editor.cursor().x);
                                    let _ = lsp_manager.request_completions(ext, &path, y, utf16_x, CompletionTriggerKind::INVOKED, None);
                                }

                                lsp_manager.pending_change = false;
                            }
                        }
                    }
                }
            }
        }

        // 1. Process LSP messages
        let mut ready_clients = Vec::new();
        {
            let mut clients_lock = lsp_manager.clients.lock().unwrap();
            for (ext, clients) in clients_lock.iter_mut() {
                for (client, state, cmd) in clients {
                    while let Ok(msg) = client.receiver().try_recv() {
                        match msg {
                            Message::Notification(notif) => {
                                if notif.method == "textDocument/publishDiagnostics" {
                                    if let Ok(params) = serde_json::from_value::<lsp_types::PublishDiagnosticsParams>(notif.params) {
                                        let mut diagnostics = lsp_manager.diagnostics.lock().unwrap();
                                        let file_diags = diagnostics.entry(params.uri).or_default();
                                        file_diags.insert(cmd.clone(), params.diagnostics);
                                    }
                                }
                            }
                            Message::Response(resp) => {
                                if resp.id == lsp_server::RequestId::from(1) {
                                    let notification = lsp_server::Message::Notification(lsp_server::Notification::new("initialized".to_string(), serde_json::json!({})));
                                    let _ = client.connection.sender.send(notification);
                                    *state = lsp::ClientState::Ready;
                                    ready_clients.push((ext.clone(), cmd.clone()));
                                    vim.lsp_status = LspStatus::Ready;
                                } else {
// ... existing completion logic
                                    // Extract numeric ID for comparison
                                    let id_val = resp.id.to_string().parse::<i32>().unwrap_or(0);
                                    if id_val >= 100 && id_val >= vim.last_lsp_id {
                                        vim.last_lsp_id = id_val;

                                        if let Some(result) = resp.result {
                                            if let Ok(completions) = serde_json::from_value::<lsp_types::CompletionResponse>(result.clone()) {
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
                                            } else if let Ok(ranges) = serde_json::from_value::<Vec<lsp_types::FoldingRange>>(result.clone()) {
                                                vim.folding_ranges = ranges;
                                            } else if let Ok(definition) = serde_json::from_value::<lsp_types::GotoDefinitionResponse>(result) {
                                                let location = match definition {
                                                    lsp_types::GotoDefinitionResponse::Scalar(l) => Some(l),
                                                    lsp_types::GotoDefinitionResponse::Array(a) => a.into_iter().next(),
                                                    lsp_types::GotoDefinitionResponse::Link(links) => links.into_iter().next().map(|link| lsp_types::Location {
                                                        uri: link.target_uri,
                                                        range: link.target_range,
                                                    }),
                                                };

                                                if let Some(loc) = location {
                                                    if let Ok(path) = loc.uri.to_file_path() {
                                                        let mut found = false;
                                                        for i in 0..editor.buffers.len() {
                                                            if editor.buffers[i].file_path.as_ref() == Some(&path) {
                                                                editor.active_idx = i;
                                                                found = true;
                                                                break;
                                                            }
                                                        }
                                                        if !found {
                                                            let _ = editor.open_file(path.clone());
                                                        }
                                                        
                                                        editor.cursor_mut().y = loc.range.start.line as usize;
                                                        editor.cursor_mut().x = loc.range.start.character as usize; // Simplified
                                                        
                                                        // Trigger LSP open for the new buffer
                                                        if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                            let text = editor.buffer().lines.join("\n");
                                                            let _ = lsp_manager.did_open(&ext, &path, text, None);
                                                            let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                                        }
                                                    }
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
        }
        for (ext, cmd) in ready_clients {
            // Send didOpen for the current buffer now that it's ready
            if let Some(path) = editor.buffer().file_path.clone() {
                if path.extension().and_then(|s| s.to_str()) == Some(&ext) {
                    let text = editor.buffer().lines.join("\n");
                    let _ = lsp_manager.did_open(&ext, &path, text, Some(&cmd));
                    let _ = lsp_manager.request_folding_ranges(&ext, &path);
                }
            }
        }

        // 2. Render
        match vim.mode {
            Mode::Insert | Mode::ExplorerInput(_) => execute!(terminal.backend_mut(), SetCursorStyle::SteadyBar)?,
            _ => execute!(terminal.backend_mut(), SetCursorStyle::SteadyBlock)?,
        }

        // Update trouble list if visible
        if trouble.visible {
            let todos: Vec<TroubleItem> = if !trouble.scanned {
                let project_root = explorer.root.clone();
                trouble.scanned = true;
                crate::editor::todo::scan_project_todos(&project_root)
            } else {
                let mut current_todos: Vec<TroubleItem> = trouble.items.iter()
                    .filter(|item| matches!(item.item_type, TroubleType::Todo))
                    .cloned()
                    .collect();
                
                if let Some(path) = &editor.buffer().file_path {
                    let file_todos = crate::editor::todo::scan_todos(path, &editor.buffer().lines);
                    current_todos.retain(|item| item.path != *path);
                    current_todos.extend(file_todos);
                }
                current_todos
            };
            trouble.update_from_lsp(&lsp_manager.diagnostics.lock().unwrap(), todos);
        }

        terminal.draw(|f| ui.draw(f, &editor, &mut vim, &explorer, &trouble, &lsp_manager))?;

        // 3. Handle Events
        if vim.yank_highlight_line.is_some() {
            if flash_counter > 5 { vim.yank_highlight_line = None; flash_counter = 0; }
            else { flash_counter += 1; }
        }

        if event::poll(Duration::from_millis(20))? {
            let event = event::read()?;
            if let Event::Mouse(mouse) = &event {
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        if let Mode::Telescope(_) = vim.mode {
                            vim.telescope.scroll_preview_up(3);
                        } else {
                            editor.move_up();
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if let Mode::Telescope(_) = vim.mode {
                            vim.telescope.scroll_preview_down(3);
                        } else {
                            editor.move_down();
                        }
                    }
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
                            let utf16_x = crate::lsp::char_to_utf16_offset(&editor.buffer().lines[y], editor.cursor().x);
                            let _ = lsp_manager.request_completions(ext, path, y, utf16_x, CompletionTriggerKind::INVOKED, None);
                        }
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
                                        let mut insert_text = item.insert_text.as_ref().unwrap_or(&item.label).clone();

                                        // Fix "double dot" issue: if we have a dot before cursor and completion starts with dot, strip it
                                        if insert_text.starts_with('.') && start_x > 0 && chars[start_x-1] == '.' {
                                        insert_text.remove(0);
                                        }

                                        let line_mut = &mut editor.buffer_mut().lines[y];

                                        // Replace prefix
                                        for _ in start_x..x {
                                        line_mut.remove(start_x);
                                        }
                                        line_mut.insert_str(start_x, &insert_text);
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

                if vim.focus == Focus::Trouble {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => { 
                            trouble.toggle();
                            vim.focus = Focus::Editor; 
                        }
                        KeyCode::Char('j') | KeyCode::Down => trouble.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => trouble.move_up(),
                        KeyCode::Enter => {
                            if let Some(item) = trouble.selected_item() {
                                let path = item.path.clone();
                                let line = item.line;
                                let col = item.col;
                                
                                let mut found = false;
                                for i in 0..editor.buffers.len() {
                                    if editor.buffers[i].file_path.as_ref() == Some(&path) {
                                        editor.active_idx = i;
                                        found = true;
                                        break;
                                    }
                                }
                                if !found {
                                    let _ = editor.open_file(path.clone());
                                }
                                
                                editor.cursor_mut().y = line;
                                editor.cursor_mut().x = col;
                                vim.focus = Focus::Editor;
                            }
                        }
                        _ => {}
                    }
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
                        KeyCode::Char('o') => { explorer.open_in_system_explorer(); }
                        KeyCode::Char('/') => { vim.mode = Mode::ExplorerInput(ExplorerInputType::Filter); vim.input_buffer = explorer.filter.clone(); }
                        KeyCode::Char('Z') => { explorer.close_all(); }
                        KeyCode::Char('H') => { explorer.show_hidden = !explorer.show_hidden; explorer.refresh(); }
                        KeyCode::Char('I') => { explorer.show_ignored = !explorer.show_ignored; explorer.refresh(); }
                        KeyCode::Enter => {
                            if let Some(entry) = explorer.selected_entry() {
                                let path = entry.path.clone();
                                if entry.is_dir { explorer.toggle_expand(); }
                                else { 
                                    if editor.open_file(path.clone()).is_ok() {
                                        vim.focus = Focus::Editor; 
                                        vim.set_message(format!("Opened \"{}\"", path.display()));
                                        if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                            let text = editor.buffer().lines.join("\n");
                                            let _ = lsp_manager.did_open(&ext, &path, text, None);
                                        let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                    continue;
                }

                let format_buffer = |editor: &mut crate::editor::Editor, lsp_manager: &crate::lsp::LspManager, vim: &mut crate::vim::VimState, terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, ui: &crate::ui::TerminalUi, explorer: &crate::ui::explorer::FileExplorer, trouble: &crate::ui::trouble::TroubleList| -> Result<(), String> {
                    if let Some(path) = editor.buffer().file_path.clone() {
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                            vim.lsp_status = LspStatus::Formatting;
                            // Draw immediately to show "Formatting..."
                            let _ = terminal.draw(|f| ui.draw(f, editor, vim, explorer, trouble, lsp_manager));
                            
                            let text = editor.buffer().lines.join("\n");
                            match lsp_manager.format_document(&ext, &path, text) {
                                Some(Ok(formatted)) => {
                                    editor.buffer_mut().lines = formatted.lines().map(|s| s.to_string()).collect();
                                    editor.clamp_cursor();
                                    let _ = lsp_manager.did_change(&ext, &path, editor.buffer().lines.join("\n"));
                                    vim.lsp_status = LspStatus::None;
                                    return Ok(());
                                }
                                Some(Err(e)) => {
                                    vim.lsp_status = LspStatus::None;
                                    return Err(e);
                                }
                                None => {
                                    vim.lsp_status = LspStatus::None;
                                    return Err("No formatter available for this file type".to_string());
                                }
                            }
                        }
                    }
                    vim.lsp_status = LspStatus::None;
                    Ok(())
                };

                let save_and_format = |editor: &mut crate::editor::Editor, lsp_manager: &crate::lsp::LspManager, vim: &mut crate::vim::VimState, terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>, ui: &crate::ui::TerminalUi, explorer: &crate::ui::explorer::FileExplorer, trouble: &crate::ui::trouble::TroubleList, path_to_save: Option<PathBuf>| {
                    let mut format_info = String::new();
                    if !vim.config.disable_autoformat {
                        if let Err(e) = format_buffer(editor, lsp_manager, vim, terminal, ui, explorer, trouble) {
                            if e != "No formatter available for this file type" {
                                vim.set_message(format!("Error: {}", e));
                            }
                        } else {
                            format_info.push_str("formatted, ");
                        }
                    }
                    let res = if let Some(path) = path_to_save {
                        editor.save_file_as(path.clone()).map(|_| path.to_string_lossy().to_string())
                    } else {
                        editor.save_file().map(|_| editor.buffer().file_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default())
                    };
                    if let Ok(path_str) = res {
                        let line_count = editor.buffer().lines.len();
                        let char_count: usize = editor.buffer().lines.iter().map(|l| l.len() + 1).sum();
                        vim.set_message(format!("\"{}\" {} {}L, {}C written", path_str, format_info, line_count, char_count));
                        
                        if let Some(path) = editor.buffer().file_path.clone() {
                            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                let text = editor.buffer().lines.join("\n");
                                let _ = lsp_manager.did_save(&ext, &path, text);
                            }
                        }
                    } else {
                        vim.set_message("Error: Could not save file".to_string());
                    }
                };

                let toggle_comment = |editor: &mut crate::editor::Editor, vim: &mut crate::vim::VimState| {
                    let path = editor.buffer().file_path.clone();
                    let ext = path.as_ref().and_then(|p| p.extension()).and_then(|s| s.to_str()).unwrap_or("rs");
                    let comment_prefix = match ext {
                        "rs" | "js" | "ts" | "c" | "cpp" | "java" | "go" | "svelte" => "// ",
                        "py" | "rb" | "sh" | "yaml" | "yml" | "toml" | "dockerfile" => "# ",
                        "html" | "xml" => "<!-- ",
                        "css" => "/* ",
                        _ => "// ",
                    };
                    let comment_suffix = match ext {
                        "html" | "xml" => " -->",
                        "css" => " */",
                        _ => "",
                    };

                    editor.buffer_mut().push_history();
                    let (s_y, e_y) = if let Mode::Visual = vim.mode {
                        let start = vim.selection_start.unwrap();
                        let cur = editor.cursor();
                        if start.y < cur.y { (start.y, cur.y) } else { (cur.y, start.y) }
                    } else {
                        (editor.cursor().y, editor.cursor().y)
                    };

                    let all_commented = (s_y..=e_y).all(|y| {
                        let line = &editor.buffer().lines[y];
                        line.trim().is_empty() || line.trim().starts_with(comment_prefix)
                    });

                    for y in s_y..=e_y {
                        let line = &mut editor.buffer_mut().lines[y];
                        if line.trim().is_empty() { continue; }
                        if all_commented {
                            // Uncomment
                            if let Some(pos) = line.find(comment_prefix) {
                                line.replace_range(pos..pos + comment_prefix.len(), "");
                            }
                            if !comment_suffix.is_empty() {
                                if let Some(pos) = line.rfind(comment_suffix) {
                                    line.replace_range(pos..pos + comment_suffix.len(), "");
                                }
                            }
                        } else {
                            // Comment
                            let indent = line.chars().take_while(|c| c.is_whitespace()).count();
                            line.insert_str(indent, comment_prefix);
                            line.push_str(comment_suffix);
                        }
                    }
                };

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
                    Mode::Telescope(_) => match key.code {
                        KeyCode::Esc => { vim.telescope.close(); vim.mode = Mode::Normal; }
                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            vim.telescope.scroll_preview_up(10);
                        }
                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            vim.telescope.scroll_preview_down(10);
                        }
                        KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => { vim.telescope.move_down(); }
                        KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => { vim.telescope.move_up(); }
                        KeyCode::Enter => {
                            if let Some(result) = vim.telescope.results.get(vim.telescope.selected_idx) {
                                match vim.telescope.kind {
                                    vim::mode::TelescopeKind::Themes => {
                                        let new_theme_name = result.path.to_string_lossy().to_string();
                                        editor.highlighter.theme = crate::ui::colorscheme::ColorScheme::new(&new_theme_name);
                                        vim.config.colorscheme = new_theme_name;
                                        let _ = vim.config.save();
                                    }
                                    vim::mode::TelescopeKind::Buffers => {
                                        if let Some(idx) = result.buffer_idx {
                                            if idx < editor.buffers.len() {
                                                editor.active_idx = idx;
                                            }
                                        }
                                    }
                                    _ => {
                                        let path = result.path.clone();
                                        let mut found = false;
                                        for i in 0..editor.buffers.len() {
                                            if editor.buffers[i].file_path.as_ref() == Some(&path) {
                                                editor.active_idx = i;
                                                found = true;
                                                break;
                                            }
                                        }
                                        if !found {
                                            let _ = editor.open_file(path);
                                        }
                                        if let Some(line) = result.line_number {
                                            editor.cursor_mut().y = line.saturating_sub(1);
                                            editor.cursor_mut().x = 0;
                                        }
                                    }
                                }
                                // Common post-select actions
                                if let Some(path) = editor.buffer().file_path.clone() {
                                    if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                        let text = editor.buffer().lines.join("\n");
                                        let _ = lsp_manager.did_open(&ext, &path, text, None);
                                        let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                    }
                                }
                                vim.focus = Focus::Editor;
                            }
                            vim.telescope.close();
                            vim.mode = Mode::Normal;
                        }
                        KeyCode::Char(c) => {
                            vim.telescope.query.push(c);
                            vim.telescope.update_results(&editor);
                        }
                        KeyCode::Backspace => {
                            vim.telescope.query.pop();
                            vim.telescope.update_results(&editor);
                        }
                        _ => {}
                    },
                    Mode::Mason => {
                        let filtered_packages: Vec<_> = crate::lsp::PACKAGES.iter()
                            .filter(|p| {
                                let matches_tab = match vim.mason_tab {
                                    0 => true,
                                    1 => p.kind == crate::lsp::PackageKind::Lsp,
                                    2 => p.kind == crate::lsp::PackageKind::Dap,
                                    3 => p.kind == crate::lsp::PackageKind::Linter,
                                    4 => p.kind == crate::lsp::PackageKind::Formatter,
                                    _ => true,
                                };
                                let matches_filter = p.name.to_lowercase().contains(&vim.mason_filter.to_lowercase()) || 
                                                   p.description.to_lowercase().contains(&vim.mason_filter.to_lowercase());
                                matches_tab && matches_filter
                            })
                            .collect();
                        
                        let (mut installed, mut available): (Vec<_>, Vec<_>) = filtered_packages.into_iter().partition(|p| lsp_manager.is_managed(p.cmd));
                        installed.sort_by_key(|p| p.name);
                        available.sort_by_key(|p| p.name);
                        let list_len = 3 + installed.len() + available.len();

                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') => { vim.mode = Mode::Normal; }
                            KeyCode::Char('j') | KeyCode::Down => {
                                let i = match vim.mason_state.selected() {
                                    Some(i) => (i + 1).min(list_len.saturating_sub(1)),
                                    None => 0,
                                };
                                vim.mason_state.select(Some(i));
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                let i = match vim.mason_state.selected() {
                                    Some(i) => i.saturating_sub(1),
                                    None => 0,
                                };
                                vim.mason_state.select(Some(i));
                            }
                            KeyCode::Char('1') => { vim.mason_tab = 0; vim.mason_state.select(Some(0)); }
                            KeyCode::Char('2') => { vim.mason_tab = 1; vim.mason_state.select(Some(0)); }
                            KeyCode::Char('3') => { vim.mason_tab = 2; vim.mason_state.select(Some(0)); }
                            KeyCode::Char('4') => { vim.mason_tab = 3; vim.mason_state.select(Some(0)); }
                            KeyCode::Char('5') => { vim.mason_tab = 4; vim.mason_state.select(Some(0)); }
                            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                vim.mode = Mode::MasonFilter;
                            }
                            KeyCode::Char('i') | KeyCode::Enter => {
                                if let Some(idx) = vim.mason_state.selected() {
                                    let mut selected_package = None;
                                    if idx >= 1 && idx < 1 + installed.len() {
                                        selected_package = Some(installed[idx - 1]);
                                    } else if idx >= 3 + installed.len() && idx < 3 + installed.len() + available.len() {
                                        selected_package = Some(available[idx - (3 + installed.len())]);
                                    }
                                    
                                    if let Some(pkg) = selected_package {
                                        let lsp_manager_clone = lsp_manager.clone();
                                        let cmd = pkg.cmd.to_string();
                                        // Use a sender or just set a flag if we had a shared state for messages
                                        // For now, the main loop will see is_any_installing()
                                        std::thread::spawn(move || {
                                            if let Ok(_) = lsp_manager_clone.install_server(&cmd) {
                                                // We can't easily set vim.message from here without Mutex, 
                                                // but is_any_installing will handle the status line.
                                            }
                                        });
                                    }
                                }
                            }
                            _ => {}
                        }
                    },
                    Mode::MasonFilter => match key.code {
                        KeyCode::Esc | KeyCode::Enter => { vim.mode = Mode::Mason; }
                        KeyCode::Backspace => { vim.mason_filter.pop(); vim.mason_state.select(Some(0)); }
                        KeyCode::Char(c) => { vim.mason_filter.push(c); vim.mason_state.select(Some(0)); }
                        _ => {}
                    },
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => {
                            if vim.pending_op == Some('?') {
                                vim.mode = Mode::Keymaps;
                                vim.keymap_state.select(Some(0));
                                vim.pending_op = None;
                            }
                        }
                        KeyCode::Char('?') => { 
                            vim.mode = Mode::Keymaps;
                            vim.keymap_state.select(Some(0));
                        }
                        KeyCode::Char('g') if vim.pending_op.is_none() => { vim.pending_op = Some('g'); }
                        KeyCode::Char('c') if vim.pending_op == Some('g') => { vim.pending_op = Some('c'); }
                        KeyCode::Char('c') if vim.pending_op == Some('c') => {
                            toggle_comment(&mut editor, &mut vim);
                            vim.pending_op = None;
                        }
                        KeyCode::Char('/') if vim.pending_op == Some(' ') => {
                            toggle_comment(&mut editor, &mut vim);
                            vim.pending_op = None;
                        }
                        KeyCode::Char('n') if vim.pending_op == Some(' ') => {
                            vim.relative_number = !vim.relative_number;
                            vim.pending_op = None;
                        }
                        KeyCode::Char('i') => { editor.buffer_mut().push_history(); vim.mode = Mode::Insert; },
                        KeyCode::Char('v') => { vim.mode = Mode::Visual; let cursor = editor.cursor(); vim.selection_start = Some(Position { x: cursor.x, y: cursor.y }); }
                        KeyCode::Char(':') => { vim.mode = Mode::Command; vim.command_buffer.clear(); }
                        KeyCode::Char('/') => { vim.mode = Mode::Search; vim.search_query.clear(); }
                        KeyCode::Char('u') => { editor.undo(); }
                        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => { editor.redo(); }
                        KeyCode::Char(']') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Some(path) = &editor.buffer().file_path {
                                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                                    let (y, x) = (editor.cursor().y, editor.cursor().x);
                                    let utf16_x = crate::lsp::char_to_utf16_offset(&editor.buffer().lines[y], x);
                                    let _ = lsp_manager.request_definition(ext, path, y, utf16_x);
                                }
                            }
                        }
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => { save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None); }
                        KeyCode::Tab => {
                            if vim.focus != Focus::Explorer {
                                editor.next_buffer();
                                if let Some(path) = editor.buffer().file_path.clone() {
                                    if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                        let text = editor.buffer().lines.join("\n");
                                        let _ = lsp_manager.did_open(&ext, &path, text, None);
                                        let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                    }
                                }
                                vim.set_message(format!("Buffer {}/{}", editor.active_idx + 1, editor.buffers.len()));
                            }
                        }
                        KeyCode::BackTab => {
                            if vim.focus != Focus::Explorer {
                                editor.prev_buffer();
                                if let Some(path) = editor.buffer().file_path.clone() {
                                    if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                        let text = editor.buffer().lines.join("\n");
                                        let _ = lsp_manager.did_open(&ext, &path, text, None);
                                        let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                    }
                                }
                                vim.set_message(format!("Buffer {}/{}", editor.active_idx + 1, editor.buffers.len()));
                            }
                        }
                        KeyCode::Char(' ') => { vim.pending_op = Some(' '); }
                        KeyCode::Char('x') if vim.pending_op == Some(' ') => {
                            vim.pending_op = None;
                            if editor.buffer().modified {
                                vim.mode = Mode::Confirm(vim::mode::ConfirmAction::CloseBuffer);
                            } else {
                                editor.close_current_buffer();
                            }
                        }
                        KeyCode::Char('b') if vim.pending_op == Some(' ') => {
                            vim.config.disable_autoformat = !vim.config.disable_autoformat;
                            let _ = vim.config.save();
                            vim.set_message(if vim.config.disable_autoformat { "Autoformat Disabled".to_string() } else { "Autoformat Enabled".to_string() });
                            vim.pending_op = None;
                        }
                        KeyCode::Char('t') if vim.pending_op == Some(' ') => {
                            vim.pending_op = Some('t');
                        }
                        KeyCode::Char('f') if vim.pending_op == Some(' ') => {
                            vim.pending_op = Some('f');
                        }
                        KeyCode::Char('f') if vim.pending_op == Some('f') => {
                            vim.telescope.open(vim::mode::TelescopeKind::Files, explorer.root.clone(), &editor);
                            vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Files);
                            vim.pending_op = None;
                        }
                        KeyCode::Char('g') if vim.pending_op == Some('f') => {
                            vim.telescope.open(vim::mode::TelescopeKind::Words, explorer.root.clone(), &editor);
                            vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Words);
                            vim.pending_op = None;
                        }
                        KeyCode::Char('b') if vim.pending_op == Some('f') => {
                            vim.telescope.open(vim::mode::TelescopeKind::Buffers, explorer.root.clone(), &editor);
                            vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Buffers);
                            vim.pending_op = None;
                        }
                        KeyCode::Char('h') if vim.pending_op == Some('t') => {
                            vim.telescope.open(vim::mode::TelescopeKind::Themes, explorer.root.clone(), &editor);
                            vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Themes);
                            vim.pending_op = None;
                        }
                        KeyCode::Char('t') if vim.pending_op == Some('t') => {
                            if !trouble.visible {
                                trouble.toggle();
                                vim.focus = Focus::Trouble;
                            } else if vim.focus == Focus::Trouble {
                                trouble.toggle();
                                vim.focus = Focus::Editor;
                            } else {
                                vim.focus = Focus::Trouble;
                            }
                            vim.pending_op = None;
                        }
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
                        KeyCode::Char('z') if vim.pending_op.is_none() => { vim.pending_op = Some('z'); }
                        KeyCode::Char('c') if vim.pending_op == Some('z') => { editor.toggle_fold(&vim.folding_ranges); vim.pending_op = None; }
                        KeyCode::Char('a') if vim.pending_op == Some('z') => { editor.toggle_fold(&vim.folding_ranges); vim.pending_op = None; }
                        KeyCode::Char(c) if c.is_ascii_digit() && vim.pending_op.is_none() => {
                            let digit = c.to_digit(10).unwrap() as usize;
                            if let Some(count) = vim.count {
                                vim.count = Some(count * 10 + digit);
                            } else if digit > 0 {
                                vim.count = Some(digit);
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            let count = vim.count.unwrap_or(1);
                            for _ in 0..count { editor.move_down(); }
                            vim.count = None;
                            vim.pending_op = None;
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            let count = vim.count.unwrap_or(1);
                            for _ in 0..count { editor.move_up(); }
                            vim.count = None;
                            vim.pending_op = None;
                        }
                        KeyCode::Char('h') | KeyCode::Left => { vim.pending_op = None; editor.move_left(); },
                        KeyCode::Char('l') | KeyCode::Right => { vim.pending_op = None; editor.move_right(); },
                        _ => { vim.pending_op = None; }
                    },
                    Mode::Visual => match key.code {
                        KeyCode::Esc => { vim.mode = Mode::Normal; vim.selection_start = None; }
                        KeyCode::Char('g') if vim.pending_op.is_none() => { vim.pending_op = Some('g'); }
                        KeyCode::Char('c') if vim.pending_op == Some('g') => {
                            toggle_comment(&mut editor, &mut vim);
                            vim.mode = Mode::Normal;
                            vim.selection_start = None;
                            vim.pending_op = None;
                        }
                        KeyCode::Char('/') if vim.pending_op == Some(' ') => {
                            toggle_comment(&mut editor, &mut vim);
                            vim.mode = Mode::Normal;
                            vim.selection_start = None;
                            vim.pending_op = None;
                        }
                        KeyCode::Char(' ') => { vim.pending_op = Some(' '); }
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
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => { save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None); }
                        KeyCode::Char(c) => {
                            let (y, x) = { let cur = editor.cursor(); (cur.y, cur.x) };
                            let new_x = x + 1;
                            
                            // Auto-close pairs
                            let pair = match c {
                                '(' => Some(')'),
                                '[' => Some(']'),
                                '{' => Some('}'),
                                '"' => Some('"'),
                                '\'' => Some('\''),
                                _ => None,
                            };

                            // Auto-close tags (HTML/JSX/Svelte)
                            let tag_to_close = if c == '>' {
                                let line = &editor.buffer().lines[y];
                                let prefix = &line[..x];
                                if let Some(start_pos) = prefix.rfind('<') {
                                    let tag_content = &prefix[start_pos + 1..];
                                    if !tag_content.starts_with('/') && !tag_content.is_empty() && !tag_content.contains(' ') {
                                        Some(format!("</{}>", tag_content))
                                    } else { None }
                                } else { None }
                            } else { None };

                            {
                                let line = &mut editor.buffer_mut().lines[y];
                                line.insert(x, c);
                                if let Some(close) = pair {
                                    line.insert(new_x, close);
                                }
                                if let Some(close_tag) = tag_to_close {
                                    line.insert_str(new_x, &close_tag);
                                }
                            }
                            
                            editor.cursor_mut().x = new_x;
                            lsp_manager.last_change = Some(std::time::Instant::now());
                            lsp_manager.pending_change = true;

                            // Auto-trigger completion on certain characters
                            if c == '.' || c == ':' {
                                vim.suggestions.clear(); // Clear old irrelevant suggestions
                                vim.show_suggestions = true; // Keep menu open or prepare to show it
                                if let Some(path) = &editor.buffer().file_path {
                                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                                        let text = editor.buffer().lines.join("\n");
                                        let _ = lsp_manager.did_change(ext, path, text);
                                        let utf16_x = crate::lsp::char_to_utf16_offset(&editor.buffer().lines[y], editor.cursor().x);
                                        let _ = lsp_manager.request_completions(ext, path, y, utf16_x, CompletionTriggerKind::TRIGGER_CHARACTER, Some(c.to_string()));
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            let (y, x) = { let cur = editor.cursor(); (cur.y, cur.x) };
                            if x > 0 {
                                let line = &mut editor.buffer_mut().lines[y];
                                line.remove(x - 1);
                                editor.cursor_mut().x -= 1;
                                
                                // Auto-trigger completion on backspace if we are in a word
                                if let Some(path) = &editor.buffer().file_path {
                                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                                        let new_x = editor.cursor().x;
                                        let line = &editor.buffer().lines[y];
                                        let chars: Vec<char> = line.chars().collect();
                                        if new_x > 0 && (chars[new_x-1].is_alphanumeric() || chars[new_x-1] == '_' || chars[new_x-1] == '$' || chars[new_x-1] == '.' || chars[new_x-1] == ':') {
                                            let utf16_x = crate::lsp::char_to_utf16_offset(line, new_x);
                                            let _ = lsp_manager.request_completions(ext, path, y, utf16_x, CompletionTriggerKind::INVOKED, None);
                                        }
                                    }
                                }
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
                            let cmd_parts: Vec<String> = vim.command_buffer.split_whitespace().map(|s| s.to_string()).collect();
                            if !cmd_parts.is_empty() {
                                // Check if command is a number (line jump)
                                if let Ok(line_num) = cmd_parts[0].parse::<usize>() {
                                    editor.cursor_mut().y = line_num.saturating_sub(1).min(editor.buffer().lines.len().saturating_sub(1));
                                    editor.cursor_mut().x = 0;
                                    vim.focus = Focus::Editor;
                                    vim.mode = Mode::Normal;
                                    vim.command_buffer.clear();
                                    continue;
                                }

                                match cmd_parts[0].as_str() {
                                    "q" | "quit" => {
                                        let modified = editor.buffer().modified;
                                        if modified {
                                            vim.mode = Mode::Confirm(vim::mode::ConfirmAction::CloseBuffer);
                                        } else {
                                            if editor.buffers.len() > 1 {
                                                editor.close_current_buffer();
                                                vim.mode = Mode::Normal;
                                            } else {
                                                break;
                                            }
                                        }
                                    }
                                    "q!" => {
                                        if editor.buffers.len() > 1 {
                                            editor.close_current_buffer();
                                            vim.mode = Mode::Normal;
                                        } else {
                                            break;
                                        }
                                    }
                                    "qa" | "quitall" => {
                                        let modified = editor.buffers.iter().any(|b| b.modified);
                                        if modified {
                                            vim.mode = Mode::Confirm(vim::mode::ConfirmAction::Quit);
                                        } else {
                                            break;
                                        }
                                    }
                                    "qa!" => break,
                                    "w" | "write" => { 
                                        let path_to_save = if cmd_parts.len() > 1 { Some(PathBuf::from(&cmd_parts[1])) } else { None };
                                        save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, path_to_save);
                                    }
                                    "wq" => { 
                                        save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None);
                                        let modified = editor.buffers.iter().any(|b| b.modified);
                                        if !modified {
                                            break;
                                        } else {
                                            vim.set_message("Error: Could not save all buffers".to_string());
                                        }
                                    }
                                    "Format" | "format" => {
                                        match format_buffer(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble) {
                                            Ok(_) => vim.set_message("Formatted".to_string()),
                                            Err(e) => vim.set_message(e),
                                        }
                                    }
                                    "FormatAll" | "formatall" => {
                                        vim.lsp_status = LspStatus::Formatting;
                                        let _ = terminal.draw(|f| ui.draw(f, &editor, &mut vim, &explorer, &trouble, &lsp_manager));
                                        
                                        let mut count = 0;
                                        let original_idx = editor.active_idx;
                                        let total = editor.buffers.len();
                                        
                                        for i in 0..total {
                                            editor.active_idx = i;
                                            if let Some(path) = editor.buffer().file_path.clone() {
                                                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                    let text = editor.buffer().lines.join("\n");
                                                    if let Some(Ok(formatted)) = lsp_manager.format_document(&ext, &path, text) {
                                                        editor.buffer_mut().lines = formatted.lines().map(|s| s.to_string()).collect();
                                                        editor.clamp_cursor();
                                                        let _ = lsp_manager.did_change(&ext, &path, editor.buffer().lines.join("\n"));
                                                        count += 1;
                                                    }
                                                }
                                            }
                                        }
                                        
                                        editor.active_idx = original_idx;
                                        vim.lsp_status = LspStatus::None;
                                        vim.set_message(format!("Formatted {}/{} buffers", count, total));
                                    }
                                    "FormatDisable" => { vim.config.disable_autoformat = true; let _ = vim.config.save(); vim.set_message("Autoformat Disabled".to_string()); }
                                    "FormatEnable" => { vim.config.disable_autoformat = false; let _ = vim.config.save(); vim.set_message("Autoformat Enabled".to_string()); }
                                    "bn" | "bnext" => { 
                                        editor.next_buffer(); 
                                        if let Some(path) = editor.buffer().file_path.clone() {
                                            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                let text = editor.buffer().lines.join("\n");
                                                let _ = lsp_manager.did_open(&ext, &path, text, None);
                                        let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                            }
                                        }
                                        vim.set_message(format!("Buffer {}/{}", editor.active_idx + 1, editor.buffers.len())); 
                                    }
                                    "bp" | "bprev" => { 
                                        editor.prev_buffer(); 
                                        if let Some(path) = editor.buffer().file_path.clone() {
                                            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                let text = editor.buffer().lines.join("\n");
                                                let _ = lsp_manager.did_open(&ext, &path, text, None);
                                        let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                            }
                                        }
                                        vim.set_message(format!("Buffer {}/{}", editor.active_idx + 1, editor.buffers.len())); 
                                    }
                                    "bd" | "bdelete" => { editor.close_current_buffer(); vim.set_message("Buffer closed".to_string()); }
                                    "reload" | "Reload" | "e!" => {
                                        if let Some(path) = editor.buffer().file_path.clone() {
                                            if let Ok(new_buffer) = crate::editor::buffer::Buffer::load(path.clone()) {
                                                *editor.buffer_mut() = new_buffer;
                                                vim.set_message(format!("Reloaded \"{}\"", path.display()));
                                                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                    let text = editor.buffer().lines.join("\n");
                                                    let _ = lsp_manager.did_open(&ext, &path, text, None);
                                        let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                                }
                                            }
                                        }
                                    }
                                    "e" | "edit" => { 
                                        if cmd_parts.len() > 1 { 
                                            let path = PathBuf::from(&cmd_parts[1]); 
                                            if editor.open_file(path.clone()).is_ok() {
                                                vim.set_message(format!("Opened \"{}\"", path.display()));
                                                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                    let text = editor.buffer().lines.join("\n");
                                                    let _ = lsp_manager.did_open(&ext, &path, text, None);
                                        let _ = lsp_manager.request_folding_ranges(&ext, &path);
                                                }
                                            } else {
                                                vim.set_message(format!("Error: Could not open \"{}\"", path.display()));
                                            }
                                        } 
                                    }
                                    "Mason" | "mason" => { vim.mode = Mode::Mason; }
                                    "colorscheme" | "colo" => {
                                        if cmd_parts.len() > 1 {
                                            let new_theme_name = &cmd_parts[1];
                                            editor.highlighter.theme = crate::ui::colorscheme::ColorScheme::new(new_theme_name);
                                            vim.config.colorscheme = new_theme_name.clone();
                                            let _ = vim.config.save();
                                            vim.set_message(format!("Colorscheme set to {}", new_theme_name));
                                        } else {
                                            vim.set_message(format!("Current colorscheme: {}", vim.config.colorscheme));
                                        }
                                    }
                                    _ => { vim.set_message(format!("Unknown command: {}", cmd_parts[0])); }                                }
                                // Only reset to Normal if we didn't change mode (like to Mason)
                                if let Mode::Command = vim.mode {
                                    vim.mode = Mode::Normal;
                                }
                            } else {
                                vim.mode = Mode::Normal;
                            }
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
                    Mode::Confirm(action) => match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            match action {
                                vim::mode::ConfirmAction::Quit => break,
                                vim::mode::ConfirmAction::CloseBuffer => {
                                    editor.close_current_buffer();
                                    vim.mode = Mode::Normal;
                                }
                            }
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            vim.mode = Mode::Normal;
                        }
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
