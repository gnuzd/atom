pub mod mode;
pub mod motion;

use mode::{Focus, YankType};
use lsp_types::CompletionItem;
use std::time::Instant;

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

use ratatui::widgets::ListState;

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
    pub selected_suggestion: usize,
    pub suggestion_state: ListState,
    pub keymap_state: ListState,
    pub show_suggestions: bool,
    pub lsp_to_install: Option<String>,
    pub lsp_status: LspStatus,
    pub spinner_idx: usize,
    pub disable_autoformat: bool,
    pub message: Option<String>,
    pub message_time: Option<Instant>,
}

impl VimState {
    pub fn new() -> Self {
        let mut suggestion_state = ListState::default();
        suggestion_state.select(Some(0));
        let keymap_state = ListState::default();
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
            selected_suggestion: 0,
            suggestion_state,
            keymap_state,
            show_suggestions: false,
            lsp_to_install: None,
            lsp_status: LspStatus::None,
            spinner_idx: 0,
            disable_autoformat: false,
            message: None,
            message_time: None,
        }
    }

    pub fn set_message(&mut self, text: String) {
        self.message = Some(text);
        self.message_time = Some(Instant::now());
    }

    pub fn get_spinner(&mut self) -> &str {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = frames[self.spinner_idx % frames.len()];
        self.spinner_idx += 1;
        frame
    }
}
