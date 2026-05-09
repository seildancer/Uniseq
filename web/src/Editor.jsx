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

// ── BlockRow ───────────────────────────────────────────────────────────────

function BlockRow({ block, idx, isFocused, onFocus, onContentChange, onKeyDown }) {
  const textareaRef = useRef(null);
  const isOutliner = block.kind === "outliner";

  useEffect(() => {
    if (isFocused && textareaRef.current) {
      const el = textareaRef.current;
      el.focus();
      const len = el.value.length;
      el.setSelectionRange(len, len);
      adjustHeight(el);
    }
  }, [isFocused]);

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
          <ReactMarkdown>{block.content || " "}</ReactMarkdown>
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
      <textarea
        ref={textareaRef}
        className="block-textarea"
        value={block.content}
        onChange={(e) => {
          onContentChange(idx, e.target.value);
          adjustHeight(e.target);
        }}
        onKeyDown={(e) => onKeyDown(e, idx)}
        onBlur={() => onFocus(null)}
      />
    </div>
  );
}

// ── Editor ─────────────────────────────────────────────────────────────────

export default function Editor({ pageId, blocks }) {
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
        />
      ))}
    </>
  );
}
