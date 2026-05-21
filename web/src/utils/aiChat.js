export function buildAiChatContextSpec(selection, dualStreamNames = []) {
  if (!selection || typeof selection !== "object") {
    return null;
  }

  if (selection.kind === "page" && selection.pageId) {
    return { kind: "page", page_id: selection.pageId };
  }

  if (selection.kind === "stream_single" && selection.streamName) {
    return { kind: "stream_single", stream_name: selection.streamName };
  }

  if (selection.kind === "stream_dual") {
    return {
      kind: "stream_dual",
      stream_names: Array.isArray(dualStreamNames) ? dualStreamNames.slice(0, 2) : [],
    };
  }

  return null;
}

export function resolveAiChatPresentation(isMobile) {
  return isMobile ? "mobile" : "desktop";
}

export function createClosedAiChatState(apiKey = "") {
  return {
    isOpen: false,
    loadingSession: false,
    sending: false,
    presentation: "desktop",
    sessionId: "",
    previewSummary: "",
    truncated: false,
    messages: [],
    draft: "",
    apiKey,
    error: "",
  };
}

export function createOpeningAiChatState(isMobile, apiKey = "") {
  return {
    ...createClosedAiChatState(apiKey),
    isOpen: true,
    loadingSession: true,
    presentation: resolveAiChatPresentation(isMobile),
  };
}

export function applyOpenedAiChatSession(openedSession, isMobile, apiKey = "") {
  return {
    ...createOpeningAiChatState(isMobile, apiKey),
    loadingSession: false,
    sessionId: openedSession?.session_id ?? "",
    previewSummary: openedSession?.preview_summary ?? "",
    truncated: Boolean(openedSession?.truncated),
  };
}

export function appendAiChatMessage(state, role, content) {
  return {
    ...state,
    messages: [...(state?.messages ?? []), { role, content }],
  };
}

export function shouldShowAiChatPreview(state) {
  return Boolean(state?.previewSummary);
}
