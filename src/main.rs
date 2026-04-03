pub mod config;
pub mod editor;
pub mod ui;
pub mod vim;

use std::{env, error::Error, io, path::PathBuf};

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use editor::Editor;
use ui::TerminalUi;
use vim::{mode::Mode, VimState, Position};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut editor = Editor::new();
    let mut vim = VimState::new();
    let ui = TerminalUi::new();

    // Handle CLI arguments
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        let path = PathBuf::from(&args[1]);
        if path.exists() {
            editor.open_file(path)?;
        } else {
            // New file
            editor.buffer.file_path = Some(path);
        }
    } else {
        // Welcome message for empty buffer
        editor.buffer.lines = vec![
            "Welcome to Atom IDE!".to_string(),
            "Press 'i' for Insert mode, 'v' for Visual mode.".to_string(),
            "Press 'w/b' for words, 'y' to yank (Visual), 'p' to paste.".to_string(),
            "Press '/' to search, 'u' to undo, 'Ctrl-r' to redo.".to_string(),
        ];
    }

    loop {
        // Set cursor style based on mode
        match vim.mode {
            Mode::Insert => execute!(terminal.backend_mut(), SetCursorStyle::SteadyBar)?,
            _ => execute!(terminal.backend_mut(), SetCursorStyle::SteadyBlock)?,
        }

        terminal.draw(|f| ui.draw(f, &editor, &vim))?;

        if let Event::Key(key) = event::read()? {
            match vim.mode {
                Mode::Normal => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('i') => {
                        editor.buffer.push_history();
                        vim.mode = Mode::Insert;
                    },
                    KeyCode::Char('v') => {
                        vim.mode = Mode::Visual;
                        vim.selection_start = Some(Position { x: editor.cursor.x, y: editor.cursor.y });
                    }
                    KeyCode::Char(':') => {
                        vim.mode = Mode::Command;
                        vim.command_buffer.clear();
                    }
                    KeyCode::Char('/') => {
                        vim.mode = Mode::Search;
                        vim.search_query.clear();
                    }
                    KeyCode::Char('u') => {
                        editor.undo();
                    }
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        editor.redo();
                    }
                    KeyCode::Char('w') => editor.move_word_forward(),
                    KeyCode::Char('b') => editor.move_word_backward(),
                    KeyCode::Char('p') => editor.paste(&vim.register),
                    KeyCode::Char('j') | KeyCode::Down => editor.move_down(),
                    KeyCode::Char('k') | KeyCode::Up => editor.move_up(),
                    KeyCode::Char('h') | KeyCode::Left => editor.move_left(),
                    KeyCode::Char('l') | KeyCode::Right => editor.move_right(),
                    _ => {}
                },
                Mode::Visual => match key.code {
                    KeyCode::Esc => {
                        vim.mode = Mode::Normal;
                        vim.selection_start = None;
                    }
                    KeyCode::Char('y') => {
                        if let Some(start) = vim.selection_start {
                            vim.register = editor.yank(start.x, start.y, editor.cursor.x, editor.cursor.y);
                        }
                        vim.mode = Mode::Normal;
                        vim.selection_start = None;
                    }
                    KeyCode::Char('w') => editor.move_word_forward(),
                    KeyCode::Char('b') => editor.move_word_backward(),
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
                Mode::Command => match key.code {
                    KeyCode::Esc => {
                        vim.mode = Mode::Normal;
                        vim.command_buffer.clear();
                    }
                    KeyCode::Char(c) => {
                        vim.command_buffer.push(c);
                    }
                    KeyCode::Backspace => {
                        if vim.command_buffer.is_empty() {
                            vim.mode = Mode::Normal;
                        } else {
                            vim.command_buffer.pop();
                        }
                    }
                    KeyCode::Enter => {
                        let cmd_parts: Vec<&str> = vim.command_buffer.split_whitespace().collect();
                        if !cmd_parts.is_empty() {
                            match cmd_parts[0] {
                                "q" | "quit" => break,
                                "w" | "write" => {
                                    if cmd_parts.len() > 1 {
                                        let path = PathBuf::from(cmd_parts[1]);
                                        let _ = editor.save_file_as(path);
                                    } else {
                                        let _ = editor.save_file();
                                    }
                                }
                                "wq" => {
                                    let _ = editor.save_file();
                                    break;
                                }
                                _ => {}
                            }
                        }
                        vim.mode = Mode::Normal;
                        vim.command_buffer.clear();
                    }
                    _ => {}
                },
                Mode::Search => match key.code {
                    KeyCode::Esc => {
                        vim.mode = Mode::Normal;
                        vim.search_query.clear();
                    }
                    KeyCode::Char(c) => {
                        vim.search_query.push(c);
                    }
                    KeyCode::Backspace => {
                        if vim.search_query.is_empty() {
                            vim.mode = Mode::Normal;
                        } else {
                            vim.search_query.pop();
                        }
                    }
                    KeyCode::Enter => {
                        vim.mode = Mode::Normal;
                    }
                    _ => {}
                },
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        SetCursorStyle::DefaultUserShape
    )?;
    terminal.show_cursor()?;

    Ok(())
}
