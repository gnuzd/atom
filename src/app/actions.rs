use super::*;
use arboard::Clipboard;
use std::sync::Arc;

impl App {
    pub fn format_buffer(&mut self, op: BackgroundFileOp) {
        if let Some(path) = self.editor.buffer().file_path.clone() {
            if let Some(ext) = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
            {
                self.vim.lsp_status = LspStatus::Formatting;
                let text = self.editor.buffer().text.to_string();
                let lsp = self.lsp_manager.clone();
                let git = self.vim.git_manager.clone();
                let tx = self.format_tx.clone();

                tokio::task::spawn_blocking(move || {
                    let res = lsp.format_document(&ext, &path, text);
                    if let Some(r) = res {
                        let signs = if let Ok(formatted) = &r {
                            let _ = lsp.did_change(&ext, &path, formatted.clone());
                            git.get_signs(&path, formatted)
                        } else {
                            Vec::new()
                        };
                        let _ = tx.send((path, ext, AsyncResult::Format(r), signs, op));
                    }
                });
            }
        }
    }

    pub fn save_and_format(&mut self, path_to_save: Option<PathBuf>) {
        let current_path = path_to_save.or_else(|| self.editor.buffer().file_path.clone());
        let Some(path) = current_path else {
            self.vim
                .set_message("Error: No file path to save".to_string());
            return;
        };

        let text = self.editor.buffer().text.to_string();
        let lsp = self.lsp_manager.clone();
        let git = self.vim.git_manager.clone();
        let tx = self.format_tx.clone();
        let path_clone = path.clone();
        let autoformat = !self.vim.config.disable_autoformat;

        self.vim.lsp_status = if autoformat {
            LspStatus::Formatting
        } else {
            LspStatus::None
        };

        tokio::task::spawn_blocking(move || {
            use std::fs;
            use std::io::{self, Write};

            let mut final_text = text;
            let ext = path_clone
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();

            if autoformat && !ext.is_empty() {
                if let Some(res) = lsp.format_document(&ext, &path_clone, final_text.clone()) {
                    if let Ok(formatted) = res {
                        final_text = formatted;
                        let _ = lsp.did_change(&ext, &path_clone, final_text.clone());
                    }
                }
            }

            let save_res = (|| -> io::Result<()> {
                let file = fs::File::create(&path_clone)?;
                let mut writer = io::BufWriter::new(file);
                writer.write_all(final_text.as_bytes())?;
                writer.flush()?;
                Ok(())
            })();

            let signs = git.get_signs(&path_clone, &final_text);
            if !ext.is_empty() {
                let _ = lsp.did_save(&ext, &path_clone, final_text.clone());
            }

            match save_res {
                Ok(_) => {
                    let _ = tx.send((
                        path_clone,
                        ext,
                        AsyncResult::Save(Ok(final_text)),
                        signs,
                        BackgroundFileOp::Save,
                    ));
                }
                Err(e) => {
                    let _ = tx.send((
                        path_clone,
                        ext,
                        AsyncResult::Save(Err(e.to_string())),
                        signs,
                        BackgroundFileOp::Save,
                    ));
                }
            }
        });
    }

    pub fn refresh_filtered_suggestions(&mut self) {
        let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
        let Some(line) = self.editor.buffer().line(y) else {
            self.vim.show_suggestions = false;
            return;
        };
        let line_str = line.to_string();
        let mut start_x = x;
        let chars: Vec<char> = line_str.chars().collect();
        while start_x > 0
            && chars
                .get(start_x - 1)
                .is_some_and(|&c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            start_x -= 1;
        }
        let prefix = if start_x < x {
            line_str[start_x..x].to_lowercase()
        } else {
            String::new()
        };

        let mut unique_items = std::collections::HashSet::new();
        let mut filtered: Vec<lsp_types::CompletionItem> = self
            .vim
            .suggestions
            .iter()
            .filter(|item| {
                let key = format!("{}:{:?}", item.label, item.kind);
                if unique_items.contains(&key) {
                    return false;
                }
                if item.label.to_lowercase().contains(&prefix) {
                    unique_items.insert(key);
                    true
                } else {
                    false
                }
            })
            .cloned()
            .collect();

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

        self.vim.filtered_suggestions = filtered;
        self.vim.selected_suggestion = 0;
        self.vim.suggestion_state.select(Some(0));
        self.vim.show_suggestions = !self.vim.filtered_suggestions.is_empty();
    }

    pub fn sync_explorer(&mut self) {
        if self.explorer.visible {
            if let Some(path) = self.editor.buffer().file_path.as_ref() {
                self.explorer.reveal_path(path);
            }
        }
    }

