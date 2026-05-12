# Fix: Glossy Gradient Block End Marker

## Problem
The glossy gradient block end marker (`milkdown-block--active`) is being placed on the **first ancestor block** containing the cursor, rather than the **last leaf** of that block.

## Root Cause
In `web/src/plugins/blockHighlightPlugin.js`, lines 21-23 walk UP the document tree from the cursor position and stop at the **first** matching block type (`list_item`, `paragraph`, or `heading`). This finds the outermost container, not the end of the block.

## Solution
Replace the simple ancestor-finding logic with a two-phase approach:

1. **Find the root block** containing the cursor (same as current)
2. **Traverse DOWN** to find the actual end:
   - For `list_item`: Recursively find the last child `list_item` by traversing through `bullet_list`/`ordered_list` children until reaching a leaf
   - For `paragraph`/`heading`: Find the last contiguous plaintext sibling at the same depth level

## Implementation

### File: `web/src/plugins/blockHighlightPlugin.js`

**New functions to add:**

```javascript
function findLastLeafOfBlock(state, $from) {
    for (let depth = $from.depth; depth > 0; depth--) {
        const node = $from.node(depth);
        const name = node.type.name;
        if (name === "list_item" || name === "paragraph" || name === "heading") {
            if (name === "list_item") {
                return findLastLeafListItem(state, $from, depth);
            } else {
                return findLastPlaintextSibling(state, $from, depth);
            }
        }
    }
    return null;
}

function findLastLeafListItem(state, $from, startDepth) {
    let currentDepth = startDepth;
    let currentNode = $from.node(currentDepth);

    while (true) {
        let foundChild = false;
        const content = currentNode.content;
        for (let i = content.childCount - 1; i >= 0; i--) {
            const child = content.child(i);
            if (child.type.name === "bullet_list" || child.type.name === "ordered_list") {
                const listContent = child.content;
                if (listContent.childCount > 0) {
                    const lastItem = listContent.child(listContent.childCount - 1);
                    if (lastItem.type.name === "list_item") {
                        currentDepth++;
                        currentNode = lastItem;
                        foundChild = true;
                        break;
                    }
                }
            }
        }
        if (!foundChild) {
            break;
        }
    }

    const pos = $from.start(currentDepth) - 1;
    return { pos, node: currentNode };
}

function findLastPlaintextSibling(state, $from, startDepth) {
    const parent = $from.node(startDepth - 1);
    const startOffset = $from.start(startDepth);

    let lastPlaintextPos = startOffset - 1;
    let lastPlaintextNode = $from.node(startDepth);

    let pos = startOffset;
    const content = parent.content;

    for (let i = 0; i < content.childCount; i++) {
        const child = content.child(i);
        if (pos >= startOffset) {
            const name = child.type.name;
            if (name === "paragraph" || name === "heading") {
                lastPlaintextPos = pos;
                lastPlaintextNode = child;
            } else if (name !== "paragraph" && name !== "heading") {
                if (pos > startOffset) {
                    break;
                }
            }
        }
        pos += child.nodeSize;
    }

    return { pos: lastPlaintextPos, node: lastPlaintextNode };
}
```

**Replace the `decorations` function:**

```javascript
decorations(state) {
    if (!hasFocus) return DecorationSet.empty;
    const { selection } = state;
    const $from = selection.$from;

    const result = findLastLeafOfBlock(state, $from);
    if (!result) return DecorationSet.empty;

    return DecorationSet.create(state.doc, [
        Decoration.node(result.pos, result.pos + result.node.nodeSize, {
            class: "milkdown-block--active",
            "data-block-active": "true",
        }),
    ]);
}
```

## Expected Behavior After Fix

### Outliner block example:
```
- parent
  - child 1
  - child 2  <- cursor here
```
Marker appears on: `child 2` (last leaf)

### Plaintext block example:
```
line 1
line 2  <- cursor here
line 3
```
Marker appears on: `line 3` (last paragraph in contiguous group)
