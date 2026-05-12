import { $prose } from "@milkdown/utils";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

const blockHighlightKey = new PluginKey("blockHighlight");

let hasFocus = false;

export function resetBlockHighlightFocus() {
  hasFocus = false;
}

export default $prose(() =>
  new Plugin({
    key: blockHighlightKey,
    props: {
      decorations(state) {
        if (!hasFocus) return DecorationSet.empty;
        const { selection } = state;
        const $from = selection.$from;
        for (let depth = $from.depth; depth > 0; depth--) {
          const node = $from.node(depth);
          const name = node.type.name;
          if (name === "list_item" || name === "paragraph" || name === "heading") {
            const pos = $from.before(depth);
            return DecorationSet.create(state.doc, [
              Decoration.node(pos, pos + node.nodeSize, {
                class: "milkdown-block--active",
                "data-block-active": "true",
              }),
            ]);
          }
        }
        return DecorationSet.empty;
      },
      handleDOMEvents: {
        focus: () => {
          hasFocus = true;
          return false;
        },
        blur: () => {
          hasFocus = false;
          return false;
        },
      },
    },
  })
);
