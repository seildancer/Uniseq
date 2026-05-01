# Phase 06 — Navigation, Settings, Onboarding, and Assets

## Goal

Complete the essential product shell around the core surfaces so the app feels coherent and safe for real workspace use.

## Architecture Sources

- `architecture_new_uniseq/app-shell/settings-onboarding.md`
- `architecture_new_uniseq/app-shell/state-routing.md`
- `architecture_new_uniseq/features/pages.md`
- `architecture_new_uniseq/features/references.md`
- `architecture_new_uniseq/model/file-layout.md`
- `architecture_new_uniseq/product/feature-scope.md`

## Scope

- Implement hierarchical page navigation as a first-class UX surface.
- Add workspace onboarding, open/create flows, and safe degraded-state messaging.
- Add durable settings owned by Rust.
- Implement asset and attachment handling through Rust commands.
- Add tag/page autocomplete and navigation refinements.
- Add appearance/theme settings if not already included in the app shell.

## Deliverables

- Hierarchical page tree UI based on page identities/namespaces.
- Onboarding and workspace selection flows.
- Settings storage and settings UI.
- Asset import/move/reference update commands and UI.
- Autocomplete for tags and page links.

## Acceptance Criteria

- Namespace-style pages can be browsed hierarchically without requiring nested disk folders.
- New users can create a workspace and understand the journal-first model.
- Settings persist locally and do not require server ownership.
- Assets are moved or referenced through Rust-owned write APIs.

## Risks

- Confusing namespace hierarchy with filesystem hierarchy can create migration and compatibility problems.
- Settings can become a dumping ground unless they are clearly separated from workspace content.

## Exit Gate

The app has complete workspace lifecycle UX, durable settings, hierarchical navigation, autocomplete, and asset handling sufficient for daily single-device use.
