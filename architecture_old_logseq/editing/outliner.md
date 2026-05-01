# Outliner Pipeline

The app edits structured block trees.

## Responsibilities

- represent blocks as tree nodes
- preserve ordering
- move subtrees
- normalize parent and sibling relationships
- convert block edits into transactions
- write page-level metadata when structural blocks change

## Editing model

Input is not applied directly to files.

The app:

1. updates the runtime model
2. records structured changes
3. writes back to the page/file layer when appropriate

This is why the outliner sits between the editor and persistence layers.

