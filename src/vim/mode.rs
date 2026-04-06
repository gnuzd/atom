#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
    Search,
    ExplorerInput(ExplorerInputType),
    Mason,
    MasonFilter,
    Keymaps,
    Telescope(TelescopeKind),
    Confirm(ConfirmAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelescopeKind {
    Files,
    Words,
    Buffers,
    Themes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Quit,
    CloseBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorerInputType {
    Add,
    Rename,
    Move,
    Filter,
    DeleteConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YankType {
    Char,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Editor,
    Explorer,
    Trouble,
}
