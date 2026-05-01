# Indexing and Query Engine

Indexing is a core Rust responsibility.

## Index inputs

- journal markdown files
- page markdown files
- assets and metadata
- config files

## Index outputs

- page registry
- incoming edges by page title
- entries by date
- tasks by status and target
- assets by reference
- full-text search index
- related page suggestions

All index outputs are derived and disposable.

## Incremental indexing

The engine should eagerly reparse and update indexes for the changed file.

Cross-file or global refresh should be coarse and lazy by default.

Examples:

- refresh backlinks for an opened page
- recompute graph neighborhoods when graph view opens
- rebuild secondary suggestion caches during idle time

Full reindex should be available and safe, but normal usage should not require reparsing the whole workspace.

## Benchmark

The intended benchmark is Obsidian-style behavior:

- markdown files are canonical
- metadata and search caches are persisted for speed
- caches may be rebuilt safely
- file changes update derived state without making the cache itself canonical

## Query examples

- entries tagged `project-x`
- open tasks linked to `client-a`
- entries from last 30 days mentioning `budget`
- all pages with incoming links but no page file
- assets referenced from `research`
