# Phase 02 — Journal/Page Parsing and Page Identity

## Goal

Turn markdown files into the core domain graph inputs: entries, links, tags, aliases, tasks, and normalized page identities.

## Architecture Sources

- `architecture_new_uniseq/AGENT_HANDOFF.md`
- `architecture_new_uniseq/model/entities.md`
- `architecture_new_uniseq/model/page-identity.md`
- `architecture_new_uniseq/model/graph.md`
- `architecture_new_uniseq/features/journal.md`
- `architecture_new_uniseq/features/pages.md`

## Scope

- Parse journals and page-owned markdown content into entries with source anchors.
- Resolve `#tag` and `[[Page]]` references to the same page identity when spellings match.
- Support namespace-style page naming without treating it as a filesystem hierarchy.
- Extract task markers and task state from markdown.
- Extract aliases/front matter needed for compatibility.
- Represent pages that exist only as tag/link targets without requiring page files.

## Deliverables

- Parser and resolver modules in Rust.
- Source-anchor format based on file path, span, and optional snippet/hash.
- Page identity normalization tests.
- Golden fixtures for journals, page files, nested namespaces, aliases, and mixed link/tag references.

## Acceptance Criteria

- A journal entry containing `#Project` and `[[Project]]` resolves both to one page identity.
- A referenced page can appear in page lists and page views even before a page file exists.
- Entries do not receive hidden durable UUIDs by default.
- Source anchors are sufficient to navigate to and patch the original markdown span.

## Risks

- Identity normalization mistakes will cascade into backlinks, search, sync conflicts, and renames.
- Span anchors can drift if writes are not carefully validated later.

## Exit Gate

Given a workspace, the engine can parse all journals/pages and return normalized entries, pages, references, tasks, and source anchors without mutating source files.
