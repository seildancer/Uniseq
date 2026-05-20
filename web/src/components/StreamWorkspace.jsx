import { useEffect, useRef, useState } from "react";
import { breadcrumbItemsForStreamSelection } from "./EditorBreadcrumb.jsx";
import SidebarCalendar from "./SidebarCalendar.jsx";
import StreamView from "./StreamView.jsx";
import { areArraysEqual } from "../utils/arrays.js";
import {
  isDiaryStream,
  PRIMARY_STREAM_NAMES,
  selectionForCalendarDate,
} from "../utils/streamWorkspace.js";
import { attachTouchDragListeners, startLongPressTouchDrag } from "../utils/touchDrag.js";

const SIDEBAR_MIN_WIDTH_PX = 280;
const STREAM_DRAG_LONG_PRESS_MS = 260;
const STREAM_DRAG_MOVE_SLOP_PX = 8;

export default function StreamWorkspace({
  streamSelection,
  selectedStreamDate,
  isMobile = false,
  isKeyboardVisible = false,
  orderedStreamNames,
  dualStreamNames,
  streamPagesByDate,
  regularPages,
  streamReloadToken,
  diaryBlurEnabled,
  hideEmptyDates,
  onDiaryBlurToggle,
  onSidebarWidthChange,
  sidebarCollapsed,
  sidebarChrome,
  sidebarCollapsedToggle,
  pageSidebarContent,
  fallbackEditor,
  onSelectStreamDual,
  onSelectStreamSingle,
  onCreateStream,
  onDeleteStream,
  onRenameStream,
  onReorderStreams,
  onNavigatePage,
  onError,
  onRefresh,
  panelChrome,
}) {
  const sidebarRef = useRef(null);
  const editorScrollRef = useRef(null);
  const resizeStateRef = useRef(null);
  const createInputRef = useRef(null);
  const dragLongPressTimerRef = useRef(null);
  const suppressStreamClickRef = useRef(false);
  const [isCreating, setIsCreating] = useState(false);
  const [draftName, setDraftName] = useState("");
  const [streamMenuOpenFor, setStreamMenuOpenFor] = useState(null);
  const [deleteConfirmStream, setDeleteConfirmStream] = useState(null);
  const [streamDragState, setStreamDragState] = useState(null);

  useEffect(() => {
    return () => {
      if (resizeStateRef.current) {
        window.removeEventListener("pointermove", resizeStateRef.current.handlePointerMove);
        window.removeEventListener("pointerup", resizeStateRef.current.handlePointerUp);
        document.body.classList.remove("sidebar-resizing");
      }
      if (dragLongPressTimerRef.current) {
        clearTimeout(dragLongPressTimerRef.current);
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

  function handleCreateRowBlur(event) {
    if (event.currentTarget.contains(event.relatedTarget)) {
      return;
    }
    if (!draftName.trim()) {
      cancelCreating();
    }
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
    ? (() => {
      const streamNames = streamSelection.kind === "stream_dual"
        ? dualStreamNames
        : [streamSelection.streamName];

      const editorProps = {
        isMobile,
        selectedDate: selectedStreamDate,
        streamNames,
        streamPagesByDate,
        pages: regularPages,
        reloadToken: streamReloadToken,
        scrollContainerRef: editorScrollRef,
        onNavigate: onNavigatePage,
        onError,
        onRefresh,
        diaryBlurEnabled,
        hideEmptyDates,
      };

      return <StreamView {...editorProps} />;
    })()
    : null;

  function clearPendingStreamDragState() {
    if (dragLongPressTimerRef.current) {
      clearTimeout(dragLongPressTimerRef.current);
      dragLongPressTimerRef.current = null;
    }
  }

  function handleSelectStream(streamName) {
    if (suppressStreamClickRef.current) {
      suppressStreamClickRef.current = false;
      return;
    }
    onSelectStreamSingle(streamName, selectedStreamDate);
  }

  function handleStreamDragPointerDown(event, streamName) {
    if (event.pointerType !== "mouse") {
      return;
    }

    clearPendingStreamDragState();

    const nextDragState = {
      streamName,
      pointerId: event.pointerId,
      pointerType: "mouse",
      startX: event.clientX,
      startY: event.clientY,
      clientX: event.clientX,
      clientY: event.clientY,
      hover: null,
      active: false,
    };

    setStreamDragState(nextDragState);
  }

  function handleStreamDragTouchStart(event, streamName) {
    if (event.touches.length !== 1) {
      return;
    }

    clearPendingStreamDragState();
    dragLongPressTimerRef.current = startLongPressTouchDrag({
      event,
      longPressMs: STREAM_DRAG_LONG_PRESS_MS,
      setDragState: setStreamDragState,
      buildDragState: (touch) => ({
        streamName,
        pointerId: touch.identifier,
        pointerType: "touch",
        startX: touch.clientX,
        startY: touch.clientY,
        clientX: touch.clientX,
        clientY: touch.clientY,
        hover: null,
        active: false,
      }),
      matchesDragState: (current, nextDragState) => (
        current
        && current.pointerId === nextDragState.pointerId
        && current.streamName === nextDragState.streamName
      ),
    });
  }

  function computeStreamHover(clientX, clientY, sourceStreamName) {
    const row = document.elementFromPoint(clientX, clientY)?.closest?.("[data-stream-row='true']");
    if (!row) {
      return null;
    }

    const targetStreamName = row.getAttribute("data-stream-name");
    if (!targetStreamName || targetStreamName === sourceStreamName) {
      return null;
    }

    const rect = row.getBoundingClientRect();
    return {
      streamName: targetStreamName,
      mode: clientY <= rect.top + rect.height / 2 ? "before" : "after",
    };
  }

  function insertStreamNameRelative(streamNames, movingStreamName, targetStreamName, mode) {
    const filtered = streamNames.filter((streamName) => streamName !== movingStreamName);
    const targetIndex = filtered.indexOf(targetStreamName);
    if (targetIndex < 0) {
      return [...filtered, movingStreamName];
    }

    const insertIndex = mode === "before" ? targetIndex : targetIndex + 1;
    return [
      ...filtered.slice(0, insertIndex),
      movingStreamName,
      ...filtered.slice(insertIndex),
    ];
  }

  async function performStreamDrop(currentDragState) {
    const hover = currentDragState?.hover;
    const sourceStreamName = currentDragState?.streamName;
    if (!hover || !sourceStreamName) {
      return;
    }

    const nextOrderedStreamNames = insertStreamNameRelative(
      orderedStreamNames,
      sourceStreamName,
      hover.streamName,
      hover.mode,
    );

    const didChange = !areArraysEqual(nextOrderedStreamNames, orderedStreamNames);
    if (!didChange) {
      return;
    }

    onReorderStreams?.(nextOrderedStreamNames);
  }

  useEffect(() => {
    if (!streamDragState) {
      clearPendingStreamDragState();
      return undefined;
    }

    const updateStreamHover = (clientX, clientY) => {
      const hover = computeStreamHover(clientX, clientY, streamDragState.streamName);
      setStreamDragState((current) => current ? {
        ...current,
        hover,
        clientX,
        clientY,
      } : current);
    };

    if (streamDragState.pointerType === "touch") {
      return attachTouchDragListeners({
        dragState: streamDragState,
        setDragState: setStreamDragState,
        clearPendingDragState: clearPendingStreamDragState,
        moveSlopPx: STREAM_DRAG_MOVE_SLOP_PX,
        updateHover: updateStreamHover,
        onDrop: async (currentDragState) => {
          suppressStreamClickRef.current = true;
          await performStreamDrop(currentDragState);
        },
      });
    }

    const handlePointerMove = (event) => {
      if (event.pointerId !== streamDragState.pointerId) {
        return;
      }

      if (!streamDragState.active) {
        const distance = Math.hypot(
          event.clientX - streamDragState.startX,
          event.clientY - streamDragState.startY,
        );
        if (distance > STREAM_DRAG_MOVE_SLOP_PX) {
          suppressStreamClickRef.current = true;
          setStreamDragState((current) => current ? {
            ...current,
            active: true,
            clientX: event.clientX,
            clientY: event.clientY,
          } : current);
        }
        return;
      }

      if (event.cancelable) {
        event.preventDefault();
      }

      updateStreamHover(event.clientX, event.clientY);
    };

    const finishDrag = async (event) => {
      if (event.pointerId !== streamDragState.pointerId) {
        return;
      }
      clearPendingStreamDragState();
      const currentDragState = streamDragState;
      setStreamDragState(null);
      if (currentDragState.active) {
        suppressStreamClickRef.current = true;
        await performStreamDrop(currentDragState);
      }
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", finishDrag);
    window.addEventListener("pointercancel", finishDrag);

    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", finishDrag);
      window.removeEventListener("pointercancel", finishDrag);
    };
  }, [orderedStreamNames, streamDragState]);

  return (
    <>
      {!isMobile && sidebarCollapsed ? <div className="workspace-sidebar-toggle">{sidebarCollapsedToggle}</div> : null}
      <aside
        ref={sidebarRef}
        className={`workspace-sidebar${sidebarCollapsed ? " workspace-sidebar--collapsed" : ""}`}
      >
        <div className="workspace-sidebar-surface">
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
                  <svg viewBox="0 0 10 10" width="10" height="10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
                    <path d="M5 1v8M1 5h8" />
                  </svg>
                </button>
              </div>

            <div className="sidebar-section-scroll">
              {orderedStreamNames.length > 0 ? (
                <ul className="stream-list">
                  {orderedStreamNames.map((streamName) => {
                    const isDiary = isDiaryStream(streamName);
                    const isPrimary = PRIMARY_STREAM_NAMES.includes(streamName);
                    const isMenuOpen = streamMenuOpenFor === streamName;
                    const isDragged = streamDragState?.streamName === streamName;
                    const hoverMode = streamDragState?.hover?.streamName === streamName
                      ? streamDragState.hover.mode
                      : null;

                    return (
                      <li key={streamName} className="stream-list-item">
                        <div
                          className={`stream-row${streamSelection?.kind === "stream_single" && streamSelection.streamName === streamName ? " stream-row--active" : ""}${isDragged ? " stream-row--dragged" : ""}${hoverMode === "before" ? " stream-row--drop-before" : ""}${hoverMode === "after" ? " stream-row--drop-after" : ""}${isDiary ? " stream-list-item--with-toggle" : ""}${!isPrimary ? " stream-list-item--with-menu" : ""}${isMenuOpen ? " stream-list-item--menu-open" : ""}`}
                          data-stream-row="true"
                          data-stream-name={streamName}
                        >
                          <button
                            type="button"
                            className={`stream-list-btn${streamSelection?.kind === "stream_single" && streamSelection.streamName === streamName
                              ? " stream-list-btn--active"
                              : ""
                              }${isDiary ? " stream-list-btn--with-toggle" : ""}`}
                            onPointerDown={(event) => handleStreamDragPointerDown(event, streamName)}
                            onTouchStart={(event) => handleStreamDragTouchStart(event, streamName)}
                            onClick={() => handleSelectStream(streamName)}
                          >
                            {streamName}
                          </button>
                          {isDiary ? (
                            <button
                              type="button"
                              className={`stream-blur-toggle${diaryBlurEnabled ? " stream-blur-toggle--active" : ""}`}
                              aria-pressed={diaryBlurEnabled}
                              title={diaryBlurEnabled ? "Diary blur on — click to reveal" : "Diary blur off — click to hide"}
                              onClick={onDiaryBlurToggle}
                            >
                              {diaryBlurEnabled ? (
                                <svg viewBox="0 0 16 12" width="13" height="10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                                  <path d="M2 1.5 14 10.5" />
                                  <path d="M5 4C3.2 5.2 1.5 6.5 1.5 6.5C4 10.5 12 10.5 14.5 6.5" />
                                  <path d="M9.5 2.5C11.5 3.5 13.5 5.2 14.5 6.5" />
                                </svg>
                              ) : (
                                <svg viewBox="0 0 16 12" width="13" height="10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                                  <path d="M1.5 6C4 1.5 12 1.5 14.5 6C12 10.5 4 10.5 1.5 6Z" />
                                  <circle cx="8" cy="6" r="2" />
                                </svg>
                              )}
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
                                    className="stream-dropdown-item"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      setStreamMenuOpenFor(null);
                                      onRenameStream?.(streamName);
                                    }}
                                  >
                                    Rename
                                  </button>
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
                        </div>
                      </li>
                    );
                  })}
                </ul>
              ) : null}

              {isCreating ? (
                <form
                  className="stream-create-row"
                  onSubmit={(event) => {
                    event.preventDefault();
                    void submitCreate();
                  }}
                  onBlur={handleCreateRowBlur}
                >
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
                  />
                  <button
                    type="submit"
                    className="stream-create-action"
                    aria-label="Create stream"
                    disabled={!draftName.trim()}
                  >
                    <svg viewBox="0 0 16 16" width="12" height="12" fill="none" aria-hidden="true">
                      <path
                        d="M3 8.5 6.25 11.75 13 5"
                        stroke="currentColor"
                        strokeWidth="1.8"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  </button>
                  <button
                    type="button"
                    className="stream-create-action"
                    aria-label="Cancel creating stream"
                    onClick={cancelCreating}
                  >
                    <svg viewBox="0 0 16 16" width="12" height="12" fill="none" aria-hidden="true">
                      <path
                        d="M4 4 12 12M12 4 4 12"
                        stroke="currentColor"
                        strokeWidth="1.8"
                        strokeLinecap="round"
                      />
                    </svg>
                  </button>
                </form>
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
        </div>
      </aside>

      <div
        className="workspace-resizer"
        aria-hidden="true"
        onPointerDown={startSidebarResize}
      />

      {streamSelection ? (
        <section className={`editor-panel editor-panel--stream${isKeyboardVisible ? " editor-panel--keyboard-visible" : ""}`}>
          {panelChrome(breadcrumbItemsForStreamSelection(streamSelection))}
          <div
            ref={editorScrollRef}
            className={`editor-panel-scroll${isKeyboardVisible ? " editor-panel-scroll--keyboard-visible" : ""}`}
          >
            {streamEditor}
          </div>
        </section>
      ) : fallbackEditor}

      {streamDragState?.active ? (
        <div
          className="page-tree-drag-ghost"
          style={{
            left: streamDragState.clientX + 14,
            top: streamDragState.clientY + 14,
          }}
        >
          <span className="page-tree-drag-ghost-title">{streamDragState.streamName}</span>
        </div>
      ) : null}

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
