import test from "node:test";
import assert from "node:assert/strict";
import { mockConvertFileSrc } from "@tauri-apps/api/mocks";

import { toEditorMarkdown, toStoredMarkdown } from "./imageMarkdown.js";

globalThis.window = globalThis;
mockConvertFileSrc("windows");

test("toEditorMarkdown rewrites stored asset markdown to a displayable tauri file URL", () => {
  const source = "Before\n![photo.jpg](../assets/photo_123_0.jpg){:height 878, :width 304}\nAfter";
  const transformed = toEditorMarkdown(source, "C:\\Users\\me\\Notebook");

  assert.match(transformed, /!\[photo\.jpg\]\(http:\/\/asset\.localhost\//);
  assert.match(transformed, /"uniseq-image\|878\|304\|\.\.%2Fassets%2Fphoto_123_0\.jpg"\)/);
});

test("toStoredMarkdown restores persisted workspace-relative asset markdown", () => {
  const source =
    '![photo.jpg](asset://localhost/C:%5CUsers%5Cme%5CNotebook%5Cassets%5Cphoto_123_0.jpg "uniseq-image|878|304|..%2Fassets%2Fphoto_123_0.jpg")';

  assert.equal(
    toStoredMarkdown(source),
    "![photo.jpg](../assets/photo_123_0.jpg){:height 878, :width 304}",
  );
});
