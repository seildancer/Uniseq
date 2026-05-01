# Uniseq Implementation Roadmap

This roadmap converts `architecture_new_uniseq/` into implementation phases. Each phase is persisted as a separate markdown file and should be completed in order unless an explicit dependency is removed.

## Phase Index

1. [Workspace and Markdown Contract](./phase-01-workspace-markdown-contract.md)
2. [Journal/Page Parsing and Page Identity](./phase-02-parsing-page-identity.md)
3. [Incremental Indexing, Query Layer, and Cache](./phase-03-indexing-query-cache.md)
4. [Canonical Write Path and Safe Mutations](./phase-04-canonical-write-path.md)
5. [Core UI Surfaces](./phase-05-core-ui-surfaces.md)
6. [Navigation, Settings, Onboarding, and Assets](./phase-06-navigation-settings-onboarding-assets.md)
7. [Local-First Sync and Conflict Flows](./phase-07-sync-conflict-flows.md)
8. [Logseq Compatibility and Migration Hardening](./phase-08-logseq-compatibility-migration.md)
9. [Extended Features, Platforms, and Plugins](./phase-09-extended-features-platforms-plugins.md)

## Roadmap Principles

- Markdown files are the durable source of truth.
- Rust owns correctness-sensitive and latency-sensitive work.
- React + TypeScript own UI composition and editor/visual feature surfaces.
- Derived views must remain derived and must not be written into target page files.
- Normal markdown content must not receive hidden durable block IDs by default.
- Cache data may persist for speed, but it must always be disposable and rebuildable.
- Desktop/Tauri should ship first, while browser and mobile constraints remain visible.
