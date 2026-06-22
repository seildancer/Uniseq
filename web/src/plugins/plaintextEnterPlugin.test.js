import test from "node:test";
import assert from "node:assert/strict";
import { Schema } from "prosemirror-model";

import { parsePlainTextPaste } from "./plaintextEnterPlugin.js";

const schema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: {
      content: "inline*",
      group: "block",
      parseDOM: [{ tag: "p" }],
      toDOM: () => ["p", 0],
    },
    text: { group: "inline" },
    hardbreak: {
      inline: true,
      group: "inline",
      selectable: false,
      parseDOM: [{ tag: "br" }],
      toDOM: () => ["br"],
    },
  },
});

function paragraphContext() {
  const doc = schema.nodes.doc.create(null, schema.nodes.paragraph.create());
  return doc.resolve(1);
}

function parse(text) {
  return parsePlainTextPaste(
    text,
    paragraphContext(),
    true,
    { state: { schema } },
  );
}

function parseWithMarkdown(text) {
  return parsePlainTextPaste(
    text,
    paragraphContext(),
    true,
    { state: { schema } },
    { parseMarkdown: (markdown) => ({ markdown }) },
  );
}

test("parsePlainTextPaste turns single newlines into hardbreaks", () => {
  assert.deepEqual(
    parse("foo\nbar\n")?.toJSON(),
    {
      content: [
        { type: "text", text: "foo" },
        { type: "hardbreak" },
        { type: "text", text: "bar" },
      ],
    },
  );
});

test("parsePlainTextPaste keeps blank lines as paragraph boundaries", () => {
  assert.deepEqual(
    parse("foo\nbar\n\nfoobar")?.toJSON(),
    {
      content: [
        {
          type: "paragraph",
          content: [
            { type: "text", text: "foo" },
            { type: "hardbreak" },
            { type: "text", text: "bar" },
          ],
        },
        {
          type: "paragraph",
          content: [{ type: "text", text: "foobar" }],
        },
      ],
    },
  );
});

test("parsePlainTextPaste lets markdown block paste use Milkdown defaults", () => {
  assert.equal(parse("- foo\n- bar"), null);
});

test("parsePlainTextPaste normalizes mixed markdown and plaintext through one path", () => {
  assert.deepEqual(
    parseWithMarkdown("# asdf\nasdf\naasdf\nasdf\n\n# fdsa\nfdsa\nfdsa\n"),
    {
      markdown: "# asdf\nasdf\\\naasdf\\\nasdf\n\n# fdsa\nfdsa\\\nfdsa",
    },
  );
});

test("parsePlainTextPaste preserves structural markdown when parser is available", () => {
  assert.deepEqual(
    parseWithMarkdown("- foo\n- bar"),
    { markdown: "- foo\n- bar" },
  );
});
