import { $prose } from "@milkdown/utils";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

const blockHighlightKey = new PluginKey("blockHighlight");
const PLAINTEXT_TYPES = new Set(["paragraph", "heading"]);

let hasFocus = false;

export function resetBlockHighlightFocus() {
  hasFocus = false;
}

// Starting from the focused block node, walk forward in document order until
// a block-type change or depth decrease, and return the last valid end node.
//
// Outliner (list_item): last list_item in document order within the subtree
// = the last leaf descendant, matching the backend's block_span.
//
// Plaintext (paragraph/heading): last consecutive sibling of the same kind
// at the same depth, matching the backend's plaintext block rule.
function findBlockEnd(doc, blockNode, blockPos, blockDepth) {
  if (blockNode.type.name === "list_item") {
    let lastPos = blockPos;
    let lastNode = blockNode;
    doc.nodesBetween(blockPos, blockPos + blockNode.nodeSize, (node, pos) => {
      if (node.type.name === "list_item" && pos > blockPos) {
        lastPos = pos;
        lastNode = node;
      }
    });
    return { node: lastNode, pos: lastPos };
  }

  // Plaintext: scan forward while sibling nodes are also plaintext.
  // Between same-level siblings the resolved depth equals blockDepth - 1.
  let lastPos = blockPos;
  let lastNode = blockNode;
  let scanPos = blockPos + blockNode.nodeSize;
  const siblingDepth = blockDepth - 1;

  while (scanPos < doc.nodeSize - 1) {
    const $scan = doc.resolve(scanPos);
    if ($scan.depth !== siblingDepth) break;
    const next = $scan.nodeAfter;
    if (!next || !PLAINTEXT_TYPES.has(next.type.name)) break;
    lastPos = scanPos;
    lastNode = next;
    scanPos += next.nodeSize;
  }

  return { node: lastNode, pos: lastPos };
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

          if (name === "list_item") {
            const pos = $from.before(depth);
            const { node: targetNode, pos: targetPos } = findBlockEnd(state.doc, node, pos, depth);
            return DecorationSet.create(state.doc, [
              Decoration.node(targetPos, targetPos + targetNode.nodeSize, {
                class: "milkdown-block--active",
                "data-block-active": "true",
              }),
            ]);
          }

          if (PLAINTEXT_TYPES.has(name)) {
            // Skip paragraphs directly inside a list_item — the list_item above will handle it.
            if (depth > 1 && $from.node(depth - 1).type.name === "list_item") continue;
            const pos = $from.before(depth);
            const { node: targetNode, pos: targetPos } = findBlockEnd(state.doc, node, pos, depth);
            return DecorationSet.create(state.doc, [
              Decoration.node(targetPos, targetPos + targetNode.nodeSize, {
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
