import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { replaceAll } from "@milkdown/utils";
import { stripBreakOutsideFencedCode } from "../utils/stripBreak";

const WRITE_DEBOUNCE_MS = 300;

export function useEditorPersistence({ get, pageId, text, flushRef, onMarkdownUpdatedRef }) {
  const debounceRef = useRef(null);
  const suppressWriteRef = useRef(false);
  const latestTextRef = useRef(text);
  const initializedRef = useRef(false);

  useEffect(() => {
    flushRef.current = () => {
      clearTimeout(debounceRef.current);
      const cleaned = stripBreakOutsideFencedCode(latestTextRef.current);
      invoke("write_page_content", { pageId, text: cleaned }).catch(() => {});
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
  }, [text]); // eslint-disable-line react-hooks/exhaustive-deps

  onMarkdownUpdatedRef.current = (markdown) => {
    const cleaned = stripBreakOutsideFencedCode(markdown);
    latestTextRef.current = cleaned;
    if (suppressWriteRef.current) return;
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      invoke("write_page_content", { pageId, text: latestTextRef.current }).catch(() => {});
    }, WRITE_DEBOUNCE_MS);
  };
}
