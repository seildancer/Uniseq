## Uniseq

- Uniseq is a local-first, file-first outliner PKM.
- Markdown files are the only durable product truth.
- Parsed blocks, references, hierarchy metadata, watcher state, and caches are disposable derived state.

## Architecture Split

- `src/` is the Rust backend crate (`uniseq-backend`).
- `src-tauri/` is the desktop shell and command surface.
- `web/` is the React frontend and editor UI.

## Product Rules

- Every supported markdown file is a page.
- Normal pages are page-backed and live under `pages/`.
- Stream pages are file-backed and live in top-level stream folders such as `journals/`, `diary/`, or any other valid non-reserved stream directory.
- `assets/` is reserved for binary files and attachments.
- `uniseq/` is reserved for app metadata and config.
- Stream folders are storage buckets only. They do not participate in page hierarchy.

## Storage Model

- Normal page files use flat filename encoding under `pages/`:
  - `pages/A.md`
  - `pages/A___B.md`
  - `pages/A___B___C.md`
- Stream page files use `yyyy_mm_dd.md` names inside a top-level stream folder:
  - `journals/2026_05_07.md`
- `pages/` is mandatory and may contain only flat markdown files.
- A top-level non-reserved folder is treated as a stream only when it is empty or contains only `yyyy_mm_dd.md` markdown files.
- Other non-stream folders are ignored by discovery and watching.
- `prepare_workspace_root` backfills the standard folders `pages/`, `assets/`, `uniseq/`, `journals/`, and `diary/`.

## Page Identity

- Regular page ids are hierarchical, for example `pages:A/B/C`.
- Stream page ids are location-aware, for example `stream:journals/2026_05_07`.
- Page identity is derived from workspace-aware paths, not stored separately.
- A page-backed page and a stream page with the same visible segments are still distinct identities.

## Backend Dialect

- The backend does not implement general Markdown semantics.
- It only understands:
  - page-backed page identity and storage paths
  - stream page identity and storage paths
  - block parsing
  - page-reference extraction
- Block kinds:
  - `Outliner`: a line starting with `-` followed by end-of-line, a space, or a tab
  - `Plaintext`: any other content region
- Blank lines are preserved as plaintext content when they belong to plaintext runs, and can also survive as standalone plaintext separator blocks between outliner blocks.
- References are only `block -> page`.
- Supported page reference syntax is `[[Page]]` and `#Page`.
- References inside fenced code blocks are ignored.
- Indentation-only Markdown code blocks are not special to the backend.
- Stream pages are not targetable through markdown page-ref syntax.

## Discovery And Materialization

- Discovery scans supported workspace roots and loads pages into a `WorkspaceCache`.
- Discovery materializes missing parent page-backed pages as empty markdown files.
- Discovery also materializes missing page-backed reference targets as empty markdown files.
- Because of that, opening a workspace can create empty page files for referenced-but-missing page-backed pages.
- Stream pages are discovered, parsed, and readable, but they do not create hierarchy parents.

## Read Model

- The backend exposes:
  - page summaries
  - page details
  - page content snapshots
  - incoming refs
  - outgoing refs
  - lists of all pages
  - paginated page lists
  - lists of pages with missing targets
  - lists of stream names
- Page content snapshots include:
  - exact text
  - parsed flat block snapshots
  - a file fingerprint revision
- Incoming refs are anchored to source block spans in the owning source page.
- Linked references are derived views only. They never live in the target page file.

## Write Model

- Ordinary page-content editing is frontend-driven.
- The frontend writes markdown text through the desktop command surface using `write_page_content`.
- Writes are optimistic and can include an expected file fingerprint revision.
- Rust reparses the written file and updates derived state immediately for that write path.
- Structural identity changes remain backend-owned:
  - page create
  - page delete-subtree
  - page rename
  - page move
  - stream create
  - stream delete

## Reconciliation And Watching

- `WorkspaceSession` owns the live cache, filesystem snapshot, event queue, and watcher state.
- The session supports native watching and polling fallback.
- Incremental reconciliation is for already-known file writes where identity and location stay stable.
- Batched incremental reconciliation is supported for modified existing markdown files.
- Full refresh is used for structural ambiguity, including:
  - created files
  - deleted files
  - hierarchy healing
  - unsupported watcher bursts
  - external raw renames and moves
- Transaction recovery replays interrupted backend structural operations to their recorded final state instead of rolling them back.
- Frontend invalidation signals are:
  - `WorkspaceReloaded`
  - `PagesChanged`
  - `PageRemoved`
  - watcher mode / degradation events

## Desktop Shell Responsibilities

- `src-tauri/` owns workspace bootstrap and desktop command wiring.
- It exposes commands for:
  - opening and creating workspaces
  - remembering the last workspace path
  - reading page lists, summaries, details, content, refs, and stream names
  - writing page content
  - structural page operations
  - draining workspace events
  - starting and stopping watchers
- It serializes backend types into DTOs for the web app.

## Frontend Responsibilities

- `web/` owns the app UI and editor UX.
- The current editor stack is React plus Milkdown.
- The frontend owns:
  - onboarding and workspace open/create flows
  - page tree UI
  - stream list UI
  - page navigation
  - editor behavior and markdown editing UX
  - rename/move/delete dialogs
  - polling backend workspace events and refreshing UI state
  - conflict handling when a page revision changes while editing
- The frontend should treat backend page ids and watcher events as authoritative.

## Rule Of Thumb

- If something defines durable workspace truth, page identity, parsed block structure, reference indexing, or structural workspace mutation semantics, it belongs in Rust core.
- If something is about editor behavior, rendering, interaction design, or adapting backend state into app UX, it belongs in the web app or Tauri shell.

## Non-Goals

- Durable global block UUIDs.
- `block -> block` references.
- Hidden database state as the real product model.
- General Markdown parsing in the backend.
- Perfectly precise incremental handling for every possible external structural filesystem change.
