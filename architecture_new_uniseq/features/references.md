# References and Backlinks

References are authored in source notes.

The source markdown owns the link.

The target page does not store copied backlink data in its own file.

## Reference forms

- tags such as `#project-x`
- page links such as `[[Project X]]`
- task context links when tasks mention pages

## Backlink behavior

Backlinks are derived from indexed source notes and rendered in the frontend.

They should support:

- filtering by date
- filtering by source page or journal
- filtering by task state where relevant
- grouping by date, source, or recency

## Source navigation

Going back to the source page is enough for correctness.

Best-effort block-level return may exist using cached spans, but it is not part of the durable data model.

Derived reference items should carry a source anchor such as:

- `file_path`
- `span`
- optional snippet or hash

## Unlinked references

Unlinked references are optional and can be deferred.

If supported later, they should remain derived suggestions rather than stored page mutations.
