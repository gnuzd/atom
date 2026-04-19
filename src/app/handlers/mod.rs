mod command;
mod confirm;
mod explorer;
mod insert;
mod nucleus;
mod normal;
mod search;
mod telescope;
mod visual;

use super::*;

impl App {
    /// Dispatches a raw terminal key event to the handler for the current mode.
    pub fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        self.vim.show_intro = false;
        self.vim.yank_highlight_line = None;

        if self.vim.blame_popup.is_some() {
            self.vim.blame_popup = None;
            return;
        }

        match self.vim.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Visual => self.handle_visual_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Search => self.handle_search_mode(key),
            Mode::ExplorerInput(input_type) => self.handle_explorer_input_mode(key, input_type),
            Mode::Confirm(action) => self.handle_confirm_mode(key, action),
            Mode::Telescope(_) => self.handle_telescope_mode(key),
            Mode::Nucleus => self.handle_nucleus_mode(key),
            Mode::NucleusFilter => self.handle_nucleus_filter_mode(key),
            Mode::Keymaps => self.handle_keymaps_mode(key),
            Mode::Command => self.handle_command_mode(key),
        }
    }
}
