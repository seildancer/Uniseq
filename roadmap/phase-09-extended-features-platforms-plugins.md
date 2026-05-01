# Phase 09 — Extended Features, Platforms, and Plugins

## Goal

Add optional high-value features and broader platform support while keeping the core markdown contract simple.

## Architecture Sources

- `architecture_new_uniseq/features/graph.md`
- `architecture_new_uniseq/features/pdf-whiteboard-flashcards.md`
- `architecture_new_uniseq/plugins/runtime.md`
- `architecture_new_uniseq/plugins/api-shape.md`
- `architecture_new_uniseq/plugins/capabilities.md`
- `architecture_new_uniseq/platforms/browser.md`
- `architecture_new_uniseq/platforms/mobile.md`
- `architecture_new_uniseq/product/feature-scope.md`

## Scope

- Add graph visualization as a projection, not the product's storage center.
- Add PDF, whiteboard, and flashcard workflows with feature-scoped IDs only where justified.
- Build plugin runtime and capability-gated API shape.
- Enforce plugin restrictions in Rust, especially around filesystem and workspace mutation.
- Adapt app architecture for browser and mobile constraints after desktop is stable.
- Add templates and quick capture if they fit the core journal-first workflow.

## Deliverables

- Graph UI backed by existing derived edges.
- PDF/whiteboard/flashcard feature modules and storage rules.
- Plugin host, manifest format, capability model, and safe API bridge.
- Browser/mobile feasibility implementation plans and platform-specific prototypes.
- Extended-feature regression tests that prove core markdown remains clean.

## Acceptance Criteria

- Optional features do not require hidden block UUIDs in normal markdown content.
- Plugins cannot mutate workspace storage directly.
- Graph, whiteboard, PDF, and flashcard data remain feature-scoped and documented.
- Browser/mobile adaptations do not weaken desktop local-first guarantees.

## Risks

- Extended features can introduce model complexity that leaks into the core architecture.
- Plugin APIs are hard to retract once public, so initial capability boundaries must be conservative.

## Exit Gate

Optional features and platform expansions are available behind clear boundaries, with no regression to the journal-first, markdown-first core product model.
