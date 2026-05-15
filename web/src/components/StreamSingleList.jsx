import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { useLazyStreamDateRange } from "../hooks/useLazyStreamDateRange.js";
import { useMobileStreamDatePager } from "../hooks/useMobileStreamDatePager.js";
import { formatDateLabel, maxDateName, todayDateName } from "../utils/streamDates.js";
import { isDiaryStream, streamPageExists, streamPageId } from "../utils/streamWorkspace.js";

export default function StreamSingleList({
  streamName,
  selectedDate,
  isMobile = false,
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
  const [focusedDateName, setFocusedDateName] = useState(null);
  const [editorReadyByKey, setEditorReadyByKey] = useState(() => new Map());
  const dayRefs = useRef(new Map());
  const editorFocusRef = useRef(null);
  const restoreDateAfterBlurRef = useRef(null);
  const latestDateName = useMemo(
    () => maxDateName([todayDateName(), selectedDate, ...streamPagesByDate.keys()], selectedDate),
    [selectedDate, streamPagesByDate],
  );
  const { visibleDates: lazyVisibleDates } = useLazyStreamDateRange({
    selectedDate,
    latestDateName,
    scrollContainerRef,
    disabled: isMobile || Boolean(focusedDateName),
  });
  const visibleDates = useMemo(
    () => (isMobile ? [selectedDate] : lazyVisibleDates),
    [isMobile, lazyVisibleDates, selectedDate],
  );
  const pendingSelectedScrollRef = useRef(selectedDate);
  const selectedScrollStartedRef = useRef(false);
  const selectedScrollGenerationRef = useRef(0);
  const selectedScrollRafRef = useRef(null);

  function cancelSelectedScrollFrame() {
    if (selectedScrollRafRef.current !== null) {
      cancelAnimationFrame(selectedScrollRafRef.current);
      selectedScrollRafRef.current = null;
    }
  }

  useMobileStreamDatePager({
    enabled: isMobile,
    selectedDate,
    latestDateName,
    scrollContainerRef,
    onSelectDate,
  });

  function isDateReady(dateName) {
    const editorKey = `${streamName}/${dateName}`;
    const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
      ? streamPageId(streamName, dateName)
      : null;
    if (!existingPageId) {
      return true;
    }

    const readyState = editorReadyByKey.get(editorKey);
    return readyState?.generation === selectedScrollGenerationRef.current && readyState.ready;
  }

  function areDatesReadyThrough(dateName) {
    const targetIndex = visibleDates.indexOf(dateName);
    if (targetIndex < 0) {
      return false;
    }

    for (let index = 0; index <= targetIndex; index += 1) {
      if (!isDateReady(visibleDates[index])) {
        return false;
      }
    }
    return true;
  }

  function scrollDateIntoView(dateName, behavior) {
    const node = dayRefs.current.get(dateName);
    if (!node) {
      return;
    }

    const container = scrollContainerRef.current;
    if (!container) {
      node.scrollIntoView({ block: "start", behavior });
      return;
    }

    const containerRect = container.getBoundingClientRect();
    const nodeRect = node.getBoundingClientRect();
    container.scrollTo({
      top: container.scrollTop + nodeRect.top - containerRect.top,
      behavior,
    });
  }

  function queueFinalSelectedScroll(dateName, generation) {
    cancelSelectedScrollFrame();
    selectedScrollRafRef.current = requestAnimationFrame(() => {
      selectedScrollRafRef.current = null;
      if (selectedScrollGenerationRef.current !== generation) {
        return;
      }
      scrollDateIntoView(dateName, "auto");
    });
  }

  useLayoutEffect(() => {
    selectedScrollGenerationRef.current += 1;
    pendingSelectedScrollRef.current = selectedDate;
    selectedScrollStartedRef.current = false;
    cancelSelectedScrollFrame();

    const container = scrollContainerRef.current;
    if (container) {
      container.scrollTo({ top: isMobile ? 0 : container.scrollTop, behavior: "auto" });
    }
  }, [isMobile, selectedDate, scrollContainerRef]);

  useEffect(() => () => {
    cancelSelectedScrollFrame();
  }, []);

  useEffect(() => {
    if (focusedDateName && !visibleDates.includes(focusedDateName)) {
      setFocusedDateName(null);
    }
  }, [focusedDateName, visibleDates]);

  useLayoutEffect(() => {
    if (focusedDateName) {
      return;
    }
    const dateName = pendingSelectedScrollRef.current;
    if (!dateName || !visibleDates.includes(dateName)) {
      return;
    }
    const didStartScroll = selectedScrollStartedRef.current;
    const datesReady = areDatesReadyThrough(dateName);
    const generation = selectedScrollGenerationRef.current;

    scrollDateIntoView(dateName, didStartScroll ? "auto" : "smooth");
    selectedScrollStartedRef.current = true;

    if (datesReady) {
      pendingSelectedScrollRef.current = null;
      selectedScrollStartedRef.current = false;
      if (didStartScroll) {
        queueFinalSelectedScroll(dateName, generation);
      }
    }
  }, [focusedDateName, selectedDate, visibleDates, editorReadyByKey, streamName, streamPagesByDate]);

  useLayoutEffect(() => {
    if (focusedDateName) {
      return;
    }
    const dateName = restoreDateAfterBlurRef.current;
    if (!dateName) {
      return;
    }
    restoreDateAfterBlurRef.current = null;
    dayRefs.current.get(dateName)?.scrollIntoView({
      block: "start",
      behavior: "auto",
    });
  }, [focusedDateName]);

  function enterFocusMode(dateName) {
    restoreDateAfterBlurRef.current = dateName;
    dayRefs.current.get(dateName)?.scrollIntoView({
      block: "start",
      behavior: "auto",
    });
    setFocusedDateName(dateName);
  }

  function focusPaneEditor(event) {
    if (event.target.closest?.(".ProseMirror")) {
      return;
    }
    if (event.button !== undefined && event.button !== 0) {
      return;
    }

    const didFocus = editorFocusRef.current?.({ atEnd: true });
    if (!didFocus) {
      event.currentTarget.querySelector(".ProseMirror")?.focus();
    }
    if (event.pointerType === "mouse") {
      event.preventDefault();
    }
  }

  function handleEditorReadyChange(editorKey, ready) {
    setEditorReadyByKey((current) => {
      const nextReadyState = {
        generation: selectedScrollGenerationRef.current,
        ready,
      };
      const currentReadyState = current.get(editorKey);
      if (
        currentReadyState?.generation === nextReadyState.generation
        && currentReadyState?.ready === nextReadyState.ready
      ) {
        return current;
      }

      const next = new Map(current);
      next.set(editorKey, nextReadyState);
      return next;
    });
  }

  return (
    <div className={`stream-day-list${isMobile ? " stream-day-list--mobile-single" : ""}${focusedDateName ? " stream-day-list--has-focus" : ""}`}>
      {visibleDates.map((dateName) => {
        const editorKey = `${streamName}/${dateName}`;
        const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
          ? streamPageId(streamName, dateName)
          : null;
        const isFocused = focusedDateName === dateName;
        const isSelected = selectedDate === dateName;
        const isEmpty = !existingPageId;
        const readyState = editorReadyByKey.get(editorKey);
        const isReady = !existingPageId
          || (readyState?.generation === selectedScrollGenerationRef.current && readyState.ready);
        const shouldBlurDiary = diaryBlurEnabled && Boolean(existingPageId) && isDiaryStream(streamName) && !isFocused;

        return (
          <section
            key={`${streamName}/${dateName}`}
            ref={(node) => {
              if (node) {
                dayRefs.current.set(dateName, node);
              } else {
                dayRefs.current.delete(dateName);
              }
            }}
            className={`stream-day-entry${isFocused ? " stream-day-entry--focused" : ""}${isSelected ? " stream-day-entry--selected" : ""}${isEmpty ? " stream-day-entry--empty" : ""}${isReady ? " stream-day-entry--ready" : " stream-day-entry--loading"}`}
          >
            <div className="stream-day-entry-header">
              <h2 className="stream-day-entry-title">{formatDateLabel(dateName)}</h2>
            </div>
            <div className="stream-day-entry-body">
              <div className="stream-single-pane" onPointerDown={focusPaneEditor}>
                <p className="stream-panel-label">{streamName}</p>
                <div className={`stream-editor-pane${shouldBlurDiary ? " stream-editor-pane--privacy-blurred" : ""}`}>
                  <StreamSingleEditor
                    streamName={streamName}
                    dateName={dateName}
                    existingPageId={existingPageId}
                    pages={pages}
                    reloadToken={reloadToken}
                    onNavigate={onNavigate}
                    onError={onError}
                    onRefresh={onRefresh}
                    focusEditorRef={editorFocusRef}
                    onReadyChange={(ready) => handleEditorReadyChange(editorKey, ready)}
                    onFocusChange={(focused) => {
                      if (focused) {
                        enterFocusMode(dateName);
                        return;
                      }
                      setFocusedDateName((current) => {
                        if (current === dateName) {
                          restoreDateAfterBlurRef.current = dateName;
                          return null;
                        }
                        return current;
                      });
                    }}
                  />
                </div>
              </div>
            </div>
          </section>
        );
      })}
    </div>
  );
}
