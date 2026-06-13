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

export const DEFAULT_AI_CHAT_MODEL = "gemini-3.5-flash";

export const AI_CHAT_MODELS = [
  { value: "gemini-3.5-flash", label: "Gemini 3.5 Flash", status: "Stable" },
  { value: "gemini-3-flash-preview", label: "Gemini 3 Flash", status: "Preview" },
  { value: "gemini-3.1-pro-preview", label: "Gemini 3.1 Pro", status: "Preview" },
  { value: "gemini-3.1-flash-lite", label: "Gemini 3.1 Flash-Lite", status: "Stable" },
  { value: "gemini-2.5-pro", label: "Gemini 2.5 Pro", status: "Stable" },
  { value: "gemini-2.5-flash", label: "Gemini 2.5 Flash", status: "Stable" },
  { value: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite", status: "Stable" },
];

export function normalizeAiChatModel(model) {
  const trimmed = typeof model === "string" ? model.trim() : "";
  return AI_CHAT_MODELS.some((entry) => entry.value === trimmed)
    ? trimmed
    : DEFAULT_AI_CHAT_MODEL;
}

export function createClosedAiChatState(apiKey = "", model = DEFAULT_AI_CHAT_MODEL) {
  return {
    isOpen: false,
    loadingSession: false,
    sending: false,
    presentation: "desktop",
    sessionId: "",
    sessionTitle: "",
    contextSpec: null,
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

export function createOpeningAiChatState(isMobile, apiKey = "", model = DEFAULT_AI_CHAT_MODEL) {
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
    contextSpec: openedSession?.context_spec ?? null,
    previewSummary: openedSession?.preview_summary ?? "",
    truncated: Boolean(openedSession?.truncated),
    messages: Array.isArray(openedSession?.messages) ? openedSession.messages : [],
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
    presentation: resolveAiChatPresentation(isMobile),
    sessionId: session?.session_id ?? "",
    sessionTitle: session?.title ?? "New chat",
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

export function shouldShowAiChatPreview(state) {
  return Boolean(state?.previewSummary);
}
