import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { useLazyStreamDateRange } from "../hooks/useLazyStreamDateRange.js";
import { formatDateLabel, maxDateName, todayDateName } from "../utils/streamDates.js";
import { isDiaryStream, streamPageExists, streamPageId } from "../utils/streamWorkspace.js";

export default function StreamSingleList({
  streamName,
  selectedDate,
  streamPagesByDate,
  pages,
  reloadToken,
  scrollContainerRef,
  onNavigate,
  onError,
  onRefresh,
  diaryBlurEnabled = true,
}) {
  const [focusedDateName, setFocusedDateName] = useState(null);
  const [editorReadyByKey, setEditorReadyByKey] = useState(() => new Map());
  const dayRefs = useRef(new Map());
  const restoreDateAfterBlurRef = useRef(null);
  const latestDateName = useMemo(
    () => maxDateName([todayDateName(), selectedDate, ...streamPagesByDate.keys()], selectedDate),
    [selectedDate, streamPagesByDate],
  );
  const { visibleDates } = useLazyStreamDateRange({
    selectedDate,
    latestDateName,
    scrollContainerRef,
    disabled: Boolean(focusedDateName),
  });
  const pendingSelectedScrollRef = useRef(selectedDate);
  const lastSelectedDateRef = useRef(selectedDate);

  if (lastSelectedDateRef.current !== selectedDate) {
    lastSelectedDateRef.current = selectedDate;
    pendingSelectedScrollRef.current = selectedDate;
  }

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
    pendingSelectedScrollRef.current = null;
    dayRefs.current.get(dateName)?.scrollIntoView({
      block: "start",
      behavior: "smooth",
    });
  }, [focusedDateName, selectedDate, visibleDates]);

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
    const editor = event.currentTarget.querySelector(".ProseMirror");
    if (!editor) {
      return;
    }
    event.preventDefault();
    editor.focus();
  }

  function handleEditorReadyChange(editorKey, ready) {
    setEditorReadyByKey((current) => {
      if (current.get(editorKey) === ready) {
        return current;
      }

      const next = new Map(current);
      next.set(editorKey, ready);
      return next;
    });
  }

  return (
    <div className={`stream-day-list${focusedDateName ? " stream-day-list--has-focus" : ""}`}>
      {visibleDates.map((dateName) => {
        const editorKey = `${streamName}/${dateName}`;
        const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
          ? streamPageId(streamName, dateName)
          : null;
        const isFocused = focusedDateName === dateName;
        const isSelected = selectedDate === dateName;
        const isEmpty = !existingPageId;
        const isReady = editorReadyByKey.get(editorKey) ?? !existingPageId;
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
              <div className="stream-single-pane" onMouseDown={focusPaneEditor}>
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
