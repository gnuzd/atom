use crate::input::keymap::{Keymap, Action};
use crate::vim::mode::Mode;

pub trait Plugin {
    fn name(&self) -> &'static str;
    fn register_keymaps(&self, keymap: &mut Keymap, mode: Mode);
}

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

pub struct TroublePlugin;
impl Plugin for TroublePlugin {
    fn name(&self) -> &'static str { "trouble" }
    fn register_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        if let Mode::Normal = mode {
            keymap.bind("<Space>tt", Action::ToggleTrouble);
        }
    }
}

pub struct TreesitterPlugin;
impl Plugin for TreesitterPlugin {
    fn name(&self) -> &'static str { "treesitter" }
    fn register_keymaps(&self, _keymap: &mut Keymap, _mode: Mode) {
        // Treesitter specific bindings if any
    }
}

pub struct TodoCommentsPlugin;
impl Plugin for TodoCommentsPlugin {
    fn name(&self) -> &'static str { "todo-comments" }
    fn register_keymaps(&self, _keymap: &mut Keymap, _mode: Mode) {
        // Todo comments specific bindings if any
    }
}

pub struct PluginManager {
    pub plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: vec![
                Box::new(LspPlugin),
                Box::new(TelescopePlugin),
                Box::new(TroublePlugin),
                Box::new(TreesitterPlugin),
                Box::new(TodoCommentsPlugin),
            ],
        }
    }

    pub fn register_all_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        for plugin in &self.plugins {
            plugin.register_keymaps(keymap, mode);
        }
    }
}
