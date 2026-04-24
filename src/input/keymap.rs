use crate::input::event::key_to_string;
use crossterm::event::KeyEvent;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    // Mode transitions
    EnterInsert,
    EnterInsertLineStart,
    EnterVisual,
    EnterVisualBlock,
    EnterCommand,
    EnterSearch,
    ExitMode,
    EnterNucleus,
    EnterTrouble,
    EnterKeymaps,

    // File/Buffer ops
    Save,
    SaveAs,
    Quit,
    QuitAll,
    SaveAndQuit,
    QuitWithoutSaving,
    CloseBuffer,
    NextBuffer,
    PrevBuffer,
    ReloadFile,

    // Motion
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordForward,
    MoveWordBackward,
    MoveWordEnd,
    MoveLineStart,
    MoveLineEnd,
    MovePageUp,
    MovePageDown,
    JumpToFirstLine,
    JumpToLastLine,

    // Editing
    DeleteChar,
    DeleteCharBefore,
    Substitute,
    DeleteLine,
    YankLine,
    CopyToClipboard,
    PasteAfter,
    PasteBefore,
    PasteFromClipboard,
    Undo,
    Redo,
    ToggleComment,
    OpenLineBelow,
    OpenLineAbove,
    DeleteSelection,
    Indent,
    Outdent,

    // Plugins / Specialized
    TelescopeFiles,
    TelescopeLiveGrep,
    TelescopeBuffers,
    TelescopeThemes,
    LspDefinition,
    LspHover,
    DiagnosticFloat,
    ToggleExplorer,
    ToggleRelativeNumber,
    ToggleTrouble,
    ToggleAutoformat,
    GitBlame,
    GitDiffHunk,
    ToggleFold,
    NextHunk,
    PrevHunk,
    Format,

    // Explorer specific
    ExplorerExpand,
    ExplorerCollapse,
    ExplorerToggleExpand,
    ExplorerAdd,
    ExplorerRename,
    ExplorerDelete,
    ExplorerMove,
    ExplorerFilter,
    ExplorerOpenSystem,
    ExplorerToggleHidden,
    ExplorerToggleIgnored,
    ExplorerCloseAll,

    // Generic
    SelectNext,
    SelectPrev,
    Confirm,

    // Raw key passthrough
    Unbound,

    // User-defined ex command (e.g. ":split", ":w")
    Custom(String),
}

