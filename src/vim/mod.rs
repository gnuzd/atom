pub mod mode;
pub mod motion;

use mode::{Focus, SplitKind, YankType};
use lsp_types::CompletionItem;
use std::time::Instant;
use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspStatus {
    None,
    Loading,
    Ready,
    Installing,
    Formatting,
    Error(String),
}

use ratatui::widgets::{ListState, TableState};

#[derive(Debug, Clone, Default)]
pub struct GitInfo {
    pub branch: String,
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
}

pub struct VimState {
    pub mode: mode::Mode,
    pub focus: mode::Focus,
    pub command_buffer: String,
    pub input_buffer: String,
    pub selection_start: Option<Position>,
    pub search_query: String,
    pub register: String,
    pub yank_type: YankType,
    pub pending_op: Option<char>,
    pub yank_highlight_line: Option<usize>,
    pub suggestions: Vec<CompletionItem>,
    pub filtered_suggestions: Vec<CompletionItem>,
    pub selected_suggestion: usize,
    pub suggestion_state: ListState,
    pub keymap_state: TableState,
    pub nucleus_state: ListState,
    pub theme_state: ListState,
    pub nucleus_tab: usize,
    pub nucleus_filter: String,
    pub nucleus_pending_delete: Option<String>,
    pub block_insert_text: String,
    pub block_insert_col: usize,
    pub split: Option<SplitKind>,
    pub split_buffer_idx: usize,
    pub split_focused: bool, // false = primary pane, true = split pane
    pub preview_lines: Option<Vec<String>>, // floating file preview
    pub preview_scroll: usize,
    pub show_suggestions: bool,
    pub keymap_filter: String,
    pub command_suggestions: Vec<String>,
    pub selected_command_suggestion: usize,
    pub lsp_to_install: Option<String>,
    pub lsp_status: LspStatus,
    pub spinner_idx: usize,
    pub last_lsp_id: i32,
    pub config: Config,
    pub message: Option<String>,
    pub message_time: Option<Instant>,
    pub telescope: crate::ui::telescope::Telescope,
    pub project_root: std::path::PathBuf,
    pub count: Option<usize>,
    pub relative_number: bool,
    pub show_number: bool,
    pub show_diagnostics: bool,
    pub git_info: Option<GitInfo>,
    pub git_manager: crate::git::GitManager,
    pub blame_popup: Option<String>,
    pub last_git_update: Option<Instant>,
    pub show_intro: bool,
    pub folding_ranges: Vec<lsp_types::FoldingRange>,
    pub definition_request_id: Option<i32>,
    pub jumplist: Vec<(std::path::PathBuf, Position)>,
    pub jumplist_idx: usize,
}

impl VimState {
    pub fn new(config: Config, project_root: std::path::PathBuf) -> Self {
        let mut suggestion_state = ListState::default();
        suggestion_state.select(Some(0));
        let mut keymap_state = TableState::default();
        keymap_state.select(Some(0));
        let mut nucleus_state = ListState::default();
        nucleus_state.select(Some(0));
        let mut theme_state = ListState::default();
        theme_state.select(Some(0));

        Self {
            mode: mode::Mode::Normal,
            focus: Focus::Editor,
            command_buffer: String::new(),
            input_buffer: String::new(),
            selection_start: None,
            search_query: String::new(),
            register: String::new(),
            yank_type: YankType::Char,
            pending_op: None,
            yank_highlight_line: None,
            suggestions: Vec::new(),
            filtered_suggestions: Vec::new(),
            selected_suggestion: 0,
            suggestion_state,
            keymap_state,
            nucleus_state,
            theme_state,
            nucleus_tab: 0,
            nucleus_filter: String::new(),
            nucleus_pending_delete: None,
            block_insert_text: String::new(),
            block_insert_col: 0,
            split: None,
            split_buffer_idx: 0,
            split_focused: false,
            preview_lines: None,
            preview_scroll: 0,
            show_suggestions: false,
            keymap_filter: String::new(),
            command_suggestions: Vec::new(),
            selected_command_suggestion: 0,
            lsp_to_install: None,
            lsp_status: LspStatus::None,
            spinner_idx: 0,
            last_lsp_id: 0,
            message: None,
            message_time: None,
            telescope: crate::ui::telescope::Telescope::new(),
            project_root: project_root.clone(),
            count: None,
            relative_number: config.relativenumber,
            show_number: config.number,
            show_diagnostics: true,
            config,
            git_info: None,
            git_manager: crate::git::GitManager::new(&project_root),
            blame_popup: None,
            last_git_update: None,
            show_intro: false,
            folding_ranges: Vec::new(),
            definition_request_id: None,
            jumplist: Vec::new(),
            jumplist_idx: 0,
        }
    }

    pub fn push_jump(&mut self, path: std::path::PathBuf, pos: Position) {
        if self.jumplist_idx < self.jumplist.len() {
            self.jumplist.truncate(self.jumplist_idx);
        }
        self.jumplist.push((path, pos));
        if self.jumplist.len() > 100 {
            self.jumplist.remove(0);
        }
        self.jumplist_idx = self.jumplist.len();
    }

    pub fn disable_autoformat(&self) -> bool {
        self.config.disable_autoformat
    }

    pub fn set_message(&mut self, text: String) {
        self.message = Some(text);
        self.message_time = Some(Instant::now());
    }

    pub fn get_spinner(&mut self) -> &str {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = frames[(self.spinner_idx / 10) % frames.len()];
        self.spinner_idx = self.spinner_idx.wrapping_add(1);
        frame
    }

    pub fn reinit_git(&mut self) {
        self.git_manager = crate::git::GitManager::new(&self.project_root);
        self.last_git_update = None; // Force update
    }
}
