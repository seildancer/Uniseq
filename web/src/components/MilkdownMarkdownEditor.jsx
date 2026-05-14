import { useEffect, useRef } from "react";
import { Editor, rootCtx, defaultValueCtx, prosePluginsCtx, remarkStringifyOptionsCtx } from "@milkdown/core";
import { commonmark } from "@milkdown/preset-commonmark";
import { gfm } from "@milkdown/preset-gfm";
import { history } from "@milkdown/plugin-history";
import { listener, listenerCtx } from "@milkdown/plugin-listener";
import { Milkdown, MilkdownProvider, useEditor } from "@milkdown/react";
import { $remark } from "@milkdown/utils";
import breaks from "remark-breaks";

import createBackspacePlugin from "../plugins/backspacePlugin";
import createDeleteKeyPlugin from "../plugins/deleteKeyPlugin";
import createIndentPlugin from "../plugins/indentPlugin";
import createWikilinkPlugin, { resetWikilinkFocus } from "../plugins/wikilinkPlugin";
import blockHighlightPlugin, { resetBlockHighlightFocus } from "../plugins/blockHighlightPlugin";
import taskListClickPlugin from "../plugins/taskListClickPlugin";
import AutocompleteEditor from "./Autocomplete";

const remarkBreaks = $remark("remarkBreaks", () => breaks);

function MilkdownMarkdownEditorInner({
  documentKey,
  text,
  pages,
  onNavigate,
  onMarkdownUpdatedRef,
  className,
  editorGetRef,
  onFocusChange,
}) {
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
        ctx.set(defaultValueCtx, text);
        ctx.update(remarkStringifyOptionsCtx, (opts) => ({ ...opts, bullet: "-" }));

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
      .use(remarkBreaks.plugin)
  );

  if (editorGetRef) {
    editorGetRef.current = get;
  }

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
  onFocusChange = null,
}) {
  useEffect(() => {
    return () => {
      void flushRef.current?.();
      flushRef.current = null;
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

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
        onFocusChange={onFocusChange}
      />
    </MilkdownProvider>
  );
}
