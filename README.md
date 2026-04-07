<p align="center">
  <img src="https://raw.githubusercontent.com/gnuzd/atom/main/assets/logo.png" alt="Atom Logo" width="200" />
</p>

<h1 align="center">Atom</h1>

<p align="center">
  <strong>A lightning-fast, modal terminal editor written in Rust.</strong>
</p>

<p align="center">
  <a href="https://github.com/gnuzd/atom/releases"><img src="https://img.shields.io/github/v/release/gnuzd/atom?style=flat-square" alt="GitHub release" /></a>
  <a href="https://github.com/gnuzd/atom/blob/main/LICENSE"><img src="https://img.shields.io/github/license/gnuzd/atom?style=flat-square" alt="License" /></a>
  <a href="https://github.com/gnuzd/atom/actions"><img src="https://img.shields.io/github/actions/workflow/status/gnuzd/atom/rust.yml?branch=main&style=flat-square" alt="GitHub Actions" /></a>
</p>

---

Atom is a modern, Vim-inspired terminal editor designed for speed and productivity. It combines the best of traditional modal editing with contemporary features like LSP, Tree-sitter, and a built-in package manager.

📚 **[Documentation Site](https://gnuzd.github.io/atom)** | 🚀 **[Installation Guide](https://gnuzd.github.io/atom/installation)** | 📋 **[Prerequisites](https://gnuzd.github.io/atom/prerequisites)**

## ✨ Features

- ⌨️ **Vim-inspired Modal Editing**: Powerful motions and operators for efficient text manipulation.
- 🛠️ **Mason-like Package Manager**: Manage LSPs, DAPs, linters, and formatters with an intuitive UI.
- 🔭 **Telescope-like Fuzzy Finder**: Quickly find files, search text, and navigate your project.
- 🌳 **Tree-sitter Integration**: High-performance, language-aware syntax highlighting and indentation.
- 🚀 **Native LSP Support**: Auto-completion, diagnostics, go-to-definition, and refactoring out of the box.
- 📁 **Integrated File Explorer**: Navigate your project structure without leaving the editor.
- ⚠️ **Trouble List**: A centralized view for project-wide diagnostics and warnings.
- 🌿 **Git Integration**: Real-time branch status and file changes in the status line.
- 🎨 **Beautiful UI**: Modern aesthetics with rounded borders, icons, and customizable themes.

## 🚀 Installation

### Using Homebrew (Recommended)

```bash
brew tap gnuzd/tap
brew install atom
```

### From Source

Ensure you have [Rust](https://www.rust-lang.org/tools/install) installed.

```bash
git clone https://github.com/gnuzd/atom.git
cd atom
cargo build --release
```

The binary will be available at `target/release/atom`.

## 🛠️ Getting Started

Simply run `atom` in your terminal:

```bash
atom [file or directory]
```

### Basic Keybindings

- `i` - Insert mode
- `v` - Visual mode
- `Esc` - Back to Normal mode
- `:w` - Save file
- `:q` - Quit
- `<Space>ff` - Find files (Telescope)
- `<Space>e` - Toggle File Explorer
- `<Space>m` - Open Mason (Package Manager)

## ⚙️ Customization

Atom is highly configurable. Configuration files are located in `~/.config/atom/` (or platform equivalent).

- `config.toml`: General editor settings.
- `colorscheme.toml`: Customize your editor's look.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 📄 License

Distributed under the MIT License. See `LICENSE` for more information.

---

<p align="center">
  Made with ❤️ by <a href="https://github.com/gnuzd">gnuzd</a>
</p>
