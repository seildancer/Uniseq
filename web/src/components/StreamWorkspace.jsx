import { useEffect, useRef, useState } from "react";
import { breadcrumbItemsForStreamSelection } from "./EditorBreadcrumb.jsx";
import SidebarCalendar from "./SidebarCalendar.jsx";
import StreamDualEditor from "./StreamDualEditor.jsx";
import StreamSingleList from "./StreamSingleList.jsx";
import {
  isDiaryStream,
  orderStreamNamesForDisplay,
  PRIMARY_STREAM_NAMES,
  selectionForCalendarDate,
} from "../utils/streamWorkspace.js";

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
  onCreateStream,
  onDeleteStream,
  onNavigatePage,
  onError,
  onRefresh,
  panelChrome,
}) {
  const sidebarRef = useRef(null);
  const editorScrollRef = useRef(null);
  const resizeStateRef = useRef(null);
  const createInputRef = useRef(null);
  const [isCreating, setIsCreating] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [streamMenuOpenFor, setStreamMenuOpenFor] = useState(null);
  const [deleteConfirmStream, setDeleteConfirmStream] = useState(null);

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

  useEffect(() => {
    if (isCreating) {
      createInputRef.current?.focus();
    }
  }, [isCreating]);

  function startCreating() {
    setDraftName("");
    setIsCreating(true);
  }

  function cancelCreating() {
    setIsCreating(false);
    setDraftName("");
  }

  async function submitCreate() {
    const name = draftName.trim();
    cancelCreating();
    if (name) {
      await onCreateStream?.(name);
    }
  }

  useEffect(() => {
    if (!streamMenuOpenFor) return;
    function handleClickOutside(event) {
      if (!event.target.closest(".stream-menu-wrap")) {
        setStreamMenuOpenFor(null);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [streamMenuOpenFor]);

  async function handleConfirmDeleteStream() {
    const streamName = deleteConfirmStream;
    setDeleteConfirmStream(null);
    await onDeleteStream?.(streamName);
  }

  function streamNoteCount(sName) {
    let count = 0;
    for (const set of streamPagesByDate.values()) {
      if (set.has(sName)) count++;
    }
    return count;
  }

  function handleCalendarSelect(dateName) {
    const nextSelection = selectionForCalendarDate(streamSelection, dateName);
    if (nextSelection.kind === "stream_single") {
      onSelectStreamSingle(nextSelection.streamName, nextSelection.dateName);
      return;
    }
    onSelectStreamDual(nextSelection.dateName);
  }

  const streamEditor = streamSelection
    ? (
      streamSelection.kind === "stream_dual" ? (
        <StreamDualEditor
          selectedDate={selectedStreamDate}
          streamPagesByDate={streamPagesByDate}
          pages={regularPages}
          reloadToken={streamReloadToken}
          scrollContainerRef={editorScrollRef}
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
          scrollContainerRef={editorScrollRef}
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
              <button
                type="button"
                className="stream-add-btn"
                title="New stream"
                onClick={startCreating}
              >
                +
              </button>
            </div>

            <div className="sidebar-section-scroll">
              {streamNames.length > 0 ? (
                <ul className="stream-list">
                  {orderStreamNamesForDisplay(streamNames).map((streamName) => {
                    const isDiary = isDiaryStream(streamName);
                    const isPrimary = PRIMARY_STREAM_NAMES.includes(streamName);
                    const isMenuOpen = streamMenuOpenFor === streamName;

                    return (
                      <li key={streamName} className={`stream-list-item${isDiary ? " stream-list-item--with-toggle" : ""}${!isPrimary ? " stream-list-item--with-menu" : ""}${isMenuOpen ? " stream-list-item--menu-open" : ""}`}>
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
                        {!isPrimary ? (
                          <div className="stream-menu-wrap">
                            <button
                              type="button"
                              className={`stream-menu-btn${isMenuOpen ? " stream-menu-btn--open" : ""}`}
                              aria-label="Stream options"
                              aria-expanded={isMenuOpen}
                              onClick={(e) => {
                                e.stopPropagation();
                                setStreamMenuOpenFor((current) => current === streamName ? null : streamName);
                              }}
                            >
                              <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true">
                                <circle cx="3" cy="8" r="1.5" />
                                <circle cx="8" cy="8" r="1.5" />
                                <circle cx="13" cy="8" r="1.5" />
                              </svg>
                            </button>
                            {isMenuOpen && (
                              <div className="stream-dropdown">
                                <button
                                  type="button"
                                  className="stream-dropdown-item stream-dropdown-item--danger"
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    setStreamMenuOpenFor(null);
                                    setDeleteConfirmStream(streamName);
                                  }}
                                >
                                  Delete stream
                                </button>
                              </div>
                            )}
                          </div>
                        ) : null}
                      </li>
                    );
                  })}
                </ul>
              ) : null}

              {isCreating ? (
                <div className="stream-create-row">
                  <input
                    ref={createInputRef}
                    className="stream-create-input"
                    type="text"
                    placeholder="Stream name"
                    value={draftName}
                    onChange={(e) => setDraftName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") { e.preventDefault(); void submitCreate(); }
                      if (e.key === "Escape") { cancelCreating(); }
                    }}
                    onBlur={cancelCreating}
                  />
                </div>
              ) : null}

              <SidebarCalendar
                selectedDate={selectedStreamDate}
                streamPagesByDate={streamPagesByDate}
                onSelectDate={handleCalendarSelect}
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
          {panelChrome(breadcrumbItemsForStreamSelection(streamSelection))}
          <div ref={editorScrollRef} className="editor-panel-scroll">
            {streamEditor}
          </div>
        </section>
      ) : fallbackEditor}

      {deleteConfirmStream && (
        <div className="modal-overlay" onClick={() => setDeleteConfirmStream(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Delete stream</h3>
            <p>
              Delete <strong>{deleteConfirmStream}</strong> and all {streamNoteCount(deleteConfirmStream)} {streamNoteCount(deleteConfirmStream) === 1 ? "note" : "notes"}? This cannot be undone.
            </p>
            <div className="modal-actions">
              <button
                className="secondary-button"
                type="button"
                onClick={() => setDeleteConfirmStream(null)}
              >
                Cancel
              </button>
              <button
                className="primary-button"
                type="button"
                onClick={() => void handleConfirmDeleteStream()}
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
