# Views

Views turn natural writing into usable surfaces.

## Page view

A page view shows:

- optional persisted page markdown content
- pinned entries
- incoming journal entries
- related tags and pages
- open tasks
- assets and annotations

Only the page's own markdown content is stored in the page file.

Incoming references, task rollups, and other derived sections are rendered from indexed data in the frontend.

## Journal view

The journal view is the main writing surface.

It should support:

- today's note
- previous/next day navigation
- calendar jump
- built-in calendar surface
- quick entry
- inline task capture
- tag suggestions

## Calendar view

Calendar is a first-class built-in view tied to the journal system.

It should support:

- month and week browsing
- jump to day
- visibility into journal density and task density
- filtered views by page, tag, or task state when useful

## Tag view

Tag view and page view can share the same implementation.

The distinction can be visual: a tag is usually lightweight and collection-like, while a page may have curated content.

## Task view

Tasks are extracted from markdown checkboxes and optional markers.

Task views should support:

- all open tasks
- tasks by tag/page
- due/scheduled date if syntax is added
- completed task history

Edits from task views should apply to the source markdown file through the same canonical write path used by the editor.

## Timeline view

The timeline shows entries across dates filtered by page, tag, search, or task status.

This is one of the most important surfaces because it preserves the journal-first model.
