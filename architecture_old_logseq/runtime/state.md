# Application State

The app uses a central runtime state object for coordination.

## State split

- document state: pages, blocks, files, DB, sync metadata
- UI state: sidebar state, modals, editor focus, scroll positions, theme, selection
- background state: sync jobs, file queues, persistence jobs, network status

## Why this split matters

Document state must be durable and shareable across views. UI state is often ephemeral and session-local.

Separating them keeps:

- file writes from being triggered by purely visual state
- derived views stable while UI changes
- background jobs from depending on component internals

## Communication

The runtime uses channels, atoms, reactive subscriptions, and event handlers for coordination.

That gives the app:

- predictable mutation flow
- decoupled background tasks
- straightforward rerendering

