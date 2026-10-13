const OPENAI_KEY_STORAGE_KEY = "aiChatOpenAiApiKey";

export async function loadAiChatApiKey(invoke, storage) {
  const savedKey = await invoke("get_ai_chat_api_key");
  if (savedKey !== null) return savedKey;

  // Move existing OpenAI keys out of WebView storage after a successful disk save.
  let previousKey = "";
  try {
    previousKey = storage.getItem(OPENAI_KEY_STORAGE_KEY) ?? "";
  } catch {
    return "";
  }
  if (previousKey) await saveAiChatApiKey(invoke, storage, previousKey);
  return previousKey.trim();
}

export async function saveAiChatApiKey(invoke, storage, apiKey) {
  await invoke("set_ai_chat_api_key", { apiKey: apiKey.trim() });
  try {
    storage.removeItem(OPENAI_KEY_STORAGE_KEY);
  } catch {
    // The native store is authoritative even if WebView cleanup is unavailable.
  }
}
