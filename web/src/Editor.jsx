import { useState } from "react";
import MilkdownMarkdownEditor from "./components/MilkdownMarkdownEditor";
import { useMarkdownEditorBridge } from "./hooks/useMarkdownEditorBridge";
import { useEditorPersistence } from "./hooks/useEditorPersistence";

function PageEditorInner({
  pageId,
  text,
  revision,
  pages,
  onNavigate,
  flushRef,
  onConflict,
  onPersisted,
  focusEditorRef,
  onFocusChange,
}) {
  const { onMarkdownUpdatedRef, editorGetRef, getEditor } = useMarkdownEditorBridge();
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

export default function MilkdownEditor({
  pageId,
  text,
  revision,
  pages,
  onNavigate,
  onConflict,
  onPersisted,
  focusEditorRef = null,
  onFocusChange = null,
}) {
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
      onPersisted={onPersisted}
      focusEditorRef={focusEditorRef}
      onFocusChange={onFocusChange}
    />
  );
}
