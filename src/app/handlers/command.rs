use super::*;
use crossterm::event::{KeyCode, KeyEvent};

/// All recognised Ex commands. Extend this slice to add new commands;
/// the tab-completion and dispatch logic picks them up automatically.
pub const COMMANDS: &[&str] = &[
    "q",
    "quit",
    "qa",
    "qall",
    "w",
    "write",
    "wa",
    "wall",
    "wq",
    "x",
    "wqa",
    "xa",
    "bn",
    "bnext",
    "bp",
    "bprev",
    "bd",
    "bdelete",
    "e",
    "edit",
    "e!",
    "Reload",
    "colorscheme",
    "Nucleus",
    "TreesitterManager",
    "TressitterManager",
    "Trouble",
    "format",
    "Format",
    "FormatAll",
    "FormatEnable",
    "FormatDisable",
    "gd",
    "LspInfo",
    "LspRestart",
    "set",
    "config",
    "help",
    "checkhealth",
];

impl App {
    pub fn handle_command_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.vim.mode = Mode::Normal;
                self.vim.command_suggestions.clear();
            }
            KeyCode::Char(c) => {
                self.vim.command_buffer.push(c);
                self.refresh_command_suggestions();
            }
            KeyCode::Backspace => {
                self.vim.command_buffer.pop();
                if self.vim.command_buffer.is_empty() {
                    self.vim.command_suggestions.clear();
                } else {
                    self.refresh_command_suggestions();
                }
                self.vim.selected_command_suggestion = 0;
            }
            KeyCode::Tab => {
                if !self.vim.command_suggestions.is_empty() {
                    self.vim.selected_command_suggestion =
                        (self.vim.selected_command_suggestion + 1)
                            % self.vim.command_suggestions.len();
                }
            }
            KeyCode::Enter => self.execute_command(),
            _ => {}
        }
    }

    fn refresh_command_suggestions(&mut self) {
        self.vim.command_suggestions = COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(&self.vim.command_buffer))
            .map(|s| s.to_string())
            .collect();
        self.vim.selected_command_suggestion = 0;
    }

    fn execute_command(&mut self) {
        let cmd_str = if !self.vim.command_suggestions.is_empty() {
            self.vim.command_suggestions[self.vim.selected_command_suggestion].clone()
        } else {
            self.vim.command_buffer.trim().to_string()
        };
        self.vim.command_buffer.clear();
        self.vim.command_suggestions.clear();
        self.vim.mode = Mode::Normal;

        if cmd_str.is_empty() {
            return;
        }

        let mut parts = cmd_str.split_whitespace();
        let first_part = parts.next().unwrap_or("");
        let force = first_part.ends_with('!');
        let cmd = if force {
            &first_part[..first_part.len() - 1]
        } else {
            first_part
        };
        let args: Vec<&str> = parts.collect();

        // Jump to line number if the command is a bare integer.
        if let Ok(line) = cmd.parse::<usize>() {
            self.editor.cursor_mut().y = line.saturating_sub(1);
            self.editor.clamp_cursor();
            return;
        }

        self.dispatch_ex_command(cmd, force, &args);
    }

    fn dispatch_ex_command(&mut self, cmd: &str, force: bool, args: &[&str]) {
        match cmd {
            "q" | "quit" => self.dispatch_action(
                if force { Action::QuitAll } else { Action::Quit },
                1,
            ),
            "qa" | "qall" => self.dispatch_action(Action::QuitAll, 1),
            "w" | "write" => {
                let path = args.first().map(|s| PathBuf::from(*s));
                self.save_and_format(path);
            }
            "wa" | "wall" => {
                let current = self.editor.active_idx;
                for i in 0..self.editor.buffers.len() {
                    self.editor.active_idx = i;
                    self.save_and_format(None);
                }
                self.editor.active_idx = current;
            }
            "wq" | "x" => {
                self.save_and_format(None);
                self.dispatch_action(Action::Quit, 1);
            }
            "wqa" | "xa" => {
                let current = self.editor.active_idx;
                for i in 0..self.editor.buffers.len() {
                    self.editor.active_idx = i;
                    self.save_and_format(None);
                }
                self.editor.active_idx = current;
                self.should_quit = true;
            }
            "bn" | "bnext" => self.dispatch_action(Action::NextBuffer, 1),
            "bp" | "bprev" => self.dispatch_action(Action::PrevBuffer, 1),
            "bd" | "bdelete" => self.dispatch_action(Action::CloseBuffer, 1),
            "e" | "edit" => {
                if let Some(p) = args.first() {
                    let _ = self.editor.open_file(PathBuf::from(*p));
                    self.sync_explorer();
                }
            }
            "e!" | "Reload" => self.dispatch_action(Action::ReloadFile, 1),
            "colorscheme" => {
                if let Some(theme) = args.first() {
                    self.editor.set_theme(theme);
                } else {
                    self.dispatch_action(Action::TelescopeThemes, 1);
                }
            }
            "Nucleus" => self.dispatch_action(Action::EnterNucleus, 1),
            "TreesitterManager" | "TressitterManager" => self.enter_treesitter_manager(),
            "Trouble" => self.dispatch_action(Action::ToggleTrouble, 1),
            "format" | "Format" => self.dispatch_action(Action::Format, 1),
            "FormatAll" => {
                let current = self.editor.active_idx;
                for i in 0..self.editor.buffers.len() {
                    self.editor.active_idx = i;
                    self.format_buffer(BackgroundFileOp::Format);
                }
                self.editor.active_idx = current;
            }
            "FormatEnable" => self.vim.config.disable_autoformat = false,
            "FormatDisable" => self.vim.config.disable_autoformat = true,
            "gd" | "Definition" => self.dispatch_action(Action::LspDefinition, 1),
            "set" => {
                if let Some(arg) = args.first() {
                    match *arg {
                        "number" => self.vim.config.number = true,
                        "nonumber" => self.vim.config.number = false,
                        "relativenumber" => self.vim.config.relativenumber = true,
                        "norelativenumber" => self.vim.config.relativenumber = false,
                        _ => {}
                    }
                }
            }
            "config" => {
                let _ = self.vim.config.save();
            }
            "help" => self.dispatch_action(Action::EnterKeymaps, 1),
            _ => {}
        }
    }
}
