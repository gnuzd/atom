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
        let words: Vec<&str> = vec!["fn", "let", "mut", "use", "pub", "mod", "match", "if", "else", "loop", "while", "for", "in", "impl", "struct", "enum", "type", "trait", "as", "return", "true", "false"];

        // Simple comment check
        if let Some(comment_start) = line.find("//") {
            let style = self.theme.get("Comment");
            for i in comment_start..line.len() {
                styles[i] = style;
            }
        }

        // Simple keyword check
        let mut current_word = String::new();
        let mut word_start = 0;

        for (i, c) in line.chars().enumerate() {
            if c.is_alphanumeric() || c == '_' {
                if current_word.is_empty() { word_start = i; }
                current_word.push(c);
            } else {
                if !current_word.is_empty() {
                    if words.contains(&current_word.as_str()) {
                        let style = self.theme.get("Keyword");
                        for j in word_start..i { styles[j] = style; }
                    }
                    current_word.clear();
                }
            }
        }
        if !current_word.is_empty() && words.contains(&current_word.as_str()) {
            let style = self.theme.get("Keyword");
            for j in word_start..line.len() { styles[j] = style; }
        }

        // Simple string check
        let mut in_string = false;
        let string_style = self.theme.get("String");
        for (i, c) in line.chars().enumerate() {
            if c == '"' {
                styles[i] = string_style;
                in_string = !in_string;
            } else if in_string {
                styles[i] = string_style;
            }
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
