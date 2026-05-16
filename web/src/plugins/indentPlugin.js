import { keymap } from "prosemirror-keymap";
import { sinkListItem, liftListItem } from "prosemirror-schema-list";

export default function createIndentPlugin() {
  return keymap({
    Tab: (state, dispatch, view) => {
      const listItemType = state.schema.nodes.list_item;
      if (!listItemType) return false;
      return sinkListItem(listItemType)(state, dispatch, view);
    },
    "Shift-Tab": (state, dispatch, view) => {
      const listItemType = state.schema.nodes.list_item;
      if (!listItemType) return false;
      return liftListItem(listItemType)(state, dispatch, view);
    },
  });
}
