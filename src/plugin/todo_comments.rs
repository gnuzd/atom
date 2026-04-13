use crate::input::keymap::Keymap;
use crate::vim::mode::Mode;
use super::Plugin;

pub struct TodoCommentsPlugin;

impl Plugin for TodoCommentsPlugin {
    fn name(&self) -> &'static str { "todo-comments" }
    fn register_keymaps(&self, _keymap: &mut Keymap, _mode: Mode) {
        // Todo comments specific bindings
    }
}
