use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Resize { width: u16, height: u16 },
    Click { col: u16, row: u16, button: MouseButton },
    Scroll { col: u16, row: u16, direction: ScrollDir },
    Tick,
}

#[derive(Debug, Clone, Copy)]
pub enum ScrollDir { Up, Down }

pub fn translate_key(key: KeyEvent) -> AppEvent {
    AppEvent::Key(key)
}

pub fn translate_mouse(mouse: MouseEvent) -> Option<AppEvent> {
    match mouse.kind {
        MouseEventKind::Down(button) => Some(AppEvent::Click {
            col: mouse.column,
            row: mouse.row,
            button,
        }),
        MouseEventKind::ScrollUp => Some(AppEvent::Scroll {
            col: mouse.column,
            row: mouse.row,
            direction: ScrollDir::Up,
        }),
        MouseEventKind::ScrollDown => Some(AppEvent::Scroll {
            col: mouse.column,
            row: mouse.row,
            direction: ScrollDir::Down,
        }),
        _ => None,
    }
}

pub fn translate_resize(w: u16, h: u16) -> AppEvent {
    AppEvent::Resize { width: w, height: h }
}

pub fn key_to_string(key: &KeyEvent) -> String {
    let ctrl  = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt   = key.modifiers.contains(KeyModifiers::ALT);

    let base = match key.code {
        KeyCode::Char(c)   => {
            if c == ' ' { "Space".into() } else { c.to_string() }
        },
        KeyCode::Enter     => "CR".into(),
        KeyCode::Esc       => "Esc".into(),
        KeyCode::Tab       => "Tab".into(),
        KeyCode::BackTab   => "S-Tab".into(),
        KeyCode::Backspace => "BS".into(),
        KeyCode::Delete    => "Del".into(),
        KeyCode::Up        => "Up".into(),
        KeyCode::Down      => "Down".into(),
        KeyCode::Left      => "Left".into(),
        KeyCode::Right     => "Right".into(),
        KeyCode::Home      => "Home".into(),
        KeyCode::End       => "End".into(),
        KeyCode::PageUp    => "PageUp".into(),
        KeyCode::PageDown  => "PageDown".into(),
        KeyCode::F(n)      => format!("F{n}"),
        _                  => "?".into(),
    };

    let mut mods = String::new();
    if ctrl  { mods.push_str("C-"); }
    if alt   { mods.push_str("A-"); }
    if shift && !matches!(key.code, KeyCode::Char(_)) {
        mods.push_str("S-");
    }

    if mods.is_empty() {
        base
    } else {
        format!("<{}{}>", mods, base)
    }
}
