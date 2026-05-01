# Phase 08 — Logseq Compatibility and Migration Hardening

## Goal

Make Logseq-like workspaces understandable, safely degraded where necessary, and convenient to import or open under Uniseq's product model.

## Architecture Sources

- `architecture_new_uniseq/reference/from-logseq.md`
- `architecture_new_uniseq/reference/logseq-compatibility-contract.md`
- `architecture_new_uniseq/reference/logseq-feature-disposition.md`
- `architecture_new_uniseq/product/non-goals.md`

## Scope

- Preserve compatible behavior for local markdown workspace ownership, journals, tags, page links, page aliases, namespace-style page naming, and derived backlinks.
- Explicitly document degraded or unsupported behavior for block refs, block embeds, universal block UUIDs, graph-native page assembly, and advanced block queries.
- Add compatibility scanner/reporting for existing workspaces.
- Add convenience import/export paths where they fit the markdown-first model.
- Ensure compatibility handling does not reintroduce a block-addressable graph as the product center.

## Deliverables

- Compatibility report command and UI.
- Import/open checklist for Logseq-like workspaces.
- Degraded-feature warnings and documentation links.
- Regression fixtures based on common Logseq workspace patterns.

## Acceptance Criteria

- Opening a Logseq-like workspace does not silently pretend unsupported block features are fully supported.
- Compatible tags, page links, journals, aliases, and backlinks work as Uniseq projections.
- Unsupported constructs remain visible enough for users to preserve or manually resolve them.
- The migration layer does not add hidden block IDs to normal markdown content.

## Risks

- Trying to match every Logseq behavior would conflict with the deliberate architecture divergences.
- Silent degradation could cause user data interpretation mistakes.

## Exit Gate

Users can open or import compatible markdown workflows with clear reporting of what works, what degrades, and what is intentionally out of scope.
