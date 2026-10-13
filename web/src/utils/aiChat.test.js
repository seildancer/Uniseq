import test from "node:test";
import assert from "node:assert/strict";

import {
  AI_CHAT_MODELS,
  DEFAULT_AI_CHAT_MODEL,
  appendAiChatMessage,
  appendAiChatDelta,
  beginAiChatResponse,
  aiChatScopeLabel,
  createClosedAiChatState,
  applyOpenedAiChatSession,
  applyLoadedAiChatSession,
  buildAiChatContextSpec,
  createOpeningAiChatState,
  normalizeAiChatModel,
  reopenAiChatState,
  resolveAiChatPresentation,
  shouldShowAiChatPreview,
  sameAiChatContext,
  shouldStartAiChatForSelection,
} from "./aiChat.js";

test("buildAiChatContextSpec mirrors page and stream selections", () => {
  assert.deepEqual(
    buildAiChatContextSpec({ kind: "page", pageId: "pages:Project" }),
    { kind: "page", page_id: "pages:Project" },
  );
  assert.deepEqual(
    buildAiChatContextSpec({
      kind: "stream_single",
      streamName: "journals",
      dateName: "2026_05_21",
    }),
    { kind: "stream_single", stream_name: "journals" },
  );
  assert.deepEqual(
    buildAiChatContextSpec({ kind: "stream_dual", dateName: "2026_05_21" }, [
      "journals",
      "diary",
      "logs",
    ]),
    { kind: "stream_dual", stream_names: ["journals", "diary"] },
  );
});

test("opening a new AI session discards prior transcript state", () => {
  const prior = appendAiChatMessage(
    appendAiChatMessage(
      applyOpenedAiChatSession(
        {
          session_id: "ai-session-1",
          preview_summary: "Old preview",
          truncated: false,
        },
        false,
        "test-key",
        "gpt-6.1-sol",
      ),
      "user",
      "First question",
    ),
    "assistant",
    "First answer",
  );

  const reopened = applyOpenedAiChatSession(
    {
      session_id: "",
      context_spec: { kind: "page", page_id: "pages:Fresh" },
      preview_summary: "Fresh preview",
      truncated: true,
    },
    false,
    "test-key",
    "gpt-6.1-sol",
  );

  assert.equal(prior.messages.length, 2);
  assert.equal(reopened.messages.length, 0);
  assert.equal(reopened.sessionId, "");
  assert.deepEqual(reopened.contextSpec, {
    kind: "page",
    page_id: "pages:Fresh",
  });
  assert.equal(reopened.previewSummary, "Fresh preview");
  assert.equal(reopened.truncated, true);
  assert.equal(reopened.isPrivate, false);
  assert.equal(reopened.apiKey, "test-key");
  assert.equal(reopened.model, "gpt-6.1-sol");
});

test("preview summary is available before the first message is sent", () => {
  const opening = createOpeningAiChatState(false, "test-key", "gpt-6-luna");
  const opened = applyOpenedAiChatSession(
    {
      session_id: "",
      context_spec: { kind: "stream_single", stream_name: "journals" },
      preview_summary:
        "Start chat based on journals from 2026-05-20 to 2026-05-21.",
      truncated: false,
    },
    false,
    "test-key",
    "gpt-6-luna",
  );

  assert.equal(shouldShowAiChatPreview(opening), false);
  assert.equal(shouldShowAiChatPreview(opened), true);
  assert.equal(opened.sessionId, "");
  assert.equal(opened.isPrivate, false);
  assert.deepEqual(opened.contextSpec, {
    kind: "stream_single",
    stream_name: "journals",
  });
  assert.equal(opened.messages.length, 0);
  assert.equal(opening.apiKey, "test-key");
  assert.equal(opening.model, "gpt-6-luna");
});

