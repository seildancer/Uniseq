# Plugin Capabilities

Plugins should be able to request capabilities explicitly.

## Core capabilities

- read journal content
- append journal entry
- read page view
- create or update page file
- search
- list tags/pages
- read tasks
- create commands
- add panels or views
- access selected text/entry
- read/write plugin-local data

## Restricted capabilities

- filesystem access outside workspace
- sync APIs
- account APIs
- network access
- raw file writes

Restricted capabilities should require user approval.

