# Graph Visualization

Graph drawing can be implemented in TypeScript/JavaScript.

The Rust engine supplies graph data.

## Nodes

- pages
- tags
- journals as optional date nodes
- assets if useful

## Edges

Edges are directed from source context to target page.

The graph UI may visually collapse many journal entries into weighted edges.

## Design rule

The graph is a discovery view, not the main editing model.

It should tolerate cache lag and lazy refresh.

Opening graph view may trigger additional derived computation without changing the markdown data model.
