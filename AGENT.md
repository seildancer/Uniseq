## Uniseq project

- Uniseq is a local-first, file-first outliner PKM.
- Markdown files are the only durable product truth.
- The backend only understands the Uniseq block/ref dialect, not general Markdown semantics.

## Rule of Thumb

- Rust backend owns workspace discovery, page identity, block parsing, page-ref extraction, derived indexes/caches, and a narrow set of structural workspace mutations.
    - If it defines truth about the workspace, put it in core.
    - If it adapts truth for the desktop/mobile app, keep it in Tauri.
- React frontend owns Markdown semantics: rendering, editor UX, routing user actions, linked-reference editing flows, and ordinary page-content file writes.

## Core Rules

- Every supported markdown file is a page.
- Normal pages live under `pages/` and use `A___B.md` filename hierarchy encoding.
- Stream pages live in top-level stream folders such as `journals/` or `diary/`, with files named `yyyy_mm_dd.md`.
- `assets/` is reserved for binaries such as images, PDFs, and attachments.
- `uniseq/` is reserved for persisted app metadata, caches, and config.
- Page bodies are modeled into blocks. Block kinds:
  - **Outliner**: a line starting with `- ` (optional leading tabs set nesting depth)
  - **Plaintext**: any content before, after, or between outliner blocks — no explicit marker
- All references are `block -> page`.
- Supported page reference syntax is `[[Page]]` and `#Page`, and both resolve to page-backed pages only.
- Stream pages are file-backed pages for discovery, parsing, reads, and controlled create/delete operations, but they are not targetable through markdown ref syntax.
- References inside fenced code blocks are ignored. Indentation-only Markdown code blocks are not special to the backend.
- `linked references` are derived views over source blocks and never live in the target page file.
- Disposable indexes and caches may exist, but the backend must be rebuildable from files alone.
- In-app page identity changes must use backend-owned structural operations. Raw external filesystem renames/moves reconcile from disk truth, but they do not preserve semantic rename/move behavior or rewrite refs.

## Backend Responsibilities

- Discover supported markdown files from the workspace and derive page identities from storage-aware paths.
- Build and maintain page hierarchy for page-backed pages.
- Materialize missing parent pages during discovery/full refresh only.
- Parse page files into Uniseq block trees with source spans.
- Extract page references from block content and maintain incoming/outgoing ref indexes.
- Expose normalized page/block/reference read APIs.
- Apply only narrow structural mutations:
  - page create
  - page delete-subtree
  - page rename
  - page move
  - stream create
  - stream delete
- Reconcile ordinary file-content edits back into derived cache/index state.

## Frontend Responsibilities

- Render blocks and page screens.
- Own Markdown semantics beyond the Uniseq block/ref dialect.
- Handle editor UX and translate user actions into markdown text edits.
- Write ordinary page-content markdown edits directly to files.
- Route edits performed from linked-reference views back to the source page.
- Call backend-owned structural operations for page identity changes instead of performing raw file renames/moves directly.

## Write Model

- Ordinary content editing is frontend-driven: React computes markdown edits and writes the affected page file directly.
- Rust treats those file changes as authoritative and reparses affected existing files asynchronously.
- Derived views such as linked references, counts, and hierarchy metadata may be briefly stale during reconciliation, but should converge quickly.
- Structural workspace mutations are backend-owned for create/delete/rename/move and stream create/delete.

## Reconciliation Model

- Keep the hot path incremental only for content edits to already-known files whose identity and location do not change.
- Allow batched incremental updates when all changed paths are modified existing files.
- Fall back to coarse full refresh for structural ambiguity, including:
  - created files
  - deleted files
  - hierarchy healing
  - unsupported or ambiguous watcher bursts
- External raw renames/moves remain part of that structural ambiguity path and are reconciled as disk-truth changes rather than semantic in-app rename/move operations.
- `WorkspaceReloaded` means a real coarse rebuild occurred.
- `PagesChanged` and `PageRemoved` remain the primary frontend invalidation signals.

## Storage Model

- Normal pages:
  - `pages/<page-file>.md`
  - hierarchy encoded through `___`
- Stream pages:
  - `<stream-name>/<yyyy_mm_dd>.md`
- Supported discovery roots are inferred:
  - `pages/` is mandatory and may contain only flat markdown files
  - any other non-reserved top-level folder is treated as a stream only when it is empty or contains only `yyyy_mm_dd.md` files
- `assets/` and `uniseq/` are ignored by discovery/watch.
- Other folders are ignored during discovery/watch.
- Stream folders are storage buckets only and do not participate in page hierarchy.

## Non-Goals

- Durable global block UUIDs.
- Manual `block -> block` references.
- Hidden database state as the real product model.
- General Markdown parsing in the backend.
- Precise incremental handling for every structural filesystem change.
