import { useState, useEffect, useRef, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";

const WRITE_DEBOUNCE_MS = 300;

// ── Block parser ───────────────────────────────────────────────────────────

const TAB_WIDTH = 4;

function measureIndent(text, i, len) {
  let width = 0;
  let j = i;
  while (j < len) {
    if (text[j] === " ") {
      width++;
      j++;
    } else if (text[j] === "\t") {
      width += TAB_WIDTH;
      j++;
    } else {
      break;
    }
  }
  return { width, end: j };
}

function consumeContinuation(text, i, len, contentColumn) {
  while (i < len) {
    const { width: indentWidth, end: j } = measureIndent(text, i, len);

    if (j < len && text[j] === "-" && j + 1 < len && text[j + 1] === " ") break;
    if (indentWidth === 0 && j < len && text[j] === "◦" && j + 1 < len && text[j + 1] === " ") break;

    if (contentColumn !== undefined && indentWidth < contentColumn) break;

    while (i < len && text[i] !== "\n") i++;
    if (i < len) i++;
  }
  return i;
}

function consumeOutlinerContinuation(text, i, len, contentColumn) {
  while (i < len) {
    const { width: indentWidth, end: j } = measureIndent(text, i, len);

    if (j < len && text[j] === "-" && j + 1 < len && text[j + 1] === " ") break;
    if (indentWidth === 0 && j < len && text[j] === "◦" && j + 1 < len && text[j + 1] === " ") break;

    if (indentWidth < contentColumn) break;

    while (i < len && text[i] !== "\n") i++;
    if (i < len) i++;
  }
  return i;
}

function parseBlocks(text) {
  const blocks = [];
  let i = 0;
  const len = text.length;

  while (i < len) {
    const blockStart = i;
    const { width: indentWidth, end: markerStart } = measureIndent(text, i, len);

    if (markerStart < len && text[markerStart] === "-" && markerStart + 1 < len && text[markerStart + 1] === " ") {
      const contentStart = markerStart + 2;
      const contentColumn = indentWidth + 2;
      i = contentStart;
      while (i < len && text[i] !== "\n") i++;
      if (i < len) i++;
      i = consumeOutlinerContinuation(text, i, len, contentColumn);
      blocks.push({ start: blockStart, end: i, depth: Math.floor(indentWidth / TAB_WIDTH), kind: "outliner", contentStart });
    } else if (indentWidth === 0 && markerStart < len && text[markerStart] === "◦" && markerStart + 1 < len && text[markerStart + 1] === " ") {
      const contentStart = markerStart + 2;
      const contentColumn = indentWidth + 2;
      i = contentStart;
      while (i < len && text[i] !== "\n") i++;
      if (i < len) i++;
      i = consumeContinuation(text, i, len, contentColumn);
      blocks.push({ start: blockStart, end: i, depth: 0, kind: "explicit_plaintext", contentStart });
    } else {
      i = blockStart;
      while (i < len && text[i] !== "\n") i++;
      if (i < len) i++;
      i = consumeContinuation(text, i, len, 0);
      blocks.push({ start: blockStart, end: i, depth: 0, kind: "implicit_plaintext", contentStart: blockStart });
    }
  }

  return blocks;
}

// ── Helpers ────────────────────────────────────────────────────────────────

function blockPrefix(depth, kind) {
  if (kind === "outliner") return "\t".repeat(depth) + "- ";
  if (kind === "explicit_plaintext") return "◦ ";
  return "";
}

function spliceText(text, start, end, replacement) {
  return text.slice(0, start) + replacement + text.slice(end);
}

function blocksToText(blocks, depth = 0) {
  if (!blocks?.length) return "";
  return blocks
    .map((b) => {
      const prefix = blockPrefix(depth, b.kind);
      const own = prefix + (b.content ?? "") + "\n";
      const children = b.children?.length ? blocksToText(b.children, depth + 1) : "";
      return own + children;
    })
    .join("");
}

function getAbsolutePath(workspace, page) {
  if (!workspace || !page) return null;
  return workspace.root_path.replace(/\\/g, "/") + "/" + page.workspace_path;
}

function adjustHeight(el) {
  el.style.height = "auto";
  el.style.height = el.scrollHeight + "px";
}

function blockContent(text, block) {
  const raw = text.slice(block.contentStart, block.end);
  return raw.endsWith("\n") ? raw.slice(0, -1) : raw;
}

// ── BlockRow ───────────────────────────────────────────────────────────────

function BlockRow({ text, block, idx, isFocused, onFocus, onContentChange, onKeyDown }) {
  const textareaRef = useRef(null);
  const isOutliner = block.kind === "outliner";
  const content = blockContent(text, block);

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
          <ReactMarkdown>{content || " "}</ReactMarkdown>
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
        value={content}
        onChange={(e) => {
          onContentChange(idx, block, e.target.value);
          adjustHeight(e.target);
        }}
        onKeyDown={(e) => onKeyDown(e, idx, block, content)}
        onBlur={() => onFocus(null)}
      />
    </div>
  );
}

