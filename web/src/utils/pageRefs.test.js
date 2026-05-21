import test from "node:test";
import assert from "node:assert/strict";

import { pageMatchesRefText, pageRefBody, pageRefLabel } from "./pageRefs.js";

test("pageRefBody returns the hierarchy path for regular pages", () => {
  assert.equal(pageRefBody("pages:foo/bar"), "foo/bar");
  assert.equal(pageRefBody("pages:Solo"), "Solo");
});

test("pageRefBody ignores non-page-backed ids", () => {
  assert.equal(pageRefBody("stream:diary/2026_05_21"), "");
  assert.equal(pageRefBody("foo/bar"), "");
});

test("pageRefLabel prefers the full hierarchy path", () => {
  assert.equal(pageRefLabel({ page_id: "pages:foo/bar", title: "bar" }), "foo/bar");
  assert.equal(pageRefLabel({ page_id: "pages:Solo", title: "Solo" }), "Solo");
});

test("pageMatchesRefText resolves hierarchical refs by their ref body", () => {
  const page = { page_id: "pages:foo/bar", title: "bar" };

  assert.equal(pageMatchesRefText(page, "foo/bar"), true);
  assert.equal(pageMatchesRefText(page, "bar"), true);
  assert.equal(pageMatchesRefText(page, "pages:foo/bar"), true);
  assert.equal(pageMatchesRefText(page, "baz"), false);
});
