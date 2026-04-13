use std::{env, io, path::{Path, PathBuf}, time::{Duration, Instant}, sync::mpsc};
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config};
use crossterm::{
    cursor::SetCursorStyle,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use anyhow::Result;

use crate::editor::Editor;
use crate::ui::TerminalUi;
use crate::vim::{mode::{Mode, YankType, Focus, ExplorerInputType}, VimState, Position, LspStatus};
use crate::ui::explorer::FileExplorer;
use crate::lsp::LspManager;
use crate::ui::trouble::TroubleList;
use crate::input::keymap::{Keymap, Action};
use crate::plugins::PluginManager;
use lsp_types::{GotoDefinitionResponse, CompletionResponse, PublishDiagnosticsParams, CompletionTriggerKind};

pub struct App {
    pub vim: VimState,
    pub editor: Editor,
    pub ui: TerminalUi,
    pub explorer: FileExplorer,
    pub trouble: TroubleList,
    pub lsp_manager: LspManager,
    pub terminal: Terminal<CrosstermBackend<io::Stdout>>,
    pub rx: mpsc::Receiver<notify::Result<notify::Event>>,
    pub watcher: RecommendedWatcher,
    pub keymap_normal: Keymap,
    pub keymap_insert: Keymap,
    pub keymap_explorer: Keymap,
    pub plugin_manager: PluginManager,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = crate::config::Config::load();
        let project_root = find_project_root(&env::current_dir().unwrap_or_default());
        let vim = VimState::new(config, project_root);

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if vim.config.mouse {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        } else {
            execute!(stdout, EnterAlternateScreen)?;
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let editor = Editor::new(&vim.config.colorscheme);
        let ui = TerminalUi::new();
        let explorer = FileExplorer::new();
        let trouble = TroubleList::new();
        let lsp_manager = LspManager::new();

        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        watcher.watch(&vim.project_root, RecursiveMode::Recursive)?;

        let mut keymap_normal = Keymap::default_normal();
        let mut keymap_insert = Keymap::default_insert();
        let mut keymap_explorer = Keymap::new();
        
        let plugin_manager = PluginManager::new();
        plugin_manager.register_all_keymaps(&mut keymap_normal, Mode::Normal);
        plugin_manager.register_all_keymaps(&mut keymap_insert, Mode::Insert);
        
        // Populate keymap_explorer from plugins that have explorer bindings
        plugin_manager.register_all_keymaps(&mut keymap_explorer, Mode::Normal);
        // Explicitly bind generic ones if not handled by plugins
        keymap_explorer.bind("Esc", Action::ExitMode);
        keymap_explorer.bind("\\", Action::ExitMode);
        keymap_explorer.bind(":", Action::EnterCommand);
        keymap_explorer.bind("Up", Action::MoveUp);
        keymap_explorer.bind("Down", Action::MoveDown);

        Ok(Self {
            vim,
            editor,
            ui,
            explorer,
            trouble,
            lsp_manager,
            terminal,
            rx,
            watcher,
            keymap_normal,
            keymap_insert,
            keymap_explorer,
            plugin_manager,
            should_quit: false,
        })
    }

    pub fn format_buffer(&mut self) -> Result<(), String> {
        if let Some(path) = self.editor.buffer().file_path.clone() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                self.vim.lsp_status = LspStatus::Formatting;
                let _ = self.terminal.draw(|f| self.ui.draw(f, &self.editor, &mut self.vim, &mut self.explorer, &self.trouble, &self.lsp_manager));
                let text = self.editor.buffer().text.to_string();
                match self.lsp_manager.format_document(&ext, &path, text) {
                    Some(Ok(formatted)) => {
                        self.editor.buffer_mut().text = ropey::Rope::from_str(&formatted);
                        self.editor.clamp_cursor();
                        let _ = self.lsp_manager.did_change(&ext, &path, formatted);
                        self.vim.lsp_status = LspStatus::None;
                        return Ok(());
                    }
                    Some(Err(e)) => { self.vim.lsp_status = LspStatus::None; return Err(e); }
                    None => { self.vim.lsp_status = LspStatus::None; return Err("No formatter available".to_string()); }
                }
            }
        }
        self.vim.lsp_status = LspStatus::None;
        Ok(())
    }

    pub fn save_and_format(&mut self, path_to_save: Option<PathBuf>) {
        let mut format_info = String::new();
        if !self.vim.config.disable_autoformat {
            let _ = self.format_buffer();
            format_info.push_str("formatted, ");
        }
        let res = if let Some(path) = path_to_save {
            self.editor.save_file_as(path.clone()).map(|_| path.to_string_lossy().to_string())
        } else {
            self.editor.save_file().map(|_| self.editor.buffer().file_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default())
        };
        if let Ok(path_str) = res {
            let line_count = self.editor.buffer().len_lines();
            let char_count = self.editor.buffer().text.len_chars();
            self.vim.set_message(format!("\"{}\" {} {}L, {}C written", path_str, format_info, line_count, char_count));
            if let Some(path) = self.editor.buffer().file_path.clone() {
                let text = self.editor.buffer().text.to_string();
                self.editor.buffer_mut().git_signs = self.vim.git_manager.get_signs(&path, &text);
                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                    let _ = self.lsp_manager.did_save(&ext, &path, text);
                }
            }
        } else {
            self.vim.set_message("Error: Could not save file".to_string());
        }
    }

    pub fn refresh_filtered_suggestions(&mut self) {
        let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
        let line = self.editor.buffer().line(y).unwrap().to_string();
        let mut start_x = x;
        let chars: Vec<char> = line.chars().collect();
        while start_x > 0 && (chars[start_x-1].is_alphanumeric() || chars[start_x-1] == '_' || chars[start_x-1] == '$') {
            start_x -= 1;
        }
        let prefix = if start_x < x { line[start_x..x].to_lowercase() } else { String::new() };

        let mut unique_items = std::collections::HashSet::new();
        let mut filtered: Vec<lsp_types::CompletionItem> = self.vim.suggestions.iter()
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

    pub fn install_selected_package(&mut self) {
        let selected_idx = self.vim.mason_state.selected().unwrap_or(0);
        if self.vim.mason_tab == 5 {
            let languages = &crate::editor::treesitter::LANGUAGES;
            let filtered_langs: Vec<_> = languages.iter()
                .filter(|l| l.name.to_lowercase().contains(&self.vim.mason_filter.to_lowercase()))
                .collect();
            if let Some(lang) = filtered_langs.get(selected_idx) {
                if let Err(e) = self.editor.treesitter.install(lang) {
                    self.vim.set_message(format!("Error installing parser: {}", e));
                } else {
                    self.vim.set_message(format!("Parser {} installed", lang.name));
                }
            }
        } else {
            let packages: Vec<&crate::lsp::Package> = crate::lsp::PACKAGES.iter()
                .filter(|p| {
                    let matches_tab = match self.vim.mason_tab {
                        0 => true,
                        1 => p.kind == crate::lsp::PackageKind::Lsp,
                        2 => p.kind == crate::lsp::PackageKind::Dap,
                        3 => p.kind == crate::lsp::PackageKind::Linter,
                        4 => p.kind == crate::lsp::PackageKind::Formatter,
                        _ => true,
                    };
                    let matches_filter = p.name.to_lowercase().contains(&self.vim.mason_filter.to_lowercase()) ||
                                       p.description.to_lowercase().contains(&self.vim.mason_filter.to_lowercase());
                    matches_tab && matches_filter
                })
                .collect();
            
            let (installed, available): (Vec<_>, Vec<_>) = packages.into_iter().partition(|p| self.lsp_manager.is_managed(p.cmd));
            let target = if selected_idx < installed.len() { Some(installed[selected_idx]) }
                         else if selected_idx < installed.len() + available.len() { Some(available[selected_idx - installed.len()]) }
                         else { None };

            if let Some(pkg) = target {
                let _ = self.lsp_manager.install_server(pkg.cmd);
                self.vim.set_message(format!("Installing {}...", pkg.name));
            }
        }
    }

    pub fn toggle_comment(&mut self) {
        let path = self.editor.buffer().file_path.clone();
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

        self.editor.buffer_mut().push_history();
        let (s_y, e_y) = if let Mode::Visual = self.vim.mode {
            let start = self.vim.selection_start.unwrap();
            let cur = self.editor.cursor();
            if start.y < cur.y { (start.y, cur.y) } else { (cur.y, start.y) }
        } else {
            (self.editor.cursor().y, self.editor.cursor().y)
        };

        let all_commented = (s_y..=e_y).all(|y| {
            let line = self.editor.buffer().line(y).unwrap().to_string();
            line.trim().is_empty() || line.trim().starts_with(comment_prefix)
        });

        for y in s_y..=e_y {
            let line_str = self.editor.buffer().line(y).unwrap().to_string();
            if line_str.trim().is_empty() { continue; }
            let line_start_char = self.editor.buffer().text.line_to_char(y);
            if all_commented {
                if let Some(pos) = line_str.find(comment_prefix) {
                    self.editor.buffer_mut().apply_edit(|t| {
                        t.remove((line_start_char + pos)..(line_start_char + pos + comment_prefix.len()));
                    });
                }
                if !comment_suffix.is_empty() {
                    let updated = self.editor.buffer().line(y).unwrap().to_string();
                    if let Some(pos) = updated.rfind(comment_suffix) {
                        self.editor.buffer_mut().apply_edit(|t| {
                            t.remove((line_start_char + pos)..(line_start_char + pos + comment_suffix.len()));
                        });
                    }
                }
            } else {
                let indent = line_str.chars().take_while(|c| c.is_whitespace()).count();
                self.editor.buffer_mut().apply_edit(|t| {
                    t.insert(line_start_char + indent, comment_prefix);
                });
                let end_pos = line_start_char + self.editor.buffer().line(y).unwrap().len_chars();
                let has_newline = self.editor.buffer().line(y).unwrap().to_string().ends_with('\n');
                self.editor.buffer_mut().apply_edit(|t| {
                    t.insert(if has_newline { end_pos - 1 } else { end_pos }, comment_suffix);
                });
            }
        }
    }

    pub fn handle_args(&mut self, args: Vec<String>) {
        if args.len() > 1 {
            self.editor.buffers.clear(); self.editor.cursors.clear();
            for arg in &args[1..] {
                let path = PathBuf::from(arg).canonicalize().unwrap_or(PathBuf::from(arg));
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
                self.editor.buffers.push(crate::editor::buffer::Buffer::new()); 
                self.editor.cursors.push(crate::editor::cursor::Cursor::new()); 
            }
            self.editor.active_idx = 0;
        }

        if args.len() == 1 {
            self.vim.show_intro = true;
        }
    }

    pub fn dispatch_action(&mut self, action: Action, count: usize) {
        match action {
            Action::EnterInsert => { self.editor.buffer_mut().push_history(); self.vim.mode = Mode::Insert; }
            Action::EnterVisual => { self.vim.mode = Mode::Visual; let c = self.editor.cursor(); self.vim.selection_start = Some(Position { x: c.x, y: c.y }); }
            Action::EnterCommand => { self.vim.mode = Mode::Command; self.vim.command_buffer.clear(); }
            Action::EnterSearch => { self.vim.mode = Mode::Search; self.vim.search_query.clear(); }
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
            Action::EnterMason => { self.vim.mode = Mode::Mason; }
            Action::EnterTrouble => { self.trouble.toggle(); if self.trouble.visible { self.vim.focus = Focus::Trouble; } else { self.vim.focus = Focus::Editor; } }
            Action::EnterKeymaps => { self.vim.mode = Mode::Keymaps; self.vim.keymap_filter.clear(); self.vim.keymap_state.select(Some(0)); }

            Action::Save => { self.save_and_format(None); }
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
                match self.vim.focus {
                    Focus::Editor => { self.editor.next_buffer(); self.sync_explorer(); }
                    _ => {}
                }
            }
            Action::PrevBuffer => {
                match self.vim.focus {
                    Focus::Editor => { self.editor.prev_buffer(); self.sync_explorer(); }
                    _ => {}
                }
            }
            Action::ReloadFile => {
                if let Some(path) = self.editor.buffer().file_path.clone() {
                    let _ = self.editor.open_file(path);
                }
            }

            Action::MoveLeft => { for _ in 0..count { self.editor.move_left(); } }
            Action::MoveRight => { for _ in 0..count { self.editor.move_right(); } }
            Action::MoveUp => {
                match self.vim.focus {
                    Focus::Editor => { for _ in 0..count { self.editor.move_up(); } }
                    Focus::Explorer => { for _ in 0..count { self.explorer.move_up(); } }
                    Focus::Trouble => { for _ in 0..count { self.trouble.move_up(); } }
                }
            }
            Action::MoveDown => {
                match self.vim.focus {
                    Focus::Editor => { for _ in 0..count { self.editor.move_down(); } }
                    Focus::Explorer => { for _ in 0..count { self.explorer.move_down(); } }
                    Focus::Trouble => { for _ in 0..count { self.trouble.move_down(); } }
                }
            }
            Action::MoveWordForward => { for _ in 0..count { self.editor.move_word_forward(); } }
            Action::MoveWordBackward => { for _ in 0..count { self.editor.move_word_backward(); } }
            Action::MoveWordEnd => { for _ in 0..count { self.editor.move_word_end(); } }
            Action::MoveLineStart => { self.editor.move_to_line_start(); }
            Action::MoveLineEnd => { self.editor.move_to_line_end(); }
            Action::JumpToFirstLine => { self.editor.jump_to_first_line(); }
            Action::JumpToLastLine => { self.editor.jump_to_last_line(); }
            Action::MovePageUp => { let area = self.terminal.size().unwrap(); let h = area.height.saturating_sub(2) as usize; self.editor.move_page_up(h); }
            Action::MovePageDown => { let area = self.terminal.size().unwrap(); let h = area.height.saturating_sub(2) as usize; self.editor.move_page_down(h); }

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
                    self.editor.buffer_mut().apply_edit(|t| { t.remove((idx-1)..idx); });
                    self.editor.cursor_mut().x -= 1;
                    if let Some(path) = self.editor.buffer().file_path.clone() {
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                            let text = self.editor.buffer().text.to_string();
                            let _ = self.lsp_manager.did_change(&ext, &path, text);
                            let _ = self.lsp_manager.request_completions(&ext, &path, y, x - 1, CompletionTriggerKind::INVOKED, None);
                        }
                    }
                    self.refresh_filtered_suggestions();
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
                self.vim.set_message(format!("{} lines yanked", count));
            }
            Action::PasteAfter => { let t = self.vim.register.clone(); let y = self.vim.yank_type; self.editor.paste_after(&t, y); }
            Action::PasteBefore => { let t = self.vim.register.clone(); let y = self.vim.yank_type; self.editor.paste_before(&t, y); }
            Action::Undo => { self.editor.undo(); }
            Action::Redo => { self.editor.redo(); }
            Action::ToggleComment => { self.toggle_comment(); }
            Action::OpenLineBelow => { self.editor.open_line_below(); self.vim.mode = Mode::Insert; }
            Action::OpenLineAbove => { self.editor.open_line_above(); self.vim.mode = Mode::Insert; }
            Action::DeleteSelection => {
                if let Mode::Visual = self.vim.mode {
                    let start = self.vim.selection_start.unwrap();
                    let cur = self.editor.cursor();
                    self.vim.register = self.editor.delete_selection(start.x, start.y, cur.x, cur.y);
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
                self.editor.buffer_mut().apply_edit(|t| { t.insert(idx, &spaces); });
                self.editor.cursor_mut().x += self.vim.config.tabstop;
            }

            Action::TelescopeFiles => { self.vim.telescope.open(crate::vim::mode::TelescopeKind::Files, self.vim.project_root.clone(), &self.editor); self.vim.mode = Mode::Telescope(crate::vim::mode::TelescopeKind::Files); }
            Action::TelescopeLiveGrep => { self.vim.telescope.open(crate::vim::mode::TelescopeKind::Words, self.vim.project_root.clone(), &self.editor); self.vim.mode = Mode::Telescope(crate::vim::mode::TelescopeKind::Words); }
            Action::TelescopeBuffers => { self.vim.telescope.open(crate::vim::mode::TelescopeKind::Buffers, self.vim.project_root.clone(), &self.editor); self.vim.mode = Mode::Telescope(crate::vim::mode::TelescopeKind::Buffers); }
            Action::TelescopeThemes => { self.vim.telescope.open(crate::vim::mode::TelescopeKind::Themes, self.vim.project_root.clone(), &self.editor); self.vim.mode = Mode::Telescope(crate::vim::mode::TelescopeKind::Themes); }
            Action::LspDefinition => {
                if let Some(path) = self.editor.buffer().file_path.clone() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                        let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                        match self.lsp_manager.request_definition(&ext, &path, y, x) {
                            Ok(id) => { self.vim.definition_request_id = Some(id); }
                            Err(e) => { self.vim.set_message(format!("LSP Error: {}", e)); }
                        }
                    }
                }
            }
            Action::ToggleExplorer => {
                if self.explorer.visible {
                    if self.vim.focus == Focus::Explorer { self.explorer.visible = false; self.vim.focus = Focus::Editor; }
                    else { self.vim.focus = Focus::Explorer; self.sync_explorer(); }
                } else {
                    self.explorer.visible = true;
                    self.explorer.init_root();
                    self.vim.focus = Focus::Explorer;
                    self.sync_explorer();
                }
            }
            Action::ToggleRelativeNumber => { self.vim.relative_number = !self.vim.relative_number; }
            Action::ToggleTrouble => { self.trouble.toggle(); if self.trouble.visible { self.vim.focus = Focus::Trouble; } else { self.vim.focus = Focus::Editor; } }
            Action::ToggleAutoformat => { self.vim.config.disable_autoformat = !self.vim.config.disable_autoformat; self.vim.set_message(format!("Autoformat {}", if self.vim.config.disable_autoformat { "disabled" } else { "enabled" })); }
            Action::GitBlame => { self.vim.blame_popup = Some("Git Blame: You (just now) - placeholder".to_string()); }
            Action::ToggleFold => { self.editor.toggle_fold(&self.vim.folding_ranges); }
            Action::NextHunk => { self.editor.jump_to_next_hunk(); }
            Action::PrevHunk => { self.editor.jump_to_prev_hunk(); }
            Action::Format => { let _ = self.format_buffer(); }

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
            Action::ExplorerCollapse => { self.explorer.collapse(); }
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
            Action::ExplorerAdd => { self.vim.mode = Mode::ExplorerInput(ExplorerInputType::Add); self.vim.input_buffer.clear(); }
            Action::ExplorerRename => { self.vim.mode = Mode::ExplorerInput(ExplorerInputType::Rename); self.vim.input_buffer.clear(); }
            Action::ExplorerDelete => { self.vim.mode = Mode::ExplorerInput(ExplorerInputType::DeleteConfirm); self.vim.input_buffer.clear(); }
            Action::ExplorerMove => { self.vim.mode = Mode::ExplorerInput(ExplorerInputType::Move); self.vim.input_buffer.clear(); }
            Action::ExplorerFilter => { self.vim.mode = Mode::ExplorerInput(ExplorerInputType::Filter); self.vim.input_buffer.clear(); }
            Action::ExplorerOpenSystem => { self.explorer.open_in_system_explorer(); }
            Action::ExplorerToggleHidden => { self.explorer.show_hidden = !self.explorer.show_hidden; self.explorer.refresh(); }

            Action::SelectNext => {
                match self.vim.mode {
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
                            self.vim.selected_suggestion = (self.vim.selected_suggestion + 1) % self.vim.filtered_suggestions.len();
                            self.vim.suggestion_state.select(Some(self.vim.selected_suggestion));
                        } else {
                            self.editor.move_down();
                        }
                    }
                    _ => {}
                }
            }
            Action::SelectPrev => {
                match self.vim.mode {
                    Mode::Telescope(_) => self.vim.telescope.move_up(),
                    Mode::Mason => {
                        let i = self.vim.mason_state.selected().unwrap_or(0);
                        if i > 0 { self.vim.mason_state.select(Some(i - 1)); }
                    }
                    Mode::Keymaps => {
                        let i = self.vim.keymap_state.selected().unwrap_or(0);
                        if i > 0 { self.vim.keymap_state.select(Some(i - 1)); }
                    }
                    Mode::Insert => {
                        if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                            if self.vim.selected_suggestion > 0 {
                                self.vim.selected_suggestion -= 1;
                            } else {
                                self.vim.selected_suggestion = self.vim.filtered_suggestions.len() - 1;
                            }
                            self.vim.suggestion_state.select(Some(self.vim.selected_suggestion));
                        } else {
                            self.editor.move_up();
                        }
                    }
                    _ => {}
                }
            }

            Action::Confirm => {
                if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                    let selected = &self.vim.filtered_suggestions[self.vim.selected_suggestion];
                    let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                    let line = self.editor.buffer().line(y).unwrap().to_string();
                    let mut start_x = x;
                    let chars: Vec<char> = line.chars().collect();
                    while start_x > 0 && (chars[start_x-1].is_alphanumeric() || chars[start_x-1] == '_' || chars[start_x-1] == '$') { start_x -= 1; }
                    let line_start_char = self.editor.buffer().text.line_to_char(y);
                    self.editor.buffer_mut().apply_edit(|t| {
                        t.remove((line_start_char + start_x)..(line_start_char + x));
                        t.insert(line_start_char + start_x, &selected.label);
                    });
                    self.editor.cursor_mut().x = start_x + selected.label.len();
                    self.vim.show_suggestions = false;
                    self.vim.filtered_suggestions.clear();
                    self.vim.suggestions.clear();
                } else if let Mode::Telescope(_) = self.vim.mode {
                    if let Some(result) = self.vim.telescope.results.get(self.vim.telescope.selected_idx) {
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
                } else if let Mode::Insert = self.vim.mode {
                    let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                    let idx = self.editor.buffer().text.line_to_char(y) + x;
                    self.editor.buffer_mut().apply_edit(|t| { t.insert(idx, "\n"); });
                    self.editor.cursor_mut().y += 1; self.editor.cursor_mut().x = 0;
                }
            }

            _ => {}
        }
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            let area = self.terminal.size()?;
            let visible_height = area.height.saturating_sub(2) as usize;

            // 0. Updates
            if let Some(time) = self.vim.message_time {
                if time.elapsed().as_secs() >= 3 { self.vim.message = None; self.vim.message_time = None; }
            }

            if self.vim.last_git_update.is_none() || self.vim.last_git_update.unwrap().elapsed() > Duration::from_secs(5) {
                self.vim.git_info = update_git_info(&self.vim.project_root);
                for buffer in &mut self.editor.buffers {
                    if let Some(path) = &buffer.file_path {
                        let text = buffer.text.to_string();
                        buffer.git_signs = self.vim.git_manager.get_signs(path, &text);
                    }
                }
                self.vim.last_git_update = Some(Instant::now());
            }

            // File Watcher Events
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

            for _path in buffers_to_reload {
                if !self.editor.buffer().modified {
                    if let Err(e) = self.editor.buffer_mut().reload() {
                        self.vim.set_message(format!("Error reloading file: {}", e));
                    } else {
                        self.editor.refresh_syntax();
                    }
                }
            }

            // LSP ensure/debouncing/message processing
            if let Some(path) = self.editor.buffer().file_path.clone() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) {
                    let _ = self.lsp_manager.start_client(&ext, self.vim.project_root.clone());
                }
            }

            // Process LSP messages
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
                                            let _ = client.send_notification("initialized", serde_json::json!({}));
                                            newly_ready_clients.push((ext.clone(), cmd.clone()));
                                        }
                                    }
                                }
                            } else if Some(id) == self.vim.definition_request_id {
                                self.vim.definition_request_id = None;
                                if let Ok(value) = serde_json::from_value::<GotoDefinitionResponse>(resp.result.unwrap_or_default()) {
                                    match value {
                                        GotoDefinitionResponse::Scalar(loc) => {
                                            let path = PathBuf::from(loc.uri.to_file_path().unwrap());
                                            let pos = Position { x: loc.range.start.character as usize, y: loc.range.start.line as usize };
                                            let _ = self.editor.open_file(path);
                                            self.editor.cursor_mut().y = pos.y;
                                            self.editor.cursor_mut().x = pos.x;
                                            self.sync_explorer();
                                        }
                                        _ => {}
                                    }
                                }
                            } else {
                                if let Ok(value) = serde_json::from_value::<CompletionResponse>(resp.result.unwrap_or_default()) {
                                    match value {
                                        CompletionResponse::Array(items) => { self.vim.suggestions = items; }
                                        CompletionResponse::List(list) => { self.vim.suggestions = list.items; }
                                    }
                                    self.refresh_filtered_suggestions();
                                }
                            }
                        }
                    }
                    lsp_server::Message::Notification(notif) => {
                        if notif.method == "textDocument/publishDiagnostics" {
                            if let Ok(params) = serde_json::from_value::<PublishDiagnosticsParams>(notif.params) {
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
                        if path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) == Some(ext.clone()) {
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
                            MouseEventKind::ScrollUp => { if let Mode::Telescope(_) = self.vim.mode { self.vim.telescope.scroll_preview_up(3); } else { self.editor.move_up(); } }
                            MouseEventKind::ScrollDown => { if let Mode::Telescope(_) = self.vim.mode { self.vim.telescope.scroll_preview_down(3); } else { self.editor.move_down(); } }
                            _ => {}
                        }
                    }
                    if let Event::Key(key) = event {
                        self.vim.show_intro = false;
                        self.vim.yank_highlight_line = None;
                        if self.vim.blame_popup.is_some() { self.vim.blame_popup = None; continue; }
                        
                        match self.vim.mode {
                            Mode::Normal => {
                                match self.vim.focus {
                                    Focus::Editor => {
                                        let action = self.keymap_normal.resolve(&key);
                                        match action {
                                            Action::Unbound => {
                                                if let KeyCode::Char(c) = key.code {
                                                    if c.is_ascii_digit() && (self.vim.input_buffer.is_empty() || self.vim.input_buffer.chars().all(|c| c.is_ascii_digit())) {
                                                        self.vim.input_buffer.push(c);
                                                        continue;
                                                    }
                                                    
                                                    let count = if !self.vim.input_buffer.is_empty() && self.vim.input_buffer.chars().all(|c| c.is_ascii_digit()) {
                                                        let c_val = self.vim.input_buffer.parse::<usize>().unwrap_or(1);
                                                        self.vim.input_buffer.clear();
                                                        c_val
                                                    } else { 1 };
                                                    
                                                    self.vim.input_buffer.push(c);
                                                    let seq = self.vim.input_buffer.clone();
                                                    let mut matched = true;
                                                    match seq.as_str() {
                                                        " ff" => self.dispatch_action(Action::TelescopeFiles, count),
                                                        " fg" => self.dispatch_action(Action::TelescopeLiveGrep, count),
                                                        " fb" => self.dispatch_action(Action::TelescopeBuffers, count),
                                                        " th" | "th" => self.dispatch_action(Action::TelescopeThemes, count),
                                                        " n" => self.dispatch_action(Action::ToggleRelativeNumber, count),
                                                        " /" => self.dispatch_action(Action::ToggleComment, count),
                                                        " tt" => self.dispatch_action(Action::ToggleTrouble, count),
                                                        " bb" => self.dispatch_action(Action::ToggleAutoformat, count),
                                                        " bl" => self.dispatch_action(Action::GitBlame, count),
                                                        " x" => self.dispatch_action(Action::CloseBuffer, count),
                                                        "gg" => self.dispatch_action(Action::JumpToFirstLine, count),
                                                        "dd" => self.dispatch_action(Action::DeleteLine, count),
                                                        "yy" => self.dispatch_action(Action::YankLine, count),
                                                        "[[" => self.dispatch_action(Action::JumpToFirstLine, count),
                                                        "]]" => self.dispatch_action(Action::JumpToLastLine, count),
                                                        "gd" => self.dispatch_action(Action::LspDefinition, count),
                                                        "zc" | "za" => self.dispatch_action(Action::ToggleFold, count),
                                                        "]g" => self.dispatch_action(Action::NextHunk, count),
                                                        "[g" => self.dispatch_action(Action::PrevHunk, count),
                                                        _ => { matched = false; }
                                                    }

                                                    if matched {
                                                        self.vim.input_buffer.clear();
                                                    } else {
                                                        let is_partial = match seq.as_str() { " " | " f" | " t" | " g" | " b" | "[" | "]" | "z" | "d" | "y" | "g" => true, _ => false, };
                                                        if !is_partial {
                                                            self.vim.input_buffer.clear();
                                                        }
                                                    }
                                                } else {
                                                    self.vim.input_buffer.clear();
                                                    match key.code {
                                                        KeyCode::Esc => { self.vim.input_buffer.clear(); self.vim.selection_start = None; }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                            action => {
                                                let count = if !self.vim.input_buffer.is_empty() && self.vim.input_buffer.chars().all(|c| c.is_ascii_digit()) {
                                                    let c_val = self.vim.input_buffer.parse::<usize>().unwrap_or(1);
                                                    self.vim.input_buffer.clear();
                                                    c_val
                                                } else { 1 };
                                                self.vim.input_buffer.clear();
                                                self.dispatch_action(action.clone(), count);
                                            }
                                        }
                                    }
                                    Focus::Explorer => {
                                        let action = self.keymap_explorer.resolve(&key);
                                        match action {
                                            Action::Unbound => {
                                                match key.code {
                                                    KeyCode::Char('<') => self.explorer.decrease_width(),
                                                    KeyCode::Char('>') => self.explorer.increase_width(),
                                                    KeyCode::Char('y') => { if let Some(entry) = self.explorer.selected_entry() { self.vim.register = entry.path.to_string_lossy().to_string(); self.vim.set_message("Path copied to register".to_string()); } }
                                                    _ => {}
                                                }
                                            }
                                            action => self.dispatch_action(action.clone(), 1),
                                        }
                                    }
                                    Focus::Trouble => {
                                        match key.code {
                                            KeyCode::Char('j') | KeyCode::Down => self.trouble.move_down(),
                                            KeyCode::Char('k') | KeyCode::Up => self.trouble.move_up(),
                                            KeyCode::Enter => { if let Some(item) = self.trouble.selected_item() { let path = item.path.clone(); let line = item.line; let col = item.col; if let Err(e) = self.editor.open_file(path) { self.vim.set_message(format!("Error: {}", e)); } else { self.editor.cursor_mut().y = line; self.editor.cursor_mut().x = col; self.vim.focus = Focus::Editor; } } }
                                            KeyCode::Esc | KeyCode::Char('q') => self.dispatch_action(Action::ExitMode, 1),
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Mode::Visual => {
                                match key.code {
                                    KeyCode::Esc => self.dispatch_action(Action::ExitMode, 1),
                                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => self.dispatch_action(Action::Save, 1),
                                    KeyCode::Char('j') | KeyCode::Down => self.dispatch_action(Action::MoveDown, 1),
                                    KeyCode::Char('k') | KeyCode::Up => self.dispatch_action(Action::MoveUp, 1),
                                    KeyCode::Char('h') | KeyCode::Left => self.dispatch_action(Action::MoveLeft, 1),
                                    KeyCode::Char('l') | KeyCode::Right => self.dispatch_action(Action::MoveRight, 1),
                                    KeyCode::PageUp => self.dispatch_action(Action::MovePageUp, 1),
                                    KeyCode::PageDown => self.dispatch_action(Action::MovePageDown, 1),
                                    KeyCode::Home => self.dispatch_action(Action::MoveLineStart, 1),
                                    KeyCode::End => self.dispatch_action(Action::MoveLineEnd, 1),
                                    KeyCode::Char('w') => self.dispatch_action(Action::MoveWordForward, 1),
                                    KeyCode::Char('b') => self.dispatch_action(Action::MoveWordBackward, 1),
                                    KeyCode::Char('p') => self.dispatch_action(Action::PasteAfter, 1),
                                    KeyCode::Char('s') if !key.modifiers.contains(KeyModifiers::CONTROL) => self.dispatch_action(Action::DeleteSelection, 1),
                                    KeyCode::Char('y') => self.dispatch_action(Action::YankLine, 1),
                                    KeyCode::Char('d') | KeyCode::Char('x') => self.dispatch_action(Action::DeleteSelection, 1),
                                    _ => {}
                                }
                            }
                            Mode::Insert => {
                                let action = self.keymap_insert.resolve(&key);
                                match action {
                                    Action::ExitMode => self.dispatch_action(Action::ExitMode, 1),
                                    Action::Save => self.dispatch_action(Action::Save, 1),
                                    Action::Confirm => self.dispatch_action(Action::Confirm, 1),
                                    Action::Indent => self.dispatch_action(Action::Indent, 1),
                                    _ => {
                                        match key.code {
                                            KeyCode::Up => {
                                                if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                                                    if self.vim.selected_suggestion > 0 { self.vim.selected_suggestion -= 1; } 
                                                    else { self.vim.selected_suggestion = self.vim.filtered_suggestions.len() - 1; }
                                                    self.vim.suggestion_state.select(Some(self.vim.selected_suggestion));
                                                } else {
                                                    self.editor.move_up();
                                                }
                                            }
                                            KeyCode::Down => {
                                                if self.vim.show_suggestions && !self.vim.filtered_suggestions.is_empty() {
                                                    self.vim.selected_suggestion = (self.vim.selected_suggestion + 1) % self.vim.filtered_suggestions.len();
                                                    self.vim.suggestion_state.select(Some(self.vim.selected_suggestion));
                                                } else {
                                                    self.editor.move_down();
                                                }
                                            }
                                            KeyCode::Left => self.editor.move_left(),
                                            KeyCode::Right => self.editor.move_right(),
                                            KeyCode::Char(' ') | KeyCode::Null if key.modifiers.contains(KeyModifiers::CONTROL) || key.code == KeyCode::Null => { if let Some(path) = self.editor.buffer().file_path.clone() { if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) { let (y, x) = (self.editor.cursor().y, self.editor.cursor().x); let _ = self.lsp_manager.request_completions(&ext, &path, y, x, CompletionTriggerKind::INVOKED, None); } } }
                                            KeyCode::Char(c) => {
                                                let (y, x) = (self.editor.cursor().y, self.editor.cursor().x);
                                                let idx = self.editor.buffer().text.line_to_char(y) + x;
                                                let mut to_insert = c.to_string();
                                                match c {
                                                    '(' => { to_insert.push(')'); }
                                                    '[' => { to_insert.push(']'); }
                                                    '{' => { to_insert.push('}'); }
                                                    '\'' => { to_insert.push('\''); }
                                                    '"' => { to_insert.push('"'); }
                                                    '>' => { if let Some(line) = self.editor.buffer().line(y) { let line_str = line.to_string(); let before_cursor = &line_str[..x.min(line_str.len())]; if let Some(tag_start) = before_cursor.rfind('<') { let tag_content = &before_cursor[tag_start+1..]; if !tag_content.is_empty() && !tag_content.contains(' ') && !tag_content.contains('/') { to_insert.push_str(&format!("</{}>", tag_content)); } } } }
                                                    _ => {}
                                                }
                                                self.editor.buffer_mut().apply_edit(|t| { t.insert(idx, &to_insert); });
                                                self.editor.cursor_mut().x += 1;
                                                if let Some(path) = self.editor.buffer().file_path.clone() { if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) { let text = self.editor.buffer().text.to_string(); let _ = self.lsp_manager.did_change(&ext, &path, text); let trigger_kind = if c == '.' || c == ':' || c == '>' { CompletionTriggerKind::TRIGGER_CHARACTER } else { CompletionTriggerKind::INVOKED }; let trigger_char = if trigger_kind == CompletionTriggerKind::TRIGGER_CHARACTER { Some(c.to_string()) } else { None }; let _ = self.lsp_manager.request_completions(&ext, &path, y, x + 1, trigger_kind, trigger_char); } }
                                                self.refresh_filtered_suggestions();
                                            }
                                            KeyCode::Backspace => { let (y, x) = (self.editor.cursor().y, self.editor.cursor().x); if x > 0 { let idx = self.editor.buffer().text.line_to_char(y) + x; self.editor.buffer_mut().apply_edit(|t| { t.remove((idx-1)..idx); }); self.editor.cursor_mut().x -= 1; if let Some(path) = self.editor.buffer().file_path.clone() { if let Some(ext) = path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) { let text = self.editor.buffer().text.to_string(); let _ = self.lsp_manager.did_change(&ext, &path, text); let _ = self.lsp_manager.request_completions(&ext, &path, y, x - 1, CompletionTriggerKind::INVOKED, None); } } self.refresh_filtered_suggestions(); } }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Mode::Search => { match key.code { KeyCode::Esc => { self.vim.mode = Mode::Normal; } KeyCode::Char(c) => { self.vim.search_query.push(c); } KeyCode::Backspace => { self.vim.search_query.pop(); } KeyCode::Enter => { self.vim.mode = Mode::Normal; } _ => {} } }
                            Mode::ExplorerInput(input_type) => { match key.code { KeyCode::Esc => { if let ExplorerInputType::Filter = input_type { self.explorer.filter.clear(); self.explorer.refresh(); } self.vim.mode = Mode::Normal; } KeyCode::Char(c) => { self.vim.input_buffer.push(c); if let ExplorerInputType::Filter = input_type { self.explorer.filter = self.vim.input_buffer.clone(); self.explorer.refresh(); } } KeyCode::Backspace => { self.vim.input_buffer.pop(); if let ExplorerInputType::Filter = input_type { self.explorer.filter = self.vim.input_buffer.clone(); self.explorer.refresh(); } } KeyCode::Enter => { let input = self.vim.input_buffer.clone(); self.vim.input_buffer.clear(); self.vim.mode = Mode::Normal; match input_type { ExplorerInputType::Add => { if let Err(e) = self.explorer.create_file(&input) { self.vim.set_message(format!("Error: {}", e)); } } ExplorerInputType::Rename => { if let Err(e) = self.explorer.rename_selected(&input) { self.vim.set_message(format!("Error: {}", e)); } } ExplorerInputType::Move => { if let Err(e) = self.explorer.move_selected(Path::new(&input)) { self.vim.set_message(format!("Error: {}", e)); } } ExplorerInputType::DeleteConfirm => { if input.to_lowercase() == "y" { if let Err(e) = self.explorer.delete_selected() { self.vim.set_message(format!("Error: {}", e)); } } } ExplorerInputType::Filter => { self.explorer.filter = input; self.explorer.refresh(); } } } _ => {} } }
                            Mode::Confirm(action) => { match key.code { KeyCode::Char('y') | KeyCode::Char('Y') => { match action { crate::vim::mode::ConfirmAction::Quit => { self.save_and_format(None); self.should_quit = true; } crate::vim::mode::ConfirmAction::CloseBuffer => { self.save_and_format(None); self.editor.close_current_buffer(); self.vim.mode = Mode::Normal; } } } KeyCode::Char('n') | KeyCode::Char('N') => { match action { crate::vim::mode::ConfirmAction::Quit => { self.should_quit = true; } crate::vim::mode::ConfirmAction::CloseBuffer => { self.editor.close_current_buffer(); self.vim.mode = Mode::Normal; } } } KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => { self.vim.mode = Mode::Normal; } _ => {} } }
                            Mode::Telescope(_) => {
                                match key.code {
                                    KeyCode::Esc => self.dispatch_action(Action::ExitMode, 1),
                                    KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => self.dispatch_action(Action::SelectNext, 1),
                                    KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => self.dispatch_action(Action::SelectPrev, 1),
                                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => self.vim.telescope.scroll_preview_up(5),
                                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => self.vim.telescope.scroll_preview_down(5),
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
                                }
                            }
                            Mode::Mason => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('q') => self.dispatch_action(Action::ExitMode, 1),
                                    KeyCode::Char('j') | KeyCode::Down => self.dispatch_action(Action::SelectNext, 1),
                                    KeyCode::Char('k') | KeyCode::Up => self.dispatch_action(Action::SelectPrev, 1),
                                    KeyCode::Char('1') => { self.vim.mason_tab = 0; self.vim.mason_state.select(Some(0)); }
                                    KeyCode::Char('2') => { self.vim.mason_tab = 1; self.vim.mason_state.select(Some(0)); }
                                    KeyCode::Char('3') => { self.vim.mason_tab = 2; self.vim.mason_state.select(Some(0)); }
                                    KeyCode::Char('4') => { self.vim.mason_tab = 3; self.vim.mason_state.select(Some(0)); }
                                    KeyCode::Char('5') => { self.vim.mason_tab = 4; self.vim.mason_state.select(Some(0)); }
                                    KeyCode::Char('6') => { self.vim.mason_tab = 5; self.vim.mason_state.select(Some(0)); }
                                    KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        self.vim.mode = Mode::MasonFilter;
                                        self.vim.mason_filter.clear();
                                    }
                                    KeyCode::Char(' ') | KeyCode::Char('i') | KeyCode::Char('u') | KeyCode::Char('d') | KeyCode::Char('x') => self.install_selected_package(),
                                    _ => {}
                                }
                            }
                            Mode::MasonFilter => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Enter => { self.vim.mode = Mode::Mason; }
                                    KeyCode::Char(c) => {
                                        self.vim.mason_filter.push(c);
                                        self.vim.mason_state.select(Some(0));
                                    }
                                    KeyCode::Backspace => {
                                        self.vim.mason_filter.pop();
                                        self.vim.mason_state.select(Some(0));
                                    }
                                    _ => {}
                                }
                            }
                            Mode::Keymaps => {
                                match key.code {
                                    KeyCode::Esc | KeyCode::Char('?') => self.dispatch_action(Action::ExitMode, 1),
                                    KeyCode::Char('j') | KeyCode::Down => self.dispatch_action(Action::SelectNext, 1),
                                    KeyCode::Char('k') | KeyCode::Up => self.dispatch_action(Action::SelectPrev, 1),
                                    KeyCode::Char(c) => {
                                        self.vim.keymap_filter.push(c);
                                        self.vim.keymap_state.select(Some(0));
                                    }
                                    KeyCode::Backspace => {
                                        self.vim.keymap_filter.pop();
                                        self.vim.keymap_state.select(Some(0));
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
                                    "set", "config", "help", "checkhealth"
                                ];
                                match key.code {
                                    KeyCode::Esc => { self.vim.mode = Mode::Normal; self.vim.command_suggestions.clear(); }
                                    KeyCode::Char(c) => { 
                                        self.vim.command_buffer.push(c); 
                                        self.vim.command_suggestions = commands.iter().filter(|cmd| cmd.starts_with(&self.vim.command_buffer)).map(|s| s.to_string()).collect(); 
                                        self.vim.selected_command_suggestion = 0; 
                                    }
                                    KeyCode::Backspace => { 
                                        self.vim.command_buffer.pop(); 
                                        if self.vim.command_buffer.is_empty() { self.vim.command_suggestions.clear(); } 
                                        else { self.vim.command_suggestions = commands.iter().filter(|cmd| cmd.starts_with(&self.vim.command_buffer)).map(|s| s.to_string()).collect(); } 
                                        self.vim.selected_command_suggestion = 0; 
                                    }
                                    KeyCode::Tab => { if !self.vim.command_suggestions.is_empty() { self.vim.selected_command_suggestion = (self.vim.selected_command_suggestion + 1) % self.vim.command_suggestions.len(); } }
                                    KeyCode::Enter => {
                                        let cmd_str = if !self.vim.command_suggestions.is_empty() { self.vim.command_suggestions[self.vim.selected_command_suggestion].clone() } else { self.vim.command_buffer.trim().to_string() };
                                        self.vim.command_buffer.clear(); self.vim.command_suggestions.clear(); self.vim.mode = Mode::Normal;
                                        if !cmd_str.is_empty() {
                                            let mut parts = cmd_str.split_whitespace();
                                            let first_part = parts.next().unwrap_or("");
                                            let force = first_part.ends_with('!');
                                            let cmd = if force { &first_part[..first_part.len()-1] } else { first_part };
                                            let args: Vec<&str> = parts.collect();
                                            if let Ok(line) = cmd.parse::<usize>() { 
                                                self.editor.cursor_mut().y = line.saturating_sub(1); self.editor.clamp_cursor(); 
                                            } else {
                                                match cmd {
                                                    "q" | "quit" => self.dispatch_action(if force { Action::QuitAll } else { Action::Quit }, 1),
                                                    "qa" | "qall" => self.dispatch_action(Action::QuitAll, 1),
                                                    "w" | "write" => { let path = args.get(0).map(|s| PathBuf::from(*s)); self.save_and_format(path); }
                                                    "wa" | "wall" => { let current = self.editor.active_idx; for i in 0..self.editor.buffers.len() { self.editor.active_idx = i; self.save_and_format(None); } self.editor.active_idx = current; }
                                                    "wq" | "x" => { self.save_and_format(None); self.dispatch_action(Action::Quit, 1); }
                                                    "wqa" | "xa" => { let current = self.editor.active_idx; for i in 0..self.editor.buffers.len() { self.editor.active_idx = i; self.save_and_format(None); } self.editor.active_idx = current; self.should_quit = true; }
                                                    "bn" | "bnext" => self.dispatch_action(Action::NextBuffer, 1),
                                                    "bp" | "bprev" => self.dispatch_action(Action::PrevBuffer, 1),
                                                    "bd" | "bdelete" => self.dispatch_action(if force { Action::CloseBuffer } else { Action::CloseBuffer }, 1), // simplified
                                                    "e" | "edit" => { if let Some(p) = args.get(0) { let _ = self.editor.open_file(PathBuf::from(*p)); self.sync_explorer(); } }
                                                    "e!" | "Reload" => self.dispatch_action(Action::ReloadFile, 1),
                                                    "colorscheme" => { if let Some(theme) = args.get(0) { self.editor.set_theme(theme); } else { self.dispatch_action(Action::TelescopeThemes, 1); } }
                                                    "Mason" => self.dispatch_action(Action::EnterMason, 1),
                                                    "Trouble" => self.dispatch_action(Action::ToggleTrouble, 1),
                                                    "format" | "Format" => self.dispatch_action(Action::Format, 1),
                                                    "FormatAll" => { let current = self.editor.active_idx; for i in 0..self.editor.buffers.len() { self.editor.active_idx = i; let _ = self.format_buffer(); } self.editor.active_idx = current; }
                                                    "FormatEnable" => { self.vim.config.disable_autoformat = false; }
                                                    "FormatDisable" => { self.vim.config.disable_autoformat = true; }
                                                    "gd" | "Definition" => self.dispatch_action(Action::LspDefinition, 1),
                                                    "set" => { if let Some(arg) = args.get(0) { match *arg { "number" => self.vim.config.number = true, "nonumber" => self.vim.config.number = false, "relativenumber" => self.vim.config.relativenumber = true, "norelativenumber" => self.vim.config.relativenumber = false, _ => {} } } }
                                                    "config" => { let _ = self.vim.config.save(); }
                                                    "help" => { self.dispatch_action(Action::EnterKeymaps, 1); } // or actual help
                                                    _ => {}
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
            if self.should_quit { break; }

            if self.trouble.visible && !self.trouble.scanned {
                let todos = crate::editor::todo::scan_project_todos(&self.vim.project_root);
                let diagnostics = self.lsp_manager.diagnostics.lock().unwrap();
                self.trouble.update_from_lsp(&diagnostics, todos);
                self.trouble.scanned = true;
            }

            let editor_width = if self.explorer.visible { (area.width as f32 * 0.85) as usize - 8 } else { area.width as usize - 8 };
            self.editor.scroll_into_view(visible_height, editor_width, self.vim.config.wrap);
            self.editor.refresh_syntax();
            self.terminal.draw(|f| self.ui.draw(f, &self.editor, &mut self.vim, &mut self.explorer, &self.trouble, &self.lsp_manager))?;

            let cursor_style = match self.vim.mode { Mode::Insert => SetCursorStyle::SteadyBar, _ => SetCursorStyle::SteadyBlock, };
            execute!(self.terminal.backend_mut(), cursor_style)?;
        }

        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture, SetCursorStyle::DefaultUserShape)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

pub fn find_project_root(path: &PathBuf) -> PathBuf {
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

pub fn update_git_info(project_root: &PathBuf) -> Option<crate::vim::GitInfo> {
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

    let mut info = crate::vim::GitInfo {
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
