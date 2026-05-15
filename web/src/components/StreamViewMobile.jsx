import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { useMobileStreamDatePager } from "../hooks/useMobileStreamDatePager.js";
import { formatDateLabel, maxDateName, todayDateName } from "../utils/streamDates.js";
import {
  isDiaryStream,
  streamPageExists,
  streamPageId,
} from "../utils/streamWorkspace.js";

export default function StreamViewMobile({
  selectedDate,
  streamNames,
  streamPagesByDate,
  pages,
  reloadToken,
  scrollContainerRef,
  onNavigate,
  onError,
  onRefresh,
  onSelectDate,
  diaryBlurEnabled = true,
}) {
  const [focusedEditor, setFocusedEditor] = useState(null);
  const [editorReadyByKey, setEditorReadyByKey] = useState(() => new Map());
  const dayRefs = useRef(new Map());
  const editorFocusRefs = useRef(new Map());
  const latestDateName = useMemo(
    () => maxDateName([todayDateName(), selectedDate, ...streamPagesByDate.keys()], selectedDate),
    [selectedDate, streamPagesByDate],
  );
  const visibleDates = useMemo(() => [selectedDate], [selectedDate]);

  useMobileStreamDatePager({
    enabled: true,
    selectedDate,
    latestDateName,
    scrollContainerRef,
    onSelectDate,
  });

  useLayoutEffect(() => {
    const container = scrollContainerRef.current;
    if (container) {
      container.scrollTo({ top: 0, behavior: "auto" });
    }
  }, [selectedDate, scrollContainerRef]);

  useEffect(() => {
    if (focusedEditor && !streamNames.includes(focusedEditor.streamName)) {
      setFocusedEditor(null);
    }
  }, [streamNames, focusedEditor]);

  function enterFocusMode(dateName, streamName) {
    setFocusedEditor({ dateName, streamName });
  }

  function editorFocusRefForKey(editorKey) {
    let focusRef = editorFocusRefs.current.get(editorKey);
    if (!focusRef) {
      focusRef = { current: null };
      editorFocusRefs.current.set(editorKey, focusRef);
    }
    return focusRef;
  }

  function focusPaneEditor(event, editorKey) {
    if (event.target.closest?.(".ProseMirror")) {
      return;
    }
    if (event.button !== undefined && event.button !== 0) {
      return;
    }

    const didFocus = editorFocusRefs.current.get(editorKey)?.current?.({ atEnd: true });
    if (!didFocus) {
      event.currentTarget.querySelector(".ProseMirror")?.focus();
    }
    if (event.pointerType === "mouse") {
      event.preventDefault();
    }
  }

  function handleEditorReadyChange(editorKey, ready) {
    setEditorReadyByKey((current) => {
      const next = new Map(current);
      next.set(editorKey, ready);
      return next;
    });
  }

  return (
    <div className="stream-dual-wrap stream-dual-wrap--mobile">
      <div className="stream-day-list stream-day-list--mobile-single">
        {visibleDates.map((dateName) => {
          const focusedStreamName = focusedEditor?.dateName === dateName ? focusedEditor.streamName : null;
          const isSelected = selectedDate === dateName;
          const paneStates = streamNames.map((streamName) => {
            const editorKey = `${streamName}/${dateName}`;
            const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
              ? streamPageId(streamName, dateName)
              : null;
            const focusEditorRef = editorFocusRefForKey(editorKey);
            const shouldBlur = diaryBlurEnabled
              && isDiaryStream(streamName)
              && Boolean(existingPageId)
              && focusedStreamName !== streamName;

            return {
              streamName,
              editorKey,
              focusEditorRef,
              existingPageId,
              shouldBlur,
            };
          });

          return (
            <section
              key={dateName}
              ref={(node) => {
                if (node) {
                  dayRefs.current.set(dateName, node);
                } else {
                  dayRefs.current.delete(dateName);
                }
              }}
              className={`stream-day-entry${focusedStreamName ? " stream-day-entry--focused" : ""}${isSelected ? " stream-day-entry--selected" : ""}`}
            >
              <div className="stream-day-entry-header">
                <h2 className="stream-day-entry-title">{formatDateLabel(dateName)}</h2>
              </div>

              <div className="stream-day-entry-body">
                <div
                  className="stream-dual-pane"
                  style={{
                    gridTemplateRows: paneStates.map((pane) => (
                      pane.streamName === focusedStreamName ? "minmax(0, 9fr)" : "minmax(0, 1fr)"
                    )).join(" ")
                      || "minmax(0, 1fr)",
                  }}
                >
                  {paneStates.map(({ streamName, editorKey, focusEditorRef, existingPageId, shouldBlur }) => (
                    <div
                      key={editorKey}
                      className={`stream-dual-panel${focusedStreamName === streamName ? " stream-dual-panel--focused" : ""}${focusedStreamName && focusedStreamName !== streamName ? " stream-dual-panel--compressed" : ""}`}
                      onPointerDown={(event) => focusPaneEditor(event, editorKey)}
                    >
                      <p className="stream-panel-label">{streamName}</p>
                      <div className={`stream-editor-pane${shouldBlur ? " stream-editor-pane--privacy-blurred" : ""}`}>
                        <StreamSingleEditor
                          key={editorKey}
                          streamName={streamName}
                          dateName={dateName}
                          existingPageId={existingPageId}
                          pages={pages}
                          reloadToken={reloadToken}
                          onNavigate={onNavigate}
                          onError={onError}
                          onRefresh={onRefresh}
                          focusEditorRef={focusEditorRef}
                          onReadyChange={(ready) => handleEditorReadyChange(editorKey, ready)}
                          onFocusChange={(focused) => {
                            if (focused) {
                              enterFocusMode(dateName, streamName);
                              return;
                            }
                            setFocusedEditor((current) => {
                              if (current?.dateName === dateName && current?.streamName === streamName) {
                                return null;
                              }
                              return current;
                            });
                          }}
                        />
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}
