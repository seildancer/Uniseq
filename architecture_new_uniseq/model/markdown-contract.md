# Markdown Contract

Markdown is the source of truth.

The app should avoid hidden syntax unless it is clearly necessary.

## Supported syntax

- headings
- paragraphs
- bullet and ordered lists
- tasks: `- [ ]`, `- [x]`
- tags: `#tag`
- page links: `[[Page Name]]`
- markdown links and images
- code blocks
- tables where supported by the editor
- front matter for optional page metadata

## Page metadata

Page metadata should live in front matter when needed.

Example:

```markdown
---
title: Project X
aliases:
  - px
view:
  sort: newest
  group_by: day
---
```

## Entry metadata

Avoid per-entry metadata by default.

If needed later, use visible markdown conventions rather than mandatory hidden IDs.

Unidirectional block -> page refs do not require stable IDs on normal blocks when page-level source navigation is sufficient.

## Parsing rule

The parser must preserve user-authored markdown as much as possible.

Formatting should not churn files unless the user explicitly formats or restructures content.
