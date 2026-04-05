use std::path::{Path, PathBuf};
use std::process::Command;
use ignore::WalkBuilder;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span, Text},
    widgets::{Block, Borders, BorderType, List, ListItem, Paragraph, Clear},
    Frame,
};
use crate::vim::VimState;
use crate::vim::mode::{Mode, TelescopeKind};

pub struct TelescopeResult {
    pub path: PathBuf,
    pub line_number: Option<usize>,
    pub content: Option<String>,
}

pub struct Telescope {
    pub query: String,
    pub results: Vec<TelescopeResult>,
    pub selected_idx: usize,
    pub visible: bool,
    pub preview_lines: Vec<String>,
    pub preview_start_line: usize,
    pub preview_scroll: usize,
    pub kind: TelescopeKind,
    pub search_root: PathBuf,
}

impl Telescope {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected_idx: 0,
            visible: false,
            preview_lines: Vec::new(),
            preview_start_line: 0,
            preview_scroll: 0,
            kind: TelescopeKind::Files,
            search_root: PathBuf::from("."),
        }
    }

    pub fn open(&mut self, kind: TelescopeKind, root: PathBuf, editor: &crate::editor::Editor) {
        self.kind = kind;
        self.search_root = root;
        self.query.clear();
        self.results.clear();
        self.selected_idx = 0;
        self.visible = true;
        self.preview_lines.clear();
        self.update_results(editor);
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn update_results(&mut self, editor: &crate::editor::Editor) {
        match self.kind {
            TelescopeKind::Files => self.search_files(),
            TelescopeKind::Words => self.search_words(),
            TelescopeKind::Buffers => self.search_buffers(editor),
        }
        if self.selected_idx >= self.results.len() {
            self.selected_idx = 0;
        }
        self.update_preview();
    }

    fn search_buffers(&mut self, editor: &crate::editor::Editor) {
        self.results.clear();
        for buffer in &editor.buffers {
            if let Some(path) = &buffer.file_path {
                let path_str = path.to_string_lossy().to_string();
                if self.query.is_empty() || path_str.to_lowercase().contains(&self.query.to_lowercase()) {
                    self.results.push(TelescopeResult {
                        path: path.clone(),
                        line_number: None,
                        content: None,
                    });
                }
            } else if self.query.is_empty() {
                self.results.push(TelescopeResult {
                    path: PathBuf::from("[No Name]"),
                    line_number: None,
                    content: None,
                });
            }
        }
    }

    fn search_files(&mut self) {
        self.results.clear();
        if self.query.is_empty() { return; }

        let walker = WalkBuilder::new(&self.search_root)
            .hidden(true)
            .git_ignore(true)
            .build();

        let query_lower = self.query.to_lowercase();
        for entry in walker.filter_map(|e| e.ok()) {
            if entry.file_type().map(|f| f.is_file()).unwrap_or(false) {
                let path = entry.path().strip_prefix(&self.search_root).unwrap_or(entry.path()).to_path_buf();
                if path == Path::new("") { continue; }
                let path_str = path.to_string_lossy().to_string();
                if path_str.to_lowercase().contains(&query_lower) {
                    self.results.push(TelescopeResult {
                        path: entry.path().to_path_buf(),
                        line_number: None,
                        content: None,
                    });
                }
            }
            if self.results.len() > 100 { break; }
        }
    }

    fn search_words(&mut self) {
        self.results.clear();
        if self.query.is_empty() { return; }

        let output = Command::new("rg")
            .arg("--vimgrep")
            .arg("--smart-case")
            .arg(&self.query)
            .arg(&self.search_root)
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.splitn(4, ':').collect();
                if parts.len() >= 4 {
                    let path = PathBuf::from(parts[0]);
                    let line_number = parts[1].parse().ok();
                    let content = Some(parts[3].trim().to_string());
                    self.results.push(TelescopeResult {
                        path,
                        line_number,
                        content,
                    });
                }
                if self.results.len() > 100 { break; }
            }
        }
    }

    pub fn update_preview(&mut self) {
        self.preview_lines.clear();
        self.preview_scroll = 0;
        if let Some(result) = self.results.get(self.selected_idx) {
            if let Ok(content) = std::fs::read_to_string(&result.path) {
                self.preview_lines = content.lines().map(|s| s.to_string()).collect();
                let target_line = result.line_number.unwrap_or(1).saturating_sub(1);
                
                // Center the target line in the preview window (assuming default height of ~40)
                self.preview_scroll = target_line.saturating_sub(15);
            }
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
            self.update_preview();
        }
    }

    pub fn move_down(&mut self) {
        if !self.results.is_empty() && self.selected_idx < self.results.len() - 1 {
            self.selected_idx += 1;
            self.update_preview();
        }
    }

    pub fn scroll_preview_up(&mut self, amount: usize) {
        self.preview_scroll = self.preview_scroll.saturating_sub(amount);
    }

    pub fn scroll_preview_down(&mut self, amount: usize) {
        if !self.preview_lines.is_empty() {
            self.preview_scroll = std::cmp::min(
                self.preview_scroll + amount,
                self.preview_lines.len().saturating_sub(1)
            );
        }
    }

    pub fn draw(
        &self,
        frame: &mut Frame,
        theme: &crate::ui::colorscheme::ColorScheme,
        vim: &VimState,
        editor: &crate::editor::Editor,
    ) {
        let area = frame.area();
        let width = (area.width as f32 * 0.80) as u16;
        let height = (area.height as f32 * 0.70) as u16;
        let telescope_area = Rect {
            x: (area.width - width) / 2,
            y: (area.height - height) / 2,
            width,
            height,
        };

        frame.render_widget(Clear, telescope_area);

        // Main split: Left (Results + Search) vs Right (Preview)
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Percentage(60),
            ])
            .split(telescope_area);

        // Left column split: Results (top) and Search (bottom)
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(main_chunks[0]);

        // 1. Results (Left Top)
        let results_title = match self.kind {
            TelescopeKind::Files => " Results ",
            TelescopeKind::Words => " Grep Results ",
            TelescopeKind::Buffers => " Open Buffers ",
        };

        let results_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(results_title)
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));
        
        let items: Vec<ListItem> = self.results.iter().enumerate().map(|(i, res)| {
            let is_selected = i == self.selected_idx;
            let mut style = theme.get("Normal");
            let mut item_style = ratatui::style::Style::default();
            
            if is_selected {
                style = style.add_modifier(Modifier::BOLD);
                item_style = theme.get("CursorLine");
            }

            let (icon, icon_group) = crate::ui::TerminalUi::get_file_icon(&res.path);
            let icon_style = theme.get(&icon_group);
            
            let rel_path = res.path.strip_prefix(&self.search_root).unwrap_or(&res.path);
            let path_str = if rel_path == res.path {
                rel_path.to_string_lossy().to_string()
            } else {
                format!("./{}", rel_path.display())
            };

            let mut spans = vec![
                Span::raw(" "),
                Span::styled(icon, icon_style),
                Span::raw(" "),
                Span::styled(path_str, style)
            ];
            
            if let Some(line) = res.line_number {
                spans.push(Span::styled(format!(":{}", line), theme.get("Comment")));
            }
            
            if let Some(content) = &res.content {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(content, theme.get("Comment")));
            }

            ListItem::new(Line::from(spans)).style(item_style)
        }).collect();

        frame.render_widget(List::new(items).block(results_block), left_chunks[0]);

        // 2. Search Input (Left Bottom)
        let input_title = match self.kind {
            TelescopeKind::Files => " Find Files ",
            TelescopeKind::Words => " Live Grep ",
            TelescopeKind::Buffers => " Select Buffer ",
        };

        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(input_title)
            .border_style(theme.get("Keyword"))
            .style(theme.get("Normal"));
        
        let inner_input_area = input_block.inner(left_chunks[1]);
        frame.render_widget(input_block, left_chunks[1]);

        let input_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(10)])
            .split(inner_input_area);

        let input_text = format!("> {}", self.query);
        frame.render_widget(Paragraph::new(input_text).style(theme.get("Normal")), input_chunks[0]);

        let count_text = format!("{}/{}", if self.results.is_empty() { 0 } else { self.selected_idx + 1 }, self.results.len());
        frame.render_widget(Paragraph::new(count_text).alignment(ratatui::layout::Alignment::Right).style(theme.get("Comment")), input_chunks[1]);

        if let Mode::Telescope(_) = vim.mode {
            frame.set_cursor_position((
                input_chunks[0].x + self.query.len() as u16 + 2,
                input_chunks[0].y,
            ));
        }

        // 3. Preview (Right Column - Full Height)
        let selected_result = self.results.get(self.selected_idx);
        let preview_path = selected_result
            .map(|r| {
                let rel = r.path.strip_prefix(&self.search_root).unwrap_or(&r.path);
                if rel == r.path {
                    rel.to_string_lossy().to_string()
                } else {
                    format!("./{}", rel.display())
                }
            })
            .unwrap_or_default();
        let preview_title = format!(" Preview: {} ", preview_path);
        let preview_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(preview_title)
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));

        let inner_preview_area = preview_block.inner(main_chunks[1]);
        frame.render_widget(preview_block, main_chunks[1]);

        let preview_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(6), Constraint::Min(1)])
            .split(inner_preview_area);

        let mut line_numbers = Text::default();
        let mut preview_text = Text::default();
        let target_line_idx = selected_result.and_then(|r| r.line_number.map(|l| l.saturating_sub(1)));

        let preview_height = inner_preview_area.height as usize;
        for i in 0..preview_height {
            let actual_line_idx = self.preview_scroll + i;
            if actual_line_idx >= self.preview_lines.len() { break; }
            let line = &self.preview_lines[actual_line_idx];
            let is_target = Some(actual_line_idx) == target_line_idx;
            
            // Highlight active line background in preview
            if is_target {
                let highlight_rect = Rect {
                    x: inner_preview_area.x,
                    y: inner_preview_area.y + i as u16,
                    width: inner_preview_area.width,
                    height: 1,
                };
                frame.render_widget(Block::default().style(theme.get("CursorLine")), highlight_rect);
            }

            // Line numbers column
            let ln_style = if is_target { theme.get("CursorLineNr") } else { theme.get("LineNr") };
            line_numbers.lines.push(Line::from(vec![Span::styled(format!("{:>4} ", actual_line_idx + 1), ln_style)]));

            // Code content
            let syntax_styles = editor.highlighter.highlight_line(line);
            let mut spans = Vec::new();
            for (x, c) in line.chars().enumerate() {
                let mut style = syntax_styles.get(x).copied().unwrap_or(theme.get("Normal"));
                if is_target && style.bg.is_none() {
                    style = style.bg(theme.palette.black2);
                }

                if c == '\t' {
                    for _ in 0..2 {
                        spans.push(Span::styled(" ", style));
                    }
                } else {
                    spans.push(Span::styled(c.to_string(), style));
                }
            }
            if line.is_empty() {
                let mut style = theme.get("Normal");
                if is_target { style = style.bg(theme.palette.black2); }
                spans.push(Span::styled(" ", style));
            }
            preview_text.lines.push(Line::from(spans));
        }

        frame.render_widget(Paragraph::new(line_numbers).alignment(ratatui::layout::Alignment::Right), preview_layout[0]);
        frame.render_widget(Paragraph::new(preview_text), preview_layout[1]);
    }
}
