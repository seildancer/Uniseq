# Feature Architecture

Features should be layered on top of the journal-first model.

They should not require block-level graph identity unless explicitly justified.

Feature docs should distinguish clearly between:

- persisted markdown content
- derived frontend sections
- separate durable feature files such as whiteboards

Ephemeral focus/zoom behavior belongs to the editor and view layer, not to the durable data model.
