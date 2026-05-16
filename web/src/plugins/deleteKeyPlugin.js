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

      // For non-empty blocks at position 0, delete the first character
      // to prevent the lift behavior while still allowing forward-delete
      if (dispatch) {
        const from = $from.pos;
        const to = from + 1;
        dispatch(state.tr.delete(from, to).scrollIntoView());
      }
      return true;
    },
  });
}
