#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
    Search,
    ExplorerInput(ExplorerInputType),
    Mason,
    Keymaps,
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
