import { useRef } from "react";
import MilkdownMarkdownEditor from "./components/MilkdownMarkdownEditor";
import { useEditorPersistence } from "./hooks/useEditorPersistence";
function PageEditorInner({ pageId, text, revision, pages, onNavigate, flushRef, onConflict }) {
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

export default function MilkdownEditor({ pageId, text, revision, pages, onNavigate, onConflict, flushRef: externalFlushRef = null }) {
  const localFlushRef = useRef(null);
  const flushRef = externalFlushRef ?? localFlushRef;

  return (
    <PageEditorInner
      pageId={pageId}
      text={text}
      revision={revision}
      pages={pages}
      onNavigate={onNavigate}
      flushRef={flushRef}
      onConflict={onConflict}
    />
  );
}
