pub mod mode;
pub mod motion;

pub struct VimState {
    pub mode: mode::Mode,
}

impl VimState {
    pub fn new() -> Self {
        Self {
            mode: mode::Mode::Normal,
        }
    }
}
