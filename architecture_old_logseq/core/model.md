# Core Data Model

The application is page-first, but block-structured (outliner).

## Primary entities

- `graph` or repository: the user-owned workspace
- `page`: a named document container
- `journal page`: a date-based page with creation and navigation behavior
- `block`: an ordered unit inside a page
- `file`: the on-disk representation of a page or asset
- `tag` and `alias`: page-level naming and cross-reference systems
- `query result`: a derived view, not a canonical object

## Shape of content

The runtime model represents content as a tree of blocks rather than a flat document string.

That gives the app:

- nesting
- block movement
- subtree operations
- property extraction from structural locations
- sidebar and zoomed views

Markdown remains the interchange format, but the runtime model is structural.

## Identity

The current codebase uses stable block identity internally. That identity supports references, embeds, focused views, and tree mutation.

If the rebuilt app intentionally drops block-level refs and embeds, stable block IDs become unnecessary. The product then becomes page-first and markdown-first, with document structure but no block-addressable behavior.

## Derived data

The system also derives data such as:

- backlinks
- namespace relations
- scheduled and deadline data
- page aliases
- search index records
- collapsed state in the UI

These are outputs of the model, not the source of truth.

