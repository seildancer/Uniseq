import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { addDaysToDateName, compareDateNames, formatDateLabel, maxDateName, todayDateName } from "../utils/streamDates.js";
import {
  isDiaryStream,
  streamPageExists,
  streamPageId,
} from "../utils/streamWorkspace.js";

const NEIGHBOR_DAY_COUNT = 1;
const SCROLL_SETTLE_MS = 140;
const PROGRAMMATIC_SCROLL_MS = 260;
const FOCUS_RETRY_LIMIT = 24;

function editorKeyFor(streamName, dateName) {
  return `${streamName}/${dateName}`;
}

function MobileFocusScreen({
  focusedEditor,
  editorKey,
  existingPageId,
  pages,
  reloadToken,
  onNavigate,
  onError,
  onRefresh,
  focusEditorRef,
  onClose,
}) {
  return (
    <section className="stream-mobile-focus-screen">
      <div className="stream-mobile-focus-header">
        <div className="stream-mobile-focus-heading">
          <div className="stream-day-entry-header">
            <h2 className="stream-day-entry-title">{formatDateLabel(focusedEditor.dateName)}</h2>
          </div>
          <div className="stream-mobile-focus-stream">
            <p className="stream-panel-label">{focusedEditor.streamName}</p>
          </div>
        </div>
        <button
          type="button"
          className="stream-mobile-focus-back"
          onClick={onClose}
          aria-label="Back to stream dates"
        >
          Back
        </button>
      </div>

      <div className="stream-mobile-focus-body">
        <div className="stream-mobile-focus-editor">
          <StreamSingleEditor
            key={editorKey}
            streamName={focusedEditor.streamName}
            dateName={focusedEditor.dateName}
            existingPageId={existingPageId}
            pages={pages}
            reloadToken={reloadToken}
            onNavigate={onNavigate}
            onError={onError}
            onRefresh={onRefresh}
            focusEditorRef={focusEditorRef}
            onFocusChange={() => {}}
          />
        </div>
      </div>
    </section>
  );
}

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
  const dayRefs = useRef(new Map());
  const editorFocusRefs = useRef(new Map());
  const selectedDateRef = useRef(selectedDate);
  const focusedEditorRef = useRef(null);
  const shouldFocusScreenEditorRef = useRef(false);
  const pendingFocusRequestRef = useRef(null);
  const scrollSettleTimerRef = useRef(null);
  const programmaticScrollTimerRef = useRef(null);
  const focusRetryTimerRef = useRef(null);
  const focusRetryCountRef = useRef(0);
  const isProgrammaticScrollRef = useRef(false);

  selectedDateRef.current = selectedDate;
  focusedEditorRef.current = focusedEditor;

  const latestDateName = useMemo(
    () => maxDateName([todayDateName(), selectedDate, ...streamPagesByDate.keys()], selectedDate),
    [selectedDate, streamPagesByDate],
  );

  const visibleDates = useMemo(() => {
    const dates = [];
    for (let offset = NEIGHBOR_DAY_COUNT; offset >= -NEIGHBOR_DAY_COUNT; offset -= 1) {
      const dateName = addDaysToDateName(selectedDate, offset);
      if (latestDateName && compareDateNames(dateName, latestDateName) > 0) {
        continue;
      }
      dates.push(dateName);
    }
    return dates;
  }, [latestDateName, selectedDate]);

  function clearScrollTimers() {
    if (scrollSettleTimerRef.current !== null) {
      window.clearTimeout(scrollSettleTimerRef.current);
      scrollSettleTimerRef.current = null;
    }
    if (programmaticScrollTimerRef.current !== null) {
      window.clearTimeout(programmaticScrollTimerRef.current);
      programmaticScrollTimerRef.current = null;
    }
    if (focusRetryTimerRef.current !== null) {
      window.clearTimeout(focusRetryTimerRef.current);
      focusRetryTimerRef.current = null;
    }
  }

  function markProgrammaticScroll() {
    isProgrammaticScrollRef.current = true;
    if (programmaticScrollTimerRef.current !== null) {
      window.clearTimeout(programmaticScrollTimerRef.current);
    }
    programmaticScrollTimerRef.current = window.setTimeout(() => {
      isProgrammaticScrollRef.current = false;
      programmaticScrollTimerRef.current = null;
    }, PROGRAMMATIC_SCROLL_MS);
  }

  function scrollDateIntoView(dateName, behavior = "auto") {
    const node = dayRefs.current.get(dateName);
    const container = scrollContainerRef.current;
    if (!node || !container) {
      return;
    }

    const containerRect = container.getBoundingClientRect();
    const nodeRect = node.getBoundingClientRect();
    markProgrammaticScroll();
    container.scrollTo({
      top: container.scrollTop + nodeRect.top - containerRect.top,
      behavior,
    });
  }

  function closestVisibleDate() {
    const container = scrollContainerRef.current;
    if (!container) {
      return null;
    }

    let closestDate = null;
    let closestDistance = Infinity;
    const containerTop = container.getBoundingClientRect().top;
    for (const [dateName, node] of dayRefs.current) {
      if (!node) {
        continue;
      }
      const distance = Math.abs(node.getBoundingClientRect().top - containerTop);
      if (distance < closestDistance) {
        closestDate = dateName;
        closestDistance = distance;
      }
    }
    return closestDate;
  }

  function editorFocusRefForKey(editorKey) {
    let focusRef = editorFocusRefs.current.get(editorKey);
    if (!focusRef) {
      focusRef = { current: null };
      editorFocusRefs.current.set(editorKey, focusRef);
    }
    return focusRef;
  }

  function tryFocusEntry(dateName, streamName) {
    const editorKey = editorKeyFor(streamName, dateName);
    return editorFocusRefs.current.get(editorKey)?.current?.(pendingFocusRequestRef.current ?? { atEnd: true }) ?? false;
  }

  function scheduleFocusRetry(dateName, streamName) {
    if (focusRetryCountRef.current >= FOCUS_RETRY_LIMIT) {
      shouldFocusScreenEditorRef.current = false;
      pendingFocusRequestRef.current = null;
      focusRetryTimerRef.current = null;
      return;
    }

    focusRetryCountRef.current += 1;
    focusRetryTimerRef.current = window.setTimeout(() => {
      focusRetryTimerRef.current = null;
      if (!focusedEditorRef.current) {
        shouldFocusScreenEditorRef.current = false;
        pendingFocusRequestRef.current = null;
        return;
      }
      if (tryFocusEntry(dateName, streamName)) {
        shouldFocusScreenEditorRef.current = false;
        pendingFocusRequestRef.current = null;
        focusRetryCountRef.current = 0;
        return;
      }
      scheduleFocusRetry(dateName, streamName);
    }, 32);
  }

  function openFocusScreen(dateName, streamName, focusRequest = { atEnd: true }) {
    if (dateName !== selectedDateRef.current) {
      onSelectDate?.(dateName);
    }
    scrollDateIntoView(dateName, "auto");
    shouldFocusScreenEditorRef.current = true;
    pendingFocusRequestRef.current = focusRequest;
    focusRetryCountRef.current = 0;
    setFocusedEditor({ dateName, streamName });
  }

  function focusRequestFromPointerEvent(event) {
    const rect = event.currentTarget.getBoundingClientRect();
    const width = Math.max(rect.width, 1);
    const height = Math.max(rect.height, 1);
    const clientX = event.clientX ?? rect.left;
    const clientY = event.clientY ?? rect.top;
    return {
      atEnd: false,
      point: {
        xRatio: Math.min(Math.max((clientX - rect.left) / width, 0), 1),
        yRatio: Math.min(Math.max((clientY - rect.top) / height, 0), 1),
      },
    };
  }

  function handleBrowsePaneClick(event, dateName, streamName) {
    openFocusScreen(dateName, streamName, focusRequestFromPointerEvent(event));
  }

  function handleBrowsePaneKeyDown(event, dateName, streamName) {
    if (event.key !== "Enter" && event.key !== " ") {
      return;
    }
    event.preventDefault();
    openFocusScreen(dateName, streamName);
  }

  useLayoutEffect(() => {
    if (focusedEditorRef.current) {
      return;
    }
    scrollDateIntoView(selectedDate, "auto");
  }, [selectedDate, scrollContainerRef, focusedEditor]);

  useEffect(() => {
    return () => {
      clearScrollTimers();
    };
  }, []);

  useEffect(() => {
    if (focusedEditor && !streamNames.includes(focusedEditor.streamName)) {
      setFocusedEditor(null);
    }
  }, [focusedEditor, streamNames]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) {
      return undefined;
    }
    container.classList.toggle("editor-panel-scroll--focus-locked", Boolean(focusedEditor));
    return () => {
      container.classList.remove("editor-panel-scroll--focus-locked");
    };
  }, [focusedEditor, scrollContainerRef]);

  useEffect(() => {
    if (!focusedEditor || !shouldFocusScreenEditorRef.current) {
      return;
    }
    const timerId = window.setTimeout(() => {
      if (tryFocusEntry(focusedEditor.dateName, focusedEditor.streamName)) {
        shouldFocusScreenEditorRef.current = false;
        pendingFocusRequestRef.current = null;
        focusRetryCountRef.current = 0;
        return;
      }
      scheduleFocusRetry(focusedEditor.dateName, focusedEditor.streamName);
    }, 0);
    return () => {
      window.clearTimeout(timerId);
      if (focusRetryTimerRef.current !== null) {
        window.clearTimeout(focusRetryTimerRef.current);
        focusRetryTimerRef.current = null;
      }
    };
  }, [focusedEditor]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container || typeof onSelectDate !== "function") {
      return undefined;
    }

    function handleScroll() {
      if (isProgrammaticScrollRef.current || focusedEditorRef.current) {
        return;
      }

      if (scrollSettleTimerRef.current !== null) {
        window.clearTimeout(scrollSettleTimerRef.current);
      }
      scrollSettleTimerRef.current = window.setTimeout(() => {
        scrollSettleTimerRef.current = null;
        if (focusedEditorRef.current) {
          return;
        }
        const nextDate = closestVisibleDate();
        if (nextDate && nextDate !== selectedDateRef.current) {
          onSelectDate(nextDate);
        }
      }, SCROLL_SETTLE_MS);
    }

    container.addEventListener("scroll", handleScroll, { passive: true });
    return () => {
      container.removeEventListener("scroll", handleScroll);
      if (scrollSettleTimerRef.current !== null) {
        window.clearTimeout(scrollSettleTimerRef.current);
        scrollSettleTimerRef.current = null;
      }
    };
  }, [onSelectDate, scrollContainerRef]);

  if (focusedEditor) {
    const editorKey = editorKeyFor(focusedEditor.streamName, focusedEditor.dateName);
    const existingPageId = streamPageExists(
      streamPagesByDate,
      focusedEditor.dateName,
      focusedEditor.streamName,
    )
      ? streamPageId(focusedEditor.streamName, focusedEditor.dateName)
      : null;

    return (
      <div className="stream-dual-wrap stream-dual-wrap--mobile">
        <MobileFocusScreen
          focusedEditor={focusedEditor}
          editorKey={editorKey}
          existingPageId={existingPageId}
          pages={pages}
          reloadToken={reloadToken}
          onNavigate={onNavigate}
          onError={onError}
          onRefresh={onRefresh}
          focusEditorRef={editorFocusRefForKey(editorKey)}
          onClose={() => setFocusedEditor(null)}
        />
      </div>
    );
  }

  return (
    <div className="stream-dual-wrap stream-dual-wrap--mobile">
      <div className="stream-day-list stream-day-list--mobile-single">
        {visibleDates.map((dateName) => {
          const paneStates = streamNames.map((streamName) => {
            const editorKey = editorKeyFor(streamName, dateName);
            const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
              ? streamPageId(streamName, dateName)
              : null;

            return {
              editorKey,
              existingPageId,
              focusEditorRef: editorFocusRefForKey(editorKey),
              shouldBlur: diaryBlurEnabled && isDiaryStream(streamName) && Boolean(existingPageId),
              streamName,
            };
          });
          const isEmpty = paneStates.every((pane) => !pane.existingPageId);

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
              className={`stream-day-entry stream-day-entry--ready${selectedDate === dateName ? " stream-day-entry--selected" : ""}${isEmpty ? " stream-day-entry--empty" : ""}`}
            >
              <div className="stream-day-entry-header">
                <h2 className="stream-day-entry-title">{formatDateLabel(dateName)}</h2>
              </div>

              <div className="stream-day-entry-body">
                <div className="stream-dual-pane">
                  {paneStates.map(({ streamName, editorKey, focusEditorRef, existingPageId, shouldBlur }) => (
                    <div
                      key={editorKey}
                      className="stream-dual-panel"
                      role="button"
                      tabIndex={0}
                      onClick={(event) => handleBrowsePaneClick(event, dateName, streamName)}
                      onKeyDown={(event) => handleBrowsePaneKeyDown(event, dateName, streamName)}
                    >
                      <p className="stream-panel-label">{streamName}</p>
                      <div className={`stream-editor-pane${shouldBlur ? " stream-editor-pane--privacy-blurred" : ""}`}>
                        <StreamSingleEditor
                          streamName={streamName}
                          dateName={dateName}
                          existingPageId={existingPageId}
                          pages={pages}
                          reloadToken={reloadToken}
                          onNavigate={onNavigate}
                          onError={onError}
                          onRefresh={onRefresh}
                          focusEditorRef={focusEditorRef}
                          onFocusChange={(focused) => {
                            if (focused) {
                              openFocusScreen(dateName, streamName);
                            }
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
