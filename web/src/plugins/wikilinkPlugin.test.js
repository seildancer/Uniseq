import test from "node:test";
import assert from "node:assert/strict";
import { Schema } from "prosemirror-model";
import { EditorState } from "prosemirror-state";

import createWikilinkPlugin from "./wikilinkPlugin.js";

const schema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: {
      content: "inline*",
      group: "block",
      toDOM: () => ["p", 0],
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

function decorationClassesForDoc(doc) {
  const plugin = createWikilinkPlugin({ current: null }, { current: [] });
  const state = EditorState.create({ schema, doc, plugins: [plugin] });
  return plugin.props.decorations(state).find().map((decoration) => decoration.type.attrs.class);
}

test("wikilink plugin decorates plain refs", () => {
  const doc = schema.node("doc", null, [
    schema.node("paragraph", null, [schema.text("See [[Page]] and #Tag")]),
  ]);

  assert.deepEqual(
    decorationClassesForDoc(doc).sort(),
    [
      "tag-link tag-link-hash",
      "tag-link tag-link-wiki",
      "wikilink-bracket",
      "wikilink-bracket",
    ],
  );
});

test("wikilink plugin ignores refs inside inline code", () => {
  const doc = schema.node("doc", null, [
    schema.node("paragraph", null, [
      schema.text("#Tag [[Page]]", [schema.mark("inlineCode")]),
    ]),
  ]);

  assert.deepEqual(decorationClassesForDoc(doc), []);
});

test("wikilink plugin resolves hierarchical hashtag refs to existing pages", () => {
  let navigatedTo = null;
  const plugin = createWikilinkPlugin(
    { current: (pageId) => { navigatedTo = pageId; } },
    { current: [{ page_id: "pages:foo/bar", title: "bar" }] },
  );
  const tagEl = {
    textContent: "#foo/bar",
    classList: {
      contains: (name) => name === "tag-link-hash",
    },
  };
  const event = {
    target: {
      closest: (selector) => (selector === ".tag-link" ? tagEl : null),
    },
  };

  const handled = plugin.props.handleClick(null, 0, event);

  assert.equal(handled, false);
  assert.equal(navigatedTo, "pages:foo/bar");
});
