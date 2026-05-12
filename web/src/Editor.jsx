import { useEffect, useRef } from "react";
import { Editor, rootCtx, defaultValueCtx, prosePluginsCtx, remarkStringifyOptionsCtx } from "@milkdown/core";
import { commonmark } from "@milkdown/preset-commonmark";
import { gfm } from "@milkdown/preset-gfm";
import { history } from "@milkdown/plugin-history";
import { listener, listenerCtx } from "@milkdown/plugin-listener";
import { Milkdown, MilkdownProvider, useEditor } from "@milkdown/react";
import { $remark } from "@milkdown/utils";
import breaks from "remark-breaks";

import createBackspacePlugin from "./plugins/backspacePlugin";
import createDeleteKeyPlugin from "./plugins/deleteKeyPlugin";
import createIndentPlugin from "./plugins/indentPlugin";
import createWikilinkPlugin, { resetWikilinkFocus } from "./plugins/wikilinkPlugin";
import blockHighlightPlugin, { resetBlockHighlightFocus } from "./plugins/blockHighlightPlugin";
import taskListClickPlugin from "./plugins/taskListClickPlugin";
import { useEditorPersistence } from "./hooks/useEditorPersistence";
import AutocompleteEditor from "./components/Autocomplete";

const remarkBreaks = $remark("remarkBreaks", () => breaks);

function MilkdownEditorInner({ pageId, text, pages, onNavigate, flushRef }) {
  const navigateRef = useRef(onNavigate);
  const pagesRef = useRef(pages);
  const onMarkdownUpdatedRef = useRef(null);
  navigateRef.current = onNavigate;
  pagesRef.current = pages;

  useEffect(() => {
    resetWikilinkFocus();
    resetBlockHighlightFocus();
  }, [pageId]);

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

  useEditorPersistence({ get, pageId, text, flushRef, onMarkdownUpdatedRef });

  return (
    <AutocompleteEditor get={get} pages={pages}>
      <Milkdown />
    </AutocompleteEditor>
  );
}

export default function MilkdownEditor({ pageId, text, pages, onNavigate }) {
  const flushRef = useRef(null);

  useEffect(() => {
    return () => { flushRef.current?.(); };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <MilkdownProvider>
      <MilkdownEditorInner
        pageId={pageId}
        text={text}
        pages={pages}
        onNavigate={onNavigate}
        flushRef={flushRef}
      />
    </MilkdownProvider>
  );
}
