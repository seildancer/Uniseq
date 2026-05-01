# Editor Integration

The editor can use existing JS/React components.

The editor is not the source of truth. It is the interaction surface for markdown.

## Requirements

- fast markdown editing
- task checkbox toggles
- tag and page suggestions
- autocomplete for page titles
- inline links and asset embeds
- keyboard shortcuts
- mobile-friendly text input later if needed

## Data flow

1. React loads markdown for a journal or page.
2. The editor may build a temporary structural model for interaction and selection.
3. User edits through the editor surface.
4. UI sends patches or save commands to Rust.
5. Rust validates and writes canonical markdown.
6. Rust emits index/view invalidation events.
7. React refreshes affected views.

The editor may be structurally aware in memory, but it must not become a separate durable document model.

## Editor library rule

Choose editor components for UX and extension quality, not because they can own the document model.
