import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { addDaysToDateName, compareDateNames, formatDateLabel, maxDateName, todayDateName } from "../utils/streamDates.js";
import {
  isDiaryStream,
  streamPageExists,
  streamPageId,
} from "../utils/streamWorkspace.js";

const SNAP_BUFFER_DAYS = 1;
const TAP_MAX_DURATION_MS = 250;
const TAP_MAX_MOVE_PX = 10;

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
  const focusedEditorRef = useRef(null);
  const pendingTapRef = useRef(null);
  const latestDateName = useMemo(
    () => maxDateName([todayDateName(), selectedDate, ...streamPagesByDate.keys()], selectedDate),
    [selectedDate, streamPagesByDate],
  );
  const visibleDates = useMemo(() => {
    const dates = [];
    for (let i = SNAP_BUFFER_DAYS; i >= -SNAP_BUFFER_DAYS; i -= 1) {
      const candidate = addDaysToDateName(selectedDate, i);
      if (latestDateName && compareDateNames(candidate, latestDateName) > 0) {
        continue;
      }
      dates.push(candidate);
    }
    return dates;
  }, [selectedDate, latestDateName]);

  const selectedDateRef = useRef(selectedDate);
  selectedDateRef.current = selectedDate;
  const programmaticScrollRef = useRef(false);
  const suppressScrollSnapUntilRef = useRef(0);

  focusedEditorRef.current = focusedEditor;

  function suppressDateSnap(durationMs = 900) {
    suppressScrollSnapUntilRef.current = Date.now() + durationMs;
  }

  function isEditorDomFocused() {
    const activeElement = document.activeElement;
    if (!activeElement) {
      return false;
    }
    return Boolean(activeElement.closest?.(".milkdown-editor, .ProseMirror"));
  }

  useLayoutEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const selectedEntry = dayRefs.current.get(selectedDate);
    if (selectedEntry) {
      programmaticScrollRef.current = true;
      selectedEntry.scrollIntoView({ block: "start", behavior: "auto" });
    }
  }, [selectedDate, scrollContainerRef]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || typeof onSelectDate !== "function") return;

    function handleScrollEnd() {
      if (programmaticScrollRef.current) {
        programmaticScrollRef.current = false;
        return;
      }
      if (focusedEditorRef.current || isEditorDomFocused()) {
        return;
      }
      if (Date.now() < suppressScrollSnapUntilRef.current) {
        return;
      }

      let closestDate = null;
      let closestDist = Infinity;
      const containerRect = container.getBoundingClientRect();

      for (const [dateName, node] of dayRefs.current) {
        if (!node) continue;
        const rect = node.getBoundingClientRect();
        const dist = Math.abs(rect.top - containerRect.top);
        if (dist < closestDist) {
          closestDist = dist;
          closestDate = dateName;
        }
      }

      if (closestDate && closestDate !== selectedDateRef.current) {
        onSelectDate(closestDate);
      }
    }

    container.addEventListener("scrollend", handleScrollEnd);
    return () => container.removeEventListener("scrollend", handleScrollEnd);
  }, [scrollContainerRef, onSelectDate]);

  useEffect(() => {
    if (focusedEditor && !streamNames.includes(focusedEditor.streamName)) {
      setFocusedEditor(null);
    }
  }, [streamNames, focusedEditor]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || !focusedEditor) {
      return undefined;
    }

    const previousOverflow = container.style.overflow;
    const previousTouchAction = container.style.touchAction;
    const previousOverscrollBehavior = container.style.overscrollBehavior;

    container.style.overflow = "hidden";
    container.style.touchAction = "none";
    container.style.overscrollBehavior = "none";

    return () => {
      container.style.overflow = previousOverflow;
      container.style.touchAction = previousTouchAction;
      container.style.overscrollBehavior = previousOverscrollBehavior;
    };
  }, [focusedEditor, scrollContainerRef]);

  function enterFocusMode(dateName, streamName) {
    suppressDateSnap();
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

  function handlePanePointerDown(event, editorKey) {
    if (event.target.closest?.(".ProseMirror")) {
      pendingTapRef.current = null;
      return;
    }
    if (event.button !== undefined && event.button !== 0) {
      pendingTapRef.current = null;
      return;
    }

    pendingTapRef.current = {
      editorKey,
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      startedAt: Date.now(),
      moved: false,
    };
  }

  function handlePanePointerMove(event) {
    const pendingTap = pendingTapRef.current;
    if (!pendingTap || pendingTap.pointerId !== event.pointerId) {
      return;
    }

    const distance = Math.hypot(event.clientX - pendingTap.startX, event.clientY - pendingTap.startY);
    if (distance > TAP_MAX_MOVE_PX) {
      pendingTapRef.current = {
        ...pendingTap,
        moved: true,
      };
    }
  }

  function handlePanePointerCancel(event) {
    if (pendingTapRef.current?.pointerId === event.pointerId) {
      pendingTapRef.current = null;
    }
  }

  function handlePanePointerUp(event, editorKey) {
    const pendingTap = pendingTapRef.current;
    pendingTapRef.current = null;

    if (!pendingTap || pendingTap.pointerId !== event.pointerId || pendingTap.editorKey !== editorKey) {
      return;
    }

    const duration = Date.now() - pendingTap.startedAt;
    if (pendingTap.moved || duration > TAP_MAX_DURATION_MS) {
      return;
    }

    suppressDateSnap();
    focusPaneEditor(event, editorKey);
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
      <div className={`stream-day-list stream-day-list--mobile-single${focusedEditor ? " stream-day-list--has-focus" : ""}`}>
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
                      onPointerDown={(event) => handlePanePointerDown(event, editorKey)}
                      onPointerMove={handlePanePointerMove}
                      onPointerCancel={handlePanePointerCancel}
                      onPointerUp={(event) => handlePanePointerUp(event, editorKey)}
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
