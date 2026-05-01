# Rust Core Engine

The Rust core owns expensive work and disposable derived state.

Markdown files remain canonical.

The engine is the source of truth for runtime projections, indexes, cache contents, sync mechanics, and file operations.

The UI calls the engine through Tauri commands or a thin IPC/event bridge.
