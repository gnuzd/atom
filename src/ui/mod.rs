pub mod colorscheme;
pub mod explorer;
pub mod icons;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, BorderType, List, ListItem, Padding, Paragraph, Clear},
    Frame,
};
use crate::vim::mode::{Mode, ExplorerInputType, Focus};
use crate::vim::LspStatus;

pub struct TerminalUi;

impl TerminalUi {
    pub fn new() -> Self {
        Self
    }

    fn get_file_icon(path: &std::path::Path) -> (&'static str, String) {
        if path.is_dir() { return (icons::FOLDER, "TreeExplorerFolderIcon".into()); }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext {
            "rs" => (icons::STRUCT, "Identifier".into()),
            "toml" => (icons::PACKAGE, "Type".into()),
            "md" => (icons::TEXT, "Function".into()),
            "lock" => (icons::FILE, "Comment".into()),
            _ => (icons::FILE, "TreeExplorerFileIcon".into()),
        }
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        editor: &crate::editor::Editor,
        vim: &mut crate::vim::VimState,
        explorer: &explorer::FileExplorer,
    ) {
        let area = frame.area();
        let theme = &editor.highlighter.theme;
        frame.render_widget(Block::default().style(theme.get("Normal")), area);

        let root_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(if explorer.visible { [Constraint::Percentage(15), Constraint::Percentage(85)] } else { [Constraint::Percentage(0), Constraint::Percentage(100)] })
            .split(root_chunks[0]);

        // 1. Explorer
        if explorer.visible {
            let sidebar_divider = Block::default().borders(Borders::RIGHT).border_style(theme.get("TreeExplorerConnector"));
            frame.render_widget(sidebar_divider, main_chunks[0]);
            let explorer_content_area = Rect { x: main_chunks[0].x, y: main_chunks[0].y, width: main_chunks[0].width.saturating_sub(1), height: main_chunks[0].height };
            let explorer_layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(3), Constraint::Min(1)]).split(explorer_content_area);
            let header_block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(Span::styled(" Explorer ", theme.get("TreeExplorerRoot"))).border_style(theme.get("TreeExplorerConnector")).padding(Padding::horizontal(1));
            let header_inner = header_block.inner(explorer_layout[0]);
            frame.render_widget(header_block, explorer_layout[0]);
            let header_chunks = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Min(1), Constraint::Length(8)]).split(header_inner);
            let filter_display = if let Mode::ExplorerInput(ExplorerInputType::Filter) = vim.mode { format!("> {}", vim.input_buffer) } else { format!("> {}", explorer.filter) };
            frame.render_widget(Paragraph::new(filter_display).style(theme.get("Keyword")), header_chunks[0]);
            frame.render_widget(Paragraph::new(format!("{}/{}", explorer.entries.len(), explorer.entries.len())).alignment(Alignment::Right).style(theme.get("Comment")), header_chunks[1]);
            if let Mode::ExplorerInput(ExplorerInputType::Filter) = vim.mode { frame.set_cursor_position((header_chunks[0].x + vim.input_buffer.len() as u16 + 2, header_chunks[0].y)); }
            let items: Vec<ListItem> = explorer.entries.iter().enumerate().map(|(i, entry)| {
                let name = entry.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                let mut guide = String::new();
                for _ in 0..entry.depth { guide.push_str("│ "); }
                if entry.depth > 0 { guide.pop(); guide.pop(); if entry.is_last { guide.push_str("└─"); } else { guide.push_str("├─"); } }
                let (icon, icon_group) = Self::get_file_icon(&entry.path);
                let mut name_style = if entry.is_dir { theme.get("TreeExplorerFolderName") } else { theme.get("TreeExplorerFileName") };
                let icon_style = if name.starts_with('.') || entry.is_ignored { theme.get("Comment") } else { theme.get(&icon_group) };
                if name.starts_with('.') || entry.is_ignored { name_style = theme.get("Comment"); }
                let mut spans = vec![Span::raw(" "), Span::styled(guide, theme.get("TreeExplorerConnector")), Span::styled(format!("{} ", icon), icon_style), Span::styled(name, name_style), Span::raw("    ")];
                let mut line_style = Style::default();
                if i == explorer.selected_idx { line_style = theme.get("CursorLine"); spans[3] = spans[3].clone().add_modifier(Modifier::BOLD); }
                ListItem::new(Line::from(spans)).style(line_style)
            }).collect();
            frame.render_widget(List::new(items), explorer_layout[1]);
        }

        // 2. Editor
        let buffer = editor.buffer();
        let cursor = editor.cursor();
        let scroll_y = cursor.scroll_y;
        let visible_height = main_chunks[1].height as usize;
        let editor_layout = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(6), Constraint::Min(1)]).split(main_chunks[1]);
        let current_line_screen_y = cursor.y.saturating_sub(scroll_y);
        if current_line_screen_y < visible_height {
            let highlight_rect = Rect { x: main_chunks[1].x, y: main_chunks[1].y + current_line_screen_y as u16, width: main_chunks[1].width, height: 1 };
            frame.render_widget(Block::default().style(theme.get("CursorLine")), highlight_rect);
        }
        let mut line_numbers = Text::default();
        for i in scroll_y..std::cmp::min(scroll_y + visible_height, buffer.lines.len()) {
            let is_active = i == cursor.y;
            let style = if is_active { theme.get("CursorLineNr") } else { theme.get("LineNr") };
            let mut line = Line::from(vec![Span::styled(format!("{:>4} ", i + 1), style)]);
            if is_active { line = line.style(theme.get("CursorLine")); }
            line_numbers.lines.push(line);
        }
        frame.render_widget(Paragraph::new(line_numbers).alignment(Alignment::Right).style(theme.get("Normal")), editor_layout[0]);
        let mut text = Text::default();
        let search_query = &vim.search_query;
        for i in scroll_y..std::cmp::min(scroll_y + visible_height, buffer.lines.len()) {
            let line = &buffer.lines[i];
            let mut spans = Vec::new();
            let syntax_styles = editor.highlighter.highlight_line(line);
            let is_current_line = i == cursor.y;
            for (x, c) in line.chars().enumerate() {
                let mut style = syntax_styles.get(x).copied().unwrap_or(theme.get("Normal"));
                if is_current_line { style = style.bg(theme.palette.black2); }
                if let Some(start) = vim.selection_start {
                    // ... selection logic ...
                }
                if !search_query.is_empty() {
                    if let Some(pos) = line.to_lowercase().find(&search_query.to_lowercase()) {
                        if x >= pos && x < pos + search_query.len() { style = theme.get("Search"); }
                    }
                }
                if vim.yank_highlight_line == Some(i) { style = Style::default().bg(theme.palette.blue).fg(theme.palette.black); }
                spans.push(Span::styled(c.to_string(), style));
            }
            if line.is_empty() { let style = if is_current_line { theme.get("CursorLine") } else { theme.get("Normal") }; spans.push(Span::styled(" ", style)); }
            let mut line_obj = Line::from(spans);
            if is_current_line { line_obj = line_obj.style(theme.get("CursorLine")); }
            text.lines.push(line_obj);
        }
        frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), editor_layout[1]);

        // Floating Menu
        if vim.show_suggestions && !vim.suggestions.is_empty() {
            let menu_width = 40;
            let menu_height = std::cmp::min(10, vim.suggestions.len()) as u16;
            let menu_x = editor_layout[1].x + cursor.x as u16;
            let menu_y = editor_layout[1].y + current_line_screen_y as u16 + 1;
            let menu_area = Rect { x: menu_x.min(area.right().saturating_sub(menu_width)), y: menu_y.min(area.bottom().saturating_sub(menu_height + 2)), width: menu_width, height: menu_height };
            let items: Vec<ListItem> = vim.suggestions.iter().enumerate().map(|(i, item)| {
                let mut style = Style::default().fg(theme.palette.white).bg(theme.palette.black2);
                if i == vim.selected_suggestion { style = Style::default().fg(theme.palette.black).bg(theme.palette.blue).add_modifier(Modifier::BOLD); }
                ListItem::new(item.label.clone()).style(style)
            }).collect();
            let menu = List::new(items).block(Block::default().borders(Borders::ALL).border_style(theme.get("TreeExplorerConnector")));
            frame.render_widget(Clear, menu_area);
            frame.render_widget(menu, menu_area);
        }

        // LSP Prompt
        if let Some(lsp_cmd) = &vim.lsp_to_install {
            let prompt_text = format!(" LSP '{}' not found. Install it? (y/n) ", lsp_cmd);
            let prompt_width = prompt_text.len() as u16 + 4;
            let prompt_area = Rect { x: (area.width.saturating_sub(prompt_width)) / 2, y: area.height / 2, width: prompt_width, height: 3 };
            let prompt = Paragraph::new(prompt_text).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).style(theme.get("Keyword")));
            frame.render_widget(Clear, prompt_area);
            frame.render_widget(prompt, prompt_area);
        }

        // 3. Status Line
        let (mode_group, mode_label) = match vim.mode {
            Mode::Normal => ("StatusLineNormal", " NORMAL "),
            Mode::Insert => ("StatusLineInsert", " INSERT "),
            Mode::Visual => ("StatusLineVisual", " VISUAL "),
            Mode::Command => ("StatusLineCommand", " COMMAND "),
            _ => ("StatusLine", " OTHER "),
        };
        let file_name = buffer.file_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("[No Name]");
        
        let mut status_spans = vec![
            Span::styled(mode_label, theme.get(mode_group)),
            Span::styled(format!(" {} ", file_name), theme.get("StatusLineFile")),
        ];

        // LSP Status with Spinner
        match &vim.lsp_status {
            LspStatus::Loading | LspStatus::Installing => {
                status_spans.push(Span::styled(format!(" {} Loading... ", vim.get_spinner()), theme.get("Keyword")));
            }
            LspStatus::Ready => {
                status_spans.push(Span::styled(" LSP: Ready ", theme.get("String")));
            }
            LspStatus::Error(e) => {
                status_spans.push(Span::styled(format!(" LSP Error: {} ", e), theme.get("Identifier")));
            }
            _ => {}
        }

        status_spans.push(Span::styled(" ", theme.get("StatusLine"))); // Filler
        status_spans.push(Span::styled(format!(" {}:{} (Buffer {}/{}) ", cursor.y + 1, cursor.x + 1, editor.active_idx + 1, editor.buffers.len()), theme.get("StatusLinePos")));
        
        frame.render_widget(Paragraph::new(Line::from(status_spans)).style(theme.get("StatusLine")), root_chunks[1]);

        // 4. Command Line
        match vim.mode {
            Mode::Command => { let text = format!(":{}", vim.command_buffer); frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]); frame.set_cursor_position((root_chunks[2].x + vim.command_buffer.len() as u16 + 1, root_chunks[2].y)); }
            Mode::Search => { let text = format!("/{}", vim.search_query); frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]); frame.set_cursor_position((root_chunks[2].x + vim.search_query.len() as u16 + 1, root_chunks[2].y)); }
            Mode::ExplorerInput(ExplorerInputType::DeleteConfirm) => { let text = format!("Delete selected? (y/n): {}", vim.input_buffer); frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]); frame.set_cursor_position((root_chunks[2].x + 23 + vim.input_buffer.len() as u16, root_chunks[2].y)); }
            Mode::ExplorerInput(_) => { /* handled elsewhere */ }
            _ => {
                frame.render_widget(Paragraph::new("").style(theme.get("Normal")), root_chunks[2]);
                if vim.focus == Focus::Editor { frame.set_cursor_position((editor_layout[1].x + cursor.x as u16, editor_layout[1].y + current_line_screen_y as u16)); }
            }
        }
    }
}
