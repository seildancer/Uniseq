import { keymap } from "prosemirror-keymap";
import { TextSelection } from "prosemirror-state";
import { joinBackward } from "prosemirror-commands";
import { liftListItem } from "prosemirror-schema-list";

export default function createBackspacePlugin() {
  return keymap({
    Backspace(state, dispatch, view) {
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

      // Only intercept for empty blocks
      if ($from.parent.content.size !== 0) return false;

      // Try to unindent (lift) first
      if (liftListItem(state.schema.nodes.list_item)(state, dispatch, view)) {
        return true;
      }

      // If lift fails (e.g., top-level item), fall back to joinBackward
      return joinBackward(state, dispatch, view);
    },
  });
}
