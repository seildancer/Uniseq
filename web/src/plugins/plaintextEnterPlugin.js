import { keymap } from "prosemirror-keymap";
import { Slice, Fragment } from "prosemirror-model";
import { Plugin, TextSelection } from "prosemirror-state";

const BLOCK_START_RE = /^\s*(?:[-*+] |\d+[.)] |#{1,6}\s|>|```)/;

function hasAncestor($pos, typeName) {
  for (let depth = $pos.depth; depth > 0; depth -= 1) {
    if ($pos.node(depth).type.name === typeName) {
      return true;
    }
  }
  return false;
}

function isPlaintextParagraphSelection(selection) {
  if (!(selection instanceof TextSelection)) return false;
  if (selection.$from.parent.type.name !== "paragraph") return false;
  return !hasAncestor(selection.$from, "list_item");
}

function shouldLetMarkdownHandlePaste(text) {
  return text.split(/\r\n?|\n/).some((line) => BLOCK_START_RE.test(line));
}

function plainTextToHardbreakSlice(text, schema) {
  const hardbreak = schema.nodes.hardbreak;
  if (!hardbreak) return null;

  const nodes = [];
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  lines.forEach((line, index) => {
    if (line) {
      nodes.push(schema.text(line));
    }
    if (index < lines.length - 1) {
      nodes.push(hardbreak.create());
    }
  });

  return new Slice(Fragment.fromArray(nodes), 0, 0);
}

export default function createPlaintextEnterPlugin() {
  const enterKeymap = keymap({
    Enter(state, dispatch) {
      const { selection, schema } = state;
      if (!isPlaintextParagraphSelection(selection)) return false;
      if (!schema.nodes.hardbreak) return false;

      if (dispatch) {
        dispatch(
          state.tr
            .replaceSelectionWith(schema.nodes.hardbreak.create())
            .scrollIntoView(),
        );
      }
      return true;
    },
  });

  const pastePlugin = new Plugin({
    props: {
      handlePaste(view, event) {
        const text = event.clipboardData?.getData("text/plain");
        if (!text || !text.includes("\n")) return false;
        if (!isPlaintextParagraphSelection(view.state.selection)) return false;
        if (shouldLetMarkdownHandlePaste(text)) return false;

        const slice = plainTextToHardbreakSlice(text, view.state.schema);
        if (!slice) return false;

        view.dispatch(view.state.tr.replaceSelection(slice).scrollIntoView());
        return true;
      },
    },
  });

  return [pastePlugin, enterKeymap];
}
