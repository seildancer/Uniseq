# Latency and Background Work

Sync and indexing must not block editing.

## Background work

Run these in Rust background tasks:

- file watching
- parsing changed files
- updating indexes
- syncing changes
- thumbnail extraction
- PDF metadata extraction where appropriate

## UI contract

The UI should receive status updates:

- idle
- indexing
- syncing
- offline
- conflict
- error

The editor should remain usable during all of these states.

