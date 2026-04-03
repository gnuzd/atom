# Atom IDE

A minimalist, terminal-based IDE built with Rust, Ratatui, and Vim-inspired motions.

## Features

- **Vim-inspired Modes:** Supports `Normal` and `Insert` modes.
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
- `h`, `j`, `k`, `l` or **Arrow Keys**: Move the cursor.
- `q`: Quit the application.

#### Insert Mode

- `Esc`: Return to **Normal** mode.
- **Type**: Input text at the cursor position.
- **Backspace**: Delete characters or merge lines.
- **Enter**: Split lines.
- **Arrow Keys**: Move the cursor while editing.

## Project Structure

- `src/editor/`: Core editor logic (Buffer, Cursor).
- `src/vim/`: Vim state and motion abstractions.
- `src/ui/`: Ratatui-based terminal rendering.
- `src/main.rs`: Application entry point and event loop.

## License

MIT
