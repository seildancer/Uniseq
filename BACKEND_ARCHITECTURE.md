## Rule of Thumb

- Backend owns durable markdown semantics: workspace/page discovery, page parsing into blocks, derived indexes, source anchors, and safe file mutations.
- Frontend owns interaction semantics: rendering, editor UX, routing user actions, and translating intent into markdown edits or narrow backend write commands.
- Backend APIs should expose authored blocks, source anchors, and queryable derived data; frontend code decides how to present and compose that data.

## Core Rules

- Markdown files are the only durable source of truth.
- Every markdown file is a page.
- Journal files and normal page files are the same backend object.
- User-authored body content is modeled as blocks.
- The core authored relationship is `block -> page`.
- `linked references` are derived views over source blocks and never live in the target page file.
- Pages are identified by filename stem for now.
- Page IDs must be globally unique across the workspace.
- Page rename is a first-class backend operation and must rewrite references atomically.
- Disposable indexes and caches may exist, but the backend must be rebuildable from files alone.
- File-level sync is the working storage assumption, but sync logic is out of scope for this rewrite.

## Backend Responsibilities

- Discover markdown pages from the workspace.
- Parse page files into authored block trees.
- Extract source anchors and derived block annotations such as page references and task markers.
- Build derived indexes such as incoming references, search hits, and task rollups.
- Expose authored block trees and query endpoints for derived data such as incoming references, search hits, and task rollups.
- Apply source-anchored markdown text edits by reparsing affected pages and refreshing derived indexes page-locally.
- Apply page rename as an atomic transactional write.

## Internal Shape

- Each parsed page carries an explicit authored `BlockTree`.
- Block identity is page-local and disposable.
- Search, timeline, task, and incoming-reference indexes are derived from block traversal, not flat text slices.
- Ordinary text edits refresh only changed markdown pages in memory before projections are rebuilt.

## Frontend Responsibilities

- Render blocks and page screens.
- Compose page screens from backend-authored block trees plus separately queried derived data.
- Handle editor UX and convert user actions into markdown text edits.
- Route edits performed from linked-reference views back to the source page.

## Non-Goals

- Durable global block UUIDs.
- Manual `block -> block` references.
- Hidden database state as the real product model.
- Sync-specific merge logic in this stage.
