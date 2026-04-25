use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect, Margin},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Table, Row, Cell},
    Frame,
};

pub mod colorscheme;
pub mod explorer;
pub mod icons;
pub mod intro;
pub mod keymaps;
pub mod nucleus;
pub mod telescope;
pub mod trouble;

use crate::vim::mode::{Focus, Mode};

pub struct TerminalUi {
}

impl TerminalUi {
    pub fn new() -> Self {
        Self {
        }
    }

    pub fn get_file_icon(path: &std::path::Path) -> (String, String) {
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        match ext {
            "rs" => (" ".to_string(), "TreeExplorerFileIcon".into()),
            "ts" | "tsx" => (" ".to_string(), "Type".into()),
            "js" | "jsx" => (" ".to_string(), "Constant".into()),
            "py" => (" ".to_string(), "Function".into()),
            "go" => (" ".to_string(), "Type".into()),
            "lua" => (" ".to_string(), "Constant".into()),
            "json" => (" ".to_string(), "String".into()),
            "toml" => (" ".to_string(), "Keyword".into()),
            "md" => (" ".to_string(), "Comment".into()),
            "html" => (" ".to_string(), "Tag".into()),
            "css" => (" ".to_string(), "Attribute".into()),
            "lock" => (icons::FILE.to_string(), "Comment".into()),
            _ => (icons::FILE.to_string(), "TreeExplorerFileIcon".into()),
        }
    }

