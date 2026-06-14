import test from "node:test";
import assert from "node:assert/strict";

import { toStoredLineBreakMarkdown } from "./editorLineBreaks.js";

test("toStoredLineBreakMarkdown stores markdown hardbreak escapes as plain newlines", () => {
  assert.equal(toStoredLineBreakMarkdown("one\\\ntwo\\\nthree"), "one\ntwo\nthree");
});

test("toStoredLineBreakMarkdown preserves pasted plaintext blank lines", () => {
  assert.equal(
    toStoredLineBreakMarkdown("sample\\\ntext\\\n\\\nlike\\\nthis"),
    "sample\ntext\n\nlike\nthis",
  );
});

test("toStoredLineBreakMarkdown keeps fenced code hardbreak escapes untouched", () => {
  const source = "```md\none\\\ntwo\n```";
  assert.equal(toStoredLineBreakMarkdown(source), source);
});
