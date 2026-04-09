use std::{io, path::PathBuf};

pub mod buffer;
pub mod cursor;
pub mod highlighter;
pub mod todo;
pub mod treesitter;

pub struct Editor {
    pub buffers: Vec<buffer::Buffer>,
    pub cursors: Vec<cursor::Cursor>,
    pub active_idx: usize,
    pub highlighter: highlighter::Highlighter,
    pub treesitter: treesitter::TreesitterManager,
    pub syntax_styles: Vec<Vec<ratatui::style::Style>>,
    pub last_syntax_text: String,
}

impl Editor {
    pub fn new(colorscheme: &str) -> Self {
        let theme = crate::ui::colorscheme::ColorScheme::new(colorscheme);
        Self {
            buffers: vec![buffer::Buffer::new()],
            cursors: vec![cursor::Cursor::new()],
            active_idx: 0,
            highlighter: highlighter::Highlighter::new(theme),
            treesitter: treesitter::TreesitterManager::new(),
            syntax_styles: Vec::new(),
            last_syntax_text: String::new(),
        }
    }

    pub fn refresh_syntax(&mut self) {
        let (text, ext) = {
            let buffer = self.buffer();
            let text = buffer.text.to_string();
            
            // Optimization: Skip if text is exactly the same as last time
            if text == self.last_syntax_text {
                return;
            }

            let ext = buffer.file_path.as_ref()
                .and_then(|p| p.extension())
                .and_then(|s| s.to_str())
                .unwrap_or("rs")
                .to_string();
            (text, ext)
        };
        
        // Map extension to treesitter language name
        let lang_name = match ext.as_str() {
            "rs" => "rust",
            "ts" => "typescript",
            "tsx" => "tsx",
            "js" | "jsx" => "javascript",
            "py" => "python",
            "go" => "go",
            "c" | "h" => "c",
            "cpp" | "hpp" | "cc" | "hh" => "cpp",
            "lua" => "lua",
            "json" => "json",
            "toml" => "toml",
            "html" => "html",
            "css" => "css",
            _ => &ext,
        };

        self.syntax_styles = self.highlighter.highlight_buffer(&text, lang_name, &mut self.treesitter);
        self.last_syntax_text = text;
    }

    pub fn set_theme(&mut self, name: &str) {
        let theme = crate::ui::colorscheme::ColorScheme::new(name);
        self.highlighter.theme = theme;
        // Reset last_syntax_text to force a refresh even if content hasn't changed
        self.last_syntax_text = String::new();
        self.refresh_syntax();
    }

    pub fn buffer(&self) -> &buffer::Buffer {
        &self.buffers[self.active_idx]
    }

    pub fn buffer_mut(&mut self) -> &mut buffer::Buffer {
        &mut self.buffers[self.active_idx]
    }

    pub fn cursor(&self) -> &cursor::Cursor {
        &self.cursors[self.active_idx]
    }

    pub fn cursor_mut(&mut self) -> &mut cursor::Cursor {
        &mut self.cursors[self.active_idx]
    }

    pub fn open_file(&mut self, path: PathBuf) -> io::Result<()> {
        let new_buffer = buffer::Buffer::load(path)?;
        self.buffers.push(new_buffer);
        self.cursors.push(cursor::Cursor::new());
        self.active_idx = self.buffers.len() - 1;
        Ok(())
    }

    pub fn open_scratch_buffer(&mut self, name: &str, content: &str) {
        let mut new_buffer = buffer::Buffer::new();
        new_buffer.text = ropey::Rope::from_str(content);
        new_buffer.file_path = Some(PathBuf::from(name));
        new_buffer.modified = false;
        self.buffers.push(new_buffer);
        self.cursors.push(cursor::Cursor::new());
        self.active_idx = self.buffers.len() - 1;
    }

    pub fn next_buffer(&mut self) {
        if !self.buffers.is_empty() {
            self.active_idx = (self.active_idx + 1) % self.buffers.len();
        }
    }

