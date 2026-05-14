import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cleanEditorMarkdownForPersistence } from "../utils/stripBreak";

const WRITE_DEBOUNCE_MS = 300;

export function useVirtualStreamPersistence({
  streamName,
  dateName,
  flushRef,
  onMarkdownUpdatedRef,
  onError,
  onFirstWrite,
}) {
  const debounceRef = useRef(null);
  const latestTextRef = useRef("");
  const createdRef = useRef(false);
  const creatingRef = useRef(false);

  async function persist(cleanedText) {
    if (cleanedText.trim() === "") return;
    if (createdRef.current || creatingRef.current) return;

    creatingRef.current = true;
    try {
      const result = await invoke("write_virtual_stream_page", {
        streamName,
        dateName,
        text: cleanedText,
      });
      createdRef.current = true;
      onFirstWrite({
        pageId: `stream:${streamName}/${dateName}`,
        text: result.text,
        revision: result.revision,
      });
    } catch (error) {
      if (error?.code === "destination_page_exists") {
        createdRef.current = true;
        try {
          const content = await invoke("page_content", {
            pageId: `stream:${streamName}/${dateName}`,
          });
          onFirstWrite({
            pageId: `stream:${streamName}/${dateName}`,
            text: content.text,
            revision: content.revision,
          });
        } catch (contentError) {
          onError?.(contentError);
        }
      } else {
        onError?.(error);
      }
    } finally {
      creatingRef.current = false;
    }
  }

  useEffect(() => {
    flushRef.current = () => {
      clearTimeout(debounceRef.current);
      const cleaned = cleanEditorMarkdownForPersistence(latestTextRef.current);
      void persist(cleaned);
    };
    return () => {
      clearTimeout(debounceRef.current);
    };
  });

  onMarkdownUpdatedRef.current = (markdown) => {
    const cleaned = cleanEditorMarkdownForPersistence(markdown);
    latestTextRef.current = cleaned;
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      void persist(latestTextRef.current);
    }, WRITE_DEBOUNCE_MS);
  };
}
