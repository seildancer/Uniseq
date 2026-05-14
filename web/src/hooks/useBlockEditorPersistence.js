import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getMarkdown, replaceAll } from "@milkdown/utils";
import { cleanEditorMarkdownForPersistence } from "../utils/stripBreak";

const WRITE_DEBOUNCE_MS = 300;

export function useBlockEditorPersistence({
  get,
  blockHandle,
  text,
  flushRef,
  onMarkdownUpdatedRef,
  onConflict,
}) {
  const suppressWriteRef = useRef(false);
  const latestTextRef = useRef(text);
  const handleRef = useRef(blockHandle);
  const persistedTextRef = useRef(text);
  const initializedRef = useRef(false);
  const persistPromiseRef = useRef(null);
  const debounceRef = useRef(null);
  const dirtyRef = useRef(false);

  useEffect(() => {
    handleRef.current = blockHandle;
    persistedTextRef.current = text;
    if (!dirtyRef.current) {
      latestTextRef.current = text;
    } else if (latestTextRef.current === text) {
      dirtyRef.current = false;
    }
  }, [blockHandle, text]);

  async function persist(cleanedText) {
    if (cleanedText === persistedTextRef.current) {
      if (latestTextRef.current === cleanedText) {
        dirtyRef.current = false;
      }
      return false;
    }

    if (persistPromiseRef.current) {
      await persistPromiseRef.current;
      if (cleanedText === persistedTextRef.current) {
        if (latestTextRef.current === cleanedText) {
          dirtyRef.current = false;
        }
        return false;
      }
    }

    const persistPromise = (async () => {
      try {
        await invoke("write_block_markdown", {
          handle: handleRef.current,
          replacementMarkdown: cleanedText,
        });
        persistedTextRef.current = cleanedText;
        if (latestTextRef.current === cleanedText) {
          dirtyRef.current = false;
        }
        return true;
      } catch (error) {
        if (error?.code === "structural_conflict") {
          await onConflict?.(error);
          return false;
        }
        console.error(error);
        return false;
      } finally {
        persistPromiseRef.current = null;
      }
    })();

    persistPromiseRef.current = persistPromise;
    return persistPromise;
  }

  useEffect(() => {
    flushRef.current = async () => {
      clearTimeout(debounceRef.current);
      const editor = get();
      const markdown = editor ? editor.action(getMarkdown()) : latestTextRef.current;
      const cleaned = cleanEditorMarkdownForPersistence(markdown);
      latestTextRef.current = cleaned;
      dirtyRef.current = cleaned !== persistedTextRef.current;
      return persist(cleaned);
    };
  });

  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      return;
    }
    const editor = get();
    if (!editor) return;
    if (dirtyRef.current) return;
    suppressWriteRef.current = true;
    editor.action(replaceAll(text));
    setTimeout(() => { suppressWriteRef.current = false; }, 0);
  }, [text]); // eslint-disable-line react-hooks/exhaustive-deps

  onMarkdownUpdatedRef.current = (markdown) => {
    const cleaned = cleanEditorMarkdownForPersistence(markdown);
    latestTextRef.current = cleaned;
    if (suppressWriteRef.current) return;
    dirtyRef.current = cleaned !== persistedTextRef.current;
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      void persist(latestTextRef.current);
    }, WRITE_DEBOUNCE_MS);
  };
}
