## Uniseq project

- Uniseq is a markdown-native, local-first, simple outliner pkm that only supports unidirectional refs from block to pages.
- It is inspired by logseq OG when it comes to block structure and journal (uniseq streams) => pages flow, but uniseq aims to remove the dependence on intermediate block DB and keep it file-first like obsidian.

## Rule of Thumb

- Rust backend owns page/block level work: workspace/page discovery, parsing pages into block trees and reference/source-span indexes, manage page hierarchy, maintain derived caches/indexes, and execute structural workspace mutations safely.
- React frontend owns markdown related work: rendering, editor UX, routing user actions, translating intent into markdown edits, and performing ordinary page-content file writes.

## Core Rules

- Markdown files are the only durable source of truth.
- Every markdown file is a page.
- Stream (journal) files and normal page files are the same backend object.
- Page body is modelled into blocks.
- All references are `block -> page`.
- Supported page reference syntax is `[[Page]]` and `#Page`.
- References inside fenced code blocks are ignored.
- `linked references` are derived views over source blocks and never live in the target page file.
- Pages have a backend-resolved identity derived from filesystem layout and page hierarchy.
- Structural page mutations are first-class backend operations. Create/delete-subtree/rename/move are handled by Rust; rename/move must be crash-safe, with reference rewrites and recovery from interrupted writes.
- Disposable indexes and caches may exist, but the backend must be rebuildable from files alone.
- File-level sync is the working storage assumption, but sync logic is out of scope for now.

## Backend Responsibilities

- Discover markdown pages from the workspace, and organize them into the hierarchy of pages. We follow the logseq convention of `A___B.md` to mark page `B` under page `A`, and `A___B___C.md` is valid for deeper nesting.
- Ensure parent pages exist for hierarchical pages. If `A___B.md` exists, `A.md` must exist; missing parents may be materialized as empty files.
- Observe markdown file changes and reconcile derived cache/index state back to the current file contents.
- Apply structural workspace mutations such as page create/delete-subtree/rename/move. Rename/move are crash-safe transactional writes with recovery support.
- Parse page files into block trees.
- Expose normalized block/page/reference state derived from files, with quick eventual consistency after ordinary content edits.

## Frontend Responsibilities

- Render blocks and page screens.
- Handle editor UX and convert user actions into markdown text edits.
- Write ordinary page-content markdown edits directly to files.
- Route edits performed from linked-reference views back to the source page.
- Call narrow backend commands only for structural operations or backend-owned workspace mutations.

## Write Model

- Markdown files remain the only durable source of truth.
- Ordinary content editing is frontend-driven: React computes markdown edits and writes the affected page file directly.
- Rust treats those file changes as authoritative and updates parse trees, source-span anchors, hierarchy-derived state, references, and other disposable indexes asynchronously.
- Derived views such as linked references, counts, and hierarchy metadata are allowed to be briefly stale during reconciliation, but should converge quickly.
- Structural workspace mutations are backend-owned: page create/delete-subtree/rename/move and any multi-file ref rewrite happen through synchronous Rust commands. Rename/move use crash-safe recovery.
- The file watcher/reconciliation pipeline is primarily for ordinary content edits, external edits, sync-delivered changes, and recovery after interruptions.
- Linked references are frontend-composed views over Rust-maintained normalized reference state. React should not rescan raw markdown or rebuild page/block semantics independently.

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
6. Read APIs for normalized page/block/reference state: expose stable derived state the frontend can consume without knowing file-layout details or reparsing markdown.
7. Derived-state reconciliation for content edits: React writes normal markdown edits directly, while Rust reparses affected files and refreshes block trees, refs, and source-span anchors from disk.
8. Structural page operations: create pages and delete page subtrees in Rust, and rename or relocate pages transactionally with reference rewrites.
9. Recovery and incremental file watching: recover interrupted structural operations on startup, then watch file changes so derived backend state stays close to disk truth without becoming the source of truth.
