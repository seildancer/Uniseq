# Pages and Journals

Pages are the primary navigation and content unit.

## Pages

Pages are named containers. They can be created, renamed, aliased, tagged, referenced, and linked from blocks.

Important page behaviors:

- page titles are normalized
- page names can be namespaced
- pages can have aliases and tags
- page properties can be inferred from special blocks
- page routes map directly to page views

## Journals

Journals are date-driven pages with special handling.

The codebase treats them differently from ordinary pages because they are:

- auto-created on schedule
- linked to today and nearby dates
- surfaced in dedicated journal views
- used heavily for daily capture workflows

## Page-level relations

The page system supports:

- namespace parents and child pages
- tagged page relations
- alias source pages
- unlinked references

These relations are what make the app feel like a knowledge system instead of a plain note editor.

