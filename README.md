# Satellite 🛰️

**Satellite** is a fast, highly-concurrent, visually polished Terminal UI (TUI) wrapper for your system's global package managers. It aggregates packages from almost every major ecosystem into a single unified terminal dashboard. 

If you are setting up a new machine, managing a sprawling dev environment, or just keeping your global tools up to date, Satellite makes it frictionless.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-Windows-lightgrey.svg)
![Language](https://img.shields.io/badge/language-Rust-orange.svg)

## 🚀 Features

- **Universal Support**: Seamlessly reads and executes updates for `winget`, `npm`, `pnpm`, `pip`, `uv`, `cargo`, `choco`, `scoop`, `gem`, and `dotnet`.
- **Blazing Fast Concurrent Fetching**: Uses multi-threading to scan all 10 package managers concurrently the exact millisecond the app opens, preventing UI blocking.
- **Two-Tier Dashboard**: 
  - *Tier 1*: See all supported ecosystems and exactly how many packages you have installed in each.
  - *Tier 2*: Dive into an ecosystem, view versions, and batch-select packages to manage.
- **Batch Actions**: Select multiple packages with the Spacebar and execute a bulk Update (`u`) or Delete (`d`) across them natively in your terminal.
- **Single Binary**: Written purely in Rust using `ratatui` for lightweight distribution.

## 🛠️ Getting Started

### Prerequisites
- [Rust](https://rustup.rs/) (to compile from source).
- Windows Terminal (recommended for the best color rendering).

### Installation & Usage
You can install Satellite directly from crates.io:

```bash
cargo install sat-cli
```

Once installed, simply launch the dashboard from anywhere in your terminal by typing:
```bash
sat
```

## 🎮 Controls

### Global 
- `q`: Quit application

### Managers View (Home)
- `j` / `k` or `Up` / `Down` : Navigate the package manager list
- `Enter` : Enter a package manager to view its installed packages

### Packages View
- `j` / `k` or `Up` / `Down` : Navigate the packages list
- `Space` : Toggle selection for batch operations
- `u` : **Update** selected packages
- `d` : **Delete / Uninstall** selected packages
- `Esc` or `Backspace` : Go back to the Managers View

## 📦 Supported Package Managers

| Package Manager | Language / Ecosystem | Windows Execution Strategy |
| --------------- | -------------------- | ------------------------- |
| **Winget**      | Windows Native       | `winget list`             |
| **Pip**         | Python               | `pip list --format=json`  |
| **uv**          | Modern Python        | `uv tool list`            |
| **npm**         | Node.js              | `npm list -g --json`      |
| **pnpm**        | Modern Node.js       | `pnpm ls -g --json`       |
| **Cargo**       | Rust                 | `cargo install --list`    |
| **Choco**       | Chocolatey           | `choco list -lo`          |
| **Scoop**       | Windows Env          | `scoop list`              |
| **Gem**         | Ruby                 | `gem list --local`        |
| **Dotnet**      | .NET Global Tools    | `dotnet tool list -g`     |

*(Note: Package manager binaries are wrapped in `cmd /C` execution threads to guarantee path resolution on Windows environments).*

## 🤝 Contributing
Contributions are always welcome! Feel free to open issues or submit PRs to add support for missing package managers (like `apt`, `brew`, or `go modules`) or UI enhancements.

## 📜 License
This project is licensed under the MIT License.
