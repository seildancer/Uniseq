import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { useLazyStreamDateRange } from "../hooks/useLazyStreamDateRange.js";
import { formatDateLabel, maxDateName, todayDateName } from "../utils/streamDates.js";
import {
  DIARY_STREAM,
  PRIMARY_STREAM_LEFT,
  PRIMARY_STREAM_RIGHT,
  streamPageExists,
  streamPageId,
} from "../utils/streamWorkspace.js";

export default function StreamDualEditor({
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
  const [mobileTab, setMobileTab] = useState(PRIMARY_STREAM_LEFT);
  const [focusedEditor, setFocusedEditor] = useState(null);
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
    disabled: Boolean(focusedEditor),
  });
  const pendingSelectedScrollRef = useRef(selectedDate);
  const lastSelectedDateRef = useRef(selectedDate);

  if (lastSelectedDateRef.current !== selectedDate) {
    lastSelectedDateRef.current = selectedDate;
    pendingSelectedScrollRef.current = selectedDate;
  }

  useEffect(() => {
    if (focusedEditor && !visibleDates.includes(focusedEditor.dateName)) {
      setFocusedEditor(null);
    }
  }, [focusedEditor, visibleDates]);

  useLayoutEffect(() => {
    if (focusedEditor) {
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
  }, [focusedEditor, selectedDate, visibleDates]);

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
          const leftEditorKey = `${PRIMARY_STREAM_LEFT}/${dateName}`;
          const rightEditorKey = `${PRIMARY_STREAM_RIGHT}/${dateName}`;
          const focusedStreamName = focusedEditor?.dateName === dateName ? focusedEditor.streamName : null;
          const leftPageId = streamPageExists(streamPagesByDate, dateName, PRIMARY_STREAM_LEFT)
            ? streamPageId(PRIMARY_STREAM_LEFT, dateName)
            : null;
          const rightPageId = streamPageExists(streamPagesByDate, dateName, PRIMARY_STREAM_RIGHT)
            ? streamPageId(PRIMARY_STREAM_RIGHT, dateName)
            : null;
          const isSelected = selectedDate === dateName;
          const isEmpty = !leftPageId && !rightPageId;
          const isLeftReady = editorReadyByKey.get(leftEditorKey) ?? !leftPageId;
          const isRightReady = editorReadyByKey.get(rightEditorKey) ?? !rightPageId;
          const isReady = isLeftReady && isRightReady;
          const shouldBlurRight = diaryBlurEnabled
            && PRIMARY_STREAM_RIGHT === DIARY_STREAM
            && Boolean(rightPageId)
            && !focusedStreamName;

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
              className={`stream-day-entry${focusedStreamName ? " stream-day-entry--focused" : ""}${isSelected ? " stream-day-entry--selected" : ""}${isEmpty ? " stream-day-entry--empty" : ""}${isReady ? " stream-day-entry--ready" : " stream-day-entry--loading"}`}
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
                    <div className="stream-editor-pane">
                      <StreamSingleEditor
                        key={`${PRIMARY_STREAM_LEFT}/${dateName}`}
                        streamName={PRIMARY_STREAM_LEFT}
                        dateName={dateName}
                        existingPageId={leftPageId}
                        pages={pages}
                        reloadToken={reloadToken}
                        onNavigate={onNavigate}
                        onError={onError}
                        onRefresh={onRefresh}
                        onReadyChange={(ready) => handleEditorReadyChange(leftEditorKey, ready)}
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
                    <div className={`stream-editor-pane${shouldBlurRight ? " stream-editor-pane--privacy-blurred" : ""}`}>
                      <StreamSingleEditor
                        key={`${PRIMARY_STREAM_RIGHT}/${dateName}`}
                        streamName={PRIMARY_STREAM_RIGHT}
                        dateName={dateName}
                        existingPageId={rightPageId}
                        pages={pages}
                        reloadToken={reloadToken}
                        onNavigate={onNavigate}
                        onError={onError}
                        onRefresh={onRefresh}
                        onReadyChange={(ready) => handleEditorReadyChange(rightEditorKey, ready)}
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
