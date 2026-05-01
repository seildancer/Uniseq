# Phase 07 — Local-First Sync and Conflict Flows

## Goal

Add single-user cross-device sync without making any server the source of truth.

## Architecture Sources

- `architecture_new_uniseq/sync/local-first.md`
- `architecture_new_uniseq/sync/protocol.md`
- `architecture_new_uniseq/sync/latency.md`
- `architecture_new_uniseq/engine/modules.md`
- `architecture_new_uniseq/product/non-goals.md`

## Scope

- Define sync manifests, hashes, versions, and file-level change tracking.
- Implement background sync workers in Rust.
- Preserve full offline operation.
- Detect conflicts conservatively at file level.
- Surface conflict states clearly in the UI.
- Avoid real-time collaborative editing and multiplayer semantics.

## Deliverables

- Sync protocol implementation and local sync state store.
- Background worker scheduling and cancellation.
- Conflict file creation and conflict summary APIs.
- UI for sync status, errors, and conflict resolution entry points.
- Tests for offline edits, same-file conflicts, journal appends, and cache rebuild after sync.

## Acceptance Criteria

- Local markdown files remain canonical.
- The app is fully usable offline.
- Sync never requires derived cache state to be authoritative.
- Conflicts are not silently overwritten.
- Conflict UX makes clear which file/content requires user attention.

## Risks

- Journal appends across devices may be common and need especially careful conflict presentation.
- Server-assisted sync must not drift into server-owned content.

## Exit Gate

Users can sync a workspace across devices with robust offline behavior, visible sync status, and safe conflict handling.
