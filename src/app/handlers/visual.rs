use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub fn handle_visual_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.dispatch_action(Action::ExitMode, 1),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.dispatch_action(Action::Save, 1)
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.dispatch_action(Action::CopyToClipboard, 1)
            }
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.dispatch_action(Action::PasteFromClipboard, 1)
            }
            KeyCode::Char('j') | KeyCode::Down => self.dispatch_action(Action::MoveDown, 1),
            KeyCode::Char('k') | KeyCode::Up => self.dispatch_action(Action::MoveUp, 1),
            KeyCode::Char('h') | KeyCode::Left => self.dispatch_action(Action::MoveLeft, 1),
            KeyCode::Char('l') | KeyCode::Right => self.dispatch_action(Action::MoveRight, 1),
            KeyCode::PageUp | KeyCode::Home => self.dispatch_action(Action::MoveLineStart, 1),
            KeyCode::PageDown | KeyCode::End => self.dispatch_action(Action::MoveLineEnd, 1),
            KeyCode::Char('w') => self.dispatch_action(Action::MoveWordForward, 1),
            KeyCode::Char('b') => self.dispatch_action(Action::MoveWordBackward, 1),
            KeyCode::Char('p') => self.dispatch_action(Action::PasteAfter, 1),
            KeyCode::Char('s') => self.dispatch_action(Action::Substitute, 1),
            KeyCode::Char('y') => self.dispatch_action(Action::YankLine, 1),
            KeyCode::Char('d') | KeyCode::Char('x') => {
                self.dispatch_action(Action::DeleteSelection, 1)
            }
            _ => {}
        }
    }
}
