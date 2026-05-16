import { useMemo, useState } from "react";
import BlockEditor from "../BlockEditor";
import { isDiaryStream, readStreamName } from "../utils/streamWorkspace.js";

function readPageLabel(page) {
  if (!page) {
    return { title: "", streamName: null };
  }

  const title = page.title || page.page_id;
  const streamName = readStreamName(page.location);
  return { title, streamName };
}

export default function LinkedReferences({
  entries,
  pages,
  onNavigate,
  onReload,
  onNotice,
  diaryBlurEnabled = true,
}) {
  const [focusedRefKey, setFocusedRefKey] = useState(null);
  const pagesById = useMemo(() => new Map(pages.map((page) => [page.page_id, page])), [pages]);
  const sourcePageSortTitle = (sourcePageId) => {
    const page = pagesById.get(sourcePageId);
    return page?.title || page?.page_id || sourcePageId;
  };
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
    return groups.sort((left, right) =>
      sourcePageSortTitle(right.sourcePageId).localeCompare(sourcePageSortTitle(left.sourcePageId))
    );
  }, [entries, pagesById]);

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
      <div className="linked-refs-list">
        {groupedEntries.map((group) => {
          const label = readPageLabel(pagesById.get(group.sourcePageId));
          const isDiarySource = isDiaryStream(label.streamName);

          return (
            <section key={group.sourcePageId} className="linked-refs-source">
              <div className="linked-refs-source-meta">
                <button className="linked-refs-group-title" type="button" onClick={() => onNavigate(group.sourcePageId)}>
                  <span>{label.title || group.sourcePageId}</span>
                  {label.streamName ? (
                    <span className="linked-refs-stream-pill">{label.streamName}</span>
                  ) : null}
                </button>
                <span>{group.entries.length} mention{group.entries.length === 1 ? "" : "s"}</span>
              </div>
              {group.entries.map((entry) => {
                const refKey = [
                  entry.source_page_id,
                  entry.block.handle.block_span.start,
                  entry.block.handle.block_span.end,
                  entry.ref_span.start,
                  entry.ref_span.end,
                ].join(":");
                const shouldBlurReference = diaryBlurEnabled && isDiarySource && focusedRefKey !== refKey;

                return (
                  <div
                    className={`linked-ref-row${shouldBlurReference ? " linked-ref-row--privacy-blurred" : ""}`}
                    key={refKey}
                  >
                    <BlockEditor
                      entry={entry}
                      pages={pages}
                      onNavigate={onNavigate}
                      onReload={onReload}
                      onNotice={onNotice}
                      onFocusChange={(focused) => {
                        setFocusedRefKey((current) => {
                          if (focused) {
                            return refKey;
                          }
                          return current === refKey ? null : current;
                        });
                      }}
                    />
                  </div>
                );
              })}
            </section>
          );
        })}
      </div>
    </section>
  );
}
