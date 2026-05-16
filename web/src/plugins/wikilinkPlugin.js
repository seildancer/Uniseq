import { Plugin, PluginKey } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";
import { invoke } from "@tauri-apps/api/core";
import pageLeafName from "../utils/pageLeafName";

const wikilinkKey = new PluginKey("wikilinks");

let hasFocus = false;

export function resetWikilinkFocus() {
  hasFocus = false;
}

function isCodeTextNode(node, parent) {
  return node.marks?.some((mark) => mark.type.name === "code") || parent?.type?.name === "code_block";
}

export default function createWikilinkPlugin(navigateRef, pagesRef) {
  return new Plugin({
    key: wikilinkKey,
    props: {
      decorations(state) {
        const decos = [];
        const { from, to } = state.selection;
        state.doc.descendants((node, pos, parent) => {
          if (!node.isText) return;
          if (isCodeTextNode(node, parent)) return;
          const text = node.text;
          // [[PageRef]] — hide brackets unless cursor is inside
          const bracketRegex = /\[\[([^\]]+)\]\]/g;
          let m;
          while ((m = bracketRegex.exec(text)) !== null) {
            const start = pos + m.index;
            const nameStart = start + 2;
            const nameEnd = nameStart + m[1].length;
            const closeEnd = nameEnd + 2;
            const cursorInside = hasFocus && from <= closeEnd && to >= start;
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
        focus: () => {
          hasFocus = true;
          return false;
        },
        blur: () => {
          hasFocus = false;
          return false;
        },
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
