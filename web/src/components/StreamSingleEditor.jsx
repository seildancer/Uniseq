import { useEffect, useRef, useState } from "react";
import MilkdownMarkdownEditor from "./MilkdownMarkdownEditor";
import { useMarkdownEditorBridge } from "../hooks/useMarkdownEditorBridge";
import { useEditorPersistence } from "../hooks/useEditorPersistence";
import { useStreamDocumentState } from "../hooks/useStreamDocumentState";

function BackedStreamEditor({
  pageId,
  text,
  revision,
  pages,
  onNavigate,
  onConflict,
  onPersisted,
  focusEditorRef,
  onFocusChange,
}) {
  const { flushRef, onMarkdownUpdatedRef, editorGetRef, getEditor } = useMarkdownEditorBridge();
  const [isFocused, setIsFocused] = useState(false);

  useEditorPersistence({
    get: getEditor,
    text,
    revision,
    pageId,
    isFocused,
    flushRef,
    onMarkdownUpdatedRef,
    onConflict,
    onPersisted,
  });

  return (
    <MilkdownMarkdownEditor
      documentKey={pageId}
      text={text}
      pages={pages}
      onNavigate={onNavigate}
      flushRef={flushRef}
      onMarkdownUpdatedRef={onMarkdownUpdatedRef}
      editorGetRef={editorGetRef}
      focusEditorRef={focusEditorRef}
      onFocusChange={(focused) => {
        setIsFocused(focused);
        onFocusChange?.(focused);
      }}
    />
  );
}

export default function StreamSingleEditor({
  streamName,
  dateName,
  existingPageId,
  pages,
  reloadToken,
  onNavigate,
  onError,
  onRefresh,
  focusEditorRef,
  onFocusChange,
  onReadyChange,
}) {
  const {
    backedPageId,
    backedRevision,
    backedText,
    loading,
    handleConflictReload,
    handlePersisted,
  } = useStreamDocumentState({
    streamName,
    dateName,
    existingPageId,
    reloadToken,
    onError,
    onRefresh,
  });

  const onReadyChangeRef = useRef(onReadyChange);
  useEffect(() => {
    onReadyChangeRef.current = onReadyChange;
  }, [onReadyChange]);

  useEffect(() => {
    onReadyChangeRef.current?.(!loading);
  }, [loading]);

  const createdRefreshSentRef = useRef(Boolean(existingPageId));
  useEffect(() => {
    createdRefreshSentRef.current = Boolean(existingPageId);
  }, [existingPageId, streamName, dateName]);

  if (loading) {
    return null;
  }

  return (
    <BackedStreamEditor
      key={backedPageId}
      pageId={backedPageId}
      text={backedText}
      revision={backedRevision}
      pages={pages}
      onNavigate={onNavigate}
      onConflict={handleConflictReload}
      onPersisted={() => {
        if (createdRefreshSentRef.current) {
          return;
        }
        createdRefreshSentRef.current = true;
        handlePersisted();
      }}
      focusEditorRef={focusEditorRef}
      onFocusChange={onFocusChange}
    />
  );
}
