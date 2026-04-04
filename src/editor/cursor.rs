pub struct Cursor {
    pub x: usize,
    pub y: usize,
    pub scroll_y: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { x: 0, y: 0, scroll_y: 0 }
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
