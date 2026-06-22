# Uniseq

Uniseq is a local-first, Markdown-native personal knowledge app for people who want fast capture, plain files, and durable structure. It combines Logseq-style daily capture with Notion-style page hierarchy.

## Features

* Markdown-native
* Local-first
* Daily note stream flows into page hierarchy via tags
* Multiple stream support
* Private diary blur
* Block-To-Page references, aggregating linked blocks under a page
* Outliner + plain Markdown
* Basic search
* AI chat with your notes
* Sync support

## Why Uniseq Exists

Uniseq started from a simple personal problem.

I used Notion as my main note app, but I also kept a sticky note widget on my phone because it was the fastest place to capture thoughts. Over time, more and more notes piled up in the widget, and I had to periodically move them into Notion. That extra step became friction.

I later tried Logseq and liked its daily note workflow, especially how small pieces of writing could accumulate naturally under linked topics. But not long after, logseq announced diverging into a DB version that handles reference system better, in the cost of losing markdown nativeness. While trying to understand why the migration was necessary I realized that if I give up block-to-block references and just keep block-to-page references, I can keep things simple and lightweight while not losing much functionality, as not many people use manual block embeddings.

## Product Thesis

Notion is strong at structured pages and hierarchy, but the data lives inside the app. Logseq is strong at daily capture and linked thinking, but hierarchy and long-term organization can feel weaker.

Uniseq tries to bring both strengths together:

* **Streams** for fast, date-based capture
* **Pages** for durable, organized knowledge
* **References** for connecting transient notes to stable topics
* **Markdown files** as the permanent source of truth

## Mental Model

Uniseq has two complementary modes of writing.

### Pages

Pages are for durable knowledge.

They are where ideas become more intentional, organized, and reusable. Pages can be arranged in a hierarchy, making them useful for building a personal wiki, project documentation, structured notes, or long-term knowledge collections.

### Streams

Streams are for fast, date-based capture.

They are useful for journals, logs, diaries, daily notes, work notes, research logs, project logs, and routine tracking.

Instead of forcing every thought into the right folder immediately, Uniseq lets users capture quickly first and organize later.

### From Streams to Pages

The main workflow is simple:

1. Capture thoughts quickly in a stream.
2. Link important ideas to pages.
3. Let references accumulate naturally.
4. Promote stable ideas into durable pages over time.

This keeps capture lightweight while still allowing long-term structure to emerge.

## Key Design Decisions

### Markdown-native by default

Uniseq keeps notes as plain Markdown files.

The app should enhance the files, not trap them. Even without Uniseq, the user should still be able to open, read, edit, back up, and move their notes.

### Local-first

The workspace should feel reliable without depending on the network.

Sync can exist, but the primary experience should remain grounded in local files.

### Block-to-page references

Uniseq avoids a heavy block-to-block reference model.

Instead, references are centered around pages. This keeps the structure easier to understand, easier to implement, and closer to the main use case: letting daily notes accumulate around meaningful topics.

### Time axis and space axis

Uniseq combines two ways of organizing knowledge:

* the **time axis**, through streams and daily writing
* the **space axis**, through page hierarchy and durable structure

Streams are good for capturing life as it happens. Pages are good for organizing what becomes important.

### Multiple streams

Uniseq supports multiple streams instead of a single daily note.

This makes it possible to separate different kinds of date-based writing, such as:

* journal
* diary
* work log
* research log
* project log
* habit log

Private diary entries can also be visually blurred, so the app can stay open without exposing sensitive content.

### Outliner and plain text

Uniseq supports outliner-style thinking, but does not force everything into an outliner model.

Users can write structured bullets when useful, or plain Markdown text when that feels better.

## What Uniseq Is Good At

Uniseq is designed for:

* fast daily note-taking
* building a personal wiki over time
* linking daily thoughts to long-term topics
* maintaining a Markdown-based knowledge folder
* navigating notes through hierarchy, recency, references, and search
* separating different streams of life or work
* restructuring notes without fear of losing ownership
* using AI to discuss and explore personal notes
* syncing a file-first workspace across devices

## AI and Sync

AI and sync are supporting layers, not the core identity of Uniseq.

AI helps users think with their own notes. It can support summarization, question-answering, exploration, and contextual assistance, but it should not replace the underlying note model.

Sync makes a local-first Markdown workspace available across devices, but it should not turn Uniseq into a cloud-first product.

The notes remain primary.

## Values

* **File-first** — notes should remain understandable outside the app.
* **Local-first** — the workspace should feel reliable without network services.
* **Low magic** — behavior should be predictable and legible.
* **Fast capture, calm structure** — writing should be easy; organizing later should also be easy.
* **Durable knowledge over novelty** — features should strengthen long-term ownership and retrieval.
* **Optional augmentation** — AI and sync should support the notes, not redefine them.

## Non-goals

Uniseq is not trying to be:

* a database-first workspace
* a complex block-based system with hidden identities
* a cloud-first collaboration product
* a full Notion replacement
* a general-purpose document editor
* an automation-heavy productivity platform
* an app where the interface becomes more important than the files

## Positioning

Uniseq is for people who like the idea of Logseq, Obsidian, and Notion, but want a simpler Markdown-native app that combines:

* Logseq-like daily capture
* Notion-like hierarchy
* plain-file ownership
* multiple journal streams
* lightweight references
* optional AI and sync

It is especially suited for people who want to build long-term personal knowledge without giving up control of their files.

## Short Product Description

Uniseq is a local-first, Markdown-native note app that combines Logseq-style daily capture with Notion-style page hierarchy. Capture thoughts in streams, organize durable knowledge in pages, and keep everything as plain files you own.

## Tagline Options

* A Markdown-native note app for fast capture and durable knowledge.
* Local-first notes with streams, pages, references, AI, and sync.
* Logseq-style daily notes meets Notion-style hierarchy — in plain Markdown.
* A file-first personal knowledge app for people who want to own their notes.
* Capture in streams. Organize in pages. Keep your notes as Markdown.

## Developer's notes
* Honestly, this project was made with my own use in mind
* Hence I only tested it on windows and android. I didn't test mac, and I don't have ios mobile app as my apple dev plan expired. But if someone asks the codebase is capable of the platforms too.