test("private sessions stay marked private across open and reopen state helpers", () => {
  const opened = applyOpenedAiChatSession(
    {
      session_id: "ai-private-session-1",
      title: "Private chat",
      is_private: true,
      context_spec: { kind: "page", page_id: "pages:Secret" },
      preview_summary: "Private preview",
      truncated: false,
      messages: [],
    },
    true,
    "test-key",
    "gpt-6-luna",
  );

  const closed = { ...opened, isOpen: false, error: "stale" };
  const reopened = reopenAiChatState(closed, true);

  assert.equal(opened.isPrivate, true);
  assert.equal(reopened.isOpen, true);
  assert.equal(reopened.isPrivate, true);
  assert.equal(reopened.error, "stale");
  assert.equal(reopened.presentation, "mobile");
});

test("chat stays closed by default and collapsing does not lose an active response", () => {
  const closed = createClosedAiChatState("key");
  assert.equal(closed.isOpen, false);
  const sending = beginAiChatResponse(
    { ...closed, isOpen: true },
    "Question",
    "request-1",
  );
  const collapsed = { ...sending, isOpen: false };
  const updated = appendAiChatDelta(collapsed, "request-1", "Partial answer");
  const reopened = reopenAiChatState(updated, false);
  assert.equal(updated.isOpen, false);
  assert.equal(reopened.sending, true);
  assert.equal(reopened.messages.at(-1).content, "Partial answer");
  assert.equal(reopened.requestId, "request-1");
});

test("stream deltas are isolated by request and late events cannot corrupt a finished chat", () => {
  const state = beginAiChatResponse(
    createClosedAiChatState(),
    "Question",
    "current",
  );
  assert.equal(appendAiChatDelta(state, "old", "Wrong answer"), state);
  const ready = appendAiChatDelta(state, "current", "");
  assert.equal(ready.requestReady, true);
  const partial = appendAiChatDelta(ready, "current", "Hello ");
  assert.equal(
    appendAiChatDelta(partial, "current", "world").messages.at(-1).content,
    "Hello world",
  );
  const finished = { ...partial, sending: false };
  assert.equal(appendAiChatDelta(finished, "current", "late"), finished);
});

test("retry replaces the failed turn without duplicating questions or losing the next draft", () => {
  const prior = appendAiChatMessage(
    createClosedAiChatState(),
    "assistant",
    "Earlier answer",
  );
  const sending = beginAiChatResponse(prior, "Failed question", "first");
  const failed = {
    ...appendAiChatDelta(sending, "first", "Partial"),
    sending: false,
    draft: "Next question",
    error: "Disconnected",
  };
  const retry = beginAiChatResponse(failed, failed.retryMessage, "retry", true);
  assert.deepEqual(retry.messages, [
    { role: "assistant", content: "Earlier answer" },
    { role: "user", content: "Failed question" },
    { role: "assistant", content: "" },
  ]);
  assert.equal(retry.draft, "Next question");
  assert.equal(retry.error, "");
});

test("scope labels preserve individual streams, both streams, and topic hierarchy", () => {
  assert.equal(
    aiChatScopeLabel({ kind: "stream_single", stream_name: "journals" }),
    "journals",
  );
  assert.equal(
    aiChatScopeLabel({ kind: "stream_single", stream_name: "diary" }),
    "diary",
  );
  assert.equal(
    aiChatScopeLabel({
      kind: "stream_dual",
      stream_names: ["journals", "diary"],
    }),
    "journals + diary",
  );
  assert.equal(
    aiChatScopeLabel({ kind: "page", page_id: "pages:Research/Questions" }),
    "Research / Questions",
  );
});

test("loading another saved chat clears failed-request state and restores that chat's scope", () => {
  const failed = {
    ...beginAiChatResponse(
      createClosedAiChatState(),
      "Failed question",
      "failed",
    ),
    sending: false,
    stopping: true,
    error: "Disconnected",
    responseStatus: "Response interrupted",
  };
  const loaded = applyLoadedAiChatSession(
    {
      session_id: "saved-chat",
      context_spec: { kind: "stream_single", stream_name: "diary" },
      messages: [{ role: "assistant", content: "Saved answer" }],
    },
    failed,
    false,
    "key",
  );
  assert.equal(loaded.retryMessage, "");
  assert.equal(loaded.retryFromIndex, null);
  assert.equal(loaded.requestId, "");
  assert.equal(loaded.stopping, false);
  assert.equal(loaded.responseStatus, "");
  assert.deepEqual(loaded.contextSpec, {
    kind: "stream_single",
    stream_name: "diary",
  });
  assert.deepEqual(loaded.messages, [
    { role: "assistant", content: "Saved answer" },
  ]);
});

