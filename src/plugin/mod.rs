pub mod lsp;
pub mod telescope;
pub mod trouble;
pub mod treesitter;
pub mod todo_comments;
pub mod explorer;

use crate::input::keymap::Keymap;
use crate::vim::mode::Mode;

pub trait Plugin {
    fn name(&self) -> &'static str;
    fn register_keymaps(&self, keymap: &mut Keymap, mode: Mode);
}

pub struct PluginManager {
    pub plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: vec![
                Box::new(lsp::LspPlugin),
                Box::new(telescope::TelescopePlugin),
                Box::new(trouble::TroublePlugin),
                Box::new(treesitter::TreesitterPlugin),
                Box::new(todo_comments::TodoCommentsPlugin),
                Box::new(explorer::ExplorerPlugin),
            ],
        }
    }

    pub fn register_all_keymaps(&self, keymap: &mut Keymap, mode: Mode) {
        for plugin in &self.plugins {
            plugin.register_keymaps(keymap, mode);
        }
    }
}
