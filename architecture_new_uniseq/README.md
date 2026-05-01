# Uniseq Architecture

This folder is the rebuild spec for `uniseq`.

Use it as a handoff package for agents building the app from scratch. The goal is to make it as much as compatible as logseq but with the deliberate divergences from Logseq.

The original Logseq source code is available at `../og` from the repository root and should be used as the reference implementation when checking Logseq behavior or visual details.

Uniseq is not a Logseq clone.

It keeps the strongest parts of the local-first markdown workflow and drops the block-addressable graph as the center of the product.

The core product promise is:

> Let me write naturally in the daily journal, using tags and page links, and let the app turn that into useful pages and views without making me manually reorganize everything.

## Start Here

Read in this order:

1. [AGENT_HANDOFF.md](./AGENT_HANDOFF.md) for the build-oriented summary.
2. `product/` for what the app is and is not.
3. `model/` for the source-of-truth data model.
4. `engine/` for the Rust ownership boundary.
5. `app-shell/` and `features/` for UI composition.
6. `sync/`, `plugins/`, and `platforms/` for extension and deployment constraints.
7. `reference/` for Logseq compatibility and explicit divergences.

If a decision is ambiguous, use this tiebreaker:

- prefer the simpler markdown-first design
- prefer the design that keeps derived views derived
- prefer the choice that preserves Logseq-compatible workspace conventions when it does not conflict with the new product model

## Non-Negotiable Decisions

- Daily journal is the primary writing surface.
- Markdown files are the durable source of truth.
- Tags and page links create authored directed edges from source entries to pages.
- Pages are views first and documents second.
- Incoming references, task rollups, and similar sections are derived and must not be written back into page files just to maintain the view.
- The graph is unidirectional in how it is authored, even though projected cycles may exist.
- Stable block UUIDs are not required for normal markdown content.
- Manual block refs and block embeds are intentionally out of scope.
- Rust owns correctness-sensitive and latency-sensitive work.
- React + TypeScript own UI composition, editor integration, and JS-heavy feature surfaces.
- The UI style should stay as close to Logseq as practical unless a documented Uniseq product divergence requires otherwise.
- Persisted caches are allowed, but they are disposable and rebuildable.

## Recommended Reading By Concern

### Product and scope

- [product/principles.md](./product/principles.md)
- [product/feature-scope.md](./product/feature-scope.md)
- [product/non-goals.md](./product/non-goals.md)

### Durable model

- [model/entities.md](./model/entities.md)
- [model/markdown-contract.md](./model/markdown-contract.md)
- [model/page-identity.md](./model/page-identity.md)
- [model/file-layout.md](./model/file-layout.md)
- [model/graph.md](./model/graph.md)
- [model/views.md](./model/views.md)

### Engine and write path

- [engine/modules.md](./engine/modules.md)
- [engine/api-boundary.md](./engine/api-boundary.md)
- [engine/indexing.md](./engine/indexing.md)
- [engine/caching.md](./engine/caching.md)
- [engine/writes.md](./engine/writes.md)

### UI shell and feature surfaces

- [app-shell/state-routing.md](./app-shell/state-routing.md)
- [app-shell/editor.md](./app-shell/editor.md)
- [features/journal.md](./features/journal.md)
- [features/pages.md](./features/pages.md)
- [features/references.md](./features/references.md)
- [features/tasks.md](./features/tasks.md)

### Compatibility and migration

- [reference/from-logseq.md](./reference/from-logseq.md)
- [reference/logseq-compatibility-contract.md](./reference/logseq-compatibility-contract.md)
- [reference/logseq-feature-disposition.md](./reference/logseq-feature-disposition.md)

## Folder Map

- `product/`: product intent, scope, and exclusions
- `model/`: durable workspace contract and derived view model
- `engine/`: Rust core responsibilities and write/index rules
- `app-shell/`: React, routing, editor, settings, desktop shell
- `features/`: user-facing modules layered on top of the model
- `sync/`: local-first guarantees and sync protocol shape
- `plugins/`: future JS plugin runtime and permissions
- `platforms/`: desktop, browser, and mobile adaptation layers
- `reference/`: compatibility notes, feature coverage, and Logseq comparisons
