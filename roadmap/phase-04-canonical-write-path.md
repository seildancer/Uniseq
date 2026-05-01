# Phase 04 — Canonical Write Path and Safe Mutations

## Goal

Make Rust the single durable mutation path for journals, pages, tasks, page metadata, assets, and page renames.

## Architecture Sources

- `architecture_new_uniseq/AGENT_HANDOFF.md`
- `architecture_new_uniseq/engine/writes.md`
- `architecture_new_uniseq/engine/api-boundary.md`
- `architecture_new_uniseq/app-shell/editor.md`
- `architecture_new_uniseq/reference/logseq-compatibility-contract.md`

## Scope

- Implement write operations:
  - append journal entry
  - edit selected markdown span
  - toggle task checkbox
  - rename page across files
  - update page front matter
  - move asset and update references
- Enforce the write safety sequence:
  1. read latest file
  2. validate expected source anchor
  3. apply smallest reasonable patch
  4. write atomically
  5. emit invalidation events
- Prevent TypeScript from writing workspace files directly.
- Add conflict/error responses for stale anchors.

## Deliverables

- Rust command API for all supported mutations.
- Atomic write utilities and backup/recovery behavior.
- Multi-file rename implementation with validation and rollback strategy.
- Write-path test suite with stale-anchor, concurrent-edit, and malformed-file cases.

## Acceptance Criteria

- Toggling a task modifies only the intended markdown marker.
- Editing an entry fails safely when the source anchor no longer matches the current file.
- Renaming a page updates relevant `#tag` and `[[Page]]` references atomically or leaves the workspace unchanged.
- Every successful write emits enough invalidation events for the UI and index to refresh.

## Risks

- Page rename propagation is the highest-risk core mutation because it spans many files.
- Overwriting user changes because of stale editor state would undermine trust.

## Exit Gate

All core mutations are exposed through Rust commands, covered by tests, and integrated with index invalidation; no UI code needs raw filesystem writes.
