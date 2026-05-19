import test from "node:test";
import assert from "node:assert/strict";

import { cleanEditorMarkdownForPersistence } from "./stripBreak.js";

test("cleanEditorMarkdownForPersistence normalizes escaped hash tags at list starts", () => {
  const source = "- \\#foo";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    "- #foo",
  );
});

test("cleanEditorMarkdownForPersistence keeps escaped headings intact", () => {
  const source = "\\# heading";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    "\\# heading",
  );
});

test("cleanEditorMarkdownForPersistence collapses accidental blank lines in nested lists", () => {
  const source = "- \\#foo\n\n  - bar\n\n  - foobar";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    "- #foo\n  - bar\n  - foobar",
  );
});

test("cleanEditorMarkdownForPersistence does not rewrite fenced code", () => {
  const source = "```\n- \\#foo\n\n  - bar\n```";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    source,
  );
});
