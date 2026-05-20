import { useCallback, useState } from "react";
import MilkdownMarkdownEditor from "./components/MilkdownMarkdownEditor";
import { useMarkdownEditorBridge } from "./hooks/useMarkdownEditorBridge";
import { useBlockEditorPersistence } from "./hooks/useBlockEditorPersistence";

export default function BlockEditor({
  editorKey,
  entry,
  pages,
  onNavigate,
  onReload,
  onNotice,
  onFocusChange,
}) {
  const { flushRef, onMarkdownUpdatedRef, editorGetRef, getEditor } = useMarkdownEditorBridge();
  const [isFocused, setIsFocused] = useState(false);

  const handleConflict = useCallback(async () => {
    await onReload();
    onNotice?.("The source block changed while you were editing. Reloaded linked references.");
  }, [onNotice, onReload]);

  useBlockEditorPersistence({
    get: getEditor,
    blockHandle: entry.block.handle,
    text: entry.block.markdown,
    isFocused,
    flushRef,
    onMarkdownUpdatedRef,
    onConflict: handleConflict,
  });

  return (
    <div className="linked-ref-mini-editor">
      <MilkdownMarkdownEditor
        documentKey={editorKey}
        text={entry.block.markdown}
        pages={pages}
        onNavigate={onNavigate}
        flushRef={flushRef}
        onMarkdownUpdatedRef={onMarkdownUpdatedRef}
        editorGetRef={editorGetRef}
        onFocusChange={(focused) => {
          setIsFocused(focused);
          onFocusChange?.(focused);
        }}
        className="milkdown-editor milkdown-editor--linked-ref"
      />
    </div>
  );
}
