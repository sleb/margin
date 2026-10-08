# margin

A keyboard-first markdown notes app for macOS that lives in the menubar.

Status: planning. Nothing is built yet.

## Goals

- **Always available.** Starts at login; summon it from the menubar or a global hotkey.
- **Keyboard first.** Vi mode for editing, shortcuts for everything, and a command palette.
- **First-class markdown.** Obsidian-style live editing, link completion, and linting over a vault of plain markdown files.

## Stack

- **Shell:** [Tauri 2](https://tauri.app) (Rust)
- **Editor:** [CodeMirror 6](https://codemirror.net) (TypeScript), with `@replit/codemirror-vim`
- **Vault core:** a UI-agnostic Rust crate for indexing, search, and linting

macOS first; other platforms are possible later.
