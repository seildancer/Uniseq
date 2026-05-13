import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { replaceAll } from "@milkdown/utils";
import { cleanEditorMarkdownForPersistence } from "../utils/stripBreak";

const WRITE_DEBOUNCE_MS = 300;

export function useEditorPersistence({
  get,
  pageId,
  text,
  revision,
  flushRef,
  onMarkdownUpdatedRef,
  onConflict,
}) {
  const debounceRef = useRef(null);
  const suppressWriteRef = useRef(false);
  const latestTextRef = useRef(text);
  const revisionRef = useRef(revision);
  const initializedRef = useRef(false);

  useEffect(() => {
    latestTextRef.current = text;
    revisionRef.current = revision;
  }, [text, revision]);

  async function persist(cleanedText) {
    try {
      const updated = await invoke("write_page_content", {
        pageId,
        text: cleanedText,
        expectedRevision: revisionRef.current,
      });
      latestTextRef.current = updated.text;
      revisionRef.current = updated.revision;
    } catch (error) {
      if (error?.code === "structural_conflict") {
        onConflict?.(error);
        return;
      }
      console.error(error);
    }
  }

  useEffect(() => {
    flushRef.current = () => {
      clearTimeout(debounceRef.current);
      const cleaned = cleanEditorMarkdownForPersistence(latestTextRef.current);
      void persist(cleaned);
    };
  });

  // Reload editor content when an external file change arrives on the same page.
  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      return;
    }
    const editor = get();
    if (!editor) return;
    suppressWriteRef.current = true;
    editor.action(replaceAll(text));
    clearTimeout(debounceRef.current);
    setTimeout(() => { suppressWriteRef.current = false; }, 0);
  }, [text, revision]); // eslint-disable-line react-hooks/exhaustive-deps

  onMarkdownUpdatedRef.current = (markdown) => {
    const cleaned = cleanEditorMarkdownForPersistence(markdown);
    latestTextRef.current = cleaned;
    if (suppressWriteRef.current) return;
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      void persist(latestTextRef.current);
    }, WRITE_DEBOUNCE_MS);
  };
}
