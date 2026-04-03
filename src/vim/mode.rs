#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YankType {
    Char,
    Line,
}
