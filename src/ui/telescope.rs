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
            kind: TelescopeKind::Files,
            search_root: PathBuf::from("."),
        }
    }

    pub fn open(&mut self, kind: TelescopeKind, root: PathBuf) {
        self.kind = kind;
        self.search_root = root;
        self.query.clear();
        self.results.clear();
        self.selected_idx = 0;
        self.visible = true;
        self.preview_lines.clear();
        self.update_results();
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn update_results(&mut self) {
        match self.kind {
            TelescopeKind::Files => self.search_files(),
            TelescopeKind::Words => self.search_words(),
        }
        if self.selected_idx >= self.results.len() {
            self.selected_idx = 0;
        }
        self.update_preview();
    }

    fn search_files(&mut self) {
        self.results.clear();
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
                if self.query.is_empty() || path_str.to_lowercase().contains(&query_lower) {
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
        if let Some(result) = self.results.get(self.selected_idx) {
            if let Ok(content) = std::fs::read_to_string(&result.path) {
                let lines: Vec<&str> = content.lines().collect();
                let target_line = result.line_number.unwrap_or(1).saturating_sub(1);
                
                let preview_start = target_line.saturating_sub(15);
                let preview_end = std::cmp::min(lines.len(), target_line + 30);
                
                for i in preview_start..preview_end {
                    self.preview_lines.push(lines[i].to_string());
                }
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

    pub fn draw(
        &self,
        frame: &mut Frame,
        theme: &crate::ui::colorscheme::ColorScheme,
        vim: &VimState,
        editor: &crate::editor::Editor,
    ) {
        let area = frame.area();
        let width = (area.width as f32 * 0.95) as u16;
        let height = (area.height as f32 * 0.95) as u16;
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
        };

        let results_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(results_title)
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));
        
        let items: Vec<ListItem> = self.results.iter().enumerate().map(|(i, res)| {
            let mut style = theme.get("Normal");
            if i == self.selected_idx {
                style = theme.get("CursorLine").add_modifier(Modifier::BOLD);
            }

            let (icon, icon_group) = crate::ui::TerminalUi::get_file_icon(&res.path);
            let icon_style = theme.get(&icon_group);
            
            let path_str = res.path.strip_prefix(&vim.project_root).unwrap_or(&res.path).to_string_lossy();
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

            ListItem::new(Line::from(spans))
        }).collect();

        frame.render_widget(List::new(items).block(results_block), left_chunks[0]);

        // 2. Search Input (Left Bottom)
        let input_title = match self.kind {
            TelescopeKind::Files => " Find Files ",
            TelescopeKind::Words => " Live Grep ",
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
        let preview_path = self.results.get(self.selected_idx)
            .map(|r| r.path.strip_prefix(&vim.project_root).unwrap_or(&r.path).to_string_lossy().to_string())
            .unwrap_or_default();
        let preview_title = format!(" Preview: {} ", preview_path);
        let preview_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(preview_title)
            .border_style(theme.get("TreeExplorerConnector"))
            .style(theme.get("Normal"));

        let mut preview_text = Text::default();
        for line in &self.preview_lines {
            let syntax_styles = editor.highlighter.highlight_line(line);
            let mut spans = Vec::new();
            for (x, c) in line.chars().enumerate() {
                let style = syntax_styles.get(x).copied().unwrap_or(theme.get("Normal"));
                spans.push(Span::styled(c.to_string(), style));
            }
            preview_text.lines.push(Line::from(spans));
        }

        frame.render_widget(Paragraph::new(preview_text).block(preview_block), main_chunks[1]);
    }
}
