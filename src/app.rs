mod actions;
pub mod handlers;
mod runtime;

use anyhow::Result;
use crossterm::{
    cursor::SetCursorStyle,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::{
    env, io,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

use crate::editor::Editor;
use crate::input::keymap::{Action, Keymap};
use crate::lsp::LspManager;
use crate::plugins::PluginManager;
use crate::ui::explorer::FileExplorer;
use crate::ui::trouble::TroubleList;
use crate::ui::TerminalUi;
use crate::vim::{
    mode::{ExplorerInputType, Focus, Mode, YankType},
    LspStatus, Position, VimState,
};
use lsp_types::{
    CompletionResponse, CompletionTriggerKind, GotoDefinitionResponse, PublishDiagnosticsParams,
};

pub enum BackgroundFileOp {
    Format,
    Save,
}

pub enum AsyncResult {
    Format(Result<String, String>),
    Save(Result<String, String>),
}

/// Carries the result of an async file operation (format or save) back to the main loop.
pub struct AsyncFileResult {
    pub path: PathBuf,
    pub ext: String,
    pub result: AsyncResult,
    pub git_signs: Vec<(usize, crate::git::GitSign)>,
    pub op: BackgroundFileOp,
}

pub struct App {
    pub vim: VimState,
    pub editor: Editor,
    pub ui: TerminalUi,
    pub explorer: FileExplorer,
    pub trouble: TroubleList,
    pub lsp_manager: LspManager,
    pub terminal: Terminal<CrosstermBackend<io::Stdout>>,
    pub rx: mpsc::Receiver<notify::Result<notify::Event>>,
    pub async_tx: mpsc::Sender<AsyncFileResult>,
    pub async_rx: mpsc::Receiver<AsyncFileResult>,
    pub watcher: RecommendedWatcher,
    pub keymap_normal: Keymap,
    pub keymap_insert: Keymap,
    pub keymap_explorer: Keymap,
    pub plugin_manager: PluginManager,
    pub last_click: Option<(Instant, u16, u16)>,
    pub last_lsp_update: Option<Instant>,
    pub should_quit: bool,
    pub is_dragging: bool,
    pub drag_anchor: Option<Position>,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = crate::config::Config::load();
        let project_root = find_project_root(&env::current_dir().unwrap_or_default());
        let vim = VimState::new(config, project_root);

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if vim.config.mouse {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        } else {
            execute!(stdout, EnterAlternateScreen)?;
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let editor = Editor::new(&vim.config.colorscheme);
        let ui = TerminalUi::new();
        let explorer = FileExplorer::new();
        let trouble = TroubleList::new();
        let lsp_manager = LspManager::new();

        let (tx, rx) = mpsc::channel();
        let (async_tx, async_rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        watcher.watch(&vim.project_root, RecursiveMode::Recursive)?;

        let mut keymap_normal = Keymap::default_normal();
        let mut keymap_insert = Keymap::default_insert();
        let mut keymap_explorer = Keymap::new();

        let plugin_manager = PluginManager::new();
        plugin_manager.register_all_keymaps(&mut keymap_normal, Mode::Normal);
        plugin_manager.register_all_keymaps(&mut keymap_insert, Mode::Insert);

        // Populate keymap_explorer from plugins that have explorer bindings
        plugin_manager.register_focused_keymaps("explorer", &mut keymap_explorer, Mode::Normal);

        // Explicitly bind generic ones if not handled by plugins
        keymap_explorer.bind("Esc", Action::ExitMode);
        keymap_explorer.bind("\\", Action::ToggleExplorer);
        keymap_explorer.bind(":", Action::EnterCommand);

        // Navigation
        keymap_explorer.bind("j", Action::MoveDown);
        keymap_explorer.bind("k", Action::MoveUp);
        keymap_explorer.bind("Down", Action::MoveDown);
        keymap_explorer.bind("Up", Action::MoveUp);
        keymap_explorer.bind("PageUp", Action::MovePageUp);
        keymap_explorer.bind("PageDown", Action::MovePageDown);

        // Open / expand / collapse
        keymap_explorer.bind("l", Action::ExplorerExpand);
        keymap_explorer.bind("Right", Action::ExplorerExpand);
        keymap_explorer.bind("CR", Action::Confirm);
        keymap_explorer.bind("h", Action::ExplorerCollapse);
        keymap_explorer.bind("Left", Action::ExplorerCollapse);

        Ok(Self {
            vim,
            editor,
            ui,
            explorer,
            trouble,
            lsp_manager,
            terminal,
            rx,
            async_tx,
            async_rx,
            watcher,
            keymap_normal,
            keymap_insert,
            keymap_explorer,
            plugin_manager,
            last_click: None,
            last_lsp_update: None,
            should_quit: false,
            is_dragging: false,
            drag_anchor: None,
        })
    }
}

pub fn find_project_root(path: &PathBuf) -> PathBuf {
    let mut current = path.clone();
    if current.is_file() {
        current.pop();
    }
    while current.parent().is_some() {
        if current.join("Cargo.toml").exists()
            || current.join(".git").exists()
            || current.join("package.json").exists()
            || current.join("tsconfig.json").exists()
            || current.join("jsconfig.json").exists()
        {
            return current;
        }
        current.pop();
    }
    path.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| env::current_dir().unwrap_or_default())
}

pub fn update_git_info(project_root: &PathBuf) -> Option<crate::vim::GitInfo> {
    use std::process::Command;

    let branch = Command::new("git")
        .args(&["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_root)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })?;

    let status = Command::new("git")
        .args(&["status", "--porcelain"])
        .current_dir(project_root)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let mut info = crate::vim::GitInfo {
        branch,
        added: 0,
        modified: 0,
        removed: 0,
    };

    for line in status.lines() {
        if line.starts_with('A') || line.starts_with("??") {
            info.added += 1;
        } else if line.starts_with('M') || line.starts_with(" M") {
            info.modified += 1;
        } else if line.starts_with('D') || line.starts_with(" D") {
            info.removed += 1;
        }
    }

    Some(info)
}