impl Action {
    /// Parse an action name string (from init.lua rhs) into an Action.
    pub fn from_str(s: &str) -> Self {
        if s.starts_with(':') {
            return Action::Custom(s[1..].to_string());
        }
        match s {
            "EnterInsert"           => Action::EnterInsert,
            "EnterInsertLineStart"  => Action::EnterInsertLineStart,
            "EnterVisual"           => Action::EnterVisual,
            "EnterVisualBlock"      => Action::EnterVisualBlock,
            "EnterCommand"          => Action::EnterCommand,
            "EnterSearch"           => Action::EnterSearch,
            "ExitMode"              => Action::ExitMode,
            "EnterNucleus"          => Action::EnterNucleus,
            "EnterTrouble"          => Action::EnterTrouble,
            "EnterKeymaps"          => Action::EnterKeymaps,
            "Save"                  => Action::Save,
            "SaveAs"                => Action::SaveAs,
            "Quit"                  => Action::Quit,
            "QuitAll"               => Action::QuitAll,
            "SaveAndQuit"           => Action::SaveAndQuit,
            "QuitWithoutSaving"     => Action::QuitWithoutSaving,
            "CloseBuffer"           => Action::CloseBuffer,
            "NextBuffer"            => Action::NextBuffer,
            "PrevBuffer"            => Action::PrevBuffer,
            "ReloadFile"            => Action::ReloadFile,
            "MoveLeft"              => Action::MoveLeft,
            "MoveRight"             => Action::MoveRight,
            "MoveUp"                => Action::MoveUp,
            "MoveDown"              => Action::MoveDown,
            "MoveWordForward"       => Action::MoveWordForward,
            "MoveWordBackward"      => Action::MoveWordBackward,
            "MoveWordEnd"           => Action::MoveWordEnd,
            "MoveLineStart"         => Action::MoveLineStart,
            "MoveLineEnd"           => Action::MoveLineEnd,
            "MovePageUp"            => Action::MovePageUp,
            "MovePageDown"          => Action::MovePageDown,
            "JumpToFirstLine"       => Action::JumpToFirstLine,
            "JumpToLastLine"        => Action::JumpToLastLine,
            "DeleteChar"            => Action::DeleteChar,
            "DeleteCharBefore"      => Action::DeleteCharBefore,
            "Substitute"            => Action::Substitute,
            "DeleteLine"            => Action::DeleteLine,
            "YankLine"              => Action::YankLine,
            "CopyToClipboard"       => Action::CopyToClipboard,
            "PasteAfter"            => Action::PasteAfter,
            "PasteBefore"           => Action::PasteBefore,
            "PasteFromClipboard"    => Action::PasteFromClipboard,
            "Undo"                  => Action::Undo,
            "Redo"                  => Action::Redo,
            "ToggleComment"         => Action::ToggleComment,
            "OpenLineBelow"         => Action::OpenLineBelow,
            "OpenLineAbove"         => Action::OpenLineAbove,
            "DeleteSelection"       => Action::DeleteSelection,
            "Indent"                => Action::Indent,
            "Outdent"               => Action::Outdent,
            "TelescopeFiles"        => Action::TelescopeFiles,
            "TelescopeLiveGrep"     => Action::TelescopeLiveGrep,
            "TelescopeBuffers"      => Action::TelescopeBuffers,
            "TelescopeThemes"       => Action::TelescopeThemes,
            "LspDefinition"         => Action::LspDefinition,
            "LspHover"              => Action::LspHover,
            "DiagnosticFloat"       => Action::DiagnosticFloat,
            "ToggleExplorer"        => Action::ToggleExplorer,
            "ToggleRelativeNumber"  => Action::ToggleRelativeNumber,
            "ToggleTrouble"         => Action::ToggleTrouble,
            "ToggleAutoformat"      => Action::ToggleAutoformat,
            "GitBlame"              => Action::GitBlame,
            "GitDiffHunk"           => Action::GitDiffHunk,
            "ToggleFold"            => Action::ToggleFold,
            "NextHunk"              => Action::NextHunk,
            "PrevHunk"              => Action::PrevHunk,
            "Format"                => Action::Format,
            "ExplorerExpand"        => Action::ExplorerExpand,
            "ExplorerCollapse"      => Action::ExplorerCollapse,
            "ExplorerToggleExpand"  => Action::ExplorerToggleExpand,
            "ExplorerAdd"           => Action::ExplorerAdd,
            "ExplorerRename"        => Action::ExplorerRename,
            "ExplorerDelete"        => Action::ExplorerDelete,
            "ExplorerMove"          => Action::ExplorerMove,
            "ExplorerFilter"        => Action::ExplorerFilter,
            "ExplorerOpenSystem"    => Action::ExplorerOpenSystem,
            "ExplorerToggleHidden"  => Action::ExplorerToggleHidden,
            "ExplorerToggleIgnored" => Action::ExplorerToggleIgnored,
            "ExplorerCloseAll"      => Action::ExplorerCloseAll,
            "SelectNext"            => Action::SelectNext,
            "SelectPrev"            => Action::SelectPrev,
            "Confirm"               => Action::Confirm,
            other                   => Action::Custom(other.to_string()),
        }
    }
}

