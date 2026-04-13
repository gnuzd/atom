use crate::input::keymap::{Keymap, Action};
use crate::vim::mode::Mode;
use super::Plugin;

pub struct ExplorerPlugin;

impl Plugin for ExplorerPlugin {
    fn name(&self) -> &'static str { "explorer" }
    
    // Global bindings for explorer (e.g. toggle)
    fn register_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        if let Mode::Normal = mode {
            keymap.bind("\\", Action::ToggleExplorer);
        }
    }

    // Bindings when explorer is focused
    fn register_focused_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        if let Mode::Normal = mode {
            keymap.bind("j", Action::MoveDown);
            keymap.bind("k", Action::MoveUp);
            keymap.bind("Down", Action::MoveDown);
            keymap.bind("Up", Action::MoveUp);
            keymap.bind("h", Action::ExplorerCollapse);
            keymap.bind("l", Action::ExplorerExpand);
            keymap.bind("Left", Action::ExplorerCollapse);
            keymap.bind("Right", Action::ExplorerExpand);
            keymap.bind("CR", Action::ExplorerToggleExpand);
            keymap.bind("a", Action::ExplorerAdd);
            keymap.bind("r", Action::ExplorerRename);
            keymap.bind("d", Action::ExplorerDelete);
            keymap.bind("m", Action::ExplorerMove);
            keymap.bind("f", Action::ExplorerFilter);
            keymap.bind("o", Action::ExplorerOpenSystem);
            keymap.bind("H", Action::ExplorerToggleHidden);
            keymap.bind("I", Action::ExplorerToggleIgnored);
            keymap.bind("Z", Action::ExplorerCloseAll);
        }
    }
}
