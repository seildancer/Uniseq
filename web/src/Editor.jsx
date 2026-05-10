import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";

const WRITE_DEBOUNCE_MS = 300;

// ── Helpers ────────────────────────────────────────────────────────────────

function blocksToText(blocks) {
  return blocks
    .map((b) => (b.kind === "outliner" ? "\t".repeat(b.depth) + "- " : "") + b.content + "\n")
    .join("");
}

function adjustHeight(el) {
  el.style.height = "auto";
  el.style.height = el.scrollHeight + "px";
}

function pageLeafName(pageId) {
  return pageId.replace(/^(?:pages|stream):/, '');
}

function preprocessTagsForRender(content) {
  return content
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/\[\[([^\]]+)\]\]/g, '[$1](PAGE:$1)')
    .replace(/#([a-zA-Z0-9_/.-]+)/g, '[#$1](PAGE:$1)');
}

function detectTagTrigger(text, cursorPos) {
  const segment = text.slice(Math.max(0, cursorPos - 50), cursorPos);
  const bracketMatch = segment.match(/\[\[([^\]]*)$/);
  if (bracketMatch) {
    return {
      kind: 'bracket',
      query: bracketMatch[1],
      triggerStart: Math.max(0, cursorPos - 50) + bracketMatch.index,
    };
  }
  const hashMatch = segment.match(/#([a-zA-Z0-9_/.-]*)$/);
  if (hashMatch) {
    return {
      kind: 'hash',
      query: hashMatch[1],
      triggerStart: Math.max(0, cursorPos - 50) + hashMatch.index,
    };
  }
  return null;
}

// ── BlockRow ───────────────────────────────────────────────────────────────

function BlockRow({ block, idx, isFocused, onFocus, onContentChange, onKeyDown, pages, onNavigate }) {
  const textareaRef = useRef(null);
  const isOutliner = block.kind === "outliner";

  const [autocomplete, setAutocomplete] = useState(null);
  const activeItemRef = useRef(null);

  useEffect(() => {
    if (isFocused && textareaRef.current) {
      const el = textareaRef.current;
      el.focus();
      const len = el.value.length;
      el.setSelectionRange(len, len);
      adjustHeight(el);
    }
  }, [isFocused]);

  useEffect(() => {
    activeItemRef.current?.scrollIntoView({ block: 'nearest' });
  }, [autocomplete?.activeIdx]);

  function updateAutocomplete(value, cursorPos) {
    const trigger = detectTagTrigger(value, cursorPos);
    if (!trigger) { setAutocomplete(null); return; }
    const q = trigger.query.toLowerCase();
    const suggestions = pages
      .filter(p => {
        const id = p.page_id.toLowerCase();
        const title = (p.title || '').toLowerCase();
        const leaf = pageLeafName(p.page_id).toLowerCase();
        return id.includes(q) || title.includes(q) || leaf.includes(q);
      })
      .slice(0, 8);
    const createName = suggestions.length === 0 && trigger.query.length > 0 ? trigger.query : null;
    if (!suggestions.length && !createName) { setAutocomplete(null); return; }
    setAutocomplete(prev => {
      const sameStart = prev?.trigger.triggerStart === trigger.triggerStart;
      const maxIdx = suggestions.length - 1 + (createName ? 1 : 0);
      const prevIdx = sameStart ? prev.activeIdx : 0;
      return { trigger, suggestions, createName, activeIdx: Math.min(prevIdx, maxIdx) };
    });
  }

  function applyAutocomplete(page) {
    const { trigger, createName } = autocomplete;
    const name = page ? pageLeafName(page.page_id) : createName;
    if (!page && createName) {
      invoke("create_page", { pageId: `pages:${createName}` }).catch(console.error);
    }
    const cursorPos = textareaRef.current?.selectionStart ?? block.content.length;
    const replacement = trigger.kind === 'bracket' ? `[[${name}]]` : `#${name}`;
    const newContent =
      block.content.slice(0, trigger.triggerStart) +
      replacement +
      block.content.slice(cursorPos);
    onContentChange(idx, newContent);
    setAutocomplete(null);
    const newCursor = trigger.triggerStart + replacement.length;
    requestAnimationFrame(() => {
      if (textareaRef.current) {
        textareaRef.current.setSelectionRange(newCursor, newCursor);
        textareaRef.current.focus();
        adjustHeight(textareaRef.current);
      }
    });
  }

  const rowClass = [
    "block-row",
    isOutliner ? "block-row--outliner" : "block-row--plaintext",
    isFocused ? "block-row--editing" : "",
  ]
    .filter(Boolean)
    .join(" ");

  if (!isFocused) {
    return (
      <div
        className={rowClass}
        style={{ "--block-depth": block.depth }}
        onClick={() => onFocus(idx)}
      >
        {isOutliner && (
          <span className="block-bullet" aria-hidden="true">
            •
          </span>
        )}
        <div className="block-content">
          <ReactMarkdown
            urlTransform={(url) => url}
            components={{
              a: ({ href, children }) => {
                if (href?.startsWith('PAGE:')) {
                  const name = href.slice(5);
                  return (
                    <span
                      className="tag-link"
                      onClick={(e) => {
                        e.stopPropagation(); // prevent block-row onClick from firing
                        const target = pages.find(
                          p => p.page_id === name || p.title === name || pageLeafName(p.page_id) === name
                        );
                        if (target) {
                          onNavigate(target.page_id);
                        } else {
                          const pageId = `pages:${name}`;
                          invoke("create_page", { pageId })
                            .then(() => onNavigate(pageId))
                            .catch(console.error);
                        }
                      }}
                    >
                      {children}
                    </span>
                  );
                }
                return <a href={href}>{children}</a>;
              }
            }}
          >
            {preprocessTagsForRender(block.content || ' ')}
          </ReactMarkdown>
        </div>
      </div>
    );
  }

  return (
    <div className={rowClass} style={{ "--block-depth": block.depth }}>
      {isOutliner && (
        <span className="block-bullet" aria-hidden="true">
          •
        </span>
      )}
      <div className="block-textarea-wrap">
        <textarea
          ref={textareaRef}
          className="block-textarea"
          value={block.content}
          onChange={(e) => {
            onContentChange(idx, e.target.value);
            adjustHeight(e.target);
            updateAutocomplete(e.target.value, e.target.selectionStart);
          }}
          onKeyDown={(e) => {
            if (autocomplete) {
              if (e.key === 'ArrowDown') {
                e.preventDefault();
                setAutocomplete(prev => {
                  const maxIdx = prev.suggestions.length - 1 + (prev.createName ? 1 : 0);
                  return { ...prev, activeIdx: Math.min(prev.activeIdx + 1, maxIdx) };
                });
                return;
              }
              if (e.key === 'ArrowUp') {
                e.preventDefault();
                setAutocomplete(prev => ({ ...prev, activeIdx: Math.max(prev.activeIdx - 1, 0) }));
                return;
              }
              if (e.key === 'Enter' || e.key === 'Tab') {
                e.preventDefault();
                const isCreateItem = autocomplete.activeIdx === autocomplete.suggestions.length;
                applyAutocomplete(isCreateItem ? null : autocomplete.suggestions[autocomplete.activeIdx]);
                return;
              }
              if (e.key === 'Escape') {
                e.preventDefault();
                setAutocomplete(null);
                return;
              }
            }
            onKeyDown(e, idx);
          }}
          onBlur={() => { setAutocomplete(null); onFocus(null); }}
        />
        {autocomplete && (
          <ul className="autocomplete-dropdown" role="listbox">
            {autocomplete.suggestions.map((page, i) => (
              <li
                key={page.page_id}
                ref={i === autocomplete.activeIdx ? activeItemRef : null}
                className={`autocomplete-item${i === autocomplete.activeIdx ? ' autocomplete-item--active' : ''}`}
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
                className={`autocomplete-item autocomplete-item--create${autocomplete.activeIdx === autocomplete.suggestions.length ? ' autocomplete-item--active' : ''}`}
                role="option"
                onMouseDown={(e) => { e.preventDefault(); applyAutocomplete(null); }}
              >
                <span className="autocomplete-item-title">+ Create "{autocomplete.createName}"</span>
              </li>
            )}
          </ul>
        )}
      </div>
    </div>
  );
}

// ── Editor ─────────────────────────────────────────────────────────────────

export default function Editor({ pageId, blocks, pages, onNavigate }) {
  const [localBlocks, setLocalBlocks] = useState(blocks);
  const [focusedIdx, setFocusedIdx] = useState(null);
  const debounceRef = useRef(null);
  const localBlocksRef = useRef(blocks);

  useEffect(() => {
    localBlocksRef.current = localBlocks;
  }, [localBlocks]);

  // Reset when the incoming block array changes (page switch or external file change).
  useEffect(() => {
    clearTimeout(debounceRef.current);
    setLocalBlocks(blocks);
    localBlocksRef.current = blocks;
    setFocusedIdx(null);
  }, [blocks]); // eslint-disable-line react-hooks/exhaustive-deps

  // Flush on unmount (app close / workspace close).
  useEffect(() => {
    return () => {
      clearTimeout(debounceRef.current);
      if (pageId) {
        invoke("write_page_content", {
          pageId,
          text: blocksToText(localBlocksRef.current),
        }).catch(() => {});
      }
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  function scheduleWrite(newBlocks) {
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      invoke("write_page_content", {
        pageId,
        text: blocksToText(newBlocks),
      }).catch(() => {});
    }, WRITE_DEBOUNCE_MS);
  }

  function flushWrite(newBlocks) {
    clearTimeout(debounceRef.current);
    invoke("write_page_content", {
      pageId,
      text: blocksToText(newBlocks),
    }).catch(() => {});
  }

  function handleFocus(idx) {
    if (idx === null && focusedIdx !== null) {
      flushWrite(localBlocksRef.current);
    }
    setFocusedIdx(idx);
  }

  function handleContentChange(idx, newContent) {
    const newBlocks = localBlocks.map((b, i) =>
      i === idx ? { ...b, content: newContent } : b,
    );
    setLocalBlocks(newBlocks);
    localBlocksRef.current = newBlocks;
    scheduleWrite(newBlocks);
  }

  function handleKeyDown(e, idx) {
    const block = localBlocks[idx];
    const isOutliner = block.kind === "outliner";

    if (e.key === "Tab") {
      e.preventDefault();
      if (!isOutliner) return;

      let newDepth;
      if (e.shiftKey) {
        newDepth = Math.max(0, block.depth - 1);
      } else {
        const prev = localBlocks[idx - 1];
        const maxDepth = prev ? prev.depth + 1 : 0;
        newDepth = Math.min(block.depth + 1, maxDepth);
      }
      if (newDepth === block.depth) return;

      const newBlocks = localBlocks.map((b, i) =>
        i === idx ? { ...b, depth: newDepth } : b,
      );
      setLocalBlocks(newBlocks);
      localBlocksRef.current = newBlocks;
      scheduleWrite(newBlocks);
    } else if (e.key === "Enter" && isOutliner) {
      e.preventDefault();
      let newBlocks;
      if (block.content.trim() === "") {
        newBlocks = localBlocks.map((b, i) =>
          i === idx ? { kind: "plaintext", depth: 0, content: "" } : b,
        );
      } else {
        const cursor = e.target.selectionStart;
        const before = block.content.slice(0, cursor);
        const after = block.content.slice(cursor);
        newBlocks = [
          ...localBlocks.slice(0, idx),
          { ...block, content: before },
          { kind: block.kind, depth: block.depth, content: after },
          ...localBlocks.slice(idx + 1),
        ];
        setFocusedIdx(idx + 1);
      }
      setLocalBlocks(newBlocks);
      localBlocksRef.current = newBlocks;
      scheduleWrite(newBlocks);
    } else if (e.key === "Backspace" && isOutliner && block.content === "") {
      e.preventDefault();
      const newBlocks = localBlocks.map((b, i) =>
        i === idx ? { kind: "plaintext", depth: 0, content: "" } : b,
      );
      setLocalBlocks(newBlocks);
      localBlocksRef.current = newBlocks;
      scheduleWrite(newBlocks);
    }
  }

  return (
    <>
      {localBlocks.map((block, idx) => (
        <BlockRow
          key={idx}
          block={block}
          idx={idx}
          isFocused={idx === focusedIdx}
          onFocus={handleFocus}
          onContentChange={handleContentChange}
          onKeyDown={handleKeyDown}
          pages={pages}
          onNavigate={onNavigate}
        />
      ))}
    </>
  );
}
