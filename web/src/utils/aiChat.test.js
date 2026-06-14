import test from "node:test";
import assert from "node:assert/strict";

import {
  AI_CHAT_MODELS,
  DEFAULT_AI_CHAT_MODEL,
  appendAiChatMessage,
  applyOpenedAiChatSession,
  buildAiChatContextSpec,
  createOpeningAiChatState,
  normalizeAiChatModel,
  reopenAiChatState,
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
      }, false, "test-key", "gemini-2.5-pro"),
      "user",
      "First question",
    ),
    "assistant",
    "First answer",
  );

  const reopened = applyOpenedAiChatSession({
    session_id: "",
    context_spec: { kind: "page", page_id: "pages:Fresh" },
    preview_summary: "Fresh preview",
    truncated: true,
  }, false, "test-key", "gemini-2.5-pro");

  assert.equal(prior.messages.length, 2);
  assert.equal(reopened.messages.length, 0);
  assert.equal(reopened.sessionId, "");
  assert.deepEqual(reopened.contextSpec, { kind: "page", page_id: "pages:Fresh" });
  assert.equal(reopened.previewSummary, "Fresh preview");
  assert.equal(reopened.truncated, true);
  assert.equal(reopened.isPrivate, false);
  assert.equal(reopened.apiKey, "test-key");
  assert.equal(reopened.model, "gemini-2.5-pro");
});

test("preview summary is available before the first message is sent", () => {
  const opening = createOpeningAiChatState(false, "test-key", "gemini-2.5-flash");
  const opened = applyOpenedAiChatSession({
    session_id: "",
    context_spec: { kind: "stream_single", stream_name: "journals" },
    preview_summary: "Start chat based on journals from 2026-05-20 to 2026-05-21.",
    truncated: false,
  }, false, "test-key", "gemini-2.5-flash");

  assert.equal(shouldShowAiChatPreview(opening), false);
  assert.equal(shouldShowAiChatPreview(opened), true);
  assert.equal(opened.sessionId, "");
  assert.equal(opened.isPrivate, false);
  assert.deepEqual(opened.contextSpec, { kind: "stream_single", stream_name: "journals" });
  assert.equal(opened.messages.length, 0);
  assert.equal(opening.apiKey, "test-key");
  assert.equal(opening.model, "gemini-2.5-flash");
});

test("private sessions stay marked private across open and reopen state helpers", () => {
  const opened = applyOpenedAiChatSession({
    session_id: "ai-private-session-1",
    title: "Private chat",
    is_private: true,
    context_spec: { kind: "page", page_id: "pages:Secret" },
    preview_summary: "Private preview",
    truncated: false,
    messages: [],
  }, true, "test-key", "gemini-2.5-flash");

  const closed = { ...opened, isOpen: false, error: "stale" };
  const reopened = reopenAiChatState(closed, true);

  assert.equal(opened.isPrivate, true);
  assert.equal(reopened.isOpen, true);
  assert.equal(reopened.isPrivate, true);
  assert.equal(reopened.error, "");
  assert.equal(reopened.presentation, "mobile");
});

test("resolveAiChatPresentation switches mobile sessions to full-screen mode", () => {
  assert.equal(resolveAiChatPresentation(false), "desktop");
  assert.equal(resolveAiChatPresentation(true), "mobile");
});

test("normalizeAiChatModel falls back to the default for unknown values", () => {
  assert.equal(normalizeAiChatModel("gemini-2.5-pro"), "gemini-2.5-pro");
  assert.equal(normalizeAiChatModel(""), DEFAULT_AI_CHAT_MODEL);
  assert.equal(normalizeAiChatModel("not-a-real-model"), DEFAULT_AI_CHAT_MODEL);
  assert.ok(AI_CHAT_MODELS.length >= 7);
});
