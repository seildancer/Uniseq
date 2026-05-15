import { useContext, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { replaceAll } from "@milkdown/utils";
import { cleanEditorMarkdownForPersistence } from "../utils/stripBreak";
import { toEditorMarkdown, toStoredMarkdown } from "../utils/imageMarkdown";
import { WorkspaceContext } from "../WorkspaceContext.js";

const WRITE_DEBOUNCE_MS = 300;

export function useBlockEditorPersistence({
  get,
  blockHandle,
  text,
  flushRef,
  onMarkdownUpdatedRef,
  onConflict,
}) {
  const workspaceRoot = useContext(WorkspaceContext);
  const workspaceRootRef = useRef(workspaceRoot);
  workspaceRootRef.current = workspaceRoot;

  const suppressWriteRef = useRef(false);
  const latestTextRef = useRef(text);
  const handleRef = useRef(blockHandle);
  const persistedTextRef = useRef(text);
  const initializedRef = useRef(false);
  const persistPromiseRef = useRef(null);
  const debounceRef = useRef(null);

  useEffect(() => {
    latestTextRef.current = text;
    handleRef.current = blockHandle;
    persistedTextRef.current = text;
  }, [blockHandle, text]);

  async function persist(cleanedText) {
    if (cleanedText === persistedTextRef.current) {
      return false;
    }

    if (persistPromiseRef.current) {
      await persistPromiseRef.current;
      if (cleanedText === persistedTextRef.current) {
        return false;
      }
    }

    const persistPromise = (async () => {
      try {
        await invoke("write_block_markdown", {
          handle: handleRef.current,
          replacementMarkdown: cleanedText,
        });
        latestTextRef.current = cleanedText;
        persistedTextRef.current = cleanedText;
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
      const cleaned = cleanEditorMarkdownForPersistence(toStoredMarkdown(latestTextRef.current));
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
    suppressWriteRef.current = true;
    editor.action(replaceAll(toEditorMarkdown(text, workspaceRootRef.current)));
    setTimeout(() => { suppressWriteRef.current = false; }, 0);
  }, [text]); // eslint-disable-line react-hooks/exhaustive-deps

  onMarkdownUpdatedRef.current = (markdown) => {
    const cleaned = cleanEditorMarkdownForPersistence(toStoredMarkdown(markdown));
    latestTextRef.current = cleaned;
    if (suppressWriteRef.current) return;
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      void persist(latestTextRef.current);
    }, WRITE_DEBOUNCE_MS);
  };
}