// ── Editor ─────────────────────────────────────────────────────────────────

export default function Editor({ pageId, blocks, workspace, page }) {
  const [text, setText] = useState(() => blocksToText(blocks));
  const [focusedIdx, setFocusedIdx] = useState(null);
  const debounceRef = useRef(null);
  const textRef = useRef(text);
  const pathRef = useRef(getAbsolutePath(workspace, page));

  const parsedBlocks = useMemo(() => parseBlocks(text), [text]);

  useEffect(() => {
    textRef.current = text;
  }, [text]);

  // Reset when page's blocks arrive. Also updates pathRef here — not earlier — so
  // pathRef and textRef are always in sync. Any flush before this point still uses
  // the previous page's path, which is correct.
  useEffect(() => {
    clearTimeout(debounceRef.current);
    pathRef.current = getAbsolutePath(workspace, page);
    const newText = blocksToText(blocks);
    setText(newText);
    textRef.current = newText;
    setFocusedIdx(null);
  }, [blocks]); // eslint-disable-line react-hooks/exhaustive-deps

  // Flush on unmount (app close / workspace close)
  useEffect(() => {
    return () => {
      clearTimeout(debounceRef.current);
      if (pathRef.current) {
        invoke("write_file", { path: pathRef.current, content: textRef.current }).catch(() => {});
      }
    };
  }, []);

  function scheduleWrite(newText) {
    const path = pathRef.current;
    if (!path) return;
    clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      invoke("write_file", { path, content: newText }).catch(() => {});
    }, WRITE_DEBOUNCE_MS);
  }

  function flushWrite(newText) {
    const path = pathRef.current;
    if (!path) return;
    clearTimeout(debounceRef.current);
    invoke("write_file", { path, content: newText }).catch(() => {});
  }

  function handleFocus(idx) {
    if (idx === null && focusedIdx !== null) {
      flushWrite(textRef.current);
    }
    setFocusedIdx(idx);
  }

  function handleContentChange(idx, block, newContent) {
    const newBlockText = blockPrefix(block.depth, block.kind) + newContent + "\n";
    const newText = spliceText(text, block.start, block.end, newBlockText);
    setText(newText);
    textRef.current = newText;
    scheduleWrite(newText);
  }

  function handleKeyDown(e, idx, block, currentContent) {
    const isOutliner = block.kind === "outliner";

    if (e.key === "Tab") {
      e.preventDefault();
      if (!isOutliner) return;

      let newDepth;
      if (e.shiftKey) {
        newDepth = Math.max(0, block.depth - 1);
      } else {
        const prevBlock = parsedBlocks[idx - 1];
        const maxDepth = prevBlock ? prevBlock.depth + 1 : 0;
        newDepth = Math.min(block.depth + 1, maxDepth);
      }
      if (newDepth === block.depth) return;

      const newText = spliceText(
        text,
        block.start,
        block.end,
        blockPrefix(newDepth, block.kind) + currentContent + "\n",
      );
      setText(newText);
      textRef.current = newText;
      scheduleWrite(newText);
    } else if (e.key === "Enter" && isOutliner) {
      e.preventDefault();
      if (currentContent.trim() === "") {
        const newText = spliceText(text, block.start, block.end, "◦ \n");
        setText(newText);
        textRef.current = newText;
        scheduleWrite(newText);
      } else {
        const cursor = e.target.selectionStart;
        const before = currentContent.slice(0, cursor);
        const after = currentContent.slice(cursor);
        const newText = spliceText(
          text,
          block.start,
          block.end,
          blockPrefix(block.depth, block.kind) + before + "\n" +
            blockPrefix(block.depth, block.kind) + after + "\n",
        );
        setText(newText);
        textRef.current = newText;
        scheduleWrite(newText);
        setFocusedIdx(idx + 1);
      }
    } else if (e.key === "Backspace" && isOutliner && currentContent === "") {
      e.preventDefault();
      const newText = spliceText(text, block.start, block.end, "◦ \n");
      setText(newText);
      textRef.current = newText;
      scheduleWrite(newText);
    }
  }

  return (
    <>
      {parsedBlocks.map((block, idx) => (
        <BlockRow
          key={idx}
          text={text}
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
