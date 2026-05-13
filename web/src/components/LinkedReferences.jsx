import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

function linkedRefKey(entry) {
  return [
    entry.source_page_id,
    entry.block.handle.block_span.start,
    entry.block.handle.block_span.end,
    entry.ref_span.start,
    entry.ref_span.end,
  ].join(":");
}

function readPageLabel(page) {
  if (!page) {
    return "";
  }

  return page.title || page.page_id;
}

function trimTrailingBreak(value) {
  return value.replace(/\r?\n$/, "");
}

function renderHighlightedContent(highlight) {
  if (!highlight) {
    return null;
  }

  return (
    <>
      {highlight.prefix}
      <mark className="linked-ref-mark">{highlight.highlight}</mark>
      {highlight.suffix}
    </>
  );
}

function BlockTree({ block, highlight, depth = 0 }) {
  const blockClassName = [
    "linked-ref-block",
    block.kind === "plaintext" ? "linked-ref-block--plaintext" : "linked-ref-block--outliner",
  ].join(" ");
  const content = highlight ? null : trimTrailingBreak(block.content);

  return (
    <div className={blockClassName} style={{ "--linked-ref-depth": depth }}>
      <div className="linked-ref-block-line">
        {block.kind === "outliner" ? <span className="linked-ref-bullet">-</span> : null}
        <span className="linked-ref-block-content">
          {highlight ? renderHighlightedContent(highlight) : content}
        </span>
      </div>
      {block.children.length > 0 ? (
        <div className="linked-ref-children">
          {block.children.map((child) => (
            <BlockTree key={`${child.block_span.start}:${child.block_span.end}`} block={child} depth={depth + 1} />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function LinkedRefRow({
  entry,
  isEditing,
  draft,
  saving,
  error,
  onStartEdit,
  onDraftChange,
  onCancel,
  onSave,
}) {
  const editorRef = useRef(null);

  useEffect(() => {
    if (!isEditing) {
      return;
    }

    editorRef.current?.focus();
    editorRef.current?.setSelectionRange(draft.length, draft.length);
  }, [draft.length, isEditing]);

  return (
    <article className={`linked-ref-row${isEditing ? " linked-ref-row--editing" : ""}`}>
      {!isEditing ? (
        <div
          className="linked-ref-preview"
          role="button"
          tabIndex={0}
          onClick={() => onStartEdit(entry)}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              onStartEdit(entry);
            }
          }}
        >
          <BlockTree block={entry.block} highlight={entry.block_content_highlight} />
        </div>
      ) : (
        <div className="linked-ref-editor">
          <textarea
            ref={editorRef}
            id={`linked-ref-editor-${linkedRefKey(entry)}`}
            className="linked-ref-editor-input"
            value={draft}
            onChange={(event) => onDraftChange(event.target.value)}
            spellCheck={false}
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                event.preventDefault();
                onSave(entry);
              }
              if (event.key === "Escape") {
                event.preventDefault();
                onCancel();
              }
            }}
          />
          {error ? <p className="linked-ref-editor-error">{error}</p> : null}
          <div className="linked-ref-editor-actions">
            <button className="primary-button" type="button" onClick={() => onSave(entry)} disabled={saving}>
              {saving ? "Saving..." : "Save block"}
            </button>
            <button className="secondary-button" type="button" onClick={onCancel} disabled={saving}>
              Cancel
            </button>
          </div>
        </div>
      )}
    </article>
  );
}

export default function LinkedReferences({
  entries,
  pages,
  onNavigate,
  onReload,
  onNotice,
}) {
  const [editingKey, setEditingKey] = useState("");
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  const pagesById = useMemo(() => new Map(pages.map((page) => [page.page_id, page])), [pages]);
  const groupedEntries = useMemo(() => {
    const groups = [];
    const bySource = new Map();
    for (const entry of entries) {
      const existing = bySource.get(entry.source_page_id);
      if (existing) {
        existing.entries.push(entry);
        continue;
      }
      const group = { sourcePageId: entry.source_page_id, entries: [entry] };
      bySource.set(entry.source_page_id, group);
      groups.push(group);
    }
    return groups;
  }, [entries]);

  useEffect(() => {
    if (!editingKey) {
      return;
    }

    const stillExists = entries.some((entry) => linkedRefKey(entry) === editingKey);
    if (!stillExists) {
      setEditingKey("");
      setDraft("");
      setSaving(false);
      setError("");
    }
  }, [editingKey, entries]);

  async function handleSave(entry) {
    setSaving(true);
    setError("");
    try {
      await invoke("write_block_markdown", {
        handle: entry.block.handle,
        replacementMarkdown: draft,
      });
      await onReload();
      setEditingKey("");
      setDraft("");
    } catch (saveError) {
      if (saveError?.code === "structural_conflict") {
        try {
          const freshBlock = await invoke("block_snapshot", {
            handle: entry.block.handle,
          });
          setDraft(freshBlock.markdown);
          setError("The source changed while you were editing. Reloaded the latest block.");
          onNotice?.("The source block changed while you were editing. Reloaded the latest block.");
        } catch {
          await onReload();
          setEditingKey("");
          setDraft("");
          onNotice?.("The source block changed while you were editing. Reloaded linked references.");
        }
      } else {
        setError(saveError?.message ?? "Could not save block.");
      }
    } finally {
      setSaving(false);
    }
  }

  if (entries.length === 0) {
    return (
      <section className="linked-refs-panel">
        <div className="linked-refs-heading">
          <h2>Linked references</h2>
          <span className="linked-refs-count">0</span>
        </div>
        <p className="empty-state">Mentions from other pages will appear here.</p>
      </section>
    );
  }

  return (
    <section className="linked-refs-panel">
      <div className="linked-refs-heading">
        <h2>Linked references</h2>
        <span className="linked-refs-count">{entries.length}</span>
      </div>
      <div className="linked-refs-groups">
        {groupedEntries.map((group) => (
          <section key={group.sourcePageId} className="linked-refs-group">
            <div className="linked-refs-group-header">
              <button
                className="linked-refs-group-title"
                type="button"
                onClick={() => onNavigate(group.sourcePageId)}
              >
                {readPageLabel(pagesById.get(group.sourcePageId)) || group.sourcePageId}
              </button>
              <span>{group.entries.length} mention{group.entries.length === 1 ? "" : "s"}</span>
            </div>
            <div className="linked-refs-group-body">
              {group.entries.map((entry) => {
                const key = linkedRefKey(entry);
                return (
                  <LinkedRefRow
                    key={key}
                    entry={entry}
                    isEditing={editingKey === key}
                    draft={editingKey === key ? draft : ""}
                    saving={editingKey === key && saving}
                    error={editingKey === key ? error : ""}
                    onStartEdit={(nextEntry) => {
                      setEditingKey(linkedRefKey(nextEntry));
                      setDraft(nextEntry.block.markdown);
                      setSaving(false);
                      setError("");
                    }}
                    onDraftChange={setDraft}
                    onCancel={() => {
                      setEditingKey("");
                      setDraft("");
                      setSaving(false);
                      setError("");
                    }}
                    onSave={handleSave}
                  />
                );
              })}
            </div>
          </section>
        ))}
      </div>
    </section>
  );
}
