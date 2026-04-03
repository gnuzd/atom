pub mod mode;
pub mod motion;

pub struct VimState {
    pub mode: mode::Mode,
    pub command_buffer: String,
}

impl VimState {
    pub fn new() -> Self {
        Self {
            mode: mode::Mode::Normal,
            command_buffer: String::new(),
        }
    }
}
