import test from "node:test";
import assert from "node:assert/strict";
import { addDaysToDateName, buildRecentStreamDateWindow } from "./streamDates.js";

test("addDaysToDateName walks across month and year boundaries", () => {
  assert.equal(addDaysToDateName("2026_01_01", -1), "2025_12_31");
  assert.equal(addDaysToDateName("2026_02_28", 1), "2026_03_01");
});

test("buildRecentStreamDateWindow returns the selected day followed by prior days", () => {
  assert.deepEqual(buildRecentStreamDateWindow("2026_05_14", 4), [
    "2026_05_14",
    "2026_05_13",
    "2026_05_12",
    "2026_05_11",
  ]);
});
