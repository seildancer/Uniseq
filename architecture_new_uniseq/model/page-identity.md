# Page Identity

Pages are identified by their normalized page path.

When namespacing is used, the identity is `namespace/title`.

There are not separate classes such as "tag page", "link page", or "authored page".

Those are only different ways a page may first be referenced or receive its own markdown content.

## Page title rules

- page path is the primary page identity
- namespaced titles inherit identity from the full path-like title
- tags and links should resolve to the same page when they spell the same title
- aliases are alternate resolution names, not separate pages
- rename or move changes page identity and must update references accordingly

## Path model

The `/` character is a namespace separator in page names.

It is not a real filesystem folder boundary.

Examples:

- `project-x`
- `people/alice`
- `areas/work/client-a`

The UI may present these as a hierarchy, but storage remains page-based rather than folder-based.

## Storage mapping

The app stores page markdown files in a flat pages directory.

The file path is derived from the page path by escaping namespace separators.

Recommended rule:

- page path `people/alice` -> file `pages/people___alice.md`
- page path `areas/work/client-a` -> file `pages/areas___work___client-a.md`

The exact escape token should be stable. `___` is acceptable and aligns with current Logseq practice.

## Hierarchy

Namespace is part of the page name.

It is not a real folder structure.

If the product supports a Notion-style page tree, that tree should be represented as page metadata or a parent relation between pages.

The page tree is a UI/product hierarchy, not a filesystem hierarchy.

## Page content versus page view

A page file stores:

- the page's own markdown content
- page metadata such as aliases or view settings

A page view may also show:

- incoming references
- task rollups
- related pages
- assets and annotations

These derived sections are rendered from indexed data and are not written back into the page file.

## Aliases

Aliases should resolve to the same page identity.

They are lookup helpers and display helpers, not duplicate pages.

## Namespace

Namespace is a page naming feature, not a different storage system.

Namespace trees and explicit parent/child hierarchy may coexist, but they are different concepts.
