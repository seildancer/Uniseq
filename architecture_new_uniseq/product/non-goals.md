# Non-Goals

These are intentional exclusions.

## Not a Logseq clone

The app does not need to reproduce every Logseq behavior.

It should preserve the best user outcome: natural capture leading to useful linked views.

## Not a block graph

The app does not expose every block as an addressable graph object.

Blocks are markdown structure. They are useful for editing and display, but they are not the core knowledge unit.

## Not collaborative editing

The app does not target real-time collaborative editing.

Cross-device sync is single-user sync. Conflict handling should be robust, but it does not need multiplayer semantics.

## Not server-owned content

Paid sync may exist, but the server should not become the source of truth.

Local markdown files remain canonical.

## Not dependent on SSR

The app is offline-first and local-first. Next.js-style server rendering is not central to the product.
