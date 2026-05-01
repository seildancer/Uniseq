# Sync

Sync is separate from persistence.

Persistence keeps the local graph healthy. Sync reconciles local state with remote or alternate file systems.

## Core behavior

- full sync and incremental sync
- local-to-remote and remote-to-local passes
- idle flushes
- file-diff based updates
- sync metadata tracking

## Important boundary

Sync works on files and file changes, not on UI state.

That separation keeps the app portable and makes the sync engine reusable outside the UI shell.

