import { replaceAll } from "@milkdown/utils";
import { toEditorMarkdown } from "../utils/imageMarkdown";

export function applyExternalEditorText({
  initializedRef,
  getEditor,
  nextText,
  latestTextRef,
  suppressWriteRef,
  workspaceRoot,
  clearPendingWrite,
}) {
  if (!initializedRef.current) {
    initializedRef.current = true;
    return;
  }

  const editor = getEditor();
  if (!editor || nextText === latestTextRef.current) {
    return;
  }

  suppressWriteRef.current = true;
  latestTextRef.current = nextText;
  editor.action(replaceAll(toEditorMarkdown(nextText, workspaceRoot)));
  clearPendingWrite?.();
  setTimeout(() => {
    suppressWriteRef.current = false;
  }, 0);
}
