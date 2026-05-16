import MilkdownMarkdownEditor from "./components/MilkdownMarkdownEditor";
import { useMarkdownEditorBridge } from "./hooks/useMarkdownEditorBridge";
import { useEditorPersistence } from "./hooks/useEditorPersistence";
function PageEditorInner({ pageId, text, revision, pages, onNavigate, flushRef, onConflict }) {
  const { onMarkdownUpdatedRef, editorGetRef, getEditor } = useMarkdownEditorBridge();

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
    />
  );
}

export default function MilkdownEditor({ pageId, text, revision, pages, onNavigate, onConflict }) {
  const { flushRef } = useMarkdownEditorBridge();

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
