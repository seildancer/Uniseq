import { useRef } from "react";

export function useMarkdownEditorBridge() {
  const flushRef = useRef(null);
  const onMarkdownUpdatedRef = useRef(null);
  const editorGetRef = useRef(null);

  return {
    flushRef,
    onMarkdownUpdatedRef,
    editorGetRef,
    getEditor: () => editorGetRef.current?.(),
  };
}
