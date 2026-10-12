import test from "node:test";
import assert from "node:assert/strict";
import { describeSyncDifference, describeSyncText, revealSyncWhitespace } from "./syncComparison.js";

test("comparison distinguishes identical text, line endings, whitespace, and changed words", () => {
  assert.match(describeSyncDifference("same", "same"), /identical/);
  assert.match(describeSyncDifference("a\r\nb\r", "a\nb\n"), /Only line endings/);
  assert.match(describeSyncDifference("a  \n", "a"), /Only whitespace/);
  assert.match(describeSyncDifference("old", "new"), /contents differ/);
});

test("metadata counts mixed line endings and UTF-8 bytes without double counting CRLF", () => {
  assert.equal(describeSyncText("é\r\na\nb\r"), "8 bytes · 1 CRLF, 1 LF, 1 CR · final newline");
  assert.equal(describeSyncText(""), "0 bytes · no line breaks · no final newline");
});

test("visible whitespace exposes final newlines and otherwise invisible characters", () => {
  assert.equal(revealSyncWhitespace(" \t\r\n\r\n\u00a0\ufeff\u200b"), "·[TAB][CRLF]\n[CRLF]\n[NBSP][BOM][ZWSP][EOF]");
  assert.equal(revealSyncWhitespace("a\r"), "a[CR]\n[EOF]");
  assert.equal(revealSyncWhitespace("a\n"), "a[LF]\n[EOF]");
  assert.equal(revealSyncWhitespace("a"), "a[EOF]");
});
