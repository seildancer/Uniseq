# Editor and Mutation Handlers

The codebase has many handlers because user intent is varied.

## Common mutation categories

- insert and split block
- delete block or subtree
- move block
- toggle collapse state
- paste and transform content
- convert between block and page semantics
- apply commands from shortcut or palette

## Why handlers matter

The handlers translate interaction into structure.

That includes keyboard events, mouse actions, drag and drop, and palette-driven commands.

