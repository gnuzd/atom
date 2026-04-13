use crate::input::keymap::{Keymap, Action};
use crate::vim::mode::Mode;
use super::Plugin;

pub struct TroublePlugin;

impl Plugin for TroublePlugin {
    fn name(&self) -> &'static str { "trouble" }
    fn register_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        if let Mode::Normal = mode {
            keymap.bind("<Space>tt", Action::ToggleTrouble);
        }
    }
}
