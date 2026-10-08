# Architecture

Status: proposed. Nothing here is built yet; this is the design to build against.

## Overview

margin has three components and one boundary between them:

```
┌──────────────────────────── webview (TypeScript) ────────────────────────────┐
│  Frontend                                                                    │
│  ┌──────────────┐   ┌──────────────────┐   ┌──────────────────────────────┐  │
│  │ Editor       │   │ Command registry │   │ Palette / switcher           │  │
│  │ (CodeMirror) │◄──┤ + keymap         │◄──┤ (UI over registry + search)  │  │
│  └──────┬───────┘   └────────┬─────────┘   └──────────────┬───────────────┘  │
└─────────┼────────────────────┼────────────────────────────┼──────────────────┘
          │        IPC: commands (request/response) and events (push)
┌─────────┼────────────────────┼────────────────────────────┼──────────────────┐
│  Shell (Rust, Tauri 2)       ▼                                               │
│  lifecycle · tray · global hotkey · launch at login · panel · settings       │
└─────────┬────────────────────────────────────────────────────────────────────┘
          │        Rust API: `Vault` handle + `VaultEvent` channel
┌─────────▼────────────────────────────────────────────────────────────────────┐
│  Vault core (Rust crate, no Tauri dependency)                                │
│  Store ──► Index ──► Search / Completion / Lint          Watcher ──► Index   │
└─────────┬────────────────────────────────────────────────────────────────────┘
          ▼
   markdown files on disk (the vault)
```

| Component | Lives in | One-line role |
|---|---|---|
| Vault core | `crates/margin-vault` | Everything about notes: files, index, search, lint |
| Shell | `src-tauri` | Everything about being a macOS app |
| Frontend | `src` | Everything the user sees and types into |

Two rules shape the rest of the design:

1. **The markdown files on disk are the source of truth.** Everything else (index, search data, UI state) is a cache that can be rebuilt from them.
2. **Dependencies point downward only.** The frontend knows the IPC contract, the shell knows the core's Rust API, and the core knows neither.

## Vault core

A UI-agnostic Rust library. It could be driven by a CLI or by tests with no app running.

### Parts

| Part | Responsibility |
|---|---|
| **Store** | Reads and writes note files. Owns path handling and versions. |
| **Watcher** | Notices file changes made outside margin and feeds them to the index. |
| **Index** | In-memory metadata for every note: title, headings, tags, outgoing links, and the reverse link graph (backlinks). Resolves link text to a note. |
| **Search** | Fuzzy match over titles and paths for the switcher; full-text search over note bodies. |
| **Completion** | Given a partial link, returns candidate notes and headings from the index. |
| **Lint** | Given a note's text and the index, returns diagnostics (for example, a link to a note that does not exist). |

### Key types

- `NoteId`: the note's path relative to the vault root, normalized. It is the only identifier for a note anywhere in the system.
- `Version`: a hash of a note's content. Two notes with the same version have identical text.

### Invariants