/// Normalize a Vim-notation key string to our internal format.
/// e.g. "<CR>" → "CR", "<C-s>" → "<C-s>", "<leader>" → "\\"
pub fn normalize_key(key: &str, leader: &str) -> String {
    // Replace <leader> with the actual leader key
    let key = key.replace("<leader>", leader);
    // Strip outer <> from special keys that are already bare in our format
    let k = key.trim();
    if k.starts_with('<') && k.ends_with('>') {
        let inner = &k[1..k.len()-1];
        // Modifier combos like C-s, S-Tab stay wrapped
        if inner.contains('-') {
            return format!("<{}>", inner);
        }
        // Bare specials: CR, BS, Esc, Tab, Space, etc.
        return inner.to_string();
    }
    k.to_string()
}

#[derive(Debug, Default, Clone)]
pub struct Keymap {
    pub bindings: HashMap<String, Action>,
}

impl Keymap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&mut self, key: impl Into<String>, action: Action) {
        self.bindings.insert(key.into(), action);
    }

    pub fn resolve(&self, key: &KeyEvent) -> &Action {
        let s = key_to_string(key);
        self.bindings.get(&s).unwrap_or(&Action::Unbound)
    }

    pub fn default_normal() -> Self {
        let mut km = Self::new();

        // Motion
        km.bind("h", Action::MoveLeft);
        km.bind("j", Action::MoveDown);
        km.bind("k", Action::MoveUp);
        km.bind("l", Action::MoveRight);
        km.bind("Up", Action::MoveUp);
        km.bind("Down", Action::MoveDown);
        km.bind("Left", Action::MoveLeft);
        km.bind("Right", Action::MoveRight);
        km.bind("w", Action::MoveWordForward);
        km.bind("b", Action::MoveWordBackward);
        km.bind("e", Action::MoveWordEnd);
        km.bind("G", Action::JumpToLastLine);
        km.bind("Home", Action::MoveLineStart);
        km.bind("End", Action::MoveLineEnd);
        km.bind("PageUp", Action::MoveLineStart);
        km.bind("PageDown", Action::MoveLineEnd);
        km.bind("Left", Action::MoveLeft);
        km.bind("Down", Action::MoveDown);
        km.bind("Up", Action::MoveUp);
        km.bind("Right", Action::MoveRight);

        // Editing
        km.bind("i", Action::EnterInsert);
        km.bind("I", Action::EnterInsertLineStart);
        km.bind("v", Action::EnterVisual);
        km.bind("<C-v>", Action::EnterVisualBlock);
        km.bind(":", Action::EnterCommand);
        km.bind("/", Action::EnterSearch);
        km.bind("u", Action::Undo);
        km.bind("<C-r>", Action::Redo);
        km.bind("x", Action::DeleteChar);
        km.bind("o", Action::OpenLineBelow);
        km.bind("O", Action::OpenLineAbove);
        km.bind("p", Action::PasteAfter);
        km.bind("P", Action::PasteBefore);
        km.bind("<C-c>", Action::CopyToClipboard);
        km.bind("s", Action::Substitute);

        // Sequences (handled via input_buffer currently, but we can pre-bind first char)
        // These will be resolved if we move sequence handling into Keymap later.
        // For now, App::run still does sequence detection.

        km.bind("Tab", Action::NextBuffer);
        km.bind("S-Tab", Action::PrevBuffer);
        km.bind("CR", Action::Confirm);

        // Global shortcuts
        km.bind("<C-s>", Action::Save);
        km.bind("\\", Action::ToggleExplorer);
        km.bind("?", Action::EnterKeymaps);

        km
    }

    pub fn default_insert() -> Self {
        let mut km = Self::new();
        km.bind("Esc", Action::ExitMode);
        km.bind("<C-s>", Action::Save);
        km.bind("CR", Action::Confirm);
        km.bind("Tab", Action::SelectNext);
        km.bind("<S-Tab>", Action::SelectPrev);
        km.bind("BS", Action::DeleteCharBefore);
        km.bind("<C-v>", Action::PasteFromClipboard);
        km.bind("PageUp", Action::MoveLineStart);
        km.bind("PageDown", Action::MoveLineEnd);
        km
    }
}
