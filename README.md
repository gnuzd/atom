# Atom IDE

A minimalist, terminal-based IDE built with Rust, Ratatui, and Vim-inspired motions.

## Features

- **Vim-inspired Modes:** Supports `Normal`, `Insert`, and `Command` modes.
- **File I/O:** Open files via CLI and save changes using `:w` commands.
- **Dynamic Cursor:** Uses a thin bar cursor in `Insert` mode and a block cursor in `Normal/Command` modes.
- **Minimalist UI:** Borderless, clean design focused on your code.
- **Navigation:** Supports both classic `hjkl` and standard arrow key movement.
- **Dynamic Status Line:** Real-time feedback on current mode and cursor position.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)

### Installation

Clone the repository and run using Cargo:

```bash
cargo run
```

### Controls

#### Normal Mode (Default)

- `i`: Enter **Insert** mode.
- `v`: Enter **Visual** mode.
- `: `: Enter **Command** mode.
- `h`, `j`, `k`, `l` or **Arrow Keys**: Move the cursor.
- `w`, `b`, `e`: Move forward/backward by word (start/end).
- `o`, `O`: Open new line below/above and enter Insert mode.
- `p`, `P`: Paste yanked text after/before the cursor.
- `u`: Undo change.
- `Ctrl-r`: Redo change.
- `q`: Quit the application (or `:q` in Command mode).

#### Insert Mode

- `Esc`: Return to **Normal** mode.
- **Type**: Input text at the cursor position.
- **Backspace**: Delete characters or merge lines.
- **Enter**: Split lines.
- **Arrow Keys**: Move the cursor while editing.

#### Visual Mode

- `Esc`: Return to **Normal** mode.
- `h`, `j`, `k`, `l`, `w`, `b`: Expand selection.
- `y`: Yank selection to register.

#### Command Mode

- `Esc`: Return to **Normal** mode.
- `q` or `quit` followed by `Enter`: Quit the application.
- `w` or `write` followed by `Enter`: Save the current file.
- `w <filename>` followed by `Enter`: Save the current buffer to a specific file.
- `wq` followed by `Enter`: Save and quit.
- `Backspace`: Delete characters or exit to Normal mode if empty.

## Project Structure

- `src/editor/`: Core editor logic (Buffer, Cursor).
- `src/vim/`: Vim state and motion abstractions.
- `src/ui/`: Ratatui-based terminal rendering.
- `src/main.rs`: Application entry point and event loop.

## License

MIT
