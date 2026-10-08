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
│  lifecycle · tray · global hotkey · launch at login · window · settings      │
└─────────┬────────────────────────────────────────────────────────────────────┘
          │        Rust API: `Vault` handle + `VaultEvent` channel
┌─────────▼────────────────────────────────────────────────────────────────────┐
│  Vault core (Rust crate, no Tauri dependency)                                │
│  Store ──► Index ──► Search / Completion / Lint          Watcher ──► Index   │
└─────────┬────────────────────────────────────────────────────────────────────┘
          ▼
   markdown files on disk (the vault)
```

| Component  | Lives in              | One-line role                                      |
| ---------- | --------------------- | -------------------------------------------------- |
| Vault core | `crates/margin-vault` | Everything about notes: files, index, search, lint |
| Shell      | `src-tauri`           | Everything about being a macOS app                 |
| Frontend   | `src`                 | Everything the user sees and types into            |

Two rules shape the rest of the design:

1. **The markdown files on disk are the source of truth.** Everything else (index, search data, UI state) is a cache that can be rebuilt from them.
2. **Dependencies point downward only.** The frontend knows the IPC contract, the shell knows the core's Rust API, and the core knows neither.

## Vault core

A UI-agnostic Rust library. It could be driven by a CLI or by tests with no app running.

### Parts

| Part           | Responsibility                                                                                                                                                                                                                                                                    |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Store**      | Reads and writes note files. Owns path handling and versions.                                                                                                                                                                                                                     |
| **Watcher**    | Notices file changes made outside margin and feeds them to the index.                                                                                                                                                                                                             |
| **Index**      | In-memory metadata for every note: title, headings, tags, outgoing links, and the reverse link graph (backlinks). For each link it records where the destination sits in the source text and which version of the note that position was read from. Resolves link text to a note. |
| **Search**     | Fuzzy match over titles and paths for the switcher; full-text search over note bodies.                                                                                                                                                                                            |
| **Completion** | Given a partial link, returns candidate notes and headings from the index.                                                                                                                                                                                                        |
| **Lint**       | Given a note's text and the index, returns diagnostics (for example, a link to a note that does not exist).                                                                                                                                                                       |
| **Refactor**   | Plans and applies changes that span several files, such as renaming or moving a note, so that every link stays intact.                                                                                                                                                            |

### Key types

- `NoteId`: the note's path relative to the vault root, normalized. It is the only identifier for a note anywhere in the system.
- `Version`: a hash of a note's content. Two notes with the same version have identical text.

### Invariants

- The core never reads or writes outside the vault root.
- Writes are atomic: write a temporary file, then rename it over the target. A crash never leaves a half-written note.
- A write that would overwrite content the caller has not seen fails with a conflict; it never silently wins (see [Saving](#saving-and-conflicts)).
- The index reflects what is on disk, not unsaved editor buffers. It is eventually consistent with disk and can always be rebuilt by rescanning.
- A refactor is all or nothing, short of a crash while it is being committed: either every file it touches is updated or none is (see [Refactoring](#refactoring)).
- A refactor changes only link destinations and file locations. Every other byte of every file is left as it was.
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
    fn delete(&self, id: &NoteId) -> Result<()>;                 // moves to system trash

    fn plan_rename(&self, from: &NoteId, to: &NoteId) -> Result<RefactorPlan, PlanError>;
    fn apply(&self, plan: RefactorPlan) -> Result<(), ApplyError>;   // ApplyError::Stale
    fn undo_last_refactor(&self) -> Result<(), ApplyError>;

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
    Refactored { moved: Vec<(NoteId, NoteId)>, changed: Vec<NoteId> },
    IndexStatus(IndexStatus),                       // Scanning | Ready
}
```

A `write` produces no event for the note it wrote. Changes made outside margin produce `NoteChanged`, `NoteCreated`, or `NoteRemoved`. A refactor produces exactly one `Refactored` event listing everything it moved and changed. The core recognizes its own writes by comparing the changed file's hash with the version it last wrote.

### Links

Links are standard markdown links, `[text](path/to/note.md)`, with the path relative to the linking note. This covers inline links, reference-style links and their definitions, and image links. A relative path resolves to exactly one file, so a link is never ambiguous.

The index tracks links to every file in the vault, not only to notes, because moving a note also has to fix its links to images and other attachments.

Recognizing a link and resolving it both happen in one module of the core, so that wikilinks (`[[note]]`) can be added later as a second syntax without touching the index, lint, completion, or refactoring.

### Refactoring

A refactor is a change that spans several files and must leave every link intact. Renaming or moving a note is the first one. Moving a folder and renaming a heading are later additions that use the same mechanism.

