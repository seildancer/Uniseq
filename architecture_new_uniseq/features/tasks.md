# Tasks

Tasks are extracted from markdown checkboxes.

## Task model

- source file
- source span
- text
- checked state
- linked tags/pages
- date context
- optional due/scheduled metadata later

Tasks remain markdown-owned content.

The canonical syntax is markdown checkbox syntax such as `- [ ]` and `- [x]`.

Task toggles from derived views should modify the source markdown through `file_path + span` anchoring rather than through a separate task store.

## Views

- today
- open tasks
- completed tasks
- tasks by page/tag
- timeline
