use crate::input::keymap::{Keymap, Action};
use crate::vim::mode::Mode;
use super::Plugin;

pub struct LspPlugin;

impl Plugin for LspPlugin {
    fn name(&self) -> &'static str { "lsp" }
    fn register_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        if let Mode::Normal = mode {
            keymap.bind("gd", Action::LspDefinition);
            keymap.bind("<Space>f", Action::Format);
        }
    }
}
