# Atom IDE

A minimalist, terminal-based IDE built with Rust, Ratatui, and Vim-inspired motions.

## Features

- **Vim-inspired Modes:** Supports `Normal`, `Insert`, `Visual`, and `Command` modes.
- **LSP Integration:**
    - **Automatic Installation:** Built-in LSP manager (`:Mason`) to install and track language servers.
    - **Auto-completion:** Real-time suggestions with documentation tooltips.
    - **Status Tracking:** Real-time feedback on LSP status (Loading, Installing, Ready).
- **Advanced Formatting:**
    - **Multi-language Support:** Out-of-the-box support for Rust (`rustfmt`), Lua (`stylua`), and Web/JS stack (Prettier).
    - **High Performance:** Multi-layer caching (Project Root, Local Binary, and Not Found caches) for near-instant formatting.
    - **Auto-format:** Automatically formats files on open and save.
- **File Explorer:** Integrated file tree with support for adding, renaming, moving, and deleting files.
- **Minimalist UI:** Clean, modern design with Catppuccin-inspired colors and dynamic notifications.
- **Vim Motions:** Supports word-wise movement (`w`, `b`, `e`), line operations (`o`, `O`, `dd`, `yy`), and undo/redo.

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)
- Optional: `rustfmt`, `prettier`, or `stylua` for formatting.

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
- `\`: Toggle/Focus **File Explorer**.
- `?`: Toggle **Keymaps Help**.
- `h`, `j`, `k`, `l` or **Arrow Keys**: Move the cursor.
- `w`, `b`, `e`: Move forward/backward by word (start/end).
- `o`, `O`: Open new line below/above and enter Insert mode.
- `p`, `P`: Paste yanked text after/before the cursor.
- `u`: Undo change.
- `Ctrl-r`: Redo change.
- `q`: Quit the application.

#### File Explorer

- `j/k`: Navigate entries.
- `l / Enter`: Expand folder or open file.
- `h`: Collapse folder.
- `a`: Add new file/folder.
- `r`: Rename selected.
- `d`: Delete selected (with confirmation).
- `/`: Filter files.

#### Command Mode

- `:w`: Save and Format current file.
- `:Format`: Manually trigger formatter.
- `:FormatAll`: Format all open buffers.
- `:FormatEnable / :FormatDisable`: Toggle auto-formatting.
- `:Mason`: Open LSP Manager.
- `:bn / :bp`: Next/Previous buffer.
- `:bd`: Close current buffer.
- `:e <path>`: Open/Edit a file.
- `:q`: Quit the application.

## Project Structure

- `src/editor/`: Core editor logic (Buffer, Cursor, Highlighter).
- `src/lsp/`: LSP client and Manager (Formatting, Installation).
- `src/vim/`: Vim state, modes, and motion abstractions.
- `src/ui/`: Ratatui-based UI components (Explorer, Statusline, CMP).

## Roadmap / TODO

### Core Editor
- [ ] Improved Syntax Highlighting (Tree-sitter integration)
- [ ] Project-wide Search (Grep/Ripgrep integration)
- [ ] Search & Replace (:s/old/new/g)
- [ ] Command History (Up/Down in `:` mode)
- [ ] Indentation support (Auto-indent, Tab/Shift-Tab)
- [ ] Horizontal scrolling for long lines

### LSP & Tooling
- [x] Auto-completion
- [ ] Diagnostics (inline errors/warnings)
- [ ] Go to definition / References
- [ ] Hover documentation (expanded)
- [x] Multi-layer caching for Formatters

### UI/UX
- [x] Floating Windows for CMP/Documentation
- [ ] Customizable Color Schemes (Themes)
- [ ] Customizable Keybindings
- [ ] Tab bar for open buffers
- [x] Command Line Notifications

### Miscellaneous
- [ ] Plugin system (Lua-based)
- [ ] Session management (restore open buffers)
- [ ] Configuration file support (`atom.toml`)

## License

MIT
