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
            darker_black: Color::Rgb(25, 24, 40),
            black: Color::Rgb(30, 29, 45),
            black2: Color::Rgb(40, 39, 55),
            grey: Color::Rgb(47, 46, 62),
            grey_fg: Color::Rgb(56, 55, 71),
            red: Color::Rgb(243, 139, 168),
            green: Color::Rgb(171, 233, 179),
            yellow: Color::Rgb(250, 227, 176),
            blue: Color::Rgb(137, 180, 250),
            purple: Color::Rgb(203, 166, 247),
            cyan: Color::Rgb(137, 220, 235),
            orange: Color::Rgb(248, 189, 150),
        }
    }

    pub fn gruvbox_material() -> Self {
        Self {
            white: Color::Rgb(199, 184, 157),
            darker_black: Color::Rgb(26, 29, 30),
            black: Color::Rgb(30, 33, 34),
            black2: Color::Rgb(44, 47, 48),
            grey: Color::Rgb(54, 57, 58),
            grey_fg: Color::Rgb(64, 67, 68),
            red: Color::Rgb(236, 107, 100),
            green: Color::Rgb(169, 182, 101),
            yellow: Color::Rgb(224, 192, 128),
            blue: Color::Rgb(125, 174, 163),
            purple: Color::Rgb(211, 134, 155),
            cyan: Color::Rgb(134, 177, 127),
            orange: Color::Rgb(231, 138, 78),
        }
    }

    pub fn ayu_dark() -> Self {
        Self {
            white: Color::Rgb(191, 198, 212), // base05/base07 mix
            darker_black: Color::Rgb(11, 14, 20),
            black: Color::Rgb(11, 14, 20),   // base00
            black2: Color::Rgb(28, 31, 37),  // base01
            grey: Color::Rgb(36, 39, 45),    // base02
            grey_fg: Color::Rgb(43, 46, 52), // base03
            red: Color::Rgb(240, 113, 116),  // base0D in ayu is red-ish
            green: Color::Rgb(170, 216, 76),
            yellow: Color::Rgb(255, 238, 153),
            blue: Color::Rgb(86, 195, 249),
            purple: Color::Rgb(255, 180, 84),
            cyan: Color::Rgb(149, 230, 203),
            orange: Color::Rgb(255, 180, 84),
        }
    }

    pub fn tokyonight() -> Self {
        Self {
            white: Color::Rgb(192, 202, 245),
            darker_black: Color::Rgb(26, 27, 38),
            black: Color::Rgb(26, 27, 38),
            black2: Color::Rgb(36, 38, 54),
            grey: Color::Rgb(59, 66, 97),
            grey_fg: Color::Rgb(68, 75, 110),
            red: Color::Rgb(247, 118, 118),
            green: Color::Rgb(158, 206, 106),
            yellow: Color::Rgb(224, 175, 104),
            blue: Color::Rgb(122, 162, 247),
            purple: Color::Rgb(187, 154, 247),
            cyan: Color::Rgb(125, 207, 255),
            orange: Color::Rgb(255, 158, 100),
        }
    }

    pub fn onedark() -> Self {
        Self {
            white: Color::Rgb(171, 178, 191), // base05
            darker_black: Color::Rgb(27, 31, 39),
            black: Color::Rgb(30, 34, 42),    // base00
            black2: Color::Rgb(37, 41, 49),   // base01 (#252931)
            grey: Color::Rgb(84, 88, 98),     // base03
            grey_fg: Color::Rgb(86, 92, 100), // base04
            red: Color::Rgb(224, 108, 117),
            green: Color::Rgb(152, 195, 121),
            yellow: Color::Rgb(229, 192, 123),
            blue: Color::Rgb(97, 175, 239),
            purple: Color::Rgb(198, 120, 221),
            cyan: Color::Rgb(86, 182, 194),
            orange: Color::Rgb(209, 154, 102),
        }
    }

    pub fn everforest() -> Self {
        Self {
            white: Color::Rgb(211, 198, 170),
            darker_black: Color::Rgb(30, 35, 38),
            black: Color::Rgb(35, 42, 46),
            black2: Color::Rgb(45, 53, 59),
            grey: Color::Rgb(71, 82, 88),
            grey_fg: Color::Rgb(86, 95, 100),
            red: Color::Rgb(230, 126, 128),
            green: Color::Rgb(167, 192, 128),
            yellow: Color::Rgb(214, 182, 125),
            blue: Color::Rgb(127, 187, 179),
            purple: Color::Rgb(214, 153, 182),
            cyan: Color::Rgb(131, 192, 146),
            orange: Color::Rgb(227, 139, 110),
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
        let name_lower = name.to_lowercase();
        let palette = if name_lower.contains("gruvbox") {
            Palette::gruvbox_material()
        } else if name_lower.contains("ayu") {
            Palette::ayu_dark()
        } else if name_lower.contains("one") {
            Palette::onedark()
        } else if name_lower.contains("tokyo") {
            Palette::tokyonight()
        } else if name_lower.contains("everforest") {
            Palette::everforest()
        } else {
            Palette::catppuccin()
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
        hl.insert("Todo".into(), Style::default().fg(palette.black).bg(palette.yellow).add_modifier(Modifier::BOLD));

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
