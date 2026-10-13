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
      stream_names: Array.isArray(dualStreamNames)
        ? dualStreamNames.slice(0, 2)
        : [],
    };
  }

  return null;
}

export function resolveAiChatPresentation(isMobile) {
  return isMobile ? "mobile" : "desktop";
}

export const DEFAULT_AI_CHAT_MODEL = "gpt-6-luna";

export const AI_CHAT_MODELS = [
  { value: "gpt-6-luna", label: "GPT-6 Luna", status: "Stable" },
  { value: "gpt-6.1-sol", label: "GPT-6.1 Sol", status: "Stable" },
];

export function normalizeAiChatModel(model) {
  const trimmed = typeof model === "string" ? model.trim() : "";
  return AI_CHAT_MODELS.some((entry) => entry.value === trimmed)
    ? trimmed
    : DEFAULT_AI_CHAT_MODEL;
}

export function createClosedAiChatState(
  apiKey = "",
  model = DEFAULT_AI_CHAT_MODEL,
) {
  return {
    isOpen: false,
    loadingSession: false,
    sending: false,
    stopping: false,
    requestId: "",
    requestReady: false,
    retryMessage: "",
    retryFromIndex: null,
    responseStatus: "",
    presentation: "desktop",
    sessionId: "",
    sessionTitle: "",
    isPrivate: false,
    contextSpec: null,
    navigationContextSpec: null,
    sessions: [],
    previewSummary: "",
    truncated: false,
    messages: [],
    draft: "",
    apiKey,
    model: normalizeAiChatModel(model),
    error: "",
  };
}

export function createOpeningAiChatState(
  isMobile,
  apiKey = "",
  model = DEFAULT_AI_CHAT_MODEL,
) {
  return {
    ...createClosedAiChatState(apiKey, model),
    isOpen: true,
    loadingSession: true,
    presentation: resolveAiChatPresentation(isMobile),
  };
}

export function applyOpenedAiChatSession(
  openedSession,
  isMobile,
  apiKey = "",
  model = DEFAULT_AI_CHAT_MODEL,
) {
  return {
    ...createOpeningAiChatState(isMobile, apiKey, model),
    loadingSession: false,
    sessionId: openedSession?.session_id ?? "",
    sessionTitle: openedSession?.title ?? "New chat",
    isPrivate: Boolean(openedSession?.is_private),
    contextSpec: openedSession?.context_spec ?? null,
    previewSummary: openedSession?.preview_summary ?? "",
    truncated: Boolean(openedSession?.truncated),
    messages: Array.isArray(openedSession?.messages)
      ? openedSession.messages
      : [],
  };
}

export function applyLoadedAiChatSession(
  session,
  currentState,
  isMobile,
  apiKey = "",
  model = DEFAULT_AI_CHAT_MODEL,
) {
  return {
    ...currentState,
    isOpen: true,
    loadingSession: false,
    sending: false,
    stopping: false,
    requestReady: false,
    requestId: "",
    retryMessage: "",
    retryFromIndex: null,
    responseStatus: "",
    presentation: resolveAiChatPresentation(isMobile),
    sessionId: session?.session_id ?? "",
    sessionTitle: session?.title ?? "New chat",
    isPrivate: Boolean(session?.is_private),
    contextSpec: session?.context_spec ?? null,
    previewSummary: session?.preview_summary ?? "",
    truncated: Boolean(session?.truncated),
    messages: Array.isArray(session?.messages) ? session.messages : [],
    draft: "",
    apiKey,
    model: normalizeAiChatModel(model),
    error: "",
  };
}

export function appendAiChatMessage(state, role, content) {
  return {
    ...state,
    messages: [...(state?.messages ?? []), { role, content }],
  };
}

export function reopenAiChatState(state, isMobile) {
  return {
    ...state,
    isOpen: true,
    loadingSession: false,
    presentation: resolveAiChatPresentation(isMobile),
  };
}

export function aiChatScopeLabel(spec) {
  if (spec?.kind === "stream_single") return spec.stream_name;
  if (spec?.kind === "stream_dual")
    return (spec.stream_names ?? []).join(" + ");
  if (spec?.kind === "page")
    return (spec.page_id ?? "").replace(/^pages:/, "").replaceAll("/", " / ");
  return "Choose context";
}

export function sameAiChatContext(left, right) {
  if (!left || !right || left.kind !== right.kind) return false;
  if (left.kind === "page") return left.page_id === right.page_id;
  if (left.kind === "stream_single")
    return left.stream_name === right.stream_name;
  if (left.kind === "stream_dual") {
    return (
      JSON.stringify(left.stream_names ?? []) ===
      JSON.stringify(right.stream_names ?? [])
    );
  }
  return false;
}

export function shouldStartAiChatForSelection(
  state,
  selection,
  dualStreamNames,
) {
  const navigationContext = buildAiChatContextSpec(selection, dualStreamNames);
  return Boolean(
    navigationContext &&
    !sameAiChatContext(
      state.navigationContextSpec ?? state.contextSpec,
      navigationContext,
    ),
  );
}

export function beginAiChatResponse(state, content, requestId, retry = false) {
  const messages =
    retry && state.retryFromIndex != null
      ? state.messages.slice(0, state.retryFromIndex)
      : state.messages;
  return {
    ...state,
    messages: [
      ...messages,
      { role: "user", content },
      { role: "assistant", content: "" },
    ],
    draft: retry ? state.draft : "",
    sending: true,
    requestReady: false,
    stopping: false,
    requestId,
    retryMessage: content,
    retryFromIndex: messages.length,
    responseStatus: "",
    error: "",
  };
}

export function appendAiChatDelta(state, requestId, delta) {
  if (!state.sending || state.requestId !== requestId) return state;
  if (!delta) return { ...state, requestReady: true };
  const index = state.messages.length - 1;
  if (state.messages[index]?.role !== "assistant") return state;
  return {
    ...state,
    requestReady: true,
    messages: state.messages.map((message, i) =>
      i === index ? { ...message, content: message.content + delta } : message,
    ),
  };
}

export function shouldShowAiChatPreview(state) {
  return Boolean(state?.previewSummary);
}
