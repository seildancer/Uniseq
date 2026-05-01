# Engine Modules

Recommended Rust modules:

```text
engine/
  workspace/
  parser/
  model/
  index/
  query/
  writer/
  sync/
  search/
  assets/
  tasks/
  events/
  api/
```

## `workspace`

Discovers workspaces, validates layout, reads config, and manages graph lifecycle.

## `parser`

Parses markdown into entries, links, tasks, assets, headings, and metadata.

## `model`

Defines durable domain structs: workspace, page title, journal date, entry, edge, task, asset.

## `index`

Builds and maintains derived projections from files.

These projections are caches, not canonical content.

## `query`

Answers structured questions from UI and plugins.

## `writer`

Performs safe markdown writes and append operations.

## `sync`

Handles cross-device sync, conflict detection, file manifests, and remote protocol.

## `search`

Builds full-text and fielded search indexes.

## `assets`

Tracks attachments, checksums, metadata, thumbnails, and references.

## `events`

Publishes changes to the TypeScript layer.

## `api`

Exposes stable commands and event payloads to Tauri.
