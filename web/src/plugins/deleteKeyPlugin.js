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
