# Product Principles

## Journal first

The daily journal is the default place to write.

The app should optimize for capture:

- start typing quickly
- add tags naturally
- link to concepts without choosing a destination folder
- keep chronological context intact
- let later views organize the material

## Pages are views before documents

A page is primarily a view that combines its own markdown content with entries that point to it through tags or links.

The page's own markdown content is optional, but the default page behavior is still aggregation.

This is the central divergence from classic note apps: the user does not need to move content into the "right" page.

## Calendar is a built-in primitive

Calendar is not a plugin-style extra.

Date navigation, journal browsing, and date-scoped filtering should be built into the core product and visible in the main shell.

## Directed authorship

When a user writes `#project-x` in a journal entry, the source entry points to the page titled `project-x`.

That page does not need to mutate. The frontend simply shows incoming entries there.

This keeps markdown files simple and makes the graph a projection rather than a hand-maintained structure.

## No block-level knowledge graph

The app intentionally drops:

- copy block ref
- copy block embed
- block transclusion
- stable IDs for every block
- fine-grained zettelkasten behavior

This makes the data model smaller and makes markdown a stronger source of truth.

## Strong core, flexible UI

Rust owns correctness-sensitive and latency-sensitive work.

TypeScript owns composition, UI features, and plugin extensibility.

## Compatibility tiebreaker

Compatibility with existing Logseq workspaces should be the default tiebreaker.

When multiple designs fit the new app equally well:

- prefer the choice that preserves Logseq-compatible file conventions
- prefer the choice that preserves Logseq-compatible markdown conventions
- prefer the choice that keeps migration and import simpler

Only diverge when the new product model clearly requires it.

Every intentional divergence from Logseq should be explicit in the architecture.

## Hierarchy over feature clutter

Primary navigation should favor a Notion-style page hierarchy and clear document tree over a feature-heavy sidebar.

Whiteboard, flashcards, graph, PDF, and similar extra surfaces should live in secondary navigation such as a top bar, right utility rail, or contextual page actions.

Users should not need to go through an "all pages" dumping ground to understand their workspace structure.

## Derived views are not the primary edit surface

Graph, backlink, query, and similar derived views may navigate, filter, and summarize freely.

When they support editing, the default behavior should route the user back to source markdown context or apply a markdown write through the same canonical write path.

Do not let derived views quietly become a second graph-native editing model.
