# Frontend State and Routing

Frontend routing should reflect product surfaces:

- today
- journal date
- calendar
- page view
- search
- tasks
- graph
- whiteboard
- PDF
- settings
- plugins

## Navigation model

The main navigation should center on:

- journal and calendar
- hierarchical pages
- search
- tasks

Graph, whiteboard, flashcards, PDF, plugins, and similar extra surfaces should not dominate the primary sidebar.

Prefer a Notion-style page tree in the main navigation and place extra feature entry points in a secondary top bar, right utility rail, or contextual actions.

## Frontend state owns

- current route
- panel layout
- editor focus
- selected entries
- pending command state
- optimistic UI state
- ephemeral collapse state

## Rust owns

- workspace files
- parsed entries
- indexes
- sync state
- durable settings
- search data

Collapse state is frontend sugar.

It may be cached locally for UX, but it is not part of the durable content model and need not sync.
