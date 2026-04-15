use super::*;

impl TerminalUi {
    pub(crate) fn draw_mason(
        &self,
        frame: &mut Frame,
        editor: &crate::editor::Editor,
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
            .title(" Manage.atom ")
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));

        frame.render_widget(Clear, mason_area);
        frame.render_widget(block, mason_area);

        let inner_area = mason_area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(inner_area);

        let tabs = [
            "All",
            "LSP",
            "DAP",
            "Linter",
            "Formatter",
            "Treesitter",
        ];
        let mut tab_spans = Vec::new();
        for (i, tab) in tabs.iter().enumerate() {
            let style = if i == vim.mason_tab {
                Style::default()
                    .fg(theme.palette.black)
                    .bg(theme.palette.orange)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.get("Comment")
            };
            tab_spans.push(Span::styled(format!(" {} ", tab), style));
            tab_spans.push(Span::raw("  "));
        }
        frame.render_widget(Paragraph::new(Line::from(tab_spans)), chunks[0]);

        let filter_prompt = "Language Filter: ";
        let filter_text = if let Mode::MasonFilter = vim.mode {
            format!("{}{}", filter_prompt, vim.mason_filter)
        } else if vim.mason_filter.is_empty() {
            "Language Filter: press <C-f> to apply filter".to_string()
        } else {
            format!("{}{}", filter_prompt, vim.mason_filter)
        };

        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(vec![Span::styled(filter_text, theme.get("Comment"))]),
            ]),
            chunks[1],
        );

        if let Mode::MasonFilter = vim.mode {
            frame.set_cursor_position((
                chunks[1].x + filter_prompt.len() as u16 + vim.mason_filter.len() as u16,
                chunks[1].y + 1,
            ));
        }

        let mut items = Vec::new();
        let installing_set = lsp_manager.installing.lock().unwrap();

        if vim.mason_tab == 5 {
            let ts = editor.treesitter.lock().unwrap();
            let languages = &crate::editor::treesitter::LANGUAGES;
            let filtered_langs: Vec<_> = languages
                .iter()
                .filter(|l| {
                    l.name
                        .to_lowercase()
                        .contains(&vim.mason_filter.to_lowercase())
                })
                .collect();

            let (installed, available): (Vec<_>, Vec<_>) = filtered_langs
                .into_iter()
                .partition(|l| ts.is_installed(l.name));
            drop(ts); // Release lock before drawing items which might call it again via get_spinner if nested, though it's not here.

            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("Installed ({})", installed.len()),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.palette.orange),
            )])));

            for l in &installed {
                let is_installing = installing_set.contains(l.name);
                let mut spans = vec![
                    Span::styled(" ● ", theme.get("String")),
                    Span::styled(format!("{:<25} ", l.name), theme.get("Keyword")),
                    Span::styled(format!("{:<40} ", l.repo), theme.get("Comment")),
                ];
                if is_installing {
                    spans.push(Span::styled(format!(" {} installing...", vim.get_spinner()), theme.get("Type")));
                } else {
                    spans.push(Span::styled(" installed", theme.get("String")));
                }
                items.push(ListItem::new(Line::from(spans)));
            }

            items.push(ListItem::new(Line::from("")));
            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("Available ({})", available.len()),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.palette.blue),
            )])));

            for l in &available {
                let is_installing = installing_set.contains(l.name);
                let mut spans = vec![
                    Span::styled(" ○ ", theme.get("Comment")),
                    Span::styled(format!("{:<25} ", l.name), theme.get("Normal")),
                    Span::styled(format!("{:<40} ", l.repo), theme.get("Comment")),
                ];
                if is_installing {
                    spans.push(Span::styled(format!(" {} downloading...", vim.get_spinner()), theme.get("Type")));
                }
                items.push(ListItem::new(Line::from(spans)));
            }
        } else {
            let packages: Vec<&crate::lsp::Package> = crate::lsp::PACKAGES
                .iter()
                .filter(|p| {
                    let matches_tab = match vim.mason_tab {
                        0 => true,
                        1 => p.kind == crate::lsp::PackageKind::Lsp,
                        2 => p.kind == crate::lsp::PackageKind::Dap,
                        3 => p.kind == crate::lsp::PackageKind::Linter,
                        4 => p.kind == crate::lsp::PackageKind::Formatter,
                        _ => true,
                    };
                    let filter = vim.mason_filter.to_lowercase();
                    let matches_filter = p.name.to_lowercase().contains(&filter)
                        || p.description.to_lowercase().contains(&filter);
                    matches_tab && matches_filter
                })
                .collect();

            let (mut installed, mut available): (Vec<_>, Vec<_>) = packages
                .into_iter()
                .partition(|p| lsp_manager.is_managed(p.cmd));
            installed.sort_by_key(|p| p.name);
            available.sort_by_key(|p| p.name);

            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("Installed ({})", installed.len()),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.palette.orange),
            )])));

            for p in &installed {
                let is_installing = installing_set.contains(p.cmd);
                let mut spans = vec![
                    Span::styled(" ● ", theme.get("String")),
                    Span::styled(format!("{:<25} ", p.name), theme.get("Keyword")),
                    Span::styled(format!("{:<40} ", p.cmd), theme.get("Comment")),
                ];
                if is_installing {
                    spans.push(Span::styled(format!(" {} installing...", vim.get_spinner()), theme.get("Type")));
                } else {
                    spans.push(Span::styled(" installed", theme.get("String")));
                }
                items.push(ListItem::new(Line::from(spans)));
            }

            items.push(ListItem::new(Line::from("")));
            items.push(ListItem::new(Line::from(vec![Span::styled(
                format!("Available ({})", available.len()),
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(theme.palette.blue),
            )])));

            for p in &available {
                let is_installing = installing_set.contains(p.cmd);
                let mut spans = vec![
                    Span::styled(" ○ ", theme.get("Comment")),
                    Span::styled(format!("{:<25} ", p.name), theme.get("Normal")),
                    Span::styled(format!("{:<40} ", p.description), theme.get("Comment")),
                ];
                if is_installing {
                    spans.push(Span::styled(format!(" {} downloading...", vim.get_spinner()), theme.get("Type")));
                }
                items.push(ListItem::new(Line::from(spans)));
            }
        }

        let list = List::new(items)
            .highlight_style(theme.get("CursorLine"))
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, chunks[2], &mut vim.mason_state);

        let mut help_spans = vec![Span::styled(
            " space/i: install  u: update  d/x: uninstall  q: close ",
            theme.get("Comment"),
        )];

        if !installing_set.is_empty() {
            let pkg = installing_set.iter().next().unwrap();
            help_spans.push(Span::styled(
                format!("  {} Installing {}... ", vim.get_spinner(), pkg),
                theme.get("Keyword"),
            ));
        }

        frame.render_widget(
            Paragraph::new(Line::from(help_spans)).alignment(Alignment::Center),
            chunks[3],
        );
    }
}
