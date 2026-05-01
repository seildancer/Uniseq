# Logseq Architecture Blueprint

This folder documents the codebase as a rebuild guide.

The structure is hierarchical:

- `core/` for the document model and user-facing primitives
- `runtime/` for bootstrap, routing, state, and event flow
- `storage/` for filesystem, persistence, sync, and database caches
- `editing/` for outliner behavior, commands, and text mutation
- `ui/` for shell layout, navigation, components, and search surfaces
- `features/` for major optional capabilities
- `platforms/` for browser, desktop, and mobile bridges
- `deps/` for the separately packaged libraries that support the app

Start at the top-level indexes in each folder, then drill into the specific topics.

