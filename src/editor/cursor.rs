pub struct Cursor {
    pub x: usize,
    pub y: usize,
    pub scroll_y: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { x: 0, y: 0, scroll_y: 0 }
    }

    pub fn character_idx_from_utf16(&mut self, line: &str, utf16_offset: usize) {
        let mut current_utf16 = 0;
        let mut char_idx = 0;
        for c in line.chars() {
            if current_utf16 >= utf16_offset {
                break;
            }
            current_utf16 += c.len_utf16();
            if current_utf16 <= utf16_offset {
                char_idx += 1;
            }
        }
        self.x = char_idx;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_new() {
        let cursor = Cursor::new();
        assert_eq!(cursor.x, 0);
        assert_eq!(cursor.y, 0);
        assert_eq!(cursor.scroll_y, 0);
    }
}
