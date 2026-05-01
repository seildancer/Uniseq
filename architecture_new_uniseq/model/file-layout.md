# File Layout

The workspace should stay understandable without the app.

Recommended layout:

```text
workspace/
  journals/
    2026-04-29.md
    2026-04-30.md
  pages/
    project-x.md
    people___alice.md
    areas___work___client-a.md
  whiteboards/
    project-x.edn
  pdf/
    annotations.db
  assets/
    2026/
      image.png
  app/
    config.toml
    views.toml
    sync.toml
  .cache/
    index/
    thumbnails/
```

## Journals

Journals are the canonical capture stream.

File names should use ISO dates: `YYYY-MM-DD.md`.

## Pages

Page files are optional, but when they exist they are always markdown files.

They are used for:

- curated page notes
- page descriptions
- pinned sections
- persistent page settings

A page can exist without a page file if it is only a tag target.

Page storage is flat.

Namespace-like page names do not imply nested directories.

Recommended filename mapping:

- `project-x` -> `pages/project-x.md`
- `people/alice` -> `pages/people___alice.md`
- `areas/work/client-a` -> `pages/areas___work___client-a.md`

The app should treat the escaped filename as a storage detail.

## Whiteboards and Other Feature Files

Whiteboards are separate durable files.

They are not markdown pages.

Other feature-specific durable data such as PDF annotation state or flashcard review state may also live outside markdown when justified.

Those files remain local-first workspace data, but they are not part of the markdown page model.

## Cache

The cache is disposable.

It may contain search indexes, parsed projections, thumbnails, and sync acceleration data. Deleting the cache must not delete user content.
