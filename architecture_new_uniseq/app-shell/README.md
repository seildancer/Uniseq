# App Shell

The app shell is React + Vite running inside Tauri.

The TypeScript side exists to compose UI, editor components, plugin APIs, and UI-heavy feature modules.

It should not become the durable data engine.

The shell should prioritize:

- built-in calendar access
- clear journal and page navigation
- a Notion-style hierarchical page tree
- secondary placement for non-core feature surfaces such as whiteboard and flashcards

The shell is also responsible for rendering derived sections on pages without making them feel like separate stored documents.
