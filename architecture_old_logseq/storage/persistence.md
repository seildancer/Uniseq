# Persistence and Caching

The app keeps a serialized cache of the graph for faster startup and recovery.

## Persistence flow

- restore cache if available
- inject built-in pages
- migrate old schemas if required
- install DB listeners
- persist when the app is idle enough

## Why it exists

This is a performance and resilience layer.

It reduces startup cost and makes recovery from reloads or crashes faster than reparsing everything from disk every time.

