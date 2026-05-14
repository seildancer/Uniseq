import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getMarkdown, replaceAll } from "@milkdown/utils";
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
  const persistedTextRef = useRef(text);
  const initializedRef = useRef(false);
  const dirtyRef = useRef(false);
  const persistPromiseRef = useRef(null);

  useEffect(() => {
    revisionRef.current = revision;
    persistedTextRef.current = text;
    if (!dirtyRef.current) {
      latestTextRef.current = text;
    } else if (latestTextRef.current === text) {
      dirtyRef.current = false;
    }
  }, [text, revision]);

  async function writePageContent(cleanedText, expectedRevision) {
    return invoke("write_page_content", {
      pageId,
      text: cleanedText,
      expectedRevision,
    });
  }

  async function retryAfterStructuralConflict(cleanedText) {
    const latest = await invoke("page_content", { pageId });
    revisionRef.current = latest.revision;

    if (latest.text === cleanedText) {
      persistedTextRef.current = latest.text;
      if (latestTextRef.current === cleanedText) {
        dirtyRef.current = false;
      }
      return latest;
    }

    if (latest.text !== persistedTextRef.current) {
      persistedTextRef.current = latest.text;
      return null;
    }

    const updated = await writePageContent(cleanedText, latest.revision);
    persistedTextRef.current = updated.text;
    revisionRef.current = updated.revision;
    if (latestTextRef.current === updated.text) {
      dirtyRef.current = false;
    }
    return updated;
  }

  async function persist(cleanedText) {
    if (cleanedText === persistedTextRef.current) {
      if (latestTextRef.current === cleanedText) {
        dirtyRef.current = false;
      }
      return true;
    }

    if (persistPromiseRef.current) {
      await persistPromiseRef.current;
      if (cleanedText === persistedTextRef.current) {
        if (latestTextRef.current === cleanedText) {
          dirtyRef.current = false;
        }
        return true;
      }
    }

    const persistPromise = (async () => {
      try {
        const updated = await writePageContent(cleanedText, revisionRef.current);
        persistedTextRef.current = updated.text;
        revisionRef.current = updated.revision;
        if (latestTextRef.current === updated.text) {
          dirtyRef.current = false;
        }
        return true;
      } catch (error) {
        if (error?.code === "structural_conflict") {
          try {
            if (await retryAfterStructuralConflict(cleanedText)) {
              return true;
            }
          } catch (retryError) {
            console.error(retryError);
          }
          onConflict?.(error);
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
    flushRef.current = () => {
      clearTimeout(debounceRef.current);
      const editor = get();
      const markdown = editor ? editor.action(getMarkdown()) : latestTextRef.current;
      const cleaned = cleanEditorMarkdownForPersistence(markdown);
      latestTextRef.current = cleaned;
      dirtyRef.current = cleaned !== persistedTextRef.current;
      return persist(cleaned);
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
    if (dirtyRef.current) return;
    suppressWriteRef.current = true;
    editor.action(replaceAll(text));
    clearTimeout(debounceRef.current);
    setTimeout(() => { suppressWriteRef.current = false; }, 0);
  }, [text, revision]); // eslint-disable-line react-hooks/exhaustive-deps

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
