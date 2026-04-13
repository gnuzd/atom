use crate::input::keymap::{Keymap, Action};
use crate::vim::mode::Mode;
use super::Plugin;

pub struct ExplorerPlugin;

impl Plugin for ExplorerPlugin {
    fn name(&self) -> &'static str { "explorer" }
    fn register_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        if let Mode::Normal = mode {
            keymap.bind("j", Action::MoveDown);
            keymap.bind("k", Action::MoveUp);
            keymap.bind("Down", Action::MoveDown);
            keymap.bind("Up", Action::MoveUp);
            keymap.bind("h", Action::ExplorerCollapse);
            keymap.bind("l", Action::ExplorerExpand);
            keymap.bind("Enter", Action::ExplorerToggleExpand);
            keymap.bind("a", Action::ExplorerAdd);
            keymap.bind("r", Action::ExplorerRename);
            keymap.bind("d", Action::ExplorerDelete);
            keymap.bind("m", Action::ExplorerMove);
            keymap.bind("f", Action::ExplorerFilter);
            keymap.bind("o", Action::ExplorerOpenSystem);
            keymap.bind("H", Action::ExplorerToggleHidden);
        }
    }
}
