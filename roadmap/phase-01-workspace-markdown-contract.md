# Phase 01 — Workspace and Markdown Contract

## Goal

Establish the durable local-first storage contract that every later subsystem depends on.

## Architecture Sources

- `architecture_new_uniseq/README.md`
- `architecture_new_uniseq/AGENT_HANDOFF.md`
- `architecture_new_uniseq/model/file-layout.md`
- `architecture_new_uniseq/model/markdown-contract.md`
- `architecture_new_uniseq/model/entities.md`

## Scope

- Define and implement workspace discovery/opening.
- Support the canonical workspace layout:
  - `journals/`
  - `pages/`
  - `assets/`
  - `whiteboards/`
  - `pdf/`
  - `app/`
  - `.cache/`
- Define journal file naming and page file naming rules.
- Define the supported markdown subset and front matter handling.
- Model core entities: workspace, journal, entry, page, tag, asset, edge, source anchor.
- Treat `.cache/` as disposable and rebuildable.

## Deliverables

- Rust workspace module that can create, open, validate, and inspect a workspace.
- Workspace config and migration versioning placeholders.
- Markdown contract tests for journals, page files, links, tags, tasks, aliases, front matter, and assets.
- Fixture workspaces covering empty, minimal, mixed journal/page, and Logseq-like layouts.

## Acceptance Criteria

- Opening a workspace never requires a server or network connection.
- Existing markdown files are not rewritten during discovery.
- Unsupported or degraded Logseq constructs are detected or documented, not silently treated as fully supported.
- Cache deletion followed by app restart can rebuild all derived state from markdown.

## Risks

- Over-normalizing file names too early may break compatibility.
- Allowing TypeScript to write files directly would violate the ownership boundary.

## Exit Gate

The app can open a local workspace, identify journal/page/assets/config/cache locations, parse basic markdown files, and report a structured workspace summary through a Rust-owned API.
