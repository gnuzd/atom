pub mod config;
pub mod editor;
pub mod ui;
pub mod vim;

use std::{error::Error, io};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use editor::Editor;
use ui::TerminalUi;
use vim::{mode::Mode, VimState};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut editor = Editor::new();
    let mut vim = VimState::new();
    let ui = TerminalUi::new();

    editor.buffer.lines = vec![
        "Welcome to Atom IDE!".to_string(),
        "Press 'i' for Insert mode, 'Esc' for Normal mode.".to_string(),
        "Use hjkl or Arrow keys to move around.".to_string(),
    ];

    loop {
        terminal.draw(|f| ui.draw(f, &editor, &vim))?;

        if let Event::Key(key) = event::read()? {
            match vim.mode {
                Mode::Normal => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('i') => vim.mode = Mode::Insert,
                    KeyCode::Char('j') | KeyCode::Down => editor.move_down(),
                    KeyCode::Char('k') | KeyCode::Up => editor.move_up(),
                    KeyCode::Char('h') | KeyCode::Left => editor.move_left(),
                    KeyCode::Char('l') | KeyCode::Right => editor.move_right(),
                    _ => {}
                },
                Mode::Insert => match key.code {
                    KeyCode::Esc => {
                        vim.mode = Mode::Normal;
                        if editor.cursor.x > 0 {
                            editor.cursor.x -= 1;
                        }
                    }
                    KeyCode::Up => editor.move_up(),
                    KeyCode::Down => editor.move_down(),
                    KeyCode::Left => editor.move_left(),
                    KeyCode::Right => editor.move_right(),
                    KeyCode::Char(c) => {
                        let line = &mut editor.buffer.lines[editor.cursor.y];
                        line.insert(editor.cursor.x, c);
                        editor.cursor.x += 1;
                    }
                    KeyCode::Backspace => {
                        if editor.cursor.x > 0 {
                            let line = &mut editor.buffer.lines[editor.cursor.y];
                            line.remove(editor.cursor.x - 1);
                            editor.cursor.x -= 1;
                        } else if editor.cursor.y > 0 {
                            let current_line = editor.buffer.lines.remove(editor.cursor.y);
                            editor.cursor.y -= 1;
                            let prev_line = &mut editor.buffer.lines[editor.cursor.y];
                            editor.cursor.x = prev_line.len();
                            prev_line.push_str(&current_line);
                        }
                    }
                    KeyCode::Enter => {
                        let line = &mut editor.buffer.lines[editor.cursor.y];
                        let new_line = line.split_off(editor.cursor.x);
                        editor.buffer.lines.insert(editor.cursor.y + 1, new_line);
                        editor.cursor.y += 1;
                        editor.cursor.x = 0;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
