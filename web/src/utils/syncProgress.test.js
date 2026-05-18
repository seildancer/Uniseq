import test from "node:test";
import assert from "node:assert/strict";

import {
  coerceSyncProgress,
  normalizeSyncProgressOperation,
  normalizeSyncProgressPhase,
} from "./syncProgress.js";

test("normalizeSyncProgressOperation accepts snake case and pascal case enum values", () => {
  assert.equal(normalizeSyncProgressOperation("initial_pull"), "initial_pull");
  assert.equal(normalizeSyncProgressOperation("InitialPull"), "initial_pull");
  assert.equal(normalizeSyncProgressOperation("initial-pull"), "initial_pull");
});

test("normalizeSyncProgressPhase accepts pascal case enum values", () => {
  assert.equal(normalizeSyncProgressPhase("DeletingRemote"), "deleting_remote");
  assert.equal(normalizeSyncProgressPhase("deleting_remote"), "deleting_remote");
});

test("coerceSyncProgress normalizes a raw progress payload into the UI shape", () => {
  assert.deepEqual(coerceSyncProgress({
    operation: "Sync",
    phase: "Listing",
    current: "4",
    total: 10,
    path: "pages/intro.md",
    detail: "Uploading pages/intro.md",
  }), {
    operation: "sync",
    phase: "listing",
    current: 4,
    total: 10,
    path: "pages/intro.md",
    detail: "Uploading pages/intro.md",
  });
});
