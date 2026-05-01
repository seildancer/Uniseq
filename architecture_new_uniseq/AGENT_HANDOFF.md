# Uniseq Agent Handoff

This is the shortest complete description of what agents should build.

Read this first, then drill into the linked detailed docs.

## Product In One Paragraph

Uniseq is a local-first markdown workspace centered on the daily journal. Users write naturally in journal entries and page notes using tags and page links. The app turns those authored links into derived page views, task views, timeline views, search results, and optional graph or whiteboard surfaces. Pages are not maintained by copying content into them. They are mostly projections over incoming references plus optional page-owned markdown content.

## Primary Product Rules

- The journal is the default writing surface.
- Calendar is a built-in primitive, not an optional extra.
- Pages are views before documents.
- Hierarchical page navigation is a first-class UX surface.
- The app should reduce manual reorganization, not demand it.
- Derived views may summarize and navigate, but should not become an alternate graph-native editing model.

Detailed docs:

- [product/principles.md](./product/principles.md)
- [features/journal.md](./features/journal.md)
- [features/pages.md](./features/pages.md)

## Durable Source Of Truth

- Canonical content is markdown files in the workspace.
- Journals are date-named markdown files.
- Page files are optional markdown files.
- A page may exist without a page file if it only exists as a link or tag target.
- Assets and some feature-specific files may live outside markdown.
- Cache data is persisted for speed but always disposable.

Recommended layout:

```text
workspace/
  journals/
  pages/
  assets/
  whiteboards/
  pdf/
  app/
  .cache/
```

Detailed docs:

- [model/file-layout.md](./model/file-layout.md)
- [model/markdown-contract.md](./model/markdown-contract.md)

## Core Domain Model

- `workspace`: local folder plus config and disposable cache
- `journal`: primary dated markdown file
- `entry`: parsed markdown segment inside a journal or page
- `page`: normalized page identity with optional markdown content and derived incoming content
- `tag`: compact page link
- `asset`: referenced file
- `edge`: derived directed relation from a source entry to a target page
- `source anchor`: runtime locator back to markdown source, usually `file_path + span + optional snippet/hash`

Important constraints:

- Entries do not have durable UUIDs by default.
- Tags and `[[Page]]` links resolve to the same page identity when they spell the same title.
- Namespace is part of page naming, not a filesystem tree.
- Notion-style hierarchy is a page-level product hierarchy, not a disk-folder hierarchy.

Detailed docs:

- [model/entities.md](./model/entities.md)
- [model/page-identity.md](./model/page-identity.md)

## Graph Model

The graph is authored in one direction:

```text
source entry -> target page
```

Sources can be journal entries or page notes. Targets are pages. Incoming references are computed, indexed, and rendered. They are not stored back into target files.

The graph can still contain cycles at the projection level. What is removed is not cyclicity. What is removed is the product assumption that every block must be a durable graph object.

Detailed docs:

- [model/graph.md](./model/graph.md)
- [features/references.md](./features/references.md)

## Page Model

A page view combines:

- optional page-owned markdown content
- pinned entries
- incoming journal or page entries
- related pages and tags
- open tasks
- optional assets or feature-specific related material

Only the page-owned markdown content and page metadata are stored in the page file. Everything else is derived.

Detailed docs:

- [model/views.md](./model/views.md)
- [features/pages.md](./features/pages.md)

## Write Model

All durable mutations go through Rust.

Common operations:

- append journal entry
- edit selected markdown span
- toggle task checkbox
- rename page across files
- update page front matter
- move asset and update references

Write safety rules:

1. Read the latest file.
2. Validate the expected source anchor.
3. Apply the smallest reasonable patch.
4. Write atomically.
5. Emit invalidation events.

Normal markdown content must not receive hidden block IDs by default.

Detailed docs:

- [engine/writes.md](./engine/writes.md)
- [engine/api-boundary.md](./engine/api-boundary.md)

## Runtime Ownership Boundary

Rust owns:

- filesystem access
- markdown parsing
- indexing
- search
- sync
- durable settings and persistence
- cache lifecycle
- conflict detection
- background workers

React + TypeScript own:

- routing
- navigation shell
- editor integration
- visual rendering
- command palette
- panels and modals
- plugin host UI
- JS-heavy features like graph drawing, whiteboard UI, PDF UI, and flashcard UI

TypeScript must not write workspace files directly.

UI style guidance:

- Keep Uniseq's visual style, interaction feel, spacing, and information density as close to Logseq as practical.
- Use the original Logseq source code at `../og` from the repository root as the reference when checking existing Logseq UI behavior or implementation details.
- Deviate visually only when the Uniseq product model explicitly requires it, and document the reason.

Detailed docs:

- [engine/modules.md](./engine/modules.md)
- [app-shell/editor.md](./app-shell/editor.md)
- [app-shell/state-routing.md](./app-shell/state-routing.md)
- [reference/framework-decision.md](./reference/framework-decision.md)

## Indexing And Cache Strategy

- Parse changed files eagerly.
- Update outbound refs and local search data eagerly enough for immediate UI feedback.
- Refresh broader derived views lazily unless profiling proves otherwise.
- Persist derived caches locally for startup speed.
- Always allow a safe full rebuild.
- Benchmark expected responsiveness against Obsidian-style behavior, not against a graph-database-native product.

Detailed docs:

- [engine/indexing.md](./engine/indexing.md)
- [engine/caching.md](./engine/caching.md)

## Main User-Facing Surfaces

Core:

- journal
- built-in calendar
- page view
- hierarchical page tree
- search
- tasks
- timeline

Strong optional features:

- graph visualization
- PDF workflows
- whiteboards
- flashcards
- plugin APIs
- templates
- quick capture

Detailed docs:

- [product/feature-scope.md](./product/feature-scope.md)
- [features/README.md](./features/README.md)

## Logseq Compatibility Contract

Uniseq should stay compatible with Logseq where the new product model allows it, but compatibility is not the primary goal.

Preserve where possible:

- local markdown workspace ownership
- journals
- tags and page links
- page aliases
- namespace-style page naming
- backlinks as a derived view

Intentionally diverge:

- no universal block UUID model
- no manual block refs
- no block embeds/transclusion
- no graph-native page assembly
- no durable graph database as the product center

When a Logseq workspace is opened in Uniseq, lost or degraded behavior must be explicitly documented rather than silently ignored.

Detailed docs:

- [reference/logseq-compatibility-contract.md](./reference/logseq-compatibility-contract.md)
- [reference/logseq-feature-disposition.md](./reference/logseq-feature-disposition.md)

## Suggested Build Order

1. Workspace discovery, file layout, and markdown contract.
2. Journal and page parsing.
3. Page identity resolution and tag/link resolution.
4. Incremental indexing and query layer.
5. Canonical write path with source anchors.
6. Journal UI, calendar, page view, search, and task views.
7. Hierarchical page tree and settings/onboarding.
8. Sync and conflict flows.
9. Optional features: graph, PDF, whiteboard, flashcards, plugins.

## Open Constraints To Keep In Mind

- Browser and mobile are first-class targets, even if desktop ships first.
- Some feature-specific files may justify stable IDs, but normal markdown blocks do not.
- Derived sections can feel rich in the UI, but the storage contract must remain simple.
- If a feature needs a more complex model than markdown-first projections can support, that divergence should be explicit and feature-scoped.
