use ratatui::style::Style;
use crate::ui::colorscheme::ColorScheme;

pub struct Highlighter {
    pub theme: ColorScheme,
}

impl Highlighter {
    pub fn new(theme: ColorScheme) -> Self {
        Self { theme }
    }

    pub fn highlight_line(&self, line: &str) -> Vec<Style> {
        let mut styles = vec![self.theme.get("Normal"); line.len()];
        if line.is_empty() { return styles; }

        let keywords = ["fn", "let", "mut", "use", "pub", "mod", "match", "if", "else", "loop", "while", "for", "in", "impl", "struct", "enum", "type", "trait", "as", "return", "const", "static", "async", "await", "where", "dyn", "move", "unsafe", "extern", "crate", "self", "Self", "import", "from", "export", "default", "class", "interface", "extends", "implements", "readonly", "private", "protected", "public", "abstract", "override", "virtual", "new", "delete", "throw", "try", "catch", "finally", "instanceof", "typeof", "void", "yield", "package", "namespace", "using", "var", "function", "goto", "break", "continue", "switch", "case", "true", "false", "null", "undefined", "NaN", "Infinity", "this", "super"];
        let builtins = ["String", "Option", "Result", "Some", "None", "Ok", "Err", "Box", "Vec", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "Arc", "Rc", "RefCell", "Mutex", "RwLock", "Console", "Math", "JSON", "Promise", "Object", "Array", "Number", "Boolean", "Symbol", "Error", "Map", "Set", "WeakMap", "WeakSet", "Intl", "WebAssembly", "Global", "Int8Array", "Uint8Array", "Uint8ClampedArray", "Int16Array", "Uint16Array", "Int32Array", "Uint32Array", "Float32Array", "Float64Array", "BigInt64Array", "BigUint64Array"];
        let types = ["i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "bool", "char", "str", "number", "string", "boolean", "any", "unknown", "never", "void", "object", "bigint", "symbol"];

        let mut i = 0;
        let chars: Vec<char> = line.chars().collect();

        while i < chars.len() {
            // Comments
            if chars[i] == '/' && i + 1 < chars.len() && chars[i+1] == '/' {
                let style = self.theme.get("Comment");
                for j in i..chars.len() { styles[j] = style; }
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
                    style = self.theme.get("Identifier"); // Using red for keywords as per screenshot
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