- The core never reads or writes outside the vault root.
- Writes are atomic: write a temporary file, then rename it over the target. A crash never leaves a half-written note.
- A write that would overwrite content the caller has not seen fails with a conflict; it never silently wins (see [Saving](#saving-and-conflicts)).
- The index reflects what is on disk, not unsaved editor buffers. It is eventually consistent with disk and can always be rebuilt by rescanning.
- Lint and completion are pure reads: they never modify the vault or the index.
- The core's markdown parser is the authority on what counts as a link, heading, or tag. The editor's parser is used for display only.
- The core has no dependency on Tauri or on any UI type.

### Rust API (sketch)

```rust
impl Vault {                       // cheap to clone; Send + Sync
    fn open(root: &Path) -> Result<(Vault, Receiver<VaultEvent>)>;

    fn read(&self, id: &NoteId) -> Result<Note>;                 // text + version
    fn write(&self, id: &NoteId, text: &str, base: Option<Version>)
        -> Result<Version, WriteError>;                          // WriteError::Conflict
    fn create(&self, id: &NoteId) -> Result<Version>;
    fn rename(&self, from: &NoteId, to: &NoteId) -> Result<()>;
    fn delete(&self, id: &NoteId) -> Result<()>;                 // moves to system trash

    fn search_notes(&self, query: &str, limit: usize) -> Vec<NoteHit>;
    fn search_text(&self, query: &str, limit: usize) -> Vec<TextHit>;
    fn complete_link(&self, query: &str, limit: usize) -> Vec<LinkCandidate>;
    fn backlinks(&self, id: &NoteId) -> Vec<LinkRef>;
    fn lint(&self, id: &NoteId, text: &str) -> Vec<Diagnostic>;
}

enum VaultEvent {
    NoteChanged { id: NoteId, version: Version },   // changed outside margin
    NoteCreated { id: NoteId },
    NoteRemoved { id: NoteId },
    IndexStatus(IndexStatus),                       // Scanning | Ready
}
```

`VaultEvent`s are emitted only for changes margin did not make itself. The core tells the two apart by comparing the changed file's hash with the version it last wrote.

## Shell

The Tauri application. It makes margin a well-behaved macOS menubar app and connects the frontend to the core.

### Responsibilities

- Process lifecycle: single instance, launch at login, quit.
- The menubar (tray) icon and its menu.
- The global hotkey that summons and dismisses the panel.
- The panel window: creating it once, showing, hiding, and positioning it.
- Settings: vault location, hotkey, and editor preferences, persisted as a file in the app's config directory.
- Hosting the core: opens the `Vault`, exposes its methods as IPC commands, and forwards `VaultEvent`s to the frontend as IPC events.

### Invariants

- At most one instance runs. Launching a second one summons the first.
- Dismissing the panel hides it; it never destroys the webview or quits the app. Editor state survives every summon.
- The shell contains no note logic. Each vault command is a thin wrapper over one core method.
- Core calls never run on the main (UI) thread.
- Exactly one vault is open at a time. Changing the vault setting closes the old one and opens the new one.

## Frontend

The TypeScript code in the webview.

### Parts

| Part | Responsibility |
|---|---|
| **Editor** | The CodeMirror 6 instance: vi mode, live-preview rendering, and the completion and lint sources that call the core. Owns the text of open notes. |
| **Command registry** | The list of every action in the app, each with an id, a title, and a function. The keymap binds keys to command ids. |
| **Palette / switcher** | One overlay that fuzzy-searches commands (from the registry) or notes (from the core). |
| **Session** | Which notes are open, which is focused, and each buffer's base version and dirty flag. |

### Invariants

- Every action is a registered command. Keys, the palette, and the tray menu all dispatch through the registry, so anything the app can do is reachable from the keyboard and listed in the palette.
- The editor holds the truth for a note with unsaved changes; the core holds the truth for everything else.
- The frontend never touches the filesystem. All note access goes through IPC.
- Keyboard focus is always in exactly one place: the editor or the overlay. Closing the overlay returns focus to the editor.

## IPC contract

The only path between the frontend and the Rust side. TypeScript types are generated from the Rust definitions so the two cannot drift.

**Commands** (frontend calls, Rust answers):

| Command | Returns | Used for |
|---|---|---|
| `read_note(id)` | `{ text, version }` | Opening a note |
| `write_note(id, text, base_version)` | `version` or `Conflict` | Saving |
| `create_note(id)`, `rename_note(from, to)`, `delete_note(id)` | result | File operations |
| `search_notes(query, limit)` | `NoteHit[]` | Switcher |
| `search_text(query, limit)` | `TextHit[]` | Full-text search |
| `complete_link(query, limit)` | `LinkCandidate[]` | Link completion in the editor |
| `backlinks(id)` | `LinkRef[]` | Backlinks view |
| `lint(id, text)` | `Diagnostic[]` | Diagnostics for the current buffer |
| `get_settings()`, `set_settings(patch)` | settings | Preferences |
| `hide_panel()` | nothing | Dismissing from the keyboard |

**Events** (Rust pushes, frontend listens):

| Event | Payload | Frontend reaction |
|---|---|---|
| `note-changed` | `{ id, version }` | Reload the buffer if clean; mark a conflict if dirty |
| `note-created`, `note-removed` | `{ id }` | Update any open lists; close or flag the buffer |
| `index-status` | `scanning` or `ready` | Show progress on first scan |
| `panel-shown` | nothing | Focus the editor |
| `settings-changed` | settings | Re-apply keymap and editor options |

### Contract invariants

- **Text positions crossing IPC are UTF-16 code unit offsets**, which is what CodeMirror uses. Rust works in UTF-8 bytes, so the shell converts at the boundary. Diagnostics and completion ranges depend on this.
- `lint` and `complete_link` take the buffer's current text or query as an argument; they do not assume the note has been saved.
- Commands are safe to call concurrently. The frontend discards a response if the buffer has changed since the request was made.

## Key flows

### Summoning

1. The user presses the global hotkey; the shell shows the existing panel and emits `panel-shown`.
2. The frontend focuses the editor, which still holds the last note and cursor position.

No vault or disk work happens on this path, which keeps summoning instant.

### Saving and conflicts

1. The editor loads a note with `read_note` and records the returned version as the buffer's base version.
2. After a short pause in typing, the frontend calls `write_note(id, text, base_version)`.
3. The core compares `base_version` with the file's current version.
   - Equal: it writes atomically, updates the index, and returns the new version, which becomes the new base.
   - Different: it returns `Conflict` and writes nothing. The frontend keeps the buffer and asks the user which side to keep.

### External change

1. Another program edits a file in the vault. The watcher reports it; the core re-indexes the note and emits `NoteChanged`.
2. The shell forwards it as `note-changed`.
3. If that note is open and clean, the frontend reloads it. If it is open and dirty, the buffer is marked conflicted and the next save follows the conflict path above.

### Link completion

1. The user types a link opener in the editor; the editor's completion source calls `complete_link` with the text typed so far.
2. The core answers from the in-memory index, with no disk access.
3. The editor shows the candidates; accepting one inserts the link text.

## Open decisions

- **Link syntax.** Wikilinks (`[[note]]`), standard markdown links, or both. This sets the core parser's rules and what completion inserts.
- **Renames.** Whether renaming a note rewrites the links that point to it.
- **Panel style.** A Spotlight-style floating panel that appears over full-screen apps, or an ordinary window. The floating panel needs extra macOS-specific work in the shell.
- **Index persistence.** The index starts in memory and is rebuilt on launch. A disk cache is an optimization to add only if large vaults make startup slow.
- **Multiple open notes.** A single buffer at a time, or splits and tabs. This affects only the frontend's session.
