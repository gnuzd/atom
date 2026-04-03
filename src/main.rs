pub mod config;
pub mod editor;
pub mod ui;
pub mod vim;

use std::{env, error::Error, io, path::PathBuf, time::Duration};

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use editor::Editor;
use ui::TerminalUi;
use vim::{mode::{Mode, YankType}, VimState, Position};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut editor = Editor::new();
    let mut vim = VimState::new();
    let ui = TerminalUi::new();

    // Handle CLI arguments - Open multiple files
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        // Clear initial empty buffer
        editor.buffers.clear();
        editor.cursors.clear();
        
        for arg in &args[1..] {
            let path = PathBuf::from(arg);
            if path.exists() {
                let _ = editor.open_file(path);
            } else {
                let mut new_buffer = editor::buffer::Buffer::new();
                new_buffer.file_path = Some(path);
                editor.buffers.push(new_buffer);
                editor.cursors.push(editor::cursor::Cursor::new());
            }
        }
        editor.active_idx = 0;
    } else {
        // Welcome message
        let active_buffer = editor.buffer_mut();
        active_buffer.lines = vec![
            "Welcome to Atom IDE!".to_string(),
            "Press 'i' for Insert mode, 'v' for Visual mode.".to_string(),
            "Press 'yy' to copy line, 'p/P' to paste.".to_string(),
            "Press 'PgUp/PgDn' to jump to start/end of line.".to_string(),
            "Commands: :bn (next buffer), :bp (prev), :bd (close), :e <file> (open).".to_string(),
        ];
    }

    let mut flash_counter = 0;

    loop {
        // Set cursor style based on mode
        match vim.mode {
            Mode::Insert => execute!(terminal.backend_mut(), SetCursorStyle::SteadyBar)?,
            _ => execute!(terminal.backend_mut(), SetCursorStyle::SteadyBlock)?,
        }

        terminal.draw(|f| ui.draw(f, &editor, &vim))?;

        // Clear yank highlight after some iterations (brief flash)
        if vim.yank_highlight_line.is_some() {
            if flash_counter > 5 {
                vim.yank_highlight_line = None;
                flash_counter = 0;
            } else {
                flash_counter += 1;
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Any key press clears the highlight immediately
                vim.yank_highlight_line = None;
                flash_counter = 0;

                match vim.mode {
                    Mode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('i') => {
                            editor.buffer_mut().push_history();
                            vim.mode = Mode::Insert;
                        },
                        KeyCode::Char('v') => {
                            vim.mode = Mode::Visual;
                            let cursor = editor.cursor();
                            vim.selection_start = Some(Position { x: cursor.x, y: cursor.y });
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
                        KeyCode::Char('e') => editor.move_word_end(),
                        KeyCode::Char('o') => {
                            editor.open_line_below();
                            vim.mode = Mode::Insert;
                        }
                        KeyCode::Char('O') => {
                            editor.open_line_above();
                            vim.mode = Mode::Insert;
                        }
                        KeyCode::Char('p') => editor.paste_after(&vim.register, vim.yank_type),
                        KeyCode::Char('P') => editor.paste_before(&vim.register, vim.yank_type),
                        KeyCode::Char('y') => {
                            if vim.pending_op == Some('y') {
                                // yy logic
                                let cursor_y = editor.cursor().y;
                                vim.register = editor.buffer().lines[cursor_y].clone();
                                vim.yank_type = YankType::Line;
                                vim.pending_op = None;
                                // Visual feedback
                                vim.yank_highlight_line = Some(cursor_y);
                                flash_counter = 0;
                            } else {
                                vim.pending_op = Some('y');
                            }
                        }
                        KeyCode::PageUp => editor.move_to_line_start(),
                        KeyCode::PageDown => editor.move_to_line_end(),
                        KeyCode::Char('j') | KeyCode::Down => {
                            vim.pending_op = None;
                            editor.move_down();
                        },
                        KeyCode::Char('k') | KeyCode::Up => {
                            vim.pending_op = None;
                            editor.move_up();
                        },
                        KeyCode::Char('h') | KeyCode::Left => {
                            vim.pending_op = None;
                            editor.move_left();
                        },
                        KeyCode::Char('l') | KeyCode::Right => {
                            vim.pending_op = None;
                            editor.move_right();
                        },
                        _ => {
                            vim.pending_op = None;
                        }
                    },
                    Mode::Visual => match key.code {
                        KeyCode::Esc => {
                            vim.mode = Mode::Normal;
                            vim.selection_start = None;
                        }
                        KeyCode::Char('y') => {
                            if let Some(start) = vim.selection_start {
                                let cursor = editor.cursor();
                                vim.register = editor.yank(start.x, start.y, cursor.x, cursor.y);
                                vim.yank_type = YankType::Char;
                            }
                            vim.mode = Mode::Normal;
                            vim.selection_start = None;
                        }
                        KeyCode::Char('d') | KeyCode::Char('x') => {
                            if let Some(start) = vim.selection_start {
                                let cursor = editor.cursor();
                                vim.register = editor.delete_selection(start.x, start.y, cursor.x, cursor.y);
                                vim.yank_type = YankType::Char;
                            }
                            vim.mode = Mode::Normal;
                            vim.selection_start = None;
                        }
                        KeyCode::Char('w') => editor.move_word_forward(),
                        KeyCode::Char('b') => editor.move_word_backward(),
                        KeyCode::Char('e') => editor.move_word_end(),
                        KeyCode::PageUp => editor.move_to_line_start(),
                        KeyCode::PageDown => editor.move_to_line_end(),
                        KeyCode::Char('j') | KeyCode::Down => editor.move_down(),
                        KeyCode::Char('k') | KeyCode::Up => editor.move_up(),
                        KeyCode::Char('h') | KeyCode::Left => editor.move_left(),
                        KeyCode::Char('l') | KeyCode::Right => editor.move_right(),
                        _ => {}
                    },
                    Mode::Insert => match key.code {
                        KeyCode::Esc => {
                            vim.mode = Mode::Normal;
                        }
                        KeyCode::Up => editor.move_up(),
                        KeyCode::Down => editor.move_down(),
                        KeyCode::Left => editor.move_left(),
                        KeyCode::Right => editor.move_right(),
                        KeyCode::PageUp => editor.move_to_line_start(),
                        KeyCode::PageDown => editor.move_to_line_end(),
                        KeyCode::Char(c) => {
                            let cursor = editor.cursor();
                            let cursor_y = cursor.y;
                            let cursor_x = cursor.x;
                            let line = &mut editor.buffer_mut().lines[cursor_y];
                            line.insert(cursor_x, c);
                            editor.cursor_mut().x += 1;
                        }
                        KeyCode::Backspace => {
                            let cursor = editor.cursor();
                            let cursor_x = cursor.x;
                            let cursor_y = cursor.y;
                            if cursor_x > 0 {
                                let line = &mut editor.buffer_mut().lines[cursor_y];
                                line.remove(cursor_x - 1);
                                editor.cursor_mut().x -= 1;
                            } else if cursor_y > 0 {
                                let current_line = editor.buffer_mut().lines.remove(cursor_y);
                                editor.cursor_mut().y -= 1;
                                let prev_y = editor.cursor().y;
                                let prev_line = &mut editor.buffer_mut().lines[prev_y];
                                let new_x = prev_line.len();
                                prev_line.push_str(&current_line);
                                editor.cursor_mut().x = new_x;
                            }
                        }
                        KeyCode::Enter => {
                            let cursor = editor.cursor();
                            let cursor_y = cursor.y;
                            let cursor_x = cursor.x;
                            let line = &mut editor.buffer_mut().lines[cursor_y];
                            let new_line = line.split_off(cursor_x);
                            editor.buffer_mut().lines.insert(cursor_y + 1, new_line);
                            editor.cursor_mut().y += 1;
                            editor.cursor_mut().x = 0;
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
                                    "bn" | "bnext" => editor.next_buffer(),
                                    "bp" | "bprev" => editor.prev_buffer(),
                                    "bd" | "bdelete" => editor.close_current_buffer(),
                                    "e" | "edit" => {
                                        if cmd_parts.len() > 1 {
                                            let path = PathBuf::from(cmd_parts[1]);
                                            let _ = editor.open_file(path);
                                        }
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
