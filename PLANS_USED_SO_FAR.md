5/5

# Core Types And Invariants Plan

## Summary

Implement the backend core as a new Rust crate with a simple file-first model: Page is the durable unit, each page owns its parsed block tree, and each page keeps a derived list of incoming refs from source-owned
blocks. Blocks have no durable identity.

## Core Model

- Define PageId as canonical hierarchy segments derived from workspace-relative markdown paths using the A___B.md convention.
- Define Page with page_id, canonical title/name, workspace-relative file path, child page ids, owned block tree, incoming refs, and file revision/fingerprint.
- Define Block as non-durable parsed source content with block_span, content_span, child blocks, and outgoing page-reference occurrences.
- Define IncomingRef as a lightweight backlink on the target page: source_page_id, source_page_revision, source_block_span, and ref_span.
- Treat incoming refs as references to source-owned blocks, not duplicated block objects and not page-owned content.

## Page IDs, Names, And Paths

- Enforce strict portable page-name segments: reject empty segments, ./.., path separators, reserved Windows filename characters, control characters, .md suffix inside a segment, and ___ inside a segment.
- Map PageId(["A", "B", "C"]) to A___B___C.md; stream-backed pages live in top-level stream folders and use `yyyy_mm_dd.md` filenames.
- Treat every markdown file as one page and every page as one markdown file.
- Materialize missing parent pages as empty files when hierarchy requires it, e.g. A___B.md requires A.md.
- Keep rename/move as future first-class operations over PageId and path mapping, enabling crash-safe file moves and reference rewrites without hidden database state.

- Define SourceSpan as UTF-8 byte offsets into the exact file text version that was parsed.
- Preserve original file text and line endings during parse; do not normalize text before calculating spans.
- Store both block_span for the full block/subtree source range and content_span for the block’s own textual content.
- Parse references only outside code blocks.
- Support [[Page]] and simple-token #Page; multi-word page refs must use [[Page Name]].
- Reference targets always resolve to PageId; normal blocks are never valid reference targets.
- A block can contain multiple outgoing PageRefOccurrence values, each with its own ref_span and resolved target PageId.

## File-First Constraints

- Markdown files are the only durable source of truth.
- Caches, parsed block trees, outgoing refs, and incoming refs are disposable and rebuildable from files alone.
- Only Page has durable identity via PageId; Block has no ID, UUID, lifecycle, or standalone persistence.
- Incoming refs are derived display/cache data, not content owned by the target page file.
- Any span-based edit must include the source page revision/fingerprint and be rejected if the source file changed since parse.
- For validated frontend-originated edits, update the affected block/subtree and adjust only the outgoing/incoming refs impacted by that changed subtree.
- Use whole-page reparse as the fallback for external file changes, stale revisions, ambiguous structural edits, recovery, and initial workspace load.

## Assumptions

- The backend crate will be newly created in Rust.
- No durable global block UUIDs, page-block abstraction, markdown-block abstraction, or block-to-block refs will be introduced.
- Per-page span lookup will use simple tree traversal in v1; no separate block index is needed unless profiling later proves it necessary.
- Incoming refs may be stored directly on target Page caches as long as they remain explicitly derived and rebuildable.





# Block Tree Parser Plan

## Summary

Implement the parser around two backend block syntaxes, not one explicit syntax plus one implicit fallback:

- outliner block: starts with -
- plaintext block: starts with ◦

The parser must also support a legacy/fallback plaintext shape for existing non-Uniseq markdown:

- a contiguous non-list region without ◦ is still parsed as a plaintext block
- but it is treated as a compatibility parse result, so later edits can rewrite it into explicit ◦ syntax

This keeps the backend syntax aligned with the frontend model while still opening arbitrary markdown files safely.

## Implementation Changes

- Add a parser module with a public entrypoint like parse_blocks(text: &str) -> Result<Vec<Block>, CoreError>.
- Add a parser error type to CoreError, but keep parsing permissive for mixed existing markdown.
- Extend Block to distinguish block kind explicitly.
Add a small enum such as:
    - BlockKind::Outliner
    - BlockKind::Plaintext
- Add a second parser-level flag on plaintext blocks to distinguish:
    - explicit ◦ plaintext blocks from native Uniseq syntax
    - implicit plaintext blocks parsed from legacy/non-Uniseq markdown
    This can be a field on Block or a sub-variant, but it should be explicit so later edit/rewrite logic can converge implicit plaintext into ◦.
- Parse three source cases:
    - explicit outliner block: marker -
    - explicit plaintext block: marker ◦
    - implicit plaintext compatibility block: contiguous non--/non-◦ region not attached to a previous block
- Use visual indentation width for structure, accepting mixed tabs/spaces in existing files while preserving exact source bytes and spans.
- Define continuation for both explicit block types:
    - after - item, following non-marker lines belong to that block when their indentation reaches at least the content column after -
    - after ◦ text, following non-marker lines belong to that block when their indentation reaches at least the content column after ◦
- If a contiguous text region does not begin with ◦, parse it as an implicit plaintext block with multiline ownership rules analogous to explicit ◦ blocks.
- Define child nesting from greater indentation width on later - or ◦ marker lines.
Both block kinds may have children.
- Define block_span as the full source range owned by the block, including child blocks.
- Define content_span as the block’s own textual region only, excluding child block source ranges.
For - and ◦ blocks, this starts after the marker and following space.
For implicit plaintext blocks, this covers the owned text region directly.
- Treat triple-backtick fenced code regions as opaque for block-start detection.
- or ◦ inside a fence must not start blocks.
- Do not extract refs in this step; outgoing_refs remains empty.

