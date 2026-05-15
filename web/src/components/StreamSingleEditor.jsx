import { useEffect } from "react";
import MilkdownMarkdownEditor from "./MilkdownMarkdownEditor";
import { useMarkdownEditorBridge } from "../hooks/useMarkdownEditorBridge";
import { useEditorPersistence } from "../hooks/useEditorPersistence";
import { useStreamDocumentState } from "../hooks/useStreamDocumentState";
import { useVirtualStreamPersistence } from "../hooks/useVirtualStreamPersistence";

function BackedStreamEditor({
  pageId,
  text,
  revision,
  pages,
  onNavigate,
  onConflict,
  focusEditorRef,
  onFocusChange,
}) {
  const { flushRef, onMarkdownUpdatedRef, editorGetRef, getEditor } = useMarkdownEditorBridge();

  useEditorPersistence({
    get: getEditor,
    text,
    revision,
    pageId,
    flushRef,
    onMarkdownUpdatedRef,
    onConflict,
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
      onFocusChange={onFocusChange}
    />
  );
}

function VirtualStreamEditor({
  streamName,
  dateName,
  pages,
  onNavigate,
  onError,
  onFirstWrite,
  focusEditorRef,
  onFocusChange,
}) {
  const { flushRef, onMarkdownUpdatedRef, editorGetRef } = useMarkdownEditorBridge();

  useVirtualStreamPersistence({
    streamName,
    dateName,
    flushRef,
    onMarkdownUpdatedRef,
    onError,
    onFirstWrite,
  });

  return (
    <MilkdownMarkdownEditor
      documentKey={`virtual:${streamName}/${dateName}`}
      text=""
      pages={pages}
      onNavigate={onNavigate}
      flushRef={flushRef}
      onMarkdownUpdatedRef={onMarkdownUpdatedRef}
      editorGetRef={editorGetRef}
      focusEditorRef={focusEditorRef}
      onFocusChange={onFocusChange}
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
    handleFirstWrite,
    virtualDocumentKey,
  } = useStreamDocumentState({
    streamName,
    dateName,
    existingPageId,
    reloadToken,
    onError,
    onRefresh,
  });

  useEffect(() => {
    onReadyChange?.(!loading);
  }, [loading, onReadyChange]);

  if (loading) {
    return null;
  }

  if (backedPageId) {
    const editorKey = backedRevision
      ? `${backedPageId}:${backedRevision.len_bytes}:${backedRevision.content_hash}`
      : backedPageId;
    return (
      <BackedStreamEditor
        key={editorKey}
        pageId={backedPageId}
        text={backedText}
        revision={backedRevision}
        pages={pages}
        onNavigate={onNavigate}
        onConflict={handleConflictReload}
        focusEditorRef={focusEditorRef}
        onFocusChange={onFocusChange}
      />
    );
  }

  return (
    <VirtualStreamEditor
      key={virtualDocumentKey}
      streamName={streamName}
      dateName={dateName}
      pages={pages}
      onNavigate={onNavigate}
      onError={onError}
      onFirstWrite={handleFirstWrite}
      focusEditorRef={focusEditorRef}
      onFocusChange={onFocusChange}
    />
  );
}
