import test from "node:test";
import assert from "node:assert/strict";
import {
  isDiaryStream,
  orderStreamNamesForDisplay,
  readSelectedStreamDate,
  shouldBumpStreamReloadToken,
  streamPageExists,
  streamPageId,
} from "./streamWorkspace.js";

test("readSelectedStreamDate preserves the last stream date outside stream mode", () => {
  assert.equal(
    readSelectedStreamDate({ kind: "page", pageId: "pages:A" }, "2026_05_10"),
    "2026_05_10",
  );
  assert.equal(
    readSelectedStreamDate({ kind: "stream_dual", dateName: "2026_05_11" }, "2026_05_10"),
    "2026_05_11",
  );
  assert.equal(
    readSelectedStreamDate({ kind: "stream_single", streamName: "diary", dateName: "2026_05_12" }, "2026_05_10"),
    "2026_05_12",
  );
});

test("streamPageId and streamPageExists resolve single-stream backing ids", () => {
  const streamPagesByDate = new Map([
    ["2026_05_14", new Set(["diary", "journals"])],
  ]);

  assert.equal(streamPageId("diary", "2026_05_14"), "stream:diary/2026_05_14");
  assert.equal(streamPageExists(streamPagesByDate, "2026_05_14", "diary"), true);
  assert.equal(streamPageExists(streamPagesByDate, "2026_05_14", "logs"), false);
});

test("primary streams display journals before diary while preserving diary semantics", () => {
  assert.deepEqual(
    orderStreamNamesForDisplay(["diary", "journals", "logs"]),
    ["journals", "diary", "logs"],
  );
  assert.equal(isDiaryStream("diary"), true);
  assert.equal(isDiaryStream("journals"), false);
});

test("shouldBumpStreamReloadToken only reacts to stream-affecting events while stream UI is active", () => {
  assert.equal(
    shouldBumpStreamReloadToken({ type: "workspace_reloaded" }, true),
    true,
  );
  assert.equal(
    shouldBumpStreamReloadToken({ type: "pages_changed", page_ids: ["pages:A", "stream:diary/2026_05_14"] }, true),
    true,
  );
  assert.equal(
    shouldBumpStreamReloadToken({ type: "pages_changed", page_ids: ["pages:A"] }, true),
    false,
  );
  assert.equal(
    shouldBumpStreamReloadToken({ type: "page_removed", page_id: "stream:diary/2026_05_14" }, true),
    true,
  );
  assert.equal(
    shouldBumpStreamReloadToken({ type: "page_removed", page_id: "stream:diary/2026_05_14" }, false),
    false,
  );
});
