import { Plugin } from "prosemirror-state";

export default function createTaskListClickPlugin() {
  return new Plugin({
    props: {
      handleClick(view, pos, event) {
        const target = event.target;
        if (!(target instanceof HTMLElement)) return false;

        const li = target.closest("li[data-item-type='task']");
        if (!li) return false;

        const { state, dispatch } = view;
        const $pos = state.doc.resolve(pos);
        let depth = $pos.depth;
        while (depth > 0) {
          const node = $pos.node(depth);
          if (node.type.name === "list_item" && node.attrs.checked != null) {
            const nodeStart = $pos.before(depth);
            const newChecked = !node.attrs.checked;
            const tr = state.tr.setNodeMarkup(nodeStart, null, {
              ...node.attrs,
              checked: newChecked,
            });
            dispatch(tr);
            return true;
          }
          depth--;
        }
        return false;
      },
    },
  });
}
