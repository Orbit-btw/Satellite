# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - Cross-Platform Update

### Added
- **Linux & macOS Support**: Satellite is now a truly universal, cross-platform package manager dashboard.
- **Native Unix Package Managers**: Seamless integration for major Unix system package managers:
  - `brew` (Homebrew for macOS and Linux)
  - `apt` (Debian/Ubuntu)
  - `pacman` (Arch Linux)
- **Conditional Compilation**: Utilized Rust's `#[cfg(target_os)]` macro to conditionally compile OS-specific execution logics, ensuring the binary remains lightweight and native to the host environment.
- **Smart OS Filtering**: Automatically filters out Windows-only managers (`winget`, `choco`, `scoop`) when running on Unix, and vice versa.
- **In-App Terminal Logs**: Updates and deletions no longer exit the application! Operations are now streamed directly into a beautiful new in-app Log View.
- **Async Runtime**: Swapped standard threads for a fully asynchronous `tokio` runtime, significantly reducing CPU and memory overhead during concurrent fetching.
- **Sudo / Admin Detection**: The app now intelligently warns you if you try to perform OS-level package updates (`apt`, `pacman`) without proper privileges, preventing frozen TUI states.
- **JSON Parsing**: Overhauled parsing logic to use robust JSON outputs where possible, eliminating spacing bugs.

### Changed
- Abstracted `cmd /C` wrapper calls into a unified, cross-platform `build_command` macro. Commands execute natively on Unix and gracefully fallback to `cmd` wrapping on Windows.

## [0.1.0] - Initial Release

### Added
- **Core TUI Dashboard**: A highly-concurrent, visually polished Terminal UI for managing global packages, built using `ratatui` and `crossterm`.
- **Package Manager Support**: Seamless integration for 10 major package ecosystems on Windows:
  - `winget` (Windows Native)
  - `npm` (Node.js)
  - `pnpm` (Modern Node.js)
  - `pip` (Python)
  - `uv` (Modern Python)
  - `cargo` (Rust)
  - `choco` (Chocolatey)
  - `scoop` (Windows Env)
  - `gem` (Ruby)
  - `dotnet` (.NET Global Tools)
- **Multi-threaded Scanning**: Spawns concurrent threads to scan all package managers simultaneously on startup, preventing UI blocking.
- **Two-Tier Navigation System**: 
  - *Home View*: Displays all available package managers and their installed package counts.
  - *Packages View*: Detailed list of installed packages and their versions for a specific ecosystem.
- **Batch Operations**: 
  - Select multiple packages using `Spacebar`.
  - Batch Update (`u`) selected packages.
  - Batch Delete/Uninstall (`d`) selected packages.
- **Windows Process Wrapping**: Safely executes underlying package manager commands using `cmd /C` to guarantee path resolution on Windows environments.
