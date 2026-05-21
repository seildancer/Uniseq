import test from "node:test";
import assert from "node:assert/strict";

import {
  appendAiChatMessage,
  applyOpenedAiChatSession,
  buildAiChatContextSpec,
  createOpeningAiChatState,
  resolveAiChatPresentation,
  shouldShowAiChatPreview,
} from "./aiChat.js";

test("buildAiChatContextSpec mirrors page and stream selections", () => {
  assert.deepEqual(
    buildAiChatContextSpec({ kind: "page", pageId: "pages:Project" }),
    { kind: "page", page_id: "pages:Project" },
  );
  assert.deepEqual(
    buildAiChatContextSpec({ kind: "stream_single", streamName: "journals", dateName: "2026_05_21" }),
    { kind: "stream_single", stream_name: "journals" },
  );
  assert.deepEqual(
    buildAiChatContextSpec({ kind: "stream_dual", dateName: "2026_05_21" }, ["journals", "diary", "logs"]),
    { kind: "stream_dual", stream_names: ["journals", "diary"] },
  );
});

test("opening a new AI session discards prior transcript state", () => {
  const prior = appendAiChatMessage(
    appendAiChatMessage(
      applyOpenedAiChatSession({
        session_id: "ai-session-1",
        preview_summary: "Old preview",
        truncated: false,
      }, false, "test-key"),
      "user",
      "First question",
    ),
    "assistant",
    "First answer",
  );

  const reopened = applyOpenedAiChatSession({
    session_id: "ai-session-2",
    preview_summary: "Fresh preview",
    truncated: true,
  }, false, "test-key");

  assert.equal(prior.messages.length, 2);
  assert.equal(reopened.messages.length, 0);
  assert.equal(reopened.sessionId, "ai-session-2");
  assert.equal(reopened.previewSummary, "Fresh preview");
  assert.equal(reopened.truncated, true);
  assert.equal(reopened.apiKey, "test-key");
});

test("preview summary is available before the first message is sent", () => {
  const opening = createOpeningAiChatState(false, "test-key");
  const opened = applyOpenedAiChatSession({
    session_id: "ai-session-3",
    preview_summary: "Start chat based on journals from 2026-05-20 to 2026-05-21.",
    truncated: false,
  }, false, "test-key");

  assert.equal(shouldShowAiChatPreview(opening), false);
  assert.equal(shouldShowAiChatPreview(opened), true);
  assert.equal(opened.messages.length, 0);
  assert.equal(opening.apiKey, "test-key");
});

test("resolveAiChatPresentation switches mobile sessions to full-screen mode", () => {
  assert.equal(resolveAiChatPresentation(false), "desktop");
  assert.equal(resolveAiChatPresentation(true), "mobile");
});
