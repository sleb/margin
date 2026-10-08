# margin

A keyboard-first markdown notes app for macOS that lives in the menubar.

Status: early scaffolding. The app opens a blank window.

## Goals

- **Always available.** Starts at login; summon it from the menubar or a global hotkey.
- **Keyboard first.** Vi mode for editing, shortcuts for everything, and a command palette.
- **First-class markdown.** Obsidian-style live editing, link completion, and linting over a vault of plain markdown files.
- **Fearless refactoring.** Rename or move a note and every link to it is updated, all or nothing, with a preview and an undo.

## Stack

- **Shell:** [Tauri 2](https://tauri.app) (Rust)
- **UI:** [Solid](https://www.solidjs.com), bundled with [Vite](https://vite.dev), managed with [Bun](https://bun.sh)
- **Editor:** [CodeMirror 6](https://codemirror.net) (TypeScript), with `@replit/codemirror-vim`
- **Vault core:** a UI-agnostic Rust crate for indexing, search, and linting

macOS first; other platforms are possible later.

## Development

Prerequisites: [Rust](https://rustup.rs), [Bun](https://bun.sh), and the Xcode command line tools (`xcode-select --install`).

```sh
bun install              # install frontend dependencies
bun run tauri dev        # run the app with hot reload
bun run tauri build      # build a release bundle
bun run typecheck        # type-check the frontend
cargo test --workspace   # run the Rust tests
```

## Layout

- `src` — frontend (TypeScript, Solid)
- `src-tauri` — the Tauri shell
- `crates/margin-vault` — vault core, with no Tauri dependency

See [ARCHITECTURE.md](ARCHITECTURE.md) for the component design.
