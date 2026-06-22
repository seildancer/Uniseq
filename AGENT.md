## Uniseq

Uniseq is a local-first personal knowledge app for people who want plain files, fast capture, and durable structure without giving up a focused desktop writing experience.

Its center of gravity is simple:

- Your notes are files.
- The app should help you think, not trap you in app-only data.
- Organization should feel lightweight while still supporting long-term knowledge building.

## Product Thesis

Uniseq treats Markdown files as the product, not as an export format.

The app exists to make a folder of notes feel alive:

- daily writing should be frictionless
- evergreen notes should stay easy to navigate and refactor
- links between notes should accumulate naturally over time
- sync, AI, and interface state should support the notes, not become the notes

## Core Mental Model

There are two complementary modes of writing:

- **Pages** for durable, organized knowledge
- **Streams** for date-based capture such as journals, logs, or diaries

Pages are where information becomes intentional and reusable. Streams are where information starts life quickly, with less ceremony.

The app should make it easy to move between the two modes:

- capture quickly in a stream
- promote ideas into pages when they become stable
- connect pages through references so knowledge becomes navigable

## Main Values

- **File-first.** User content should remain understandable and useful outside the app.
- **Local-first.** The app should feel reliable even without network services.
- **Low magic.** Behavior should be legible and predictable. Hidden complexity is a cost.
- **Fast capture, calm structure.** Writing something new should be easy; organizing it later should also be easy.
- **Durable knowledge over novelty.** Features should strengthen long-term note ownership and retrieval.
- **Optional augmentation.** AI and sync can be important, but they are supporting layers around the notes rather than the canonical source of truth.

## What The App Is Trying To Be Good At

- Daily note-taking with minimal friction
- Building a personal wiki over time
- Linking transient writing to durable notes
- Restructuring note hierarchies without fear
- Browsing and revisiting ideas through references, search, and context
- Keeping the user close to their actual files

## UX Priorities

- Opening the app should feel immediate and grounded in a real workspace.
- Writing should feel lightweight, especially for short notes and incremental edits.
- Navigation should make both hierarchy and recency useful.
- References should help notes discover each other without requiring heavy manual bookkeeping.
- Stream workflows should support routine capture without turning into clutter.
- Destructive actions should be explicit and unsurprising.

## Role Of AI And Sync

AI and sync matter, but they are not the core identity of Uniseq.

- **AI** is for contextual assistance around the user's notes.
- **Sync** is for moving a file-first workspace across devices or locations.

Neither should redefine the note model. The notes remain primary.

## Non-Goals

Uniseq is not trying to be:

- a database-first workspace where notes are trapped behind app schemas
- a highly abstract block system with complex hidden identities
- a general-purpose document editor with every Markdown feature as a first-class concern
- a cloud-first collaboration product
- an app where automation matters more than clarity and ownership

## Guidance For Future Changes

When evaluating new features or refactors, preserve these questions:

- Does this strengthen the file-first model or erode it?
- Does this make capture or retrieval meaningfully better?
- Does this keep the user's mental model simple?
- Does this reduce friction without hiding important behavior?
- Does this help pages and streams complement each other?
- Does this treat AI and sync as support systems rather than the product core?

If a change improves implementation elegance but weakens user ownership, legibility, or note durability, it is probably the wrong trade.