    pub fn get_panes_and_borders(
        layout: &crate::vim::PaneLayout,
        area: Rect,
        focused_id: usize,
    ) -> (
        Vec<(Rect, usize, bool)>,
        Vec<(Rect, crate::vim::mode::SplitKind)>,
    ) {
        match layout {
            crate::vim::PaneLayout::Window(pane) => {
                (vec![(area, pane.buffer_idx, pane.id == focused_id)], vec![])
            }
            crate::vim::PaneLayout::Split(kind, children) => {
                let direction = match kind {
                    crate::vim::mode::SplitKind::Vertical => Direction::Horizontal,
                    crate::vim::mode::SplitKind::Horizontal => Direction::Vertical,
                };

                let mut constraints = Vec::new();
                for i in 0..children.len() {
                    constraints.push(Constraint::Ratio(1, children.len() as u32));
                    if i < children.len() - 1 {
                        constraints.push(Constraint::Length(1)); // border
                    }
                }

                let chunks = Layout::default()
                    .direction(direction)
                    .constraints(constraints)
                    .split(area);

                let mut panes = Vec::new();
                let mut borders = Vec::new();
                for (i, child) in children.iter().enumerate() {
                    let (p, b) = Self::get_panes_and_borders(child, chunks[i * 2], focused_id);
                    panes.extend(p);
                    borders.extend(b);
                    if i < children.len() - 1 {
                        borders.push((chunks[i * 2 + 1], *kind));
                    }
                }
                (panes, borders)
            }
        }
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        editor: &mut crate::editor::Editor,
        vim: &mut crate::vim::VimState,
        explorer: &mut explorer::FileExplorer,
        trouble: &trouble::TroubleList,
        lsp_manager: &crate::lsp::LspManager,
    ) {
        let area = frame.area();
        let theme = editor.highlighter.theme.clone();

        // Ensure full screen background
        frame.render_widget(Block::default().style(theme.get("Normal")), area);

        let root_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(if vim.config.laststatus >= 2 { 1 } else { 0 }),
                Constraint::Length(1),
            ])
            .split(area);

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(if explorer.visible {
                [Constraint::Length(explorer.width), Constraint::Min(1)]
            } else {
                [Constraint::Length(0), Constraint::Min(1)]
            })
            .split(root_chunks[0]);

        // Further split main_chunks[1] if trouble is visible
        let editor_trouble_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if trouble.visible {
                [Constraint::Percentage(70), Constraint::Percentage(30)]
            } else {
                [Constraint::Percentage(100), Constraint::Percentage(0)]
            })
            .split(main_chunks[1]);

        let trouble_area = editor_trouble_chunks[1];
        let editor_container_area = editor_trouble_chunks[0];

        let (mut panes, borders) = Self::get_panes_and_borders(
            &vim.pane_layout,
            editor_container_area,
            vim.focused_pane_id,
        );

        // Adjust focus based on whether the Editor itself is focused
        for pane in &mut panes {
            pane.2 = pane.2 && vim.focus == Focus::Editor;
        }

        for (border_area, kind) in borders {
            let borders = match kind {
                crate::vim::mode::SplitKind::Vertical => Borders::LEFT,
                crate::vim::mode::SplitKind::Horizontal => Borders::TOP,
            };
            let block = Block::default()
                .borders(borders)
                .border_style(theme.get("TreeExplorerConnector"));
            frame.render_widget(block, border_area);
        }

        for (pane_area, buf_idx, is_focused) in panes.clone() {
            self.draw_editor_pane(
                frame,
                editor,
                vim,
                lsp_manager,
                pane_area,
                buf_idx,
                is_focused,
            );
        }

        if vim.show_intro {
            intro::draw_intro(frame, editor_container_area, &theme);
        }

        // Common variables for status line and completion
        let buf_idx = editor.active_idx;
        let (buffer, cursor_y, cursor_x, cursor_scroll_y, buf_added, buf_modified, buf_removed, file_name, modified_flag) = {
            let buffer = match editor.buffers.get(buf_idx) {
                Some(b) => b,
                None => return,
            };
            let cursor = match editor.cursors.get(buf_idx) {
                Some(c) => c,
                None => return,
            };
            
            let mut added = 0;
            let mut modified = 0;
            let mut removed = 0;
            for (_, sign) in &buffer.git_signs {
                match sign {
                    crate::git::GitSign::Add => added += 1,
                    crate::git::GitSign::Change | crate::git::GitSign::ChangeDelete => modified += 1,
                    crate::git::GitSign::Delete | crate::git::GitSign::TopDelete => removed += 1,
                }
            }
            let name = buffer.file_path.as_ref().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("[No Name]");
            let modified_flag = if buffer.modified { " [+]" } else { "" };
            
            (buffer.clone(), cursor.y, cursor.x, cursor.scroll_y, added, modified, removed, name.to_string(), modified_flag.to_string())
        };

        let gutter_width = match (vim.show_number || vim.relative_number, vim.config.signcolumn) {
            (true, true) => 7,
            (true, false) => 5,
            (false, true) => 3,
            (false, false) => 0,
        };
        let editor_area = panes
            .iter()
            .find(|p| p.2)
            .map(|p| p.0)
            .unwrap_or(editor_container_area);
        let editor_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(gutter_width), Constraint::Min(1)])
            .split(editor_area);
        let editor_width = editor_layout[1].width.max(1) as usize;

        // Calculate cursor screen position
        let mut cursor_pos_in_line = 0;
        if let Some(line) = buffer.line(cursor_y) {
        for (i, c) in line.chars().enumerate() {
            if i >= cursor_x {
                break;
            }
            cursor_pos_in_line += if c == '\t' {
                2
            } else {
                unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
            };
        }
        }
        let cursor_screen_x = cursor_pos_in_line % editor_width;

        let screen_to_buffer_lines =
            editor.get_screen_to_buffer_lines_for_idx(buf_idx, editor_width, vim.config.wrap);
        let cursor_screen_y_opt = screen_to_buffer_lines
            .iter()
            .position(|&(idx, row)| {
                if idx != cursor_y {
                    return false;
                }
                if !vim.config.wrap {
                    return true;
                }
                row == cursor_pos_in_line / editor_width
            })
            .map(|pos| pos.saturating_sub(cursor_scroll_y));

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

            let filter_display = if let Mode::ExplorerInput(crate::vim::mode::ExplorerInputType::Filter) = vim.mode {
                format!("> {}", vim.input_buffer)
            } else {
                format!("> {}", explorer.filter)
            };
            frame.render_widget(
                Paragraph::new(filter_display).style(theme.get("TreeExplorerFilter")),
                header_chunks[0],
            );

            let count_text = format!(" {}/{} ", explorer.filtered_count(), explorer.total_count());
            frame.render_widget(
                Paragraph::new(count_text)
                    .style(theme.get("TreeExplorerCount"))
                    .alignment(Alignment::Right),
                header_chunks[1],
            );

            explorer.draw(frame, explorer_layout[1], vim, &theme);
        }

        // 1.5 Trouble List
        if trouble.visible {
            trouble.draw(frame, trouble_area, vim, &theme);
        }

        // 2.5 Completion Menu (Floating)
        if vim.show_suggestions && !vim.filtered_suggestions.is_empty() {
            let menu_width = 45;
            let menu_height = std::cmp::min(10, vim.filtered_suggestions.len()) as u16 + 2;

            let menu_x = editor_layout[1].x + (cursor_screen_x % editor_width) as u16;
            let menu_y = editor_layout[1].y + cursor_screen_y_opt.unwrap_or(0) as u16 + 1;

            let menu_area = Rect {
                x: menu_x.min(area.right().saturating_sub(menu_width)),
                y: menu_y.min(
                    editor_trouble_chunks[0]
                        .bottom()
                        .saturating_sub(menu_height),
                ),
                width: menu_width,
                height: menu_height,
            };

            let items: Vec<ListItem> = vim
                .filtered_suggestions
                .iter()
                .enumerate()
                .map(|(display_idx, item)| {
                    let (icon, kind_name, color_group) = match item.kind {
                        Some(lsp_types::CompletionItemKind::FUNCTION) => {
                            (icons::FUNCTION.to_string(), "Function", "Function")
                        }
                        Some(lsp_types::CompletionItemKind::METHOD) => {
                            (icons::METHOD.to_string(), "Method", "Function")
                        }
                        Some(lsp_types::CompletionItemKind::VARIABLE) => {
                            (icons::VARIABLE.to_string(), "Variable", "Variable")
                        }
                        Some(lsp_types::CompletionItemKind::CLASS) => {
                            (icons::CLASS.to_string(), "Class", "Type")
                        }
                        Some(lsp_types::CompletionItemKind::INTERFACE) => {
                            (icons::INTERFACE.to_string(), "Interface", "Type")
                        }
                        Some(lsp_types::CompletionItemKind::KEYWORD) => {
                            (icons::KEYWORD.to_string(), "Keyword", "Keyword")
                        }
                        Some(lsp_types::CompletionItemKind::SNIPPET) => {
                            (icons::SNIPPET.to_string(), "Snippet", "Keyword")
                        }
                        Some(lsp_types::CompletionItemKind::FIELD) => {
                            (icons::FIELD.to_string(), "Field", "Identifier")
                        }
                        Some(lsp_types::CompletionItemKind::PROPERTY) => {
                            (icons::PROPERTY.to_string(), "Property", "Identifier")
                        }
                        Some(lsp_types::CompletionItemKind::TEXT) => {
                            (icons::TEXT.to_string(), "Text", "Comment")
                        }
                        _ => (icons::OBJECT.to_string(), "Object", "Constant"),
                    };

                    let mut label_style = theme.get("Normal");
                    let mut icon_style = theme.get(color_group);
                    let mut kind_style = theme.get("Comment");

                    if display_idx == (vim.selected_suggestion % vim.filtered_suggestions.len())
                    {
                        label_style = Style::default()
                            .fg(theme.palette.black)
                            .bg(theme.palette.blue)
                            .add_modifier(Modifier::BOLD);
                        icon_style = Style::default()
                            .fg(theme.palette.black)
                            .bg(theme.palette.blue);
                        kind_style = Style::default()
                            .fg(theme.palette.black)
                            .bg(theme.palette.blue);
                    }

                    ListItem::new(Line::from(vec![
                        Span::styled(format!(" {} ", icon), icon_style),
                        Span::styled(format!("{:<30}", item.label), label_style),
                        Span::styled(format!(" {:>8} ", kind_name), kind_style),
                    ]))
                })
                .collect();

            let menu = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.get("TreeExplorerConnector"))
                    .style(theme.get("Normal")),
            );

            frame.render_widget(Clear, menu_area);
            frame.render_stateful_widget(menu, menu_area, &mut vim.suggestion_state);

            // Floating Doc Window
            let selected_idx = vim.selected_suggestion % vim.filtered_suggestions.len();
            if let Some(item) = vim.filtered_suggestions.get(selected_idx) {
                if let Some(detail) = &item.detail {
                    let doc_width = 40;
                    let doc_height = menu_height;
                    let doc_x = if menu_area.right() + doc_width <= area.right() {
                        menu_area.right()
                    } else {
                        menu_area.left().saturating_sub(doc_width)
                    };
                    let doc_area = Rect {
                        x: doc_x,
                        y: menu_area.y,
                        width: doc_width,
                        height: doc_height,
                    };

                    let doc_text = detail.clone();
                    let doc_paragraph = Paragraph::new(doc_text)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Rounded)
                                .border_style(theme.get("TreeExplorerConnector"))
                                .style(theme.get("Normal")),
                        )
                        .wrap(ratatui::widgets::Wrap { trim: true });

                    frame.render_widget(Clear, doc_area);
                    frame.render_widget(doc_paragraph, doc_area);
                }
            }
        }

        // 2c. File preview popup (Shift+P)
        if let Some(ref preview_lines) = vim.preview_lines.clone() {
            let popup_width = (area.width as f32 * 0.6) as u16;
            let popup_height = (area.height as f32 * 0.7) as u16;
            let popup_area = Rect {
                x: (area.width - popup_width) / 2,
                y: (area.height - popup_height) / 2,
                width: popup_width,
                height: popup_height,
            };

            let block = Block::default()
                .title(" File Preview ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.get("TreeExplorerConnector"))
                .style(theme.get("Normal"));

            frame.render_widget(Clear, popup_area);
            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);

            let scroll = vim.preview_scroll;
            let items: Vec<Line> = preview_lines
                .iter()
                .skip(scroll)
                .take(inner.height as usize)
                .enumerate()
                .map(|(i, l)| {
                    let num = Span::styled(format!("{:>4} ", scroll + i + 1), theme.get("LineNr"));
                    let content = Span::styled(l.clone(), theme.get("Normal"));
                    Line::from(vec![num, content])
                })
                .collect();
            let para = Paragraph::new(items).style(theme.get("Normal"));
            frame.render_widget(para, inner);
        }

        // 3. Status Line
        let (mode_style, mode_label) = match vim.mode {
            Mode::Normal => (theme.get("StatusLineNormal"), " NORMAL "),
            Mode::Insert => (theme.get("StatusLineInsert"), " INSERT "),
            Mode::Visual | Mode::VisualBlock => (theme.get("StatusLineVisual"), if matches!(vim.mode, Mode::Visual) { " VISUAL " } else { " V-BLOCK " }),
            Mode::BlockInsert => (theme.get("StatusLineInsert"), " BLOCK INSERT "),
            Mode::Command => (theme.get("StatusLineCommand"), " COMMAND "),
            _ => (theme.get("StatusLineA"), " ATOM "),
        };

        let mut git_spans = Vec::new();
        if let Some(git) = &vim.git_info {
            git_spans.push(Span::styled(format!(" \u{e0a0} {} ", git.branch), theme.get("StatusLineB")));
            if buf_added > 0 {
                git_spans.push(Span::styled(format!("+{} ", buf_added), theme.get("GitSignsAdd")));
            }
            if buf_modified > 0 {
                git_spans.push(Span::styled(format!("~{} ", buf_modified), theme.get("GitSignsChange")));
            }
            if buf_removed > 0 {
                git_spans.push(Span::styled(format!("-{} ", buf_removed), theme.get("GitSignsDelete")));
            }
        }

        let lsp_status_text = match &vim.lsp_status {
            crate::vim::LspStatus::None => "No LSP",
            crate::vim::LspStatus::Loading => "LSP...",
            crate::vim::LspStatus::Ready => "LSP",
            crate::vim::LspStatus::Installing => "Inst...",
            crate::vim::LspStatus::Formatting => "Fmt...",
            crate::vim::LspStatus::Error(_) => "LSP Err",
        };

        let pos_text = format!("{}:{}", cursor_y + 1, cursor_x + 1);

        let mut left_spans = vec![
            Span::styled(mode_label, mode_style),
        ];
        left_spans.extend(git_spans);
        left_spans.push(Span::styled(format!(" {} ", file_name), theme.get("StatusLineC").add_modifier(Modifier::BOLD)));
        left_spans.push(Span::styled(modified_flag, theme.get("Keyword")));

        let left_part = Line::from(left_spans);

        let right_part = Line::from(vec![
            Span::styled(format!(" {} ", lsp_status_text), theme.get("StatusLineB")),
            Span::styled(format!(" {} ", pos_text), theme.get("StatusLineA")),
        ]);

        // Pre-fill the statusline background once. Each Paragraph::style() call patches its
        // style onto ALL cells in the area, so a second render overwrites fg colors set by
        // the first. By pre-filling and rendering both without a base style, span colors
        // (green/yellow/red for git signs) are preserved.
        frame.render_widget(Block::default().style(theme.get("StatusLine")), root_chunks[1]);
        frame.render_widget(Paragraph::new(left_part), root_chunks[1]);
        frame.render_widget(Paragraph::new(right_part).alignment(Alignment::Right), root_chunks[1]);

        // 4. Command Line / Message Area
        match vim.mode {
            Mode::Command => {
                let prompt = format!(":{}", vim.command_buffer);
                frame.render_widget(
                    Paragraph::new(prompt.as_str()).style(theme.get("Normal")),
                    root_chunks[2],
                );
                frame.set_cursor_position((
                    root_chunks[2].x + prompt.chars().count() as u16,
                    root_chunks[2].y,
                ));

                // Wildmenu: render suggestions popup above the command line (only after Tab)
                if vim.command_wildmenu_open && !vim.command_suggestions.is_empty() {
                    let max_visible: u16 = 15.min(vim.command_suggestions.len() as u16);
                    let popup_w = (vim.command_suggestions.iter().map(|s| s.len()).max().unwrap_or(4) as u16 + 8)
                        .max(24)
                        .min(area.width / 2);
                    let popup_h = max_visible;
                    let popup_y = root_chunks[2].y.saturating_sub(popup_h);

                    let popup_rect = Rect {
                        x: root_chunks[2].x,
                        y: popup_y,
                        width: popup_w,
                        height: popup_h,
                    };

                    // Scroll window so selected item is visible
                    let sel = vim.selected_command_suggestion;
                    let scroll_offset = if sel >= max_visible as usize {
                        sel + 1 - max_visible as usize
                    } else {
                        0
                    };

                    let items: Vec<ListItem> = vim.command_suggestions
                        .iter()
                        .enumerate()
                        .skip(scroll_offset)
                        .take(max_visible as usize)
                        .map(|(i, s)| {
                            let style = if i == sel {
                                theme.get("PmenuSel")
                            } else {
                                theme.get("Pmenu")
                            };
                            ListItem::new(format!("  {}", s)).style(style)
                        })
                        .collect();

                    frame.render_widget(Clear, popup_rect);
                    frame.render_widget(
                        List::new(items).style(theme.get("Pmenu")),
                        popup_rect,
                    );
                }
            }
            Mode::Search => {
                let prompt = format!("/{}", vim.input_buffer);
                frame.render_widget(
                    Paragraph::new(prompt.as_str()).style(theme.get("Normal")),
                    root_chunks[2],
                );
                frame.set_cursor_position((
                    root_chunks[2].x + prompt.chars().count() as u16,
                    root_chunks[2].y,
                ));
            }
            Mode::ExplorerInput(input_type) => {
                let prompt = match input_type {
                    crate::vim::mode::ExplorerInputType::Add => "New File Name: ",
                    crate::vim::mode::ExplorerInputType::Rename => "Rename To: ",
                    crate::vim::mode::ExplorerInputType::Move => "Move To: ",
                    crate::vim::mode::ExplorerInputType::DeleteConfirm => "Delete selected? (y/n): ",
                    crate::vim::mode::ExplorerInputType::Filter => "",
                };
                if !prompt.is_empty() {
                    let display = format!("{}{}", prompt, vim.input_buffer);
                    frame.render_widget(
                        Paragraph::new(display).style(theme.get("Keyword")),
                        root_chunks[2],
                    );
                    frame.set_cursor_position((
                        root_chunks[2].x + (prompt.chars().count() + vim.input_buffer.chars().count()) as u16,
                        root_chunks[2].y,
                    ));
                } else {
                    frame.render_widget(
                        Paragraph::new("").style(theme.get("Normal")),
                        root_chunks[2],
                    );
                }
            }
            Mode::Confirm(action) => {
                let prompt = match action {
                    crate::vim::mode::ConfirmAction::Quit => {
                        "Unsaved changes! Quit? [Y]es (Save), [N]o (Discard), [C]ancel: "
                    }
                    crate::vim::mode::ConfirmAction::CloseBuffer => {
                        "Unsaved changes! Close buffer? [Y]es (Save), [N]o (Discard), [C]ancel: "
                    }
                    crate::vim::mode::ConfirmAction::ReloadFile => {
                        "W11 Warning: File changed on disk. [L]oad, [I]gnore: "
                    }
                };
                frame.render_widget(
                    Paragraph::new(prompt).style(theme.get("Keyword")),
                    root_chunks[2],
                );
                frame.set_cursor_position((
                    root_chunks[2].x + prompt.chars().count() as u16,
                    root_chunks[2].y,
                ));
            }
            _ => {
                if let Some(msg) = &vim.message {
                    frame.render_widget(
                        Paragraph::new(msg.as_str()).style(theme.get("String")),
                        root_chunks[2],
                    );
                } else {
                    frame.render_widget(
                        Paragraph::new("").style(theme.get("Normal")),
                        root_chunks[2],
                    );
                }
                if !vim.show_intro && vim.focus == Focus::Editor && cursor_y < buffer.len_lines() {
                    if let Some(y) = cursor_screen_y_opt {
                        if y < editor_layout[1].height as usize {
                            frame.set_cursor_position((
                                editor_layout[1].x + cursor_screen_x as u16,
                                editor_layout[1].y + y as u16,
                            ));
                        }
                    }
                }
            }
        }

        if let Mode::Nucleus = vim.mode {
            self.draw_nucleus(frame, editor, lsp_manager, &theme, vim);
        }

        if let Mode::Keymaps = vim.mode {
            self.draw_keymaps(frame, vim, &theme);
        }

        if vim.telescope.visible {
            vim.telescope.draw(frame, &theme, vim, editor);
        }
    }

    pub fn draw_editor_pane(
        &self,
        frame: &mut Frame,
        editor: &mut crate::editor::Editor,
        vim: &mut crate::vim::VimState,
        lsp_manager: &crate::lsp::LspManager,
        pane_area: Rect,
        buf_idx: usize,
        is_focused: bool,
    ) {
        let theme = editor.highlighter.theme.clone();
        let (buffer, cursor_y, cursor_x, cursor_scroll_y) = {
            let buffer = match editor.buffers.get(buf_idx) {
                Some(b) => b,
                None => return,
            };
            let cursor = match editor.cursors.get(buf_idx) {
                Some(c) => c,
                None => return,
            };
            (buffer.clone(), cursor.y, cursor.x, cursor.scroll_y)
        };
        let visible_height = pane_area.height as usize;

        let gutter_width = match (
            vim.show_number || vim.relative_number,
            vim.config.signcolumn,
        ) {
            (true, true) => 7,
            (true, false) => 5,
            (false, true) => 3,
            (false, false) => 0,
        };

        let editor_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(gutter_width), Constraint::Min(1)])
            .split(pane_area);

        let editor_width = editor_layout[1].width.max(1) as usize;
        let screen_to_buffer_lines =
            editor.get_screen_to_buffer_lines_for_idx(buf_idx, editor_width, vim.config.wrap);

        // Find cursor screen y
        let cursor_screen_y = screen_to_buffer_lines
            .iter()
            .position(|&(idx, row)| {
                if idx != cursor_y {
                    return false;
                }
                if !vim.config.wrap {
                    return true;
                }
                let mut cursor_pos_in_line = 0;
                if let Some(line) = buffer.line(idx) {
                    for (i, _) in line.chars().enumerate() {
                        if i >= cursor_x {
                            break;
                        }
                        let c = line.char(i);
                        cursor_pos_in_line += if c == '\t' {
                            2
                        } else {
                            unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
                        };
                    }
                }
                let cursor_row = cursor_pos_in_line / editor_width;
                row == cursor_row
            })
            .map(|pos| pos.saturating_sub(cursor_scroll_y));

        // Full width highlight for active line
        if vim.config.cursorline && is_focused {
            if let Some(y) = cursor_screen_y {
                if y < visible_height {
                    let highlight_rect = Rect {
                        x: pane_area.x,
                        y: pane_area.y + y as u16,
                        width: pane_area.width,
                        height: 1,
                    };
                    frame.render_widget(
                        Block::default().style(theme.get("CursorLine")),
                        highlight_rect,
                    );
                }
            }
        }

        // Line Numbers & Gutter
        if gutter_width > 0 {
            let mut line_numbers = Text::default();
            for i in
                cursor_scroll_y..std::cmp::min(cursor_scroll_y + visible_height, screen_to_buffer_lines.len())
            {
                let (actual_idx, row) = screen_to_buffer_lines[i];
                let is_first_row = row == 0;

                if is_first_row {
                    let gutter_text = if vim.relative_number {
                        let rel = if actual_idx == cursor_y {
                            actual_idx + 1
                        } else {
                            (actual_idx as i32 - cursor_y as i32).abs() as usize
                        };
                        format!("{:>4} ", rel)
                    } else {
                        format!("{:>4} ", actual_idx + 1)
                    };

                    let mut style = theme.get("LineNr");
                    if actual_idx == cursor_y {
                        style = theme.get("CursorLineNr");
                    }

                    // Git signs
                    if vim.config.signcolumn {
                        let sign = buffer
                            .git_signs
                            .iter()
                            .find(|(l, _)| *l == actual_idx)
                            .map(|(_, s)| s);
                        let (sign_char, sign_style) = match sign {
                            Some(crate::git::GitSign::Add) => ("+", theme.get("GitSignsAdd")),
                            Some(crate::git::GitSign::Change) => ("~", theme.get("GitSignsChange")),
                            Some(crate::git::GitSign::ChangeDelete) => ("~", theme.get("GitSignsChange")),
                            Some(crate::git::GitSign::Delete) | Some(crate::git::GitSign::TopDelete) => ("-", theme.get("GitSignsDelete")),
                            _ => (" ", theme.get("Normal")),
                        };
                        line_numbers.lines.push(Line::from(vec![
                            Span::styled(gutter_text, style),
                            Span::styled(format!("{} ", sign_char), sign_style),
                        ]));
                    } else {
                        line_numbers.lines.push(Line::from(vec![Span::styled(gutter_text, style)]));
                    }
                } else {
                    line_numbers.lines.push(Line::from("      "));
                }
            }
            frame.render_widget(Paragraph::new(line_numbers).style(theme.get("Normal")), editor_layout[0]);
        }

        // Buffer Content
        let mut text = Text::default();
        let search_query = vim.search_query.clone();

        // Get syntax styles from cache
        editor.refresh_syntax_for_idx(buf_idx);
        let syntax_styles = &editor.caches[buf_idx].syntax_styles;

        for i in cursor_scroll_y..std::cmp::min(cursor_scroll_y + visible_height, screen_to_buffer_lines.len()) {
            let (actual_idx, row) = screen_to_buffer_lines[i];
            let is_current_line = actual_idx == cursor_y;

            if let Some(line) = buffer.line(actual_idx) {
                let mut spans = Vec::new();
                let mut current_pos_in_line = 0;
                for (x, c) in line.chars().enumerate() {
                    if c == '\n' || c == '\r' {
                        continue;
                    }
                    let char_width = if c == '\t' {
                        2
                    } else {
                        unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
                    };
                    let char_row = current_pos_in_line / editor_width;

                    if char_row == row {
                        let mut style = syntax_styles
                            .get(actual_idx)
                            .and_then(|s| s.get(x))
                            .copied()
                            .unwrap_or(theme.get("Normal"));

                        // Overlay Highlights
                        let mut is_in_range = false;
                        if let Some(start) = vim.selection_start {
                            let cur = crate::vim::Position {
                                x: cursor_x,
                                y: cursor_y,
                            };
                            match vim.mode {
                                Mode::VisualBlock | Mode::BlockInsert => {
                                    // Block selection: highlight rectangular region
                                    let top_y = start.y.min(cur.y);
                                    let bot_y = start.y.max(cur.y);
                                    let left_x = start.x.min(cur.x);
                                    let right_x = start.x.max(cur.x);
                                    is_in_range = actual_idx >= top_y
                                        && actual_idx <= bot_y
                                        && x >= left_x
                                        && x <= right_x;
                                }
                                _ => {
                                    // Character-wise selection
                                    let (s_y, s_x, e_y, e_x) = if (start.y, start.x) < (cur.y, cur.x)
                                    {
                                        (start.y, start.x, cur.y, cur.x)
                                    } else {
                                        (cur.y, cur.x, start.y, start.x)
                                    };
                                    is_in_range = if actual_idx > s_y && actual_idx < e_y {
                                        true
                                    } else if actual_idx == s_y && actual_idx == e_y {
                                        x >= s_x && x <= e_x
                                    } else if actual_idx == s_y {
                                        x >= s_x
                                    } else if actual_idx == e_y {
                                        x <= e_x
                                    } else {
                                        false
                                    };
                                }
                            }
                            if is_in_range {
                                style = theme.get("Visual");
                            }
                        }
                        if !search_query.is_empty() {
                            let line_str = line.to_string();
                            if let Some(pos) =
                                line_str.to_lowercase().find(&search_query.to_lowercase())
                            {
                                if x >= pos && x < pos + search_query.len() {
                                    style = theme.get("Search");
                                }
                            }
                        }
                        if vim.yank_highlight_line == Some(actual_idx) {
                            style = Style::default()
                                .bg(theme.palette.blue)
                                .fg(theme.palette.black);
                        }

                        // Diagnostics undercurl/underline
                        if let Some(path) = &buffer.file_path {
                            if let Ok(url) = lsp_types::Url::from_file_path(path) {
                                let diagnostics_lock = lsp_manager.diagnostics.lock().unwrap();
                                if let Some(server_diags) = diagnostics_lock.get(&url) {
                                    for diags in server_diags.values() {
                                        for diag in diags {
                                            if (actual_idx as u32) >= diag.range.start.line
                                                && (actual_idx as u32) <= diag.range.end.line
                                            {
                                                let s_x = if (actual_idx as u32)
                                                    == diag.range.start.line
                                                {
                                                    diag.range.start.character as usize
                                                } else {
                                                    0
                                                };
                                                let e_x = if (actual_idx as u32)
                                                    == diag.range.end.line
                                                {
                                                    diag.range.end.character as usize
                                                } else {
                                                    line.len_chars()
                                                };
                                                if x >= s_x && x < e_x {
                                                    let diag_color = match diag.severity {
                                                        Some(lsp_types::DiagnosticSeverity::ERROR) => {
                                                            theme.palette.red
                                                        }
                                                        Some(
                                                            lsp_types::DiagnosticSeverity::WARNING,
                                                        ) => theme.palette.yellow,
                                                        _ => theme.palette.blue,
                                                    };
                                                    style = style
                                                        .underline_color(diag_color)
                                                        .add_modifier(Modifier::UNDERLINED);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if is_current_line
                            && cursor_screen_y == Some(i.saturating_sub(cursor_scroll_y))
                            && !is_in_range
                        {
                            style.bg = None;
                        }

                        if c == '\t' {
                            for _ in 0..2 {
                                spans.push(Span::styled(" ", style));
                            }
                        } else {
                            let line_str = line.to_string();
                            let is_indent_pos = x > 0
                                && x % 2 == 0
                                && x < line_str.chars().take_while(|&c| c == ' ').count();
                            if is_indent_pos {
                                let mut indent_style =
                                    theme.get("Comment").add_modifier(Modifier::DIM);
                                if is_current_line
                                    && cursor_screen_y == Some(i.saturating_sub(cursor_scroll_y))
                                {
                                    indent_style.bg = None;
                                }
                                spans.push(Span::styled("┆", indent_style));
                            } else {
                                spans.push(Span::styled(c.to_string(), style));
                            }
                        }
                    }
                    current_pos_in_line += char_width;
                }

                // V-Block: show highlight on empty/short lines that have no chars in the range
                if matches!(vim.mode, Mode::VisualBlock | Mode::BlockInsert) {
                    if let Some(start) = vim.selection_start {
                        let cur = crate::vim::Position { x: cursor_x, y: cursor_y };
                        let top_y = start.y.min(cur.y);
                        let bot_y = start.y.max(cur.y);
                        let left_x = start.x.min(cur.x);
                        if actual_idx >= top_y && actual_idx <= bot_y && row == 0 {
                            let line_len = line.chars().filter(|&c| c != '\n' && c != '\r').count();
                            if line_len <= left_x {
                                // Pad up to left_x then render one highlighted space
                                let pad = left_x.saturating_sub(line_len);
                                if pad > 0 {
                                    spans.push(Span::styled(" ".repeat(pad), theme.get("Normal")));
                                }
                                spans.push(Span::styled(" ", theme.get("Visual")));
                            }
                        }
                    }
                }

                // Fill the rest of the line with CursorLine if active
                if is_current_line && cursor_screen_y == Some(i.saturating_sub(cursor_scroll_y)) {
                    let current_width = spans.iter().map(|s| s.width()).sum::<usize>();
                    if current_width < editor_width {
                        spans.push(Span::styled(
                            " ".repeat(editor_width - current_width),
                            theme.get("CursorLine"),
                        ));
                    }
                }

                let mut line_obj = Line::from(spans);
                if is_current_line && cursor_screen_y == Some(i.saturating_sub(cursor_scroll_y)) {
                    line_obj = line_obj.style(theme.get("CursorLine"));
                }

                // Diagnostic Virtual Text
                if row == 0 && vim.show_diagnostics {
                    if let Some(path) = &buffer.file_path {
                        if let Ok(url) = lsp_types::Url::from_file_path(path) {
                            let diags_lock = lsp_manager.diagnostics.lock().unwrap();
                            if let Some(server_diags) = diags_lock.get(&url) {
                                let mut line_diags = Vec::new();
                                for diags in server_diags.values() {
                                    for diag in diags {
                                        if diag.range.start.line as usize == actual_idx {
                                            line_diags.push(diag);
                                        }
                                    }
                                }
                                line_diags.sort_by_key(|d| d.severity);
                                for (idx, diag) in line_diags.iter().enumerate() {
                                    let (diag_icon, diag_color) = match diag.severity {
                                        Some(lsp_types::DiagnosticSeverity::ERROR) => ("■", theme.palette.red),
                                        Some(lsp_types::DiagnosticSeverity::WARNING) => ("▲", theme.palette.yellow),
                                        _ => ("●", theme.palette.blue),
                                    };
                                    let mut msg = diag.message.clone();
                                    if let Some(code) = &diag.code {
                                        let code_str = match code {
                                            lsp_types::NumberOrString::Number(n) => n.to_string(),
                                            lsp_types::NumberOrString::String(s) => s.clone(),
                                        };
                                        msg = format!("{} [{}]", msg, code_str);
                                    }
                                    if idx == 0 {
                                        line_obj.spans.push(Span::raw("    "));
                                    } else {
                                        line_obj.spans.push(Span::raw(", "));
                                    }
                                    line_obj.spans.push(Span::styled(
                                        format!("{} ", diag_icon),
                                        Style::default().fg(diag_color),
                                    ));
                                    line_obj.spans.push(Span::styled(
                                        msg,
                                        Style::default()
                                            .fg(theme.palette.grey_fg)
                                            .add_modifier(Modifier::ITALIC),
                                    ));
                                }
                            }
                        }
                    }
                }

                text.lines.push(line_obj);
            }
        }
        frame.render_widget(Paragraph::new(text), editor_layout[1]);

        // Blame Popup
        if let Some(blame) = &vim.blame_popup {
            let popup_width = (blame.len() as u16) + 4;
            let popup_height = 3;

            if let Some(y) = cursor_screen_y {
                let mut cursor_pos_in_line = 0;
                if let Some(line) = buffer.line(cursor_y) {
                    for (i, _) in line.chars().enumerate() {
                        if i >= cursor_x { break; }
                        let c = line.char(i);
                        cursor_pos_in_line += if c == '\t' { 2 } else {
                            unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
                        };
                    }
                }
                let screen_x = (cursor_pos_in_line % editor_width) as u16;

                let x = (editor_layout[1].x + screen_x).min(pane_area.right().saturating_sub(popup_width));
                let screen_y = (editor_layout[1].y + y as u16).saturating_sub(popup_height);

                let popup_area = Rect {
                    x,
                    y: screen_y,
                    width: popup_width,
                    height: popup_height,
                };

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(theme.get("TreeExplorerConnector"))
                    .style(theme.get("Normal"));

                frame.render_widget(Clear, popup_area);
                frame.render_widget(
                    Paragraph::new(blame.clone())
                        .block(block)
                        .alignment(Alignment::Center),
                    popup_area,
                );
            }
        }

        // Hover Popup (K / lsp.buf.hover) — with syntax highlighting
        if let Some(hover_text) = &vim.hover_popup.clone() {
            let lang_name = buffer.file_path.as_ref()
                .and_then(|p| p.extension()).and_then(|s| s.to_str())
                .map(|ext| match ext {
                    "rs" => "rust", "ts" => "typescript", "tsx" => "tsx",
                    "js" | "jsx" => "javascript", "py" => "python",
                    "go" => "go", "lua" => "lua", "json" => "json",
                    "toml" => "toml", "html" => "html", "css" => "css",
                    _ => "rust",
                })
                .unwrap_or("rust");
            Self::draw_highlighted_popup(frame, hover_text, " Hover ", pane_area, cursor_screen_y, &editor_layout, editor_width, cursor_y, cursor_x, &buffer, &theme, &editor.highlighter, lang_name);
        }

        // Diagnostic Float (D / vim.diagnostic.open_float)
        if let Some(diag_text) = &vim.diagnostic_popup {
            Self::draw_float_popup(frame, diag_text, " Diagnostics ", pane_area, cursor_screen_y, &editor_layout, editor_width, cursor_y, cursor_x, &buffer, &theme);
        }

    }
}

impl TerminalUi {
    fn draw_float_popup(
        frame: &mut Frame,
        content: &str,
        title: &str,
        pane_area: Rect,
        cursor_screen_y: Option<usize>,
        editor_layout: &[Rect],
        editor_width: usize,
        cursor_y: usize,
        cursor_x: usize,
        buffer: &crate::editor::buffer::Buffer,
        theme: &crate::ui::colorscheme::ColorScheme,
    ) {
        let lines: Vec<&str> = content.lines().collect();
        let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(4);
        let popup_width = (max_line_len as u16 + 4).min(pane_area.width.saturating_sub(2)).max(20);
        let popup_height = (lines.len() as u16 + 2).min(pane_area.height / 2).max(3);

        let mut cursor_pos_in_line = 0usize;
        if let Some(line) = buffer.line(cursor_y) {
            for (i, _) in line.chars().enumerate() {
                if i >= cursor_x { break; }
                let c = line.char(i);
                cursor_pos_in_line += if c == '\t' { 2 } else {
                    unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
                };
            }
        }
        let screen_x = (cursor_pos_in_line % editor_width) as u16;
        let base_x = (editor_layout[1].x + screen_x).min(pane_area.right().saturating_sub(popup_width));

        let base_y = if let Some(sy) = cursor_screen_y {
            let row = editor_layout[1].y + sy as u16;
            if row + popup_height + 1 < pane_area.bottom() {
                row + 1
            } else {
                row.saturating_sub(popup_height)
            }
        } else {
            editor_layout[1].y
        };

        let popup_area = Rect {
            x: base_x,
            y: base_y,
            width: popup_width,
            height: popup_height,
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(title, theme.get("Comment")))
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(content.to_string())
                .block(block)
                .wrap(ratatui::widgets::Wrap { trim: false }),
            popup_area,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_highlighted_popup(
        frame: &mut Frame,
        content: &str,
        title: &str,
        pane_area: Rect,
        cursor_screen_y: Option<usize>,
        editor_layout: &[Rect],
        editor_width: usize,
        cursor_y: usize,
        cursor_x: usize,
        buffer: &crate::editor::buffer::Buffer,
        theme: &crate::ui::colorscheme::ColorScheme,
        highlighter: &crate::editor::highlighter::Highlighter,
        lang_hint: &str,
    ) {
        let raw_lines: Vec<&str> = content.lines().collect();
        let max_line_len = raw_lines.iter().map(|l| l.len()).max().unwrap_or(4);
        let popup_width = (max_line_len as u16 + 4).min(pane_area.width.saturating_sub(2)).max(20);
        let popup_height = (raw_lines.len() as u16 + 2).min(pane_area.height / 2).max(3);

        let mut cursor_pos_in_line = 0usize;
        if let Some(line) = buffer.line(cursor_y) {
            for (i, _) in line.chars().enumerate() {
                if i >= cursor_x { break; }
                let c = line.char(i);
                cursor_pos_in_line += if c == '\t' { 2 } else {
                    unicode_width::UnicodeWidthChar::width(c).unwrap_or(1)
                };
            }
        }
        let screen_x = (cursor_pos_in_line % editor_width) as u16;
        let base_x = (editor_layout[1].x + screen_x).min(pane_area.right().saturating_sub(popup_width));
        let base_y = if let Some(sy) = cursor_screen_y {
            let row = editor_layout[1].y + sy as u16;
            if row + popup_height + 1 < pane_area.bottom() { row + 1 } else { row.saturating_sub(popup_height) }
        } else {
            editor_layout[1].y
        };

        let popup_area = Rect { x: base_x, y: base_y, width: popup_width, height: popup_height };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(title, theme.get("Comment")))
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));

        // Build styled lines
        let mut styled_lines: Vec<Line> = Vec::new();
        let mut in_code_fence = false;
        let mut _fence_lang = lang_hint;

        for raw in &raw_lines {
            if raw.starts_with("```") {
                let detected = raw.trim_start_matches('`').trim();
                _fence_lang = if detected.is_empty() { lang_hint } else {
                    match detected {
                        "rust" | "rs" => "rust",
                        "typescript" | "ts" => "typescript",
                        "javascript" | "js" => "javascript",
                        "python" | "py" => "python",
                        "go" => "go",
                        "lua" => "lua",
                        "json" => "json",
                        "toml" => "toml",
                        "html" => "html",
                        "css" => "css",
                        _ => lang_hint,
                    }
                };
                in_code_fence = !in_code_fence;
                continue;
            }

            if in_code_fence || lang_hint == "diff" {
                // Apply syntax highlighting to code / diff lines
                if lang_hint == "diff" {
                    if raw.starts_with("+ ") || raw.starts_with("- ") {
                        let is_add = raw.starts_with("+ ");
                        let content = &raw[2..];
                        let (fg, bg) = if is_add {
                            let c = theme.palette.green;
                            let dim = match c {
                                ratatui::style::Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(r / 6, g / 5, b / 6),
                                other => other,
                            };
                            (c, dim)
                        } else {
                            let c = theme.palette.red;
                            let dim = match c {
                                ratatui::style::Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(r / 5, g / 9, b / 6),
                                other => other,
                            };
                            (c, dim)
                        };
                        let line_style = Style::default().fg(fg).bg(bg);
                        // Pad to max_line_len so the background fills the popup width
                        let padded = format!("{:<width$}", content, width = max_line_len.saturating_sub(2));
                        styled_lines.push(Line::from(Span::styled(padded, line_style)));
                    } else {
                        styled_lines.push(Line::from(Span::styled(raw.to_string(), theme.get("Comment"))));
                    }
                } else {
                    let styles = highlighter.highlight_line(raw);
                    let chars: Vec<char> = raw.chars().collect();
                    let spans: Vec<Span> = chars.iter().enumerate().map(|(i, ch)| {
                        Span::styled(ch.to_string(), styles.get(i).copied().unwrap_or(theme.get("Normal")))
                    }).collect();
                    styled_lines.push(Line::from(spans));
                }
            } else {
                // Non-code documentation lines: dim them slightly
                styled_lines.push(Line::from(Span::styled(raw.to_string(), theme.get("Comment"))));
            }
        }

        frame.render_widget(Clear, popup_area);
        frame.render_widget(
            Paragraph::new(Text::from(styled_lines))
                .block(block)
                .wrap(ratatui::widgets::Wrap { trim: false }),
            popup_area,
        );
    }
}