## Assumptions And Defaults

- ◦ is real backend syntax for explicit plaintext blocks, not just a frontend rendering detail.
- Legacy non-Uniseq markdown without ◦ is supported by parsing it into implicit plaintext blocks for compatibility.
- Later edit/write paths should be able to converge implicit plaintext blocks into explicit ◦ syntax, but that rewrite behavior is out of scope for this parser step.
- Both - and ◦ blocks are first-class structural blocks in the tree.
- Fenced code handling is limited to triple backticks in v1.
- Ref extraction is the next step after block structure is stable.


# References And Indices Plan

## Summary

Implement backend reference extraction and indexing as the next core step. The goal is to make page references real parsed data, keep the workspace cache as the disposable in-memory index, and prepare the model so
page rename/rewrite work can be added later without reshaping the core types again.

## Key Changes

- Extend parse_blocks so every parsed Block gets outgoing_refs populated from its content_span.
- Support only documented page-reference syntax for now: [[Page]] and #Page.
- Keep WorkspaceCache as the only index layer for now. It should continue deriving:
    - page hierarchy from PageId
    - incoming references from parsed block outgoing_refs
- Add a small dedicated reference parser/scanner in the core layer rather than mixing token scanning deeply into block-structure parsing. parse_blocks should stay responsible for structure; reference extraction
should be a focused post-pass over completed blocks.
- Run that extraction during page load/discovery so discover_workspace returns pages whose blocks already carry outgoing refs, allowing existing incoming-ref rebuild logic to become meaningful without a new global
index type.
- Preserve source spans for each reference exactly enough to support future rename rewrites. PageRefOccurrence.ref_span remains the anchor for later text replacement work.

## Implementation Notes

- Treat [[A/B]] and #A/B as the canonical textual forms to support first, because they map cleanly onto existing PageId::from_str.
- For #Page scanning, only match hashtag references that start on a word boundary and continue through a page path token; avoid matching mid-word fragments or markdown headings accidentally.
- For [[...]], accept the inner text only if it parses as a valid PageId; otherwise ignore it.
- Walk blocks recursively after structural parsing and populate refs on every block from that block’s own content_span text slice.
- Do not introduce durable caches, block IDs, backlinks files, or a separate on-disk index.
- Do not plan linked-reference view APIs yet; callers can continue consuming Page.incoming_refs and Page::outgoing_refs() from the domain model.

## Assumptions

- Scope is backend core only; no frontend integration or new read/query API is included in this step.
- Syntax support is limited to page refs only, with no aliases, headings, block refs, or manual block-to-block links.
- This step should prepare for future rename/reference rewrites by preserving exact spans, but should not design or implement the rename transaction itself.
- Unknown or not-yet-existing target pages should not create new page records during parsing; references should only contribute incoming refs when the target page already exists in the workspace cache.





# Read APIs For Pages, Blocks, And Linked References

## Summary

Add a dedicated backend read/query layer that returns frontend-ready page and block snapshots while keeping markdown files and source pages as the only authoritative state. Linked references remain a derived read
view over source-owned blocks: the read API should return full editable block subtrees for immediate render/edit UX, plus opaque source handles that a future generic write API can round-trip unchanged.

## Key Changes

- Introduce a transport-agnostic read facade over WorkspaceCache, e.g. WorkspaceReadApi, with explicit non-mutating query methods.
- Keep Page, Block, IncomingRef, PageRefOccurrence, and WorkspaceCache as internal domain/state types, not the frontend contract.
- Define frontend-shaped read DTOs:
    - PageSummary: stable page identity, title, and hierarchy metadata for navigation.
    - PageDetail: page identity, title, child pages, root block snapshots, and page-level reference data needed for a page screen.
    - BlockSnapshot: resolved source-owned block subtree for display/edit, including content, kind, children, outgoing refs, and an opaque handle.
    - LinkedRefEntry: target-page-specific wrapper around a BlockSnapshot, including the exact reference occurrence that points to the target page.
- Define BlockHandle as an opaque round-trip token, not frontend-interpreted layout data. Internally it should contain:
    - source_page_id
    - source_page_fingerprint
    - block_span
- Keep ref_span in linked-reference results so one source block can safely support multiple reference occurrences.
- Expose unresolved outgoing refs on block/page reads, because they are part of source document state even when the target page does not exist.
- Keep incoming refs / linked refs resolved only for existing target pages in the cache, matching current backend indexing.

## Query Surface

- Add read methods for:
    - fetch one page summary/detail by PageId
    - list all pages as summaries
    - list child pages for a page
    - fetch root block snapshots for a page
    - fetch one block snapshot by BlockHandle
    - fetch linked references for a target page as editable source block snapshots
- PageId is the stable page identity in the read contract. workspace_path should not be required by the frontend.
- BlockSnapshot should preserve source order and recursive child structure so a linked-reference row can render the root block and its children immediately.
- LinkedRefEntry should include:
    - target page id
    - the specific ref_span that points at the target page
- The linked-references query is an alternate read over source-owned blocks, not a second ownership model and not a special edit surface.

## Implementation Notes

- Add a new read module rather than growing WorkspaceCache into both storage and presentation logic.
- Centralize block text resolution in the read layer using existing spans and page text, so callers never slice text manually.
- Treat resolved blocks in read responses as snapshots only. They are derived on demand and must not be stored as separate mutable backend state.
- The same BlockHandle must be returned whether a block is fetched from a page read or from linked references.
- Design the read contract to align with the future generic write path:
    - the frontend renders from BlockSnapshot
    - when editing starts, it reuses the embedded opaque BlockHandle
    - future writes will target the owning source page only, regardless of whether the edit started from page view or linked references
