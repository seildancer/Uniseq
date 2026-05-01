# Tauri Shell

Tauri provides the native app host.

## Responsibilities

- desktop window management
- native filesystem permissions
- Rust command bridge
- native menus and shortcuts where appropriate
- update packaging if used
- background tasks through Rust

## Design choice

The Tauri bridge should be narrow.

React calls stable app commands. It should not know internal Rust module layout.

