use super::*;

impl TerminalUi {
    pub(crate) fn draw_keymaps(
        &self,
        frame: &mut Frame,
        vim: &mut crate::vim::VimState,
        theme: &crate::ui::colorscheme::ColorScheme,
    ) {
        let area = frame.area();
        let width = (area.width as f32 * 0.4) as u16;
        let height = (area.height as f32 * 0.6) as u16;
        let keymap_area = Rect {
            x: (area.width - width) / 2,
            y: (area.height - height) / 2,
            width,
            height,
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Keymaps Help ")
            .border_style(theme.get("Keyword"))
            .style(theme.get("Normal"));

        frame.render_widget(Clear, keymap_area);
        frame.render_widget(block, keymap_area);

        let inner_area = keymap_area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner_area);

        let filter_prompt = " Filter: ";
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(filter_prompt, theme.get("Comment")),
                Span::styled(&vim.keymap_filter, theme.get("Normal")),
            ])),
            chunks[0],
        );

        frame.set_cursor_position((
            chunks[0].x + filter_prompt.len() as u16 + vim.keymap_filter.len() as u16,
            chunks[0].y,
        ));

        frame.render_widget(
            Paragraph::new("─".repeat(chunks[1].width as usize)).style(theme.get("Comment")),
            chunks[1],
        );

        let all_keys = vec![
            ("--- NORMAL ---", ""),
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
            ("[g / ]g", "Prev/Next Git Hunk"),
            ("<Space>bl", "Show Git Blame (Popup)"),
            ("zc / za", "Fold / Unfold"),
            ("<Space>bb", "Toggle Autoformat"),
            ("<Space>x", "Close current buffer"),
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
            ("--- INSERT ---", ""),
            ("<Esc>", "Normal mode"),
            ("<C-s>", "Save & Format"),
            ("<Tab>", "2 Spaces / CMP Next"),
            ("<Up/Down>", "CMP Nav"),
            ("<C-Space>", "Trigger CMP"),
            ("<C-n/p>", "CMP Next/Prev"),
            ("<Enter>", "Select CMP / New Line"),
            ("--- EXPLORER ---", ""),
            ("Enter", "Open File"),
            ("a", "Create File/Dir"),
            ("d", "Delete"),
            ("r", "Rename"),
            ("y", "Copy Path"),
            ("H", "Toggle Hidden"),
            ("--- COMMAND ---", ""),
            (":w / :write", "Save & Format"),
            (":q / :quit", "Close current buffer/Quit"),
            (":q!", "Quit without saving"),
            (":qa", "Close all & Quit"),
            (":wq", "Save and Quit"),
            (":wa", "Save all buffers"),
            (":e / :edit", "Open a file"),
            (":bd / :bdelete", "Close current buffer"),
            (":bn / :bnext", "Go to next buffer"),
            (":bp / :bprev", "Go to previous buffer"),
            (":colorscheme", "Switch theme"),
            (":Manage", "Manage LSPs & Parsers"),
            (":TreesitterManager", "Open Manage on the Treesitter tab"),
            (":Trouble", "Toggle trouble list"),
            (":format / :Format", "Format current buffer"),
            (":FormatAll", "Format all buffers"),
            (":FormatEnable", "Enable autoformat"),
            (":FormatDisable", "Disable autoformat"),
            (":Reload / :e!", "Reload file from disk"),
            (":gd", "Go to definition"),
            (":LspInfo", "Show LSP status"),
            (":LspRestart", "Restart LSP server"),
            ("--- TELESCOPE ---", ""),
            ("<Space>ff", "Find Files"),
            ("<Space>fg", "Live Grep"),
            ("<Space>fb", "Select Buffer"),
            ("<Esc>", "Close Telescope"),
            ("<Enter>", "Open Selected"),
            ("Up/Down / Tab / S-Tab", "Navigate"),
            ("<C-u/d>", "Scroll Preview"),
        ];

        let filter = vim.keymap_filter.to_lowercase();
        let rows: Vec<Row> = all_keys
            .iter()
            .filter(|(k, d)| {
                filter.is_empty()
                    || k.to_lowercase().contains(&filter)
                    || d.to_lowercase().contains(&filter)
            })
            .map(|(k, d)| {
                if k.starts_with("---") {
                    Row::new(vec![
                        Cell::from(Span::styled(
                            *k,
                            Style::default()
                                .add_modifier(Modifier::BOLD)
                                .fg(theme.palette.blue),
                        )),
                        Cell::from(""),
                    ])
                } else {
                    Row::new(vec![
                        Cell::from(Span::styled(*k, theme.get("Keyword"))),
                        Cell::from(Span::styled(*d, theme.get("Normal"))),
                    ])
                }
            })
            .collect();

        let table = Table::new(
            rows,
            [Constraint::Percentage(30), Constraint::Percentage(70)],
        )
        .header(
            Row::new(vec![
                Cell::from(Span::styled(
                    " Key",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(theme.palette.orange),
                )),
                Cell::from(Span::styled(
                    " Description",
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(theme.palette.orange),
                )),
            ])
            .bottom_margin(1),
        )
        .row_highlight_style(theme.get("CursorLine"))
        .highlight_symbol(" ");

        frame.render_stateful_widget(table, chunks[2], &mut vim.keymap_state);
    }
}
