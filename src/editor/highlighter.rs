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

            // Strings
            if chars[i] == '"' || chars[i] == '\'' || chars[i] == '`' {
                let quote = chars[i];
                let start = i;
                styles[i] = self.theme.get("String");
                i += 1;
                while i < chars.len() && chars[i] != quote {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        styles[i] = self.theme.get("Constant");
                        styles[i+1] = self.theme.get("Constant");
                        i += 2;
                    } else {
                        styles[i] = self.theme.get("String");
                        i += 1;
                    }
                }
                if i < chars.len() {
                    styles[i] = self.theme.get("String");
                    i += 1;
                }
                continue;
            }

            // Numbers
            if chars[i].is_ascii_digit() {
                let style = self.theme.get("Constant");
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_' || chars[i] == 'x' || chars[i] == 'b' || chars[i] == 'o' || (chars[i] >= 'a' && chars[i] <= 'f') || (chars[i] >= 'A' && chars[i] <= 'F')) {
                    styles[i] = style;
                    i += 1;
                }
                continue;
            }

            // Words (Keywords, Types, Functions, etc.)
            if chars[i].is_alphabetic() || chars[i] == '_' || chars[i] == '$' {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                
                let mut style = self.theme.get("Normal");
                if keywords.contains(&word.as_str()) {
                    style = self.theme.get("Keyword");
                } else if builtins.contains(&word.as_str()) {
                    style = self.theme.get("Function");
                } else if types.contains(&word.as_str()) {
                    style = self.theme.get("Type");
                } else if chars.get(i) == Some(&'(') {
                    style = self.theme.get("Function");
                } else if word.chars().next().unwrap().is_uppercase() {
                    style = self.theme.get("Type");
                } else if word.to_uppercase() == word && word.len() > 1 && word.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    style = self.theme.get("Constant");
                }

                for j in start..i { styles[j] = style; }
                continue;
            }

            // Symbols
            let symbols = ['=', '+', '-', '*', '/', '%', '<', '>', '&', '|', '^', '!', '?', ':', ';', ',', '.', '(', ')', '[', ']', '{', '}'];
            if symbols.contains(&chars[i]) {
                styles[i] = self.theme.get("Keyword"); // Using Keyword color for symbols often looks good
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

    #[test]
    fn test_highlight_keyword() {
        let theme = ColorScheme::new("catppuccin");
        let highlighter = Highlighter::new(theme);
        let styles = highlighter.highlight_line("fn main() {");
        assert_eq!(styles[0], highlighter.theme.get("Keyword"));
        assert_eq!(styles[1], highlighter.theme.get("Keyword"));
    }
}