- Include a focused block-refresh query by handle so stale linked-reference rows can be reloaded individually after fingerprint mismatch.
- Keep this step transport-agnostic. Do not add Tauri commands, IPC wiring, rename logic, or file watching here.
- Do not introduce durable block IDs.

## Assumptions

- This step defines the backend read contract only, not transport wiring.
- Linked references must support immediate render and immediate edit entry in the frontend, so they return full BlockSnapshot subtrees rather than metadata-only rows.
- Resolved block snapshots in read responses are disposable projections; source pages remain the only authoritative state.
- The future write API will be source-anchored and origin-agnostic: edits from page view, linked references, or any other view will use the same opaque BlockHandle.





# Source-Anchored Block Subtree Markdown Edits

## Summary

Replace the current mixed text-plus-block mutation path with a markdown-first, file-backed subtree edit API. The new write flow should accept a BlockHandle plus replacement region markdown, splice that exact source
region into the owning page’s markdown file, reparse the resulting page, and refresh derived cache state from the new markdown. Parsed blocks, trees, and references remain outputs of the write, not writable inputs.

## Key Changes

- Replace IncrementalUpdate with a markdown-first edit request type built around:
    - block_handle: BlockHandle
    - replacement_markdown: String
- Remove replacement_blocks and new_text from the public subtree-edit contract. The caller should not supply parsed blocks or a precomputed whole-page string.
- Add a file-backed subtree edit API, likely on the workspace/discovery layer rather than only on WorkspaceCache, that:
    - resolves the source page from BlockHandle
    - rejects on fingerprint mismatch with CoreError::StalePageRevision
    - validates the block span against the current page text
    - replaces exactly that block_span region in the page markdown with replacement_markdown
    - reparses the full updated page with parse_blocks
    - writes the updated markdown to the page file
    - updates the cache from the reparsed page
    - rebuilds affected incoming refs and derived hierarchy as it does today
- Keep WorkspaceCache markdown-first as well:
- Preserve replace_page_blocks only if it remains a low-level whole-page replacement helper used internally; do not let it remain the main subtree-edit API.
- Keep writes source-anchored and origin-agnostic:
    - page view edits and linked-reference edits must both use the same BlockHandle
    - backend must not care where the edit originated
- Return either updated page/block snapshots or a success result that lets the caller re-read cleanly, but keep the contract centered on the markdown edit itself rather than block-tree patching.

## Implementation Notes

- Treat BlockHandle.block_span as the exact source region to replace. This is a subtree-region replacement, not a content-only patch.
- replacement_markdown should be interpreted as the complete markdown for that source block subtree region, including any bullets/indentation/children the frontend wants persisted.
- Apply stale handling as strict reject and refresh. Do not attempt block relocation, best-effort matching, or hidden retry logic.
- Write ordering should preserve file-first semantics:
    - compute updated full page text in memory
    - validate/reparse the updated text before touching the file when possible
    - write the final markdown to disk
    - refresh the cache from that exact written text
- If file write fails, do not partially advance cache state. Cache and file should stay aligned on failure.
- Reuse existing BlockHandle, PageId, SourceSpan, parse_blocks, and FileFingerprint types rather than introducing a second edit identity model.
- Keep later recovery/watch work out of scope here. This step should be correct without live invalidation; future watching only reduces stale frequency.

## Assumptions

- Replacement payload is region-markdown, not structured block edits.
- This step includes durable writes to the underlying page markdown file.
- Stale handles must hard-reject with refresh-required behavior; no relocation or hidden reconciliation is attempted.
- Frontend live invalidation remains a later step; this write API must still be correct when the frontend is operating on potentially stale snapshots.





# Crash-Safe Page Rename/Move With Reference Rewrites

## Summary

Add a file-backed, transactional page rename/move API that treats markdown files as the only source of truth. The operation should rename or relocate a whole page subtree, rewrite inbound page references in source
files that mention any moved page, and avoid leaving the workspace half-updated by using an explicit on-disk transaction/recovery record plus staged writes.

## Key Changes

- Add a dedicated rename/move module at the file boundary, parallel to files.rs, rather than embedding this into discovery or cache code.
- Introduce two explicit request types instead of one vague path-edit operation:
    - PageRename { source_page_id, new_leaf_name }
    - PageMove { source_page_id, destination_parent_page_id }
- Keep frontend semantics narrow:
    - rename changes only the leaf segment under the same parent
    - move changes only the parent path
    - both operations relocate the full descendant subtree
- Add a shared planning/validation phase that:
    - reads current workspace state from disk, not just cache
    - discovers the full moved subtree (source_page_id plus descendants)
    - computes old->new PageId mappings for every moved page
    - rejects if destination parent does not already exist for moves
    - rejects on any destination file/path collision
    - rejects if the source page/subtree no longer matches current disk state
- Rewrite inbound references for every existing source file that mentions any moved page, using current parsed refs and stored ref_spans from reparsed current disk contents.
- Limit rewrite scope to supported page-reference syntax only: [[Page]] and #Page.
- Keep parent-page behavior strict:
    - do not auto-create destination parents
    - do not auto-delete now-empty source parents
- Update the in-memory cache only after the transaction has committed on disk, by reloading the affected pages from the committed markdown and removing old page ids from cache.

## Transaction / Recovery Design

