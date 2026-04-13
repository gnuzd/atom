use crate::input::keymap::{Keymap, Action};
use crate::vim::mode::Mode;
use super::Plugin;

pub struct TelescopePlugin;

impl Plugin for TelescopePlugin {
    fn name(&self) -> &'static str { "telescope" }
    fn register_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        if let Mode::Normal = mode {
            keymap.bind("<Space>ff", Action::TelescopeFiles);
            keymap.bind("<Space>fg", Action::TelescopeLiveGrep);
            keymap.bind("<Space>fb", Action::TelescopeBuffers);
            keymap.bind("<Space>th", Action::TelescopeThemes);
        }
    }
}