    pub fn install_selected_package(&mut self, key: crossterm::event::KeyEvent) {
        let selected_idx = self.vim.mason_state.selected().unwrap_or(0);
        let action_type = match key.code {
            crossterm::event::KeyCode::Char('u') => "update",
            crossterm::event::KeyCode::Char('d') | crossterm::event::KeyCode::Char('x') => "uninstall",
            _ => "install",
        };

        if self.vim.mason_tab == 5 {
            let languages = &crate::editor::treesitter::LANGUAGES;
            let filtered_langs: Vec<_> = languages
                .iter()
                .filter(|l| {
                    l.name
                        .to_lowercase()
                        .contains(&self.vim.mason_filter.to_lowercase())
                })
                .collect();
            if let Some(lang) = filtered_langs.get(selected_idx) {
                let lang_name = lang.name.to_string();
                let ts_manager = Arc::clone(&self.editor.treesitter);
                let lsp_manager = self.lsp_manager.clone();
                
                lsp_manager.installing.lock().unwrap().insert(lang_name.clone());
                
                match action_type {
                    "uninstall" => {
                        let ts = self.editor.treesitter.lock().unwrap();
                        if let Err(e) = ts.uninstall(&lang_name) {
                            self.vim.set_message(format!("Error uninstalling parser: {}", e));
                        } else {
                            self.vim.set_message(format!("Parser {} uninstalled", lang_name));
                        }
                        lsp_manager.installing.lock().unwrap().remove(&lang_name);
                    }
                    _ => {
                        // For install and update, use a background thread
                        self.vim.set_message(format!("{} parser {}...", if action_type == "update" { "Updating" } else { "Installing" }, lang_name));
                        
                        // We need to pass the static reference to the thread
                        // Find the static lang in LANGUAGES
                        let static_lang = crate::editor::treesitter::LANGUAGES.iter().find(|l| l.name == lang_name);
                        
                        if let Some(l) = static_lang {
                            std::thread::spawn(move || {
                                let ts = ts_manager.lock().unwrap();
                                let _ = ts.install(l);
                                let mut installing = lsp_manager.installing.lock().unwrap();
                                installing.remove(&lang_name);
                            });
                        } else {
                            lsp_manager.installing.lock().unwrap().remove(&lang_name);
                        }
                    }
                }
            }
        } else {
            let packages: Vec<&crate::lsp::Package> = crate::lsp::PACKAGES
                .iter()
                .filter(|p| {
                    let matches_tab = match self.vim.mason_tab {
                        0 => true,
                        1 => p.kind == crate::lsp::PackageKind::Lsp,
                        2 => p.kind == crate::lsp::PackageKind::Dap,
                        3 => p.kind == crate::lsp::PackageKind::Linter,
                        4 => p.kind == crate::lsp::PackageKind::Formatter,
                        _ => true,
                    };
                    let filter = self.vim.mason_filter.to_lowercase();
                    let matches_filter = p.name.to_lowercase().contains(&filter)
                        || p.description.to_lowercase().contains(&filter);
                    matches_tab && matches_filter
                })
                .collect();

            let (mut installed, mut available): (Vec<_>, Vec<_>) = packages
                .into_iter()
                .partition(|p| self.lsp_manager.is_managed(p.cmd));
            installed.sort_by_key(|p| p.name);
            available.sort_by_key(|p| p.name);

            let target = if selected_idx < installed.len() {
                Some(installed[selected_idx])
            } else if selected_idx < installed.len() + available.len() {
                Some(available[selected_idx - installed.len()])
            } else {
                None
            };

            if let Some(pkg) = target {
                let pkg_cmd = pkg.cmd.to_string();
                let pkg_name = pkg.name.to_string();
                let lsp_manager = self.lsp_manager.clone();

                match action_type {
                    "uninstall" => {
                        if let Err(e) = lsp_manager.uninstall_server(&pkg_cmd) {
                            self.vim.set_message(format!("Error uninstalling {}: {}", pkg_name, e));
                        } else {
                            self.vim.set_message(format!("{} uninstalled", pkg_name));
                        }
                    }
                    _ => {
                        self.vim.set_message(format!("{} {}...", if action_type == "update" { "Updating" } else { "Installing" }, pkg_name));
                        let pkg_cmd_thread = pkg_cmd.clone();
                        let lsp_manager_thread = lsp_manager.clone();
                        let is_update = action_type == "update";
                        std::thread::spawn(move || {
                            if is_update {
                                let _ = lsp_manager_thread.update_server(&pkg_cmd_thread);
                            } else {
                                let _ = lsp_manager_thread.install_server(&pkg_cmd_thread);
                            }
                        });
                    }
                }
            }
        }
    }

    pub fn enter_treesitter_manager(&mut self) {
        self.vim.mode = Mode::Mason;
        self.vim.mason_tab = 5;
        self.vim.mason_filter.clear();
        self.vim.mason_state.select(Some(0));
    }