- Introduce a small on-disk transaction record in the workspace root for rename/move only, for example a hidden metadata file containing:
    - transaction id
    - operation kind (rename or move)
    - source subtree old->new page-id/path mapping
    - full set of affected files
    - original file contents for every file that will be changed or moved
    - final target contents/paths for every file that will exist after commit
    - transaction phase/status
- Use a transactional write flow:
    1. Preflight from current disk state and compute the full plan.
    2. Persist the transaction record before mutating workspace files.
    3. Write staged file contents needed for the final state.
    4. Apply final renames/writes atomically as far as the filesystem allows.
    5. Remove obsolete source files from the moved subtree only after final target files exist.
    6. Mark the transaction committed and remove the record when fully done.
    7. Refresh cache from the final disk state.
- Recovery on startup should inspect the transaction record:
    - if no record exists, do nothing
    - if a record exists, complete or roll back to a fully consistent state using the recorded original/final file sets
    - prefer a deterministic “finish to final intended state if possible, otherwise restore originals” policy, chosen once and documented in code/tests
- Keep this recovery scoped to rename/move transactions; do not generalize it to all operations yet.

## Rewrite Semantics

- When a moved page id changes, all inbound refs to that page anywhere in the workspace must be rewritten to the new canonical page id text.
- Rewrites should be based on current page text plus exact ref_spans from reparsed pages, not regex-only global replacement.
- If one source block contains multiple refs to moved pages, rewrite each occurrence in descending span order so offsets stay valid.
- Canonical replacement text should preserve syntax family:
    - [[A/B]] becomes [[X/Y]]
    - #A/B becomes #X/Y
- Only rewrite refs whose parsed target page id exactly matches an old moved page id.
- Do not attempt fuzzy rewrites, alias rewrites, or textual substitutions outside parsed reference occurrences.

## Public API / Types

- Export new file-backed operations from core, for example:
    - apply_page_rename(root, cache, request) -> Result<(), CoreError>
- Add specific error cases to CoreError for rename/move precondition failures, such as:
    - missing destination parent
    - destination collision
    - interrupted transaction detected / unrecoverable transaction state
- Keep read APIs unchanged structurally; callers refresh via the existing read layer after rename/move completes.

## Assumptions

- Rename and move are separate frontend operations.
- move may only target an already-existing destination parent page.
- The operation always moves the full descendant subtree.
- Source and destination parent cleanup/materialization are out of scope here: no auto-create, no auto-delete.
- Hard reject on external-state conflicts is the correct behavior; the frontend refreshes and retries.





# Recovery And Incremental File Watching

## Summary

Add a long-lived workspace runtime that owns startup recovery, cache refresh, file watching, and frontend invalidation. The runtime should keep markdown files as the only source of truth: on startup it recovers any
interrupted rename transaction, builds cache from disk, then watches the workspace for file changes and refreshes cache incrementally where safe, falling back to full rediscovery when needed. Frontend updates should
be invalidation-first, not pushed snapshots.

## Goals

- Recover interrupted rename/move transactions before normal operation starts.
- Keep WorkspaceCache fast and current without turning it into durable authority.
- Detect external file edits, subtree-edit writes, and rename/move side effects.
- Notify the Tauri/React frontend that relevant data is stale so it can re-read through existing read APIs.
- Preserve correctness by keeping stale-handle rejection as the final safety check even after watching exists.

## Runtime Shape

- Introduce a small long-lived owner, for example WorkspaceSession.
- WorkspaceSession should own:
    - workspace_root
    - current WorkspaceCache
    - watcher lifecycle/state
    - a queue or callback sink for invalidation events
    - serialization around writes, recovery, and watcher-applied refreshes
- Keep core parsing/read/write logic in existing modules.
- Use the runtime layer to coordinate:
    - startup recovery
    - cache bootstrap
    - file watcher start/stop
    - page/workspace refresh decisions
    - event emission to Tauri

## Startup Recovery

- On session startup:
    1. call recover_workspace_transactions(root, cache) before watcher startup
    2. load cache from disk with load_workspace_cache
    3. only then start watching
- Recovery scope for this step:
    - complete any interrupted rename/move transaction to its final intended state
    - refresh cache from recovered disk state
- If recovery fails:
    - surface a hard startup error
    - do not start the watcher against ambiguous disk state

## Watching Strategy

- Watch the workspace root recursively for relevant markdown and transaction-path changes.
- Ignore backend-owned temporary noise where possible, but do not assume perfect filtering.
- Treat watcher notifications as hints, not truth:
    - coalesce bursts
    - re-read disk before mutating cache
- Incremental refresh policy:
    - single markdown file create/change/delete: try single-page refresh path
    - rename transaction dir or ambiguous multi-file bursts: refresh whole workspace
    - any event affecting page-id mapping or many files: refresh whole workspace
- Keep the rule simple and conservative first. It is better to over-refresh than to let cache drift.

## Cache Refresh Semantics

- Reuse the explicit seams already in place:
    - refresh_page_in_cache for isolated file/page changes
    - refresh_workspace_cache for broad or uncertain changes
- Define clear behavior for file events:
    - file created: load page from path/page id and upsert into cache
    - file changed: reload that page from disk and upsert
    - file deleted: remove page from cache and rebuild derived refs/hierarchy
- If path-to-page-id interpretation is uncertain from the event alone, prefer full refresh.
- Any watcher-applied refresh must recompute derived references from current markdown, never patch cached refs directly.

## Invalidation Events To Frontend

