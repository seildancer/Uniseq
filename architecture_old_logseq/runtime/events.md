# Events and Handlers

The app is event-driven.

## Event flow

1. A user action or platform event occurs.
2. A handler receives it.
3. The handler updates state or performs a transaction.
4. Derived caches and views refresh.
5. The UI rerenders.

## Handler families

- editor handlers
- page handlers
- file handlers
- search handlers
- repo and config handlers
- plugin handlers
- route and shell handlers
- whiteboard and draw handlers
- import/export handlers

## Design choice

The handler layer is where cross-cutting actions live. It is not the same thing as UI components.

UI components describe what is visible. Handlers describe what happens.