    pub fn prev_buffer(&mut self) {
        if !self.buffers.is_empty() {
            if self.active_idx == 0 {
                self.active_idx = self.buffers.len() - 1;
            } else {
                self.active_idx -= 1;
            }
        }
    }

    pub fn close_current_buffer(&mut self) {
        if self.buffers.len() > 1 {
            self.buffers.remove(self.active_idx);
            self.cursors.remove(self.active_idx);
            if self.active_idx >= self.buffers.len() {
                self.active_idx = self.buffers.len() - 1;
            }
        } else {
            // Keep at least one empty buffer
            self.buffers[0] = buffer::Buffer::new();
            self.cursors[0] = cursor::Cursor::new();
        }
    }

    pub fn save_file(&mut self) -> io::Result<()> {
        self.buffer_mut().save()
    }

    pub fn save_file_as(&mut self, path: PathBuf) -> io::Result<()> {
        self.buffer_mut().save_as(path)
    }

    pub fn undo(&mut self) -> bool {
        let res = self.buffer_mut().undo();
        if res {
            self.clamp_cursor();
        }
        res
    }

    pub fn redo(&mut self) -> bool {
        let res = self.buffer_mut().redo();
        if res {
            self.clamp_cursor();
        }
        res
    }

    pub fn clamp_cursor(&mut self) {
        let num_lines = self.buffer().len_lines();
        if self.cursor_mut().y >= num_lines {
            self.cursor_mut().y = num_lines.saturating_sub(1);
        }
        let line_len = self.buffer().line(self.cursor().y).map(|s| s.len_chars()).unwrap_or(0);
        let line_len = if self.buffer().line(self.cursor().y).map(|s| s.as_str().unwrap_or("").ends_with('\n')).unwrap_or(false) {
            line_len.saturating_sub(1)
        } else {
            line_len
        };
        if self.cursor_mut().x > line_len {
            self.cursor_mut().x = line_len;
        }
    }

    pub fn get_screen_to_buffer_lines(&self, width: usize, wrap: bool) -> Vec<(usize, usize)> {
        let buffer = self.buffer();
        let mut screen_to_buffer_lines = Vec::new();
        let mut i = 0;
        let num_lines = buffer.len_lines();
        while i < num_lines {
            if wrap {
                let line = buffer.line(i).unwrap();
                let mut line_width = 0;
                for c in line.chars() {
                    if c == '\n' || c == '\r' { continue; }
                    line_width += if c == '\t' { 2 } else { unicode_width::UnicodeWidthChar::width(c).unwrap_or(1) };
                }
                let num_rows = if line_width == 0 { 1 } else { (line_width + width - 1) / width };
                for row in 0..num_rows {
                    screen_to_buffer_lines.push((i, row));
                }
            } else {
                screen_to_buffer_lines.push((i, 0));
            }

            if let Some((_, end)) = buffer.folded_ranges.iter().find(|(s, _)| *s == i) {
                i = *end + 1;
            } else {
                i += 1;
            }
        }
        screen_to_buffer_lines
    }

    pub fn scroll_into_view(&mut self, height: usize, width: usize, wrap: bool) {
        let screen_lines = self.get_screen_to_buffer_lines(width, wrap);
        let y = self.cursor().y;
        let mut scroll_y = self.cursor().scroll_y;

        // Find first screen row of current buffer line
        let screen_y = screen_lines.iter().position(|&(idx, _)| idx == y).unwrap_or(0);

        if screen_y < scroll_y {
            scroll_y = screen_y;
        } else if screen_y >= scroll_y + height {
            scroll_y = screen_y - height + 1;
        }
        self.cursor_mut().scroll_y = scroll_y;
    }

