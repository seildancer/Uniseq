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
          <p className="stream-mobile-focus-label">{focusedEditor.streamName}</p>
          <h2 className="stream-mobile-focus-title">{formatDateLabel(focusedEditor.dateName)}</h2>
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
  const scrollSettleTimerRef = useRef(null);
  const programmaticScrollTimerRef = useRef(null);
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

  function openFocusScreen(dateName, streamName) {
    if (dateName !== selectedDateRef.current) {
      onSelectDate?.(dateName);
    }
    scrollDateIntoView(dateName, "auto");
    shouldFocusScreenEditorRef.current = true;
    setFocusedEditor({ dateName, streamName });
  }

  useLayoutEffect(() => {
    if (focusedEditorRef.current) {
      return;
    }
    scrollDateIntoView(selectedDate, "auto");
  }, [selectedDate, scrollContainerRef]);

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
    const focusRef = editorFocusRefs.current.get(editorKeyFor(focusedEditor.streamName, focusedEditor.dateName));
    const timerId = window.setTimeout(() => {
      focusRef?.current?.({ atEnd: true });
      shouldFocusScreenEditorRef.current = false;
    }, 0);
    return () => window.clearTimeout(timerId);
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
                      onClick={() => openFocusScreen(dateName, streamName)}
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
