use super::*;

impl App {
    pub fn run(&mut self) -> Result<()> {
        loop {
            let area = self.terminal.size()?;
            let visible_height = area.height.saturating_sub(2) as usize;

            if let Some(time) = self.vim.message_time {
                if time.elapsed().as_secs() >= 3 {
                    self.vim.message = None;
                    self.vim.message_time = None;
                }
            }

            if self.vim.last_git_update.is_none()
                || self.vim.last_git_update.unwrap().elapsed() > Duration::from_secs(5)
            {
                self.vim.git_info = update_git_info(&self.vim.project_root);
                for buffer in &mut self.editor.buffers {
                    if let Some(path) = &buffer.file_path {
                        let text = buffer.text.to_string();
                        buffer.git_signs = self.vim.git_manager.get_signs(path, &text);
                    }
                }
                self.vim.last_git_update = Some(Instant::now());
            }

            let mut explorer_needs_refresh = false;
            let mut buffers_to_reload = Vec::new();

            while let Ok(res) = self.rx.try_recv() {
                if let Ok(event) = res {
                    use notify::EventKind;
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                            explorer_needs_refresh = true;
                            for path in event.paths {
                                if let Some(active_path) = self.editor.buffer().file_path.as_ref() {
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

            if explorer_needs_refresh && self.explorer.visible {
                self.explorer.refresh();
            }

            // Drain completed Nucleus operation messages
            let nucleus_msgs: Vec<(String, bool)> = {
                let mut msgs = self.lsp_manager.op_messages.lock().unwrap();
                msgs.drain(..).collect()
            };
            for (msg, _ok) in nucleus_msgs {
                self.vim.set_message(msg);
            }

            // Hot-reload: invalidate highlight config cache for freshly installed parsers
            // so open buffers pick up the new grammar without a restart.
            let newly_installed: Vec<String> = {
                let mut q = self.lsp_manager.installed_parsers.lock().unwrap();
                q.drain(..).collect()
            };
            for lang_name in newly_installed {
                self.editor.highlighter.invalidate_lang_config(&lang_name);
                // Find the file extensions that map to this language and reset those buffer caches.
                let exts: Vec<&str> = crate::editor::treesitter::LANGUAGES
                    .iter()
                    .find(|l| l.name == lang_name)
                    .map(|l| l.file_types)
                    .unwrap_or(&[]).to_vec();
                for cache_idx in 0..self.editor.caches.len() {
                    let matches = self.editor.buffers[cache_idx]
                        .file_path
                        .as_ref()
                        .and_then(|p| p.extension())
                        .and_then(|e| e.to_str())
                        .map(|e| exts.contains(&e))
                        .unwrap_or(false);
                    if matches {
                        self.editor.caches[cache_idx].last_version = usize::MAX;
                    }
                }
            }

            while let Ok(AsyncFileResult { path, ext: _ext, result: async_res, git_signs: signs, op: _op }) = self.async_rx.try_recv() {
                self.vim.lsp_status = LspStatus::None;

                match async_res {
                    AsyncResult::Format(res) => match res {
                        Ok(formatted) => {
                            if let Some(buf_idx) = self
                                .editor
                                .buffers
                                .iter()
                                .position(|b| b.file_path.as_ref() == Some(&path))
                            {
                                self.editor.buffers[buf_idx].text =
                                    ropey::Rope::from_str(&formatted);
                                self.editor.buffers[buf_idx].git_signs = signs;
                                if buf_idx == self.editor.active_idx {
                                    self.editor.clamp_cursor();
                                }
                                self.vim.set_message(format!(
                                    "Formatted \"{}\"",
                                    path.to_string_lossy()
                                ));
                            }
                        }
                        Err(e) => {
                            self.vim.set_message(format!("Format Error: {}", e));
                        }
                    },
                    AsyncResult::Save(res) => match res {
                        Ok(final_text) => {
                            if let Some(buf_idx) = self
                                .editor
                                .buffers
                                .iter()
                                .position(|b| b.file_path.as_ref() == Some(&path))
                            {
                                {
                                    let buf = &mut self.editor.buffers[buf_idx];
                                    buf.text = ropey::Rope::from_str(&final_text);
                                    buf.modified = false;
                                    buf.git_signs = signs;
                                    if let Ok(meta) = std::fs::metadata(&path) {
                                        buf.last_modified = meta.modified().ok();
                                    }
                                }
                                if buf_idx == self.editor.active_idx {
                                    self.editor.clamp_cursor();
                                }

                                let buf = &self.editor.buffers[buf_idx];
                                let line_count = buf.len_lines();
                                let char_count = buf.text.len_chars();
                                self.vim.set_message(format!(
                                    "\"{}\" {}L, {}C written",
                                    path.to_string_lossy(),
                                    line_count,
                                    char_count
                                ));
                            }
                        }
                        Err(e) => {
                            self.vim.set_message(format!("Error saving file: {}", e));
                        }
                    },
                }
            }

            if !buffers_to_reload.is_empty() && self.vim.mode == Mode::Normal {
                self.vim.mode = Mode::Confirm(crate::vim::mode::ConfirmAction::ReloadFile);
            }

            if let Some(path) = self.editor.buffer().file_path.clone() {
                if let Some(ext) = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_lowercase())
                {
                    let _ = self
                        .lsp_manager
                        .start_client(&ext, self.vim.project_root.clone());
                }
            }

            let mut messages_to_process = Vec::new();
            {
                let clients = self.lsp_manager.clients.lock().unwrap();
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
                                let mut clients = self.lsp_manager.clients.lock().unwrap();
                                if let Some(ext_clients) = clients.get_mut(&ext) {
                                    for (client, state, c) in ext_clients.iter_mut() {
                                        if c == &cmd {
                                            *state = crate::lsp::ClientState::Ready;
                                            let _ = client.send_notification(
                                                "initialized",
                                                serde_json::json!({}),
                                            );
                                            newly_ready_clients.push((ext.clone(), cmd.clone()));
                                        }
                                    }
                                }
                            } else if Some(id) == self.vim.definition_request_id {
                                self.vim.definition_request_id = None;
                                if let Ok(value) = serde_json::from_value::<GotoDefinitionResponse>(
                                    resp.result.unwrap_or_default(),
                                ) {
                                    if let GotoDefinitionResponse::Scalar(loc) = value {
                                        if let Ok(file_path) = loc.uri.to_file_path() {
                                            let path = PathBuf::from(file_path);
                                            let pos = Position {
                                                x: loc.range.start.character as usize,
                                                y: loc.range.start.line as usize,
                                            };
                                            let _ = self.editor.open_file(path);
                                            self.editor.cursor_mut().y = pos.y;
                                            self.editor.cursor_mut().x = pos.x;
                                            self.sync_explorer();
                                        }
                                    }
                                }
                            } else if let Ok(value) = serde_json::from_value::<CompletionResponse>(
                                resp.result.unwrap_or_default(),
                            ) {
                                match value {
                                    CompletionResponse::Array(items) => {
                                        self.vim.suggestions = items;
                                    }
                                    CompletionResponse::List(list) => {
                                        self.vim.suggestions = list.items;
                                    }
                                }
                                self.refresh_filtered_suggestions();
                            }
                        }
                    }
                    lsp_server::Message::Notification(notif) => {
                        if notif.method == "textDocument/publishDiagnostics" {
                            if let Ok(params) =
                                serde_json::from_value::<PublishDiagnosticsParams>(notif.params)
                            {
                                let mut diagnostics = self.lsp_manager.diagnostics.lock().unwrap();
                                let file_diags = diagnostics.entry(params.uri).or_default();
                                file_diags.insert(cmd, params.diagnostics);
                            }
                        }
                    }
                    _ => {}
                }
            }

            for (ext, cmd) in newly_ready_clients {
                for buf in &self.editor.buffers {
                    if let Some(path) = &buf.file_path {
                        if path
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.to_lowercase())
                            == Some(ext.clone())
                        {
                            let text = buf.text.to_string();
                            let _ = self.lsp_manager.did_open(&ext, path, text, Some(&cmd));
                        }
                    }
                }
            }

