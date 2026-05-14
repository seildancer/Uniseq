import { useEffect, useRef } from "react";
import SidebarCalendar from "./SidebarCalendar.jsx";
import StreamDualEditor from "./StreamDualEditor.jsx";
import StreamSingleList from "./StreamSingleList.jsx";
import { PRIMARY_STREAM_LEFT } from "../utils/streamWorkspace.js";

const SIDEBAR_MIN_WIDTH_PX = 280;

export default function StreamWorkspace({
  streamSelection,
  selectedStreamDate,
  streamNames,
  streamPagesByDate,
  regularPages,
  streamReloadToken,
  diaryBlurEnabled,
  onDiaryBlurToggle,
  onSidebarWidthChange,
  sidebarCollapsed,
  sidebarChrome,
  pageSidebarContent,
  fallbackEditor,
  onSelectStreamDual,
  onSelectStreamSingle,
  onNavigatePage,
  onError,
  onRefresh,
  panelChrome,
}) {
  const sidebarRef = useRef(null);
  const resizeStateRef = useRef(null);

  useEffect(() => {
    return () => {
      if (resizeStateRef.current) {
        window.removeEventListener("pointermove", resizeStateRef.current.handlePointerMove);
        window.removeEventListener("pointerup", resizeStateRef.current.handlePointerUp);
        document.body.classList.remove("sidebar-resizing");
      }
    };
  }, []);

  function stopSidebarResize() {
    if (!resizeStateRef.current) {
      return;
    }
    window.removeEventListener("pointermove", resizeStateRef.current.handlePointerMove);
    window.removeEventListener("pointerup", resizeStateRef.current.handlePointerUp);
    resizeStateRef.current = null;
    document.body.classList.remove("sidebar-resizing");
  }

  function startSidebarResize(event) {
    if (event.button !== 0 || !sidebarRef.current || typeof onSidebarWidthChange !== "function") {
      return;
    }

    const sidebarLeft = sidebarRef.current.getBoundingClientRect().left;
    const handlePointerMove = (moveEvent) => {
      const nextWidth = Math.max(SIDEBAR_MIN_WIDTH_PX, moveEvent.clientX - sidebarLeft);
      onSidebarWidthChange(nextWidth);
    };
    const handlePointerUp = () => {
      stopSidebarResize();
    };

    resizeStateRef.current = {
      handlePointerMove,
      handlePointerUp,
    };

    document.body.classList.add("sidebar-resizing");
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    handlePointerMove(event);
    event.preventDefault();
  }

  const streamEditor = streamSelection
    ? (
      streamSelection.kind === "stream_dual" ? (
        <StreamDualEditor
          selectedDate={selectedStreamDate}
          streamPagesByDate={streamPagesByDate}
          pages={regularPages}
          reloadToken={streamReloadToken}
          onNavigate={onNavigatePage}
          onError={onError}
          onRefresh={onRefresh}
          diaryBlurEnabled={diaryBlurEnabled}
        />
      ) : (
        <StreamSingleList
          streamName={streamSelection.streamName}
          selectedDate={selectedStreamDate}
          streamPagesByDate={streamPagesByDate}
          pages={regularPages}
          reloadToken={streamReloadToken}
          onNavigate={onNavigatePage}
          onError={onError}
          onRefresh={onRefresh}
          diaryBlurEnabled={diaryBlurEnabled}
        />
      )
    )
    : null;

  return (
    <>
      <aside
        ref={sidebarRef}
        className={`workspace-sidebar${sidebarCollapsed ? " workspace-sidebar--collapsed" : ""}`}
      >
        {sidebarChrome}
        <div className="sidebar-content">
          <div className="sidebar-section sidebar-section--streams">
            <div className="section-heading">
              <button
                type="button"
                className={`stream-section-title${streamSelection?.kind === "stream_dual" ? " stream-section-title--active" : ""}`}
                onClick={() => onSelectStreamDual(selectedStreamDate)}
              >
                Streams
              </button>
            </div>

            <div className="sidebar-section-scroll">
              {streamNames.length > 0 ? (
                <ul className="stream-list">
                  {streamNames.map((streamName) => {
                    const isDiary = streamName === PRIMARY_STREAM_LEFT;

                    return (
                      <li key={streamName} className={`stream-list-item${isDiary ? " stream-list-item--with-toggle" : ""}`}>
                        <button
                          type="button"
                          className={`stream-list-btn${streamSelection?.kind === "stream_single" && streamSelection.streamName === streamName
                            ? " stream-list-btn--active"
                            : ""
                            }${isDiary ? " stream-list-btn--with-toggle" : ""}`}
                          onClick={() => onSelectStreamSingle(streamName, selectedStreamDate)}
                        >
                          {streamName}
                        </button>
                        {isDiary ? (
                          <button
                            type="button"
                            className={`stream-blur-toggle${diaryBlurEnabled ? " stream-blur-toggle--active" : ""}`}
                            aria-pressed={diaryBlurEnabled}
                            title={diaryBlurEnabled ? "Diary blur is on" : "Diary blur is off"}
                            onClick={onDiaryBlurToggle}
                          >
                            blur
                          </button>
                        ) : null}
                      </li>
                    );
                  })}
                </ul>
              ) : null}

              <SidebarCalendar
                selectedDate={selectedStreamDate}
                streamPagesByDate={streamPagesByDate}
                onSelectDate={onSelectStreamDual}
              />
            </div>
          </div>
          {pageSidebarContent}
        </div>
      </aside>

      <div
        className="workspace-resizer"
        aria-hidden="true"
        onPointerDown={startSidebarResize}
      />

      {streamSelection ? (
        <section className="editor-panel editor-panel--stream">
          {panelChrome}
          <div className="editor-panel-scroll">
            {streamEditor}
          </div>
        </section>
      ) : fallbackEditor}
    </>
  );
}