- Use invalidation events, not pushed page/block snapshots.
- Event types should stay narrow and backend-shaped, for example:
    - workspace_reloaded
    - page_changed { page_id }
    - page_removed { page_id }
    - pages_changed { page_ids }
    - recovery_applied
- Tauri layer listens to runtime events and forwards them to React.
- React responds by re-querying via existing read APIs.
- Do not make the watcher choose UI-specific payloads or push PageDetail/BlockSnapshot over events.

## Coordination With Writes

- All file-backed writes should flow through the same runtime owner.
- During backend-initiated writes:
    - perform the write
    - refresh cache through the same disk-to-cache path
    - emit invalidation event(s)
- Watcher events caused by backend writes may still arrive afterward.
- Handle this by coalescing and tolerating redundant refreshes rather than trying to make watcher events perfectly disappear.
- Keep stale-handle rejection in subtree edits even after watcher invalidation exists.

## Transaction / Watcher Interaction

- Rename/move transaction directory should be watcher-aware:
    - startup recovery runs before watcher startup
    - if transaction-path events appear during runtime, prefer full workspace refresh
    - if an interrupted transaction record appears unexpectedly, treat it as a serious state change and run recovery or fail closed
- Watching must never become the recovery mechanism itself; recovery remains an explicit startup/runtime operation.

## Tauri Integration Boundary

- Keep the runtime backend-agnostic internally, but plan one thin Tauri adapter layer that:
    - constructs and stores the shared WorkspaceSession
    - exposes commands for startup, shutdown, reads, and writes
    - forwards invalidation events to the webview
- React should remain pull-based:
    - reads via commands
    - invalidation via Tauri events
    - no backend-owned frontend cache model

## Implementation Order

1. Add WorkspaceSession with startup bootstrap:
    - recover transactions
    - load cache
    - expose read access
2. Add watcher lifecycle and coarse whole-workspace refresh on any relevant event.
3. Add event coalescing/debounce and invalidation emission to Tauri.
4. Add incremental single-page refresh for isolated file changes.
5. Add targeted delete handling and conservative fallback-to-full-refresh logic.
6. Tighten watcher filtering and transaction-path handling once behavior is stable.

## Assumptions

- Recovery remains focused on rename/move transactions only.
- Frontend freshness is event-driven plus re-read, not live pushed snapshots.
- Conservative full refresh is acceptable as the initial fallback when event interpretation is uncertain.
- WorkspaceCache stays disposable and rebuildable from markdown files at all times.




# Plan: Realign the Rust Core for Frontend-Driven Content Editing

## Summary

Refactor the Rust core so it cleanly matches the product split:

- Rust owns page/block semantics: workspace discovery, parsing, hierarchy, normalized reference state, watcher reconciliation, and structural workspace mutations.
- React owns markdown editing behavior: editor UX, intent-to-markdown translation, and ordinary page-content file writes.

After this phase, ordinary content edits are no longer a backend write flow. Rust becomes the authoritative derived-state engine over markdown files, while structural operations remain backend-owned and synchronous.

## Key Changes

### 1. Reframe the core around two write classes

- Define ordinary content edits as frontend-driven direct file writes followed by Rust watcher/poll reconciliation.
- Define structural workspace mutations as backend-owned synchronous operations: create, delete, rename, move.
- Update core docs, naming, and public exports so this split is explicit and implementers do not treat ordinary content edits as backend mutations.

### 2. Remove backend-owned ordinary content editing from the runtime API

- Remove apply_block_subtree_edit from the intended public/runtime path.
- Remove WorkspaceSession::apply_block_subtree_edit as an app-facing operation.
- Keep revision-bound block anchors only where still useful for backend read-side correlation or tests, not as the main editing contract.
- Replace write-path assumptions in tests with direct file-change observation and reconciliation tests.

### 3. Shift the read surface toward normalized backend state

- Keep Rust responsible for maintaining normalized page/block/reference state.
- Reshape the read API so React can build presentation views from backend-maintained state rather than from high-level backend-composed UI queries.
- Favor reads that expose:
    - page summaries / page records
    - hierarchy relationships
    - parsed blocks per page
    - outgoing refs per block
    - incoming-ref index per page
- Treat linked references as a frontend-composed view over backend-maintained normalized reference state.
- Keep or slim page_detail only if it remains a thin aggregation over normalized state; avoid rich bespoke read models that assume backend-owned editing flows.

### 4. Make watcher reconciliation the primary path for ordinary content changes

- Treat watcher/poll-driven reconciliation as the primary content update mechanism for:
    - frontend-written page edits
    - external edits
    - sync-delivered file changes
    - recovery after interruptions
- Preserve and strengthen the existing page-local fast path:
    - single-file edits reparse only the changed page
    - update normalized reference/hierarchy state incrementally where possible
    - use full workspace refresh only as fallback
- Keep event coalescing and page-level invalidation as the primary runtime signal.
- Keep WorkspaceReloaded as fallback/advisory only.
- Preserve strict stale semantics for revision-bound block anchors:
    - ordinary file edits may invalidate prior anchors
    - callers refetch after reconciliation
- Explicitly defer any stable block identity redesign.

### 5. Add backend-owned structural operations for create/delete alongside rename/move

- Introduce backend-owned page create and delete commands so structural workspace mutations are consistently owned by Rust.
- Apply the same standards as rename/move:
    - hierarchy-aware
    - cache/index updates included
    - safe filesystem behavior
    - clear invalidation events
- Reuse transaction/recovery machinery where appropriate, but do not overcomplicate create/delete if a simpler invariant-preserving implementation is sufficient.
- Keep these operations synchronous and explicit; they are not part of the ordinary content-edit path.

