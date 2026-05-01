# Framework Decision

## Chosen stack

- Tauri for the native app shell
- Rust for core engine and hot paths
- React + Vite for UI
- TypeScript for plugin runtime and UI-heavy features

## Why not Next.js as the main shell

The app is offline-first, local-first, and editor-heavy.

Server rendering and server routes are not central to the product.

React ecosystem reuse is still available through Vite.

## Rust scope

Rust should own:

- filesystem
- parsing
- indexing
- search
- sync
- persistence
- migrations
- background workers

TypeScript should own:

- React UI
- editor integration
- plugin host
- feature surfaces
- visual modules
- command palette and layout