    pub fn move_up(&mut self) {
        if self.cursor().y > 0 {
            let mut target_y = self.cursor().y - 1;
            
            // Jump over folded ranges if necessary
            {
                let buffer = self.buffer();
                while target_y > 0 {
                    if let Some((start, _)) = buffer.folded_ranges.iter().find(|(s, e)| target_y > *s && target_y <= *e) {
                        target_y = *start;
                    } else {
                        break;
                    }
                }
            }

            self.cursor_mut().y = target_y;
            let current_x = self.cursor().x;
            
            let line_len = {
                let line = self.buffer().line(target_y).unwrap();
                if line.as_str().unwrap_or("").ends_with('\n') { line.len_chars().saturating_sub(1) } else { line.len_chars() }
            };
            self.cursor_mut().x = current_x.min(line_len);
        }
    }

    pub fn move_down(&mut self) {
        let num_lines = self.buffer().len_lines();
        if self.cursor().y < num_lines - 1 {
            let mut target_y = self.cursor().y + 1;

            // Jump over folded ranges if necessary
            {
                let buffer = self.buffer();
                while target_y < num_lines {
                    if let Some((_, end)) = buffer.folded_ranges.iter().find(|(s, e)| target_y > *s && target_y <= *e) {
                        target_y = *end + 1;
                    } else {
                        break;
                    }
                }
            }

            if target_y < num_lines {
                self.cursor_mut().y = target_y;
                let current_x = self.cursor().x;
                
                let line_len = {
                    let line = self.buffer().line(target_y).unwrap();
                    if line.as_str().unwrap_or("").ends_with('\n') { line.len_chars().saturating_sub(1) } else { line.len_chars() }
                };
                self.cursor_mut().x = current_x.min(line_len);
            }
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor().x > 0 {
            self.cursor_mut().x -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let y = self.cursor().y;
        let x = self.cursor().x;
        if let Some(line) = self.buffer().line(y) {
            let line_len = if line.as_str().unwrap_or("").ends_with('\n') { line.len_chars().saturating_sub(1) } else { line.len_chars() };
            if x < line_len {
                self.cursor_mut().x += 1;
            }
        }
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_mut().x = 0;
    }

    pub fn move_to_line_end(&mut self) {
        let y = self.cursor().y;
        if let Some(line) = self.buffer().line(y) {
            let line_len = if line.as_str().unwrap_or("").ends_with('\n') { line.len_chars().saturating_sub(1) } else { line.len_chars() };
            self.cursor_mut().x = line_len;
        }
    }

    pub fn jump_to_first_line(&mut self) {
        self.cursor_mut().y = 0;
        self.cursor_mut().x = 0;
    }

    pub fn jump_to_last_line(&mut self) {
        let last_y = self.buffer().len_lines().saturating_sub(1);
        self.cursor_mut().y = last_y;
        self.cursor_mut().x = 0;
    }

    fn is_word_char(c: char) -> bool {
        c.is_alphanumeric() || c == '_'
    }

    pub fn move_word_forward(&mut self) {
        let y = self.cursor().y;
        let x = self.cursor().x;
        let num_lines = self.buffer().len_lines();

        if y >= num_lines { return; }
        let line = self.buffer().line(y).unwrap();
        let line_len = if line.as_str().unwrap_or("").ends_with('\n') { line.len_chars().saturating_sub(1) } else { line.len_chars() };
        
        if x >= line_len {
            if y < num_lines - 1 {
                self.cursor_mut().y += 1;
                self.cursor_mut().x = 0;
                self.move_word_forward();
            }
            return;
        }

        let mut i = x;
        let chars: Vec<char> = line.chars().collect();

        if Self::is_word_char(chars[i]) {
            while i < line_len && Self::is_word_char(chars[i]) {
                i += 1;
            }
            while i < line_len && chars[i].is_whitespace() {
                i += 1;
            }
        } else if chars[i].is_whitespace() {
            while i < line_len && chars[i].is_whitespace() {
                i += 1;
            }
        } else {
            while i < line_len && !chars[i].is_whitespace() && !Self::is_word_char(chars[i]) {
                i += 1;
            }
            while i < line_len && chars[i].is_whitespace() {
                i += 1;
            }
        }

        if i < line_len {
            self.cursor_mut().x = i;
        } else if y < num_lines - 1 {
            self.cursor_mut().y += 1;
            self.cursor_mut().x = 0;
            let y_new = self.cursor().y;
            let next_line = self.buffer().line(y_new).unwrap();
            let mut j = 0;
            let next_chars: Vec<char> = next_line.chars().collect();
            while j < next_chars.len() && next_chars[j].is_whitespace() && next_chars[j] != '\n' && next_chars[j] != '\r' {
                j += 1;
            }
            self.cursor_mut().x = j;
        } else {
            self.cursor_mut().x = line_len;
        }
    }

    pub fn move_word_backward(&mut self) {
        let y = self.cursor().y;
        let x = self.cursor().x;

        if x == 0 {
            if y > 0 {
                self.cursor_mut().y -= 1;
                let y_new = self.cursor().y;
                let line = self.buffer().line(y_new).unwrap();
                let line_len = if line.as_str().unwrap_or("").ends_with('\n') { line.len_chars().saturating_sub(1) } else { line.len_chars() };
                self.cursor_mut().x = line_len;
                self.move_word_backward();
            }
            return;
        }

        let line = self.buffer().line(y).unwrap();
        let chars: Vec<char> = line.chars().collect();
        let mut i = x.saturating_sub(1);

        while i > 0 && chars[i].is_whitespace() {
            i -= 1;
        }

        if chars[i].is_whitespace() {
            self.cursor_mut().x = i;
            return;
        }

        if Self::is_word_char(chars[i]) {
            while i > 0 && Self::is_word_char(chars[i-1]) {
                i -= 1;
            }
        } else {
            while i > 0 && !chars[i-1].is_whitespace() && !Self::is_word_char(chars[i-1]) {
                i -= 1;
            }
        }

        self.cursor_mut().x = i;
    }

    pub fn move_word_end(&mut self) {
        let y = self.cursor().y;
        let x = self.cursor().x;
        let num_lines = self.buffer().len_lines();

        if y >= num_lines { return; }
        let line = self.buffer().line(y).unwrap();
        let line_len = if line.as_str().unwrap_or("").ends_with('\n') { line.len_chars().saturating_sub(1) } else { line.len_chars() };
        
        if x >= line_len.saturating_sub(1) {
            if y < num_lines - 1 {
                self.cursor_mut().y += 1;
                self.cursor_mut().x = 0;
                self.move_word_end();
            }
            return;
        }

        let chars: Vec<char> = line.chars().collect();
        let mut i = x + 1;
        while i < line_len && chars[i].is_whitespace() {
            i += 1;
        }

        if i >= line_len {
            if y < num_lines - 1 {
                self.cursor_mut().y += 1;
                self.cursor_mut().x = 0;
                self.move_word_end();
            }
            return;
        }

        if Self::is_word_char(chars[i]) {
            while i + 1 < line_len && Self::is_word_char(chars[i+1]) {
                i += 1;
            }
        } else {
            while i + 1 < line_len && !chars[i+1].is_whitespace() && !Self::is_word_char(chars[i+1]) {
                i += 1;
            }
        }
        self.cursor_mut().x = i;
    }

    pub fn open_line_below(&mut self) {
        self.buffer_mut().push_history();
        let y = self.cursor().y;
        let line_start = self.buffer().text.line_to_char(y + 1);
        self.buffer_mut().text.insert(line_start, "\n");
        self.cursor_mut().y = y + 1;
        self.cursor_mut().x = 0;
    }

    pub fn open_line_above(&mut self) {
        self.buffer_mut().push_history();
        let y = self.cursor().y;
        let line_start = self.buffer().text.line_to_char(y);
        self.buffer_mut().text.insert(line_start, "\n");
        self.cursor_mut().y = y;
        self.cursor_mut().x = 0;
    }

    pub fn yank(&self, start_x: usize, start_y: usize, end_x: usize, end_y: usize) -> String {
        let (s_y, s_x, e_y, e_x) = if (start_y, start_x) < (end_y, end_x) {
            (start_y, start_x, end_y, end_x)
        } else {
            (end_y, end_x, start_y, start_x)
        };

        let start_char = self.buffer().text.line_to_char(s_y) + s_x;
        let end_char = self.buffer().text.line_to_char(e_y) + e_x + 1;
        
        let end_char = end_char.min(self.buffer().text.len_chars());
        self.buffer().text.slice(start_char..end_char).to_string()
    }

    pub fn paste_before(&mut self, text: &str, yank_type: crate::vim::mode::YankType) {
        if text.is_empty() { return; }
        self.buffer_mut().push_history();

        let cursor_y = self.cursor().y;
        let cursor_x = self.cursor().x;

        if yank_type == crate::vim::mode::YankType::Line {
            let line_start = self.buffer().text.line_to_char(cursor_y);
            let mut paste_text = text.to_string();
            if !paste_text.ends_with('\n') {
                paste_text.push('\n');
            }
            self.buffer_mut().text.insert(line_start, &paste_text);
            self.cursor_mut().y = cursor_y;
            self.cursor_mut().x = 0;
        } else {
            let char_idx = self.buffer().text.line_to_char(cursor_y) + cursor_x;
            self.buffer_mut().text.insert(char_idx, text);
            
            // Move cursor to end of paste
            let new_char_idx = char_idx + text.chars().count();
            let new_y = self.buffer().text.char_to_line(new_char_idx);
            let new_x = new_char_idx - self.buffer().text.line_to_char(new_y);
            self.cursor_mut().y = new_y;
            self.cursor_mut().x = new_x;
        }
    }

    pub fn paste_after(&mut self, text: &str, yank_type: crate::vim::mode::YankType) {
        if text.is_empty() { return; }
        
        if yank_type == crate::vim::mode::YankType::Line {
            self.buffer_mut().push_history();
            let cursor_y = self.cursor().y;
            let line_end = self.buffer().text.line_to_char(cursor_y + 1);
            let mut paste_text = text.to_string();
            if !paste_text.ends_with('\n') {
                paste_text.push('\n');
            }
            self.buffer_mut().text.insert(line_end, &paste_text);
            self.cursor_mut().y = cursor_y + 1;
            self.cursor_mut().x = 0;
        } else {
            let cursor_x = self.cursor().x;
            let line = self.buffer().line(self.cursor().y).unwrap();
            let line_len = if line.as_str().unwrap_or("").ends_with('\n') { line.len_chars().saturating_sub(1) } else { line.len_chars() };
            if cursor_x < line_len {
                self.cursor_mut().x += 1;
            }
            self.paste_before(text, yank_type);
        }
    }

    pub fn delete_selection(&mut self, start_x: usize, start_y: usize, end_x: usize, end_y: usize) -> String {
        let (s_y, s_x, e_y, e_x) = if (start_y, start_x) < (end_y, end_x) {
            (start_y, start_x, end_y, end_x)
        } else {
            (end_y, end_x, start_y, start_x)
        };

        let start_char = self.buffer().text.line_to_char(s_y) + s_x;
        // end_x is inclusive column index, so char index is start_of_line + end_x + 1
        let end_char = self.buffer().text.line_to_char(e_y) + e_x + 1;
        let end_char = end_char.min(self.buffer().text.len_chars());

        let yanked = self.buffer().text.slice(start_char..end_char).to_string();
        self.buffer_mut().push_history();
        self.buffer_mut().text.remove(start_char..end_char);

        self.cursor_mut().x = s_x;
        self.cursor_mut().y = s_y;
        self.clamp_cursor();
        yanked
    }

    pub fn delete_line(&mut self, y: usize) -> String {
        let num_lines = self.buffer().len_lines();
        if num_lines == 0 { return String::new(); }
        
        self.buffer_mut().push_history();
        let start_char = self.buffer().text.line_to_char(y);
        let end_char = if y + 1 < num_lines {
            self.buffer().text.line_to_char(y + 1)
        } else {
            self.buffer().text.len_chars()
        };
        
        let mut yanked = self.buffer().text.slice(start_char..end_char).to_string();
        
        // If it's the last line and we don't have a newline to delete, 
        // try to delete the preceding newline to "remove the row"
        if y > 0 && y + 1 == num_lines && !yanked.ends_with('\n') {
            self.buffer_mut().text.remove(start_char - 1 .. end_char);
            // We should probably include that newline in yanked if we want it to be a "line" yank
            if !yanked.ends_with('\n') { yanked.push('\n'); }
        } else {
            self.buffer_mut().text.remove(start_char..end_char);
        }
        
        if self.buffer().text.len_chars() == 0 {
            self.buffer_mut().text = ropey::Rope::from_str("");
        }
        
        self.clamp_cursor();
        self.cursor_mut().x = 0;
        yanked
    }

    pub fn toggle_fold(&mut self, lsp_ranges: &[lsp_types::FoldingRange]) {
        let cursor_y = self.cursor().y;
        let buffer = self.buffer_mut();
        
        // 1. If current line is already the start of a fold, unfold it
        if let Some(pos) = buffer.folded_ranges.iter().position(|(start, _)| *start == cursor_y) {
            buffer.folded_ranges.remove(pos);
            return;
        }

        // 2. Use LSP ranges if available
        if !lsp_ranges.is_empty() {
            let mut best_range = None;
            for r in lsp_ranges {
                let start = r.start_line as usize;
                let end = r.end_line as usize;
                
                if start == cursor_y && end > start {
                    best_range = Some((start, end));
                    break;
                }
            }

            if let Some((s, e)) = best_range {
                buffer.folded_ranges.push((s, e));
                return;
            }
        }

        // 3. Fallback to Indent-based folding
        let get_indent = |line_idx: usize, buffer: &crate::editor::buffer::Buffer| {
            if let Some(line) = buffer.line(line_idx) {
                let s = line.to_string();
                if s.trim().is_empty() { return usize::MAX; }
                s.chars().take_while(|c| c.is_whitespace()).count()
            } else {
                usize::MAX
            }
        };

        let current_indent = get_indent(cursor_y, buffer);
        if current_indent != usize::MAX {
            let mut end_line = cursor_y;
            let num_lines = buffer.len_lines();
            for i in cursor_y + 1..num_lines {
                let indent = get_indent(i, buffer);
                if indent != usize::MAX && indent <= current_indent {
                    break;
                }
                end_line = i;
            }

            if end_line > cursor_y {
                buffer.folded_ranges.push((cursor_y, end_line));
                return;
            }
        }
    }

    pub fn unfold_all(&mut self) {
        self.buffer_mut().folded_ranges.clear();
    }

    pub fn jump_to_next_hunk(&mut self) {
        let cursor_y = self.cursor().y;
        let buffer = self.buffer();
        if buffer.git_signs.is_empty() { return; }

        let mut hunk_starts: Vec<usize> = Vec::new();
        let mut last_line = None;
        for (line, _) in &buffer.git_signs {
            if last_line.is_none() || *line > last_line.unwrap() + 1 {
                hunk_starts.push(*line);
            }
            last_line = Some(*line);
        }

        if let Some(&next_hunk) = hunk_starts.iter().find(|&&s| s > cursor_y) {
            self.cursor_mut().y = next_hunk;
            self.cursor_mut().x = 0;
        } else if let Some(&first_hunk) = hunk_starts.first() {
            // Wrap around
            self.cursor_mut().y = first_hunk;
            self.cursor_mut().x = 0;
        }
    }

    pub fn jump_to_prev_hunk(&mut self) {
        let cursor_y = self.cursor().y;
        let buffer = self.buffer();
        if buffer.git_signs.is_empty() { return; }

        let mut hunk_starts: Vec<usize> = Vec::new();
        let mut last_line = None;
        for (line, _) in &buffer.git_signs {
            if last_line.is_none() || *line > last_line.unwrap() + 1 {
                hunk_starts.push(*line);
            }
            last_line = Some(*line);
        }

        if let Some(&prev_hunk) = hunk_starts.iter().rev().find(|&&s| s < cursor_y) {
            self.cursor_mut().y = prev_hunk;
            self.cursor_mut().x = 0;
        } else if let Some(&last_hunk) = hunk_starts.last() {
            // Wrap around
            self.cursor_mut().y = last_hunk;
            self.cursor_mut().x = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_new() {
        let editor = Editor::new("catppuccin");
        assert_eq!(editor.buffers.len(), 1);
        assert_eq!(editor.cursors.len(), 1);
    }

    #[test]
    fn test_editor_multi_buffer() {
        let mut editor = Editor::new("catppuccin");
        editor.buffer_mut().text = ropey::Rope::from_str("Buffer 1");
        
        editor.buffers.push(buffer::Buffer::new());
        editor.cursors.push(cursor::Cursor::new());
        editor.active_idx = 1;
        editor.buffer_mut().text = ropey::Rope::from_str("Buffer 2");
        
        editor.prev_buffer();
        assert_eq!(editor.active_idx, 0);
        assert_eq!(editor.buffer().text.to_string(), "Buffer 1");
        
        editor.next_buffer();
        assert_eq!(editor.active_idx, 1);
        assert_eq!(editor.buffer().text.to_string(), "Buffer 2");
    }

    #[test]
    fn test_editor_movement() {
        let mut editor = Editor::new("catppuccin");
        editor.buffer_mut().text = ropey::Rope::from_str("abc\nde");
        editor.move_right();
        assert_eq!(editor.cursor().x, 1);
        editor.move_down();
        assert_eq!(editor.cursor().y, 1);
        assert_eq!(editor.cursor().x, 1);
    }

    #[test]
    fn test_editor_line_boundaries() {
        let mut editor = Editor::new("catppuccin");
        editor.buffer_mut().text = ropey::Rope::from_str("hello world");
        editor.move_to_line_end();
        assert_eq!(editor.cursor().x, 11);
        editor.move_to_line_start();
        assert_eq!(editor.cursor().x, 0);
    }

    #[test]
    fn test_editor_word_movement() {
        let mut editor = Editor::new("catppuccin");
        editor.buffer_mut().text = ropey::Rope::from_str("hello, world rust");
        
        editor.move_word_forward();
        assert_eq!(editor.cursor().x, 5); // start of ','
        
        editor.move_word_forward();
        assert_eq!(editor.cursor().x, 7); // start of 'world'
        
        editor.move_word_end();
        assert_eq!(editor.cursor().x, 11); // end of 'world'
        
        editor.move_word_backward();
        assert_eq!(editor.cursor().x, 7); // start of 'world'
    }

    #[test]
    fn test_editor_delete_selection() {
        let mut editor = Editor::new("catppuccin");
        editor.buffer_mut().text = ropey::Rope::from_str("hello world");
        editor.delete_selection(0, 0, 5, 0); // delete "hello "
        assert_eq!(editor.buffer().text.to_string(), "world");
    }

    #[test]
    fn test_editor_open_line() {
        let mut editor = Editor::new("catppuccin");
        editor.buffer_mut().text = ropey::Rope::from_str("line 1");
        
        editor.open_line_below();
        assert_eq!(editor.buffer().len_lines(), 2);
        assert_eq!(editor.cursor().y, 1);
        
        editor.open_line_above();
        assert_eq!(editor.buffer().len_lines(), 3);
        assert_eq!(editor.cursor().y, 1);
        assert_eq!(editor.buffer().line(1).unwrap().to_string(), "\n");
    }

    #[test]
    fn test_editor_paste() {
        let mut editor = Editor::new("catppuccin");
        editor.buffer_mut().text = ropey::Rope::from_str("ab");
        editor.cursor_mut().x = 1; // On 'b'
        
        editor.paste_after("X", crate::vim::mode::YankType::Char);
        assert_eq!(editor.buffer().text.to_string(), "abX");
        
        editor.cursor_mut().x = 1; // On 'b'
        editor.paste_before("Y", crate::vim::mode::YankType::Char);
        assert_eq!(editor.buffer().text.to_string(), "aYbX");
    }
}