### 6. Narrow and future-proof the public core surface

- Adjust exports to match the new direction:
    - expose normalized read/query types
    - expose watcher/session lifecycle and events
    - expose structural mutation commands
    - avoid exposing ordinary content-write helpers as first-class runtime APIs
- Keep WorkspaceSession as the core runtime owner for now:
    - read access
    - watcher lifecycle
    - reconciliation
    - structural mutations
- Keep event shapes suitable for a later service/Tauri layer:
    - changed page ids
    - removed page ids
    - workspace reload fallback
    - watcher mode/degradation diagnostics

- Replace subtree-edit write tests with reconciliation tests:
    - write page content directly to disk
    - trigger poll/watch reconciliation
    - assert updated blocks, refs, counts, and invalidation events
- Add structural operation tests for create and delete:
    - create root page
    - create nested page with hierarchy invariants
    - delete leaf page
    - delete hierarchy cases according to chosen deletion invariant
    - collisions / invalid operations
- Keep and extend watcher tests:
    - single-page content edits stay page-local
    - multi-file or ambiguous bursts fall back predictably
    - invalid workspace shapes still error or refresh correctly
- Add read-surface tests proving the normalized backend reference state is sufficient for frontend-composed linked-reference views.
- Run full cargo test and keep rename/move/recovery/watch behavior green.

## Assumptions and Defaults

- This phase is core-only; no Tauri/service layer implementation is included.
- Ordinary content edits are frontend-driven direct file writes.
- Rust remains the owner of structural mutations: create, delete, rename, move.
- Rust maintains the normalized page/block/reference graph; React composes UI views from that state.
- React must not reimplement page/block semantics by rescanning raw markdown or rebuilding its own reference engine.
- Quick eventual consistency is acceptable for ordinary content edits.
- Immediate synchronous consistency is required for backend-owned structural mutations.
- Revision-bound block anchors may remain internally, but any broader block identity redesign is deferred.




# Separate Full-Page Parsing from Narrow Cache Reconciliation

## Summary

Keep ordinary page edits on a full-page parse model: when one markdown file changes, reparse the entire file into a new block tree.

Change only the reconciliation step after parsing. For ordinary content edits to an existing page with unchanged PageId, update only the edited page’s content-derived state and the specific ref edges affected by
that page’s outgoing refs. Do not reuse the broader identity-level upsert_page path for those edits.

## Key Changes

- Split the current cache mutation behavior into two explicit operations:
    - upsert_page_identity(...) for page creation, deletion, rename, move, discovery, and any case where hierarchy or page identity may change
    - refresh_page_content(...) for ordinary text edits on an existing page with the same PageId
- refresh_page_content(...) should:
    - accept the existing page id plus newly parsed page content for that same page
    - replace only text, blocks, and fingerprint for the source page
    - compute old vs new outgoing refs for the source page
    - remove stale incoming refs only from old target pages referenced by the old content
    - add new incoming refs only to target pages referenced by the new content
    - preserve child_page_ids
    - preserve incoming refs into the edited page from other source pages
- Keep full-page reparse for changed files. Do not attempt block-local or subtree-local parsing in this pass.
- Keep existing broader refresh behavior for structural operations and watcher fallback paths.

## Session / Watcher Behavior

- For isolated Modified events on an existing markdown file whose path still resolves to the same PageId, the session should:
    - load the file
    - parse the full page
    - call refresh_page_content(...)
- Continue to fall back to full workspace refresh when current hierarchy safety checks fail or watcher classification is ambiguous.
- Keep rename/move/create/delete flows on the identity-level path; no structural optimization in this pass.

## Public API / Internal Contracts

- Keep external read APIs and WorkspaceEvent shapes unchanged.
- PagesChanged should continue to be the authoritative invalidation signal, but it should now include only the edited source page and target pages whose incoming_refs changed.

## Assumptions

- Full-page parse remains the correct unit because block spans and ref spans are page-text-relative and the model has no durable block ids.
- The optimization target is cache reconciliation scope, not parser granularity.
- Structural operations are intentionally out of scope for this pass.
- If a choice arises between maximum minimality and simpler correctness, prefer source-page plus touched-target-page updates over any page-global recomputation.




# Make Rename/Move Latency Vault-Size-Independent

## Summary

Refactor structural rename/move transactions to stop doing full-workspace discovery and full-cache refresh while the session write lock is held. Plan the transaction from the in-memory WorkspaceCache, then update
both WorkspaceCache and the session filesystem snapshot incrementally from the committed transaction’s known write/delete paths. The result should change rename/move latency from O(all pages) to O(affected pages)
plus direct file I/O for the transaction itself.

## Implementation Changes

- Replace transaction planning from disk with planning from the live cache.
    - In src/core/structure/mod.rs, remove the load_workspace_cache(root) call from plan_and_commit_transaction and call plan_transaction(cache, ...) directly.
    - Keep the existing preflight guards and recovery flow; the source of truth for planning becomes the locked in-memory cache after recover_workspace_transactions.
- Replace post-commit full cache refresh with targeted cache mutation.
    - Add a helper in the structural transaction flow that applies the committed plan to WorkspaceCache.
    - For every moved/renamed page ID, remove the old page entry and reinsert a page with the new PageId, new workspace path, and final text from the transaction plan.
    - For pages whose content changes but whose ID does not, reparse and upsert from final_text.
    - For deleted old paths/page IDs that are not reintroduced under a new ID, remove them from cache.
    - Do not call refresh_workspace_cache from rename/move commit or recovery paths once targeted mutation is in place.
