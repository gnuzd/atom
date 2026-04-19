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
    ToggleExplorer,
    ToggleRelativeNumber,
    ToggleTrouble,
    ToggleAutoformat,
    GitBlame,
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
        km.bind("PageDown", Action::MovePageDown);
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
        km.bind("s", Action::DeleteSelection); // 's' in normal mode usually deletes char and enters insert

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
        km.bind("BS", Action::DeleteCharBefore); // Usually handled specially
        km.bind("<C-v>", Action::PasteFromClipboard);
        km
    }
}
