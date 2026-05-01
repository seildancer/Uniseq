# Logseq Feature Disposition

This document is the coverage checklist for `architecture_new`.

Its purpose is not to force one-to-one product parity.

Its purpose is to ensure the new architecture explicitly accounts for every major feature family present in the current Logseq architecture, even when the answer is "drop", "replace", or "defer".

Status labels:

- `keep`: should exist in the new app in roughly the same role
- `adapt`: should exist, but with a different data model or UX
- `separate`: should remain a feature, but as a separate file/model rather than markdown-derived content
- `defer`: recognized and intentionally postponed
- `drop`: intentionally not part of the new product

## Core model and writing

- journals: `keep`
- pages: `keep`
- block-structured editing: `adapt`
- outliner tree editing: `adapt`
- page aliases: `keep`
- tags as page links: `keep`
- namespaces: `adapt`
- page properties: `adapt`
- page files as optional authored documents: `keep`
- backlinks: `keep`
- unlinked references: `defer`
- block refs: `drop`
- block embeds/transclusion: `drop`
- stable UUIDs on all blocks: `drop`
- zoom/focus views: `adapt`
- sidebar block workflows: `defer`

## Editing system

- insert/split/delete/move subtree: `keep`
- collapse state: `adapt`
- keyboard-driven editing: `keep`
- selection and cursor behavior: `keep`
- formatting commands: `keep`
- block/page conversion flows: `adapt`
- command palette driven mutations: `keep`
- paste transforms: `keep`
- markdown serialization: `keep`
- block content normalization: `adapt`

## Search, queries, and derived views

- full-text search: `keep`
- page search: `keep`
- block or entry search: `adapt`
- task search: `keep`
- query tables: `adapt`
- custom queries: `defer`
- query builders: `defer`
- reactive query refresh: `keep`
- index rebuilds: `keep`
- filterable backlinks: `adapt`
- timeline views: `keep`
- calendar navigation: `keep`

## Tasks

- checkbox tasks: `keep`
- tasks by page/tag: `keep`
- today/open/completed task views: `keep`
- scheduled/deadline data: `adapt`
- repeated tasks: `defer`
- advanced task workflows/query language: `defer`

## Graph and visual features

- graph visualization: `keep`
- graph picker / graph selection: `adapt`
- discovery-oriented graph navigation: `keep`
- graph as primary editing model: `drop`
- image/lightbox handling: `keep`
- code rendering: `keep`
- math rendering: `keep`
- slide/presentation support: `defer`

## Whiteboard and canvas

- whiteboard/canvas feature: `keep`
- shapes and connectors: `keep`
- embedded content cards: `keep`
- whiteboard route and dedicated files: `keep`
- deep block integration inside whiteboard: `drop`

## PDF and research workflows

- PDF viewer: `keep`
- PDF annotation/highlights: `keep`
- capture annotation into notes: `keep`
- PDF search helpers: `adapt`
- Zotero integration: `drop`

## Import, export, and publishing

- markdown workspace portability: `keep`
- OPML export: `defer`
- HTML export: `defer`
- zip/package export: `defer`
- external document conversion: `defer`
- Logseq graph compatibility import: `adapt`
- repo conversion flows: `defer`
- static publishing app: `defer`
- broad data exchange story: `adapt`

## Plugins and customization

- startup and lifecycle hooks: `keep`
- slash commands: `keep`
- commands and shortcuts: `keep`
- panels/views: `keep`
- plugin settings/config: `keep`
- themes: `keep`
- services/integrations: `adapt`
- plugin marketplace: `defer`
- resource registration: `adapt`

## UI and product surfaces

- routing/layout shell: `keep`
- sidebar and panel structure: `adapt`
- right sidebar as a core block workflow surface: `defer`
- page/journal/file views: `adapt`
- all-files browser: `defer`
- settings: `keep`
- onboarding: `keep`
- notifications: `keep`
- modals: `keep`
- bug report/debugging views: `adapt`

## Sync, storage, and persistence

- filesystem abstraction: `keep`
- file watching: `keep`
- local-first files as canonical storage: `keep`
- DB snapshot persistence: `adapt`
- durable graph database as product center: `drop`
- disposable local cache/index persistence: `keep`
- full and incremental sync: `keep`
- sync metadata tracking: `keep`
- file-diff based sync: `adapt`
- graph restore and migration: `adapt`

## Platforms

- desktop app: `keep`
- browser app: `defer`
- mobile app: `defer`
- CLI-compatible parser/index pieces: `adapt`

## Explicit divergences from current Logseq

- manual block refs and embeds are removed
- block-level graph identity is no longer the center of the product
- markdown remains canonical rather than a serialized view of a canonical graph store
- derived views should not quietly become a second graph-native editing model
- whiteboard remains important, but not as a block graph surface
- sync remains file-first rather than entity-first

## Gaps still needing dedicated docs in `architecture_new`

These areas are recognized here but are not yet described in enough detail elsewhere:

- all-files / file browser surfaces
- themes and plugin marketplace story
- import/export and publishing detail