Renaming or moving a note changes three things:

- The file's location.
- **Inbound links:** every link in another note that resolves to it. Any `#heading` part of the link is kept.
- **Outbound links:** the note's own relative links, when it moves to a different folder, because they are relative to where the note sits.

Every refactor has two steps.

**Plan.** `plan_rename` computes a `RefactorPlan` from the index alone and writes nothing. The plan lists the file moves and, for each file to edit, the version it was computed against and the exact replacements to make. A plan is plain data, so it can be previewed ("12 links in 7 notes"), stored, and inverted.

**Apply.** `apply` carries out a plan in four stages:

1. **Validate.** Re-read every file the plan touches and check that its version still matches the plan, and that no move would land on an existing file. On any mismatch, return `Stale` with nothing changed; the caller plans again.
2. **Stage.** Write the new content of every edited file into a staging directory, `.margin/`, inside the vault. A failure here, such as a full disk, aborts with nothing changed.
3. **Commit.** Move each staged file over its target and perform the file moves. Each step is one atomic rename.
4. **Finish.** Update the index in a single step, delete the staging directory, and emit one `Refactored` event.

Every failure that can be anticipated happens in the first two stages, where nothing in the vault has changed yet. The commit stage is a short run of renames.

If margin stops before the commit stage, the next `Vault::open` discards the leftover staging directory and the vault is untouched. If it stops during the commit stage, the rename is left half applied: some files updated and others not. That is accepted for now. If it becomes a problem, the fix is a journal: write the plan to disk before committing, and finish any journaled plan on the next launch. Plans are already plain data, so this adds a stage without changing the rest.

Guarantees:

- **All or nothing**, short of a crash during the commit stage.
- **Never applied to content it was not computed from.** The version check in the validate stage enforces this.
- **Minimal edits.** Only link destinations change; nothing else in a file is reformatted.
- **No half-way view.** Readers of the index see the vault before the refactor or after it, never in between.
- **Undoable.** The core keeps the inverse of the last applied plan. `undo_last_refactor` applies it with the same checks, so an undo fails with `Stale` instead of overwriting edits made since.

Known limits:

- A crash during the commit stage leaves the refactor half applied, as described above. Lint will report any links left broken.
- A program that writes to an affected file in the instant between validation and commit can have its change overwritten.
- Links the core cannot see are not updated: links written as raw HTML, and links from files outside the vault.
- The index reflects disk, so unsaved text in the editor is not part of a plan. The frontend saves the open note before asking for one.
- On a case-insensitive filesystem, the macOS default, a rename that changes only letter case must be handled as a move and not rejected as a collision.

## Shell

The Tauri application. It makes margin a well-behaved macOS menubar app and connects the frontend to the core.

### Responsibilities

- Process lifecycle: single instance, launch at login, quit.
- The menubar (tray) icon and its menu.
- The global hotkey that summons and dismisses the main window.
- The main window: creating it once, showing, hiding, and positioning it.
- Settings: vault location, hotkey, and editor preferences, persisted as a file in the app's config directory.
- Hosting the core: opens the `Vault`, exposes its methods as IPC commands, and forwards `VaultEvent`s to the frontend as IPC events.

### Invariants

- At most one instance runs. Launching a second one summons the first.
- Dismissing the main window hides it; it never destroys the webview or quits the app. Editor state survives every summon.
- The shell contains no note logic. Each vault command is a thin wrapper over one core method.
- Core calls never run on the main (UI) thread.
- Exactly one vault is open at a time. Changing the vault setting closes the old one and opens the new one.

## Frontend

The TypeScript code in the webview.

### Parts

| Part                   | Responsibility                                                                                                                                       |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Editor**             | The CodeMirror 6 instance: vi mode, live-preview rendering, and the completion and lint sources that call the core. Owns the text of open notes.     |
| **Command registry**   | The list of every action in the app, each with an id, a title, and a function. The keymap binds keys to command ids.                                 |
| **Palette / switcher** | One overlay inside the main window, in the style of Obsidian and VS Code, that fuzzy-searches commands (from the registry) or notes (from the core). |
| **Session**            | The one open note, its base version, and whether it has unsaved changes.                                                                             |

### Invariants

- Every action is a registered command. Keys, the palette, and the tray menu all dispatch through the registry, so anything the app can do is reachable from the keyboard and listed in the palette.
- The editor holds the truth for a note with unsaved changes; the core holds the truth for everything else.
- The frontend never touches the filesystem. All note access goes through IPC.
- Keyboard focus is always in exactly one place: the editor or the overlay. Closing the overlay returns focus to the editor.

