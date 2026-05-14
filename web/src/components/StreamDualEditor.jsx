import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { buildRecentStreamDateWindow, formatDateLabel } from "../utils/streamDates.js";
import { PRIMARY_STREAM_LEFT, PRIMARY_STREAM_RIGHT, streamPageExists, streamPageId } from "../utils/streamWorkspace.js";

const STREAM_DAY_WINDOW = 9;

export default function StreamDualEditor({
  selectedDate,
  streamPagesByDate,
  pages,
  reloadToken,
  onNavigate,
  onError,
  onRefresh,
  diaryBlurEnabled = true,
}) {
  const [mobileTab, setMobileTab] = useState(PRIMARY_STREAM_LEFT);
  const [focusedEditor, setFocusedEditor] = useState(null);
  const dayRefs = useRef(new Map());
  const restoreDateAfterBlurRef = useRef(null);
  const visibleDates = useMemo(
    () => buildRecentStreamDateWindow(selectedDate, STREAM_DAY_WINDOW),
    [selectedDate],
  );

  useEffect(() => {
    if (focusedEditor && !visibleDates.includes(focusedEditor.dateName)) {
      setFocusedEditor(null);
    }
  }, [focusedEditor, visibleDates]);

  useEffect(() => {
    dayRefs.current.get(selectedDate)?.scrollIntoView({
      block: "start",
      behavior: "smooth",
    });
  }, [selectedDate]);

  useLayoutEffect(() => {
    if (focusedEditor) {
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
  }, [focusedEditor]);

  function enterFocusMode(dateName, streamName) {
    restoreDateAfterBlurRef.current = dateName;
    dayRefs.current.get(dateName)?.scrollIntoView({
      block: "start",
      behavior: "auto",
    });
    setFocusedEditor({ dateName, streamName });
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
    <div className="stream-dual-wrap">
      <div className="stream-dual-tabs">
        <button
          type="button"
          className={`stream-dual-tab${mobileTab === PRIMARY_STREAM_LEFT ? " stream-dual-tab--active" : ""}`}
          onClick={() => setMobileTab(PRIMARY_STREAM_LEFT)}
        >
          {PRIMARY_STREAM_LEFT}
        </button>
        <button
          type="button"
          className={`stream-dual-tab${mobileTab === PRIMARY_STREAM_RIGHT ? " stream-dual-tab--active" : ""}`}
          onClick={() => setMobileTab(PRIMARY_STREAM_RIGHT)}
        >
          {PRIMARY_STREAM_RIGHT}
        </button>
      </div>

      <div className={`stream-day-list${focusedEditor ? " stream-day-list--has-focus" : ""}`}>
        {visibleDates.map((dateName) => {
          const focusedStreamName = focusedEditor?.dateName === dateName ? focusedEditor.streamName : null;
          const diaryPageId = streamPageExists(streamPagesByDate, dateName, PRIMARY_STREAM_LEFT)
            ? streamPageId(PRIMARY_STREAM_LEFT, dateName)
            : null;
          const journalsPageId = streamPageExists(streamPagesByDate, dateName, PRIMARY_STREAM_RIGHT)
            ? streamPageId(PRIMARY_STREAM_RIGHT, dateName)
            : null;
          const isSelected = selectedDate === dateName;
          const isEmpty = !diaryPageId && !journalsPageId;
          const shouldBlurDiary = diaryBlurEnabled && Boolean(diaryPageId) && !focusedStreamName;

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
              className={`stream-day-entry${focusedStreamName ? " stream-day-entry--focused" : ""}${isSelected ? " stream-day-entry--selected" : ""}${isEmpty ? " stream-day-entry--empty" : ""}`}
            >
              <div className="stream-day-entry-header">
                <h2 className="stream-day-entry-title">{formatDateLabel(dateName)}</h2>
              </div>

              <div className="stream-day-entry-body">
                <div className="stream-dual-pane">
                  <div
                    className={`stream-dual-panel${mobileTab !== PRIMARY_STREAM_LEFT ? " stream-dual-panel--hidden-mobile" : ""}${focusedStreamName === PRIMARY_STREAM_LEFT ? " stream-dual-panel--focused" : ""}${focusedStreamName && focusedStreamName !== PRIMARY_STREAM_LEFT ? " stream-dual-panel--compressed" : ""}`}
                    onMouseDown={focusPaneEditor}
                  >
                    <p className="stream-panel-label">{PRIMARY_STREAM_LEFT}</p>
                    <div className={`stream-editor-pane${shouldBlurDiary ? " stream-editor-pane--privacy-blurred" : ""}`}>
                      <StreamSingleEditor
                        key={`${PRIMARY_STREAM_LEFT}/${dateName}`}
                        streamName={PRIMARY_STREAM_LEFT}
                        dateName={dateName}
                        existingPageId={diaryPageId}
                        pages={pages}
                        reloadToken={reloadToken}
                        onNavigate={onNavigate}
                        onError={onError}
                        onRefresh={onRefresh}
                        onFocusChange={(focused) => {
                          if (focused) {
                            enterFocusMode(dateName, PRIMARY_STREAM_LEFT);
                            setMobileTab(PRIMARY_STREAM_LEFT);
                            return;
                          }
                          setFocusedEditor((current) => {
                            if (current?.dateName === dateName && current?.streamName === PRIMARY_STREAM_LEFT) {
                              restoreDateAfterBlurRef.current = dateName;
                              return null;
                            }
                            return current;
                          });
                        }}
                      />
                    </div>
                  </div>
                  <div
                    className={`stream-dual-panel${mobileTab !== PRIMARY_STREAM_RIGHT ? " stream-dual-panel--hidden-mobile" : ""}${focusedStreamName === PRIMARY_STREAM_RIGHT ? " stream-dual-panel--focused" : ""}${focusedStreamName && focusedStreamName !== PRIMARY_STREAM_RIGHT ? " stream-dual-panel--compressed" : ""}`}
                    onMouseDown={focusPaneEditor}
                  >
                    <p className="stream-panel-label">{PRIMARY_STREAM_RIGHT}</p>
                    <div className="stream-editor-pane">
                      <StreamSingleEditor
                        key={`${PRIMARY_STREAM_RIGHT}/${dateName}`}
                        streamName={PRIMARY_STREAM_RIGHT}
                        dateName={dateName}
                        existingPageId={journalsPageId}
                        pages={pages}
                        reloadToken={reloadToken}
                        onNavigate={onNavigate}
                        onError={onError}
                        onRefresh={onRefresh}
                        onFocusChange={(focused) => {
                          if (focused) {
                            enterFocusMode(dateName, PRIMARY_STREAM_RIGHT);
                            setMobileTab(PRIMARY_STREAM_RIGHT);
                            return;
                          }
                          setFocusedEditor((current) => {
                            if (current?.dateName === dateName && current?.streamName === PRIMARY_STREAM_RIGHT) {
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
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}
