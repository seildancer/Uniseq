# Database Layer

The current codebase uses DataScript as the internal working database.

## Responsibilities

- store the working graph model
- support Datalog queries
- serve derived views
- support transaction listeners
- persist and restore serialized DB state

## Design choice

The DB is an implementation detail of the current app architecture, not the product identity.

A rebuild can keep the same document model without keeping the exact same database technology.

