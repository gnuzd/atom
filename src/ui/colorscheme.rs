use ratatui::style::{Color, Modifier, Style};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Palette {
    pub white: Color,
    pub darker_black: Color,
    pub black: Color,
    pub black2: Color,
    pub grey: Color,
    pub grey_fg: Color,
    pub red: Color,
    pub green: Color,
    pub yellow: Color,
    pub blue: Color,
    pub purple: Color,
    pub cyan: Color,
    pub orange: Color,
}

impl Palette {
    pub fn catppuccin() -> Self {
        Self {
            white: Color::Rgb(217, 224, 238),
            darker_black: Color::Rgb(22, 22, 34),
            black: Color::Rgb(30, 30, 46),
            black2: Color::Rgb(24, 24, 37),
            grey: Color::Rgb(49, 50, 68),
            grey_fg: Color::Rgb(88, 91, 112),
            red: Color::Rgb(243, 139, 168),
            green: Color::Rgb(166, 227, 161),
            yellow: Color::Rgb(249, 226, 175),
            blue: Color::Rgb(137, 180, 250),
            purple: Color::Rgb(203, 166, 247),
            cyan: Color::Rgb(137, 220, 235),
            orange: Color::Rgb(250, 179, 135),
        }
    }

    pub fn gruvbox_material() -> Self {
        Self {
            white: Color::Rgb(212, 190, 152),
            darker_black: Color::Rgb(29, 32, 33),
            black: Color::Rgb(40, 40, 40),
            black2: Color::Rgb(50, 48, 47),
            grey: Color::Rgb(124, 111, 100),
            grey_fg: Color::Rgb(146, 131, 116),
            red: Color::Rgb(234, 105, 98),
            green: Color::Rgb(169, 182, 101),
            yellow: Color::Rgb(216, 166, 94),
            blue: Color::Rgb(125, 174, 163),
            purple: Color::Rgb(211, 134, 155),
            cyan: Color::Rgb(137, 180, 130),
            orange: Color::Rgb(231, 138, 78),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColorScheme {
    pub palette: Palette,
    pub highlights: HashMap<String, Style>,
}

impl ColorScheme {
    pub fn new(name: &str) -> Self {
        let palette = match name {
            "gruvbox-material" => Palette::gruvbox_material(),
            _ => Palette::catppuccin(),
        };

        let mut hl = HashMap::new();

        // Base UI
        hl.insert("Normal".into(), Style::default().fg(palette.white).bg(palette.black));
        hl.insert("CursorLine".into(), Style::default().bg(palette.black2));
        hl.insert("LineNr".into(), Style::default().fg(palette.grey));
        hl.insert("CursorLineNr".into(), Style::default().fg(palette.white).add_modifier(Modifier::BOLD));
        hl.insert("Visual".into(), Style::default().bg(palette.grey_fg));
        hl.insert("Search".into(), Style::default().fg(palette.black).bg(palette.yellow));
        
        // Syntax
        hl.insert("Keyword".into(), Style::default().fg(palette.purple).add_modifier(Modifier::BOLD));
        hl.insert("Function".into(), Style::default().fg(palette.blue));
        hl.insert("String".into(), Style::default().fg(palette.green));
        hl.insert("Comment".into(), Style::default().fg(palette.grey_fg).add_modifier(Modifier::ITALIC));
        hl.insert("Constant".into(), Style::default().fg(palette.orange));
        hl.insert("Type".into(), Style::default().fg(palette.yellow));
        hl.insert("Variable".into(), Style::default().fg(palette.white));
        hl.insert("Identifier".into(), Style::default().fg(palette.red));
        hl.insert("Tag".into(), Style::default().fg(palette.orange));
        hl.insert("Attribute".into(), Style::default().fg(palette.cyan));
        hl.insert("Property".into(), Style::default().fg(palette.blue));

        // Statusline
        hl.insert("StatusLine".into(), Style::default().fg(palette.white).bg(palette.black2));
        hl.insert("StatusLineNormal".into(), Style::default().fg(palette.black).bg(palette.blue).add_modifier(Modifier::BOLD));
        hl.insert("StatusLineInsert".into(), Style::default().fg(palette.black).bg(palette.green).add_modifier(Modifier::BOLD));
        hl.insert("StatusLineVisual".into(), Style::default().fg(palette.black).bg(palette.purple).add_modifier(Modifier::BOLD));
        hl.insert("StatusLineCommand".into(), Style::default().fg(palette.black).bg(palette.yellow).add_modifier(Modifier::BOLD));
        
        hl.insert("StatusLineA".into(), Style::default().fg(palette.black).bg(palette.blue).add_modifier(Modifier::BOLD));
        hl.insert("StatusLineB".into(), Style::default().fg(palette.white).bg(palette.black2));
        hl.insert("StatusLineC".into(), Style::default().fg(palette.white).bg(palette.black2));
        
        hl.insert("StatusLineX".into(), Style::default().fg(palette.white).bg(palette.black2));
        hl.insert("StatusLineY".into(), Style::default().fg(palette.white).bg(palette.black2));
        hl.insert("StatusLineZ".into(), Style::default().fg(palette.blue).bg(palette.black2).add_modifier(Modifier::BOLD));

        hl.insert("StatusLineGitAdd".into(), Style::default().fg(palette.green).bg(palette.black2));
        hl.insert("StatusLineGitMod".into(), Style::default().fg(palette.blue).bg(palette.black2));
        hl.insert("StatusLineGitDel".into(), Style::default().fg(palette.red).bg(palette.black2));

        hl.insert("GitSignsAdd".into(), Style::default().fg(palette.green));
        hl.insert("GitSignsChange".into(), Style::default().fg(palette.yellow));
        hl.insert("GitSignsDelete".into(), Style::default().fg(palette.red));

        hl.insert("StatusLineDiagnosticError".into(), Style::default().fg(palette.red).bg(palette.black2));
        hl.insert("StatusLineDiagnosticWarn".into(), Style::default().fg(palette.yellow).bg(palette.black2));
        hl.insert("StatusLineDiagnosticInfo".into(), Style::default().fg(palette.blue).bg(palette.black2));
        hl.insert("StatusLineDiagnosticHint".into(), Style::default().fg(palette.purple).bg(palette.black2));

        // Explorer
        hl.insert("TreeExplorerRoot".into(), Style::default().fg(palette.green).add_modifier(Modifier::BOLD));
        hl.insert("TreeExplorerConnector".into(), Style::default().fg(palette.grey));
        hl.insert("TreeExplorerFolderIcon".into(), Style::default().fg(palette.yellow));
        hl.insert("TreeExplorerFileIcon".into(), Style::default().fg(palette.blue));
        hl.insert("TreeExplorerFolderName".into(), Style::default().fg(palette.white).add_modifier(Modifier::BOLD));
        hl.insert("TreeExplorerFileName".into(), Style::default().fg(palette.white));

        Self { palette, highlights: hl }
    }

    pub fn get(&self, group: &str) -> Style {
        self.highlights.get(group).copied().unwrap_or_default()
    }
}
