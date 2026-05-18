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
  const fallbackPageId = `stream:${streamName}/${dateName}`;
  const loadSeqRef = useRef(0);
  const loadedPageIdRef = useRef(null);
  const skipNextExistingPageHydrationRef = useRef(false);
  const [backedPageId, setBackedPageId] = useState(existingPageId ?? fallbackPageId);
  const [backedText, setBackedText] = useState("");
  const [backedRevision, setBackedRevision] = useState(null);
  const [loading, setLoading] = useState(Boolean(existingPageId));
  const shouldSkipFirstExistingPageHydration = (
    skipNextExistingPageHydrationRef.current
    && existingPageId === fallbackPageId
    && loadedPageIdRef.current === existingPageId
  );

  useEffect(() => {
    setBackedPageId(existingPageId ?? fallbackPageId);
    if (!existingPageId) {
      loadedPageIdRef.current = null;
      setBackedText("");
      setBackedRevision(null);
      setLoading(false);
      return;
    }
    if (shouldSkipFirstExistingPageHydration) {
      setLoading(false);
      return;
    }
    if (existingPageId !== loadedPageIdRef.current) {
      setBackedText("");
      setBackedRevision(null);
      setLoading(true);
    }
  }, [existingPageId, fallbackPageId, shouldSkipFirstExistingPageHydration]);

  useEffect(() => {
    if (!existingPageId) {
      setLoading(false);
      return;
    }
    if (shouldSkipFirstExistingPageHydration) {
      skipNextExistingPageHydrationRef.current = false;
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
          setBackedPageId(fallbackPageId);
          setBackedText("");
          setBackedRevision(null);
          return;
        }
        onError?.(error);
      });
  }, [
    backedPageId,
    existingPageId,
    fallbackPageId,
    reloadToken,
    onError,
    shouldSkipFirstExistingPageHydration,
  ]);

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
    handlePersisted: () => {
      if (!existingPageId && backedPageId === fallbackPageId) {
        loadedPageIdRef.current = fallbackPageId;
        skipNextExistingPageHydrationRef.current = true;
      }
      void onRefresh?.();
    },
  };
}
