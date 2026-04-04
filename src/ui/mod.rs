pub mod colorscheme;
pub mod explorer;
pub mod icons;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect, Margin},
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

    fn draw_mason(
        &self,
        frame: &mut Frame,
        lsp_manager: &crate::lsp::LspManager,
        theme: &crate::ui::colorscheme::ColorScheme,
    ) {
        let area = frame.area();
        let mason_width = (area.width as f32 * 0.6) as u16;
        let mason_height = (area.height as f32 * 0.7) as u16;
        let mason_area = Rect {
            x: (area.width - mason_width) / 2,
            y: (area.height - mason_height) / 2,
            width: mason_width,
            height: mason_height,
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Mason.atom ")
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));
        
        frame.render_widget(Clear, mason_area);
        frame.render_widget(block, mason_area);

        let inner_area = mason_area.inner(Margin { horizontal: 2, vertical: 1 });
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(inner_area);

        frame.render_widget(Paragraph::new(" Language Servers ").style(Style::default().add_modifier(Modifier::BOLD)), chunks[0]);
        frame.render_widget(Paragraph::new("──────────────────").style(theme.get("TreeExplorerConnector")), chunks[1]);

        let servers = [
            ("rust-analyzer", "rs"),
            ("pyright-langserver", "py"),
            ("typescript-language-server", "ts"),
        ];

        let mut items = Vec::new();
        for (cmd, ext) in servers {
            let is_installed = lsp_manager.is_installed(cmd);
            let status = if is_installed {
                Span::styled(" ● installed ", theme.get("String"))
            } else {
                Span::styled(" ○ not installed ", theme.get("Comment"))
            };
            
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<30} ", cmd), theme.get("Keyword")),
                status,
                Span::styled(format!(" ({})", ext), theme.get("Type")),
            ])));
        }

        frame.render_widget(List::new(items), chunks[2]);
        frame.render_widget(Paragraph::new(" Press Esc to close ").style(theme.get("Comment")).alignment(Alignment::Center), chunks[3]);
    }

    fn draw_keymaps(
        &self,
        frame: &mut Frame,
        vim: &mut crate::vim::VimState,
        theme: &crate::ui::colorscheme::ColorScheme,
    ) {
        let area = frame.area();
        let width = 50;
        let height = 25;
        let x = area.width.saturating_sub(width + 2);
        let y = area.height.saturating_sub(height + 2);
        let keymap_area = Rect { x, y, width, height: height.min(area.height - 2) };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Keymaps Help ")
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));
        
        frame.render_widget(Clear, keymap_area);
        frame.render_widget(block, keymap_area);

        let inner_area = keymap_area.inner(Margin { horizontal: 2, vertical: 1 });
        
        let mut items = Vec::new();
        let header_style = Style::default().add_modifier(Modifier::BOLD).fg(theme.palette.blue);
        let key_style = theme.get("Keyword");
        let desc_style = theme.get("Normal");

        // Normal Mode
        items.push(ListItem::new(Line::from(vec![Span::styled("--- NORMAL ---", header_style)])));
        let normal_keys = [
            ("i", "Insert mode"),
            ("v", "Visual mode"),
            (":", "Command mode"),
            ("/", "Search mode"),
            ("h/j/k/l", "Movement"),
            ("w/b/e", "Word movement"),
            ("u", "Undo"),
            ("<C-r>", "Redo"),
            ("dd", "Delete line"),
            ("yy", "Yank line"),
            ("p/P", "Paste after/before"),
            ("o/O", "Open line below/above"),
            ("\\", "Toggle Explorer"),
            ("?", "Close Help"),
            ("q", "Quit"),
        ];
        for (k, d) in normal_keys {
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<12}", k), key_style),
                Span::styled(" - ", theme.get("Comment")),
                Span::styled(d, desc_style),
            ])));
        }

        items.push(ListItem::new(Line::from("")));

        // Insert Mode
        items.push(ListItem::new(Line::from(vec![Span::styled("--- INSERT ---", header_style)])));
        let insert_keys = [
            ("<Esc>", "Normal mode"),
            ("<Tab>", "2 Spaces / CMP Next"),
            ("<Up/Down>", "CMP Nav"),
            ("<C-Space>", "Trigger CMP"),
            ("<C-n/p>", "CMP Next/Prev"),
            ("<Enter>", "Select CMP / New Line"),
        ];
        for (k, d) in insert_keys {
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<12}", k), key_style),
                Span::styled(" - ", theme.get("Comment")),
                Span::styled(d, desc_style),
            ])));
        }

        items.push(ListItem::new(Line::from("")));

        // Explorer
        items.push(ListItem::new(Line::from(vec![Span::styled("--- EXPLORER ---", header_style)])));
        let explorer_keys = [
            ("j/k", "Navigate"),
            ("l/Enter", "Expand/Open"),
            ("h", "Collapse"),
            ("a", "Add file/folder"),
            ("r", "Rename"),
            ("d", "Delete"),
        ];
        for (k, d) in explorer_keys {
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<12}", k), key_style),
                Span::styled(" - ", theme.get("Comment")),
                Span::styled(d, desc_style),
            ])));
        }

        let list = List::new(items)
            .highlight_style(Style::default().bg(theme.palette.black2));
        
        frame.render_stateful_widget(list, inner_area, &mut vim.keymap_state);
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        editor: &crate::editor::Editor,
        vim: &mut crate::vim::VimState,
        explorer: &explorer::FileExplorer,
        lsp_manager: &crate::lsp::LspManager,
    ) {
        let area = frame.area();
        let theme = &editor.highlighter.theme;
        
        // Ensure full screen background
        frame.render_widget(Block::default().style(theme.get("Normal")), area);

        let root_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area);

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(if explorer.visible {
                [Constraint::Percentage(15), Constraint::Percentage(85)]
            } else {
                [Constraint::Percentage(0), Constraint::Percentage(100)]
            })
            .split(root_chunks[0]);

        // 1. File Explorer
        if explorer.visible {
            let sidebar_divider = Block::default()
                .borders(Borders::RIGHT)
                .border_style(theme.get("TreeExplorerConnector"));
            frame.render_widget(sidebar_divider, main_chunks[0]);

            let explorer_content_area = Rect {
                x: main_chunks[0].x,
                y: main_chunks[0].y,
                width: main_chunks[0].width.saturating_sub(1),
                height: main_chunks[0].height,
            };

            let explorer_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(1)])
                .split(explorer_content_area);

            let header_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(Span::styled(" Explorer ", theme.get("TreeExplorerRoot")))
                .border_style(theme.get("TreeExplorerConnector"))
                .padding(Padding::horizontal(1));
            
            let header_inner = header_block.inner(explorer_layout[0]);
            frame.render_widget(header_block, explorer_layout[0]);

            let header_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Min(1), Constraint::Length(8)])
                .split(header_inner);

            let filter_display = if let Mode::ExplorerInput(ExplorerInputType::Filter) = vim.mode {
                format!("> {}", vim.input_buffer)
            } else {
                format!("> {}", explorer.filter)
            };
            frame.render_widget(Paragraph::new(filter_display).style(theme.get("Keyword")), header_chunks[0]);

            let count_text = format!("{}/{}", explorer.entries.len(), explorer.entries.len());
            frame.render_widget(Paragraph::new(count_text).alignment(Alignment::Right).style(theme.get("Comment")), header_chunks[1]);

            if let Mode::ExplorerInput(ExplorerInputType::Filter) = vim.mode {
                frame.set_cursor_position((
                    header_chunks[0].x + vim.input_buffer.len() as u16 + 2,
                    header_chunks[0].y,
                ));
            }

            let items: Vec<ListItem> = explorer
                .entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let name = entry.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                    let mut guide = String::new();
                    for _ in 0..entry.depth { guide.push_str("│ "); }
                    if entry.depth > 0 {
                        guide.pop(); guide.pop();
                        if entry.is_last { guide.push_str("└─"); } else { guide.push_str("├─"); }
                    }

                    let (icon, icon_group) = Self::get_file_icon(&entry.path);
                    let mut name_style = if entry.is_dir { theme.get("TreeExplorerFolderName") } else { theme.get("TreeExplorerFileName") };
                    let icon_style = if name.starts_with('.') || entry.is_ignored { theme.get("Comment") } else { theme.get(&icon_group) };

                    if name.starts_with('.') || entry.is_ignored {
                        name_style = theme.get("Comment");
                    }

                    let mut spans = vec![
                        Span::raw(" "),
                        Span::styled(guide, theme.get("TreeExplorerConnector")),
                        Span::styled(format!("{} ", icon), icon_style),
                        Span::styled(name, name_style),
                        Span::raw("    "),
                    ];

                    let mut line_style = Style::default();
                    if i == explorer.selected_idx {
                        line_style = theme.get("CursorLine");
                        spans[3] = spans[3].clone().add_modifier(Modifier::BOLD);
                    }

                    ListItem::new(Line::from(spans)).style(line_style)
                })
                .collect();

            frame.render_widget(List::new(items), explorer_layout[1]);
        }

        // 2. Editor Area
        let buffer = editor.buffer();
        let cursor = editor.cursor();
        let scroll_y = cursor.scroll_y;
        let visible_height = main_chunks[1].height as usize;
        
        let editor_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(6), Constraint::Min(1)])
            .split(main_chunks[1]);

        // Full width highlight for active line
        let current_line_screen_y = cursor.y.saturating_sub(scroll_y);
        if current_line_screen_y < visible_height {
            let highlight_rect = Rect {
                x: main_chunks[1].x,
                y: main_chunks[1].y + current_line_screen_y as u16,
                width: main_chunks[1].width,
                height: 1,
            };
            frame.render_widget(Block::default().style(theme.get("CursorLine")), highlight_rect);
        }

        // Line Numbers
        let mut line_numbers = Text::default();
        for i in scroll_y..std::cmp::min(scroll_y + visible_height, buffer.lines.len()) {
            let is_active = i == cursor.y;
            let style = if is_active { theme.get("CursorLineNr") } else { theme.get("LineNr") };
            let mut line = Line::from(vec![Span::styled(format!("{:>4} ", i + 1), style)]);
            // Don't apply background to line number column unless we want it unified
            line_numbers.lines.push(line);
        }
        frame.render_widget(Paragraph::new(line_numbers).alignment(Alignment::Right), editor_layout[0]);

        // Code Content
        let mut text = Text::default();
        let search_query = &vim.search_query;

        for i in scroll_y..std::cmp::min(scroll_y + visible_height, buffer.lines.len()) {
            let line = &buffer.lines[i];
            let mut spans = Vec::new();
            let syntax_styles = editor.highlighter.highlight_line(line);
            let is_current_line = i == cursor.y;

            for (x, c) in line.chars().enumerate() {
                let mut style = syntax_styles.get(x).copied().unwrap_or(theme.get("Normal"));
                
                // Overlay Highlights
                if let Some(start) = vim.selection_start {
                    let cur = crate::vim::Position { x: cursor.x, y: cursor.y };
                    let (s_y, s_x, e_y, e_x) = if (start.y, start.x) < (cur.y, cur.x) { (start.y, start.x, cur.y, cur.x) } else { (cur.y, cur.x, start.y, start.x) };
                    let is_in_range = if i > s_y && i < e_y { true } else if i == s_y && i == e_y { x >= s_x && x <= e_x } else if i == s_y { x >= s_x } else if i == e_y { x <= e_x } else { false };
                    if is_in_range { style = theme.get("Visual"); }
                }
                if !search_query.is_empty() {
                    if let Some(pos) = line.to_lowercase().find(&search_query.to_lowercase()) {
                        if x >= pos && x < pos + search_query.len() {
                            style = theme.get("Search");
                        }
                    }
                }
                if vim.yank_highlight_line == Some(i) { style = Style::default().bg(theme.palette.blue).fg(theme.palette.black); }
                
                // Apply CursorLine background to character if it's the current line
                if is_current_line && style.bg.is_none() {
                    style = style.bg(theme.palette.black2);
                }

                spans.push(Span::styled(c.to_string(), style));
            }
            if line.is_empty() { 
                let mut style = theme.get("Normal");
                if is_current_line { style = style.bg(theme.palette.black2); }
                spans.push(Span::styled(" ", style)); 
            }
            
            let mut line_obj = Line::from(spans);
            if is_current_line {
                line_obj = line_obj.style(theme.get("CursorLine"));
            }
            text.lines.push(line_obj);
        }
        frame.render_widget(Paragraph::new(text), editor_layout[1]);

        // 2.5 Completion Menu (Floating)
        if vim.show_suggestions && !vim.suggestions.is_empty() {
            // Filter suggestions based on current word prefix
            let (y, x) = (cursor.y, cursor.x);
            let line = &buffer.lines[y];
            let mut start_x = x;
            let chars: Vec<char> = line.chars().collect();
            while start_x > 0 && (chars[start_x-1].is_alphanumeric() || chars[start_x-1] == '_' || chars[start_x-1] == '$') {
                start_x -= 1;
            }
            let prefix = if start_x < x { line[start_x..x].to_lowercase() } else { String::new() };

            let mut unique_items = std::collections::HashSet::new();
            let filtered_suggestions: Vec<(&lsp_types::CompletionItem, usize)> = vim.suggestions.iter().enumerate()
                .filter(|(_, item)| {
                    let key = format!("{}:{:?}", item.label, item.kind);
                    if unique_items.contains(&key) { return false; }
                    if item.label.to_lowercase().contains(&prefix) {
                        unique_items.insert(key);
                        true
                    } else { false }
                })
                .map(|(i, item)| (item, i))
                .collect();

            if !filtered_suggestions.is_empty() {
                let menu_width = 45;
                let menu_height = std::cmp::min(10, filtered_suggestions.len()) as u16 + 2;
                let menu_x = editor_layout[1].x + cursor.x as u16;
                let menu_y = editor_layout[1].y + current_line_screen_y as u16 + 1;

                let menu_area = Rect {
                    x: menu_x.min(area.right().saturating_sub(menu_width)),
                    y: menu_y.min(root_chunks[1].y.saturating_sub(menu_height)), 
                    width: menu_width,
                    height: menu_height,
                };

                let items: Vec<ListItem> = filtered_suggestions.iter().enumerate().map(|(display_idx, (item, _))| {
                    let (icon, kind_name, color_group) = match item.kind {
                        Some(lsp_types::CompletionItemKind::FUNCTION) => (icons::FUNCTION, "Function", "Function"),
                        Some(lsp_types::CompletionItemKind::METHOD) => (icons::METHOD, "Method", "Function"),
                        Some(lsp_types::CompletionItemKind::VARIABLE) => (icons::VARIABLE, "Variable", "Variable"),
                        Some(lsp_types::CompletionItemKind::CLASS) => (icons::CLASS, "Class", "Type"),
                        Some(lsp_types::CompletionItemKind::INTERFACE) => (icons::INTERFACE, "Interface", "Type"),
                        Some(lsp_types::CompletionItemKind::KEYWORD) => (icons::KEYWORD, "Keyword", "Keyword"),
                        Some(lsp_types::CompletionItemKind::SNIPPET) => (icons::SNIPPET, "Snippet", "Keyword"),
                        Some(lsp_types::CompletionItemKind::FIELD) => (icons::FIELD, "Field", "Identifier"),
                        Some(lsp_types::CompletionItemKind::PROPERTY) => (icons::PROPERTY, "Property", "Identifier"),
                        Some(lsp_types::CompletionItemKind::TEXT) => (icons::TEXT, "Text", "Comment"),
                        _ => (icons::OBJECT, "Object", "Constant"),
                    };
                    
                    let mut label_style = theme.get("Normal");
                    let mut icon_style = theme.get(color_group);
                    let mut kind_style = theme.get("Comment");
                    
                    if display_idx == (vim.selected_suggestion % filtered_suggestions.len()) {
                        label_style = Style::default().fg(theme.palette.black).bg(theme.palette.blue).add_modifier(Modifier::BOLD);
                        icon_style = Style::default().fg(theme.palette.black).bg(theme.palette.blue);
                        kind_style = Style::default().fg(theme.palette.black).bg(theme.palette.blue);
                    }
                    
                    ListItem::new(Line::from(vec![
                        Span::styled(format!(" {} ", icon), icon_style),
                        Span::styled(format!("{:<30}", item.label), label_style),
                        Span::styled(format!(" {:>8} ", kind_name), kind_style),
                    ]))
                }).collect();

                let menu = List::new(items)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(theme.get("TreeExplorerConnector"))
                        .style(theme.get("Normal")));
                
                frame.render_widget(Clear, menu_area);
                frame.render_stateful_widget(menu, menu_area, &mut vim.suggestion_state);

                // Floating Doc Window
                let selected_idx = vim.selected_suggestion % filtered_suggestions.len();
                if let Some((item, _)) = filtered_suggestions.get(selected_idx) {
                    if let Some(detail) = &item.detail {
                        let doc_width = 40;
                        let doc_height = menu_height;
                        let doc_x = if menu_area.right() + doc_width <= area.right() { menu_area.right() } else { menu_area.left().saturating_sub(doc_width) };
                        let doc_area = Rect { x: doc_x, y: menu_area.y, width: doc_width, height: doc_height };

                        let doc_text = detail.clone();
                        let doc_paragraph = Paragraph::new(doc_text)
                            .block(Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Rounded)
                                .border_style(theme.get("TreeExplorerConnector"))
                                .style(theme.get("Normal")))
                            .wrap(ratatui::widgets::Wrap { trim: true });
                        
                        frame.render_widget(Clear, doc_area);
                        frame.render_widget(doc_paragraph, doc_area);
                    }
                }
            }
        }

        // 2.6 LSP Install Prompt
        if let Some(lsp_cmd) = &vim.lsp_to_install {
            let prompt_text = format!(" LSP '{}' not found. Install it? (y/n) ", lsp_cmd);
            let prompt_width = prompt_text.len() as u16 + 4;
            let prompt_area = Rect {
                x: (area.width.saturating_sub(prompt_width)) / 2,
                y: area.height / 2,
                width: prompt_width,
                height: 3,
            };
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

        // 4. Command/Search/Input Line
        match vim.mode {
            Mode::Command => {
                let text = format!(":{}", vim.command_buffer);
                frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]);
                frame.set_cursor_position((root_chunks[2].x + vim.command_buffer.len() as u16 + 1, root_chunks[2].y));
            }
            Mode::Search => {
                let text = format!("/{}", vim.search_query);
                frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]);
                frame.set_cursor_position((root_chunks[2].x + vim.search_query.len() as u16 + 1, root_chunks[2].y));
            }
            Mode::ExplorerInput(input_type) => {
                let prompt = match input_type {
                    ExplorerInputType::Add => "Add a new file or directory (directories end with a \"/\"): ",
                    ExplorerInputType::Rename => "New File Name: ",
                    ExplorerInputType::Move => "Move To: ",
                    ExplorerInputType::DeleteConfirm => "Delete selected? (y/n): ",
                    ExplorerInputType::Filter => "", 
                };
                if input_type != ExplorerInputType::Filter {
                    let text = format!("{}{}", prompt, vim.input_buffer);
                    frame.render_widget(Paragraph::new(text).style(theme.get("Normal")), root_chunks[2]);
                    frame.set_cursor_position((root_chunks[2].x + prompt.len() as u16 + vim.input_buffer.len() as u16, root_chunks[2].y));
                } else {
                    frame.render_widget(Paragraph::new("").style(theme.get("Normal")), root_chunks[2]);
                }
            }
            _ => {
                frame.render_widget(Paragraph::new("").style(theme.get("Normal")), root_chunks[2]);
                if vim.focus == Focus::Editor {
                    frame.set_cursor_position((editor_layout[1].x + cursor.x as u16, editor_layout[1].y + current_line_screen_y as u16));
                }
            }
        }

        if let Mode::Mason = vim.mode {
            self.draw_mason(frame, lsp_manager, theme);
        }

        if let Mode::Keymaps = vim.mode {
            self.draw_keymaps(frame, vim, theme);
        }
    }
}