## IPC contract

The only path between the frontend and the Rust side. TypeScript types are generated from the Rust definitions so the two cannot drift.

**Commands** (frontend calls, Rust answers):

| Command                                 | Returns                 | Used for                           |
| --------------------------------------- | ----------------------- | ---------------------------------- |
| `read_note(id)`                         | `{ text, version }`     | Opening a note                     |
| `write_note(id, text, base_version)`    | `version` or `Conflict` | Saving                             |
| `create_note(id)`, `delete_note(id)`    | result                  | File operations                    |
| `plan_rename(from, to)`                 | `{ plan, summary }`     | Previewing a rename or move        |
| `apply_refactor(plan)`                  | nothing or `Stale`      | Carrying out a previewed plan      |
| `undo_refactor()`                       | nothing or `Stale`      | Undoing the last refactor          |
| `search_notes(query, limit)`            | `NoteHit[]`             | Switcher                           |
| `search_text(query, limit)`             | `TextHit[]`             | Full-text search                   |
| `complete_link(query, limit)`           | `LinkCandidate[]`       | Link completion in the editor      |
| `backlinks(id)`                         | `LinkRef[]`             | Backlinks view                     |
| `lint(id, text)`                        | `Diagnostic[]`          | Diagnostics for the current buffer |
| `get_settings()`, `set_settings(patch)` | settings                | Preferences                        |
| `hide_window()`                         | nothing                 | Dismissing from the keyboard       |

**Events** (Rust pushes, frontend listens):

| Event                          | Payload               | Frontend reaction                                           |
| ------------------------------ | --------------------- | ----------------------------------------------------------- |
| `note-changed`                 | `{ id, version }`     | Reload the buffer if clean; mark a conflict if dirty        |
| `note-created`, `note-removed` | `{ id }`              | Update any open lists; close or flag the buffer             |
| `refactored`                   | `{ moved, changed }`  | Follow the open note to its new id; reload it if it changed |
| `index-status`                 | `scanning` or `ready` | Show progress on first scan                                 |
| `window-shown`                 | nothing               | Focus the editor                                            |
| `settings-changed`             | settings              | Re-apply keymap and editor options                          |

### Contract invariants

- **Text positions crossing IPC are UTF-16 code unit offsets**, which is what CodeMirror uses. Rust works in UTF-8 bytes, so the shell converts at the boundary. Diagnostics and completion ranges depend on this.
- `lint` and `complete_link` take the buffer's current text or query as an argument; they do not assume the note has been saved.
- Commands are safe to call concurrently. The frontend discards a response if the buffer has changed since the request was made.
- A plan is opaque to the frontend. It displays the summary and passes the plan back to `apply_refactor` unchanged.

## Key flows

### Summoning

1. The user presses the global hotkey; the shell shows the existing main window and emits `window-shown`.
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
3. The editor shows the candidates; accepting one inserts the path relative to the current note.

### Renaming a note

1. The user runs the rename command and types a new name. The frontend saves the open note if it has unsaved changes.
2. The frontend calls `plan_rename` and shows the summary beside the prompt, for example "updates 12 links in 7 notes".
3. The user confirms; the frontend calls `apply_refactor` with the plan.
   - Applied: the `refactored` event arrives, and the session follows the note to its new id and reloads it.
   - `Stale`: something changed since the plan was made. Nothing was written; the frontend plans again and shows the new summary.
4. An undo command calls `undo_refactor`, which restores the old name and the old links under the same rules.

## Decisions

- **Link syntax.** Standard markdown links first. The design leaves room for wikilinks later (see [Links](#links)).
- **Renames.** Renaming or moving a note rewrites every affected link, all or nothing, with a preview and an undo (see [Refactoring](#refactoring)).
- **Crash recovery for refactors.** Deferred. No journal until a half-applied rename actually becomes a problem.
- **Undo depth.** The last refactor only.
- **Other refactors.** Moving a folder and renaming a heading come later.
- **Main window and palette.** The main window is an ordinary window. The palette is a floating fuzzy-match overlay inside it, as in Obsidian and VS Code.
- **Index persistence.** In memory only, rebuilt on launch. No disk cache until it is unavoidable.
- **Multiple open notes.** A single buffer for now. Splits are a likely later addition; tabs are unlikely.
- **Frontend tooling.** Solid for the reactive overlays (palette, rename preview, conflict prompts), Vite for bundling, Bun for installs and scripts. The CodeMirror editor stays imperative and is mounted once; the UI talks to it through a small command and event API rather than mirroring its buffer into reactive state.
