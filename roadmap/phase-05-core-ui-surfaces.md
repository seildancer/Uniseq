# Phase 05 — Core UI Surfaces

## Goal

Ship the primary user experience: journal-first writing with calendar navigation, page views, search, tasks, and timeline projections.

## Architecture Sources

- `architecture_new_uniseq/app-shell/react-vite.md`
- `architecture_new_uniseq/app-shell/state-routing.md`
- `architecture_new_uniseq/app-shell/editor.md`
- `architecture_new_uniseq/app-shell/tauri.md`
- `architecture_new_uniseq/features/journal.md`
- `architecture_new_uniseq/features/pages.md`
- `architecture_new_uniseq/features/search.md`
- `architecture_new_uniseq/features/tasks.md`
- `architecture_new_uniseq/features/references.md`

## Scope

- Build Tauri + React + Vite shell.
- Implement routing, layout, command palette foundation, and app state boundaries.
- Integrate markdown editor with Rust-backed reads/writes.
- Build journal surface with date navigation and built-in calendar.
- Build page view combining page-owned content and derived incoming sections.
- Build search UI for full-text and filtered results.
- Build task views and timeline/calendar navigation.

## Deliverables

- Desktop app shell that can open a workspace and render indexed data.
- Editor integration for journal entries and page-owned content.
- Core navigation components: journal, page view, search, tasks, timeline.
- UI state tests and command API integration tests.

## Acceptance Criteria

- The default route centers the daily journal.
- Calendar is built-in, not an optional plugin.
- Page incoming references and task rollups are rendered as derived UI sections and not persisted into page markdown.
- Search results navigate back to source anchors.
- UI writes call Rust commands only.

## Risks

- UI convenience can accidentally blur derived views into editable stored content.
- Editor integration can become the latency bottleneck if it requests too much data synchronously.

## Exit Gate

A user can open a workspace, write in today's journal, create links/tags, browse derived pages, search content, and manage tasks in the desktop shell.
