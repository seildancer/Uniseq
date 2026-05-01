# Phase 03 — Incremental Indexing, Query Layer, and Cache

## Goal

Build the disposable derived state that powers fast journal, page, reference, task, timeline, and search views.

## Architecture Sources

- `architecture_new_uniseq/engine/indexing.md`
- `architecture_new_uniseq/engine/caching.md`
- `architecture_new_uniseq/engine/modules.md`
- `architecture_new_uniseq/model/views.md`
- `architecture_new_uniseq/features/references.md`
- `architecture_new_uniseq/features/search.md`
- `architecture_new_uniseq/features/tasks.md`

## Scope

- Create the derived index from parsed markdown.
- Maintain outbound references, incoming references, page projections, task rollups, and timeline data.
- Add full-text and filtered search indexes.
- Support eager parsing of changed files and lazy refresh of broader derived projections.
- Persist local cache for startup speed while preserving full rebuild safety.
- Add file watching and invalidation events.

## Deliverables

- Rust index/query/cache modules.
- Query APIs for journal dates, page view data, backlinks, tasks, timeline, and search.
- Cache rebuild, partial invalidation, and full invalidation commands.
- Benchmarks against realistic fixture workspaces.

## Acceptance Criteria

- Deleting `.cache/` does not lose user content and can be recovered by full rebuild.
- Editing one journal file updates directly affected references and search results promptly.
- Page views render derived incoming references without writing them into target page files.
- The query layer hides cache implementation details from React.

## Risks

- Missed cross-file invalidation may show stale backlinks or task rollups.
- Premature graph-database complexity could conflict with the markdown-first product model.

## Exit Gate

The engine can serve all core read projections from markdown-derived indexes with deterministic rebuilds and reliable invalidation events.
