import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import StreamSingleEditor from "./StreamSingleEditor";
import { useLazyStreamDateRange } from "../hooks/useLazyStreamDateRange.js";
import { formatDateLabel, maxDateName, todayDateName } from "../utils/streamDates.js";
import {
  isDiaryStream,
  streamPageExists,
  streamPageId,
} from "../utils/streamWorkspace.js";

export default function StreamView({
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
  const isDual = streamNames.length > 1;
  const [mobileTab, setMobileTab] = useState(() => streamNames[0] ?? null);
  const [focusedEditor, setFocusedEditor] = useState(null);
  const [editorReadyByKey, setEditorReadyByKey] = useState(() => new Map());
  const dayRefs = useRef(new Map());
  const editorFocusRefs = useRef(new Map());
  const latestDateName = useMemo(
    () => maxDateName([todayDateName(), selectedDate, ...streamPagesByDate.keys()], selectedDate),
    [selectedDate, streamPagesByDate],
  );
  const { visibleDates: lazyVisibleDates } = useLazyStreamDateRange({
    selectedDate,
    latestDateName,
    scrollContainerRef,
    disabled: Boolean(focusedEditor),
  });
  const visibleDates = useMemo(
    () => lazyVisibleDates,
    [lazyVisibleDates],
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

  function isDateReady(dateName) {
    return streamNames.every((streamName) => {
      const editorKey = `${streamName}/${dateName}`;
      const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
        ? streamPageId(streamName, dateName)
        : null;
      if (!existingPageId) {
        return true;
      }

      const readyState = editorReadyByKey.get(editorKey);
      return readyState?.generation === selectedScrollGenerationRef.current && readyState.ready;
    });
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
      container.scrollTo({ top: container.scrollTop, behavior: "auto" });
    }
  }, [selectedDate, scrollContainerRef]);

  useEffect(() => () => {
    cancelSelectedScrollFrame();
  }, []);

  useEffect(() => {
    if (focusedEditor && !visibleDates.includes(focusedEditor.dateName)) {
      setFocusedEditor(null);
    }
  }, [focusedEditor, visibleDates]);

  useEffect(() => {
    if (!isDual) return;
    if (mobileTab && streamNames.includes(mobileTab)) {
      return;
    }
    setMobileTab(streamNames[0] ?? null);
  }, [isDual, streamNames, mobileTab]);

  useEffect(() => {
    if (focusedEditor && !streamNames.includes(focusedEditor.streamName)) {
      setFocusedEditor(null);
    }
  }, [streamNames, focusedEditor]);

  useLayoutEffect(() => {
    if (focusedEditor) {
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
  }, [focusedEditor, selectedDate, visibleDates, editorReadyByKey, streamNames, streamPagesByDate]);

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
    <div className="stream-dual-wrap">
      {isDual ? (
        <div className="stream-dual-tabs">
          {streamNames.map((streamName) => (
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
          const focusedStreamIndex = focusedStreamName
            ? streamNames.findIndex((streamName) => streamName === focusedStreamName)
            : -1;
          const isSelected = selectedDate === dateName;
          const paneStates = streamNames.map((streamName) => {
            const editorKey = `${streamName}/${dateName}`;
            const existingPageId = streamPageExists(streamPagesByDate, dateName, streamName)
              ? streamPageId(streamName, dateName)
              : null;
            const focusEditorRef = editorFocusRefForKey(editorKey);
            const readyState = editorReadyByKey.get(editorKey);
            const isReady = !existingPageId
              || (readyState?.generation === selectedScrollGenerationRef.current && readyState.ready);
            const shouldBlur = diaryBlurEnabled
              && isDiaryStream(streamName)
              && Boolean(existingPageId)
              && focusedStreamName !== streamName;

            return {
              streamName,
              editorKey,
              focusEditorRef,
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
                {isDual ? (
                  <div
                    className={`stream-dual-pane stream-dual-pane--dual${focusedStreamIndex === 1 ? " stream-dual-pane--focus-right" : ""}`}
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
                                setMobileTab(streamName);
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
                ) : (
                  paneStates.map(({ streamName, editorKey, focusEditorRef, existingPageId, shouldBlur }) => (
                    <div
                      key={editorKey}
                      className="stream-single-pane"
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
                              setMobileTab(streamName);
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
                  ))
                )}
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}