    pub fn toggle_comment(&mut self) {
        let path = self.editor.buffer().file_path.clone();
        let ext = path
            .as_ref()
            .and_then(|p| p.extension())
            .and_then(|s| s.to_str())
            .unwrap_or("rs");
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

        self.editor.buffer_mut().push_history();
        let (s_y, e_y) = if let Mode::Visual = self.vim.mode {
            let start = self.vim.selection_start.unwrap();
            let cur = self.editor.cursor();
            if start.y < cur.y {
                (start.y, cur.y)
            } else {
                (cur.y, start.y)
            }
        } else {
            (self.editor.cursor().y, self.editor.cursor().y)
        };

        let all_commented = (s_y..=e_y).all(|y| {
            if let Some(line) = self.editor.buffer().line(y) {
                let line_str = line.to_string();
                line_str.trim().is_empty() || line_str.trim().starts_with(comment_prefix)
            } else {
                true
            }
        });

        for y in s_y..=e_y {
            let Some(line) = self.editor.buffer().line(y) else {
                continue;
            };
            let line_str = line.to_string();
            if line_str.trim().is_empty() {
                continue;
            }
            let line_start_char = self.editor.buffer().text.line_to_char(y);
            if all_commented {
                if let Some(pos) = line_str.find(comment_prefix) {
                    self.editor.buffer_mut().apply_edit(|t| {
                        t.remove(
                            (line_start_char + pos)..(line_start_char + pos + comment_prefix.len()),
                        );
                    });
                }
                if !comment_suffix.is_empty() {
                    if let Some(updated_line) = self.editor.buffer().line(y) {
                        let updated = updated_line.to_string();
                        if let Some(pos) = updated.rfind(comment_suffix) {
                            self.editor.buffer_mut().apply_edit(|t| {
                                t.remove(
                                    (line_start_char + pos)
                                        ..(line_start_char + pos + comment_suffix.len()),
                                );
                            });
                        }
                    }
                }
            } else {
                let indent = line_str.chars().take_while(|c| c.is_whitespace()).count();
                self.editor.buffer_mut().apply_edit(|t| {
                    t.insert(line_start_char + indent, comment_prefix);
                });
                if let Some(curr_line) = self.editor.buffer().line(y) {
                    let end_pos = line_start_char + curr_line.len_chars();
                    let has_newline = curr_line.to_string().ends_with('\n');
                    self.editor.buffer_mut().apply_edit(|t| {
                        t.insert(
                            if has_newline { end_pos - 1 } else { end_pos },
                            comment_suffix,
                        );
                    });
                }
            }
        }
    }

    pub fn handle_args(&mut self, args: Vec<String>) {
        if args.len() > 1 {
            self.editor.buffers.clear();
            self.editor.cursors.clear();
            for arg in &args[1..] {
                let path = PathBuf::from(arg)
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(arg));
                if path.is_dir() {
                    self.explorer.root = path.clone();
                    self.vim.project_root = find_project_root(&path);
                    self.vim.reinit_git();
                    self.explorer.refresh();
                } else {
                    let _ = self.editor.open_file(path.clone());
                    if let Some(buf) = self.editor.buffers.last_mut() {
                        let content = buf.text.to_string();
                        buf.git_signs = self.vim.git_manager.get_signs(&path, &content);
                    }
                }
            }
            if self.editor.buffers.is_empty() {
                self.editor
                    .buffers
                    .push(crate::editor::buffer::Buffer::new());
                self.editor
                    .cursors
                    .push(crate::editor::cursor::Cursor::new());
            }
            self.editor.active_idx = 0;
        }

