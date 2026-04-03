pub mod mode;
pub mod motion;

use mode::{Focus, YankType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

pub struct VimState {
    pub mode: mode::Mode,
    pub focus: mode::Focus,
    pub command_buffer: String,
    pub input_buffer: String, // For Explorer operations
    pub selection_start: Option<Position>,
    pub search_query: String,
    pub register: String,
    pub yank_type: YankType,
    pub pending_op: Option<char>,
    pub yank_highlight_line: Option<usize>,
}

impl VimState {
    pub fn new() -> Self {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::mode::Mode;

    #[test]
    fn test_vim_state_new() {
        let state = VimState::new();
        assert_eq!(state.mode, Mode::Normal);
        assert!(state.command_buffer.is_empty());
        assert!(state.selection_start.is_none());
        assert!(state.search_query.is_empty());
        assert!(state.register.is_empty());
    }
}
