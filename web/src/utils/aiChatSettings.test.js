import test from "node:test";
import assert from "node:assert/strict";
import { loadAiChatApiKey, saveAiChatApiKey } from "./aiChatSettings.js";

function storageWith(key) {
  const entries = new Map([["aiChatOpenAiApiKey", key]]);
  return {
    getItem: (name) => entries.get(name) ?? null,
    removeItem: (name) => entries.delete(name),
  };
}

test("native key is restored without relying on WebView storage", async () => {
  const storage = { getItem() { throw new Error("storage unavailable"); } };
  assert.equal(await loadAiChatApiKey(async () => "saved-openai-key", storage), "saved-openai-key");
});

test("saving and removing survive a fresh WebView storage instance", async () => {
  let nativeKey = null;
  const invoke = async (command, args) => {
    if (command === "set_ai_chat_api_key") nativeKey = args.apiKey;
    return nativeKey;
  };
  await saveAiChatApiKey(invoke, storageWith(null), " test-key ");
  assert.equal(await loadAiChatApiKey(invoke, storageWith(null)), "test-key");
  await saveAiChatApiKey(invoke, storageWith(null), "");
  assert.equal(await loadAiChatApiKey(invoke, storageWith(null)), "");
});

test("existing OpenAI key moves to native storage after successful save", async () => {
  const storage = storageWith(" existing-key ");
  const calls = [];
  const invoke = async (command, args) => {
    calls.push([command, args]);
    return null;
  };
  assert.equal(await loadAiChatApiKey(invoke, storage), "existing-key");
  assert.deepEqual(calls[1], ["set_ai_chat_api_key", { apiKey: "existing-key" }]);
  assert.equal(storage.getItem("aiChatOpenAiApiKey"), null);
});

test("failed native save rejects and preserves the previous key", async () => {
  const storage = storageWith("previous-key");
  const invoke = async () => { throw new Error("disk unavailable"); };
  await assert.rejects(saveAiChatApiKey(invoke, storage, "replacement"), /disk unavailable/);
  assert.equal(storage.getItem("aiChatOpenAiApiKey"), "previous-key");
});

test("failed OpenAI migration keeps the existing key for another attempt", async () => {
  const storage = storageWith("previous-key");
  const invoke = async (command) => {
    if (command === "get_ai_chat_api_key") return null;
    throw new Error("disk unavailable");
  };
  await assert.rejects(loadAiChatApiKey(invoke, storage), /disk unavailable/);
  assert.equal(storage.getItem("aiChatOpenAiApiKey"), "previous-key");
});

test("removed native key cannot be resurrected from old WebView storage", async () => {
  assert.equal(await loadAiChatApiKey(async () => "", storageWith("old-key")), "");
});

test("read failure is surfaced instead of treating the key as missing", async () => {
  await assert.rejects(loadAiChatApiKey(async () => { throw new Error("read failed"); }, storageWith("old-key")), /read failed/);
});
