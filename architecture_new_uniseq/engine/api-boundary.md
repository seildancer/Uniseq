# Engine API Boundary

The Rust engine should expose capabilities, not internal data structures.

## Command style

Use command/request APIs for user actions:

- `open_workspace(path)`
- `get_journal(date)`
- `append_journal_entry(date, markdown)`
- `update_file(path, patch)`
- `get_page_view(page_key, options)`
- `search(query, options)`
- `get_tasks(filter)`
- `sync_now()`

## Event style

Use event streams for changes:

- workspace opened
- file changed
- index rebuilt
- page view invalidated
- search index updated
- sync status changed
- conflict detected

## Boundary rule

TypeScript should not mutate durable files directly.

It should issue commands to Rust and receive events or query results.

This keeps plugins and UI features powerful without letting them corrupt the workspace.
