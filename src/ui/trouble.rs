use std::path::PathBuf;
use ratatui::{
    layout::{Rect, Alignment},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, List, ListItem, Padding, Paragraph, Clear},
    Frame,
};
use crate::ui::icons;
use crate::ui::colorscheme::ColorScheme;
use crate::vim::mode::Focus;
use crate::vim::VimState;
use lsp_types::{Diagnostic, DiagnosticSeverity, Url};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum TroubleType {
    Diagnostic(Diagnostic),
    Todo,
}

#[derive(Clone, Debug)]
pub struct TroubleItem {
    pub path: PathBuf,
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub severity: Option<DiagnosticSeverity>,
    pub item_type: TroubleType,
}

pub struct TroubleList {
    pub items: Vec<TroubleItem>,
    pub selected_idx: usize,
    pub visible: bool,
    pub scanned: bool,
}

impl TroubleList {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected_idx: 0,
            visible: false,
            scanned: false,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if !self.visible {
            self.scanned = false; // Reset scan when closing to allow fresh scan?
            // Actually, maybe better to keep it and add a refresh key.
            // But let's follow the simple path.
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.items.is_empty() && self.selected_idx < self.items.len() - 1 {
            self.selected_idx += 1;
        }
    }

    pub fn selected_item(&self) -> Option<&TroubleItem> {
        self.items.get(self.selected_idx)
    }

    pub fn update_from_lsp(&mut self, diagnostics: &HashMap<Url, Vec<Diagnostic>>, todos: Vec<TroubleItem>) {
        let mut new_items = Vec::new();

        // Add LSP diagnostics
        for (url, diags) in diagnostics {
            if let Ok(path) = url.to_file_path() {
                for diag in diags {
                    new_items.push(TroubleItem {
                        path: path.clone(),
                        line: diag.range.start.line as usize,
                        col: diag.range.start.character as usize,
                        message: diag.message.clone(),
                        severity: diag.severity,
                        item_type: TroubleType::Diagnostic(diag.clone()),
                    });
                }
            }
        }

        // Add TODOs
        new_items.extend(todos);

        // Sort by path, then line
        new_items.sort_by(|a, b| {
            if a.path != b.path {
                a.path.cmp(&b.path)
            } else {
                a.line.cmp(&b.line)
            }
        });

        self.items = new_items;
        if self.selected_idx >= self.items.len() {
            self.selected_idx = self.items.len().saturating_sub(1);
        }
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        area: Rect,
        vim: &VimState,
        theme: &ColorScheme,
    ) {
        if !self.visible { return; }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(" Trouble ", theme.get("TreeExplorerRoot")))
            .border_style(if vim.focus == Focus::Trouble { theme.get("Keyword") } else { theme.get("TreeExplorerConnector") })
            .style(theme.get("Normal"))
            .padding(Padding::horizontal(1));

        let inner_area = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);

        if self.items.is_empty() {
            let empty_msg = Paragraph::new("No problems found")
                .style(theme.get("Comment"))
                .alignment(Alignment::Center);
            frame.render_widget(empty_msg, inner_area);
            return;
        }

        let mut list_items = Vec::new();
        let mut current_path: Option<PathBuf> = None;

        for (i, item) in self.items.iter().enumerate() {
            // Group by file
            if current_path.as_ref() != Some(&item.path) {
                current_path = Some(item.path.clone());
                let file_name = item.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                list_items.push(ListItem::new(Line::from(vec![
                    Span::styled(format!(" {} ", icons::FILE), theme.get("TreeExplorerFileIcon")),
                    Span::styled(file_name, theme.get("TreeExplorerFileName").add_modifier(Modifier::BOLD)),
                    Span::styled(format!("  {}", item.path.display()), theme.get("Comment")),
                ])));
            }

            let (icon, icon_style) = match &item.item_type {
                TroubleType::Todo => (icons::COMMENT, theme.get("String")),
                TroubleType::Diagnostic(_) => {
                    match item.severity {
                        Some(DiagnosticSeverity::ERROR) => (icons::ERROR, theme.get("Identifier")),
                        Some(DiagnosticSeverity::WARNING) => (icons::WARNING, theme.get("Type")),
                        Some(DiagnosticSeverity::INFORMATION) => (icons::INFO, theme.get("Function")),
                        Some(DiagnosticSeverity::HINT) => (icons::HINT, theme.get("Keyword")),
                        _ => (icons::FILE, theme.get("Normal")),
                    }
                }
            };

            let style = if i == self.selected_idx && vim.focus == Focus::Trouble {
                theme.get("CursorLine")
            } else {
                Style::default()
            };

            let line_text = format!("  {:>3}:{:>2} ", item.line + 1, item.col + 1);
            let spans = vec![
                Span::styled(line_text, theme.get("LineNr")),
                Span::styled(format!("{} ", icon), icon_style),
                Span::styled(&item.message, theme.get("Normal")),
            ];

            list_items.push(ListItem::new(Line::from(spans)).style(style));
        }

        let list = List::new(list_items);
        frame.render_widget(list, inner_area);
    }
}
