# PDF, Whiteboard, and Flashcards

These features can live mostly in TypeScript because they are UI-heavy and benefit from JS libraries.

## PDF

- viewer
- highlights and annotations
- capture annotation into journal
- link annotation to tags/pages

PDF annotations may use stable feature-scoped IDs if that improves persistence and source mapping.

## Whiteboard

- canvas UI
- cards that reference pages, journal entries, or assets
- optional drawing shapes
- saved as separate whiteboard files

Whiteboard files are separate durable data, not markdown pages.

Whiteboard shapes and references may use stable feature-scoped IDs where needed.

## Flashcards

- derive cards from tagged entries or explicit syntax
- review UI in React
- scheduling data stored separately from markdown content

Flashcards are not pages by default.

They should normally be derived from markdown content rather than represented as one markdown page per card.

The Rust engine should still own persistence and indexing for any durable data.
