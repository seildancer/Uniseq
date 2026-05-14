import test from "node:test";
import assert from "node:assert/strict";
import {
  addDaysToDateName,
  buildCenteredDateRange,
  buildDateRange,
  buildRecentStreamDateWindow,
  compareDateNames,
  maxDateName,
} from "./streamDates.js";

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

test("compareDateNames follows chronological order", () => {
  assert.equal(compareDateNames("2026_05_13", "2026_05_14"), -1);
  assert.equal(compareDateNames("2026_05_14", "2026_05_14"), 0);
  assert.equal(compareDateNames("2026_05_15", "2026_05_14"), 1);
});

test("buildDateRange includes both endpoints in the requested direction", () => {
  assert.deepEqual(buildDateRange("2026_05_11", "2026_05_14"), [
    "2026_05_11",
    "2026_05_12",
    "2026_05_13",
    "2026_05_14",
  ]);
  assert.deepEqual(buildDateRange("2026_05_14", "2026_05_11"), [
    "2026_05_14",
    "2026_05_13",
    "2026_05_12",
    "2026_05_11",
  ]);
});

test("buildCenteredDateRange shifts around latest bounds instead of creating future dates", () => {
  assert.deepEqual(buildCenteredDateRange("2026_05_14", 5, null, "2026_05_14"), {
    startDateName: "2026_05_10",
    endDateName: "2026_05_14",
  });
});

test("buildCenteredDateRange centers unbounded selected dates", () => {
  assert.deepEqual(buildCenteredDateRange("2026_05_14", 5), {
    startDateName: "2026_05_12",
    endDateName: "2026_05_16",
  });
});

test("maxDateName returns the latest candidate with fallback support", () => {
  assert.equal(maxDateName(["2026_05_12", "2026_05_16", "2026_05_14"], "2026_05_10"), "2026_05_16");
  assert.equal(maxDateName([null, "", "2026_05_12"], "2026_05_10"), "2026_05_12");
  assert.equal(maxDateName([], "2026_05_10"), "2026_05_10");
});