- Make the transaction record expose enough committed state for incremental updates.
    - Add a read-only accessor or conversion on TransactionRecord that yields the committed write set and page mappings needed to update cache and snapshot after apply_final_state.
    - Do not reload files from disk for this step; use manifest/plan data already staged in the transaction.
- Update the session filesystem snapshot incrementally after structural writes.
    - Extend the write path so rename/move/move-recovery can return a mutation summary describing created/modified/deleted markdown paths.
    - In WorkspaceSessionState::apply_write, if the write returns a targeted snapshot delta, update fs_snapshot.markdown_files only for changed paths and set transaction_exists = false after successful transaction
    cleanup.
    - Preserve the existing fallback option: if a targeted snapshot update cannot be computed or validation fails, fall back to full snapshot capture, but that should be exceptional.
- Keep create/delete behavior unchanged for now unless needed for shared helpers.
    - The optimization target is rename/move. Do not broaden this refactor to page create/delete-subtree unless it is necessary to share a small internal utility.

## Public/Internal Interface Changes

- Change the structural write flow to return transaction-side metadata instead of only Result<(), CoreError>.
    - Introduce an internal result type for rename/move commit/recovery containing:
    - affected page IDs and final page contents needed for cache updates
    - affected relative paths and file stamps or enough path info to refresh them
- Keep external session APIs unchanged:
    - WorkspaceSession::apply_page_rename
    - WorkspaceSession::apply_page_move
    - No caller-visible behavior or wire format changes.
    - Add tests around rename/move commit to assert the cache is updated correctly without refresh_workspace_cache/load_workspace_cache on the hot path.
- Rename updates cache state correctly.
    - Renamed subtree pages appear under new IDs/paths.
    - Inbound/outbound refs are rewritten exactly as before.
    - Unaffected pages remain present and unchanged.
- Recovery path uses targeted updates too.
    - Stage a transaction, replay recovery, and assert final cache state matches a fresh discovery result.
- Session snapshot stays coherent after rename/move.
    - After apply_page_rename / apply_page_move, poll_once sees no synthetic broad diff caused by stale fs_snapshot.
    - Native/polling follow-up logic should not force an unnecessary full refresh immediately after a structural write.
- Regression coverage for conflicts and validation.
    - Destination already exists.
    - Missing source page.
    - Invalid move into descendant.
    - No-op rename/move still exits cheaply.

## Assumptions

- The live WorkspaceCache under the session write lock is authoritative enough for transaction planning once recover_workspace_transactions has run.
- plan_transaction needs only data already present in WorkspaceCache: page membership, hierarchy, workspace paths, and parsed refs.
- Full snapshot capture after rename/move is also part of the current latency problem and should be removed from the normal path, not just the two full cache discoveries.
- If targeted snapshot/cache update logic encounters an internal inconsistency, the implementation may fall back to the current full refresh/snapshot path as a safety valve, but the success path must remain
incremental.





# improve locks and transactions

1. Instrument the current path first.
   Add timing around:
   plan_transaction, complete_transaction_record, apply_transaction_*_to_cache, apply_incremental_snapshot_update, and the old/new page_event_states() diff path in src/core/session.rs and src/core/structure/mod.rs.
   This confirms whether event diffing is the dominant cost at 10K pages and gives a regression baseline.
2. Remove whole-cache diffing from structural writes.
   Refactor src/core/session.rs:431 so it no longer computes page_event_states() before and after the write for operations that already know what changed.
   That is the cleanest way to stop doing vault-wide work under the write lock.
3. Make structural writes return explicit event deltas.
   Extend IncrementalWorkspaceUpdate so it carries:

- changed_page_ids
- removed_page_ids
- existing written_paths
- existing deleted_paths

For rename/move, these IDs should come directly from the transaction plan and applied write set, not from recomputing a whole-workspace diff.

4. Populate those deltas in both normal commit and recovery.
   Update src/core/structure/mod.rs:256 so both:

- prepared-plan execution
- recovery replay from transaction record


5. Preserve incremental snapshot handling.
   Keep src/core/session.rs:610 as the file-level snapshot update mechanism.
   This should remove the main vault-size-sensitive work from the locked section.
7. Add focused tests.
   Add tests covering:

- rename emits only affected PagesChanged/PageRemoved
- move emits only affected PagesChanged/PageRemoved
- no structural success path triggers a full refresh
- recovery path emits the same deltas as normal commit





## Rename/Move Fresh-State Validation

### Summary

Keep the current transactional rename/move design, but harden it against stale-cache rewrites
by validating participating files against fresh disk state at commit time.

This preserves the existing architecture:

- ordinary content edits remain frontend/file-driven
- rename/move remain backend-owned transactional operations
- crash-safe forward recovery remains intact

The behavioral change is narrow: structural operations must fail rather than overwrite if
relevant files changed since planning.

### Implementation Changes

- Add a structural-operation validation step between planning and transaction apply.
- Continue using cache for initial responsiveness and impact computation, but treat it as
   provisional only.
- For each participating file in a rename/move transaction, capture an expected revision marker
   during planning.
   - Use whole-file fingerprints, consistent with current FileFingerprint semantics.
   - Include both moved subtree files and external files whose refs will be rewritten.
- Right before commit, reload or fingerprint-check all participating files on disk.
   - If every file still matches, proceed with staging and apply as today.
   - If any file differs, abort with a new conflict-style error instead of applying stale
      rewrites.
- Keep transaction recovery “finish forward”.
   - Validation applies before initial commit.
   - Once a transaction is staged and applying, recovery should continue driving disk to the
      staged final state.

