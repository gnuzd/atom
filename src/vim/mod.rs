pub mod mode;
pub mod motion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

pub struct VimState {
    pub mode: mode::Mode,
    pub command_buffer: String,
    pub selection_start: Option<Position>,
    pub search_query: String,
}

impl VimState {
    pub fn new() -> Self {
        Self {
            mode: mode::Mode::Normal,
            command_buffer: String::new(),
            selection_start: None,
            search_query: String::new(),
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
    }
}
