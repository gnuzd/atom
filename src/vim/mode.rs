#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualBlock,
    BlockInsert,
    Command,
    Search,
    ExplorerInput(ExplorerInputType),
    Nucleus,
    NucleusFilter,
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
    ReloadFile,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitKind {
    Horizontal, // primary top, split below
    Vertical,   // primary left, split right
}
