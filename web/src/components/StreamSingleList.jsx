import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { buildRecentStreamDateWindow, formatDateLabel } from "../utils/streamDates.js";
import { PRIMARY_STREAM_LEFT, streamPageExists, streamPageId } from "../utils/streamWorkspace.js";

const STREAM_DAY_WINDOW = 9;

export default function StreamSingleList({
  streamName,
  selectedDate,
  streamPagesByDate,
  pages,
  reloadToken,
  onNavigate,
  onError,
  onRefresh,
  diaryBlurEnabled = true,
}) {
  const [focusedDateName, setFocusedDateName] = useState(null);
  const dayRefs = useRef(new Map());
  const restoreDateAfterBlurRef = useRef(null);
  const visibleDates = useMemo(
    () => buildRecentStreamDateWindow(selectedDate, STREAM_DAY_WINDOW),
    [selectedDate],
  );

  useEffect(() => {
    if (focusedDateName && !visibleDates.includes(focusedDateName)) {
      setFocusedDateName(null);
    }
  }, [focusedDateName, visibleDates]);

  useEffect(() => {
    dayRefs.current.get(selectedDate)?.scrollIntoView({
      block: "start",
      behavior: "smooth",
    });
  }, [selectedDate]);

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

  return (
    <div className={`stream-day-list${focusedDateName ? " stream-day-list--has-focus" : ""}`}>
      {visibleDates.map((dateName) => {
        const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
          ? streamPageId(streamName, dateName)
          : null;
        const isFocused = focusedDateName === dateName;
        const isSelected = selectedDate === dateName;
        const isEmpty = !existingPageId;
        const shouldBlurDiary = diaryBlurEnabled && Boolean(existingPageId) && streamName === PRIMARY_STREAM_LEFT && !isFocused;

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
            className={`stream-day-entry${isFocused ? " stream-day-entry--focused" : ""}${isSelected ? " stream-day-entry--selected" : ""}${isEmpty ? " stream-day-entry--empty" : ""}`}
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
