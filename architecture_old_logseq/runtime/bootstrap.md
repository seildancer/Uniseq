# Bootstrap and Startup

The app bootstrap path is intentionally small.

## Startup sequence

1. Load plugin host hooks.
2. Initialize routing.
3. Mount the root page shell.
4. Restore repo state and config.
5. Register listeners and background loops.
6. Start UI and event processing.

## Entry points

The main entry namespace sets the router, mounts the root component, and starts sync or restore flows depending on the platform.

The top-level startup code should not contain domain logic. It should only coordinate the startup sequence.

## Background setup

During startup the runtime also sets up:

- network watchers
- file watchers
- date/journal watchers
- command registration
- instrumentation hooks
- persistent-var loading

This is why startup is more than just rendering a page.

