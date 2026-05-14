import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { useLazyStreamDateRange } from "../hooks/useLazyStreamDateRange.js";
import { formatDateLabel, maxDateName, todayDateName } from "../utils/streamDates.js";
import {
  isDiaryStream,
  streamPageExists,
  streamPageId,
} from "../utils/streamWorkspace.js";

export default function StreamDualEditor({
  selectedDate,
  dualStreamNames,
  streamPagesByDate,
  pages,
  reloadToken,
  scrollContainerRef,
  onNavigate,
  onError,
  onRefresh,
  diaryBlurEnabled = true,
}) {
  const [mobileTab, setMobileTab] = useState(() => dualStreamNames[0] ?? null);
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

  useEffect(() => {
    if (mobileTab && dualStreamNames.includes(mobileTab)) {
      return;
    }
    setMobileTab(dualStreamNames[0] ?? null);
  }, [dualStreamNames, mobileTab]);

  useEffect(() => {
    if (focusedEditor && !dualStreamNames.includes(focusedEditor.streamName)) {
      setFocusedEditor(null);
    }
  }, [dualStreamNames, focusedEditor]);

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
      {dualStreamNames.length > 0 ? (
        <div className="stream-dual-tabs">
          {dualStreamNames.map((streamName) => (
            <button
              key={streamName}
              type="button"
              className={`stream-dual-tab${mobileTab === streamName ? " stream-dual-tab--active" : ""}`}
              onClick={() => setMobileTab(streamName)}
            >
              {streamName}
            </button>
          ))}
        </div>
      ) : null}

      <div className={`stream-day-list${focusedEditor ? " stream-day-list--has-focus" : ""}`}>
        {visibleDates.map((dateName) => {
          const focusedStreamName = focusedEditor?.dateName === dateName ? focusedEditor.streamName : null;
          const isSelected = selectedDate === dateName;
          const paneStates = dualStreamNames.map((streamName) => {
            const editorKey = `${streamName}/${dateName}`;
            const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
              ? streamPageId(streamName, dateName)
              : null;
            const isReady = editorReadyByKey.get(editorKey) ?? !existingPageId;
            const shouldBlur = diaryBlurEnabled
              && isDiaryStream(streamName)
              && Boolean(existingPageId)
              && focusedStreamName !== streamName;

            return {
              streamName,
              editorKey,
              existingPageId,
              isReady,
              shouldBlur,
            };
          });
          const isEmpty = paneStates.every((pane) => !pane.existingPageId);
          const isReady = paneStates.every((pane) => pane.isReady);

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
                  {paneStates.map(({ streamName, editorKey, existingPageId, shouldBlur }) => (
                    <div
                      key={editorKey}
                      className={`stream-dual-panel${mobileTab !== streamName ? " stream-dual-panel--hidden-mobile" : ""}${focusedStreamName === streamName ? " stream-dual-panel--focused" : ""}${focusedStreamName && focusedStreamName !== streamName ? " stream-dual-panel--compressed" : ""}`}
                      onMouseDown={focusPaneEditor}
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
                          onReadyChange={(ready) => handleEditorReadyChange(editorKey, ready)}
                          onFocusChange={(focused) => {
                            if (focused) {
                              enterFocusMode(dateName, streamName);
                              setMobileTab(streamName);
                              return;
                            }
                            setFocusedEditor((current) => {
                              if (current?.dateName === dateName && current?.streamName === streamName) {
                                restoreDateAfterBlurRef.current = dateName;
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
