# Entities

## Workspace

A workspace is a local folder containing journals, optional pages, assets, config, and disposable cache data.

## Journal

A journal is a date-named markdown file.

It is the primary write target.

## Entry

An entry is a parsed segment of markdown inside a journal or page file.

Entries do not require permanent UUIDs.

For indexing and UI operations, the engine can assign ephemeral runtime IDs, but those IDs are cache details.

The normal source locator for an entry is:

- file path
- source span within the file
- optional validation snippet or hash

## Page

A page is identified by its normalized page path, or by `namespace/title` when namespaced.

A page can have:

- optional markdown content stored on the page itself
- incoming entries from journals and other pages shown in the page view
- pinned entries
- saved view settings
- aliases

## Tag

A tag is a compact page link.

`#health` and `[[health]]` should resolve to the same page title unless the product later chooses different display semantics.

## Asset

An asset is a file referenced from markdown.

Assets are stored under an assets directory and indexed by path, checksum, metadata, and references.

## Edge

An edge is a derived relation from a source entry to a target page.

Edges are not hand-authored objects. They are parsed from markdown.

## Source anchor

A source anchor is the runtime mapping from a derived item back to markdown source.

Default source anchor:

- `file_path`
- `span`
- optional snippet or hash for validation

This is not a global durable block identity.

It is a runtime locator used for navigation and edits from derived views.
