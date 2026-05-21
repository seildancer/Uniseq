import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { editorViewCtx } from "@milkdown/core";
import { invoke } from "@tauri-apps/api/core";
import pageLeafName from "../utils/pageLeafName";
import { pageRefBody, pageRefLabel } from "../utils/pageRefs.js";
import { isSelectionInsideCode } from "../utils/autocompleteCodeGuards.js";

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

export default function AutocompleteEditor({
  get,
  pages,
  children,
  className = "milkdown-editor",
  onFocusChange = null,
}) {
  const [autocomplete, setAutocomplete] = useState(null);
  const activeItemRef = useRef(null);
  const suppressNextCheckRef = useRef(false);
  const rootRef = useRef(null);
  const dropdownRef = useRef(null);

  function clearAutocomplete() {
    setAutocomplete(null);
  }

  function getBlockInfo() {
    const editor = get();
    if (!editor) return null;
    const view = editor.action((ctx) => ctx.get(editorViewCtx));
    if (isSelectionInsideCode(view.state)) return null;
    const { from } = view.state.selection;
    const $from = view.state.selection.$from;
    const blockStart = $from.start($from.depth);
    const textBefore = view.state.doc.textBetween(blockStart, from, "");
    return { view, from, blockStart, textBefore };
  }

  function checkAutocomplete() {
    const info = getBlockInfo();
    if (!info) { setAutocomplete(null); return; }
    const { view, from, blockStart, textBefore } = info;
    const trigger = detectTagTrigger(textBefore);
    if (!trigger) { setAutocomplete(null); return; }
    const q = trigger.query.toLowerCase();
    const suggestions = pages
      .filter((p) => {
        const refBody = pageRefBody(p.page_id).toLowerCase();
        const title = (p.title || "").toLowerCase();
        const leaf = pageLeafName(p.page_id).toLowerCase();
        return refBody.includes(q) || title.includes(q) || leaf.includes(q);
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
    const name = page ? pageRefBody(page.page_id) : autocomplete.createName;
    if (!page && autocomplete.createName) {
      invoke("create_page", { pageId: `pages:${autocomplete.createName}` }).catch(console.error);
    }
    const replacement = trigger.kind === "bracket" ? `[[${name}]]` : `#${name}`;
    view.dispatch(
      view.state.tr.replaceWith(blockStart + trigger.triggerStart, from, view.state.schema.text(replacement))
    );
    view.focus();
    setAutocomplete(null);
    suppressNextCheckRef.current = true;
  }

  function focusEditorFromChrome(event) {
    if (event.target.closest?.(".ProseMirror")) {
      return;
    }
    const editor = get();
    if (!editor) {
      return;
    }
    const view = editor.action((ctx) => ctx.get(editorViewCtx));
    view.focus();
  }

  useEffect(() => {
    activeItemRef.current?.scrollIntoView({ block: "nearest" });
  }, [autocomplete?.activeIdx]);

  useEffect(() => {
    if (!autocomplete) {
      return undefined;
    }

    function handlePointerDown(event) {
      const target = event.target;
      if (rootRef.current?.contains(target) || dropdownRef.current?.contains(target)) {
        return;
      }
      clearAutocomplete();
    }

    document.addEventListener("pointerdown", handlePointerDown, true);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown, true);
    };
  }, [autocomplete]);

  return (
    <div
      ref={rootRef}
      className={className}
      onMouseDown={focusEditorFromChrome}
      onFocusCapture={() => onFocusChange?.(true)}
      onBlurCapture={(e) => {
        if (
          e.currentTarget.contains(e.relatedTarget)
          || dropdownRef.current?.contains(e.relatedTarget)
        ) {
          return;
        }
        clearAutocomplete();
        onFocusChange?.(false);
      }}
      onMouseUpCapture={() => {
        requestAnimationFrame(checkAutocomplete);
      }}
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
          suppressNextCheckRef.current = true;
        }
      }}
      onKeyUp={(e) => {
        if (autocomplete && ["ArrowUp", "ArrowDown", "Escape", "Enter", "Tab"].includes(e.key)) return;
        if (suppressNextCheckRef.current) {
          suppressNextCheckRef.current = false;
          return;
        }
        checkAutocomplete();
      }}
    >
      {children}
      {autocomplete && createPortal(
        <ul
          ref={dropdownRef}
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
              <span className="autocomplete-item-title">{pageRefLabel(page)}</span>
              {page.title && page.title !== pageRefLabel(page) ? (
                <span className="autocomplete-item-id">{page.title}</span>
              ) : null}
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
        </ul>,
        document.body,
      )}
    </div>
  );
}
