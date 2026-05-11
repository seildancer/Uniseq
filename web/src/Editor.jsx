import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Editor, rootCtx, defaultValueCtx, editorViewCtx, prosePluginsCtx, remarkPluginsCtx, remarkStringifyOptionsCtx } from "@milkdown/core";
import { commonmark } from "@milkdown/preset-commonmark";
import { history } from "@milkdown/plugin-history";
import { listener, listenerCtx } from "@milkdown/plugin-listener";
import { Milkdown, MilkdownProvider, useEditor } from "@milkdown/react";
import { $prose, replaceAll } from "@milkdown/utils";
import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";
import { keymap } from "prosemirror-keymap";
import remarkBreaks from "remark-breaks";

const WRITE_DEBOUNCE_MS = 300;

function pageLeafName(pageId) {
  return pageId.replace(/^(?:pages|stream):/, "");
}

function detectTagTrigger(text) {
  const bracketMatch = text.match(/\[\[([^\]]*)$/);
  if (bracketMatch) {
    return { kind: "bracket", query: bracketMatch[1], triggerStart: bracketMatch.index };
  }
  const hashMatch = text.match(/#([a-zA-Z0-9_/.-]*)$/);
  if (hashMatch) {
    return { kind: "hash", query: hashMatch[1], triggerStart: hashMatch.index };
  }
  return null;
}

const blockHighlightKey = new PluginKey("blockHighlight");

const blockHighlightPlugin = $prose(() =>
  new Plugin({
    key: blockHighlightKey,
    props: {
      decorations(state) {
        const { selection } = state;
        const $from = selection.$from;
        for (let depth = $from.depth; depth > 0; depth--) {
          const node = $from.node(depth);
          const name = node.type.name;
          if (name === "list_item" || name === "paragraph" || name === "heading") {
            const pos = $from.before(depth);
            return DecorationSet.create(state.doc, [
              Decoration.node(pos, pos + node.nodeSize, { class: "milkdown-block--active" }),
            ]);
          }
        }
        return DecorationSet.empty;
      },
    },
  })
);

const wikilinkKey = new PluginKey("wikilinks");

function createEnterKeyPlugin() {
  return keymap({
    Enter(state, dispatch) {
      const { selection } = state;
      if (!selection.empty) return false;
      const { $from } = selection;

      // Only intercept in bare paragraphs (not inside list_item)
      if ($from.parent.type.name !== "paragraph") return false;
      for (let d = $from.depth - 1; d > 0; d--) {
        if ($from.node(d).type.name === "list_item") return false;
      }

      const nodeBefore = $from.nodeBefore;
      const atEnd = $from.parentOffset === $from.parent.content.size;

      if (nodeBefore?.type.name === "hardbreak" && atEnd) {
        const posBeforeHardbreak = $from.pos - nodeBefore.nodeSize;
        const nodeBeforeHardbreak = state.doc.resolve(posBeforeHardbreak).nodeBefore;

        if (nodeBeforeHardbreak?.type.name === "hardbreak") {
          // Triple-enter: split paragraph
          if (dispatch) {
            dispatch(state.tr.split($from.pos).scrollIntoView());
          }
          return true;
        }

        // Double-enter: insert another hardbreak
        const hardbreakType = state.schema.nodes.hardbreak;
        if (!hardbreakType) return false;
        if (dispatch) {
          dispatch(
            state.tr.replaceSelectionWith(hardbreakType.create()).scrollIntoView()
          );
        }
        return true;
      }

      // Single enter: insert hard_break (visual line break, same block)
      const hardbreakType = state.schema.nodes.hardbreak;
      if (!hardbreakType) return false;
      if (dispatch) {
        dispatch(
          state.tr.replaceSelectionWith(hardbreakType.create()).scrollIntoView()
        );
      }
      return true;
    },
  });
}

function createWikilinkPlugin(navigateRef, pagesRef) {
  return new Plugin({
    key: wikilinkKey,
    props: {
      decorations(state) {
        const decos = [];
        const { from, to } = state.selection;
        state.doc.descendants((node, pos) => {
          if (!node.isText) return;
          const text = node.text;
          // [[PageRef]] — hide brackets unless cursor is inside
          const bracketRegex = /\[\[([^\]]+)\]\]/g;
          let m;
          while ((m = bracketRegex.exec(text)) !== null) {
            const start = pos + m.index;
            const nameStart = start + 2;
            const nameEnd = nameStart + m[1].length;
            const closeEnd = nameEnd + 2;
            const cursorInside = from <= closeEnd && to >= start;
            if (!cursorInside) {
              decos.push(Decoration.inline(start, nameStart, { class: "wikilink-bracket" }));
              decos.push(Decoration.inline(nameEnd, closeEnd, { class: "wikilink-bracket" }));
            }
            decos.push(Decoration.inline(nameStart, nameEnd, { class: "tag-link tag-link-wiki" }));
          }
          // #tag — style the whole token, word-boundary only
          const hashRegex = /(?:^|(?<=\s))(#[a-zA-Z0-9_/.-]+)/g;
          while ((m = hashRegex.exec(text)) !== null) {
            decos.push(
              Decoration.inline(pos + m.index, pos + m.index + m[0].length, { class: "tag-link tag-link-hash" })
            );
          }
        });
        return DecorationSet.create(state.doc, decos);
      },
      handleDOMEvents: {
        click(view, event) {
          const target = event.target;
          // External hyperlinks — open in system browser
          const linkEl = target?.closest?.("a[href]");
          if (linkEl) {
            const href = linkEl.getAttribute("href");
            if (href && /^https?:\/\//.test(href)) {
              event.preventDefault();
              invoke("open_url", { url: href });
              return true;
            }
          }
          return false;
        },
      },
      handleClick(view, pos, event) {
        const target = event.target;
        const tagEl = target?.closest?.(".tag-link");
        if (!tagEl) return false;
        const raw = tagEl.textContent;
        let pageName;
        if (tagEl.classList.contains("tag-link-hash")) {
          pageName = raw.startsWith("#") ? raw.slice(1) : raw;
        } else {
          pageName = raw; // brackets already hidden; textContent is the name
        }
        if (!pageName) return false;
        const allPages = pagesRef.current ?? [];
        const found = allPages.find(
          (p) => pageLeafName(p.page_id) === pageName || p.page_id === pageName || p.title === pageName
        );
        if (found) {
          navigateRef.current?.(found.page_id);
        } else {
          const pageId = `pages:${pageName}`;
          invoke("create_page", { pageId })
            .then(() => navigateRef.current?.(pageId))
            .catch(console.error);
        }
        return false;
      },
    },
  });
}

function MilkdownEditorInner({ pageId, text, pages, onNavigate, flushRef }) {
  const [autocomplete, setAutocomplete] = useState(null);
  const activeItemRef = useRef(null);
  const debounceRef = useRef(null);
  const latestTextRef = useRef(text);
  const suppressWriteRef = useRef(false);
  const initializedRef = useRef(false);
  const navigateRef = useRef(onNavigate);
  const pagesRef = useRef(pages);
  navigateRef.current = onNavigate;
  pagesRef.current = pages;

  const { get } = useEditor((root) =>
    Editor.make()
      .config((ctx) => {
        ctx.set(rootCtx, root);
        ctx.set(defaultValueCtx, text);
        ctx.update(remarkStringifyOptionsCtx, (opts) => ({ ...opts, bullet: "-" }));
        ctx.update(remarkPluginsCtx, (plugins) => [
          ...plugins,
          { plugin: remarkBreaks, options: {} },
        ]);
        ctx.update(prosePluginsCtx, (plugins) => [
          createEnterKeyPlugin(),
          ...plugins,
          createWikilinkPlugin(navigateRef, pagesRef),
        ]);
        ctx.get(listenerCtx).markdownUpdated((_ctx, markdown) => {
          const cleaned = markdown
            .replace(/\\\n/g, "\n")
            .replace(/<br\s*\/?>/g, "")
            .replace(/\n{4,}/g, "\n\n\n");
          latestTextRef.current = cleaned;
          if (suppressWriteRef.current) return;
          clearTimeout(debounceRef.current);
          debounceRef.current = setTimeout(() => {
            invoke("write_page_content", { pageId, text: cleaned }).catch(() => {});
          }, WRITE_DEBOUNCE_MS);
        });
      })
      .use(commonmark)
      .use(listener)
      .use(history)
      .use(blockHighlightPlugin)
  );

  useEffect(() => {
    flushRef.current = () => {
      clearTimeout(debounceRef.current);
      invoke("write_page_content", { pageId, text: latestTextRef.current }).catch(() => {});
    };
  });

  // Reload editor content when an external file change arrives on the same page.
  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      return;
    }
    const editor = get();
    if (!editor) return;
    suppressWriteRef.current = true;
    editor.action(replaceAll(text));
    clearTimeout(debounceRef.current);
    setTimeout(() => { suppressWriteRef.current = false; }, 0);
  }, [text]); // eslint-disable-line react-hooks/exhaustive-deps

  function getBlockInfo() {
    const editor = get();
    if (!editor) return null;
    const view = editor.action((ctx) => ctx.get(editorViewCtx));
    const { from } = view.state.selection;
    const $from = view.state.selection.$from;
    const blockStart = $from.start($from.depth);
    const textBefore = view.state.doc.textBetween(blockStart, from, "\n");
    return { view, from, blockStart, textBefore };
  }

  function checkAutocomplete() {
    const info = getBlockInfo();
    if (!info) return;
    const { view, from, blockStart, textBefore } = info;
    const trigger = detectTagTrigger(textBefore);
    if (!trigger) { setAutocomplete(null); return; }
    const q = trigger.query.toLowerCase();
    const suggestions = pages
      .filter((p) => {
        const id = p.page_id.toLowerCase();
        const title = (p.title || "").toLowerCase();
        const leaf = pageLeafName(p.page_id).toLowerCase();
        return id.includes(q) || title.includes(q) || leaf.includes(q);
      })
      .slice(0, 8);
    const createName = suggestions.length === 0 && trigger.query.length > 0 ? trigger.query : null;
    if (!suggestions.length && !createName) { setAutocomplete(null); return; }
    const coords = view.coordsAtPos(from);
    setAutocomplete((prev) => {
      const maxIdx = suggestions.length - 1 + (createName ? 1 : 0);
      const sameStart = prev?.blockStart === blockStart && prev?.triggerStart === trigger.triggerStart;
      return {
        trigger, suggestions, createName, blockStart, triggerStart: trigger.triggerStart,
        activeIdx: sameStart ? Math.min(prev.activeIdx, maxIdx) : 0,
        coords: { top: coords.bottom, left: coords.left },
      };
    });
  }

  function applyAutocomplete(page) {
    const info = getBlockInfo();
    if (!info) return;
    const { view, from, blockStart, textBefore } = info;
    const trigger = detectTagTrigger(textBefore);
    if (!trigger) return;
    const name = page ? pageLeafName(page.page_id) : autocomplete.createName;
    if (!page && autocomplete.createName) {
      invoke("create_page", { pageId: `pages:${autocomplete.createName}` }).catch(console.error);
    }
    const replacement = trigger.kind === "bracket" ? `[[${name}]]` : `#${name}`;
    view.dispatch(
      view.state.tr.replaceWith(blockStart + trigger.triggerStart, from, view.state.schema.text(replacement))
    );
    view.focus();
    setAutocomplete(null);
  }

  useEffect(() => {
    activeItemRef.current?.scrollIntoView({ block: "nearest" });
  }, [autocomplete?.activeIdx]);

  return (
    <div
      className="milkdown-editor"
      onKeyDownCapture={(e) => {
        if (!autocomplete) return;
        if (e.key === "ArrowDown") {
          e.preventDefault();
          e.stopPropagation();
          setAutocomplete((prev) => ({
            ...prev,
            activeIdx: Math.min(prev.activeIdx + 1, prev.suggestions.length - 1 + (prev.createName ? 1 : 0)),
          }));
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          e.stopPropagation();
          setAutocomplete((prev) => ({ ...prev, activeIdx: Math.max(prev.activeIdx - 1, 0) }));
        } else if ((e.key === "Enter" && !e.shiftKey) || e.key === "Tab") {
          e.preventDefault();
          e.stopPropagation();
          const isCreate = autocomplete.activeIdx === autocomplete.suggestions.length;
          applyAutocomplete(isCreate ? null : autocomplete.suggestions[autocomplete.activeIdx]);
        } else if (e.key === "Escape") {
          e.preventDefault();
          e.stopPropagation();
          setAutocomplete(null);
        }
      }}
      onKeyUp={(e) => {
        if (autocomplete && ["ArrowUp", "ArrowDown", "Escape", "Enter", "Tab"].includes(e.key)) return;
        checkAutocomplete();
      }}
    >
      <Milkdown />
      {autocomplete && (
        <ul
          className="autocomplete-dropdown"
          role="listbox"
          style={{ position: "fixed", top: autocomplete.coords.top + 4, left: autocomplete.coords.left }}
        >
          {autocomplete.suggestions.map((page, i) => (
            <li
              key={page.page_id}
              ref={i === autocomplete.activeIdx ? activeItemRef : null}
              className={`autocomplete-item${i === autocomplete.activeIdx ? " autocomplete-item--active" : ""}`}
              role="option"
              onMouseDown={(e) => { e.preventDefault(); applyAutocomplete(page); }}
            >
              <span className="autocomplete-item-title">{page.title || pageLeafName(page.page_id)}</span>
              <span className="autocomplete-item-id">{pageLeafName(page.page_id)}</span>
            </li>
          ))}
          {autocomplete.createName && (
            <li
              ref={autocomplete.activeIdx === autocomplete.suggestions.length ? activeItemRef : null}
              className={`autocomplete-item autocomplete-item--create${autocomplete.activeIdx === autocomplete.suggestions.length ? " autocomplete-item--active" : ""}`}
              role="option"
              onMouseDown={(e) => { e.preventDefault(); applyAutocomplete(null); }}
            >
              <span className="autocomplete-item-title">+ Create "{autocomplete.createName}"</span>
            </li>
          )}
        </ul>
      )}
    </div>
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
