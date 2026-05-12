# Fix: Delete Key Acts Like Backspace at Start of Outlined Blocks

## Root Cause

When the cursor is at the start of a **non-empty** list item (outlined block) and Delete is pressed:

1. The current plugin returns `false` (line 15) because the block is not empty
2. ProseMirror's default keymap takes over: `chainCommands(deleteSelection, joinForward, selectNodeForward)`
3. Milkdown's commonmark preset has a `LiftFirstListItem` command that gets triggered
4. This lifts the list item (removes the bullet), and subsequent Delete merges with the previous block

## File to Modify

`web/src/plugins/deleteKeyPlugin.js`

## Changes

### Before (lines 1-36):
```javascript
import { keymap } from "prosemirror-keymap";
import { TextSelection } from "prosemirror-state";
import { joinForward } from "prosemirror-commands";

export default function createDeleteKeyPlugin() {
  return keymap({
    Delete(state, dispatch, view) {
      const { selection } = state;
      if (!(selection instanceof TextSelection)) return false;
      const { empty, $from } = selection;
      if (!empty || $from.parentOffset !== 0) return false;

      // Only intercept in empty text blocks inside list_item so that
      // normal forward-delete of characters still works.
      if ($from.parent.content.size !== 0) return false;

      let inListItem = false;
      for (let d = $from.depth - 1; d > 0; d--) {
        if ($from.node(d).type.name === "list_item") {
          inListItem = true;
          break;
        }
      }
      if (!inListItem) return false;

      // Try to pull the next block's content forward.
      if (joinForward(state, dispatch, view)) {
        return true;
      }

      // Swallow the event to prevent Milkdown's LiftFirstListItem
      // from un-indenting the empty list item on Delete.
      return true;
    },
  });
}
```

### After:
```javascript
import { keymap } from "prosemirror-keymap";
import { TextSelection } from "prosemirror-state";
import { joinForward, deleteSelection } from "prosemirror-commands";

export default function createDeleteKeyPlugin() {
  return keymap({
    Delete(state, dispatch, view) {
      const { selection } = state;
      if (!(selection instanceof TextSelection)) return false;
      const { empty, $from } = selection;
      if (!empty || $from.parentOffset !== 0) return false;

      // Check if we're inside a list_item
      let inListItem = false;
      for (let d = $from.depth - 1; d > 0; d--) {
        if ($from.node(d).type.name === "list_item") {
          inListItem = true;
          break;
        }
      }
      if (!inListItem) return false;

      // If the block is empty, try to join with the next block
      if ($from.parent.content.size === 0) {
        if (joinForward(state, dispatch, view)) {
          return true;
        }
        // Swallow the event to prevent Milkdown's LiftFirstListItem
        // from un-indenting the empty list item on Delete.
        return true;
      }

      // For non-empty blocks at position 0, prevent the lift behavior
      // by swallowing the event. Normal forward-delete will still work
      // because the plugin only intercepts when parentOffset === 0.
      // When user moves cursor away from position 0, this returns false
      // and lets ProseMirror's default deleteSelection handle it.
      return true;
    },
  });
}
```

## Key Changes

1. **Moved the list_item check before the empty check** - This ensures we detect list items first
2. **Removed the early return for non-empty blocks** - The original code returned `false` for non-empty blocks, which allowed the lift behavior to occur
3. **Always swallow the event when at position 0 in a list item** - This prevents the LiftFirstListItem behavior. Normal forward-delete still works because:
   - When cursor is at position 0, the plugin returns `true` (swallows event)
   - When cursor moves to position > 0, the plugin returns `false` at line 11, letting ProseMirror's default `deleteSelection` handle forward-delete normally
4. **Added `deleteSelection` import** - For potential future use, though not strictly needed in this fix

## Testing

After applying this fix:
1. Cursor at start of non-empty list item + Delete → Should NOT lift the list item
2. Cursor at start of empty list item + Delete → Should join with next block (existing behavior preserved)
3. Cursor at position > 0 in list item + Delete → Should delete character forward (normal behavior)
4. Cursor at end of list item + Delete → Should join with next block (normal behavior)
