import { useContext, useEffect, useRef } from "react";
import { Editor, rootCtx, defaultValueCtx, editorViewCtx, editorViewOptionsCtx, prosePluginsCtx, remarkPluginsCtx, remarkStringifyOptionsCtx } from "@milkdown/core";
import { commonmark } from "@milkdown/preset-commonmark";
import { gfm } from "@milkdown/preset-gfm";
import { history } from "@milkdown/plugin-history";
import { listener, listenerCtx } from "@milkdown/plugin-listener";
import { Milkdown, MilkdownProvider, useEditor } from "@milkdown/react";
import { TextSelection } from "prosemirror-state";
import breaks from "remark-breaks";

import createBackspacePlugin from "../plugins/backspacePlugin";
import createDeleteKeyPlugin from "../plugins/deleteKeyPlugin";
import createIndentPlugin from "../plugins/indentPlugin";
import createWikilinkPlugin, { resetWikilinkFocus } from "../plugins/wikilinkPlugin";
import blockHighlightPlugin, { resetBlockHighlightFocus } from "../plugins/blockHighlightPlugin";
import imageResizePlugin from "../plugins/imageResizePlugin";
import taskListClickPlugin from "../plugins/taskListClickPlugin";
import AutocompleteEditor from "./Autocomplete";
import { WorkspaceContext } from "../WorkspaceContext.js";
import { toEditorMarkdown } from "../utils/imageMarkdown.js";

function MilkdownMarkdownEditorInner({
  documentKey,
  text,
  pages,
  onNavigate,
  onMarkdownUpdatedRef,
  className,
  editorGetRef,
  focusEditorRef,
  onFocusChange,
}) {
  const workspaceRoot = useContext(WorkspaceContext);
  const navigateRef = useRef(onNavigate);
  const pagesRef = useRef(pages);
  navigateRef.current = onNavigate;
  pagesRef.current = pages;

  useEffect(() => {
    resetWikilinkFocus();
    resetBlockHighlightFocus();
  }, [documentKey]);

  const { get } = useEditor((root) =>
    Editor.make()
      .config((ctx) => {
        ctx.set(rootCtx, root);
        ctx.set(defaultValueCtx, toEditorMarkdown(text, workspaceRoot));
        ctx.update(remarkStringifyOptionsCtx, (opts) => ({ ...opts, bullet: "-" }));
        ctx.update(editorViewOptionsCtx, (opts) => ({ ...opts, attributes: { spellcheck: "false" } }));

        ctx.update(remarkPluginsCtx, (plugins) => [...plugins, breaks]);
        ctx.update(prosePluginsCtx, (plugins) => [
          createBackspacePlugin(),
          createDeleteKeyPlugin(),
          createIndentPlugin(),
          taskListClickPlugin(),
          ...plugins,
          createWikilinkPlugin(navigateRef, pagesRef),
        ]);
        ctx.get(listenerCtx).markdownUpdated((_ctx, markdown) => {
          onMarkdownUpdatedRef.current?.(markdown);
        });
      })
      .use(commonmark)
      .use(gfm)
      .use(listener)
      .use(history)
      .use(blockHighlightPlugin)
      .use(imageResizePlugin)
  );

  if (editorGetRef) {
    editorGetRef.current = get;
  }

  useEffect(() => {
    if (!focusEditorRef) {
      return undefined;
    }

    const focusEditor = ({ atEnd = true, point = null } = {}) => {
      const editor = get();
      if (!editor) {
        return false;
      }

      const view = editor.action((ctx) => ctx.get(editorViewCtx));
      if (point) {
        const rect = view.dom.getBoundingClientRect();
        const left = rect.left + (rect.width * point.xRatio);
        const top = rect.top + (rect.height * point.yRatio);
        const target = view.posAtCoords({ left, top });
        if (target?.pos != null) {
          view.dispatch(
            view.state.tr
              .setSelection(TextSelection.create(view.state.doc, target.pos))
              .scrollIntoView(),
          );
        } else if (atEnd) {
          view.dispatch(
            view.state.tr
              .setSelection(TextSelection.atEnd(view.state.doc))
              .scrollIntoView(),
          );
        }
      } else if (atEnd) {
        view.dispatch(
          view.state.tr
            .setSelection(TextSelection.atEnd(view.state.doc))
            .scrollIntoView(),
        );
      }
      view.focus();
      return true;
    };

    focusEditorRef.current = focusEditor;
    return () => {
      if (focusEditorRef.current === focusEditor) {
        focusEditorRef.current = null;
      }
    };
  }, [focusEditorRef, get]);

  return (
    <AutocompleteEditor get={get} pages={pages} className={className} onFocusChange={onFocusChange}>
      <Milkdown />
    </AutocompleteEditor>
  );
}

export default function MilkdownMarkdownEditor({
  documentKey,
  text,
  pages,
  onNavigate,
  flushRef,
  onMarkdownUpdatedRef,
  className = "milkdown-editor",
  editorGetRef = null,
  focusEditorRef = null,
  onFocusChange = null,
}) {
  useEffect(() => {
    return () => { flushRef.current?.(); };
  }, [flushRef]);

  return (
    <MilkdownProvider>
      <MilkdownMarkdownEditorInner
        documentKey={documentKey}
        text={text}
        pages={pages}
        onNavigate={onNavigate}
        onMarkdownUpdatedRef={onMarkdownUpdatedRef}
        className={className}
        editorGetRef={editorGetRef}
        focusEditorRef={focusEditorRef}
        onFocusChange={onFocusChange}
      />
    </MilkdownProvider>
  );
}
