import { useRef } from "react";
import MilkdownMarkdownEditor from "./MilkdownMarkdownEditor";
import { useEditorPersistence } from "../hooks/useEditorPersistence";
import { useStreamDocumentState } from "../hooks/useStreamDocumentState";
import { useVirtualStreamPersistence } from "../hooks/useVirtualStreamPersistence";

function BackedStreamEditor({ pageId, text, revision, pages, onNavigate, onConflict }) {
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
}) {
  const flushRef = useRef(null);
  const onMarkdownUpdatedRef = useRef(null);
  const editorGetRef = useRef(null);

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
    />
  );
}

export default function StreamSingleEditor({
  streamName,
  dateName,
  existingPageId,
  pages,
  onNavigate,
  onError,
  onRefresh,
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
    onError,
    onRefresh,
  });

  if (loading) {
    return <div className="stream-editor-placeholder">Loading...</div>;
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
    />
  );
}
