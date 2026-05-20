import test from "node:test";
import assert from "node:assert/strict";
import { Schema } from "prosemirror-model";
import { EditorState, TextSelection } from "prosemirror-state";

import { isSelectionInsideCode } from "./autocompleteCodeGuards.js";

const schema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: {
      content: "inline*",
      group: "block",
      toDOM: () => ["p", 0],
    },
    code_block: {
      content: "text*",
      group: "block",
      code: true,
      toDOM: () => ["pre", ["code", 0]],
    },
    text: { group: "inline" },
  },
  marks: {
    inlineCode: {
      code: true,
      toDOM: () => ["code", 0],
    },
  },
});

function stateWithSelection(doc, anchor) {
  return EditorState.create({
    schema,
    doc,
    selection: TextSelection.create(doc, anchor),
  });
}

test("isSelectionInsideCode detects inline code marks", () => {
  const doc = schema.node("doc", null, [
    schema.node("paragraph", null, [
      schema.text("plain "),
      schema.text("#Tag", [schema.mark("inlineCode")]),
    ]),
  ]);

  const state = stateWithSelection(doc, 8);
  assert.equal(isSelectionInsideCode(state), true);
});

test("isSelectionInsideCode detects code block parents", () => {
  const doc = schema.node("doc", null, [
    schema.node("code_block", null, [schema.text("#Tag")]),
  ]);

  const state = stateWithSelection(doc, 2);
  assert.equal(isSelectionInsideCode(state), true);
});

test("isSelectionInsideCode stays false in normal text", () => {
  const doc = schema.node("doc", null, [
    schema.node("paragraph", null, [schema.text("#Tag")]),
  ]);

  const state = stateWithSelection(doc, 2);
  assert.equal(isSelectionInsideCode(state), false);
});
