import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function useStreamDocumentState({
  streamName,
  dateName,
  existingPageId,
  reloadToken,
  onError,
  onRefresh,
}) {
  const loadSeqRef = useRef(0);
  const loadedPageIdRef = useRef(null);
  const [backedPageId, setBackedPageId] = useState(existingPageId);
  const [backedText, setBackedText] = useState("");
  const [backedRevision, setBackedRevision] = useState(null);
  const [loading, setLoading] = useState(Boolean(existingPageId));

  useEffect(() => {
    setBackedPageId(existingPageId ?? null);
    if (!existingPageId || existingPageId !== loadedPageIdRef.current) {
      setBackedText("");
      setBackedRevision(null);
    }
    if (existingPageId && existingPageId !== loadedPageIdRef.current) {
      setLoading(true);
    }
    if (!existingPageId) {
      loadedPageIdRef.current = null;
      setLoading(false);
    }
  }, [existingPageId]);

  useEffect(() => {
    if (!backedPageId) {
      setLoading(false);
      return;
    }

    const isColdLoad = loadedPageIdRef.current !== backedPageId;
    if (isColdLoad) {
      setLoading(true);
    }
    const seq = ++loadSeqRef.current;
    invoke("page_content", { pageId: backedPageId })
      .then((result) => {
        if (seq !== loadSeqRef.current) {
          return;
        }
        loadedPageIdRef.current = backedPageId;
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
          loadedPageIdRef.current = null;
          setBackedPageId(null);
          return;
        }
        onError?.(error);
      });
  }, [backedPageId, reloadToken, onError]);

  function handleFirstWrite({ pageId, text, revision }) {
    loadedPageIdRef.current = pageId;
    setBackedPageId(pageId);
    setBackedText(text);
    setBackedRevision(revision);
    setLoading(false);
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
