use ratatui::style::Style;
use crate::ui::colorscheme::ColorScheme;

pub enum SyntaxKind {
    Normal,
    Keyword,
    String,
    Comment,
    Number,
    Function,
    Type,
}

pub struct Highlighter {
    pub colors: ColorScheme,
}

impl Highlighter {
    pub fn new(colors: ColorScheme) -> Self {
        Self { colors }
    }

    pub fn highlight_line(&self, line: &str) -> Vec<Style> {
        let mut styles = vec![self.colors.normal; line.len()];
        let words: Vec<&str> = vec!["fn", "let", "mut", "use", "pub", "mod", "match", "if", "else", "loop", "while", "for", "in", "impl", "struct", "enum", "type", "trait", "as", "return", "true", "false"];

        // Simple comment check
        if let Some(comment_start) = line.find("//") {
            for i in comment_start..line.len() {
                styles[i] = self.colors.comment;
            }
        }

        // Simple keyword check (approximate)
        let mut current_word = String::new();
        let mut word_start = 0;

        for (i, c) in line.chars().enumerate() {
            if c.is_alphanumeric() || c == '_' {
                if current_word.is_empty() {
                    word_start = i;
                }
                current_word.push(c);
            } else {
                if !current_word.is_empty() {
                    if words.contains(&current_word.as_str()) {
                        for j in word_start..i {
                            styles[j] = self.colors.keyword;
                        }
                    }
                    current_word.clear();
                }
            }
        }
        
        // Check last word
        if !current_word.is_empty() {
            if words.contains(&current_word.as_str()) {
                for j in word_start..line.len() {
                    styles[j] = self.colors.keyword;
                }
            }
        }

        // Simple string check
        let mut in_string = false;
        for (i, c) in line.chars().enumerate() {
            if c == '"' {
                styles[i] = self.colors.string;
                in_string = !in_string;
            } else if in_string {
                styles[i] = self.colors.string;
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
        let colors = ColorScheme::default_dark();
        let highlighter = Highlighter::new(colors);
        let styles = highlighter.highlight_line("fn main() {");
        assert_eq!(styles[0], highlighter.colors.keyword);
        assert_eq!(styles[1], highlighter.colors.keyword);
        assert_eq!(styles[2], highlighter.colors.normal); // space
    }
}
