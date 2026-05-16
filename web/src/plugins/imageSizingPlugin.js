import { $prose } from "@milkdown/utils";
import { Plugin } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

function buildImageDecorations(doc) {
  const decorations = [];
  doc.descendants((node, pos) => {
    if (node.type.name !== "image") return;
    const title = node.attrs.title;
    if (!title?.startsWith("uniseq-image|")) return;

    const parts = title.split("|");
    const parsedHeight = Number.parseInt(parts[1], 10);
    const parsedWidth = Number.parseInt(parts[2], 10);
    if (
      !Number.isFinite(parsedWidth) || !Number.isFinite(parsedHeight) ||
      parsedWidth <= 0 || parsedHeight <= 0
    ) return;

    decorations.push(
      Decoration.node(pos, pos + node.nodeSize, {
        style: `width: ${parsedWidth}px; height: ${parsedHeight}px;`,
        title: "",
      })
    );
  });
  return DecorationSet.create(doc, decorations);
}

export default $prose(() =>
  new Plugin({
    state: {
      init(_, state) {
        return buildImageDecorations(state.doc);
      },
      apply(tr, old) {
        if (!tr.docChanged) return old;
        return buildImageDecorations(tr.doc);
      },
    },
    props: {
      decorations(state) {
        return this.getState(state);
      },
    },
  })
);
