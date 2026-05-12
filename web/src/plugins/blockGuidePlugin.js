import { $prose } from "@milkdown/utils";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

const blockGuideKey = new PluginKey("blockGuide");

const PLAINTEXT_TYPES = new Set(["paragraph", "heading"]);

function findAllPlaintextBlocks(doc) {
  const blocks = [];
  let currentGroup = [];
  let pos = 1;
  const content = doc.content;

  for (let i = 0; i < content.childCount; i++) {
    const child = content.child(i);
    const isPlaintext = PLAINTEXT_TYPES.has(child.type.name);

    if (isPlaintext) {
      currentGroup.push({ pos, node: child });
    } else if (currentGroup.length > 0) {
      blocks.push(currentGroup);
      currentGroup = [];
    }

    pos += child.nodeSize;
  }

  if (currentGroup.length > 0) {
    blocks.push(currentGroup);
  }

  return blocks;
}

export default $prose(() =>
  new Plugin({
    key: blockGuideKey,
    props: {
      decorations(state) {
        const { doc } = state;
        const decos = [];

        for (const group of findAllPlaintextBlocks(doc)) {
          for (const block of group) {
            decos.push(
              Decoration.node(
                block.pos,
                block.pos + block.node.nodeSize,
                { class: "block-guide-plaintext" }
              )
            );
          }
        }

        return DecorationSet.create(doc, decos);
      },
    },
  })
);
