# Storage

The storage layer keeps user-owned files as the durable source of truth.

It covers:

- filesystem abstraction
- DB snapshot persistence
- idle-based writes
- sync to remote or alternate storage
- file watching
- graph restore and migration

