# Logseq Compatibility Contract

This document defines what "compatible with Logseq when applicable" means for Uniseq.

It is not a promise of full product parity.

It is the contract agents should use when deciding whether to preserve, adapt, warn about, or intentionally drop behavior from a Logseq workspace opened in Uniseq.

## Compatibility Priority

When multiple designs fit Uniseq equally well:

- preserve Logseq-compatible markdown conventions
- preserve Logseq-compatible workspace layout conventions
- preserve import and migration simplicity

When compatibility conflicts with the core Uniseq model, Uniseq wins and the loss must be documented.

## Preserve As Directly As Possible

- local-first workspace ownership
- markdown files as canonical content
- journals as date-named markdown files
- tags and page links as authored references
- aliases
- namespace-style page names
- backlinks as derived views
- task extraction from markdown checkboxes
- assets referenced from markdown

## Adapt But Keep The User Outcome

- page views:
  preserve the ability to gather related material, but implement it as derived incoming references rather than copied graph content
- page hierarchy:
  preserve page organization, but model hierarchy at the page/product layer rather than as disk folders
- block-oriented editing:
  preserve good editing ergonomics, but do not make every block a durable graph entity
- zoom/focus:
  preserve the affordance, but keep it ephemeral rather than block-identity-based
- queries and dashboards:
  preserve useful derived views, but route edits through canonical markdown writes
- sync:
  preserve cross-device continuity, but keep it file-first rather than entity-first

## Intentionally Unsupported

- manual block refs by UUID
- manual block embeds/transclusion by UUID
- universal stable UUIDs on normal blocks
- block-level graph identity as the center of the product
- durable graph-database-first page assembly

These are not temporary omissions. They are core product divergences unless a future document changes that decision explicitly.

## Expected Losses When Opening A Logseq Workspace In Uniseq

The following behavior should be considered lost or degraded unless a future feature restores it explicitly:

1. Manual block refs using UUIDs will not resolve as first-class editable references.
2. Manual block embeds/transclusions using UUIDs will not render with original Logseq semantics.
3. Workflows that depend on durable block identity across sessions will degrade to file-and-span-based navigation where possible.
4. Any feature that assumes the canonical model is a block graph rather than markdown files plus derived indexes will be flattened into the Uniseq markdown-first model.

This first item is the most important compatibility warning and should be surfaced clearly in migration/import UX.

## Recommended User-Facing Warning Categories

- `supported`: behavior preserved closely enough that no warning is needed
- `adapted`: behavior preserved, but through a different model or UX
- `read-only/degraded`: visible or partially usable, but not fully editable with original semantics
- `unsupported`: not preserved in Uniseq

## Implementation Rule

Do not silently emulate unsupported Logseq behavior with hidden storage tricks that violate the Uniseq model.

If preserving a behavior requires introducing:

- hidden block IDs on all content
- graph-native durable state as the primary source of truth
- target-file backlink mutations

then that behavior should be treated as unsupported unless a new architecture decision explicitly approves the divergence.

## Related Docs

- [from-logseq.md](./from-logseq.md)
- [logseq-feature-disposition.md](./logseq-feature-disposition.md)
- [../product/principles.md](../product/principles.md)