        if args.len() == 1 {
            self.vim.show_intro = true;
        }
    }

    pub(crate) fn safe_line_to_char(&self, y: usize) -> usize {
        let buf = self.editor.buffer();
        if y >= buf.text.len_lines() {
            buf.text.len_chars()
        } else {
            buf.text.line_to_char(y)
        }
    }

    fn copy_to_system_clipboard(&mut self, text: &str) -> bool {
        if text.is_empty() {
            self.vim
                .set_message("Nothing to copy to clipboard".to_string());
            return false;
        }

        match Clipboard::new().and_then(|mut clipboard| clipboard.set_text(text.to_string())) {
            Ok(()) => true,
            Err(err) => {
                self.vim
                    .set_message(format!("Clipboard copy failed: {}", err));
                false
            }
        }
    }

    fn read_from_system_clipboard(&mut self) -> Option<String> {
        match Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
            Ok(text) => Some(text),
            Err(err) => {
                self.vim
                    .set_message(format!("Clipboard paste failed: {}", err));
                None
            }
        }
    }

    fn clipboard_yank_type(text: &str) -> YankType {
        if text.ends_with('\n') {
            YankType::Line
        } else {
            YankType::Char
        }
    }

    fn notify_buffer_changed(&mut self) {
        if let Some(path) = self.editor.buffer().file_path.clone() {
            if let Some(ext) = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_lowercase())
            {
                let text = self.editor.buffer().text.to_string();
                let _ = self.lsp_manager.did_change(&ext, &path, text);
                self.last_lsp_update = Some(std::time::Instant::now());
            }
        }
    }

    fn insert_text_at_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        self.editor.clamp_cursor();
        let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
        let char_idx = self.safe_line_to_char(y) + x;
        self.editor.buffer_mut().apply_edit(|t| {
            t.insert(char_idx, text);
        });

        let new_char_idx = char_idx + text.chars().count();
        let new_y = self.editor.buffer().text.char_to_line(new_char_idx);
        let new_x = new_char_idx - self.editor.buffer().text.line_to_char(new_y);
        self.editor.cursor_mut().y = new_y;
        self.editor.cursor_mut().x = new_x;

        self.notify_buffer_changed();
        self.refresh_filtered_suggestions();
    }

    pub(crate) fn dispatch_action(&mut self, action: Action, count: usize) {
        match action {
            Action::EnterInsert => {
                self.editor.buffer_mut().push_history();
                self.vim.mode = Mode::Insert;
            }
            Action::EnterInsertLineStart => {
                self.editor.buffer_mut().push_history();
                self.editor.move_to_line_start();
                let (y, _) = (self.editor.cursor().y, self.editor.cursor().x);
                if let Some(line) = self.editor.buffer().line(y) {
                    let line_str = line.to_string();
                    let indent = line_str.chars().take_while(|c| c.is_whitespace()).count();
                    self.editor.cursor_mut().x = indent;
                }
                self.vim.mode = Mode::Insert;
            }
            Action::EnterVisual => {
                self.vim.mode = Mode::Visual;
                let c = self.editor.cursor();
                self.vim.selection_start = Some(Position { x: c.x, y: c.y });
            }
            Action::EnterCommand => {
                self.vim.mode = Mode::Command;
                self.vim.command_buffer.clear();
            }
            Action::EnterSearch => {
                self.vim.mode = Mode::Search;
                self.vim.search_query.clear();
            }
            Action::ExitMode => {
                if self.vim.show_suggestions {
                    self.vim.show_suggestions = false;
                    self.vim.filtered_suggestions.clear();
                    self.vim.suggestions.clear();
                } else {
                    self.vim.mode = Mode::Normal;
                    self.vim.selection_start = None;
                    self.vim.telescope.close();
                }
            }
            Action::EnterMason => {
                self.vim.mode = Mode::Mason;
            }
            Action::EnterTrouble => {
                self.trouble.toggle();
                if self.trouble.visible {
                    self.vim.focus = Focus::Trouble;
                } else {
                    self.vim.focus = Focus::Editor;
                }
            }
            Action::EnterKeymaps => {
                self.vim.mode = Mode::Keymaps;
                self.vim.keymap_filter.clear();
                self.vim.keymap_state.select(Some(0));
            }
            Action::Save => {
                self.save_and_format(None);
            }
            Action::SaveAndQuit => {
                self.save_and_format(None);
                self.should_quit = true;
            }
            Action::QuitWithoutSaving => {
                self.should_quit = true;
            }
            Action::Quit => {
                if self.editor.buffer().modified {
                    self.vim.mode = Mode::Confirm(crate::vim::mode::ConfirmAction::Quit);
                } else if self.editor.buffers.len() > 1 {
                    self.editor.close_current_buffer();
                } else {
                    self.should_quit = true;
                }
            }
            Action::QuitAll => {
                let any_modified = self.editor.buffers.iter().any(|b| b.modified);
                if any_modified {
                    self.vim.mode = Mode::Confirm(crate::vim::mode::ConfirmAction::Quit);
                } else {
                    self.should_quit = true;
                }
            }
            Action::CloseBuffer => {
                if self.editor.buffer().modified {
                    self.vim.mode = Mode::Confirm(crate::vim::mode::ConfirmAction::CloseBuffer);
                } else {
                    self.editor.close_current_buffer();
                    self.sync_explorer();
                }
            }
            Action::NextBuffer => {
                if self.vim.focus == Focus::Editor {
                    self.editor.next_buffer();
                    self.sync_explorer();
                }
            }
            Action::PrevBuffer => {
                if self.vim.focus == Focus::Editor {
                    self.editor.prev_buffer();
                    self.sync_explorer();
                }
            }
            Action::ReloadFile => {
                if let Some(path) = self.editor.buffer().file_path.clone() {
                    let _ = self.editor.open_file(path);
                }
            }
            Action::MoveLeft => {
                for _ in 0..count {
                    self.editor.move_left();
                }
            }
            Action::MoveRight => {
                for _ in 0..count {
                    self.editor.move_right();
                }
            }
            Action::MoveUp => match self.vim.focus {
                Focus::Editor => {
                    for _ in 0..count {
                        self.editor.move_up();
                    }
                }
                Focus::Explorer => {
                    for _ in 0..count {
                        self.explorer.move_up();
                    }
                }
                Focus::Trouble => {
                    for _ in 0..count {
                        self.trouble.move_up();
                    }
                }
            },
            Action::MoveDown => match self.vim.focus {
                Focus::Editor => {
                    for _ in 0..count {
                        self.editor.move_down();
                    }
                }
                Focus::Explorer => {
                    for _ in 0..count {
                        self.explorer.move_down();
                    }
                }
                Focus::Trouble => {
                    for _ in 0..count {
                        self.trouble.move_down();
                    }
                }
            },
            Action::MoveWordForward => {
                for _ in 0..count {
                    self.editor.move_word_forward();
                }
            }
            Action::MoveWordBackward => {
                for _ in 0..count {
                    self.editor.move_word_backward();
                }
            }
            Action::MoveWordEnd => {
                for _ in 0..count {
                    self.editor.move_word_end();
                }
            }
            Action::MoveLineStart => {
                self.editor.move_to_line_start();
            }
            Action::MoveLineEnd => {
                self.editor.move_to_line_end();
            }
            Action::JumpToFirstLine => match self.vim.focus {
                Focus::Editor => self.editor.jump_to_first_line(),
                Focus::Explorer => {
                    self.explorer.selected_idx = 0;
                }
                Focus::Trouble => {
                    self.trouble.selected_idx = 0;
                }
            },
            Action::JumpToLastLine => match self.vim.focus {
                Focus::Editor => self.editor.jump_to_last_line(),
                Focus::Explorer => {
                    if !self.explorer.entries.is_empty() {
                        self.explorer.selected_idx = self.explorer.entries.len().saturating_sub(1);
                    }
                }
                Focus::Trouble => {
                    if !self.trouble.items.is_empty() {
                        self.trouble.selected_idx = self.trouble.items.len().saturating_sub(1);
                    }
                }
            },
            Action::MovePageUp => {
                let area = self.terminal.size().unwrap();
                let h = area.height.saturating_sub(2) as usize;
                match self.vim.focus {
                    Focus::Editor => self.editor.move_page_up(h),
                    Focus::Explorer => self.explorer.move_page_up(h.saturating_sub(3)),
                    Focus::Trouble => {}
                }
            }
            Action::MovePageDown => {
                let area = self.terminal.size().unwrap();
                let h = area.height.saturating_sub(2) as usize;
                match self.vim.focus {
                    Focus::Editor => self.editor.move_page_down(h),
                    Focus::Explorer => self.explorer.move_page_down(h.saturating_sub(3)),
                    Focus::Trouble => {}
                }
            }
            Action::DeleteChar => {
                let y = self.editor.cursor().y;
                let x = self.editor.cursor().x;
                self.vim.register = self.editor.delete_selection(x, y, x, y);
                self.vim.yank_type = YankType::Char;
            }
            Action::DeleteCharBefore => {
                let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                if x > 0 {
                    let idx = self.editor.buffer().text.line_to_char(y) + x;
                    self.editor.buffer_mut().apply_edit(|t| {
                        t.remove((idx - 1)..idx);
                    });
                    self.editor.cursor_mut().x -= 1;
                    if let Some(path) = self.editor.buffer().file_path.clone() {
                        if let Some(ext) = path
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_lowercase())
                        {
                            let text = self.editor.buffer().text.to_string();
                            let _ = self.lsp_manager.did_change(&ext, &path, text);
                            let _ = self.lsp_manager.request_completions(
                                &ext,
                                &path,
                                y,
                                x - 1,
                                CompletionTriggerKind::INVOKED,
                                None,
                            );
                        }
                    }
                    self.refresh_filtered_suggestions();
                } else if y > 0 {
                    let prev_line_idx = y - 1;
                    let prev_line = self.editor.buffer().text.line(prev_line_idx);
                    let prev_line_len = prev_line.len_chars();
                    let has_newline = prev_line
                        .chars()
                        .last()
                        .is_some_and(|c| c == '\n' || c == '\r');
                    let new_x = if has_newline {
                        prev_line_len - 1
                    } else {
                        prev_line_len
                    };

                    let char_idx = self.editor.buffer().text.line_to_char(y);
                    self.editor.buffer_mut().apply_edit(|t| {
                        t.remove((char_idx - 1)..char_idx);
                    });

                    self.editor.cursor_mut().y -= 1;
                    self.editor.cursor_mut().x = new_x;

                    if let Some(path) = self.editor.buffer().file_path.clone() {
                        if let Some(ext) = path
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_lowercase())
                        {
                            let text = self.editor.buffer().text.to_string();
                            let _ = self.lsp_manager.did_change(&ext, &path, text);
                        }
                    }
                }
            }
            Action::DeleteLine => {
                let mut deleted = String::new();
                for _ in 0..count {
                    let y = self.editor.cursor().y;
                    deleted.push_str(&self.editor.delete_line(y));
                }
                self.vim.register = deleted;
                self.vim.yank_type = YankType::Line;
            }
            Action::YankLine => {
                let mut yanked = String::new();
                let start_y = self.editor.cursor().y;
                for i in 0..count {
                    if let Some(line) = self.editor.buffer().line(start_y + i) {
                        yanked.push_str(&line.to_string());
                    }
                }
                self.vim.register = yanked;
                self.vim.yank_type = YankType::Line;
                if self.copy_to_system_clipboard(&self.vim.register.clone()) {
                    self.vim
                        .set_message(format!("{} lines yanked to clipboard", count));
                }
            }
            Action::CopyToClipboard => {
                let copied = if let Mode::Visual = self.vim.mode {
                    if let Some(start) = self.vim.selection_start {
                        let cur = self.editor.cursor();
                        self.editor.yank(start.x, start.y, cur.x, cur.y)
                    } else {
                        String::new()
                    }
                } else {
                    let y = self.editor.cursor().y;
                    self.editor
                        .buffer()
                        .line(y)
                        .map(|line| line.to_string())
                        .unwrap_or_default()
                };

                if !copied.is_empty() && self.copy_to_system_clipboard(&copied) {
                    self.vim.register = copied.clone();
                    self.vim.yank_type = Self::clipboard_yank_type(&copied);
                    self.vim.set_message("Copied to clipboard".to_string());
                    if let Mode::Visual = self.vim.mode {
                        self.vim.mode = Mode::Normal;
                        self.vim.selection_start = None;
                    }
                }
            }
            Action::PasteAfter => {
                let t = self.vim.register.clone();
                let y = self.vim.yank_type;
                self.editor.paste_after(&t, y);
            }
            Action::PasteBefore => {
                let t = self.vim.register.clone();
                let y = self.vim.yank_type;
                self.editor.paste_before(&t, y);
            }
            Action::PasteFromClipboard => {
                if let Some(text) = self.read_from_system_clipboard() {
                    self.vim.register = text.clone();
                    self.vim.yank_type = Self::clipboard_yank_type(&text);
                    if let Mode::Visual = self.vim.mode {
                        if let Some(start) = self.vim.selection_start {
                            let cur = self.editor.cursor();
                            self.editor.delete_selection(start.x, start.y, cur.x, cur.y);
                        }
                        self.vim.mode = Mode::Normal;
                        self.vim.selection_start = None;
                    }
                    self.insert_text_at_cursor(&text);
                    self.vim.set_message("Pasted from clipboard".to_string());
                }
            }
            Action::Undo => {
                self.editor.undo();
            }
            Action::Redo => {
                self.editor.redo();
            }
            Action::ToggleComment => {
                self.toggle_comment();
            }
            Action::OpenLineBelow => {
                self.editor.buffer_mut().push_history();
                self.editor.open_line_below();
                self.vim.mode = Mode::Insert;
            }
            Action::OpenLineAbove => {
                self.editor.buffer_mut().push_history();
                self.editor.open_line_above();
                self.vim.mode = Mode::Insert;
            }
            Action::DeleteSelection => {
                if let Mode::Visual = self.vim.mode {
                    let start = self.vim.selection_start.unwrap();
                    let cur = self.editor.cursor();
                    self.vim.register =
                        self.editor.delete_selection(start.x, start.y, cur.x, cur.y);
                    self.vim.yank_type = YankType::Char;
                    self.vim.mode = Mode::Normal;
                    self.vim.selection_start = None;
                } else {
                    let y = self.editor.cursor().y;
                    let x = self.editor.cursor().x;
                    self.editor.delete_selection(x, y, x, y);
                    self.vim.mode = Mode::Insert;
                }
            }
            Action::Indent => {
                let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                let idx = self.editor.buffer().text.line_to_char(y) + x;
                let spaces = " ".repeat(self.vim.config.tabstop);
                self.editor.buffer_mut().apply_edit(|t| {
                    t.insert(idx, &spaces);
                });
                self.editor.cursor_mut().x += self.vim.config.tabstop;
            }
            Action::TelescopeFiles => {
                self.vim.telescope.open(
                    crate::vim::mode::TelescopeKind::Files,
                    self.vim.project_root.clone(),
                    &self.editor,
                );
                self.vim.mode = Mode::Telescope(crate::vim::mode::TelescopeKind::Files);
            }
            Action::TelescopeLiveGrep => {
                self.vim.telescope.open(
                    crate::vim::mode::TelescopeKind::Words,
                    self.vim.project_root.clone(),
                    &self.editor,
                );
                self.vim.mode = Mode::Telescope(crate::vim::mode::TelescopeKind::Words);
            }
            Action::TelescopeBuffers => {
                self.vim.telescope.open(
                    crate::vim::mode::TelescopeKind::Buffers,
                    self.vim.project_root.clone(),
                    &self.editor,
                );
                self.vim.mode = Mode::Telescope(crate::vim::mode::TelescopeKind::Buffers);
            }
            Action::TelescopeThemes => {
                self.vim.telescope.open(
                    crate::vim::mode::TelescopeKind::Themes,
                    self.vim.project_root.clone(),
                    &self.editor,
                );
                self.vim.mode = Mode::Telescope(crate::vim::mode::TelescopeKind::Themes);
            }
            Action::LspDefinition => {
                if let Some(path) = self.editor.buffer().file_path.clone() {
                    if let Some(ext) = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_lowercase())
                    {
                        let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                        match self.lsp_manager.request_definition(&ext, &path, y, x) {
                            Ok(id) => {
                                self.vim.definition_request_id = Some(id);
                            }
                            Err(e) => {
                                self.vim.set_message(format!("LSP Error: {}", e));
                            }
                        }
                    }
                }
            }
            Action::ToggleExplorer => {
                if self.explorer.visible {
                    if self.vim.focus == Focus::Explorer {
                        self.explorer.visible = false;
                        self.vim.focus = Focus::Editor;
                    } else {
                        self.vim.focus = Focus::Explorer;
                        self.sync_explorer();
                    }
                } else {
                    self.explorer.visible = true;
                    self.explorer.init_root();
                    self.vim.focus = Focus::Explorer;
                    self.sync_explorer();
                }
            }
            Action::ToggleRelativeNumber => {
                self.vim.relative_number = !self.vim.relative_number;
            }
            Action::ToggleTrouble => {
                self.trouble.toggle();
                if self.trouble.visible {
                    self.vim.focus = Focus::Trouble;
                } else {
                    self.vim.focus = Focus::Editor;
                }
            }
            Action::ToggleAutoformat => {
                self.vim.config.disable_autoformat = !self.vim.config.disable_autoformat;
                self.vim.set_message(format!(
                    "Autoformat {}",
                    if self.vim.config.disable_autoformat {
                        "disabled"
                    } else {
                        "enabled"
                    }
                ));
            }
            Action::GitBlame => {
                self.vim.blame_popup = Some("Git Blame: You (just now) - placeholder".to_string());
            }
            Action::ToggleFold => {
                self.editor.toggle_fold(&self.vim.folding_ranges);
            }
            Action::NextHunk => {
                self.editor.jump_to_next_hunk();
            }
            Action::PrevHunk => {
                self.editor.jump_to_prev_hunk();
            }
            Action::Format => {
                self.format_buffer(BackgroundFileOp::Format);
            }
            Action::ExplorerExpand => {
                if let Some(entry) = self.explorer.selected_entry() {
                    if entry.is_dir {
                        self.explorer.expand();
                    } else {
                        let path = entry.path.clone();
                        if let Err(e) = self.editor.open_file(path.clone()) {
                            self.vim.set_message(format!("Error: {}", e));
                        } else {
                            self.vim.focus = Focus::Editor;
                            if let Some(buf) = self.editor.buffers.last_mut() {
                                let content = buf.text.to_string();
                                buf.git_signs = self.vim.git_manager.get_signs(&path, &content);
                            }
                        }
                    }
                }
            }
            Action::ExplorerCollapse => {
                self.explorer.collapse();
            }
            Action::ExplorerToggleExpand => {
                if let Some(entry) = self.explorer.selected_entry() {
                    if entry.is_dir {
                        self.explorer.toggle_expand();
                    } else {
                        let path = entry.path.clone();
                        if let Err(e) = self.editor.open_file(path.clone()) {
                            self.vim.set_message(format!("Error: {}", e));
                        } else {
                            self.vim.focus = Focus::Editor;
                            if let Some(buf) = self.editor.buffers.last_mut() {
                                let content = buf.text.to_string();
                                buf.git_signs = self.vim.git_manager.get_signs(&path, &content);
                            }
                        }
                    }
                }
            }
            Action::ExplorerAdd => {
                self.vim.mode = Mode::ExplorerInput(ExplorerInputType::Add);
                self.vim.input_buffer.clear();
            }
            Action::ExplorerRename => {
                self.vim.mode = Mode::ExplorerInput(ExplorerInputType::Rename);
                self.vim.input_buffer.clear();
            }
            Action::ExplorerDelete => {
                self.vim.mode = Mode::ExplorerInput(ExplorerInputType::DeleteConfirm);
                self.vim.input_buffer.clear();
            }
            Action::ExplorerMove => {
                self.vim.mode = Mode::ExplorerInput(ExplorerInputType::Move);
                self.vim.input_buffer.clear();
            }
            Action::ExplorerFilter => {
                self.vim.mode = Mode::ExplorerInput(ExplorerInputType::Filter);
                self.vim.input_buffer.clear();
            }
            Action::ExplorerOpenSystem => {
                self.explorer.open_in_system_explorer();
            }
            Action::ExplorerToggleHidden => {
                self.explorer.show_hidden = !self.explorer.show_hidden;
                self.explorer.refresh();
            }
            Action::ExplorerToggleIgnored => {
                self.explorer.show_ignored = !self.explorer.show_ignored;
                self.explorer.refresh();
            }
            Action::ExplorerCloseAll => {
                self.explorer.close_all();
            }
            Action::SelectNext => match self.vim.mode {
                Mode::Telescope(_) => self.vim.telescope.move_down(),
                Mode::Mason => {
                    let i = self.vim.mason_state.selected().unwrap_or(0);
                    self.vim.mason_state.select(Some(i + 1));
                }
                Mode::Keymaps => {
                    let i = self.vim.keymap_state.selected().unwrap_or(0);
                    self.vim.keymap_state.select(Some(i + 1));
                }
                Mode::Insert => {
                    if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                        self.vim.selected_suggestion = (self.vim.selected_suggestion + 1)
                            % self.vim.filtered_suggestions.len();
                        self.vim
                            .suggestion_state
                            .select(Some(self.vim.selected_suggestion));
                    } else {
                        self.dispatch_action(Action::Indent, 1);
                    }
                }
                _ => {}
            },
            Action::SelectPrev => match self.vim.mode {
                Mode::Telescope(_) => self.vim.telescope.move_up(),
                Mode::Mason => {
                    let i = self.vim.mason_state.selected().unwrap_or(0);
                    if i > 0 {
                        self.vim.mason_state.select(Some(i - 1));
                    }
                }
                Mode::Keymaps => {
                    let i = self.vim.keymap_state.selected().unwrap_or(0);
                    if i > 0 {
                        self.vim.keymap_state.select(Some(i - 1));
                    }
                }
                Mode::Insert => {
                    if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                        if self.vim.selected_suggestion > 0 {
                            self.vim.selected_suggestion -= 1;
                        } else {
                            self.vim.selected_suggestion = self.vim.filtered_suggestions.len() - 1;
                        }
                        self.vim
                            .suggestion_state
                            .select(Some(self.vim.selected_suggestion));
                    } else {
                        self.editor.move_up();
                    }
                }
                _ => {}
            },
            Action::Confirm => {
                if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                    let selected = &self.vim.filtered_suggestions[self.vim.selected_suggestion];
                    let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                    let line_start_char = self.editor.buffer().text.line_to_char(y);

                    let mut start_x = x;
                    let mut end_x = x;
                    let mut insert_text = selected
                        .insert_text
                        .clone()
                        .unwrap_or(selected.label.clone());

                    let mut use_fallback = true;
                    if let Some(edit) = &selected.text_edit {
                        match edit {
                            lsp_types::CompletionTextEdit::Edit(te) => {
                                start_x = te.range.start.character as usize;
                                end_x = te.range.end.character as usize;
                                insert_text = te.new_text.clone();
                                use_fallback = false;
                            }
                            lsp_types::CompletionTextEdit::InsertAndReplace(ir) => {
                                start_x = ir.insert.start.character as usize;
                                end_x = ir.insert.end.character as usize;
                                insert_text = ir.new_text.clone();
                                use_fallback = false;
                            }
                        }
                    }

                    if use_fallback {
                        if let Some(line) = self.editor.buffer().line(y) {
                            let line_str = line.to_string();
                            let chars: Vec<char> = line_str.chars().collect();
                            while start_x > 0
                                && chars
                                    .get(start_x - 1)
                                    .is_some_and(|&c| c.is_alphanumeric() || c == '_' || c == '$')
                            {
                                start_x -= 1;
                            }
                            while end_x < chars.len()
                                && chars
                                    .get(end_x)
                                    .is_some_and(|&c| c.is_alphanumeric() || c == '_' || c == '$')
                            {
                                end_x += 1;
                            }
                        }
                    }

                    if insert_text.contains('$') {
                        insert_text = insert_text
                            .replace("$0", "")
                            .replace("$1", "")
                            .replace("${1:", "")
                            .replace('}', "");
                    }

                    self.editor.buffer_mut().apply_edit(|t| {
                        let remove_start = line_start_char + start_x;
                        let remove_end = line_start_char + end_x;
                        let len = t.len_chars();
                        let safe_start = remove_start.min(len);
                        let safe_end = remove_end.min(len);
                        if safe_end > safe_start {
                            t.remove(safe_start..safe_end);
                        }
                        t.insert(safe_start, &insert_text);
                    });

                    self.editor.cursor_mut().x = start_x + insert_text.len();
                    self.vim.show_suggestions = false;
                    self.vim.suggestions.clear();
                    self.vim.filtered_suggestions.clear();
                } else if let Mode::Telescope(_) = self.vim.mode {
                    if let Some(result) = self
                        .vim
                        .telescope
                        .results
                        .get(self.vim.telescope.selected_idx)
                    {
                        match self.vim.telescope.kind {
                            crate::vim::mode::TelescopeKind::Themes => {
                                self.editor.set_theme(&result.path.to_string_lossy());
                            }
                            crate::vim::mode::TelescopeKind::Buffers => {
                                if let Some(idx) = result.buffer_idx {
                                    self.editor.active_idx = idx;
                                    self.sync_explorer();
                                }
                            }
                            _ => {
                                let path = result.path.clone();
                                let line = result.line_number.unwrap_or(1).saturating_sub(1);
                                if let Err(e) = self.editor.open_file(path) {
                                    self.vim.set_message(format!("Error: {}", e));
                                } else {
                                    self.editor.cursor_mut().y = line;
                                    self.editor.cursor_mut().x = 0;
                                    self.sync_explorer();
                                }
                            }
                        }
                    }
                    self.vim.mode = Mode::Normal;
                    self.vim.telescope.close();
                } else if self.vim.focus == Focus::Explorer {
                    if let Some(entry) = self.explorer.selected_entry() {
                        if entry.is_dir {
                            self.explorer.toggle_expand();
                        } else {
                            let path = entry.path.clone();
                            if let Err(e) = self.editor.open_file(path.clone()) {
                                self.vim.set_message(format!("Error: {}", e));
                            } else {
                                self.vim.focus = Focus::Editor;
                                if let Some(buf) = self.editor.buffers.last_mut() {
                                    let content = buf.text.to_string();
                                    buf.git_signs = self.vim.git_manager.get_signs(&path, &content);
                                }
                            }
                        }
                    }
                } else if self.vim.focus == Focus::Trouble {
                    if let Some(item) = self.trouble.selected_item() {
                        let path = item.path.clone();
                        let line = item.line;
                        let col = item.col;
                        if let Err(e) = self.editor.open_file(path.clone()) {
                            self.vim.set_message(format!("Error: {}", e));
                        } else {
                            self.editor.cursor_mut().y = line;
                            self.editor.cursor_mut().x = col;
                            self.vim.focus = Focus::Editor;
                            if let Some(buf) = self.editor.buffers.last_mut() {
                                let content = buf.text.to_string();
                                buf.git_signs = self.vim.git_manager.get_signs(&path, &content);
                            }
                        }
                    }
                } else if let Mode::Insert = self.vim.mode {
                    let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                    let idx = self.editor.buffer().text.line_to_char(y) + x;
                    self.editor.buffer_mut().apply_edit(|t| {
                        t.insert(idx, "\n");
                    });
                    self.editor.cursor_mut().y += 1;
                    self.editor.cursor_mut().x = 0;
                }
            }
            _ => {}
        }
    }
}
