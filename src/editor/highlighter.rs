use ratatui::style::Style;
use crate::ui::colorscheme::ColorScheme;
use tree_sitter_highlight::{Highlighter as TsHighlighter, HighlightConfiguration, HighlightEvent};
use std::collections::HashMap;

pub struct Highlighter {
    pub theme: ColorScheme,
    ts_highlighter: TsHighlighter,
    configs: HashMap<String, HighlightConfiguration>,
}

impl Highlighter {
    pub fn new(theme: ColorScheme) -> Self {
        Self { 
            theme,
            ts_highlighter: TsHighlighter::new(),
            configs: HashMap::new(),
        }
    }

    pub fn ensure_config(&mut self, lang_name: &str, ts_manager: &mut crate::editor::treesitter::TreesitterManager) {
        if !self.configs.contains_key(lang_name) {
            if let Some(config) = ts_manager.get_highlight_config(lang_name) {
                self.configs.insert(lang_name.to_string(), config);
            }
        }
    }

    pub fn highlight_buffer(&mut self, text: &str, lang_name: &str, ts_manager: &mut crate::editor::treesitter::TreesitterManager) -> Vec<Vec<Style>> {
        self.ensure_config(lang_name, ts_manager);
        
        let lines: Vec<&str> = text.lines().collect();
        let mut result = Vec::with_capacity(lines.len());
        for line in &lines {
            result.push(vec![self.theme.get("Normal"); line.len()]);
        }

        if let Some(config) = self.configs.get(lang_name) {
            let Ok(highlights) = self.ts_highlighter.highlight(config, text.as_bytes(), None, |_| None) else {
                return result;
            };
            
            let mut current_style = self.theme.get("Normal");
            let mut highlight_stack: Vec<Style> = Vec::new();

            // Pre-calculate line start offsets
            let mut line_starts = Vec::with_capacity(lines.len());
            let mut current_offset = 0;
            for line in text.split_inclusive('\n') {
                line_starts.push(current_offset);
                current_offset += line.len();
            }

            // Must stay in sync with the captures array in treesitter.rs
            let captures = [
                "Keyword",    // 0  keyword
                "Function",   // 1  function
                "Type",       // 2  type
                "String",     // 3  string
                "Comment",    // 4  comment
                "Constant",   // 5  constant / constant.builtin
                "Variable",   // 6  variable
                "Identifier", // 7  parameter
                "Keyword",    // 8  label
                "Tag",        // 9  tag
                "Attribute",  // 10 attribute
                "Constant",   // 11 number
                "Keyword",    // 12 operator
                "Property",   // 13 property
                "Type",       // 14 namespace
                "Normal",     // 15 punctuation
            ];

            for event in highlights {
                match event.map_err(|e| e.to_string()) {
                    Ok(HighlightEvent::Source { start, end }) => {
                        // Efficiently apply style to the range [start, end)
                        let mut i = start;
                        while i < end {
                            // Find the line index for the current offset 'i'
                            let line_idx = match line_starts.binary_search(&i) {
                                Ok(idx) => idx,
                                Err(idx) => idx.saturating_sub(1),
                            };

                            if line_idx >= result.len() { break; }

                            let line_start = line_starts[line_idx];
                            let line_end = if line_idx + 1 < line_starts.len() {
                                line_starts[line_idx + 1]
                            } else {
                                text.len()
                            };

                            // Calculate how much of this range fits in the current line
                            let apply_end = end.min(line_end);
                            
                            // Adjust for newline characters at the end of the source range
                            let col_start = i - line_start;
                            let col_end = (apply_end - line_start).min(result[line_idx].len());
                            
                            if col_start < col_end {
                                for col in col_start..col_end {
                                    result[line_idx][col] = current_style;
                                }
                            }
                            
                            i = apply_end;
                            if i == line_end && i < end {
                                // We reached the end of the line but still have more to highlight
                                // The next iteration will pick up the next line
                            }
                        }
                    }
                    Ok(HighlightEvent::HighlightStart(s)) => {
                        highlight_stack.push(current_style);
                        let cap_name = captures.get(s.0).copied().unwrap_or("Normal");
                        current_style = self.theme.get(cap_name);
                    }
                    Ok(HighlightEvent::HighlightEnd) => {
                        current_style = highlight_stack.pop().unwrap_or(self.theme.get("Normal"));
                    }
                    _ => {}
                }
            }
        } else {
            // Fallback to regex-based highlighting for each line
            for (i, line) in lines.iter().enumerate() {
                result[i] = self.highlight_line(line);
            }
        }

        result
    }

