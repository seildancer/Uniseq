# Markdown Writes

The engine should avoid rewriting files unnecessarily.

## Common write operations

- append entry to today's journal
- toggle task checkbox
- edit selected markdown span
- rename page title across files
- update page front matter
- move asset and update references

## Write safety

Every write should:

- read the latest file version
- validate expected ranges or hashes
- apply the smallest reasonable patch
- write atomically
- emit index invalidation events

Invalidation may be broader than the minimal semantic impact if that keeps the system simpler.

Prefer correct coarse invalidation plus lazy recomputation over fragile eager global maintenance.

## Source anchors for derived edits

When a derived view edits markdown-owned content, the engine should locate the source using a source anchor.

Default anchor:

- `file_path`
- `span`
- optional snippet or hash for validation

Write flow:

1. load the latest file version
2. validate that the span still matches the expected content
3. apply the smallest reasonable patch
4. fall back to snippet or structural rematch within the same file when safe
5. reject or surface conflict when the anchor is stale and cannot be rematched confidently

## No hidden block IDs by default

Normal markdown entries should not receive hidden IDs.

If a future feature needs stable anchors, that feature must justify a visible or opt-in metadata convention.

## Feature-scoped stable IDs

Some non-markdown or cross-session feature surfaces may justify stable IDs.

Examples:

- whiteboard shapes
- PDF annotations
- other separate durable feature files

These IDs are feature-scoped exceptions, not a universal content rule for normal markdown blocks.
