<p align="center">
  <img src="promo/uniseq.svg" alt="Uniseq logo" width="120">
</p>

<h1 align="center">Uniseq</h1>

<p align="center">
  A local-first, Markdown-native note app for fast capture and durable knowledge.
</p>

<p align="center">
  Streams for daily writing. Pages for long-term structure. Plain files as the source of truth.
</p>

## Why Uniseq Exists

Uniseq started from a very ordinary problem: fast capture and durable organization usually live in different apps.

Sticky notes, phone widgets, scratchpads, and daily logs are great for getting thoughts down quickly. Structured tools like Notion are better once those thoughts need hierarchy, reuse, and long-term maintenance. Moving notes from one system into another becomes friction, and friction is exactly what capture workflows cannot afford.

Logseq gets much closer to the right shape with its daily-note workflow and linked thinking, but Uniseq takes a different tradeoff: keep the files simple, stay Markdown-native, and avoid a heavy block-identity model. If most real note-making value comes from linking notes to durable topics rather than manually embedding blocks everywhere, a lighter system can preserve the important parts without turning the note folder into app-owned infrastructure.

That is the core of Uniseq:

- capture quickly on a time axis
- organize gradually on a structure axis
- keep everything in files you still own without the app

## What Uniseq Is

Uniseq combines three ideas:

- Logseq-style daily capture
- Notion-style page hierarchy
- plain Markdown files as the permanent source of truth

It is built for people who want a personal knowledge app that stays close to the underlying folder, supports both streams and structured pages, and treats AI and sync as optional layers around the notes rather than the notes themselves.

## Screenshots

| Streams | Pages |
| --- | --- |
| ![Streams view](promo/streams.png) | ![Pages view](promo/pages.png) |
| AI chat | Sync |
| ![AI chat panel](promo/aichat.png) | ![Sync setup and status](promo/sync.png) |

## Mental Model

### Pages

Pages are for durable knowledge.

They are where notes become intentional, organized, and reusable. Use them for project docs, topic pages, reference material, wiki-like structures, and anything you expect to revisit over time.

### Streams

Streams are for date-based capture.

Use them for journals, work logs, diaries, research notes, project logs, and the kinds of writing that should be easy to start before you know exactly where they belong.

Uniseq ships with `journals/` and `diary/` as the default streams, but additional streams can be created inside the app.

### References

Pages and streams are meant to work together.

Capture something quickly in a stream, link it to a durable page, and let that page collect related mentions over time. In practice, the workflow is:

1. Write quickly in a stream.
2. Reference important topics with `[[Page/Subpage]]` or `#Page/Subpage`.
3. Revisit the linked page when the topic becomes worth organizing.
4. Promote stable ideas into pages and page hierarchies.

This keeps capture lightweight without giving up long-term structure.

## What Uniseq Is Good At

- fast daily note-taking
- building a personal wiki over time
- linking transient writing to durable topics
- keeping notes in plain Markdown files
- browsing by hierarchy, recency, references, and search
- separating different areas of life or work into distinct streams
- syncing a file-first workspace across devices
- using AI to talk with your own notes instead of a detached knowledge base

## Key Capabilities

- Local-first workspace with plain Markdown files
- Pages with hierarchy and linked-reference views
- Multiple date-based streams, including diary-specific privacy blur
- Mixed writing model: bullets when you want them, normal Markdown when you do not
- Search across page titles, page ids, and note content
- AI chat over your notes with saved chats and private in-memory chats
- Optional sync with conflict handling
- Desktop and mobile-oriented Tauri app shell

## Workspace Format

Uniseq is file-first by design. A workspace is just a folder with a few conventional roots:

```text
My Notes/
  pages/
    Projects.md
    Projects___Uniseq.md
    Writing___Ideas.md
  journals/
    2026_06_23.md
  diary/
    2026_06_23.md
  assets/
  uniseq/
```

Important details:

- `pages/` contains flat Markdown files. Hierarchy is encoded in filenames with `___`, so `pages/Projects___Uniseq.md` represents the page `Projects/Uniseq`.
- Streams live as top-level folders such as `journals/`, `diary/`, or any custom stream name you create. Each stream note is a dated Markdown file like `2026_06_23.md`.
- App state such as ordering and sync metadata lives under `uniseq/`.
- Notes stay readable and movable outside the app.

## AI and Sync

AI and sync are support layers, not the core product.

### AI chat

- AI chat works against your local notes.
- You bring your own Gemini API key in the app UI.
- Regular chats are stored locally.
- Private chats stay in memory only and do not enter saved chat history.

### Sync

- Sync is optional.
- The app can connect to the built-in Uniseq provider flow or a custom sync URL.
- Sync is file-by-file, local-first, and conflict-aware.
- The custom backend contract is documented in [SYNC_SERVICE.md](SYNC_SERVICE.md).
- There is also a backend smoke-test checklist in [SYNC_SERVICE_SMOKE_TEST.md](SYNC_SERVICE_SMOKE_TEST.md).

## Current Status

Uniseq is still early and personal-use-driven.

- The current tested path is Windows and Android.
- The codebase includes Tauri desktop/mobile targets, but macOS and iOS are not verified in practice here.
- The app is designed for single-user personal knowledge work, not real-time collaboration.
- The file model and sync contract are intentional parts of the project, not temporary implementation details.

## Build From Source

### Prerequisites

- Rust toolchain
- Node.js and npm
- Tauri 2 system prerequisites for your platform

### Run in development

```bash
npm --prefix web install
cargo tauri dev
```

Tauri uses the web app from `web/` and starts Vite automatically on port `1420`.

### Tests

```bash
cargo test
npm --prefix web test
```

### Production build

```bash
cargo tauri build
```

If you only want to build the web bundle:

```bash
npm --prefix web run build
```

If you want to use the built-in Uniseq account flow during development, the web app reads these optional environment variables from `web/.env`:

```env
VITE_SYNC_ROOT_PREFIX=
VITE_SUPABASE_URL=
VITE_SUPABASE_PUBLISHABLE_KEY=
```

For local-only use, or for custom sync backends where you paste a bearer token manually, those variables are not required.

## Contributing

Issues and pull requests are welcome, especially around:

- Windows and Android workflows
- mobile polish
- macOS and iOS validation
- sync backend compatibility
- Markdown and file-model edge cases
- improving the streams-to-pages workflow without weakening file ownership

If you are proposing a larger change, the project bar is simple: preserve the file-first model, keep behavior legible, and make capture or retrieval meaningfully better.

## Non-Goals

Uniseq is not trying to be:

- a database-first workspace
- a cloud-first collaboration product
- a complex block-identity system
- a full Notion replacement
- an automation-heavy productivity platform

## License

This repository does not currently include a license file.