    pub fn highlight_line(&self, line: &str) -> Vec<Style> {
        let chars: Vec<char> = line.chars().collect();
        let mut styles = vec![self.theme.get("Normal"); chars.len()];
        if chars.is_empty() { return styles; }

        let keywords = ["fn", "let", "mut", "use", "pub", "mod", "match", "if", "else", "loop", "while", "for", "in", "impl", "struct", "enum", "type", "trait", "as", "return", "const", "static", "async", "await", "where", "dyn", "move", "unsafe", "extern", "crate", "self", "Self", "import", "from", "export", "default", "class", "interface", "extends", "implements", "readonly", "private", "protected", "public", "abstract", "override", "virtual", "new", "delete", "throw", "try", "catch", "finally", "instanceof", "typeof", "void", "yield", "package", "namespace", "using", "var", "function", "goto", "break", "continue", "switch", "case", "true", "false", "null", "undefined", "NaN", "Infinity", "this", "super"];
        let builtins = ["String", "Option", "Result", "Some", "None", "Ok", "Err", "Box", "Vec", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "Arc", "Rc", "RefCell", "Mutex", "RwLock", "Console", "Math", "JSON", "Promise", "Object", "Array", "Number", "Boolean", "Symbol", "Error", "Map", "Set", "WeakMap", "WeakSet", "Intl", "WebAssembly", "Global", "Int8Array", "Uint8Array", "Uint8ClampedArray", "Int16Array", "Uint16Array", "Int32Array", "Uint32Array", "Float32Array", "Float64Array", "BigInt64Array", "BigUint64Array"];
        let types = ["i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "bool", "char", "str", "number", "string", "boolean", "any", "unknown", "never", "void", "object", "bigint", "symbol"];

        let mut i = 0;
        while i < chars.len() {
            // Comments
            if chars[i] == '/' && i + 1 < chars.len() && chars[i+1] == '/' {
                let style = self.theme.get("Comment");
                let todo_style = self.theme.get("Todo");
                let special_keywords = ["TODO", "FIXME", "BUG", "HACK", "NOTE"];
                
                let mut j = i;
                while j < chars.len() {
                    let mut found_special = false;
                    for kw in &special_keywords {
                        if j + kw.len() <= chars.len() {
                            let word: String = chars[j..j+kw.len()].iter().collect();
                            if word == *kw {
                                for k in 0..kw.len() { styles[j+k] = todo_style; }
                                j += kw.len();
                                found_special = true;
                                break;
                            }
                        }
                    }
                    if !found_special {
                        styles[j] = style;
                        j += 1;
                    }
                }
                break;
            }

            // Strings (Highest priority after comments)
            if chars[i] == '"' || chars[i] == '\'' || chars[i] == '`' {
                let quote = chars[i];
                let style = self.theme.get("String");
                styles[i] = style;
                i += 1;
                while i < chars.len() && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        styles[i] = style;
                        styles[i+1] = style;
                        i += 2;
                    } else {
                        styles[i] = style;
                        i += 1;
                    }
                }
                if i < chars.len() {
                    styles[i] = style;
                    i += 1;
                }
                continue;
            }

            // Basic HTML/XML Tag and Attribute Highlighting
            if chars[i] == '<' {
                let j = i + 1;
                if j < chars.len() && (chars[j].is_alphabetic() || chars[j] == '/' || chars[j] == '!') {
                    styles[i] = self.theme.get("Keyword"); // <
                    i += 1;
                    
                    // Parse tag name
                    let start = i;
                    while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == ':' || chars[i] == '-' || chars[i] == '/') {
                        i += 1;
                    }
                    let tag_name_style = self.theme.get("Tag");
                    for k in start..i { styles[k] = tag_name_style; }

                    // Parse attributes
                    while i < chars.len() && chars[i] != '>' {
                        if chars[i].is_alphabetic() {
                            let attr_start = i;
                            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '-') {
                                i += 1;
                            }
                            let attr_style = self.theme.get("Attribute");
                            for k in attr_start..i { styles[k] = attr_style; }
                        } else if chars[i] == '"' || chars[i] == '\'' {
                            // Re-use string logic inside tag
                            let quote = chars[i];
                            let s_style = self.theme.get("String");
                            styles[i] = s_style;
                            i += 1;
                            while i < chars.len() && chars[i] != quote {
                                styles[i] = s_style;
                                i += 1;
                            }
                            if i < chars.len() { styles[i] = s_style; i += 1; }
                        } else {
                            i += 1;
                        }
                    }
                    if i < chars.len() && chars[i] == '>' {
                        styles[i] = self.theme.get("Keyword");
                        i += 1;
                    }
                    continue;
                }
            }

            // Numbers
            if chars[i].is_ascii_digit() {
                let style = self.theme.get("Constant");
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == 'x') {
                    styles[i] = style;
                    i += 1;
                }
                continue;
            }

            // Words (Keywords, Types, Functions, etc.)
            if chars[i].is_alphabetic() || chars[i] == '_' || chars[i] == '$' || chars[i] == '@' || chars[i] == '#' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$' || chars[i] == '@' || chars[i] == '#') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                
                let mut style = self.theme.get("Normal");
                if keywords.contains(&word.as_str()) {
                    style = self.theme.get("Keyword");
                } else if word.starts_with('$') || word.starts_with('@') || word.starts_with('#') {
                    style = self.theme.get("Identifier");
                } else if builtins.contains(&word.as_str()) {
                    style = self.theme.get("Type");
                } else if types.contains(&word.as_str()) {
                    style = self.theme.get("Type");
                } else if chars.get(i) == Some(&'(') {
                    style = self.theme.get("Function");
                } else if word.chars().next().unwrap().is_uppercase() {
                    style = self.theme.get("Tag"); // Components like <Header />
                }

                for j in start..i { styles[j] = style; }
                continue;
            }

            // Symbols
            let symbols = ['=', '+', '-', '*', '/', '%', '<', '>', '&', '|', '^', '!', '?', ':', ';', ',', '.', '(', ')', '[', ']', '{', '}'];
            if symbols.contains(&chars[i]) {
                styles[i] = self.theme.get("Normal");
                i += 1;
                continue;
            }

            i += 1;
        }

        styles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn test_highlight_keyword() {
        let theme = ColorScheme::new("catppuccin");
        let highlighter = Highlighter::new(theme);
        let styles = highlighter.highlight_line("fn main() {");
        assert_eq!(styles[0], highlighter.theme.get("Keyword"));
        assert_eq!(styles[1], highlighter.theme.get("Keyword"));
    }

    #[test]
    fn test_gruvbox_material_highlights() {
        let theme = ColorScheme::new("gruvbox-material");
        let highlighter = Highlighter::new(theme);
        let styles = highlighter.highlight_line("fn main() {");
        assert_eq!(styles[0], highlighter.theme.get("Keyword"));
        // Verify it's actually using gruvbox colors
        if let Color::Rgb(r, g, b) = highlighter.theme.palette.purple {
             // gruvbox_material purple is Rgb(211, 134, 155)
             assert_eq!(r, 211);
             assert_eq!(g, 134);
             assert_eq!(b, 155);
        } else {
            panic!("Purple should be Rgb");
        }
    }
}
