# Formatting and Text Transform

The app includes a formatting layer for markdown and block content.

## Responsibilities

- markdown serialization
- block content normalization
- property and metadata extraction
- text transforms for commands
- export formats

## Design choice

Formatting is not the same as storage.

Storage owns durable representation. Formatting owns the conversion between runtime structure and textual output.