### API / Type Changes

- Add a backend error for stale structural operations.
   - Example shape: CoreError::StructuralConflict or CoreError::StaleWorkspaceState.
   - Meaning: one or more files changed since the rename/move plan was prepared.
- Extend transaction planning state to carry expected source-file fingerprints for all
   - This must support both confirm-style move UI and optimistic rename UI with possible
      retry/failure handling.

### Assumptions

- Single-user with cross-device sync still requires stale-state protection because synced
   device edits are concurrent at the file level.
- Correctness is preferred over seamless structural-op UX.
- It is acceptable for rename/move to occasionally fail after initiation and require frontend
   refresh/retry handling.
- Whole-file fingerprint validation is the v1 policy; no semantic diff/merge behavior is added.




# Backend Hardening Plan with Controlled Stream Lifecycle

## Summary

Harden the backend around the actual product contract:

- normal pages support full structural operations: create, delete-subtree, rename, move
- stream pages support controlled lifecycle operations only: create and delete
- stream pages cannot be renamed or moved
- the backend remains markdown-native, file-first, and local-first
- the next engineering priorities are still: clarify contract, reduce session lock contention, and strengthen multi-file transaction durability

Default decisions in this plan:

- stream create/delete gets a dedicated backend API
- rename/move remain page-backed only
- [[...]] remains the general ref syntax, #... remains the compact syntax
- only triple-backtick fenced blocks are intentionally recognized

## Implementation Changes

### 1. Make the storage and lifecycle contract explicit

Update BACKEND_ARCHITECTURE.md and backend guards so the contract is explicit and enforced:

- Pages under pages/:
    - support create, delete-subtree, rename, move
    - parent materialization rules apply
- Stream pages under top-level `<stream-name>/`:
    - support create and delete only
    - do not participate in page hierarchy
    - never trigger parent-page materialization
    - cannot be renamed or moved
- Both storage kinds:
    - are discovered, parsed, indexed for refs, exposed through read APIs, and reconciled through watchers

Add explicit backend validation for unsupported operations:

- reject rename/move on stream-backed page ids with a dedicated CoreError
- keep delete semantics explicit:
    - page-backed delete means subtree delete
    - stream delete means single-file delete only

### 2. Add dedicated stream lifecycle operations

Introduce dedicated backend operations for stream lifecycle instead of overloading generic page structural APIs.

Recommended public shape:

- StreamPageCreate { stream_name, date_name }
- StreamPageDelete { stream_name, date_name }

Required behavior:

- StreamPageCreate
    - creates `<stream-name>/<yyyy_mm_dd>.md`
    - fails if the target file already exists
    - parses and inserts the new page into cache
    - does not create any parent page
- StreamPageDelete
    - deletes exactly one stream file
    - fails if the stream page does not exist
    - refreshes refs and cache state like any other file removal
- PageCreate, PageDeleteSubtree, PageRename, PageMove
    - remain for page-backed pages only
    - must reject stream ids up front where applicable

Do not add stream rename/move APIs in this pass.

### 3. Reduce WorkspaceSession write-lock hold time

Refactor WorkspaceSession so disk IO and parsing happen off-lock, and only cache application / swap plus event emission happen under the write lock.

Decision-complete behavior:

- Incremental reconciliation:
    - detect changed relative paths
    - read and parse changed files off-lock into Page values
    - derive deleted ids off-lock
    - reacquire lock
    - validate snapshot assumptions
    - if assumptions no longer hold, fall back to full refresh
    - otherwise apply page/stream updates, update snapshot state, and emit events
- Full refresh:
    - build a fresh cache and filesystem snapshot off-lock
    - reacquire lock
    - swap in the new state and emit diff events
- Structural writes:
    - remain serialized
    - perform as much non-mutating prep work off-lock as possible
    - commit cache state and events under lock

Keep WorkspaceEvent semantics unchanged.

### 4. Strengthen rename/move durability for page-backed pages

Keep the manifest-based transaction model, but replace direct destination overwrites with temp-file promotion.

Commit protocol:

1. plan rename/move and rewritten contents
2. stage manifest and final blobs in the transaction directory
3. write each final blob to a temp file in the same directory as its destination
4. atomically rename temp -> final for each destination
5. after all final files exist, delete old source files
6. remove the transaction directory last

Recovery rules:

- recovery always completes to final committed state, never rolls back
- recovery must be idempotent
- recovery must safely handle interruption:
    - before temp creation finishes
    - after some temp files exist
    - after some final promotions complete
    - before old source deletions complete

This hardening applies to page rename/move only. Stream create/delete do not need multi-file transactional rewrite machinery.

### 5. Keep incremental reconciliation narrow, but cheaper

Preserve the current conservative policy, but remove avoidable whole-cache work.

Changes:

- replace whole-cache clone simulation for parent-preservation checks with affected-id validation
- parent materialization remains a discovery/full-refresh behavior only
- if an incremental update would require structural healing, fall back to full refresh
- stream file create/delete should remain incrementally reconcilable because they have no hierarchy repair requirements

This keeps correctness simple while improving scale characteristics.

## Public APIs / Types

Add or adjust these interfaces:

- New operations:
    - StreamPageCreate
    - StreamPageDelete
- New errors:
    - dedicated error for unsupported structural operation on a stream page
- Keep existing page-backed operations:
    - PageCreate
    - PageDeleteSubtree
    - PageRename
    - PageMove
- Keep current read/query APIs unchanged
- Keep PageId and PageLocation model unchanged in this pass
