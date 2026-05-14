import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function useStreamDocumentState({
  streamName,
  dateName,
  existingPageId,
  onError,
  onRefresh,
}) {
  const loadSeqRef = useRef(0);
  const [backedPageId, setBackedPageId] = useState(existingPageId);
  const [backedText, setBackedText] = useState("");
  const [backedRevision, setBackedRevision] = useState(null);
  const [loading, setLoading] = useState(Boolean(existingPageId));

  useEffect(() => {
    setBackedPageId(existingPageId ?? null);
    if (!existingPageId) {
      setBackedText("");
      setBackedRevision(null);
    }
  }, [existingPageId]);

  useEffect(() => {
    if (!backedPageId) {
      setLoading(false);
      return;
    }

    setLoading(true);
    const seq = ++loadSeqRef.current;
    invoke("page_content", { pageId: backedPageId })
      .then((result) => {
        if (seq !== loadSeqRef.current) {
          return;
        }
        setBackedText(result.text);
        setBackedRevision(result.revision);
        setLoading(false);
      })
      .catch((error) => {
        if (seq !== loadSeqRef.current) {
          return;
        }
        setLoading(false);
        if (error?.code === "missing_page") {
          setBackedPageId(null);
          return;
        }
        onError?.(error);
      });
  }, [backedPageId, onError]);

  function handleFirstWrite({ pageId, text, revision }) {
    setBackedPageId(pageId);
    setBackedText(text);
    setBackedRevision(revision);
    void onRefresh?.();
  }

  async function handleConflictReload() {
    if (!backedPageId) {
      return;
    }

    try {
      const result = await invoke("page_content", { pageId: backedPageId });
      setBackedText(result.text);
      setBackedRevision(result.revision);
    } catch (error) {
      onError?.(error);
    }
  }

  return {
    backedPageId,
    backedRevision,
    backedText,
    loading,
    handleConflictReload,
    handleFirstWrite,
    virtualDocumentKey: `virtual:${streamName}/${dateName}`,
  };
}
