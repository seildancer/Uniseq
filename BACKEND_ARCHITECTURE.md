## Uniseq project

- Uniseq is a markdown-native, local-first, simple outliner pkm that only supports unidirectional refs from block to pages.
- It is inspired by logseq OG when it comes to block structure and journal (uniseq streams) => pages flow, but uniseq aims to remove the dependence on intermediate block DB and keep it file-first like obsidian.

## Rule of Thumb

- Rust backend owns page/block level work: workspace/page discovery, parsing pages into block trees and handle indices, manage page hierarchy, and safe file mutations.
- React frontend owns markdown related work: rendering, editor UX, routing user actions, and translating intent into markdown edits or narrow backend write commands.

## Core Rules

- Markdown files are the only durable source of truth.
- Every markdown file is a page.
- Stream (journal) files and normal page files are the same backend object.
- Page body is modelled into blocks.
- All references are `block -> page`.
- Supported page reference syntax is `[[Page]]` and `#Page`.
- References inside code blocks are ignored.
- `linked references` are derived views over source blocks and never live in the target page file.
- Pages have a backend-resolved identity derived from filesystem layout and page hierarchy.
- Page rename is a first-class backend operation and must be crash-safe, with reference rewrites and recovery from interrupted writes. Page rename can also deal with moving pages within the hierarchy.
- Disposable indexes and caches may exist, but the backend must be rebuildable from files alone.
- File-level sync is the working storage assumption, but sync logic is out of scope for now.

## Backend Responsibilities

- Discover markdown pages from the workspace, and organize them into the hierarchy of pages. We follow the logseq convention of `A___B.md` to mark page `B` under page `A`, and `A___B___C.md` is valid for deeper nesting.
- Ensure parent pages exist for hierarchical pages. If `A___B.md` exists, `A.md` must exist; missing parents may be materialized as empty files.
- Apply page rename as a crash-safe transactional write with recovery support.
- Parse page files into block trees.
- Apply source-anchored markdown text edits by updating the block subtree.

## Frontend Responsibilities

- Render blocks and page screens.
- Handle editor UX and convert user actions into markdown text edits.
- Route edits performed from linked-reference views back to the source page.

## Non-Goals to always avoid

- Durable global block UUIDs.
- Manual `block -> block` references.
- Hidden database state as the real product model.

## Implementation steps:

1. Core types and invariants: define PageId, path/name rules, block/source-span types, and the file-first constraints everything else relies on.
2. Workspace discovery and page hierarchy: scan .md files, derive page identities, and resolve A___B___C.md into a page tree.
3. Parent-page materialization: enforce the hierarchy rule by creating missing parents like A.md when A___B.md exists.
4. Markdown-to-block-tree parser with source spans: parse each page into blocks and record exact text ranges so later edits can be targeted safely.
5. Page reference extraction and rebuildable indexes: collect [[Page]] and #Page refs, ignore code blocks, and build disposable lookup indexes from files.
6. Read APIs for pages, blocks, and linked references: expose stable queries the frontend can consume without needing to know file-layout details.
7. Source-anchored block subtree edits: update markdown by replacing the exact source region for a block subtree rather than mutating a hidden DB model.
8. Crash-safe page rename/move with reference rewrites: rename or relocate pages transactionally, rewrite inbound refs, and avoid leaving the workspace half-updated.
9. Recovery and incremental file watching: recover interrupted operations on startup, then add invalidation/watch logic so the backend stays fast without becoming the source of truth.
