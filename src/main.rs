pub mod config;
pub mod editor;
pub mod lsp;
pub mod ui;
pub mod vim;
pub mod git;

use std::{env, error::Error, io, path::PathBuf, time::Duration, sync::mpsc};
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config};

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
use lsp::LspManager;
use lsp_types::{GotoDefinitionResponse, CompletionResponse, PublishDiagnosticsParams, CompletionTriggerKind};

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
    let config = crate::config::Config::load();
    let project_root = find_project_root(&env::current_dir().unwrap_or_default());
    let mut vim = VimState::new(config, project_root);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if vim.config.mouse {
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    } else {
        execute!(stdout, EnterAlternateScreen)?;
    }
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut editor = Editor::new(&vim.config.colorscheme);
    let ui = TerminalUi::new();
    let mut explorer = FileExplorer::new();
    let mut trouble = ui::trouble::TroubleList::new();
    let mut lsp_manager = LspManager::new();

    // File Watcher Setup
    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(&vim.project_root, RecursiveMode::Recursive)?;

    // Helper functions for common logic
    let format_buffer = |editor: &mut Editor, lsp_manager: &LspManager, vim: &mut VimState, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, ui: &TerminalUi, explorer: &FileExplorer, trouble: &ui::trouble::TroubleList| -> Result<(), String> {
        if let Some(path) = editor.buffer().file_path.clone() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                vim.lsp_status = LspStatus::Formatting;
                let _ = terminal.draw(|f| ui.draw(f, editor, vim, explorer, trouble, lsp_manager));
                let text = editor.buffer().text.to_string();
                match lsp_manager.format_document(&ext, &path, text) {
                    Some(Ok(formatted)) => {
                        editor.buffer_mut().text = ropey::Rope::from_str(&formatted);
                        editor.clamp_cursor();
                        let _ = lsp_manager.did_change(&ext, &path, formatted);
                        vim.lsp_status = LspStatus::None;
                        return Ok(());
                    }
                    Some(Err(e)) => { vim.lsp_status = LspStatus::None; return Err(e); }
                    None => { vim.lsp_status = LspStatus::None; return Err("No formatter available".to_string()); }
                }
            }
        }
        vim.lsp_status = LspStatus::None;
        Ok(())
    };

    let save_and_format = |editor: &mut Editor, lsp_manager: &LspManager, vim: &mut VimState, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, ui: &TerminalUi, explorer: &FileExplorer, trouble: &ui::trouble::TroubleList, path_to_save: Option<PathBuf>| {
        let mut format_info = String::new();
        if !vim.config.disable_autoformat {
            let _ = format_buffer(editor, lsp_manager, vim, terminal, ui, explorer, trouble);
            format_info.push_str("formatted, ");
        }
        let res = if let Some(path) = path_to_save {
            editor.save_file_as(path.clone()).map(|_| path.to_string_lossy().to_string())
        } else {
            editor.save_file().map(|_| editor.buffer().file_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default())
        };
        if let Ok(path_str) = res {
            let line_count = editor.buffer().len_lines();
            let char_count = editor.buffer().text.len_chars();
            vim.set_message(format!("\"{}\" {} {}L, {}C written", path_str, format_info, line_count, char_count));
            if let Some(path) = editor.buffer().file_path.clone() {
                let text = editor.buffer().text.to_string();
                editor.buffer_mut().git_signs = vim.git_manager.get_signs(&path, &text);
                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                    let _ = lsp_manager.did_save(&ext, &path, text);
                }
            }
        } else {
            vim.set_message("Error: Could not save file".to_string());
        }
    };

    let refresh_filtered_suggestions = |vim: &mut VimState, editor: &Editor| {
        let (y, x) = (editor.cursor().y, editor.cursor().x);
        let line = editor.buffer().line(y).unwrap().to_string();
        let mut start_x = x;
        let chars: Vec<char> = line.chars().collect();
        while start_x > 0 && (chars[start_x-1].is_alphanumeric() || chars[start_x-1] == '_' || chars[start_x-1] == '$') {
            start_x -= 1;
        }
        let prefix = if start_x < x { line[start_x..x].to_lowercase() } else { String::new() };

        let mut unique_items = std::collections::HashSet::new();
        let mut filtered: Vec<lsp_types::CompletionItem> = vim.suggestions.iter()
            .filter(|item| {
                let key = format!("{}:{:?}", item.label, item.kind);
                if unique_items.contains(&key) { return false; }
                if item.label.to_lowercase().contains(&prefix) {
                    unique_items.insert(key);
                    true
                } else { false }
            })
            .cloned()
            .collect();

        // Sort: Priority to starts_with, then alphabetical
        filtered.sort_by(|a, b| {
            let a_starts = a.label.to_lowercase().starts_with(&prefix);
            let b_starts = b.label.to_lowercase().starts_with(&prefix);
            if a_starts && !b_starts {
                std::cmp::Ordering::Less
            } else if !a_starts && b_starts {
                std::cmp::Ordering::Greater
            } else {
                a.label.cmp(&b.label)
            }
        });

        vim.filtered_suggestions = filtered;
        vim.selected_suggestion = 0;
        vim.suggestion_state.select(Some(0));
        vim.show_suggestions = !vim.filtered_suggestions.is_empty();
    };

    let sync_explorer = |explorer: &mut FileExplorer, editor: &Editor| {
        if explorer.visible {
            if let Some(path) = editor.buffer().file_path.as_ref() {
                explorer.reveal_path(path);
            }
        }
    };

    let install_selected_package = |vim: &mut VimState, editor: &mut Editor, lsp_manager: &LspManager| {
        let selected_idx = vim.mason_state.selected().unwrap_or(0);
        if vim.mason_tab == 5 {
            // Treesitter
            let languages = &crate::editor::treesitter::LANGUAGES;
            let filtered_langs: Vec<_> = languages.iter()
                .filter(|l| l.name.to_lowercase().contains(&vim.mason_filter.to_lowercase()))
                .collect();
            if let Some(lang) = filtered_langs.get(selected_idx) {
                if let Err(e) = editor.treesitter.install(lang) {
                    vim.set_message(format!("Error installing parser: {}", e));
                } else {
                    vim.set_message(format!("Parser {} installed", lang.name));
                }
            }
        } else {
            // LSP/DAP/etc
            let packages: Vec<&crate::lsp::Package> = crate::lsp::PACKAGES.iter()
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
            
            let (installed, available): (Vec<_>, Vec<_>) = packages.into_iter().partition(|p| lsp_manager.is_managed(p.cmd));
            let target = if selected_idx < installed.len() { Some(installed[selected_idx]) }
                         else if selected_idx < installed.len() + available.len() { Some(available[selected_idx - installed.len()]) }
                         else { None };

            if let Some(pkg) = target {
                let _ = lsp_manager.install_server(pkg.cmd);
                vim.set_message(format!("Installing {}...", pkg.name));
            }
        }
    };

    let toggle_comment = |editor: &mut Editor, vim: &mut VimState| {
        let path = editor.buffer().file_path.clone();
        let ext = path.as_ref().and_then(|p| p.extension()).and_then(|s| s.to_str()).unwrap_or("rs");
        let comment_prefix = match ext {
            "rs" | "js" | "ts" | "c" | "cpp" | "java" | "go" | "svelte" => "// ",
            "py" | "rb" | "sh" | "yaml" | "yml" | "toml" => "# ",
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
            let line = editor.buffer().line(y).unwrap().to_string();
            line.trim().is_empty() || line.trim().starts_with(comment_prefix)
        });

        for y in s_y..=e_y {
            let line_str = editor.buffer().line(y).unwrap().to_string();
            if line_str.trim().is_empty() { continue; }
            let line_start_char = editor.buffer().text.line_to_char(y);
            if all_commented {
                if let Some(pos) = line_str.find(comment_prefix) {
                    editor.buffer_mut().text.remove((line_start_char + pos)..(line_start_char + pos + comment_prefix.len()));
                }
                if !comment_suffix.is_empty() {
                    let updated = editor.buffer().line(y).unwrap().to_string();
                    if let Some(pos) = updated.rfind(comment_suffix) {
                        editor.buffer_mut().text.remove((line_start_char + pos)..(line_start_char + pos + comment_suffix.len()));
                    }
                }
            } else {
                let indent = line_str.chars().take_while(|c| c.is_whitespace()).count();
                editor.buffer_mut().text.insert(line_start_char + indent, comment_prefix);
                let end_pos = line_start_char + editor.buffer().line(y).unwrap().len_chars();
                let has_newline = editor.buffer().line(y).unwrap().to_string().ends_with('\n');
                editor.buffer_mut().text.insert(if has_newline { end_pos - 1 } else { end_pos }, comment_suffix);
            }
        }
    };

    // Handle CLI arguments
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        editor.buffers.clear(); editor.cursors.clear();
        for arg in &args[1..] {
            let path = PathBuf::from(arg).canonicalize().unwrap_or(PathBuf::from(arg));
            if path.is_dir() {
                explorer.root = path.clone();
                vim.project_root = find_project_root(&path);
                vim.reinit_git();
                explorer.refresh();
            } else {
                let _ = editor.open_file(path.clone());
                if let Some(buf) = editor.buffers.last_mut() {
                    let content = buf.text.to_string();
                    buf.git_signs = vim.git_manager.get_signs(&path, &content);
                }
            }
        }
        if editor.buffers.is_empty() { editor.buffers.push(editor::buffer::Buffer::new()); editor.cursors.push(editor::cursor::Cursor::new()); }
        editor.active_idx = 0;
    } else {
        vim.show_intro = true;
    }

    loop {
        // 0. Updates
        if let Some(time) = vim.message_time {
            if time.elapsed().as_secs() >= 3 { vim.message = None; vim.message_time = None; }
        }

        if vim.last_git_update.is_none() || vim.last_git_update.unwrap().elapsed() > Duration::from_secs(5) {
            vim.git_info = update_git_info(&vim.project_root);
            for buffer in &mut editor.buffers {
                if let Some(path) = &buffer.file_path {
                    let text = buffer.text.to_string();
                    buffer.git_signs = vim.git_manager.get_signs(path, &text);
                }
            }
            vim.last_git_update = Some(std::time::Instant::now());
        }

        // File Watcher Events
        let mut explorer_needs_refresh = false;
        let mut buffers_to_reload = Vec::new();

        while let Ok(res) = rx.try_recv() {
            if let Ok(event) = res {
                use notify::EventKind;
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        explorer_needs_refresh = true;
                        for path in event.paths {
                            if let Some(active_path) = editor.buffer().file_path.as_ref() {
                                if path == *active_path {
                                    buffers_to_reload.push(path);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if explorer_needs_refresh && explorer.visible {
            explorer.refresh();
        }

        for _path in buffers_to_reload {
            if !editor.buffer().modified {
                if let Err(e) = editor.buffer_mut().reload() {
                    vim.set_message(format!("Error reloading file: {}", e));
                } else {
                    vim.set_message("File reloaded from disk".to_string());
                    editor.refresh_syntax();
                }
            }
        }

        // LSP ensure/debouncing/message processing

        if let Some(path) = editor.buffer().file_path.clone() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                let _ = lsp_manager.start_client(&ext, vim.project_root.clone());
            }
        }

        // Process LSP messages
        let mut messages_to_process = Vec::new();
        {
            let clients = lsp_manager.clients.lock().unwrap();
            for (ext, ext_clients) in clients.iter() {
                for (client, _, cmd) in ext_clients {
                    while let Ok(msg) = client.receiver().try_recv() {
                        messages_to_process.push((ext.clone(), cmd.clone(), msg));
                    }
                }
            }
        }

        let mut newly_ready_clients = Vec::new();
        for (ext, cmd, msg) in messages_to_process {
            match msg {
                lsp_server::Message::Response(resp) => {
                    let id_str = resp.id.to_string();
                    let id = id_str.trim_matches('"').parse::<i32>().ok();

                    if let Some(id) = id {
                        if id == 1 {
                            let mut clients = lsp_manager.clients.lock().unwrap();
                            if let Some(ext_clients) = clients.get_mut(&ext) {
                                for (client, state, c) in ext_clients.iter_mut() {
                                    if c == &cmd {
                                        *state = crate::lsp::ClientState::Ready;
                                        let _ = client.send_notification("initialized", serde_json::json!({}));
                                        newly_ready_clients.push((ext.clone(), cmd.clone()));
                                    }
                                }
                            }
                        } else if Some(id) == vim.definition_request_id {
                            // Handle definition response
                            vim.definition_request_id = None;
                            if let Ok(value) = serde_json::from_value::<GotoDefinitionResponse>(resp.result.unwrap_or_default()) {
                                match value {
                                    GotoDefinitionResponse::Scalar(loc) => {
                                        let path = PathBuf::from(loc.uri.to_file_path().unwrap());
                                        let pos = Position { x: loc.range.start.character as usize, y: loc.range.start.line as usize };
                                        let _ = editor.open_file(path);
                                        editor.cursor_mut().y = pos.y;
                                        editor.cursor_mut().x = pos.x;
                                        sync_explorer(&mut explorer, &editor);
                                    }
                                    _ => {} // Handle multiple locations if needed
                                }
                            }
                        } else {
                            // Handle completions
                            if let Ok(value) = serde_json::from_value::<CompletionResponse>(resp.result.unwrap_or_default()) {
                                match value {
                                    CompletionResponse::Array(items) => { vim.suggestions = items; }
                                    CompletionResponse::List(list) => { vim.suggestions = list.items; }
                                }
                                refresh_filtered_suggestions(&mut vim, &editor);
                            }
                        }
                    }
                }
                lsp_server::Message::Notification(notif) => {
                    if notif.method == "textDocument/publishDiagnostics" {
                        if let Ok(params) = serde_json::from_value::<PublishDiagnosticsParams>(notif.params) {
                            let mut diagnostics = lsp_manager.diagnostics.lock().unwrap();
                            let file_diags = diagnostics.entry(params.uri).or_default();
                            file_diags.insert(cmd, params.diagnostics);
                        }
                    }
                }
                _ => {}
            }
        }

        for (ext, cmd) in newly_ready_clients {
            for buf in &editor.buffers {
                if let Some(path) = &buf.file_path {
                    if path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) == Some(ext.clone()) {
                        let text = buf.text.to_string();
                        let _ = lsp_manager.did_open(&ext, path, text, Some(&cmd));
                    }
                }
            }
        }
        
        // 1. Process events (Draining for zero-lag)
        let mut should_quit = false;
        if event::poll(Duration::from_millis(10))? {
            while event::poll(Duration::from_millis(0))? {
                let event = event::read()?;
                if let Event::Mouse(mouse) = &event {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => { if let Mode::Telescope(_) = vim.mode { vim.telescope.scroll_preview_up(3); } else { editor.move_up(); } }
                        MouseEventKind::ScrollDown => { if let Mode::Telescope(_) = vim.mode { vim.telescope.scroll_preview_down(3); } else { editor.move_down(); } }
                        _ => {}
                    }
                }
                if let Event::Key(key) = event {
                    vim.show_intro = false;
                    vim.yank_highlight_line = None;
                    if vim.blame_popup.is_some() { vim.blame_popup = None; continue; }
                    
                    match vim.mode {
                        Mode::Normal => {
                            match vim.focus {
                                Focus::Editor => {
                                    match key.code {
                                        KeyCode::Esc => { vim.input_buffer.clear(); vim.selection_start = None; }
                                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                            save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None);
                                        }
                                        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => { editor.redo(); }
                                        KeyCode::Tab => { editor.next_buffer(); sync_explorer(&mut explorer, &editor); }
                                        KeyCode::BackTab => { editor.prev_buffer(); sync_explorer(&mut explorer, &editor); }
                                        KeyCode::Char(c) => {
                                            vim.input_buffer.push(c);
                                            let seq = vim.input_buffer.clone();
                                            
                                            // 1. Check for complete multi-key sequences
                                            let mut matched = true;
                                            match seq.as_str() {
                                                " ff" => {
                                                    vim.telescope.open(vim::mode::TelescopeKind::Files, vim.project_root.clone(), &editor);
                                                    vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Files);
                                                }
                                                " fg" => {
                                                    vim.telescope.open(vim::mode::TelescopeKind::Words, vim.project_root.clone(), &editor);
                                                    vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Words);
                                                }
                                                " fb" => {
                                                    vim.telescope.open(vim::mode::TelescopeKind::Buffers, vim.project_root.clone(), &editor);
                                                    vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Buffers);
                                                }
                                                " th" | "th" => {
                                                    vim.telescope.open(vim::mode::TelescopeKind::Themes, vim.project_root.clone(), &editor);
                                                    vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Themes);
                                                }
                                                " n" => { vim.relative_number = !vim.relative_number; }
                                                " /" => { toggle_comment(&mut editor, &mut vim); }
                                                " tt" => { trouble.toggle(); }
                                                " bb" => {
                                                    vim.config.disable_autoformat = !vim.config.disable_autoformat;
                                                    vim.set_message(format!("Autoformat {}", if vim.config.disable_autoformat { "disabled" } else { "enabled" }));
                                                }
                                                " bl" => { vim.blame_popup = Some("Git Blame: You (just now) - placeholder".to_string()); }
                                                " x" => {
                                                    if editor.buffer().modified {
                                                        vim.mode = Mode::Confirm(crate::vim::mode::ConfirmAction::CloseBuffer);
                                                    } else {
                                                        editor.close_current_buffer();
                                                    }
                                                }
                                                "gg" => { editor.jump_to_first_line(); }
                                                "dd" => {
                                                    let y = editor.cursor().y;
                                                    vim.register = editor.delete_line(y);
                                                    vim.yank_type = YankType::Line;
                                                }
                                                "yy" => {
                                                    let y = editor.cursor().y;
                                                    let line = editor.buffer().line(y).unwrap().to_string();
                                                    vim.register = line;
                                                    vim.yank_type = YankType::Line;
                                                    vim.set_message("Line yanked".to_string());
                                                }
                                                "[[" => { editor.jump_to_first_line(); }
                                                "]]" => { editor.jump_to_last_line(); }
                                                "gd" => {
                                                    if let Some(path) = editor.buffer().file_path.clone() {
                                                        if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                            let (y, x) = (editor.cursor().y, editor.cursor().x);
                                                            match lsp_manager.request_definition(&ext, &path, y, x) {
                                                                Ok(id) => { vim.definition_request_id = Some(id); }
                                                                Err(e) => { vim.set_message(format!("LSP Error: {}", e)); }
                                                            }
                                                        }
                                                    }
                                                }
                                                "zc" | "za" => { editor.toggle_fold(&vim.folding_ranges); }
                                                "]g" => { editor.jump_to_next_hunk(); }
                                                "[g" => { editor.jump_to_prev_hunk(); }
                                                _ => { matched = false; }
                                            }

                                            if matched {
                                                vim.input_buffer.clear();
                                            } else {
                                                // 2. Check for partial sequences
                                                let is_partial = match seq.as_str() {
                                                    " " | " f" | " t" | " g" | " b" | "[" | "]" | "z" | "d" | "y" | "g" => true,
                                                    _ => false,
                                                };

                                                if !is_partial {
                                                    // 3. Fallback to single-key commands
                                                    // We use the LAST character if it's not a sequence
                                                    vim.input_buffer.clear();
                                                    match c {
                                                        'i' => { editor.buffer_mut().push_history(); vim.mode = Mode::Insert; }
                                                        'v' => { vim.mode = Mode::Visual; let c = editor.cursor(); vim.selection_start = Some(Position { x: c.x, y: c.y }); }
                                                        ':' => { vim.mode = Mode::Command; vim.command_buffer.clear(); }
                                                        '/' => { vim.mode = Mode::Search; vim.search_query.clear(); }
                                                        '\\' => {
                                                            if explorer.visible {
                                                                if vim.focus == Focus::Explorer {
                                                                    explorer.visible = false;
                                                                    vim.focus = Focus::Editor;
                                                                } else {
                                                                    vim.focus = Focus::Explorer;
                                                                    if let Some(path) = editor.buffer().file_path.as_ref() {
                                                                        explorer.reveal_path(path);
                                                                    }
                                                                }
                                                            } else {
                                                                explorer.visible = true;
                                                                explorer.init_root();
                                                                vim.focus = Focus::Explorer;
                                                                if let Some(path) = editor.buffer().file_path.as_ref() {
                                                                    explorer.reveal_path(path);
                                                                }
                                                            }
                                                        }
                                                        'j' => editor.move_down(),
                                                        'k' => editor.move_up(),
                                                        'h' => editor.move_left(),
                                                        'l' => editor.move_right(),
                                                        'u' => { editor.undo(); }
                                                        'w' => editor.move_word_forward(),
                                                        'b' => editor.move_word_backward(),
                                                        'e' => editor.move_word_end(),
                                                        'x' => {
                                                            let y = editor.cursor().y;
                                                            let x = editor.cursor().x;
                                                            vim.register = editor.delete_selection(x, y, x, y);
                                                            vim.yank_type = YankType::Char;
                                                        }
                                                        'o' => { editor.open_line_below(); vim.mode = Mode::Insert; }
                                                        'O' => { editor.open_line_above(); vim.mode = Mode::Insert; }
                                                        's' => {
                                                            let y = editor.cursor().y;
                                                            let x = editor.cursor().x;
                                                            editor.delete_selection(x, y, x, y);
                                                            vim.mode = Mode::Insert;
                                                        }
                                                        '?' => {

                                                            vim.mode = Mode::Keymaps;
                                                            vim.keymap_filter.clear();
                                                            vim.keymap_state.select(Some(0));
                                                        }
                                                        'G' => { editor.jump_to_last_line(); }

                                                        'p' => {
                                                            let text = vim.register.clone();
                                                            let y_type = vim.yank_type;
                                                            editor.paste_after(&text, y_type);
                                                        }
                                                        'P' => {
                                                            let text = vim.register.clone();
                                                            let y_type = vim.yank_type;
                                                            editor.paste_before(&text, y_type);
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        // Non-character keys always clear the sequence
                                        _ => {
                                            vim.input_buffer.clear();
                                            match key.code {
                                                KeyCode::Down => editor.move_down(),
                                                KeyCode::Up => editor.move_up(),
                                                KeyCode::Left => editor.move_left(),
                                                KeyCode::Right => editor.move_right(),
                                                KeyCode::PageUp => editor.move_to_line_start(),
                                                KeyCode::PageDown => editor.move_to_line_end(),
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                Focus::Explorer => {
                                    match key.code {
                                        KeyCode::Char('\\') | KeyCode::Esc => {
                                            explorer.visible = false;
                                            vim.focus = Focus::Editor;
                                        }
                                        KeyCode::Char(':') => {
                                            vim.mode = Mode::Command;
                                            vim.command_buffer.clear();
                                        }
                                        KeyCode::Char('j') | KeyCode::Down => explorer.move_down(),

                                        KeyCode::Char('k') | KeyCode::Up => explorer.move_up(),
                                        KeyCode::Char('h') | KeyCode::Left => explorer.collapse(),
                                        KeyCode::Char('l') | KeyCode::Right => {
                                            if let Some(entry) = explorer.selected_entry() {
                                                if entry.is_dir {
                                                    explorer.expand();
                                                } else {
                                                    let path = entry.path.clone();
                                                    if let Err(e) = editor.open_file(path.clone()) {
                                                        vim.set_message(format!("Error: {}", e));
                                                    } else {
                                                        vim.focus = Focus::Editor;
                                                        if let Some(buf) = editor.buffers.last_mut() {
                                                            let content = buf.text.to_string();
                                                            buf.git_signs = vim.git_manager.get_signs(&path, &content);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Char('<') => explorer.decrease_width(),
                                        KeyCode::Char('>') => explorer.increase_width(),
                                        KeyCode::Enter => {
                                            if let Some(entry) = explorer.selected_entry() {
                                                if entry.is_dir {
                                                    explorer.toggle_expand();
                                                } else {
                                                    let path = entry.path.clone();
                                                    if let Err(e) = editor.open_file(path.clone()) {
                                                        vim.set_message(format!("Error: {}", e));
                                                    } else {
                                                        vim.focus = Focus::Editor;
                                                        if let Some(buf) = editor.buffers.last_mut() {
                                                            let content = buf.text.to_string();
                                                            buf.git_signs = vim.git_manager.get_signs(&path, &content);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Char('a') => {
                                            vim.mode = Mode::ExplorerInput(ExplorerInputType::Add);
                                            vim.input_buffer.clear();
                                        }
                                        KeyCode::Char('r') => {
                                            vim.mode = Mode::ExplorerInput(ExplorerInputType::Rename);
                                            vim.input_buffer.clear();
                                        }
                                        KeyCode::Char('d') => {
                                            vim.mode = Mode::ExplorerInput(ExplorerInputType::DeleteConfirm);
                                            vim.input_buffer.clear();
                                        }
                                        KeyCode::Char('y') => {
                                            if let Some(entry) = explorer.selected_entry() {
                                                vim.register = entry.path.to_string_lossy().to_string();
                                                vim.set_message("Path copied to register".to_string());
                                            }
                                        }
                                        KeyCode::Char('H') => {
                                            explorer.show_hidden = !explorer.show_hidden;
                                            explorer.refresh();
                                        }
                                        _ => {}
                                    }
                                }
                                Focus::Trouble => {
                                    match key.code {
                                        KeyCode::Char('j') | KeyCode::Down => trouble.move_down(),
                                        KeyCode::Char('k') | KeyCode::Up => trouble.move_up(),
                                        KeyCode::Enter => {
                                            if let Some(item) = trouble.selected_item() {
                                                let path = item.path.clone();
                                                let line = item.line;
                                                let col = item.col;
                                                if let Err(e) = editor.open_file(path) {
                                                    vim.set_message(format!("Error: {}", e));
                                                } else {
                                                    editor.cursor_mut().y = line;
                                                    editor.cursor_mut().x = col;
                                                    vim.focus = Focus::Editor;
                                                }
                                            }
                                        }
                                        KeyCode::Esc | KeyCode::Char('q') => {
                                            trouble.visible = false;
                                            vim.focus = Focus::Editor;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Mode::Visual => {
                            match key.code {
                                KeyCode::Esc => { vim.mode = Mode::Normal; vim.selection_start = None; }
                                KeyCode::Char('j') | KeyCode::Down => editor.move_down(),
                                KeyCode::Char('k') | KeyCode::Up => editor.move_up(),
                                KeyCode::Char('h') | KeyCode::Left => editor.move_left(),
                                KeyCode::Char('l') | KeyCode::Right => editor.move_right(),
                                KeyCode::Char('w') => editor.move_word_forward(),
                                KeyCode::Char('b') => editor.move_word_backward(),
                                KeyCode::Char('y') => {

                                    let start = vim.selection_start.unwrap();
                                    let cur = editor.cursor();
                                    vim.register = editor.yank(start.x, start.y, cur.x, cur.y);
                                    vim.yank_type = YankType::Char;
                                    vim.mode = Mode::Normal;
                                    vim.selection_start = None;
                                }
                                KeyCode::Char('d') | KeyCode::Char('x') => {
                                    let start = vim.selection_start.unwrap();
                                    let cur = editor.cursor();
                                    vim.register = editor.delete_selection(start.x, start.y, cur.x, cur.y);
                                    vim.yank_type = YankType::Char;
                                    vim.mode = Mode::Normal;
                                    vim.selection_start = None;
                                }
                                _ => {}
                            }
                        }
                        Mode::Insert => {
                            match key.code {
                                KeyCode::Esc => {
                                    if vim.show_suggestions {
                                        vim.show_suggestions = false;
                                        vim.filtered_suggestions.clear();
                                        vim.suggestions.clear();
                                    } else {
                                        vim.mode = Mode::Normal;
                                    }
                                }
                                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None);
                                }
                                KeyCode::Char(' ') | KeyCode::Null if key.modifiers.contains(KeyModifiers::CONTROL) || key.code == KeyCode::Null => {
                                    if let Some(path) = editor.buffer().file_path.clone() {
                                        if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                            let (y, x) = (editor.cursor().y, editor.cursor().x);
                                            let _ = lsp_manager.request_completions(&ext, &path, y, x, CompletionTriggerKind::INVOKED, None);
                                        }
                                    }
                                }
                                KeyCode::Char(c) => {
                                    let (y, x) = (editor.cursor().y, editor.cursor().x);
                                    let idx = editor.buffer().text.line_to_char(y) + x;
                                    
                                    let mut to_insert = c.to_string();
                                    match c {
                                        '(' => { to_insert.push(')'); }
                                        '[' => { to_insert.push(']'); }
                                        '{' => { to_insert.push('}'); }
                                        '\'' => { to_insert.push('\''); }
                                        '"' => { to_insert.push('"'); }
                                        '>' => {
                                            if let Some(line) = editor.buffer().line(y) {
                                                let line_str = line.to_string();
                                                let before_cursor = &line_str[..x.min(line_str.len())];
                                                if let Some(tag_start) = before_cursor.rfind('<') {
                                                    let tag_content = &before_cursor[tag_start+1..];
                                                    if !tag_content.is_empty() && !tag_content.contains(' ') && !tag_content.contains('/') {
                                                        to_insert.push_str(&format!("</{}>", tag_content));
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    }

                                    editor.buffer_mut().text.insert(idx, &to_insert);
                                    editor.cursor_mut().x += 1;

                                    // Trigger LSP completion
                                    if let Some(path) = editor.buffer().file_path.clone() {
                                        if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                            let text = editor.buffer().text.to_string();
                                            let _ = lsp_manager.did_change(&ext, &path, text);
                                            let trigger_kind = if c == '.' || c == ':' || c == '>' { CompletionTriggerKind::TRIGGER_CHARACTER } else { CompletionTriggerKind::INVOKED };
                                            let trigger_char = if trigger_kind == CompletionTriggerKind::TRIGGER_CHARACTER { Some(c.to_string()) } else { None };
                                            let _ = lsp_manager.request_completions(&ext, &path, y, x + 1, trigger_kind, trigger_char);
                                        }
                                    }
                                    refresh_filtered_suggestions(&mut vim, &editor);
                                }
                                KeyCode::Backspace => {
                                    let (y, x) = (editor.cursor().y, editor.cursor().x);
                                    if x > 0 {
                                        let idx = editor.buffer().text.line_to_char(y) + x;
                                        editor.buffer_mut().text.remove((idx-1)..idx);
                                        editor.cursor_mut().x -= 1;
                                        if let Some(path) = editor.buffer().file_path.clone() {
                                            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                let text = editor.buffer().text.to_string();
                                                let _ = lsp_manager.did_change(&ext, &path, text);
                                                let _ = lsp_manager.request_completions(&ext, &path, y, x - 1, CompletionTriggerKind::INVOKED, None);
                                            }
                                        }
                                        refresh_filtered_suggestions(&mut vim, &editor);
                                    }
                                }
                                KeyCode::Tab => {
                                    if vim.show_suggestions && !vim.filtered_suggestions.is_empty() {
                                        vim.selected_suggestion = (vim.selected_suggestion + 1) % vim.filtered_suggestions.len();
                                        vim.suggestion_state.select(Some(vim.selected_suggestion));
                                    } else {
                                        let (y, x) = (editor.cursor().y, editor.cursor().x);
                                        let idx = editor.buffer().text.line_to_char(y) + x;
                                        let spaces = " ".repeat(vim.config.tabstop);
                                        editor.buffer_mut().text.insert(idx, &spaces);
                                        editor.cursor_mut().x += vim.config.tabstop;
                                    }
                                }
                                KeyCode::Enter => {
                                    if vim.show_suggestions && !vim.filtered_suggestions.is_empty() {
                                        let selected = &vim.filtered_suggestions[vim.selected_suggestion];
                                        let (y, x) = (editor.cursor().y, editor.cursor().x);
                                        
                                        // Simple completion: replace current word with suggestion
                                        let line = editor.buffer().line(y).unwrap().to_string();
                                        let mut start_x = x;
                                        let chars: Vec<char> = line.chars().collect();
                                        while start_x > 0 && (chars[start_x-1].is_alphanumeric() || chars[start_x-1] == '_' || chars[start_x-1] == '$') {
                                            start_x -= 1;
                                        }
                                        
                                        let line_start_char = editor.buffer().text.line_to_char(y);
                                        editor.buffer_mut().text.remove((line_start_char + start_x)..(line_start_char + x));
                                        editor.buffer_mut().text.insert(line_start_char + start_x, &selected.label);
                                        editor.cursor_mut().x = start_x + selected.label.len();
                                        
                                        vim.show_suggestions = false;
                                        vim.filtered_suggestions.clear();
                                        vim.suggestions.clear();
                                    } else {
                                        let (y, x) = (editor.cursor().y, editor.cursor().x);
                                        let idx = editor.buffer().text.line_to_char(y) + x;
                                        editor.buffer_mut().text.insert(idx, "\n");
                                        editor.cursor_mut().y += 1; editor.cursor_mut().x = 0;
                                    }
                                }
                                KeyCode::PageUp => editor.move_to_line_start(),
                                KeyCode::PageDown => editor.move_to_line_end(),
                                KeyCode::Down => {
                                    if vim.show_suggestions && !vim.filtered_suggestions.is_empty() {
                                        vim.selected_suggestion = (vim.selected_suggestion + 1) % vim.filtered_suggestions.len();
                                        vim.suggestion_state.select(Some(vim.selected_suggestion));
                                    } else {
                                        editor.move_down();
                                    }
                                }
                                KeyCode::Up => {
                                    if vim.show_suggestions && !vim.filtered_suggestions.is_empty() {
                                        if vim.selected_suggestion > 0 {
                                            vim.selected_suggestion -= 1;
                                        } else {
                                            vim.selected_suggestion = vim.filtered_suggestions.len() - 1;
                                        }
                                        vim.suggestion_state.select(Some(vim.selected_suggestion));
                                    } else {
                                        editor.move_up();
                                    }
                                }
                                KeyCode::Left => editor.move_left(),
                                KeyCode::Right => editor.move_right(),
                                _ => {}
                            }
                        }
                        Mode::Search => {
                            match key.code {
                                KeyCode::Esc => { vim.mode = Mode::Normal; }
                                KeyCode::Char(c) => { vim.search_query.push(c); }
                                KeyCode::Backspace => { vim.search_query.pop(); }
                                KeyCode::Enter => { vim.mode = Mode::Normal; }
                                _ => {}
                            }
                        }
                        Mode::ExplorerInput(input_type) => {
                            match key.code {
                                KeyCode::Esc => { vim.mode = Mode::Normal; }
                                KeyCode::Char(c) => { vim.input_buffer.push(c); }
                                KeyCode::Backspace => { vim.input_buffer.pop(); }
                                KeyCode::Enter => {
                                    let input = vim.input_buffer.clone();
                                    vim.input_buffer.clear();
                                    vim.mode = Mode::Normal;
                                    match input_type {
                                        ExplorerInputType::Add => {
                                            if let Err(e) = explorer.create_file(&input) {
                                                vim.set_message(format!("Error: {}", e));
                                            }
                                        }
                                        ExplorerInputType::Rename => {
                                            if let Err(e) = explorer.rename_selected(&input) {
                                                vim.set_message(format!("Error: {}", e));
                                            }
                                        }
                                        ExplorerInputType::DeleteConfirm => {
                                            if input.to_lowercase() == "y" {
                                                if let Err(e) = explorer.delete_selected() {
                                                    vim.set_message(format!("Error: {}", e));
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                        Mode::Confirm(action) => {
                            match key.code {
                                KeyCode::Char('y') | KeyCode::Char('Y') => {
                                    match action {
                                        crate::vim::mode::ConfirmAction::Quit => {
                                            save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None);
                                            should_quit = true;
                                        }
                                        crate::vim::mode::ConfirmAction::CloseBuffer => {
                                            save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None);
                                            editor.close_current_buffer();
                                            vim.mode = Mode::Normal;
                                        }
                                    }
                                }
                                KeyCode::Char('n') | KeyCode::Char('N') => {
                                    match action {
                                        crate::vim::mode::ConfirmAction::Quit => {
                                            should_quit = true;
                                        }
                                        crate::vim::mode::ConfirmAction::CloseBuffer => {
                                            editor.close_current_buffer();
                                            vim.mode = Mode::Normal;
                                        }
                                    }
                                }
                                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                                    vim.mode = Mode::Normal;
                                }
                                _ => {}
                            }
                        }
                        Mode::Telescope(_) => {
                            match key.code {
                                KeyCode::Esc => { vim.mode = Mode::Normal; vim.telescope.close(); }
                                KeyCode::Char('j') | KeyCode::Down => vim.telescope.move_down(),
                                KeyCode::Char('k') | KeyCode::Up => vim.telescope.move_up(),
                                KeyCode::Tab => vim.telescope.move_down(),
                                KeyCode::BackTab => vim.telescope.move_up(),
                                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => vim.telescope.scroll_preview_up(5),
                                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => vim.telescope.scroll_preview_down(5),
                                KeyCode::Char(c) => {
                                    vim.telescope.query.push(c);
                                    vim.telescope.update_results(&editor);
                                }
                                KeyCode::Backspace => {
                                    vim.telescope.query.pop();
                                    vim.telescope.update_results(&editor);
                                }
                                KeyCode::Enter => {
                                    if let Some(result) = vim.telescope.results.get(vim.telescope.selected_idx) {
                                        match vim.telescope.kind {
                                            vim::mode::TelescopeKind::Themes => {
                                                editor.set_theme(&result.path.to_string_lossy());
                                            }
                                            vim::mode::TelescopeKind::Buffers => {
                                                if let Some(idx) = result.buffer_idx {
                                                    editor.active_idx = idx;
                                                    sync_explorer(&mut explorer, &editor);
                                                }
                                            }
                                            _ => {
                                                let path = result.path.clone();
                                                let line = result.line_number.unwrap_or(1).saturating_sub(1);
                                                if let Err(e) = editor.open_file(path) {
                                                    vim.set_message(format!("Error: {}", e));
                                                } else {
                                                    editor.cursor_mut().y = line;
                                                    editor.cursor_mut().x = 0;
                                                    sync_explorer(&mut explorer, &editor);
                                                }
                                            }
                                        }
                                    }
                                    vim.mode = Mode::Normal;
                                    vim.telescope.close();
                                }
                                _ => {}
                            }
                        }
                        Mode::Mason => {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('q') => { vim.mode = Mode::Normal; }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    let i = vim.mason_state.selected().unwrap_or(0);
                                    vim.mason_state.select(Some(i + 1));
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    let i = vim.mason_state.selected().unwrap_or(0);
                                    if i > 0 { vim.mason_state.select(Some(i - 1)); }
                                }
                                KeyCode::Char('1') => { vim.mason_tab = 0; vim.mason_state.select(Some(0)); }
                                KeyCode::Char('2') => { vim.mason_tab = 1; vim.mason_state.select(Some(0)); }
                                KeyCode::Char('3') => { vim.mason_tab = 2; vim.mason_state.select(Some(0)); }
                                KeyCode::Char('4') => { vim.mason_tab = 3; vim.mason_state.select(Some(0)); }
                                KeyCode::Char('5') => { vim.mason_tab = 4; vim.mason_state.select(Some(0)); }
                                KeyCode::Char('6') => { vim.mason_tab = 5; vim.mason_state.select(Some(0)); }
                                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    vim.mode = Mode::MasonFilter;
                                    vim.mason_filter.clear();
                                }
                                KeyCode::Char(' ') | KeyCode::Char('i') => {
                                    install_selected_package(&mut vim, &mut editor, &lsp_manager);
                                }
                                KeyCode::Char('u') => {
                                    install_selected_package(&mut vim, &mut editor, &lsp_manager);
                                }
                                KeyCode::Char('d') | KeyCode::Char('x') => {
                                    // uninstall logic can be similar if added
                                    install_selected_package(&mut vim, &mut editor, &lsp_manager);
                                }
                                _ => {}
                            }
                        }
                        Mode::MasonFilter => {
                            match key.code {
                                KeyCode::Esc | KeyCode::Enter => { vim.mode = Mode::Mason; }
                                KeyCode::Char(c) => {
                                    vim.mason_filter.push(c);
                                    vim.mason_state.select(Some(0));
                                }
                                KeyCode::Backspace => {
                                    vim.mason_filter.pop();
                                    vim.mason_state.select(Some(0));
                                }
                                _ => {}
                            }
                        }
                        Mode::Keymaps => {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('?') => { vim.mode = Mode::Normal; }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    let i = vim.keymap_state.selected().unwrap_or(0);
                                    vim.keymap_state.select(Some(i + 1));
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    let i = vim.keymap_state.selected().unwrap_or(0);
                                    if i > 0 { vim.keymap_state.select(Some(i - 1)); }
                                }
                                KeyCode::Char(c) => {
                                    vim.keymap_filter.push(c);
                                    vim.keymap_state.select(Some(0));
                                }
                                KeyCode::Backspace => {
                                    vim.keymap_filter.pop();
                                    vim.keymap_state.select(Some(0));
                                }
                                _ => {}
                            }
                        }
                        Mode::Command => {
                            let commands = vec![
                                "q", "quit", "qa", "qall", "w", "write", "wa", "wall", "wq", "x", "wqa", "xa",
                                "bn", "bnext", "bp", "bprev", "bd", "bdelete", "e", "edit", "e!", "Reload",
                                "colorscheme", "Mason", "Trouble", "format", "Format",
                                "FormatAll", "FormatEnable", "FormatDisable", "gd", "LspInfo", "LspRestart",
                                "set", "config"
                            ];

                            match key.code {
                                KeyCode::Esc => { 
                                    vim.mode = Mode::Normal; 
                                    vim.command_suggestions.clear();
                                }
                                KeyCode::Char(c) => { 
                                    vim.command_buffer.push(c); 
                                    vim.command_suggestions = commands.iter()
                                        .filter(|cmd| cmd.starts_with(&vim.command_buffer))
                                        .map(|s| s.to_string())
                                        .collect();
                                    vim.selected_command_suggestion = 0;
                                }
                                KeyCode::Backspace => { 
                                    vim.command_buffer.pop(); 
                                    if vim.command_buffer.is_empty() {
                                        vim.command_suggestions.clear();
                                    } else {
                                        vim.command_suggestions = commands.iter()
                                            .filter(|cmd| cmd.starts_with(&vim.command_buffer))
                                            .map(|s| s.to_string())
                                            .collect();
                                    }
                                    vim.selected_command_suggestion = 0;
                                }
                                KeyCode::Tab => {
                                    if !vim.command_suggestions.is_empty() {
                                        vim.selected_command_suggestion = (vim.selected_command_suggestion + 1) % vim.command_suggestions.len();
                                    }
                                }
                                KeyCode::Enter => {
                                    let selected_suggestion = if !vim.command_suggestions.is_empty() {
                                        Some(vim.command_suggestions[vim.selected_command_suggestion].clone())
                                    } else {
                                        None
                                    };

                                    let should_execute = if let Some(ref suggestion) = selected_suggestion {
                                        vim.command_buffer == *suggestion
                                    } else {
                                        true
                                    };

                                    if should_execute {
                                        let cmd_str = if let Some(suggestion) = selected_suggestion {
                                            suggestion
                                        } else {
                                            vim.command_buffer.trim().to_string()
                                        };
                                        
                                        vim.command_buffer.clear();
                                        vim.command_suggestions.clear();
                                        vim.mode = Mode::Normal;
                                        
                                        if !cmd_str.is_empty() {
                                            let mut parts = cmd_str.split_whitespace();
                                            let first_part = parts.next().unwrap_or("");
                                            let force = first_part.ends_with('!');
                                            let cmd = if force { &first_part[..first_part.len()-1] } else { first_part };
                                            let args: Vec<&str> = parts.collect();

                                            match cmd {
                                            "q" | "quit" => {
                                                if !force && editor.buffer().modified {
                                                    vim.mode = Mode::Confirm(crate::vim::mode::ConfirmAction::Quit);
                                                } else if editor.buffers.len() > 1 {
                                                    editor.close_current_buffer();
                                                } else {
                                                    should_quit = true;
                                                }
                                            }
                                            "qa" | "qall" => {
                                                let any_modified = editor.buffers.iter().any(|b| b.modified);
                                                if !force && any_modified {
                                                    vim.mode = Mode::Confirm(crate::vim::mode::ConfirmAction::Quit);
                                                } else {
                                                    should_quit = true;
                                                }
                                            }
                                            "w" | "write" => {
                                                let path_to_save = args.get(0).map(|s| PathBuf::from(*s));
                                                save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, path_to_save);
                                            }
                                            "wa" | "wall" => {
                                                let current_idx = editor.active_idx;
                                                for i in 0..editor.buffers.len() {
                                                    editor.active_idx = i;
                                                    save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None);
                                                }
                                                editor.active_idx = current_idx;
                                            }
                                            "wq" | "x" => {
                                                save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None);
                                                if editor.buffers.len() > 1 {
                                                    editor.close_current_buffer();
                                                } else {
                                                    should_quit = true;
                                                }
                                            }
                                            "wqa" | "xa" => {
                                                let current_idx = editor.active_idx;
                                                for i in 0..editor.buffers.len() {
                                                    editor.active_idx = i;
                                                    save_and_format(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble, None);
                                                }
                                                editor.active_idx = current_idx;
                                                should_quit = true;
                                            }
                                            "bn" | "bnext" => { editor.next_buffer(); sync_explorer(&mut explorer, &editor); }
                                            "bp" | "bprev" => { editor.prev_buffer(); sync_explorer(&mut explorer, &editor); }
                                            "bd" | "bdelete" => {
                                                if !force && editor.buffer().modified {
                                                    vim.mode = Mode::Confirm(crate::vim::mode::ConfirmAction::CloseBuffer);
                                                } else {
                                                    editor.close_current_buffer();
                                                    sync_explorer(&mut explorer, &editor);
                                                }
                                            }
                                            "e" | "edit" => {
                                                if let Some(path_str) = args.get(0) {
                                                    let path = PathBuf::from(*path_str);
                                                    if let Err(e) = editor.open_file(path) {
                                                        vim.set_message(format!("Error: {}", e));
                                                    } else {
                                                        sync_explorer(&mut explorer, &editor);
                                                    }
                                                }
                                            }
                                            "e!" | "Reload" => {
                                                if let Some(path) = editor.buffer().file_path.clone() {
                                                    if let Err(e) = editor.open_file(path) {
                                                        vim.set_message(format!("Error: {}", e));
                                                    } else {
                                                        vim.set_message("File reloaded".to_string());
                                                    }
                                                }
                                            }
                                            "colorscheme" => {
                                                if let Some(theme) = args.get(0) {
                                                    editor.set_theme(theme);
                                                    vim.set_message(format!("Colorscheme changed to {}", theme));
                                                } else {
                                                    vim.telescope.open(vim::mode::TelescopeKind::Themes, vim.project_root.clone(), &editor);
                                                    vim.mode = Mode::Telescope(vim::mode::TelescopeKind::Themes);
                                                }
                                            }
                                            "Mason" => { vim.mode = Mode::Mason; }
                                            "Trouble" => { trouble.toggle(); }
                                            "format" | "Format" => { let _ = format_buffer(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble); }
                                            "FormatAll" => {
                                                let current_idx = editor.active_idx;
                                                for i in 0..editor.buffers.len() {
                                                    editor.active_idx = i;
                                                    let _ = format_buffer(&mut editor, &lsp_manager, &mut vim, &mut terminal, &ui, &explorer, &trouble);
                                                }
                                                editor.active_idx = current_idx;
                                            }
                                            "FormatEnable" => { vim.config.disable_autoformat = false; vim.set_message("Autoformat enabled".to_string()); }
                                            "FormatDisable" => { vim.config.disable_autoformat = true; vim.set_message("Autoformat disabled".to_string()); }
                                            "gd" | "Definition" => {
                                                if let Some(path) = editor.buffer().file_path.clone() {
                                                    if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                                                        let (y, x) = (editor.cursor().y, editor.cursor().x);
                                                        match lsp_manager.request_definition(&ext, &path, y, x) {
                                                            Ok(id) => { vim.definition_request_id = Some(id); }
                                                            Err(e) => { vim.set_message(format!("LSP Error: {}", e)); }
                                                        }
                                                    }
                                                }
                                            }
                                            "LspInfo" => { vim.set_message("LSP Info (not implemented)".to_string()); }
                                            "LspRestart" => { vim.set_message("LSP Restarted".to_string()); }
                                            "set" => {
                                                if let Some(arg) = args.get(0) {
                                                    match *arg {
                                                        "number" => { vim.show_number = true; vim.config.number = true; }
                                                        "nonumber" => { vim.show_number = false; vim.config.number = false; }
                                                        "relativenumber" => { vim.relative_number = true; vim.config.relativenumber = true; }
                                                        "norelativenumber" => { vim.relative_number = false; vim.config.relativenumber = false; }
                                                        "cursorline" => { vim.config.cursorline = true; }
                                                        "nocursorline" => { vim.config.cursorline = false; }
                                                        "signcolumn" => { vim.config.signcolumn = true; }
                                                        "nosigncolumn" => { vim.config.signcolumn = false; }
                                                        "mouse" => { vim.config.mouse = true; }
                                                        "nomouse" => { vim.config.mouse = false; }
                                                        _ => { vim.set_message(format!("Unknown option: {}", arg)); }
                                                    }
                                                }
                                            }
                                            "config" => {
                                                if let Err(e) = vim.config.save() {
                                                    vim.set_message(format!("Error saving config: {}", e));
                                                } else {
                                                    vim.set_message("Config saved".to_string());
                                                }
                                            }
                                            _ => {
                                                vim.set_message(format!("Not an editor command: {}", cmd_str));
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
    }
        if should_quit { break; }

        // 2. State & Render
        let area = terminal.size()?;
        let visible_height = area.height.saturating_sub(2) as usize;
        let editor_width = if explorer.visible { (area.width as f32 * 0.85) as usize - 8 } else { area.width as usize - 8 };
        editor.scroll_into_view(visible_height, editor_width, vim.config.wrap);
        
        editor.refresh_syntax();
        terminal.draw(|f| ui.draw(f, &editor, &mut vim, &explorer, &trouble, &lsp_manager))?;

        let cursor_style = match vim.mode {
            Mode::Insert => SetCursorStyle::SteadyBar,
            _ => SetCursorStyle::SteadyBlock,
        };
        execute!(terminal.backend_mut(), cursor_style)?;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture, SetCursorStyle::DefaultUserShape)?;
    terminal.show_cursor()?;
    Ok(())
}
