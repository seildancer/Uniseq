# Unidirectional Cyclic Graph

The graph is authored in one direction.

Written entries point to pages through tags and page links.

```text
journal entry -> tag/page
journal entry -> tag/page
page note     -> tag/page
```

## Why "unidirectional"

The source file owns the link.

The target page file does not store backlinks or copied incoming references.

Incoming references are computed by the index and shown in the frontend as part of the page view.

This makes moves, sync, cache rebuilds, and file ownership simpler.

## Why "cyclic"

Cycles can still exist conceptually.

For example:

```text
Entry A links to #project-x
Project X page note links to #company-goals
Company Goals page note links to #project-x
```

The graph can contain cycles, but cycles are projections from simple directed links. Users do not manage graph topology directly.

## Edge model

Each parsed edge should include:

- source file path
- source entry span
- source date if from a journal
- target page title
- edge kind: tag, page link, task context, asset reference
- surrounding snippet for display

The source span can be line/column or byte-range based. It does not need to be a stable block UUID.

A `span` means the exact source range in the markdown file corresponding to the parsed item shown in the UI.

If the UI offers "jump back to source", page-level navigation is enough for correctness.

Block-level source jumps may use cached spans as a best-effort enhancement.

They are convenience behavior, not part of the data contract.

## Page view behavior

A page view gathers incoming entries for the page title and combines them with the page's own persisted markdown content when present.

Derived sections are rendered by the frontend from indexed data.

They are not written back into the page file just to maintain the page view.

Default grouping:

- journal date
- source page
- task state
- pinned status
- recency
