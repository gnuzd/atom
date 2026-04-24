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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pane {
    pub id: usize,
    pub buffer_idx: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneLayout {
    Window(Pane),
    Split(SplitKind, Vec<PaneLayout>),
}

impl PaneLayout {
    pub fn split(&mut self, target_id: usize, new_pane: Pane, kind: SplitKind) -> bool {
        match self {
            PaneLayout::Window(pane) => {
                if pane.id == target_id {
                    let old_pane = pane.clone();
                    *self = PaneLayout::Split(kind, vec![PaneLayout::Window(old_pane), PaneLayout::Window(new_pane)]);
                    return true;
                }
                false
            }
            PaneLayout::Split(split_kind, children) => {
                let mut found = false;
                let mut insert_idx = 0;
                for (i, child) in children.iter_mut().enumerate() {
                    if child.split(target_id, new_pane.clone(), kind) {
                        return true;
                    }
                    if let PaneLayout::Window(p) = child {
                        if p.id == target_id {
                            found = true;
                            insert_idx = i;
                            break;
                        }
                    }
                }
                if found {
                    if *split_kind == kind {
                        children.insert(insert_idx + 1, PaneLayout::Window(new_pane));
                        return true;
                    } else {
                        let old_pane = match children.remove(insert_idx) {
                            PaneLayout::Window(p) => p,
                            _ => unreachable!(),
                        };
                        children.insert(insert_idx, PaneLayout::Split(kind, vec![PaneLayout::Window(old_pane), PaneLayout::Window(new_pane)]));
                        return true;
                    }
                }
                false
            }
        }
    }

    pub fn remove_pane(&mut self, target_id: usize) -> bool {
        match self {
            PaneLayout::Window(_) => false,
            PaneLayout::Split(_, children) => {
                for i in 0..children.len() {
                    if let PaneLayout::Window(p) = &children[i] {
                        if p.id == target_id {
                            children.remove(i);
                            return true;
                        }
                    } else {
                        if children[i].remove_pane(target_id) {
                            if let PaneLayout::Split(_, sub_children) = &children[i] {
                                if sub_children.len() == 1 {
                                    children[i] = sub_children[0].clone();
                                }
                            }
                            return true;
                        }
                    }
                }
                if children.len() == 1 {
                    let mut temp = vec![];
                    std::mem::swap(children, &mut temp);
                    *self = temp.into_iter().next().unwrap();
                }
                false
            }
        }
    }

    pub fn get_all_panes(&self) -> Vec<Pane> {
        match self {
            PaneLayout::Window(pane) => vec![pane.clone()],
            PaneLayout::Split(_, children) => {
                let mut res = Vec::new();
                for child in children {
                    res.extend(child.get_all_panes());
                }
                res
            }
        }
    }

    pub fn get_pane_mut(&mut self, target_id: usize) -> Option<&mut Pane> {
        match self {
            PaneLayout::Window(pane) => {
                if pane.id == target_id { Some(pane) } else { None }
            }
            PaneLayout::Split(_, children) => {
                for child in children {
                    if let Some(p) = child.get_pane_mut(target_id) {
                        return Some(p);
                    }
                }
                None
            }
        }
    }

    pub fn get_pane(&self, target_id: usize) -> Option<&Pane> {
        match self {
            PaneLayout::Window(pane) => {
                if pane.id == target_id { Some(pane) } else { None }
            }
            PaneLayout::Split(_, children) => {
                for child in children {
                    if let Some(p) = child.get_pane(target_id) {
                        return Some(p);
                    }
                }
                None
            }
        }
    }

    pub fn update_buffer_indices(&mut self, removed_idx: usize) {
        match self {
            PaneLayout::Window(pane) => {
                if pane.buffer_idx > removed_idx {
                    pane.buffer_idx -= 1;
                } else if pane.buffer_idx == removed_idx {
                    pane.buffer_idx = pane.buffer_idx.saturating_sub(1);
                }
            }
            PaneLayout::Split(_, children) => {
                for child in children {
                    child.update_buffer_indices(removed_idx);
                }
            }
        }
    }
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
    pub pane_layout: PaneLayout,
    pub focused_pane_id: usize,
    pub next_pane_id: usize,
    
    
    pub preview_lines: Option<Vec<String>>, // floating file preview
    pub preview_scroll: usize,
    pub show_suggestions: bool,
    pub keymap_filter: String,
    pub command_suggestions: Vec<String>,
    pub selected_command_suggestion: usize,
    pub command_wildmenu_open: bool,
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
    pub git_diff_popup: Option<(String, usize)>, // (diff text, anchor buffer line)
    pub last_git_update: Option<Instant>,
    pub show_intro: bool,
    pub folding_ranges: Vec<lsp_types::FoldingRange>,
    pub definition_request_id: Option<i32>,
    pub hover_request_id: Option<i32>,
    pub hover_popup: Option<String>,
    pub diagnostic_popup: Option<String>,
    pub jumplist: Vec<(std::path::PathBuf, Position)>,
    pub jumplist_idx: usize,
    pub user_snippets: Vec<crate::config::UserSnippet>,
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
            pane_layout: PaneLayout::Window(Pane { id: 0, buffer_idx: 0 }),
            focused_pane_id: 0,
            next_pane_id: 1,
            
            
            preview_lines: None,
            preview_scroll: 0,
            show_suggestions: false,
            keymap_filter: String::new(),
            command_suggestions: Vec::new(),
            selected_command_suggestion: 0,
            command_wildmenu_open: false,
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
            git_diff_popup: None,
            last_git_update: None,
            show_intro: false,
            folding_ranges: Vec::new(),
            definition_request_id: None,
            hover_request_id: None,
            hover_popup: None,
            diagnostic_popup: None,
            jumplist: Vec::new(),
            jumplist_idx: 0,
            user_snippets: Vec::new(),
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
