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
- References inside fenced code blocks are ignored. (we don't care about indentation code blocks)
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





## Storage-Aware Workspace Paths for Pages and Streams

### Summary

Replace the current flat A___B.md workspace model with an explicit storage-layout model:

- Normal pages live under pages/
- Stream pages live under streams/<stream-name>/
- Stream folders are storage buckets only, not part of the page hierarchy

Both kinds still load into the same page/ref/cache model. The only special handling is in
workspace-path parsing, hierarchy construction, and path round-tripping.

### Key Changes

- Introduce a storage-path model for valid markdown files:
    - pages/<page-file>.md where normal page hierarchy still uses A___B.md
    - streams/<stream-name>/<date>.md
    - Any other .md path is unsupported workspace content and should be ignored during
      discovery/watch instead of failing the workspace
- Split hierarchy behavior from page identity:
    - Normal pages keep the current hierarchical PageId behavior, e.g. pages/A___B.md => A/B
    - Stream pages get distinct page identities that include the stream name so journal/2026-
      05-07 and diary/2026-05-07 do not collide
    - Stream name must not participate in parent/child hierarchy logic
- Add minimal location metadata to preserve canonical paths:
    - Page-backed location for pages/...
    - Stream-backed location carrying the stream name for streams/<name>/...
    - Use this metadata for discovery, workspace_path, create/load, rename/move validation, and
      watcher reconciliation
- Replace the current flat path conversion assumptions:
    - PageId::from_workspace_path / to_workspace_path should become storage-aware helpers that
      parse and format both legal roots
    - Existing name validation stays unless stream/date path rules require small adjustments
- Rework discovery/materialization:
    - Discovery scans markdown files, keeps only legal storage paths, and maps them through the
      new parser
    - Parent-page materialization applies only to normal pages under pages/
    - Stream files never create parent pages
- Update watcher/session handling:
    - Valid changes under pages/ and streams/<name>/ reconcile normally
    - Unsupported nested markdown elsewhere is ignored rather than surfacing InvalidPagePath
    - Reload/recovery paths use the same storage-aware mapping logic as initial discovery

### API / Type Changes

      stream-backed pages
- Add storage-aware parse/format helpers that convert between workspace-relative paths and
  (PageId, location)
- Keep read/query APIs page-centric; no separate stream domain object is introduced

### Assumptions

- Normal page hierarchy remains encoded as A___B.md under pages/
- Stream files remain streams/<stream-name>/<date>.md
- Stream page identity includes the stream name only for uniqueness/path mapping, not as page-
  tree hierarchy
- Streams are first-class pages everywhere except page-hierarchy construction




# Scalable Single-User Sync Reconciliation

## Summary

Refactor workspace watching/reconciliation so normal multi-file sync bursts are handled
incrementally instead of forcing a whole-workspace reparse. Keep the design explicitly single-
user and file-first: no collaboration logic, no conflict-resolution layer, and no attempt to
make every ambiguous filesystem pattern incremental. The target outcome is that sync updates,
editor save bursts, and larger vaults remain responsive, while full_refresh stays as the safety
fallback for recovery and unclear states.

## Key Changes

- Change watcher reconciliation from "exactly one changed markdown path" to "batch of changed
  markdown paths":
    - native watcher bursts should collect all changed markdown paths in the burst
    - polling diffs should classify created/modified/deleted paths as one batch
    - routine multi-file batches should no longer trigger full_refresh just because there is
      more than one path
- Add a batched incremental apply path in WorkspaceSession:
    - read and parse only created/modified files
    - remove only deleted pages
    - apply the whole batch to cache state, then emit one combined page-level diff
    - reserve WorkspaceReloaded for actual full refreshes only
- Keep incremental scope intentionally narrow:
    - do not create missing parent pages during incremental reconciliation
    - if a batch would require parent materialization or leaves hierarchy invalid, fall back to
      full_refresh
    - if watcher input is ambiguous or recovery-related, fall back to full_refresh
- Reduce avoidable whole-cache work in incremental paths:
    - replace per-file cache.clone() parent checks with batch-level validation against affected
      page IDs and ancestors
    - update incoming refs and parent-child relationships only for changed/deleted pages and
      their directly affected neighbors
    - preserve untouched pages and parsed blocks in memory
- Leave polling mode pragmatic:
    - keep the filesystem scan used to detect changes
    - stop turning a multi-file diff into a whole-workspace reparse
    - accept scan cost for now; optimize reparse/rebuild cost first

## Internal Interfaces

- Replace single-change classification helpers with batch-oriented ones:
    - snapshot diff returns created, modified, and deleted markdown paths
    - native event burst classification returns either a markdown-path batch or an explicit
      fallback reason
- Add a cache batch mutation helper, or equivalent session-level orchestration, that applies
  multiple upserts/removals before calculating final page-level diffs.
- Keep public runtime semantics simple:
    - PagesChanged and PageRemoved remain the primary frontend invalidation signals
    - WorkspaceReloaded means a real coarse refresh occurred, not normal sync activity

## Assumptions

- Collaboration is out of scope permanently; no design choices should optimize for concurrent
  multi-user editing.
- Sync is file-level and may deliver bursts, reorderings, and rename-like sequences; handling
  these efficiently is in scope.
- Parent-page materialization remains a discovery/full-refresh behavior, not an incremental
  watcher responsibility.
- A conservative fallback to full_refresh is acceptable whenever incremental handling would
  need structural healing or nontrivial ambiguity resolution.