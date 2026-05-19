import { useContext, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cleanEditorMarkdownForPersistence } from "../utils/stripBreak";
import { toStoredMarkdown } from "../utils/imageMarkdown";
import { WorkspaceContext } from "../WorkspaceContext.js";
import { applyExternalEditorText } from "./applyExternalEditorText";

const WRITE_DEBOUNCE_MS = 300;

export function useEditorPersistence({
  get,
  pageId,
  text,
  revision,
  flushRef,
  onMarkdownUpdatedRef,
  onConflict,
  onPersisted,
}) {
  const workspaceRoot = useContext(WorkspaceContext);
  const workspaceRootRef = useRef(workspaceRoot);
  workspaceRootRef.current = workspaceRoot;

  const debounceRef = useRef(null);
  const suppressWriteRef = useRef(false);
  const latestTextRef = useRef(text);
  const revisionRef = useRef(revision);
  const persistedTextRef = useRef(text);
  const initializedRef = useRef(false);

  useEffect(() => {
    revisionRef.current = revision;
    persistedTextRef.current = text;
  }, [text, revision]);

  async function persist(cleanedText) {
    if (cleanedText === persistedTextRef.current) {
      return;
    }

    try {
      const updated = await invoke("write_page_content", {
        pageId,
        text: cleanedText,
        expectedRevision: revisionRef.current,
      });
      latestTextRef.current = updated.text;
      persistedTextRef.current = updated.text;
      revisionRef.current = updated.revision;
      onPersisted?.(updated);
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
      const cleaned = cleanEditorMarkdownForPersistence(toStoredMarkdown(latestTextRef.current));
      void persist(cleaned);
    };
  });

  // Reload editor content when an external file change arrives on the same page.
  useEffect(() => {
    applyExternalEditorText({
      initializedRef,
      getEditor: get,
      nextText: text,
      latestTextRef,
      suppressWriteRef,
      workspaceRoot: workspaceRootRef.current,
      clearPendingWrite: () => clearTimeout(debounceRef.current),
    });
  }, [text, revision]); // eslint-disable-line react-hooks/exhaustive-deps

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