test("resolveAiChatPresentation switches mobile sessions to full-screen mode", () => {
  assert.equal(resolveAiChatPresentation(false), "desktop");
  assert.equal(resolveAiChatPresentation(true), "mobile");
});

test("normalizeAiChatModel falls back to the default for unknown values", () => {
  assert.equal(normalizeAiChatModel("gpt-6.1-sol"), "gpt-6.1-sol");
  assert.equal(normalizeAiChatModel(""), DEFAULT_AI_CHAT_MODEL);
  assert.equal(normalizeAiChatModel("not-a-real-model"), DEFAULT_AI_CHAT_MODEL);
  assert.equal(normalizeAiChatModel("unsupported-model"), DEFAULT_AI_CHAT_MODEL);
  assert.equal(normalizeAiChatModel("  gpt-6.1-sol  "), "gpt-6.1-sol");
  assert.deepEqual(
    AI_CHAT_MODELS.map((entry) => entry.value),
    ["gpt-6-luna", "gpt-6.1-sol"],
  );
});

test("reopening after navigation starts a chat with the newly selected note scope", () => {
  const state = {
    ...createClosedAiChatState(),
    contextSpec: { kind: "page", page_id: "pages:Research" },
    navigationContextSpec: { kind: "page", page_id: "pages:Research" },
  };
  assert.equal(
    shouldStartAiChatForSelection(
      state,
      { kind: "page", pageId: "pages:Research" },
      [],
    ),
    false,
  );
  assert.equal(
    shouldStartAiChatForSelection(
      state,
      { kind: "page", pageId: "pages:Research/Questions" },
      [],
    ),
    true,
  );
  assert.equal(
    shouldStartAiChatForSelection(
      state,
      { kind: "stream_single", streamName: "diary" },
      [],
    ),
    true,
  );
  assert.equal(
    shouldStartAiChatForSelection(state, { kind: "stream_dual" }, [
      "journals",
      "diary",
    ]),
    true,
  );
});

test("moving between dates keeps the same stream scope while switching streams refreshes it", () => {
  const state = {
    contextSpec: { kind: "stream_single", stream_name: "journals" },
  };
  assert.equal(
    shouldStartAiChatForSelection(
      state,
      { kind: "stream_single", streamName: "journals", dateName: "2026_10_12" },
      [],
    ),
    false,
  );
  assert.equal(
    shouldStartAiChatForSelection(
      state,
      { kind: "stream_single", streamName: "diary", dateName: "2026_10_12" },
      [],
    ),
    true,
  );
});

test("an explicitly chosen scope or saved chat stays selected until the user navigates", () => {
  const state = {
    contextSpec: { kind: "page", page_id: "pages:Older topic" },
    navigationContextSpec: { kind: "stream_single", stream_name: "journals" },
  };
  assert.equal(
    shouldStartAiChatForSelection(
      state,
      { kind: "stream_single", streamName: "journals" },
      [],
    ),
    false,
  );
  assert.equal(
    shouldStartAiChatForSelection(
      state,
      { kind: "page", pageId: "pages:Another topic" },
      [],
    ),
    true,
  );
  assert.deepEqual(state.contextSpec, {
    kind: "page",
    page_id: "pages:Older topic",
  });
});

test("context comparisons do not depend on object property order", () => {
  assert.equal(
    sameAiChatContext(
      { kind: "page", page_id: "pages:A" },
      { page_id: "pages:A", kind: "page" },
    ),
    true,
  );
  assert.equal(
    sameAiChatContext(
      { kind: "stream_dual", stream_names: ["journals", "diary"] },
      { kind: "stream_dual", stream_names: ["journals"] },
    ),
    false,
  );
});
