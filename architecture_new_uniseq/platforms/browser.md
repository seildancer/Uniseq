# Browser

The browser target uses the same product and engine model where possible, but relies on browser-safe storage and permissions.

## Constraints

- limited direct filesystem access
- browser routing
- IndexedDB or similar local persistence
- File System Access API where available
- web-compatible background behavior

## Design rule

Browser support is first-class, not an afterthought.

The architecture should not assume Tauri-only capabilities in core product flows.
