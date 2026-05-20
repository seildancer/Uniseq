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

test("cleanEditorMarkdownForPersistence unescapes hashtag page-ref bodies", () => {
  const source = "- \\#some\\_page and #other\\_page";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    "- #some_page and #other_page",
  );
});

test("cleanEditorMarkdownForPersistence keeps escaped headings intact", () => {
  const source = "\\# heading";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    "\\# heading",
  );
});

test("cleanEditorMarkdownForPersistence unescapes wikilink page-ref bodies", () => {
  const source = "- \\[\\[some\\_page\\]\\]";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    "- [[some_page]]",
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

test("cleanEditorMarkdownForPersistence leaves inline code spans untouched", () => {
  const source = "- `\\[\\[A\\_B\\]\\] \\#foo\\_bar <br>` and \\[\\[B\\_C\\]\\] and #D\\_E";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    "- `\\[\\[A\\_B\\]\\] \\#foo\\_bar <br>` and [[B_C]] and #D_E",
  );
});

test("cleanEditorMarkdownForPersistence leaves escaped underscores outside refs untouched", () => {
  const source = "plain some\\_text and #some\\_page";

  assert.equal(
    cleanEditorMarkdownForPersistence(source),
    "plain some\\_text and #some_page",
  );
});
