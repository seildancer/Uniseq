import { useRef } from "react";
import MilkdownMarkdownEditor from "./MilkdownMarkdownEditor";
import { useEditorPersistence } from "../hooks/useEditorPersistence";
import { useStreamDocumentState } from "../hooks/useStreamDocumentState";
import { useVirtualStreamPersistence } from "../hooks/useVirtualStreamPersistence";

function BackedStreamEditor({ pageId, text, revision, pages, onNavigate, onConflict, onFocusChange }) {
  const flushRef = useRef(null);
  const onMarkdownUpdatedRef = useRef(null);
  const editorGetRef = useRef(null);

  useEditorPersistence({
    get: () => editorGetRef.current?.(),
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
  onFocusChange,
}) {
  const flushRef = useRef(null);
  const onMarkdownUpdatedRef = useRef(null);
  const editorGetRef = useRef(null);

  useVirtualStreamPersistence({
    get: () => editorGetRef.current?.(),
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
  onFocusChange,
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

  if (loading) {
    return <div className="stream-editor-placeholder">Loading...</div>;
  }

  if (backedPageId) {
    return (
      <BackedStreamEditor
        key={backedPageId}
        pageId={backedPageId}
        text={backedText}
        revision={backedRevision}
        pages={pages}
        onNavigate={onNavigate}
        onConflict={handleConflictReload}
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
      onFocusChange={onFocusChange}
    />
  );
}