            if event::poll(Duration::from_millis(10))? {
                while event::poll(Duration::from_millis(0))? {
                    let event = event::read()?;
                    if let Event::Mouse(mouse) = &event {
                        match mouse.kind {
                            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                                let is_up = matches!(mouse.kind, MouseEventKind::ScrollUp);
                                if let Mode::Telescope(_) = self.vim.mode {
                                    if is_up {
                                        self.vim.telescope.scroll_preview_up(3);
                                    } else {
                                        self.vim.telescope.scroll_preview_down(3);
                                    }
                                } else if self.explorer.visible
                                    && mouse.column < self.explorer.width
                                {
                                    if is_up {
                                        self.explorer.move_up();
                                    } else {
                                        self.explorer.move_down();
                                    }
                                } else if self.trouble.visible
                                    && mouse.row >= area.height.saturating_mul(7) / 10
                                {
                                    if is_up {
                                        self.trouble.move_up();
                                    } else {
                                        self.trouble.move_down();
                                    }
                                } else if is_up {
                                    self.editor.move_up();
                                } else {
                                    self.editor.move_down();
                                }
                            }
                            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                                let root_chunks = Layout::default()
                                    .direction(Direction::Vertical)
                                    .constraints([
                                        Constraint::Min(1),
                                        Constraint::Length(if self.vim.config.laststatus >= 2 {
                                            1
                                        } else {
                                            0
                                        }),
                                        Constraint::Length(1),
                                    ])
                                    .split(area.into());

                                let main_chunks = Layout::default()
                                    .direction(Direction::Horizontal)
                                    .constraints(if self.explorer.visible {
                                        [
                                            Constraint::Length(self.explorer.width),
                                            Constraint::Min(1),
                                        ]
                                    } else {
                                        [Constraint::Length(0), Constraint::Min(1)]
                                    })
                                    .split(root_chunks[0]);

                                if self.explorer.visible
                                    && mouse.column < self.explorer.width
                                    && mouse.row < root_chunks[0].height
                                {
                                    let list_start_y = 3;
                                    if mouse.row >= list_start_y {
                                        let click_row = (mouse.row - list_start_y) as usize;
                                        let target_idx = self.explorer.scroll_y + click_row;
                                        if target_idx < self.explorer.entries.len() {
                                            let now = Instant::now();
                                            let is_double_click =
                                                if let Some((last_time, last_col, last_row)) =
                                                    self.last_click
                                                {
                                                    now.duration_since(last_time).as_millis() < 500
                                                        && last_col == mouse.column
                                                        && last_row == mouse.row
                                                } else {
                                                    false
                                                };

                                            self.explorer.selected_idx = target_idx;
                                            self.vim.focus = Focus::Explorer;

                                            if is_double_click {
                                                self.dispatch_action(
                                                    Action::ExplorerToggleExpand,
                                                    1,
                                                );
                                                self.last_click = None;
                                            } else {
                                                self.last_click =
                                                    Some((now, mouse.column, mouse.row));
                                            }
                                        }
                                    }
                                } else if mouse.row < root_chunks[0].height {
                                    let editor_trouble_chunks = Layout::default()
                                        .direction(Direction::Vertical)
                                        .constraints(if self.trouble.visible {
                                            [Constraint::Percentage(70), Constraint::Percentage(30)]
                                        } else {
                                            [Constraint::Percentage(100), Constraint::Percentage(0)]
                                        })
                                        .split(main_chunks[1]);

                                    let (panes, _) = crate::ui::TerminalUi::get_panes_and_borders(&self.vim.pane_layout, editor_trouble_chunks[0], self.vim.focused_pane_id);
                                    let mut clicked_pane = None;
                                    for p in panes {
                                        if mouse.column >= p.0.x && mouse.column < p.0.x + p.0.width && mouse.row >= p.0.y && mouse.row < p.0.y + p.0.height {
                                            clicked_pane = Some(p);
                                            break;
                                        }
                                    }

                                    if self.trouble.visible
                                        && mouse.row >= editor_trouble_chunks[1].y
                                    {
                                        self.vim.focus = Focus::Trouble;
                                        let _click_row =
                                            (mouse.row - editor_trouble_chunks[1].y) as usize;
                                    } else if let Some((rect, buf_idx, _)) = clicked_pane {
                                        self.vim.focus = Focus::Editor;
                                        
                                        // Update focused pane
                                        let all_panes = self.vim.pane_layout.get_all_panes();
                                        if let Some(target_pane) = all_panes.iter().find(|p| p.buffer_idx == buf_idx) {
                                            self.vim.focused_pane_id = target_pane.id;
                                            self.editor.active_idx = target_pane.buffer_idx;
                                        }

                                        let gutter: u16 = match (
                                            self.vim.show_number || self.vim.relative_number,
                                            self.vim.config.signcolumn,
                                        ) {
                                            (true, true) => 7,
                                            (true, false) => 5,
                                            (false, true) => 3,
                                            (false, false) => 0,
                                        };
                                        let sidx = self.editor.active_idx;
                                        if sidx < self.editor.cursors.len() {
                                            let scroll = self.editor.cursors[sidx].scroll_y;
                                            let row = (mouse.row.saturating_sub(rect.y)) as usize + scroll;
                                            let display_col = mouse.column.saturating_sub(rect.x + gutter) as usize;
                                            let buf_len = self.editor.buffers[sidx].len_lines();
                                            let row = row.min(buf_len.saturating_sub(1));

                                            // Convert display column → char index, accounting for tabs and wide chars
                                            let char_col = if let Some(line) = self.editor.buffers[sidx].line(row) {
                                                let mut cells = 0usize;
                                                let mut idx = 0usize;
                                                for c in line.chars() {
                                                    if c == '\n' || c == '\r' { break; }
                                                    let w = if c == '\t' { self.vim.config.tabstop } else {
                                                        unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
                                                    };
                                                    if cells + w > display_col { break; }
                                                    cells += w;
                                                    idx += 1;
                                                }
                                                idx
                                            } else {
                                                0
                                            };

                                            self.editor.cursors[sidx].y = row;
                                            self.editor.cursors[sidx].x = char_col;
                                            self.editor.clamp_cursor();
                                        }
                                        
                                        let buf_line = self.editor.cursors[sidx].y;
                                        let buf_col = self.editor.cursors[sidx].x;
                                        
                                        {
                                            let now = Instant::now();
                                            let is_double =
                                                if let Some((last_time, last_col, last_row)) =
                                                    self.last_click
                                                {
                                                    now.duration_since(last_time).as_millis() < 500
                                                        && last_col == mouse.column
                                                        && last_row == mouse.row
                                                } else {
                                                    false
                                                };

                                            if is_double {
                                                if let Some((word_start, word_end)) =
                                                    self.word_at_editor_pos(buf_line, buf_col)
                                                {
                                                    self.editor.cursor_mut().x = word_end;
                                                    self.vim.selection_start =
                                                        Some(Position { x: word_start, y: buf_line });
                                                    self.vim.mode = Mode::Visual;
                                                    self.vim.yank_type = YankType::Char;
                                                }
                                                self.last_click = None;
                                                self.is_dragging = false;
                                                self.drag_anchor = None;
                                            } else {
                                                self.last_click =
                                                    Some((now, mouse.column, mouse.row));
                                                self.drag_anchor =
                                                    Some(Position { x: buf_col, y: buf_line });
                                                self.is_dragging = false;
                                                if self.vim.mode == Mode::Visual {
                                                    self.vim.mode = Mode::Normal;
                                                    self.vim.selection_start = None;
                                                }
                                            }
                                        }
                                    }
                                } else if root_chunks[1].height > 0 && mouse.row == root_chunks[1].y
                                {
                                    if mouse.column > root_chunks[1].width.saturating_sub(20) {
                                        self.editor.next_buffer();
                                        self.sync_explorer();
                                    }
                                    self.vim.focus = Focus::Editor;
                                }
                            }
                            MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                                if self.vim.focus == Focus::Editor {
                                    if let Some((buf_line, buf_col)) =
                                        self.mouse_to_editor_pos(mouse.column, mouse.row)
                                    {
                                        if !self.is_dragging {
                                            self.is_dragging = true;
                                            if let Some(anchor) = self.drag_anchor {
                                                self.vim.selection_start = Some(anchor);
                                                self.vim.mode = Mode::Visual;
                                                self.vim.yank_type = YankType::Char;
                                            }
                                        }
                                        self.editor.cursor_mut().y = buf_line;
                                        self.editor.cursor_mut().x = buf_col;
                                        self.editor.clamp_cursor();
                                    }
                                }
                            }
                            MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                                self.is_dragging = false;
                                self.drag_anchor = None;
                            }
                            _ => {}
                        }
                    }
                    if let Event::Key(key) = event {
                        self.vim.yank_highlight_line = None;
                        if self.vim.blame_popup.is_some() {
                            self.vim.blame_popup = None;
                            continue;
                        }

                        match self.vim.mode {
                            Mode::Normal => {
                                let mut action = Action::Unbound;
                                let is_in_sequence = !self.vim.input_buffer.is_empty();

                                // Ctrl+W: toggle split focus
                                if key.code == KeyCode::Char('w') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                    let panes = self.vim.pane_layout.get_all_panes();
                                    if panes.len() > 1 {
                                        let current_idx = panes.iter().position(|p| p.id == self.vim.focused_pane_id).unwrap_or(0);
                                        let next_pane = &panes[(current_idx + 1) % panes.len()];
                                        self.vim.focused_pane_id = next_pane.id;
                                        self.editor.active_idx = next_pane.buffer_idx;
                                    }
                                    continue;
                                }
                                // Preview popup scroll / close
                                if self.vim.preview_lines.is_some() {
                                    match key.code {
                                        KeyCode::Esc | KeyCode::Char('q') => {
                                            self.vim.preview_lines = None;
                                            self.vim.preview_scroll = 0;
                                        }
                                        KeyCode::Char('j') | KeyCode::Down => {
                                            self.vim.preview_scroll = self.vim.preview_scroll.saturating_add(1);
                                        }
                                        KeyCode::Char('k') | KeyCode::Up => {
                                            self.vim.preview_scroll = self.vim.preview_scroll.saturating_sub(1);
                                        }
                                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                            self.vim.preview_scroll = self.vim.preview_scroll.saturating_add(20);
                                        }
                                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                            self.vim.preview_scroll = self.vim.preview_scroll.saturating_sub(20);
                                        }
                                        _ => {}
                                    }
                                    continue;
                                }

                                if !is_in_sequence {
                                    action = match self.vim.focus {
                                        Focus::Editor => self.keymap_normal.resolve(&key),
                                        Focus::Explorer => self.keymap_explorer.resolve(&key),
                                        Focus::Trouble => self.keymap_normal.resolve(&key),
                                    }
                                    .clone();
                                }

                                match action {
                                    Action::Unbound => match self.vim.focus {
                                        Focus::Editor | Focus::Trouble => {
                                            if let KeyCode::Char(c) = key.code {
                                                if c.is_ascii_digit()
                                                    && (self.vim.input_buffer.is_empty()
                                                        || self
                                                            .vim
                                                            .input_buffer
                                                            .chars()
                                                            .all(|c| c.is_ascii_digit()))
                                                {
                                                    self.vim.input_buffer.push(c);
                                                    continue;
                                                }

                                                let count = if !self.vim.input_buffer.is_empty()
                                                    && self
                                                        .vim
                                                        .input_buffer
                                                        .chars()
                                                        .all(|c| c.is_ascii_digit())
                                                {
                                                    let c_val = self
                                                        .vim
                                                        .input_buffer
                                                        .parse::<usize>()
                                                        .unwrap_or(1);
                                                    self.vim.input_buffer.clear();
                                                    c_val
                                                } else {
                                                    1
                                                };

                                                self.vim.input_buffer.push(c);
                                                let seq = self.vim.input_buffer.clone();
                                                let mut matched = true;
                                                match seq.as_str() {
                                                    " ff" => self.dispatch_action(
                                                        Action::TelescopeFiles,
                                                        count,
                                                    ),
                                                    " fg" => self.dispatch_action(
                                                        Action::TelescopeLiveGrep,
                                                        count,
                                                    ),
                                                    " fb" => self.dispatch_action(
                                                        Action::TelescopeBuffers,
                                                        count,
                                                    ),
                                                    " th" | "th" => self.dispatch_action(
                                                        Action::TelescopeThemes,
                                                        count,
                                                    ),
                                                    " n" => self.dispatch_action(
                                                        Action::ToggleRelativeNumber,
                                                        count,
                                                    ),
                                                    " /" => self.dispatch_action(
                                                        Action::ToggleComment,
                                                        count,
                                                    ),
                                                    " tt" => self.dispatch_action(
                                                        Action::ToggleTrouble,
                                                        count,
                                                    ),
                                                    " bb" => self.dispatch_action(
                                                        Action::ToggleAutoformat,
                                                        count,
                                                    ),
                                                    " bl" => self
                                                        .dispatch_action(Action::GitBlame, count),
                                                    " x" => self.dispatch_action(
                                                        Action::CloseBuffer,
                                                        count,
                                                    ),
                                                    "gg" => self.dispatch_action(
                                                        Action::JumpToFirstLine,
                                                        count,
                                                    ),
                                                    "dd" => self
                                                        .dispatch_action(Action::DeleteLine, count),
                                                    "yy" => self
                                                        .dispatch_action(Action::YankLine, count),
                                                    "[[" => self.dispatch_action(
                                                        Action::JumpToFirstLine,
                                                        count,
                                                    ),
                                                    "]]" => self.dispatch_action(
                                                        Action::JumpToLastLine,
                                                        count,
                                                    ),
                                                    "gd" => self.dispatch_action(
                                                        Action::LspDefinition,
                                                        count,
                                                    ),
                                                    "zc" | "za" => self
                                                        .dispatch_action(Action::ToggleFold, count),
                                                    "]g" => self
                                                        .dispatch_action(Action::NextHunk, count),
                                                    "[g" => self
                                                        .dispatch_action(Action::PrevHunk, count),
                                                    "ZZ" => {
                                                        self.dispatch_action(Action::SaveAndQuit, 1)
                                                    }
                                                    "ZQ" => self.dispatch_action(
                                                        Action::QuitWithoutSaving,
                                                        1,
                                                    ),
                                                    _ => {
                                                        matched = false;
                                                    }
                                                }

                                                if matched {
                                                    self.vim.input_buffer.clear();
                                                } else {
                                                    let is_partial = matches!(
                                                        seq.as_str(),
                                                        " " | " f"
                                                            | " t"
                                                            | " g"
                                                            | " b"
                                                            | "["
                                                            | "]"
                                                            | "z"
                                                            | "d"
                                                            | "y"
                                                            | "g"
                                                            | "Z"
                                                    );
                                                    if !is_partial {
                                                        self.vim.input_buffer.clear();
                                                        let fallback =
                                                            self.keymap_normal.resolve(&key);
                                                        if fallback != &Action::Unbound {
                                                            self.dispatch_action(
                                                                fallback.clone(),
                                                                count,
                                                            );
                                                        }
                                                    }
                                                }
                                            } else {
                                                self.vim.input_buffer.clear();
                                                if key.code == KeyCode::Esc {
                                                    self.vim.input_buffer.clear();
                                                    self.vim.selection_start = None;
                                                }
                                            }
                                        }
                                        Focus::Explorer => match key.code {
                                            KeyCode::Char('<') => self.explorer.decrease_width(),
                                            KeyCode::Char('>') => self.explorer.increase_width(),
                                            KeyCode::Char('y') => {
                                                if let Some(entry) = self.explorer.selected_entry() {
                                                    self.vim.register = entry.path.to_string_lossy().to_string();
                                                    self.vim.set_message("Path copied to register".to_string());
                                                }
                                            }
                                            // Ctrl+S → open in horizontal split
                                            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                                if let Some(entry) = self.explorer.selected_entry() {
                                                    if entry.path.is_file() {
                                                        let path = entry.path.clone();
                                                        if let Some(existing_buf_idx) = self.editor.find_buffer_by_path(&path) {
                                                            let all_panes = self.vim.pane_layout.get_all_panes();
                                                            if let Some(p) = all_panes.iter().find(|p| p.buffer_idx == existing_buf_idx) {
                                                                self.vim.focused_pane_id = p.id;
                                                                self.editor.active_idx = existing_buf_idx;
                                                                self.vim.focus = Focus::Editor;
                                                            } else {
                                                                // Open new split with existing buffer
                                                                let new_id = self.vim.next_pane_id;
                                                                self.vim.next_pane_id += 1;
                                                                let new_pane = crate::vim::Pane { id: new_id, buffer_idx: existing_buf_idx };
                                                                self.vim.pane_layout.split(self.vim.focused_pane_id, new_pane, crate::vim::mode::SplitKind::Horizontal);
                                                                self.vim.focused_pane_id = new_id;
                                                                self.editor.active_idx = existing_buf_idx;
                                                                self.vim.focus = Focus::Editor;
                                                            }
                                                        } else {
                                                            let _ = self.editor.open_file(path);
                                                            let new_buf = self.editor.active_idx;
                                                            let new_id = self.vim.next_pane_id;
                                                            self.vim.next_pane_id += 1;
                                                            let new_pane = crate::vim::Pane { id: new_id, buffer_idx: new_buf };
                                                            self.vim.pane_layout.split(self.vim.focused_pane_id, new_pane, crate::vim::mode::SplitKind::Horizontal);
                                                            self.vim.focused_pane_id = new_id;
                                                            self.vim.focus = Focus::Editor;
                                                        }
                                                        self.editor.refresh_syntax_for_idx(self.editor.active_idx);
                                                    }
                                                }
                                            }
                                            // Ctrl+V → open in vertical split
                                            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                                if let Some(entry) = self.explorer.selected_entry() {
                                                    if entry.path.is_file() {
                                                        let path = entry.path.clone();
                                                        if let Some(existing_buf_idx) = self.editor.find_buffer_by_path(&path) {
                                                            let all_panes = self.vim.pane_layout.get_all_panes();
                                                            if let Some(p) = all_panes.iter().find(|p| p.buffer_idx == existing_buf_idx) {
                                                                self.vim.focused_pane_id = p.id;
                                                                self.editor.active_idx = existing_buf_idx;
                                                                self.vim.focus = Focus::Editor;
                                                            } else {
                                                                let new_id = self.vim.next_pane_id;
                                                                self.vim.next_pane_id += 1;
                                                                let new_pane = crate::vim::Pane { id: new_id, buffer_idx: existing_buf_idx };
                                                                self.vim.pane_layout.split(self.vim.focused_pane_id, new_pane, crate::vim::mode::SplitKind::Vertical);
                                                                self.vim.focused_pane_id = new_id;
                                                                self.editor.active_idx = existing_buf_idx;
                                                                self.vim.focus = Focus::Editor;
                                                            }
                                                        } else {
                                                            let _ = self.editor.open_file(path);
                                                            let new_buf = self.editor.active_idx;
                                                            let new_id = self.vim.next_pane_id;
                                                            self.vim.next_pane_id += 1;
                                                            let new_pane = crate::vim::Pane { id: new_id, buffer_idx: new_buf };
                                                            self.vim.pane_layout.split(self.vim.focused_pane_id, new_pane, crate::vim::mode::SplitKind::Vertical);
                                                            self.vim.focused_pane_id = new_id;
                                                            self.vim.focus = Focus::Editor;
                                                        }
                                                        self.editor.refresh_syntax_for_idx(self.editor.active_idx);
                                                    }
                                                }
                                            }
                                            // Shift+P → floating preview (no focus change)
                                            KeyCode::Char('P') => {
                                                if let Some(entry) = self.explorer.selected_entry() {
                                                    if entry.path.is_file() {
                                                        if let Ok(content) = std::fs::read_to_string(&entry.path) {
                                                            self.vim.preview_lines = Some(content.lines().map(|l| l.to_string()).collect());
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {}
                                        },
                                    },
                                    action => {
                                        let count = if !self.vim.input_buffer.is_empty()
                                            && self
                                                .vim
                                                .input_buffer
                                                .chars()
                                                .all(|c| c.is_ascii_digit())
                                        {
                                            let c_val =
                                                self.vim.input_buffer.parse::<usize>().unwrap_or(1);
                                            self.vim.input_buffer.clear();
                                            c_val
                                        } else {
                                            1
                                        };
                                        self.vim.input_buffer.clear();
                                        self.dispatch_action(action.clone(), count);
                                    }
                                }
                            }
                            Mode::Visual => match key.code {
                                KeyCode::Esc => self.dispatch_action(Action::ExitMode, 1),
                                KeyCode::Char('s')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    self.dispatch_action(Action::Save, 1)
                                }
                                KeyCode::Char('c')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    self.dispatch_action(Action::CopyToClipboard, 1)
                                }
                                KeyCode::Char('v')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    self.dispatch_action(Action::PasteFromClipboard, 1)
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    self.dispatch_action(Action::MoveDown, 1)
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    self.dispatch_action(Action::MoveUp, 1)
                                }
                                KeyCode::Char('h') | KeyCode::Left => {
                                    self.dispatch_action(Action::MoveLeft, 1)
                                }
                                KeyCode::Char('l') | KeyCode::Right => {
                                    self.dispatch_action(Action::MoveRight, 1)
                                }
                                KeyCode::PageUp => self.dispatch_action(Action::MoveLineStart, 1),
                                KeyCode::PageDown => self.dispatch_action(Action::MovePageDown, 1),
                                KeyCode::Home => self.dispatch_action(Action::MoveLineStart, 1),
                                KeyCode::End => self.dispatch_action(Action::MoveLineEnd, 1),
                                KeyCode::Char('w') => {
                                    self.dispatch_action(Action::MoveWordForward, 1)
                                }
                                KeyCode::Char('b') => {
                                    self.dispatch_action(Action::MoveWordBackward, 1)
                                }
                                KeyCode::Char('p') => self.dispatch_action(Action::PasteAfter, 1),
                                KeyCode::Char('s')
                                    if !key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    self.dispatch_action(Action::DeleteSelection, 1)
                                }
                                KeyCode::Char('y') => self.dispatch_action(Action::YankLine, 1),
                                KeyCode::Char('d') | KeyCode::Char('x') => {
                                    self.dispatch_action(Action::DeleteSelection, 1)
                                }
                                _ => {}
                            },
                            Mode::Insert => {
                                let action = self.keymap_insert.resolve(&key);
                                match action {
                                    Action::ExitMode => self.dispatch_action(Action::ExitMode, 1),
                                    Action::Save => self.dispatch_action(Action::Save, 1),
                                    Action::Confirm => self.dispatch_action(Action::Confirm, 1),
                                    Action::PasteFromClipboard => {
                                        self.dispatch_action(Action::PasteFromClipboard, 1)
                                    }
                                    Action::SelectNext => {
                                        self.dispatch_action(Action::SelectNext, 1)
                                    }
                                    Action::SelectPrev => {
                                        self.dispatch_action(Action::SelectPrev, 1)
                                    }
                                    Action::Indent => self.dispatch_action(Action::Indent, 1),
                                    _ => match key.code {
                                        KeyCode::Up => {
                                            if self.vim.show_suggestions
                                                && !self.vim.filtered_suggestions.is_empty()
                                            {
                                                self.dispatch_action(Action::SelectPrev, 1);
                                            } else {
                                                self.editor.move_up();
                                            }
                                        }
                                        KeyCode::Down => {
                                            if self.vim.show_suggestions
                                                && !self.vim.filtered_suggestions.is_empty()
                                            {
                                                self.dispatch_action(Action::SelectNext, 1);
                                            } else {
                                                self.editor.move_down();
                                            }
                                        }
                                        KeyCode::Left => self.editor.move_left(),
                                        KeyCode::Right => self.editor.move_right(),
                                        KeyCode::Char(' ') | KeyCode::Null
                                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                                || key.code == KeyCode::Null =>
                                        {
                                            if let Some(path) =
                                                self.editor.buffer().file_path.clone()
                                            {
                                                if let Some(ext) = path
                                                    .extension()
                                                    .and_then(|s| s.to_str())
                                                    .map(|s| s.to_lowercase())
                                                {
                                                    let (y, x) = (
                                                        self.editor.cursor().y,
                                                        self.editor.cursor().x,
                                                    );
                                                    let _ = self.lsp_manager.request_completions(
                                                        &ext,
                                                        &path,
                                                        y,
                                                        x,
                                                        CompletionTriggerKind::INVOKED,
                                                        None,
                                                    );
                                                }
                                            }
                                        }
                                        KeyCode::Char(c) => {
                                            let (y, x) =
                                                (self.editor.cursor().y, self.editor.cursor().x);
                                            let idx = self.safe_line_to_char(y) + x;
                                            let mut to_insert = c.to_string();
                                            match c {
                                                '(' => to_insert.push(')'),
                                                '[' => to_insert.push(']'),
                                                '{' => to_insert.push('}'),
                                                '\'' => to_insert.push('\''),
                                                '"' => to_insert.push('"'),
                                                '>' => {
                                                    if let Some(line) = self.editor.buffer().line(y)
                                                    {
                                                        let line_str = line.to_string();
                                                        let before_cursor =
                                                            &line_str[..x.min(line_str.len())];
                                                        if let Some(tag_start) =
                                                            before_cursor.rfind('<')
                                                        {
                                                            let tag_content =
                                                                &before_cursor[tag_start + 1..];
                                                            if !tag_content.is_empty()
                                                                && !tag_content.contains(' ')
                                                                && !tag_content.contains('/')
                                                            {
                                                                to_insert.push_str(&format!(
                                                                    "</{}>",
                                                                    tag_content
                                                                ));
                                                            }
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                            self.editor.buffer_mut().apply_edit(|t| {
                                                t.insert(idx, &to_insert);
                                            });
                                            self.editor.cursor_mut().x += 1;
                                            if let Some(path) =
                                                self.editor.buffer().file_path.clone()
                                            {
                                                if let Some(ext) = path
                                                    .extension()
                                                    .and_then(|s| s.to_str())
                                                    .map(|s| s.to_lowercase())
                                                {
                                                    let is_trigger =
                                                        c == '.' || c == ':' || c == '>';
                                                    let is_alpha = c.is_alphanumeric() || c == '_';

                                                    let text =
                                                        self.editor.buffer().text.to_string();
                                                    let _ = self
                                                        .lsp_manager
                                                        .did_change(&ext, &path, text);
                                                    self.last_lsp_update = Some(Instant::now());

                                                    if is_trigger || is_alpha {
                                                        let trigger_kind = if is_trigger {
                                                            CompletionTriggerKind::TRIGGER_CHARACTER
                                                        } else {
                                                            CompletionTriggerKind::INVOKED
                                                        };
                                                        let trigger_char = if is_trigger {
                                                            Some(c.to_string())
                                                        } else {
                                                            None
                                                        };
                                                        let _ =
                                                            self.lsp_manager.request_completions(
                                                                &ext,
                                                                &path,
                                                                y,
                                                                x + 1,
                                                                trigger_kind,
                                                                trigger_char,
                                                            );
                                                    } else {
                                                        self.vim.show_suggestions = false;
                                                        self.vim.suggestions.clear();
                                                        self.vim.filtered_suggestions.clear();
                                                    }
                                                }
                                            }
                                            self.refresh_filtered_suggestions();
                                        }
                                        KeyCode::Backspace => {
                                            let (y, x) =
                                                (self.editor.cursor().y, self.editor.cursor().x);
                                            if x > 0 {
                                                let idx = self.safe_line_to_char(y) + x;
                                                self.editor.buffer_mut().apply_edit(|t| {
                                                    t.remove((idx - 1)..idx);
                                                });
                                                self.editor.cursor_mut().x -= 1;
                                                if let Some(path) =
                                                    self.editor.buffer().file_path.clone()
                                                {
                                                    if let Some(ext) = path
                                                        .extension()
                                                        .and_then(|s| s.to_str())
                                                        .map(|s| s.to_lowercase())
                                                    {
                                                        let should_update = self
                                                            .last_lsp_update
                                                            .map_or(true, |t| {
                                                                t.elapsed()
                                                                    > Duration::from_millis(200)
                                                            });
                                                        if should_update {
                                                            let text = self
                                                                .editor
                                                                .buffer()
                                                                .text
                                                                .to_string();
                                                            let _ = self
                                                                .lsp_manager
                                                                .did_change(&ext, &path, text);
                                                            self.last_lsp_update =
                                                                Some(Instant::now());
                                                        }
                                                        if self.vim.suggestions.is_empty() {
                                                            self.vim.show_suggestions = false;
                                                        }
                                                    }
                                                }
                                                self.refresh_filtered_suggestions();
                                            } else if y > 0 {
                                                let prev_line_idx = y - 1;
                                                let prev_line =
                                                    self.editor.buffer().text.line(prev_line_idx);
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

                                                let char_idx = self.safe_line_to_char(y);
                                                self.editor.buffer_mut().apply_edit(|t| {
                                                    t.remove((char_idx - 1)..char_idx);
                                                });

                                                self.editor.cursor_mut().y -= 1;
                                                self.editor.cursor_mut().x = new_x;

                                                if let Some(path) =
                                                    self.editor.buffer().file_path.clone()
                                                {
                                                    if let Some(ext) = path
                                                        .extension()
                                                        .and_then(|s| s.to_str())
                                                        .map(|s| s.to_lowercase())
                                                    {
                                                        let text =
                                                            self.editor.buffer().text.to_string();
                                                        let _ = self
                                                            .lsp_manager
                                                            .did_change(&ext, &path, text);
                                                    }
                                                }
                                            }
                                        }
                                        _ => {}
                                    },
                                }
                            }
                            Mode::Search => match key.code {
                                KeyCode::Esc => self.vim.mode = Mode::Normal,
                                KeyCode::Char(c) => self.vim.search_query.push(c),
                                KeyCode::Backspace => {
                                    self.vim.search_query.pop();
                                }
                                KeyCode::Enter => self.vim.mode = Mode::Normal,
                                _ => {}
                            },
                            Mode::ExplorerInput(input_type) => match key.code {
                                KeyCode::Esc => {
                                    if let ExplorerInputType::Filter = input_type {
                                        self.explorer.filter.clear();
                                        self.explorer.refresh();
                                    }
                                    self.vim.mode = Mode::Normal;
                                }
                                KeyCode::Char(c) => {
                                    self.vim.input_buffer.push(c);
                                    if let ExplorerInputType::Filter = input_type {
                                        self.explorer.filter = self.vim.input_buffer.clone();
                                        self.explorer.refresh();
                                    }
                                }
                                KeyCode::Backspace => {
                                    self.vim.input_buffer.pop();
                                    if let ExplorerInputType::Filter = input_type {
                                        self.explorer.filter = self.vim.input_buffer.clone();
                                        self.explorer.refresh();
                                    }
                                }
                                KeyCode::Enter => {
                                    let input = self.vim.input_buffer.clone();
                                    self.vim.input_buffer.clear();
                                    self.vim.mode = Mode::Normal;
                                    match input_type {
                                        ExplorerInputType::Add => {
                                            if let Err(e) = self.explorer.create_file(&input) {
                                                self.vim.set_message(format!("Error: {}", e));
                                            }
                                        }
                                        ExplorerInputType::Rename => {
                                            if let Err(e) = self.explorer.rename_selected(&input) {
                                                self.vim.set_message(format!("Error: {}", e));
                                            }
                                        }
                                        ExplorerInputType::Move => {
                                            if let Err(e) =
                                                self.explorer.move_selected(Path::new(&input))
                                            {
                                                self.vim.set_message(format!("Error: {}", e));
                                            }
                                        }
                                        ExplorerInputType::DeleteConfirm => {
                                            if input.to_lowercase() == "y" {
                                                if let Err(e) = self.explorer.delete_selected() {
                                                    self.vim.set_message(format!("Error: {}", e));
                                                }
                                            }
                                        }
                                        ExplorerInputType::Filter => {
                                            self.explorer.filter = input;
                                            self.explorer.refresh();
                                        }
                                    }
                                }
                                _ => {}
                            },
                            Mode::Confirm(action) => match key.code {
                                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('l') | KeyCode::Char('L') => match action {
                                    crate::vim::mode::ConfirmAction::Quit => {
                                        self.save_and_format(None);
                                        self.should_quit = true;
                                    }
                                    crate::vim::mode::ConfirmAction::CloseBuffer => {
                                        self.save_and_format(None);
                                        self.editor.close_current_buffer();
                                        self.vim.mode = Mode::Normal;
                                    }
                                    crate::vim::mode::ConfirmAction::ReloadFile => {
                                        if let Err(e) = self.editor.buffer_mut().reload() {
                                            self.vim.set_message(format!("Error reloading file: {}", e));
                                        } else {
                                            self.editor.clamp_cursor();
                                            self.editor.refresh_syntax();
                                        }
                                        self.vim.mode = Mode::Normal;
                                    }
                                },
                                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('i') | KeyCode::Char('I') => match action {
                                    crate::vim::mode::ConfirmAction::Quit => {
                                        self.should_quit = true;
                                    }
                                    crate::vim::mode::ConfirmAction::CloseBuffer => {
                                        self.editor.close_current_buffer();
                                        self.vim.mode = Mode::Normal;
                                    }
                                    crate::vim::mode::ConfirmAction::ReloadFile => {
                                        self.vim.mode = Mode::Normal;
                                    }
                                },
                                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                                    self.vim.mode = Mode::Normal;
                                }
                                _ => {}
                            },
                            Mode::Telescope(_) => match key.code {
                                KeyCode::Esc => self.dispatch_action(Action::ExitMode, 1),
                                KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                                    self.dispatch_action(Action::SelectNext, 1)
                                }
                                KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                                    self.dispatch_action(Action::SelectPrev, 1)
                                }
                                KeyCode::Char('u')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    self.vim.telescope.scroll_preview_up(5)
                                }
                                KeyCode::Char('d')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    self.vim.telescope.scroll_preview_down(5)
                                }
                                KeyCode::Char(c) => {
                                    self.vim.telescope.query.push(c);
                                    self.vim.telescope.update_results(&self.editor);
                                }
                                KeyCode::Backspace => {
                                    self.vim.telescope.query.pop();
                                    self.vim.telescope.update_results(&self.editor);
                                }
                                KeyCode::Enter => self.dispatch_action(Action::Confirm, 1),
                                _ => {}
                            },
                            Mode::Nucleus => match key.code {
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    self.dispatch_action(Action::ExitMode, 1)
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    self.dispatch_action(Action::SelectNext, 1)
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    self.dispatch_action(Action::SelectPrev, 1)
                                }
                                KeyCode::Char('1') => {
                                    self.vim.nucleus_tab = 0;
                                    self.vim.nucleus_state.select(Some(0));
                                }
                                KeyCode::Char('2') => {
                                    self.vim.nucleus_tab = 1;
                                    self.vim.nucleus_state.select(Some(0));
                                }
                                KeyCode::Char('3') => {
                                    self.vim.nucleus_tab = 2;
                                    self.vim.nucleus_state.select(Some(0));
                                }
                                KeyCode::Char('4') => {
                                    self.vim.nucleus_tab = 3;
                                    self.vim.nucleus_state.select(Some(0));
                                }
                                KeyCode::Char('5') => {
                                    self.vim.nucleus_tab = 4;
                                    self.vim.nucleus_state.select(Some(0));
                                }
                                KeyCode::Char('6') => {
                                    self.vim.nucleus_tab = 5;
                                    self.vim.nucleus_state.select(Some(0));
                                }
                                KeyCode::Char('f')
                                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    self.vim.mode = Mode::NucleusFilter;
                                    self.vim.nucleus_filter.clear();
                                }
                                KeyCode::Char(' ')
                                | KeyCode::Char('i')
                                | KeyCode::Char('u')
                                | KeyCode::Char('d')
                                | KeyCode::Char('x') => self.install_selected_package(key),                                _ => {}
                            },
                            Mode::NucleusFilter => match key.code {
                                KeyCode::Esc | KeyCode::Enter => {
                                    self.vim.mode = Mode::Nucleus;
                                }
                                KeyCode::Char(c) => {
                                    self.vim.nucleus_filter.push(c);
                                    self.vim.nucleus_state.select(Some(0));
                                }
                                KeyCode::Backspace => {
                                    self.vim.nucleus_filter.pop();
                                    self.vim.nucleus_state.select(Some(0));
                                }
                                _ => {}
                            },
                            Mode::Keymaps => match key.code {
                                KeyCode::Esc | KeyCode::Char('?') => {
                                    self.dispatch_action(Action::ExitMode, 1)
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    self.dispatch_action(Action::SelectNext, 1)
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    self.dispatch_action(Action::SelectPrev, 1)
                                }
                                KeyCode::Char(c) => {
                                    self.vim.keymap_filter.push(c);
                                    self.vim.keymap_state.select(Some(0));
                                }
                                KeyCode::Backspace => {
                                    self.vim.keymap_filter.pop();
                                    self.vim.keymap_state.select(Some(0));
                                }
                                _ => {}
                            },
                            Mode::Command => self.handle_command_mode(key),
                            Mode::VisualBlock => self.handle_visual_block_mode(key),
                            Mode::BlockInsert => self.handle_block_insert_mode(key),
                        }

                    }
                }
            }
            if self.should_quit {
                break;
            }

            if self.trouble.visible && !self.trouble.scanned {
                let todos = crate::editor::todo::scan_project_todos(&self.vim.project_root);
                let diagnostics = self.lsp_manager.diagnostics.lock().unwrap();
                self.trouble.update_from_lsp(&diagnostics, todos);
                self.trouble.scanned = true;
            }

            let editor_width = if self.explorer.visible {
                (area.width as f32 * 0.85) as usize - 8
            } else {
                area.width as usize - 8
            };
            self.editor
                .scroll_into_view(visible_height, editor_width, self.vim.config.wrap);
            self.editor.refresh_syntax();
            let panes = self.vim.pane_layout.get_all_panes();
            for pane in panes {
                if pane.id != self.vim.focused_pane_id {
                    self.editor.refresh_split_syntax(pane.buffer_idx);
                }
            }
            // Hide intro whenever the active buffer has a file or content (checked every frame)
            if self.vim.show_intro {
                let buf = self.editor.buffers.get(self.editor.active_idx);
                if buf.map(|b| b.modified || b.file_path.is_some()).unwrap_or(false) {
                    self.vim.show_intro = false;
                }
            }

            self.terminal.draw(|f| {
                self.ui.draw(
                    f,
                    &mut self.editor,
                    &mut self.vim,
                    &mut self.explorer,
                    &self.trouble,
                    &self.lsp_manager,
                )
            })?;

            let cursor_style = match self.vim.mode {
                Mode::Insert | Mode::BlockInsert => SetCursorStyle::SteadyBar,
                _ => SetCursorStyle::SteadyBlock,
            };
            execute!(self.terminal.backend_mut(), cursor_style)?;
        }

        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            SetCursorStyle::DefaultUserShape
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    /// Converts terminal mouse coordinates to (buffer_line, buffer_col).
    fn mouse_to_editor_pos(&self, col: u16, row: u16) -> Option<(usize, usize)> {
        let area = self.terminal.size().ok()?;
        let gutter_width: u16 = match (
            self.vim.show_number || self.vim.relative_number,
            self.vim.config.signcolumn,
        ) {
            (true, true) => 7,
            (true, false) => 5,
            (false, true) => 3,
            (false, false) => 0,
        };
        let explorer_width = if self.explorer.visible { self.explorer.width } else { 0 };

        // Determine pane y-offset so row is relative to the editor content area
        let root_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(if self.vim.config.laststatus >= 2 { 1 } else { 0 }),
                Constraint::Length(1),
            ])
            .split(area.into());
        let pane_y = root_chunks[0].y;

        let text_start_col = explorer_width + gutter_width;
        if col < text_start_col {
            return None;
        }
        if row < pane_y {
            return None;
        }

        let display_col = (col - text_start_col) as usize;
        let scroll_y = self.editor.cursor().scroll_y;
        let buf_line = scroll_y + (row - pane_y) as usize;

        let num_lines = self.editor.buffer().len_lines();
        if buf_line >= num_lines {
            return None;
        }

        // Convert display column → char index (handles tabs and wide chars)
        let line = self.editor.buffer().line(buf_line)?;
        let mut cells = 0usize;
        let mut char_col = 0usize;
        for c in line.chars() {
            if c == '\n' || c == '\r' { break; }
            let w = if c == '\t' { self.vim.config.tabstop } else {
                unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
            };
            if cells + w > display_col { break; }
            cells += w;
            char_col += 1;
        }

        Some((buf_line, char_col))
    }

    /// Returns (word_start, word_end) char indices (inclusive) for the word at the given position.
    fn word_at_editor_pos(&self, line_idx: usize, col: usize) -> Option<(usize, usize)> {
        let line = self.editor.buffer().line(line_idx)?;
        let chars: Vec<char> = line.chars().take_while(|&c| c != '\n' && c != '\r').collect();

        if col >= chars.len() {
            return None;
        }

        let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

        if !is_word_char(chars[col]) {
            return None;
        }

        let start = (0..=col)
            .rev()
            .find(|&i| !is_word_char(chars[i]))
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = (col..chars.len())
            .find(|&i| !is_word_char(chars[i]))
            .unwrap_or(chars.len())
            .saturating_sub(1);

        Some((start, end))
    }
}
