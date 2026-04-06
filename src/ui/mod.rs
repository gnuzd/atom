pub mod colorscheme;
pub mod explorer;
pub mod icons;
pub mod telescope;
pub mod trouble;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect, Margin},
    style::{Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, BorderType, List, ListItem, Padding, Paragraph, Clear},
    Frame,
};
use lsp_types::DiagnosticSeverity;
use crate::vim::mode::{Mode, ExplorerInputType, Focus};
use crate::vim::LspStatus;

pub struct TerminalUi;

impl TerminalUi {
    pub fn new() -> Self {
        Self
    }

    pub fn get_file_icon(path: &std::path::Path) -> (&'static str, String) {
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
        vim: &mut crate::vim::VimState,
    ) {
        let area = frame.area();
        let mason_width = (area.width as f32 * 0.8) as u16;
        let mason_height = (area.height as f32 * 0.8) as u16;
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
                Constraint::Length(1), // Tabs
                Constraint::Length(2), // Divider/Filter
                Constraint::Min(1),    // List
                Constraint::Length(1), // Help
            ])
            .split(inner_area);

        // 1. Tabs
        let tabs = ["(1) All", "(2) LSP", "(3) DAP", "(4) Linter", "(5) Formatter"];
        let mut tab_spans = Vec::new();
        for (i, tab) in tabs.iter().enumerate() {
            let style = if i == vim.mason_tab {
                Style::default().fg(theme.palette.black).bg(theme.palette.orange).add_modifier(Modifier::BOLD)
            } else {
                theme.get("Comment")
            };
            tab_spans.push(Span::styled(format!(" {} ", tab), style));
            tab_spans.push(Span::raw("  "));
        }
        frame.render_widget(Paragraph::new(Line::from(tab_spans)), chunks[0]);

        // 2. Filter
        let filter_prompt = "Language Filter: ";
        let filter_text = if let Mode::MasonFilter = vim.mode {
            format!("{}{}", filter_prompt, vim.mason_filter)
        } else if vim.mason_filter.is_empty() {
            "Language Filter: press <C-f> to apply filter".to_string()
        } else {
            format!("{}{}", filter_prompt, vim.mason_filter)
        };
        
        frame.render_widget(Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![Span::styled(filter_text, theme.get("Comment"))])
        ]), chunks[1]);

        if let Mode::MasonFilter = vim.mode {
            frame.set_cursor_position((
                chunks[1].x + filter_prompt.len() as u16 + vim.mason_filter.len() as u16,
                chunks[1].y + 1,
            ));
        }

        // 3. Package List
        let packages: Vec<&crate::lsp::Package> = crate::lsp::PACKAGES.iter()
            .filter(|p| {
                let matches_tab = match vim.mason_tab {
                    0 => true,
                    1 => p.kind == crate::lsp::PackageKind::Lsp,
                    2 => p.kind == crate::lsp::PackageKind::Dap,
                    3 => p.kind == crate::lsp::PackageKind::Linter,
                    4 => p.kind == crate::lsp::PackageKind::Formatter,
                    _ => true,
                };
                let matches_filter = p.name.to_lowercase().contains(&vim.mason_filter.to_lowercase()) || 
                                   p.description.to_lowercase().contains(&vim.mason_filter.to_lowercase());
                matches_tab && matches_filter
            })
            .collect();

        let (mut installed, mut available): (Vec<_>, Vec<_>) = packages.into_iter().partition(|p| lsp_manager.is_managed(p.cmd));
        installed.sort_by_key(|p| p.name);
        available.sort_by_key(|p| p.name);

        let mut items = Vec::new();
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("Installed ({})", installed.len()), Style::default().add_modifier(Modifier::BOLD).fg(theme.palette.orange))
        ])));

        let installing_set = lsp_manager.installing.lock().unwrap();

        for p in &installed {
            let mut spans = vec![
                Span::styled(" ● ", theme.get("String")),
                Span::styled(format!("{:<30} ", p.name), theme.get("Keyword")),
                Span::styled(p.cmd, theme.get("Comment")),
            ];
            if installing_set.contains(p.cmd) {
                spans.push(Span::styled(" (installing...)", theme.get("Type")));
            }
            items.push(ListItem::new(Line::from(spans)));
        }

        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("Available ({})", available.len()), Style::default().add_modifier(Modifier::BOLD).fg(theme.palette.blue))
        ])));

        for p in &available {
            let mut spans = vec![
                Span::styled(" ○ ", theme.get("Comment")),
                Span::styled(format!("{:<30} ", p.name), theme.get("Normal")),
                Span::styled(p.description, theme.get("Comment")),
            ];
            if installing_set.contains(p.cmd) {
                spans.push(Span::styled(" (installing...)", theme.get("Type")));
            }
            items.push(ListItem::new(Line::from(spans)));
        }

        let list = List::new(items)
            .highlight_style(theme.get("CursorLine"))
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, chunks[2], &mut vim.mason_state);

        // 4. Help / Status
        let mut help_spans = vec![
            Span::styled(" i: install  u: update  x: uninstall  q: close ", theme.get("Comment"))
        ];

        if !installing_set.is_empty() {
            let pkg = installing_set.iter().next().unwrap();
            help_spans.push(Span::styled(format!("  {} Installing {}... ", vim.get_spinner(), pkg), theme.get("Keyword")));
        }
        
        frame.render_widget(Paragraph::new(Line::from(help_spans)).alignment(Alignment::Center), chunks[3]);
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
            ("[num]j/k", "Jump lines"),
            ("w/b/e", "Word movement"),
            ("u", "Undo"),
            ("<C-r>", "Redo"),
            ("<C-s>", "Save & Format"),
            ("<Space>n", "Toggle Relative Num"),
            ("<Space>/", "Toggle Comment"),
            ("zc / za", "Fold / Unfold"),
            ("<Space>b", "Toggle Autoformat"),
            ("dd", "Delete line"),
            ("yy", "Yank line"),
            ("p/P", "Paste after/before"),
            ("o/O", "Open line below/above"),
            ("\\", "Toggle Explorer"),
            ("<Space>ff", "Telescope Files"),
            ("<Space>fg", "Telescope Grep"),
            ("<Space>fb", "Telescope Buffers"),
            ("<Space>tt", "Toggle Trouble"),
            ("<Space>th", "Theme Picker"),
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
            ("<C-s>", "Save & Format"),
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
            ("o", "Open in System Explorer"),
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

        // Command Mode
        items.push(ListItem::new(Line::from(vec![Span::styled("--- COMMAND ---", header_style)])));
        let command_keys = [
            (":w", "Save & Format"),
            (":Format", "Trigger Format"),
            (":FormatEnable", "Enable Autoformat"),
            (":FormatDisable", "Disable Autoformat"),
            (":q", "Quit/Close"),
            (":Mason", "LSP Manager"),
            (":bn/bp", "Next/Prev Buffer"),
        ];
        for (k, d) in command_keys {
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<12}", k), key_style),
                Span::styled(" - ", theme.get("Comment")),
                Span::styled(d, desc_style),
            ])));
        }

        items.push(ListItem::new(Line::from("")));

        // Telescope
        items.push(ListItem::new(Line::from(vec![Span::styled("--- TELESCOPE ---", header_style)])));
        let telescope_keys = [
            ("<Space>ff", "Find Files"),
            ("<Space>fg", "Live Grep"),
            ("<Space>fb", "Select Buffer"),
            ("<Esc>", "Close Telescope"),
            ("<Enter>", "Open Selected"),
            ("j/k/Tab", "Navigate"),
            ("<C-u/d>", "Scroll Preview"),
        ];
        for (k, d) in telescope_keys {
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
        trouble: &trouble::TroubleList,
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

        // Further split main_chunks[1] if trouble is visible
        let editor_trouble_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if trouble.visible {
                [Constraint::Percentage(70), Constraint::Percentage(30)]
            } else {
                [Constraint::Percentage(100), Constraint::Percentage(0)]
            })
            .split(main_chunks[1]);

        let editor_area = editor_trouble_chunks[0];
        let trouble_area = editor_trouble_chunks[1];

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

            let diagnostics = lsp_manager.diagnostics.lock().unwrap();
            let items: Vec<ListItem> = explorer
                .entries
                .iter()
                .enumerate()
                .map(|(i, entry)| {
                    let name = if entry.path == explorer.root {
                        explorer.root.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or_else(|| explorer.root.to_str().unwrap_or("/"))
                    } else {
                        entry.path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                    };
                    let mut guide = String::new();
                    for _ in 0..entry.depth { guide.push_str("│ "); }
                    if entry.depth > 0 {
                        guide.pop(); guide.pop();
                        if entry.is_last { guide.push_str("└─"); } else { guide.push_str("├─"); }
                    }
                    let guide_len = guide.chars().count();

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
                    ];

                    // Diagnostic indicators in explorer
                    let mut error_count = 0;
                    let mut warning_count = 0;
                    
                    if vim.show_diagnostics {
                        // Only show diagnostic on folders if they are collapsed
                        // If it's a file, always show it.
                        if !entry.is_dir || !entry.is_expanded {
                            for (url, server_diags) in diagnostics.iter() {
                                if let Ok(path) = url.to_file_path() {
                                    if path.starts_with(&entry.path) {
                                        for diags in server_diags.values() {
                                            for diag in diags {
                                                match diag.severity {
                                                    Some(DiagnosticSeverity::ERROR) => error_count += 1,
                                                    Some(DiagnosticSeverity::WARNING) => warning_count += 1,
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if error_count > 0 || warning_count > 0 {
                        // Calculate padding to push to end of line
                        let line_len: usize = guide_len + 1 + 2 + name.chars().count(); // space + icon+space + name
                        // list area width is explorer_layout[1].width, but we have some horizontal padding
                        let available_width = (explorer_layout[1].width as usize).saturating_sub(1);
                        let padding_count = available_width.saturating_sub(line_len).saturating_sub(2);
                        
                        if padding_count > 0 {
                            spans.push(Span::raw(" ".repeat(padding_count)));
                        }

                        if error_count > 0 {
                            spans.push(Span::styled(format!(" {}", icons::ERROR), Style::default().fg(theme.palette.red)));
                        } else if warning_count > 0 {
                            spans.push(Span::styled(format!(" {}", icons::WARNING), Style::default().fg(theme.palette.yellow)));
                        }
                    }

                    // Modified indicator (if open in buffer)
                    for buffer in &editor.buffers {
                        if buffer.file_path.as_ref() == Some(&entry.path) && buffer.modified {
                            spans.push(Span::styled(" ○", Style::default().fg(theme.palette.yellow)));
                            break;
                        }
                    }

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

        // 1.5 Trouble List
        if trouble.visible {
            trouble.draw(frame, trouble_area, vim, theme);
        }

        // 2. Editor Area
        let buffer = editor.buffer();
        let cursor = editor.cursor();
        let scroll_y = cursor.scroll_y;
        let visible_height = editor_area.height as usize;

        // Map screen lines to actual buffer lines, skipping folded ranges
        let screen_to_buffer_lines = editor.get_screen_to_buffer_lines();
        
        let editor_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(7), Constraint::Min(1)])
            .split(editor_area);

        // Find cursor screen y
        let cursor_screen_y = screen_to_buffer_lines.iter().position(|&idx| idx == cursor.y)
            .map(|pos| pos.saturating_sub(scroll_y));

        // Full width highlight for active line
        if let Some(y) = cursor_screen_y {
            if y < visible_height {
                let highlight_rect = Rect {
                    x: editor_area.x,
                    y: editor_area.y + y as u16,
                    width: editor_area.width,
                    height: 1,
                };
                frame.render_widget(Block::default().style(theme.get("CursorLine")), highlight_rect);
            }
        }

        // Line Numbers
        let mut line_numbers = Text::default();
        for i in scroll_y..std::cmp::min(scroll_y + visible_height, screen_to_buffer_lines.len()) {
            let actual_idx = screen_to_buffer_lines[i];
            let is_active = actual_idx == cursor.y;
            let style = if is_active { theme.get("CursorLineNr") } else { theme.get("LineNr") };
            
            let display_num = if vim.relative_number {
                if is_active {
                    format!("{:>3} ", actual_idx + 1)
                } else {
                    let diff = (i as i32 - cursor_screen_y.unwrap_or(0) as i32).abs();
                    format!("{:>3} ", diff)
                }
            } else {
                format!("{:>3} ", actual_idx + 1)
            };

            let mut spans = Vec::new();

            // Diagnostic Icon in gutter
            if vim.show_diagnostics {
                if let Some(path) = &buffer.file_path {
                    if let Ok(url) = lsp_types::Url::from_file_path(path) {
                        let diagnostics = lsp_manager.diagnostics.lock().unwrap();
                        if let Some(server_diags) = diagnostics.get(&url) {
                            let mut line_diag = None;
                            for diags in server_diags.values() {
                                for diag in diags {
                                    if diag.range.start.line as usize == actual_idx {
                                        if line_diag.is_none() || diag.severity < line_diag.as_ref().and_then(|d: &&lsp_types::Diagnostic| d.severity) {
                                            line_diag = Some(diag);
                                        }
                                    }
                                }
                            }

                            if let Some(diag) = line_diag {
                                let (icon, color) = match diag.severity {
                                    Some(DiagnosticSeverity::ERROR) => (icons::ERROR, theme.palette.red),
                                    Some(DiagnosticSeverity::WARNING) => (icons::WARNING, theme.palette.yellow),
                                    _ => ("●", theme.palette.blue),
                                };
                                spans.push(Span::styled(format!("{} ", icon), Style::default().fg(color)));
                            } else {
                                spans.push(Span::raw("  "));
                            }
                        } else {
                            spans.push(Span::raw("  "));
                        }
                    } else {
                        spans.push(Span::raw("  "));
                    }
                } else {
                    spans.push(Span::raw("  "));
                }
            } else {
                spans.push(Span::raw("  "));
            }

            spans.push(Span::styled(display_num, style));

            if buffer.folded_ranges.iter().any(|(s, _)| *s == actual_idx) {
                spans.push(Span::styled(" >", theme.get("Keyword")));
            } else {
                spans.push(Span::raw("  "));
            }

            line_numbers.lines.push(Line::from(spans));
        }
        frame.render_widget(Paragraph::new(line_numbers).alignment(Alignment::Left), editor_layout[0]);

        // Code Content
        let mut text = Text::default();
        let search_query = &vim.search_query;

        for i in scroll_y..std::cmp::min(scroll_y + visible_height, screen_to_buffer_lines.len()) {
            let actual_idx = screen_to_buffer_lines[i];
            let line = &buffer.lines[actual_idx];
            let mut spans = Vec::new();
            let syntax_styles = editor.highlighter.highlight_line(line);
            let is_current_line = actual_idx == cursor.y;

            if let Some((_, end)) = buffer.folded_ranges.iter().find(|(s, _)| *s == actual_idx) {
                // Render a nice fold summary line: StartLine ... count lines ... EndLine
                let first_line_full = line.trim_end();
                let first_line_trimmed = first_line_full.trim_start();
                let first_line_indent = first_line_full.len() - first_line_trimmed.len();
                
                let last_line_full = buffer.lines.get(*end).map(|l| l.as_str()).unwrap_or("}");
                let last_line_trimmed = last_line_full.trim();
                let count = end - actual_idx;
                
                // Add indentation spans
                for _ in 0..first_line_indent {
                    spans.push(Span::raw(" "));
                }

                // First line content
                let first_line_styles = editor.highlighter.highlight_line(first_line_full);
                for (x, c) in first_line_trimmed.chars().enumerate() {
                    spans.push(Span::styled(c.to_string(), first_line_styles.get(x + first_line_indent).copied().unwrap_or_default()));
                }
                
                spans.push(Span::styled(format!(" ... {} lines ... ", count), theme.get("Comment").add_modifier(Modifier::BOLD)));
                
                // Last line content
                let last_line_styles = editor.highlighter.highlight_line(last_line_full);
                let last_line_indent = last_line_full.len() - last_line_full.trim_start().len();
                for (x, c) in last_line_trimmed.chars().enumerate() {
                    spans.push(Span::styled(c.to_string(), last_line_styles.get(x + last_line_indent).copied().unwrap_or_default()));
                }
            } else {
                for (x, c) in line.chars().enumerate() {
                    let mut style = syntax_styles.get(x).copied().unwrap_or(theme.get("Normal"));
                    
                    // Overlay Highlights
                    if let Some(start) = vim.selection_start {
                        let cur = crate::vim::Position { x: cursor.x, y: cursor.y };
                        let (s_y, s_x, e_y, e_x) = if (start.y, start.x) < (cur.y, cur.x) { (start.y, start.x, cur.y, cur.x) } else { (cur.y, cur.x, start.y, start.x) };
                        let is_in_range = if actual_idx > s_y && actual_idx < e_y { true } else if actual_idx == s_y && actual_idx == e_y { x >= s_x && x <= e_x } else if actual_idx == s_y { x >= s_x } else if actual_idx == e_y { x <= e_x } else { false };
                        if is_in_range { style = theme.get("Visual"); }
                    }
                    if !search_query.is_empty() {
                        if let Some(pos) = line.to_lowercase().find(&search_query.to_lowercase()) {
                            if x >= pos && x < pos + search_query.len() {
                                style = theme.get("Search");
                            }
                        }
                    }
                    if vim.yank_highlight_line == Some(actual_idx) { style = Style::default().bg(theme.palette.blue).fg(theme.palette.black); }
                    
                    // Diagnostics undercurl/underline
                    if let Some(path) = &buffer.file_path {
                        if let Ok(url) = lsp_types::Url::from_file_path(path) {
                            let diagnostics_lock = lsp_manager.diagnostics.lock().unwrap();
                            if let Some(server_diags) = diagnostics_lock.get(&url) {
                                for diags in server_diags.values() {
                                    for diag in diags {
                                        if (actual_idx as u32) >= diag.range.start.line && (actual_idx as u32) <= diag.range.end.line {
                                            let s_x = if (actual_idx as u32) == diag.range.start.line { diag.range.start.character as usize } else { 0 };
                                            let e_x = if (actual_idx as u32) == diag.range.end.line { diag.range.end.character as usize } else { line.len() };
                                            if x >= s_x && x < e_x {
                                                let diag_color = match diag.severity {
                                                    Some(lsp_types::DiagnosticSeverity::ERROR) => theme.palette.red,
                                                    Some(lsp_types::DiagnosticSeverity::WARNING) => theme.palette.yellow,
                                                    _ => theme.palette.blue,
                                                };
                                                style = style.underline_color(diag_color).add_modifier(Modifier::UNDERLINED);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Apply CursorLine background to character if it's the current line
                    if is_current_line && style.bg.is_none() {
                        style = style.bg(theme.palette.black2);
                    }

                    if c == '\t' {
                        for _ in 0..2 {
                            spans.push(Span::styled(" ", style));
                        }
                    } else {
                        // Indent guide logic for non-tab characters
                        let is_indent_pos = x > 0 && x % 2 == 0 && x < line.chars().take_while(|&c| c == ' ').count();
                        if is_indent_pos {

                            spans.push(Span::styled("┆", theme.get("Comment").add_modifier(Modifier::DIM)));
                        } else {
                            spans.push(Span::styled(c.to_string(), style));
                        }
                    }
                }
            }
            if line.is_empty() { 
                let mut style = theme.get("Normal");
                if is_current_line { style = style.bg(theme.palette.black2); }
                spans.push(Span::styled(" ", style)); 
            }

            // Indent Blankline Visualization for totally empty lines
            if line.trim().is_empty() && line.is_empty() {
                // Find previous line indentation
                let mut prev_indent = 0;
                for j in (0..actual_idx).rev() {
                    let prev_line = &buffer.lines[j];
                    if !prev_line.trim().is_empty() {
                        prev_indent = prev_line.chars().take_while(|&c| c == ' ' || c == '\t').count();
                        break;
                    }
                }
                
                if prev_indent > 0 {
                    let mut new_spans = Vec::new();
                    let indent_char = "┆";
                    let indent_style = theme.get("Comment").add_modifier(Modifier::DIM);
                    
                    for j in 0..prev_indent {
                        if j > 0 && j % 2 == 0 {
                            new_spans.push(Span::styled(indent_char, indent_style));
                        } else {
                            new_spans.push(Span::raw(" "));
                        }
                    }
                    if !new_spans.is_empty() { spans = new_spans; }
                }
            }
            
            let mut line_obj = Line::from(spans);
            if is_current_line {
                line_obj = line_obj.style(theme.get("CursorLine"));
            }

            // Diagnostic Virtual Text on the right (skip if folded)
            let is_folded = buffer.folded_ranges.iter().any(|(s, _)| *s == actual_idx);
            if vim.show_diagnostics && !is_folded {
                if let Some(path) = &buffer.file_path {
                    if let Ok(url) = lsp_types::Url::from_file_path(path) {
                        let diags_lock = lsp_manager.diagnostics.lock().unwrap();
                        if let Some(server_diags) = diags_lock.get(&url) {
                            // Collect diagnostics for this line
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
                                    Some(DiagnosticSeverity::ERROR) => ("■", theme.palette.red),
                                    Some(DiagnosticSeverity::WARNING) => ("▲", theme.palette.yellow),
                                    _ => ("●", theme.palette.blue),
                                };
                                
                                let mut msg = diag.message.clone();
                                // Include diagnostic code if available
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
                                
                                line_obj.spans.push(Span::styled(format!("{} ", diag_icon), Style::default().fg(diag_color)));
                                line_obj.spans.push(Span::styled(msg, Style::default().fg(theme.palette.grey_fg).add_modifier(Modifier::ITALIC)));
                            }
                        }
                    }
                }
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
                let menu_y = editor_layout[1].y + cursor_screen_y.unwrap_or(0) as u16 + 1;

                let menu_area = Rect {
                    x: menu_x.min(area.right().saturating_sub(menu_width)),
                    y: menu_y.min(editor_trouble_chunks[0].bottom().saturating_sub(menu_height)), 
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

        // 3. Status Line
        let (mode_color, mode_label) = match vim.mode {
            Mode::Normal => (theme.palette.blue, " NORMAL "),
            Mode::Insert => (theme.palette.green, " INSERT "),
            Mode::Visual => (theme.palette.purple, " VISUAL "),
            Mode::Command => (theme.palette.yellow, " COMMAND "),
            _ => (theme.palette.blue, " NORMAL "),
        };

        let mut status_spans = Vec::new();

        // Section A: Mode
        status_spans.push(Span::styled(mode_label, Style::default().fg(theme.palette.black).bg(mode_color).add_modifier(Modifier::BOLD)));

        // Section B: Git
        if let Some(git) = &vim.git_info {
            status_spans.push(Span::styled(format!(" {} {} ", icons::GIT_BRANCH, git.branch), theme.get("StatusLineB")));
            if git.added > 0 { status_spans.push(Span::styled(format!("{}{} ", icons::GIT_ADD, git.added), theme.get("StatusLineGitAdd"))); }
            if git.modified > 0 { status_spans.push(Span::styled(format!("{}{} ", icons::GIT_MOD, git.modified), theme.get("StatusLineGitMod"))); }
            if git.removed > 0 { status_spans.push(Span::styled(format!("{}{} ", icons::GIT_DEL, git.removed), theme.get("StatusLineGitDel"))); }
        }

        // Section C: Filename
        let file_name = buffer.file_path.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("[No Name]");
        let modified_icon = if buffer.modified { " ●" } else { "" };
        status_spans.push(Span::styled(format!(" {}{} ", file_name, modified_icon), theme.get("StatusLineC")));

        // Calculate Right Sections
        let mut right_spans = Vec::new();

        // Section X: LSP Diagnostics
        if vim.show_diagnostics {
            if let Some(path) = &buffer.file_path {
                if let Ok(url) = lsp_types::Url::from_file_path(path) {
                    let diagnostics = lsp_manager.diagnostics.lock().unwrap();
                    if let Some(server_diags) = diagnostics.get(&url) {
                        let mut e = 0; let mut w = 0; let mut i = 0; let mut h = 0;
                        for diags in server_diags.values() {
                            for diag in diags {
                                match diag.severity {
                                    Some(DiagnosticSeverity::ERROR) => e += 1,
                                    Some(DiagnosticSeverity::WARNING) => w += 1,
                                    Some(DiagnosticSeverity::INFORMATION) => i += 1,
                                    Some(DiagnosticSeverity::HINT) => h += 1,
                                    _ => {}
                                }
                            }
                        }
                        if e > 0 { right_spans.push(Span::styled(format!(" {} {} ", icons::ERROR, e), theme.get("StatusLineDiagnosticError"))); }
                        if w > 0 { right_spans.push(Span::styled(format!(" {} {} ", icons::WARNING, w), theme.get("StatusLineDiagnosticWarn"))); }
                        if i > 0 { right_spans.push(Span::styled(format!(" {} {} ", icons::INFO, i), theme.get("StatusLineDiagnosticInfo"))); }
                        if h > 0 { right_spans.push(Span::styled(format!(" {} {} ", icons::HINT, h), theme.get("StatusLineDiagnosticHint"))); }
                    }
                }
            }
        }

        // Section Y: LSP Name
        let lsp_name = if let Some(path) = &buffer.file_path {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let clients = lsp_manager.clients.lock().unwrap();
                if let Some(ext_clients) = clients.get(ext) {
                    let names: Vec<_> = ext_clients.iter().map(|(_, _, name)| name.as_str()).collect();
                    if names.is_empty() { "No LSP".to_string() } else { names.join(",") }
                } else { "No LSP".to_string() }
            } else { "No LSP".to_string() }
        } else { "No LSP".to_string() };

        let lsp_status_text = match &vim.lsp_status {
            LspStatus::Loading | LspStatus::Installing | LspStatus::Formatting => format!(" {} {} ", vim.get_spinner(), lsp_name),
            LspStatus::Ready => format!(" {} ", lsp_name),
            LspStatus::Error(_) => format!(" LSP Error "),
            _ => format!(" {} ", lsp_name),
        };
        right_spans.push(Span::styled(lsp_status_text, theme.get("StatusLineY")));

        // Section Z: Position & Buffers
        let total_lines = buffer.lines.len();
        let percent = if total_lines > 0 { (cursor.y + 1) * 100 / total_lines } else { 0 };
        let pos_text = format!(" {:>2}% {}:{} ", percent, cursor.y + 1, cursor.x + 1);
        let buf_text = format!("(Buffer {}/{}) ", editor.active_idx + 1, editor.buffers.len());
        right_spans.push(Span::styled(format!("{}{}", pos_text, buf_text), theme.get("StatusLineZ")));

        // Combine all
        let left_width: usize = status_spans.iter().map(|s| s.content.chars().count()).sum();
        let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
        let filler_width = (root_chunks[1].width as usize).saturating_sub(left_width).saturating_sub(right_width);
        
        if filler_width > 0 {
            status_spans.push(Span::styled(" ".repeat(filler_width), theme.get("StatusLine")));
        }
        status_spans.extend(right_spans);
        
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
            Mode::Confirm(action) => {
                let prompt = match action {
                    crate::vim::mode::ConfirmAction::Quit => "Unsaved changes! Quit anyway? (y/n): ",
                    crate::vim::mode::ConfirmAction::CloseBuffer => "Unsaved changes! Close buffer anyway? (y/n): ",
                };
                frame.render_widget(Paragraph::new(prompt).style(theme.get("Keyword")), root_chunks[2]);
                frame.set_cursor_position((root_chunks[2].x + prompt.len() as u16, root_chunks[2].y));
            }
            _ => {
                if let Some(msg) = &vim.message {
                    frame.render_widget(Paragraph::new(msg.as_str()).style(theme.get("String")), root_chunks[2]);
                } else {
                    frame.render_widget(Paragraph::new("").style(theme.get("Normal")), root_chunks[2]);
                }
                if vim.focus == Focus::Editor && cursor.y < buffer.lines.len() {
                    let current_line = &buffer.lines[cursor.y];
                    let screen_x: u16 = current_line.chars().take(cursor.x).map(|c| if c == '\t' { 2 } else { 1 }).sum();
                    frame.set_cursor_position((editor_layout[1].x + screen_x, editor_layout[1].y + cursor_screen_y.unwrap_or(0) as u16));
                }
            }
        }

        if let Mode::Mason = vim.mode {
            self.draw_mason(frame, lsp_manager, theme, vim);
        }

        if let Mode::Keymaps = vim.mode {
            self.draw_keymaps(frame, vim, theme);
        }

        if vim.telescope.visible {
            vim.telescope.draw(frame, theme, vim, editor);
        }
    }
}
