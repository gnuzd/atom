use crate::input::keymap::Keymap;
use crate::vim::mode::Mode;
use super::Plugin;

pub struct TreesitterPlugin;

impl Plugin for TreesitterPlugin {
    fn name(&self) -> &'static str { "treesitter" }
    fn register_keymaps(&self, _keymap: &mut Keymap, _mode: Mode) {
        // Treesitter specific bindings
    }
}
