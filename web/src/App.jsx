import { useEffect, useMemo, useRef, useState } from "react";
import { WorkspaceContext } from "./WorkspaceContext.js";
import Editor from "./Editor.jsx";
import EditorBreadcrumb, { breadcrumbItemsForPageId } from "./components/EditorBreadcrumb.jsx";
import LinkedReferences from "./components/LinkedReferences.jsx";
import StreamWorkspace from "./components/StreamWorkspace.jsx";
import { areArraysEqual } from "./utils/arrays.js";
import pageLeafName from "./utils/pageLeafName.js";
import { todayDateName } from "./utils/streamDates.js";
import {
  orderStreamNamesForDisplay,
  readStreamName,
  readDualStreamNames,
  readSelectedStreamDate,
  shouldBumpStreamReloadToken,
} from "./utils/streamWorkspace.js";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useMobileKeyboard } from "./hooks/useMobileKeyboard.js";
import { MobileKeyboardBar } from "./components/MobileKeyboardBar.jsx";

const INITIAL_CREATE_STATE = {
  parentPath: "",
  folderName: "",
};

const ROOT_PARENT_KEY = "__root__";
const DRAG_LONG_PRESS_MS = 260;
const DRAG_MOVE_SLOP_PX = 8;
const AUTO_EXPAND_ON_HOVER_MS = 600;
const SIDEBAR_WIDTH_STORAGE_KEY = "workspaceSidebarWidth";
const SIDEBAR_COLLAPSED_STORAGE_KEY = "workspaceSidebarCollapsed";
const SIDEBAR_MIN_WIDTH_PX = 280;
const SIDEBAR_COLLAPSED_WIDTH_PX = 52;
const MOBILE_WINDOW_CHROME_MEDIA_QUERY = "(max-width: 820px), (pointer: coarse)";
const STREAM_ORDER_STORAGE_KEY_PREFIX = "streamOrder:";

const appWindow = getCurrentWindow();

function defaultStreamSelection() {
  return { kind: "stream_dual", dateName: todayDateName() };
}

function shouldShowDesktopWindowControls() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return true;
  }

  return !window.matchMedia(MOBILE_WINDOW_CHROME_MEDIA_QUERY).matches;
}

function pageLabel(page) {
  return page.title || pageLeafName(page.page_id) || page.page_id;
}

function searchResultLabel(result) {
  return result?.title || pageLeafName(result?.page_id) || result?.page_id || "";
}

function parentOrderKey(parentPageId) {
  return parentPageId ?? ROOT_PARENT_KEY;
}

function orderChildPageIdsForParent(pages, parentPageId, pageOrderByParent) {
  const siblings = pages
    .filter((page) => (page.parent_page_id ?? null) === (parentPageId ?? null))
    .map((page) => page.page_id);
  const siblingSet = new Set(siblings);
  const stored = pageOrderByParent[parentOrderKey(parentPageId)] ?? [];
  const ordered = stored.filter((pageId) => siblingSet.has(pageId));
  const seen = new Set(ordered);
  ordered.push(...siblings.filter((pageId) => !seen.has(pageId)).sort((left, right) => left.localeCompare(right)));
  return ordered;
}

function buildPageTree(pages, pageOrderByParent) {
  const childrenByParent = new Map();
  const pagesById = new Map(pages.map((page) => [page.page_id, page]));

  for (const page of pages) {
    const parentId = page.parent_page_id ?? null;
    const siblings = childrenByParent.get(parentId) ?? [];
    siblings.push(page);
    childrenByParent.set(parentId, siblings);
  }

  const buildNodes = (parentId = null) => {
    const orderedChildIds = orderChildPageIdsForParent(pages, parentId, pageOrderByParent);
    const nodes = orderedChildIds.map((pageId) => {
      const page = pagesById.get(pageId);
      if (!page) {
        return null;
      }
      const children = buildNodes(page.page_id);

      return {
        page,
        children,
      };
    }).filter(Boolean);

    return nodes;
  };

  return buildNodes(null);
}

function collectAncestorPageIds(pageId, pagesById) {
  const ancestorPageIds = [];
  let currentPage = pagesById.get(pageId) ?? null;

  while (currentPage?.parent_page_id) {
    ancestorPageIds.push(currentPage.parent_page_id);
    currentPage = pagesById.get(currentPage.parent_page_id) ?? null;
  }

  return ancestorPageIds;
}

function remapSubtreePageId(pageId, sourcePageId, targetPageId) {
  if (!pageId || !sourcePageId || !targetPageId) {
    return pageId;
  }

  if (pageId === sourcePageId) {
    return targetPageId;
  }

  if (pageId.startsWith(sourcePageId + "/")) {
    return targetPageId + pageId.slice(sourcePageId.length);
  }

  return pageId;
}

function isPageInSubtree(pageId, rootPageId) {
  if (typeof pageId !== "string" || typeof rootPageId !== "string") {
    return false;
  }

  return pageId === rootPageId || pageId.startsWith(rootPageId + "/");
}

function remapPageOrderEntries(pageOrderByParent, sourcePageId, targetPageId) {
  const next = {};

  for (const [parentKey, orderedIds] of Object.entries(pageOrderByParent)) {
    const remappedParentKey =
      parentKey === ROOT_PARENT_KEY
        ? parentKey
        : remapSubtreePageId(parentKey, sourcePageId, targetPageId);
    const existing = next[remappedParentKey] ?? [];
    const seen = new Set(existing);

    for (const orderedId of orderedIds) {
      const remappedId = remapSubtreePageId(orderedId, sourcePageId, targetPageId);
      if (!seen.has(remappedId)) {
        existing.push(remappedId);
        seen.add(remappedId);
      }
    }

    next[remappedParentKey] = existing;
  }

  return next;
}

function removePageOrderEntries(pageOrderByParent, sourcePageId) {
  return Object.fromEntries(
    Object.entries(pageOrderByParent)
      .filter(([parentKey]) => parentKey === ROOT_PARENT_KEY || !isPageInSubtree(parentKey, sourcePageId))
      .map(([parentKey, orderedIds]) => [
        parentKey,
        orderedIds.filter((pageId) => !isPageInSubtree(pageId, sourcePageId)),
      ]),
  );
}

function insertPageIdRelative(orderedIds, movingPageId, targetPageId, mode) {
  const filtered = orderedIds.filter((pageId) => pageId !== movingPageId);
  const targetIndex = filtered.indexOf(targetPageId);
  if (targetIndex < 0) {
    return [...filtered, movingPageId];
  }

  const insertIndex = mode === "before" ? targetIndex : targetIndex + 1;
  return [
    ...filtered.slice(0, insertIndex),
    movingPageId,
    ...filtered.slice(insertIndex),
  ];
}

function normalizeError(error) {
  if (error && typeof error === "object" && "message" in error) {
    return {
      code: error.code ?? "unknown_error",
      message: error.message,
      path: error.path ?? null,
    };
  }

  return {
    code: "unknown_error",
    message: typeof error === "string" ? error : "Unknown error",
    path: null,
  };
}

function childPageMergeError() {
  return {
    code: "invalid_page_merge",
    message: "Can't merge a page that has subpages. Move or delete its subpages first, or rename it to a new name.",
    path: null,
  };
}

function formatError(error) {
  if (!error) {
    return "";
  }

  return error.path ? `${error.message} (${error.path})` : error.message;
}

function describeSearchMatch(matchedField) {
  switch (matchedField) {
    case "title":
      return "Title";
    case "page_id":
      return "Page";
    case "content":
      return "Content";
    default:
      return "";
  }
}

function PageTree({
  nodes,
  depth = 0,
  expandedPageIds,
  selectedPageId,
  onSelectPage,
  onTogglePageTree,
  pageMenuOpenId,
  onPageMenuToggle,
  onRename,
  onMove,
  onDelete,
  onAddSubpage,
  pickerMode = false,
  pickerValue = "",
  onPickerSelect,
  disabledIds = new Set(),
  dragState = null,
  onDragItemPointerDown,
}) {
  return (
    <ul className={depth === 0 ? "page-tree" : "page-tree page-tree--nested"}>
      {nodes.map(({ page, children }) => {
        const hasChildren = children.length > 0;
        const isExpanded = Boolean(expandedPageIds[page.page_id]);
        const isActive = !pickerMode && page.page_id === selectedPageId;
        const isPicked = pickerMode && page.page_id === pickerValue;
        const isMenuOpen = !pickerMode && pageMenuOpenId === page.page_id;
        const isDisabled = pickerMode && disabledIds.has(page.page_id);
        const isDragged = dragState?.sourcePageId === page.page_id;
        const hoverMode = dragState?.hover?.pageId === page.page_id ? dragState.hover.mode : null;
        const rowClassName = [
          "page-tree-row",
          isActive ? "page-tree-row--active" : "",
          isPicked ? "page-tree-row--picked" : "",
          isDisabled ? "page-tree-row--disabled" : "",
          isDragged ? "page-tree-row--dragged" : "",
          hoverMode === "before" ? "page-tree-row--drop-before" : "",
          hoverMode === "after" ? "page-tree-row--drop-after" : "",
          hoverMode === "child" ? "page-tree-row--drop-child" : "",
        ].filter(Boolean).join(" ");

        return (
          <li key={page.page_id} className="page-tree-node">
            <div
              className={rowClassName}
              style={{ "--page-tree-depth": depth }}
              data-page-row="true"
              data-page-id={page.page_id}
            >
              {hasChildren ? (
                <button
                  className="page-tree-toggle"
                  type="button"
                  aria-label={isExpanded ? "Collapse page" : "Expand page"}
                  aria-expanded={isExpanded}
                  onClick={() => onTogglePageTree(page.page_id)}
                >
                  <span
                    className={
                      isExpanded
                        ? "page-tree-caret page-tree-caret--expanded"
                        : "page-tree-caret"
                    }
                  >
                    <svg viewBox="0 0 8 12" width="7" height="10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <path d="M2 1.5 6 6 2 10.5" />
                    </svg>
                  </span>
                </button>
              ) : (
                <span
                  className="page-tree-toggle page-tree-toggle--placeholder"
                  aria-hidden="true"
                />
              )}

              <button
                className="page-tree-item"
                type="button"
                disabled={isDisabled}
                data-no-window-drag="true"
                onPointerDown={(event) => {
                  if (!pickerMode && !isDisabled) {
                    onDragItemPointerDown?.(event, page.page_id, pageLabel(page));
                  }
                }}
                onClick={() => {
                  if (pickerMode) {
                    if (!isDisabled) onPickerSelect?.(page.page_id);
                  } else {
                    onSelectPage(page.page_id);
                  }
                }}
              >
                <span className="page-tree-title">{pageLabel(page)}</span>
              </button>

              {!pickerMode && (
                <div className="page-tree-actions">
                  <div className="page-tree-menu-wrap">
                    <button
                      className="page-tree-action-btn"
                      type="button"
                      aria-label="More options"
                      aria-expanded={isMenuOpen}
                      onClick={() => onPageMenuToggle(page.page_id)}
                    >
                      <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true">
                        <circle cx="3" cy="8" r="1.5" />
                        <circle cx="8" cy="8" r="1.5" />
                        <circle cx="13" cy="8" r="1.5" />
                      </svg>
                    </button>
                    {isMenuOpen && (
                      <div className="page-tree-dropdown">
                        <button
                          className="page-tree-dropdown-item"
                          type="button"
                          onClick={() => onRename(page.page_id)}
                        >
                          Rename
                        </button>
                        <button
                          className="page-tree-dropdown-item"
                          type="button"
                          onClick={() => onMove(page.page_id)}
                        >
                          Move
                        </button>
                        <button
                          className="page-tree-dropdown-item"
                          type="button"
                          onClick={() => onDelete(page.page_id)}
                        >
                          Delete
                        </button>
                      </div>
                    )}
                  </div>
                  <button
                    className="page-tree-action-btn"
                    type="button"
                    aria-label="Add subpage"
                    onClick={() => onAddSubpage?.(page.page_id)}
                  >
                    <svg viewBox="0 0 10 10" width="10" height="10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
                      <path d="M5 1v8M1 5h8" />
                    </svg>
                  </button>
                </div>
              )}
            </div>

            {hasChildren && isExpanded ? (
              <PageTree
                nodes={children}
                depth={depth + 1}
                expandedPageIds={expandedPageIds}
                selectedPageId={selectedPageId}
                onSelectPage={onSelectPage}
                onTogglePageTree={onTogglePageTree}
                pageMenuOpenId={pageMenuOpenId}
                onPageMenuToggle={onPageMenuToggle}
                onRename={onRename}
                onMove={onMove}
                onDelete={onDelete}
                onAddSubpage={onAddSubpage}
                pickerMode={pickerMode}
                pickerValue={pickerValue}
                onPickerSelect={onPickerSelect}
                disabledIds={disabledIds}
                dragState={dragState}
                onDragItemPointerDown={onDragItemPointerDown}
              />
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}

function WindowMinimizeIcon() {
  return (
    <svg className="window-control-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M2 6.5h8" />
    </svg>
  );
}

function WindowBackIcon() {
  return (
    <svg className="window-control-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M7.75 2.5 4 6l3.75 3.5" />
    </svg>
  );
}

function WindowForwardIcon() {
  return (
    <svg className="window-control-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M4.25 2.5 8 6l-3.75 3.5" />
    </svg>
  );
}

function WindowMaximizeIcon() {
  return (
    <svg className="window-control-icon" viewBox="0 0 12 12" aria-hidden="true">
      <rect x="2.25" y="2.25" width="7.5" height="7.5" rx="0.6" />
    </svg>
  );
}

function WindowRestoreIcon() {
  return (
    <svg className="window-control-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M4.25 2.25h5.5v5.5" />
      <path d="M7.75 4.25h-5.5v5.5h5.5z" />
    </svg>
  );
}

function WindowCloseIcon() {
  return (
    <svg className="window-control-icon" viewBox="0 0 12 12" aria-hidden="true">
      <path d="M3 3l6 6" />
      <path d="M9 3l-6 6" />
    </svg>
  );
}

export default function App() {
  const {
    isMobile,
    isKeyboardVisible,
    keyboardHeight,
    visibleViewportHeight,
  } = useMobileKeyboard();

  const didAttemptBootRef = useRef(false);
  const isBootEffectMountedRef = useRef(false);

  const [darkMode, setDarkMode] = useState(() => {
    const stored = localStorage.getItem("theme");
    if (stored) return stored === "dark";
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  });
  const [mode, setMode] = useState("booting");
  const [workspace, setWorkspace] = useState(null);
  const [pages, setPages] = useState([]);
  const [pageOrderByParent, setPageOrderByParent] = useState({});
  const [streamNames, setStreamNames] = useState([]);
  const [streamOrder, setStreamOrder] = useState([]);
  const [diaryBlurEnabled, setDiaryBlurEnabled] = useState(true);
  const [selectionHistoryState, setSelectionHistoryState] = useState(() => ({
    entries: [defaultStreamSelection()],
    index: 0,
  }));
  const [lastStreamDate, setLastStreamDate] = useState(() => todayDateName());
  const [streamReloadToken, setStreamReloadToken] = useState(0);
  const [selectedPageText, setSelectedPageText] = useState("");
  const [selectedPageRevision, setSelectedPageRevision] = useState(null);
  const [linkedRefs, setLinkedRefs] = useState([]);
  const [loadedPageId, setLoadedPageId] = useState(null);
  const [startupError, setStartupError] = useState(null);
  const [actionError, setActionError] = useState(null);
  const [notice, setNotice] = useState(null);
  const [busyAction, setBusyAction] = useState("");
  const [createState, setCreateState] = useState(INITIAL_CREATE_STATE);
  const [onboardingTab, setOnboardingTab] = useState("create");
  const [expandedPageIds, setExpandedPageIds] = useState({});
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState([]);
  const [searchLoading, setSearchLoading] = useState(false);
  const [pageMenuOpenId, setPageMenuOpenId] = useState(null);
  const [modal, setModal] = useState(null);
  const [renameValue, setRenameValue] = useState("");
  const [editorRenameValue, setEditorRenameValue] = useState("");
  const [moveTarget, setMoveTarget] = useState("");
  const [dragState, setDragState] = useState(null);
  const [windowIsMaximized, setWindowIsMaximized] = useState(false);
  const [showDesktopWindowControls, setShowDesktopWindowControls] = useState(
    () => shouldShowDesktopWindowControls(),
  );
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    const stored = Number(localStorage.getItem(SIDEBAR_WIDTH_STORAGE_KEY));
    if (!Number.isFinite(stored)) {
      return null;
    }
    return Math.max(SIDEBAR_MIN_WIDTH_PX, stored);
  });
  const [sidebarCollapsed, setSidebarCollapsed] = useState(
    () => window.innerWidth <= 720 || localStorage.getItem(SIDEBAR_COLLAPSED_STORAGE_KEY) === "true",
  );
  const dragLongPressTimerRef = useRef(null);
  const dragHoverExpandTimerRef = useRef(null);
  const suppressPageClickRef = useRef(false);
  const editorTitleInputRef = useRef(null);

  const mobileViewportStyle = useMemo(() => (
    isMobile
      ? {
        "--mobile-visible-height": `${visibleViewportHeight}px`,
        "--mobile-keyboard-height": `${keyboardHeight}px`,
      }
      : undefined
  ), [isMobile, keyboardHeight, visibleViewportHeight]);

  const regularPages = pages.filter((page) => readStreamName(page.location) === null);
  const pageTree = buildPageTree(regularPages, pageOrderByParent);
  const regularPagesById = new Map(regularPages.map((page) => [page.page_id, page]));
  const pagesById = new Map(pages.map((page) => [page.page_id, page]));
  const selection = selectionHistoryState.entries[selectionHistoryState.index] ?? defaultStreamSelection();
  const selectedPageId = selection.kind === "page" ? selection.pageId : "";
  const streamSelection = selection.kind === "page" ? null : selection;
  const selectedStreamDate = readSelectedStreamDate(selection, lastStreamDate);
  const orderedStreamNames = useMemo(
    () => orderStreamNamesForDisplay(streamNames, streamOrder),
    [streamNames, streamOrder],
  );
  const dualStreamNames = useMemo(
    () => readDualStreamNames(streamNames, streamOrder),
    [streamNames, streamOrder],
  );
  const loadedPage = pages.find((page) => page.page_id === loadedPageId) ?? null;
  const loadedPageIsRegular = loadedPage ? readStreamName(loadedPage.location) === null : false;
  const loadedPageEditorKey = loadedPageId && selectedPageRevision
    ? `${loadedPageId}:${selectedPageRevision.len_bytes}:${selectedPageRevision.content_hash}`
    : loadedPageId;

  const streamPagesByDate = useMemo(() => {
    const map = new Map();
    for (const page of pages) {
      const sName = readStreamName(page.location);
      if (!sName) continue;
      const dName = pageLeafName(page.page_id);
      const set = map.get(dName) ?? new Set();
      set.add(sName);
      map.set(dName, set);
    }
    return map;
  }, [pages]);
  const createDisabled =
    busyAction === "create" ||
    !createState.parentPath ||
    !createState.folderName.trim();

  function renamedPageIdForTitle(pageId, newTitle) {
    const prefix = pageId.lastIndexOf("/");
    return prefix >= 0
      ? pageId.slice(0, prefix + 1) + newTitle
      : "pages:" + newTitle;
  }

  function areSelectionsEqual(left, right) {
    if (!left || !right || left.kind !== right.kind) {
      return false;
    }

    if (left.kind === "page") {
      return left.pageId === right.pageId;
    }

    if (left.kind === "stream_dual") {
      return left.dateName === right.dateName;
    }

    return left.streamName === right.streamName && left.dateName === right.dateName;
  }

  function resetSelectionHistory(nextSelection) {
    setSelectionHistoryState({
      entries: [nextSelection],
      index: 0,
    });
  }

  function pushSelection(nextSelection) {
    setSelectionHistoryState((current) => {
      const currentSelection = current.entries[current.index] ?? defaultStreamSelection();
      if (areSelectionsEqual(currentSelection, nextSelection)) {
        return current;
      }

      const entries = current.entries.slice(0, current.index + 1);
      entries.push(nextSelection);
      return {
        entries,
        index: entries.length - 1,
      };
    });
  }

  function replaceSelection(nextSelectionOrUpdater) {
    setSelectionHistoryState((current) => {
      const currentSelection = current.entries[current.index] ?? defaultStreamSelection();
      const nextSelection = typeof nextSelectionOrUpdater === "function"
        ? nextSelectionOrUpdater(currentSelection)
        : nextSelectionOrUpdater;
      if (!nextSelection || areSelectionsEqual(currentSelection, nextSelection)) {
        return current;
      }

      const entries = [...current.entries];
      entries[current.index] = nextSelection;
      return {
        entries,
        index: current.index,
      };
    });
  }

  function transformSelectionHistory(transformSelection, fallbackSelection = defaultStreamSelection()) {
    setSelectionHistoryState((current) => {
      const entries = [];
      let index = 0;

      current.entries.forEach((entry, entryIndex) => {
        const nextEntry = transformSelection(entry, entryIndex);
        if (!nextEntry) {
          return;
        }

        if (entries.length > 0 && areSelectionsEqual(entries[entries.length - 1], nextEntry)) {
          if (entryIndex <= current.index) {
            index = entries.length - 1;
          }
          return;
        }

        entries.push(nextEntry);
        if (entryIndex <= current.index) {
          index = entries.length - 1;
        }
      });

      if (entries.length === 0) {
        return {
          entries: [fallbackSelection],
          index: 0,
        };
      }

      return {
        entries,
        index: Math.min(Math.max(index, 0), entries.length - 1),
      };
    });
  }

  function remapPageSelectionEntry(entry, sourcePageId, targetPageId) {
    if (entry.kind !== "page") {
      return entry;
    }

    return {
      kind: "page",
      pageId: remapSubtreePageId(entry.pageId, sourcePageId, targetPageId),
    };
  }

  function removePageSelectionEntry(entry, removedPageId) {
    if (entry.kind === "page" && isPageInSubtree(entry.pageId, removedPageId)) {
      return null;
    }

    return entry;
  }

  function remapStreamSelectionEntry(entry, sourceStreamName, targetStreamName) {
    if (entry.kind !== "stream_single" || entry.streamName !== sourceStreamName) {
      return entry;
    }

    return {
      ...entry,
      streamName: targetStreamName,
    };
  }

  function replaceDeletedStreamSelectionEntry(entry, streamName) {
    if (entry.kind !== "stream_single" || entry.streamName !== streamName) {
      return entry;
    }

    return {
      kind: "stream_dual",
      dateName: entry.dateName,
    };
  }

  function handleNavigateBack() {
    setSelectionHistoryState((current) => (
      current.index === 0
        ? current
        : { ...current, index: current.index - 1 }
    ));
    setActionError(null);
  }

  function handleNavigateForward() {
    setSelectionHistoryState((current) => (
      current.index >= current.entries.length - 1
        ? current
        : { ...current, index: current.index + 1 }
    ));
    setActionError(null);
  }

  function renderWindowControls() {
    const canNavigateBack = selectionHistoryState.index > 0;
    const canNavigateForward = selectionHistoryState.index < selectionHistoryState.entries.length - 1;

    return (
      <div className="window-controls" data-no-window-drag="true">
        <button
          className="window-control-button"
          type="button"
          aria-label="Go back"
          title="Go back"
          disabled={!canNavigateBack}
          onClick={handleNavigateBack}
        >
          <WindowBackIcon />
        </button>
        <button
          className="window-control-button"
          type="button"
          aria-label="Go forward"
          title="Go forward"
          disabled={!canNavigateForward}
          onClick={handleNavigateForward}
        >
          <WindowForwardIcon />
        </button>
        <button className="window-control-button" type="button" aria-label="Minimize window" onClick={handleMinimizeWindow}>
          <WindowMinimizeIcon />
        </button>
        <button className="window-control-button" type="button" aria-label={windowIsMaximized ? "Restore window" : "Maximize window"} onClick={handleToggleMaximizeWindow}>
          {windowIsMaximized ? <WindowRestoreIcon /> : <WindowMaximizeIcon />}
        </button>
        <button className="window-control-button window-control-button--close" type="button" aria-label="Close window" onClick={handleCloseWindow}>
          <WindowCloseIcon />
        </button>
      </div>
    );
  }

  async function loadWorkspaceLists() {
    const [allPages, allStreamNames, order] = await Promise.all([
      invoke("all_pages"),
      invoke("all_streams"),
      invoke("page_order"),
    ]);
    setPages(allPages);
    setStreamNames(allStreamNames);
    setPageOrderByParent(order.sibling_order_by_parent ?? {});
  }

  function openSearchResult(result) {
    const streamName = readStreamName(result.location);
    if (streamName) {
      handleSelectStreamSingle(streamName, pageLeafName(result.page_id));
    } else {
      handleSelectPage(result.page_id);
    }
    closeModal();
  }

  function streamOrderStorageKey(rootPath) {
    return `${STREAM_ORDER_STORAGE_KEY_PREFIX}${rootPath}`;
  }

  const loadPageContentSeqRef = useRef(0);

  async function loadPageContent(pageId) {
    if (!pageId) {
      setSelectedPageText("");
      setSelectedPageRevision(null);
      setLinkedRefs([]);
      return;
    }

    const seq = ++loadPageContentSeqRef.current;
    const { text, revision } = await invoke("page_content", { pageId });
    if (seq === loadPageContentSeqRef.current) {
      setSelectedPageText(text);
      setSelectedPageRevision(revision);
      setLoadedPageId(pageId);
    }
  }

  const loadLinkedRefsSeqRef = useRef(0);

  async function loadPageLinkedRefs(pageId) {
    if (!pageId) {
      setLinkedRefs([]);
      return;
    }

    const seq = ++loadLinkedRefsSeqRef.current;
    const refs = await invoke("page_linked_refs", { pageId });
    if (seq === loadLinkedRefsSeqRef.current) {
      setLinkedRefs(refs);
    }
  }

  function showNotice(message, code = "linked_refs_notice") {
    setNotice({
      id: Date.now(),
      code,
      message,
    });
  }

  async function handleEditorConflict() {
    if (!loadedPageId) return;
    await loadPageContent(loadedPageId).catch(() => { });
    setNotice({
      id: Date.now(),
      code: "stale_page_reload",
      message: "Page changed while the editor was open. Reloaded the latest content.",
    });
  }

  async function openWorkspaceRoot(rootPath) {
    const openedWorkspace = await invoke("open_workspace", { rootPath });
    setWorkspace(openedWorkspace);
    resetSelectionHistory(defaultStreamSelection());
    setLastStreamDate(todayDateName());
    setStreamReloadToken(0);
    setSelectedPageText("");
    setSelectedPageRevision(null);
    setLinkedRefs([]);
    setLoadedPageId(null);
    await loadWorkspaceLists();
    setMode("workspace");
  }

  async function handleOpenDefaultWorkspace() {
    setBusyAction("open");
    setActionError(null);

    try {
      const defaultPath = await invoke("get_default_workspace_path");
      await openWorkspaceRoot(defaultPath);
      setStartupError(null);
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  async function handleOpenWorkspace() {
    setBusyAction("open");
    setActionError(null);

    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: "Choose an existing workspace folder",
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }

      await openWorkspaceRoot(selected);
      setStartupError(null);
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  async function handleChooseCreateParent() {
    setBusyAction("pick-parent");
    setActionError(null);

    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: "Choose where to create the new workspace folder",
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }

      setCreateState((current) => ({
        ...current,
        parentPath: selected,
      }));
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  async function handleCreateWorkspace(event) {
    event.preventDefault();
    setBusyAction("create");
    setActionError(null);

    try {
      const openedWorkspace = await invoke("create_workspace", {
        parentPath: createState.parentPath,
        folderName: createState.folderName,
      });
      setWorkspace(openedWorkspace);
      resetSelectionHistory(defaultStreamSelection());
      setLastStreamDate(todayDateName());
      setStreamReloadToken(0);
      setSelectedPageText("");
      setSelectedPageRevision(null);
      setLinkedRefs([]);
      setLoadedPageId(null);
      await loadWorkspaceLists();
      setStartupError(null);
      setMode("workspace");
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  async function handleCloseWorkspace() {
    await invoke("close_workspace");
    setWorkspace(null);
    setPages([]);
    setPageOrderByParent({});
    setStreamNames([]);
    setStreamOrder([]);
    resetSelectionHistory(defaultStreamSelection());
    setLastStreamDate(todayDateName());
    setStreamReloadToken(0);
    setSelectedPageText("");
    setSelectedPageRevision(null);
    setLinkedRefs([]);
    setLoadedPageId(null);
    setStartupError(null);
    setActionError(null);
    setExpandedPageIds({});
    setDragState(null);
    setSearchQuery("");
    setSearchResults([]);
    setSearchLoading(false);
    setMode("onboarding");
  }

  async function refreshStreamWorkspace(forceReload = false) {
    try {
      const result = await invoke("refresh_stream_workspace", { olderThanDays: 7 });
      const removedPageIds = Array.isArray(result?.removed_page_ids) ? result.removed_page_ids : [];
      if (forceReload || removedPageIds.length > 0) {
        await loadWorkspaceLists();
      }
    } catch {
      // Stream refresh is best-effort and should not block navigation.
      if (forceReload) {
        await loadWorkspaceLists();
      }
    }
  }

  function handleSelectPage(pageId) {
    if (suppressPageClickRef.current) {
      suppressPageClickRef.current = false;
      return;
    }
    pushSelection({ kind: "page", pageId });
    setActionError(null);
    if (isMobile) setSidebarCollapsed(true);
  }

  async function handleCreateStream(streamName) {
    try {
      await invoke("create_stream_page", { streamName, dateName: todayDateName() });
      await loadWorkspaceLists();
      handleSelectStreamSingle(streamName, todayDateName());
    } catch (error) {
      setActionError(normalizeError(error));
    }
  }

  async function handleDeleteStream(streamName) {
    try {
      await invoke("delete_stream", { streamName });
      transformSelectionHistory((entry) => replaceDeletedStreamSelectionEntry(entry, streamName));
      await loadWorkspaceLists();
      if (
        streamSelection?.kind === "stream_single" &&
        streamSelection.streamName === streamName
      ) {
        handleSelectStreamDual(selectedStreamDate);
      }
    } catch (error) {
      setActionError(normalizeError(error));
    }
  }

  function handleReorderStreams(nextOrderedStreamNames) {
    setStreamOrder(nextOrderedStreamNames);
  }

  async function handleCreatePage(title, parentPageId) {
    const trimmed = title.trim();
    if (!trimmed) return;
    const pageId = parentPageId ? `${parentPageId}/${trimmed}` : `pages:${trimmed}`;
    setBusyAction("create");
    setActionError(null);
    try {
      await invoke("create_page", { pageId });
      await loadWorkspaceLists();
      pushSelection({ kind: "page", pageId });
      closeModal();
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  function handleSelectStreamDual(dateName) {
    setLastStreamDate(dateName);
    pushSelection({ kind: "stream_dual", dateName });
    setActionError(null);
    void refreshStreamWorkspace();
    if (isMobile) setSidebarCollapsed(true);
  }

  function handleSelectStreamSingle(streamName, dateName) {
    setLastStreamDate(dateName);
    pushSelection({ kind: "stream_single", streamName, dateName });
    setActionError(null);
    void refreshStreamWorkspace();
    if (isMobile) setSidebarCollapsed(true);
  }

  function handleTogglePageTree(pageId) {
    setExpandedPageIds((current) => ({
      ...current,
      [pageId]: !current[pageId],
    }));
  }

  function openRenameModal(pageId) {
    setPageMenuOpenId(null);
    setRenameValue(pageLeafName(pageId));
    setModal({ type: "rename", pageId });
  }

  function openRenameStreamModal(streamName) {
    setRenameValue(streamName);
    setModal({ type: "rename_stream", streamName });
  }

  function resetEditorRenameValue(page = loadedPage) {
    setEditorRenameValue(page ? page.title || pageLeafName(page.page_id) : "");
  }

  function openMoveModal(pageId) {
    setPageMenuOpenId(null);
    setMoveTarget("");
    setModal({ type: "move", pageId });
  }

  function openDeleteModal(pageId) {
    setPageMenuOpenId(null);
    setModal({ type: "delete", pageId });
  }

  function closeModal() {
    setModal(null);
    setRenameValue("");
    setMoveTarget("");
    setSearchQuery("");
    setSearchResults([]);
    setSearchLoading(false);
  }

  async function renamePage(pageId, newTitle, onSuccess) {
    const trimmedTitle = newTitle.trim();
    if (!pageId || !trimmedTitle) return;
    if (trimmedTitle === pageLeafName(pageId)) {
      onSuccess?.();
      return;
    }

    setBusyAction("rename");
    setActionError(null);
    try {
      await invoke("rename_page", { pageId, newTitle: trimmedTitle });
      const newPageId = renamedPageIdForTitle(pageId, trimmedTitle);
      setPageOrderByParent((current) => remapPageOrderEntries(current, pageId, newPageId));
      transformSelectionHistory((entry) => remapPageSelectionEntry(entry, pageId, newPageId));
      setLoadedPageId((current) => remapSubtreePageId(current, pageId, newPageId));
      await loadWorkspaceLists();
      onSuccess?.(newPageId);
    } catch (error) {
      const normalized = normalizeError(error);
      if (normalized.code === "destination_page_exists") {
        const sourcePage = regularPages.find((page) => page.page_id === pageId);
        if (!sourcePage || sourcePage.child_page_count > 0) {
          setActionError(sourcePage?.child_page_count > 0 ? childPageMergeError() : normalized);
        } else {
          const targetPageId = renamedPageIdForTitle(pageId, trimmedTitle);
          const targetPage = regularPages.find((page) => page.page_id === targetPageId);
          setModal({
            type: "merge_page",
            sourcePageId: pageId,
            targetPageId,
            sourceTitle: pageLabel(sourcePage),
            targetTitle: pageLabel(targetPage ?? { page_id: targetPageId }),
          });
        }
      } else {
        setActionError(normalized);
      }
    } finally {
      setBusyAction("");
    }
  }

  async function handleConfirmRename(newTitle) {
    if (!modal?.pageId) return;
    await renamePage(modal.pageId, newTitle, () => closeModal());
  }

  async function handleConfirmRenameStream(newStreamName) {
    if (modal?.type !== "rename_stream" || !modal.streamName) return;
    const trimmedName = newStreamName.trim();
    if (!trimmedName || trimmedName === modal.streamName) {
      closeModal();
      return;
    }

    setBusyAction("rename_stream");
    setActionError(null);
    try {
      await invoke("rename_stream", {
        streamName: modal.streamName,
        newStreamName: trimmedName,
      });
      setStreamOrder((current) => current.map((name) => (
        name === modal.streamName ? trimmedName : name
      )));
      transformSelectionHistory((entry) => remapStreamSelectionEntry(entry, modal.streamName, trimmedName));
      await loadWorkspaceLists();
      closeModal();
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  async function handleEditorRenameSave() {
    if (!loadedPage) return;
    await renamePage(loadedPage.page_id, editorRenameValue, () => {
      const renamedPageId = renamedPageIdForTitle(loadedPage.page_id, editorRenameValue.trim());
      resetEditorRenameValue({
        ...loadedPage,
        page_id: renamedPageId,
        title: editorRenameValue.trim(),
      });
      editorTitleInputRef.current?.blur();
    });
  }

  async function handleConfirmMove(newParentPageId) {
    if (!modal?.pageId) return;
    setBusyAction("move");
    setActionError(null);
    try {
      await invoke("move_page", {
        pageId: modal.pageId,
        newParentPageId: newParentPageId || null,
      });
      const leafName = pageLeafName(modal.pageId);
      const newPageId = newParentPageId
        ? newParentPageId + "/" + leafName
        : "pages:" + leafName;
      setPageOrderByParent((current) => remapPageOrderEntries(current, modal.pageId, newPageId));
      transformSelectionHistory((entry) => remapPageSelectionEntry(entry, modal.pageId, newPageId));
      setLoadedPageId((current) => remapSubtreePageId(current, modal.pageId, newPageId));
      await loadWorkspaceLists();
      closeModal();
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  async function handleConfirmDelete() {
    if (!modal?.pageId) return;
    setBusyAction("delete");
    setActionError(null);
    try {
      await invoke("delete_page", { pageId: modal.pageId });
      setPageOrderByParent((current) => removePageOrderEntries(current, modal.pageId));
      transformSelectionHistory(
        (entry) => removePageSelectionEntry(entry, modal.pageId),
        { kind: "page", pageId: "" },
      );
      if (isPageInSubtree(selectedPageId, modal.pageId)) {
        setSelectedPageText("");
        setSelectedPageRevision(null);
      }
      if (isPageInSubtree(loadedPageId, modal.pageId)) {
        setLoadedPageId(null);
      }
      await loadWorkspaceLists();
      closeModal();
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  async function handleConfirmMergePage() {
    if (modal?.type !== "merge_page") return;
    const { sourcePageId, targetPageId } = modal;
    const shouldActivateTarget = selectedPageId === sourcePageId || loadedPageId === sourcePageId;
    setBusyAction("merge");
    setActionError(null);
    try {
      await invoke("merge_page", { sourcePageId, targetPageId });
      setPageOrderByParent((current) => removePageOrderEntries(current, sourcePageId));
      transformSelectionHistory((entry) => (
        entry.kind === "page" && entry.pageId === sourcePageId
          ? { kind: "page", pageId: targetPageId }
          : entry
      ));
      if (loadedPageId === sourcePageId) {
        setLoadedPageId(targetPageId);
      }
      await loadWorkspaceLists();
      if (shouldActivateTarget) {
        await Promise.all([
          loadPageContent(targetPageId),
          loadPageLinkedRefs(targetPageId),
        ]);
      }
      closeModal();
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
  }

  async function persistSiblingOrder(parentPageId, orderedChildPageIds) {
    const parentKey = parentOrderKey(parentPageId);
    setPageOrderByParent((current) => ({
      ...current,
      [parentKey]: orderedChildPageIds,
    }));
    await invoke("set_page_sibling_order", {
      parentPageId: parentPageId ?? null,
      orderedChildPageIds,
    });
  }

  function clearPendingDragState() {
    if (dragLongPressTimerRef.current) {
      clearTimeout(dragLongPressTimerRef.current);
      dragLongPressTimerRef.current = null;
    }
    if (dragHoverExpandTimerRef.current) {
      clearTimeout(dragHoverExpandTimerRef.current.timerId ?? dragHoverExpandTimerRef.current);
      dragHoverExpandTimerRef.current = null;
    }
  }

  function computeDragHover(clientX, clientY, sourcePageId) {
    const row = document.elementFromPoint(clientX, clientY)?.closest?.("[data-page-row='true']");
    if (!row) {
      return null;
    }

    const targetPageId = row.getAttribute("data-page-id");
    const targetPage = regularPagesById.get(targetPageId);
    if (!targetPage || targetPageId === sourcePageId || isPageInSubtree(targetPageId, sourcePageId)) {
      return null;
    }

    const rect = row.getBoundingClientRect();
    const upperBound = rect.top + rect.height * 0.28;
    const lowerBound = rect.bottom - rect.height * 0.28;
    const mode = clientY <= upperBound ? "before" : clientY >= lowerBound ? "after" : "child";
    const parentPageId =
      mode === "child" ? targetPage.page_id : targetPage.parent_page_id ?? null;

    return {
      mode,
      pageId: targetPage.page_id,
      parentPageId,
    };
  }

  async function performTreeDrop(currentDragState) {
    const hover = currentDragState?.hover;
    const sourcePageId = currentDragState?.sourcePageId;
    if (!hover || !sourcePageId) {
      return;
    }

    const sourcePage = regularPagesById.get(sourcePageId);
    if (!sourcePage) {
      return;
    }

    const oldParentPageId = sourcePage.parent_page_id ?? null;
    const leafName = pageLeafName(sourcePageId);
    const newParentPageId = hover.parentPageId ?? null;
    const newPageId = newParentPageId ? `${newParentPageId}/${leafName}` : `pages:${leafName}`;
    const nextOrderParentId = hover.mode === "child" ? hover.pageId : newParentPageId;
    const targetParentPageId = hover.mode === "child" ? hover.pageId : hover.parentPageId ?? null;
    const currentSiblingOrder = orderChildPageIdsForParent(regularPages, targetParentPageId, pageOrderByParent);

    let nextSiblingOrder;
    if (hover.mode === "child") {
      nextSiblingOrder = [...currentSiblingOrder.filter((pageId) => pageId !== sourcePageId), newPageId];
    } else {
      nextSiblingOrder = insertPageIdRelative(
        currentSiblingOrder.map((pageId) => (pageId === sourcePageId ? newPageId : pageId)),
        newPageId,
        hover.pageId,
        hover.mode,
      );
    }

    const oldParentNextOrder = orderChildPageIdsForParent(regularPages, oldParentPageId, pageOrderByParent)
      .filter((pageId) => pageId !== sourcePageId);

    const isSameParent = oldParentPageId === newParentPageId;
    const isStructuralMove = sourcePageId !== newPageId;
    const isOrderChanged = !areArraysEqual(currentSiblingOrder, nextSiblingOrder);

    if (!isStructuralMove && !isOrderChanged) {
      return;
    }

    setBusyAction("drag-move");
    setActionError(null);

    try {
      if (isStructuralMove) {
        await invoke("move_page", {
          pageId: sourcePageId,
          newParentPageId: newParentPageId ?? null,
        });
        setPageOrderByParent((current) => remapPageOrderEntries(current, sourcePageId, newPageId));
        transformSelectionHistory((entry) => remapPageSelectionEntry(entry, sourcePageId, newPageId));
        setLoadedPageId((current) => remapSubtreePageId(current, sourcePageId, newPageId));
      }

      if (!isSameParent) {
        await persistSiblingOrder(oldParentPageId, oldParentNextOrder);
      }
      await persistSiblingOrder(nextOrderParentId, nextSiblingOrder);
      if (isStructuralMove) {
        await loadWorkspaceLists();
      }
    } catch (error) {
      setActionError(normalizeError(error));
      await loadWorkspaceLists().catch(() => { });
    } finally {
      setBusyAction("");
    }
  }

  function handleDragItemPointerDown(event, sourcePageId, sourceLabel) {
    if (busyAction || modal) {
      return;
    }

    clearPendingDragState();

    const nextDragState = {
      sourcePageId,
      sourceLabel,
      pointerId: event.pointerId,
      pointerType: event.pointerType,
      startX: event.clientX,
      startY: event.clientY,
      clientX: event.clientX,
      clientY: event.clientY,
      hover: null,
      active: false,
    };

    if (event.pointerType !== "mouse") {
      dragLongPressTimerRef.current = window.setTimeout(() => {
        setDragState((current) => {
          if (
            current &&
            current.pointerId === nextDragState.pointerId &&
            current.sourcePageId === nextDragState.sourcePageId
          ) {
            return { ...current, active: true };
          }
          return current;
        });
        dragLongPressTimerRef.current = null;
      }, DRAG_LONG_PRESS_MS);
    }

    setDragState(nextDragState);
  }

  async function handleMinimizeWindow() {
    await appWindow.minimize();
  }

  async function handleToggleMaximizeWindow() {
    await appWindow.toggleMaximize();
    setWindowIsMaximized(await appWindow.isMaximized());
  }

  async function handleCloseWindow() {
    await appWindow.close();
  }

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return undefined;
    }

    const mediaQuery = window.matchMedia(MOBILE_WINDOW_CHROME_MEDIA_QUERY);
    const handleChange = (event) => {
      setShowDesktopWindowControls(!event.matches);
    };

    setShowDesktopWindowControls(!mediaQuery.matches);
    mediaQuery.addEventListener("change", handleChange);
    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  }, []);

  useEffect(() => {
    if (!showDesktopWindowControls) return;

    let cancelled = false;

    async function syncWindowMaximizedState() {
      const maximized = await appWindow.isMaximized();
      if (!cancelled) {
        setWindowIsMaximized(maximized);
      }
    }

    void syncWindowMaximizedState();
    const unlistenPromise = appWindow.onResized(() => {
      void syncWindowMaximizedState();
    });

    return () => {
      cancelled = true;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [showDesktopWindowControls]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!dragState) {
      clearPendingDragState();
      return undefined;
    }

    const handlePointerMove = (event) => {
      if (event.pointerId !== dragState.pointerId) {
        return;
      }

      if (!dragState.active) {
        const distance = Math.hypot(event.clientX - dragState.startX, event.clientY - dragState.startY);
        if (distance > DRAG_MOVE_SLOP_PX) {
          if (dragState.pointerType === "mouse") {
            suppressPageClickRef.current = true;
            setDragState((current) => current ? {
              ...current,
              active: true,
              clientX: event.clientX,
              clientY: event.clientY,
            } : current);
          } else {
            clearPendingDragState();
            setDragState(null);
          }
        }
        return;
      }

      const hover = computeDragHover(event.clientX, event.clientY, dragState.sourcePageId);
      setDragState((current) => (current ? {
        ...current,
        hover,
        clientX: event.clientX,
        clientY: event.clientY,
      } : current));

      if (
        hover?.mode === "child" &&
        regularPagesById.get(hover.pageId)?.child_page_count > 0 &&
        !expandedPageIds[hover.pageId]
      ) {
        if (dragHoverExpandTimerRef.current?.pageId !== hover.pageId) {
          clearTimeout(dragHoverExpandTimerRef.current?.timerId);
          dragHoverExpandTimerRef.current = {
            pageId: hover.pageId,
            timerId: window.setTimeout(() => {
              setExpandedPageIds((current) => ({ ...current, [hover.pageId]: true }));
              dragHoverExpandTimerRef.current = null;
            }, AUTO_EXPAND_ON_HOVER_MS),
          };
        }
      } else if (dragHoverExpandTimerRef.current) {
        clearTimeout(dragHoverExpandTimerRef.current.timerId);
        dragHoverExpandTimerRef.current = null;
      }
    };

    const finishDrag = async (event) => {
      if (event.pointerId !== dragState.pointerId) {
        return;
      }
      clearPendingDragState();
      const currentDragState = dragState;
      setDragState(null);
      if (currentDragState.active) {
        suppressPageClickRef.current = true;
        await performTreeDrop(currentDragState);
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
  }, [dragState, expandedPageIds, pageOrderByParent, regularPages]);

  function handleWindowDragMouseDown(event) {
    if (!showDesktopWindowControls) {
      return;
    }

    if (event.button !== 0) {
      return;
    }

    const target = event.target;
    if (!(target instanceof Element)) {
      return;
    }

    if (
      target.closest(
        'button, input, textarea, select, option, a, summary, [role="button"], [data-no-window-drag="true"]',
      )
    ) {
      return;
    }

    void appWindow.startDragging();
  }

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", darkMode ? "dark" : "light");
    localStorage.setItem("theme", darkMode ? "dark" : "light");
  }, [darkMode]);

  useEffect(() => {
    if (!workspace?.root_path) {
      setStreamOrder([]);
      return;
    }

    try {
      const stored = JSON.parse(localStorage.getItem(streamOrderStorageKey(workspace.root_path)) ?? "[]");
      setStreamOrder(Array.isArray(stored) ? stored.filter((value) => typeof value === "string") : []);
    } catch {
      setStreamOrder([]);
    }
  }, [workspace?.root_path]);

  useEffect(() => {
    if (!workspace?.root_path || streamNames.length === 0) {
      return;
    }

    const normalizedOrder = orderStreamNamesForDisplay(streamNames, streamOrder);
    const hasChanged =
      normalizedOrder.length !== streamOrder.length
      || normalizedOrder.some((streamName, index) => streamName !== streamOrder[index]);

    if (hasChanged) {
      setStreamOrder(normalizedOrder);
      return;
    }

    localStorage.setItem(
      streamOrderStorageKey(workspace.root_path),
      JSON.stringify(normalizedOrder),
    );
  }, [streamNames, streamOrder, workspace?.root_path]);

  useEffect(() => {
    if (!Number.isFinite(sidebarWidth)) {
      localStorage.removeItem(SIDEBAR_WIDTH_STORAGE_KEY);
      return;
    }
    localStorage.setItem(SIDEBAR_WIDTH_STORAGE_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    localStorage.setItem(SIDEBAR_COLLAPSED_STORAGE_KEY, sidebarCollapsed ? "true" : "false");
  }, [sidebarCollapsed]);

  useEffect(() => {
    function handleClickOutside(event) {
      if (menuRef.current && !menuRef.current.contains(event.target)) {
        setMenuOpen(false);
      }
    }
    if (menuOpen) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [menuOpen]);

  useEffect(() => {
    function handleClickOutside(event) {
      if (!event.target.closest(".page-tree-menu-wrap")) {
        setPageMenuOpenId(null);
      }
    }
    if (pageMenuOpenId !== null) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [pageMenuOpenId]);


  useEffect(() => {
    isBootEffectMountedRef.current = true;

    async function boot() {
      if (didAttemptBootRef.current) {
        return;
      }
      didAttemptBootRef.current = true;

      setMode("booting");
      setStartupError(null);

      try {
        const lastWorkspacePath = await invoke("get_last_workspace_path");
        if (!lastWorkspacePath) {
          if (isBootEffectMountedRef.current) {
            if (window.matchMedia(MOBILE_WINDOW_CHROME_MEDIA_QUERY).matches) {
              try {
                const defaultPath = await invoke("get_default_workspace_path");
                await openWorkspaceRoot(defaultPath);
              } catch {
                setMode("onboarding");
              }
            } else {
              setMode("onboarding");
            }
          }
          return;
        }

        await openWorkspaceRoot(lastWorkspacePath);
      } catch (error) {
        await invoke("clear_last_workspace_path").catch(() => undefined);

        if (isBootEffectMountedRef.current) {
          if (window.matchMedia(MOBILE_WINDOW_CHROME_MEDIA_QUERY).matches) {
            try {
              const defaultPath = await invoke("get_default_workspace_path");
              await openWorkspaceRoot(defaultPath);
            } catch (fallbackError) {
              setStartupError({
                code: "workspace_reopen_failed",
                message: "Could not open workspace. Please try again.",
                path: null,
                cause: normalizeError(fallbackError),
              });
              setMode("onboarding");
            }
          } else {
            setStartupError({
              code: "workspace_reopen_failed",
              message:
                "The last workspace could not be reopened. Choose a workspace folder or create a new one.",
              path: null,
              cause: normalizeError(error),
            });
            setMode("onboarding");
          }
        }
      }
    }

    void boot();

    return () => {
      isBootEffectMountedRef.current = false;
    };
  }, []);

  // Auto-select the first page when the page list changes.
  // Depends on `pages` (state, stable reference), not `regularPages` (computed, new ref every render).
  useEffect(() => {
    if (mode !== "workspace") {
      return;
    }

    if (streamSelection !== null) {
      return;
    }

    if (regularPages.length === 0) {
      replaceSelection({ kind: "page", pageId: "" });
      setSelectedPageText("");
      return;
    }

    if (!regularPages.some((page) => page.page_id === selectedPageId)) {
      replaceSelection({ kind: "page", pageId: regularPages[0].page_id });
    }
  }, [mode, pages, streamSelection]); // eslint-disable-line react-hooks/exhaustive-deps

  // Load blocks when the selected page changes.
  // `selectedPageId` is a string ??React compares it by value, so this fires exactly once per navigation.
  useEffect(() => {
    if (mode !== "workspace" || !selectedPageId) {
      return;
    }

    Promise.all([
      loadPageContent(selectedPageId),
      loadPageLinkedRefs(selectedPageId),
    ]).catch((error) => {
      setActionError(normalizeError(error));
    });
  }, [mode, selectedPageId]);

  // Poll watcher events to detect external file changes.
  useEffect(() => {
    if (mode !== "workspace") return;

    const id = setInterval(async () => {
      const events = await invoke("drain_workspace_events").catch(() => []);
      for (const event of events) {
        if (shouldBumpStreamReloadToken(event, Boolean(streamSelection))) {
          setStreamReloadToken((current) => current + 1);
        }

        if (event.type === "workspace_reloaded") {
          await loadWorkspaceLists().catch(() => { });
          if (loadedPageId) {
            await loadPageLinkedRefs(loadedPageId).catch(() => { });
          }
        } else if (event.type === "pages_changed") {
          await loadWorkspaceLists().catch(() => { });
          if (loadedPageId && event.page_ids.includes(loadedPageId)) {
            await loadPageContent(loadedPageId).catch(() => { });
          }
          if (
            loadedPageId &&
            (event.page_ids.includes(loadedPageId) ||
              linkedRefs.some((entry) => event.page_ids.includes(entry.source_page_id)))
          ) {
            await loadPageLinkedRefs(loadedPageId).catch(() => { });
          }
        } else if (event.type === "page_removed") {
          await loadWorkspaceLists().catch(() => { });
          transformSelectionHistory(
            (entry) => removePageSelectionEntry(entry, event.page_id),
            { kind: "page", pageId: "" },
          );
          if (event.page_id === loadedPageId) {
            setSelectedPageText("");
            setSelectedPageRevision(null);
            setLinkedRefs([]);
            setLoadedPageId(null);
          }
          if (linkedRefs.some((entry) => entry.source_page_id === event.page_id)) {
            await loadPageLinkedRefs(loadedPageId).catch(() => { });
          }
        }
      }
    }, 250);

    return () => clearInterval(id);
  }, [mode, loadedPageId, linkedRefs, streamSelection]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!selectedPageId) {
      return;
    }

    const pagesById = new Map(regularPages.map((page) => [page.page_id, page]));
    const ancestorPageIds = collectAncestorPageIds(selectedPageId, pagesById);
    if (ancestorPageIds.length === 0) {
      return;
    }

    setExpandedPageIds((current) => {
      let changed = false;
      const next = { ...current };

      for (const ancestorPageId of ancestorPageIds) {
        if (!next[ancestorPageId]) {
          next[ancestorPageId] = true;
          changed = true;
        }
      }

      return changed ? next : current;
    });
  }, [selectedPageId, pages]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    resetEditorRenameValue(loadedPage);
  }, [loadedPageId, loadedPage?.title]); // eslint-disable-line react-hooks/exhaustive-deps

  const searchRequestSeqRef = useRef(0);

  useEffect(() => {
    if (mode !== "workspace" || !workspace || modal?.type !== "search") {
      return undefined;
    }

    const trimmedQuery = searchQuery.trim();
    if (!trimmedQuery) {
      setSearchResults([]);
      setSearchLoading(false);
      return undefined;
    }

    const requestSeq = ++searchRequestSeqRef.current;
    setSearchLoading(true);
    const timeoutId = window.setTimeout(() => {
      invoke("search_pages", {
        pageQuery: trimmedQuery,
        limit: 50,
      })
        .then((results) => {
          if (requestSeq === searchRequestSeqRef.current) {
            setSearchResults(Array.isArray(results) ? results : []);
            setSearchLoading(false);
          }
        })
        .catch((error) => {
          if (requestSeq === searchRequestSeqRef.current) {
            setSearchResults([]);
            setSearchLoading(false);
            setActionError(normalizeError(error));
          }
        });
    }, 140);

    return () => window.clearTimeout(timeoutId);
  }, [mode, workspace, modal, searchQuery]);

  const visibleError =
    startupError?.cause ?? actionError ?? (startupError ? normalizeError(startupError) : null);

  if (mode === "booting") {
    return (
      <main className="app-shell">
        <div className="onboard-topbar" onMouseDown={handleWindowDragMouseDown}>
          {showDesktopWindowControls ? renderWindowControls() : null}
        </div>
        <section className="boot-panel minimal-panel">
          <h1>Uniseq</h1>
          <p className="status-copy">Opening last workspace...</p>
        </section>
      </main>
    );
  }

  if (mode === "workspace" && workspace) {
    const sidebarChrome = (
      <div className="workspace-sidebar-chrome" onMouseDown={handleWindowDragMouseDown}>
        <div className="workspace-sidebar-controls" data-no-window-drag="true">
          <button
            className="window-control-button workspace-sidebar-control-button"
            type="button"
            aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
            title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
            onClick={() => {
              setSidebarCollapsed((collapsed) => {
                const nextCollapsed = !collapsed;
                if (nextCollapsed) {
                  setMenuOpen(false);
                }
                return nextCollapsed;
              });
            }}
          >
            <svg className="workspace-sidebar-icon" viewBox="0 0 16 16" aria-hidden="true">
              <rect x="2" y="3" width="12" height="10" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.2" />
              <path d="M6 3.5v9" stroke="currentColor" strokeWidth="1.2" />
              {sidebarCollapsed ? (
                <path d="M9.5 8h2.5M11 6.5 12.5 8 11 9.5" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
              ) : (
                <path d="M12.5 8H10M11 6.5 9.5 8 11 9.5" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
              )}
            </svg>
          </button>
          <div className="topbar-menu" ref={menuRef} data-no-window-drag="true">
            <button
              className="window-control-button workspace-sidebar-control-button"
              type="button"
              aria-label="Settings"
              aria-expanded={menuOpen}
              title="Settings"
              onClick={() => {
                setMenuOpen((open) => !open);
              }}
            >
              <svg
                className="workspace-sidebar-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <path
                  d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
                />
                <circle cx="12" cy="12" r="3" />
              </svg>
            </button>
            {menuOpen && (
              <div className="topbar-settings">
                <button
                  className="topbar-menu-item"
                  type="button"
                  onClick={() => {
                    void loadWorkspaceLists();
                    setMenuOpen(false);
                  }}
                >
                  Refresh
                </button>
                <button
                  className="topbar-menu-item"
                  type="button"
                  onClick={() => {
                    setDarkMode((d) => !d);
                    setMenuOpen(false);
                  }}
                >
                  {darkMode ? "Light mode" : "Dark mode"}
                </button>
                <div className="topbar-menu-divider"></div>
                <div className="topbar-menu-info">
                  <div className="topbar-menu-path">{workspace.root_path}</div>
                  <div className="topbar-menu-info-row">
                    <span>Pages</span>
                    <span>{pages.length}</span>
                  </div>
                  <div className="topbar-menu-info-row">
                    <span>Watcher</span>
                    <span>{workspace.watcher_status.mode ?? "starting"}</span>
                  </div>
                </div>
                <div className="topbar-menu-divider"></div>
                <button
                  className="topbar-menu-item topbar-menu-item--danger"
                  type="button"
                  onClick={() => {
                    void handleCloseWorkspace();
                    setMenuOpen(false);
                  }}
                >
                  Close workspace
                </button>
              </div>
            )}
          </div>
          <div className="topbar-menu" data-no-window-drag="true">
            <button
              className="window-control-button workspace-sidebar-control-button"
              type="button"
              aria-label="Search"
              title="Search"
              onClick={() => {
                setMenuOpen(false);
                setModal({ type: "search" });
              }}
            >
              <svg
                className="workspace-sidebar-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
                aria-hidden="true"
              >
                <circle cx="11" cy="11" r="6" />
                <path d="m20 20-4.2-4.2" />
              </svg>
            </button>
          </div>
        </div>
      </div>
    );

    function renderPanelChrome(breadcrumbItems = []) {
      return (
        <div className="editor-panel-chrome" onMouseDown={handleWindowDragMouseDown}>
          <div className="editor-panel-chrome-main">
            <EditorBreadcrumb items={breadcrumbItems} />
            <div className="editor-panel-drag-region" />
          </div>
          {showDesktopWindowControls ? renderWindowControls() : null}
        </div>
      );
    }

    return (
      <WorkspaceContext.Provider value={workspace.root_path}>
        <main className="app-shell app-shell--workspace" style={mobileViewportStyle}>
          <section className="workspace-shell">
            {visibleError ? (
              <div className="snackbar" role="alert" aria-live="assertive">
                <span>{formatError(visibleError)}</span>
                <button
                  className="snackbar-dismiss"
                  type="button"
                  aria-label="Dismiss error"
                  onClick={() => { setActionError(null); setStartupError(null); }}
                >
                  Dismiss
                </button>
              </div>
            ) : notice ? (
              <div className="snackbar" role="status" aria-live="polite">
                <span>{notice.message}</span>
                <button
                  className="snackbar-dismiss"
                  type="button"
                  aria-label="Dismiss notification"
                  onClick={() => setNotice(null)}
                >
                  Dismiss
                </button>
              </div>
            ) : null}

            <div
              className={`workspace-body${sidebarCollapsed ? " workspace-body--sidebar-collapsed" : ""}${isKeyboardVisible ? " workspace-body--keyboard-visible" : ""}`}
              style={{
                "--workspace-sidebar-width": sidebarCollapsed
                  ? `${SIDEBAR_COLLAPSED_WIDTH_PX}px`
                  : Number.isFinite(sidebarWidth)
                    ? `${sidebarWidth}px`
                    : undefined,
              }}
            >
              {!sidebarCollapsed && (
                <div
                  className="sidebar-mobile-backdrop"
                  onClick={() => setSidebarCollapsed(true)}
                />
              )}
              <StreamWorkspace
                streamSelection={streamSelection}
                selectedStreamDate={selectedStreamDate}
                isMobile={isMobile}
                isKeyboardVisible={isKeyboardVisible}
                orderedStreamNames={orderedStreamNames}
                dualStreamNames={dualStreamNames}
                streamPagesByDate={streamPagesByDate}
                regularPages={regularPages}
                streamReloadToken={streamReloadToken}
                diaryBlurEnabled={diaryBlurEnabled}
                onDiaryBlurToggle={() => setDiaryBlurEnabled((enabled) => !enabled)}
                onSidebarWidthChange={(width) => {
                  setSidebarCollapsed(false);
                  setSidebarWidth(width);
                }}
                sidebarCollapsed={sidebarCollapsed}
                sidebarChrome={sidebarChrome}
                panelChrome={renderPanelChrome}
                pageSidebarContent={
                  <div className="sidebar-section sidebar-section--pages">
                    <div className="section-heading">
                      <h2>Pages</h2>
                      <button
                        type="button"
                        className="stream-add-btn"
                        title="New page"
                        onClick={() => {
                          setRenameValue("");
                          setModal({ type: "new_page" });
                        }}
                      >
                        <svg viewBox="0 0 10 10" width="10" height="10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
                          <path d="M5 1v8M1 5h8" />
                        </svg>
                      </button>
                    </div>

                    <div className="sidebar-section-scroll">
                      {regularPages.length === 0 ? (
                        <p className="empty-state">No regular pages yet.</p>
                      ) : (
                        <PageTree
                          nodes={pageTree}
                          expandedPageIds={expandedPageIds}
                          selectedPageId={selectedPageId}
                          onSelectPage={handleSelectPage}
                          onTogglePageTree={handleTogglePageTree}
                          pageMenuOpenId={pageMenuOpenId}
                          onPageMenuToggle={(id) => setPageMenuOpenId((prev) => (prev === id ? null : id))}
                          onRename={openRenameModal}
                          onMove={openMoveModal}
                          onDelete={openDeleteModal}
                          onAddSubpage={(parentPageId) => {
                            setRenameValue("");
                            setModal({ type: "new_page", parentPageId });
                          }}
                          dragState={dragState?.active ? dragState : null}
                          onDragItemPointerDown={handleDragItemPointerDown}
                        />
                      )}
                    </div>
                  </div>
                }
                fallbackEditor={
                  <section className={`editor-panel${isKeyboardVisible ? " editor-panel--keyboard-visible" : ""}`}>
                    {renderPanelChrome(loadedPage ? breadcrumbItemsForPageId(loadedPage.page_id) : [])}
                    <div className={`editor-panel-scroll${isKeyboardVisible ? " editor-panel-scroll--keyboard-visible" : ""}`}>
                      {loadedPage ? (
                        <div className="editor-panel-content">
                          {loadedPageIsRegular ? (
                            <form
                              className="editor-title-form"
                              onSubmit={(event) => {
                                event.preventDefault();
                                void handleEditorRenameSave();
                              }}
                              onBlur={(event) => {
                                if (busyAction === "rename") return;
                                const nextFocused = event.relatedTarget;
                                if (nextFocused instanceof Node && event.currentTarget.contains(nextFocused)) {
                                  return;
                                }
                                resetEditorRenameValue();
                              }}
                            >
                              <input
                                ref={editorTitleInputRef}
                                className="editor-title-input"
                                type="text"
                                value={editorRenameValue}
                                size={Math.max(editorRenameValue.length, 1)}
                                onFocus={() => setActionError(null)}
                                onChange={(event) => setEditorRenameValue(event.target.value)}
                                onKeyDown={(event) => {
                                  if (event.key === "Escape") {
                                    event.preventDefault();
                                    resetEditorRenameValue();
                                    editorTitleInputRef.current?.blur();
                                  }
                                }}
                              />
                              <div className="editor-title-actions">
                                <button
                                  className="stream-create-action editor-title-action"
                                  type="submit"
                                  aria-label="Save title"
                                  title="Save title"
                                  disabled={
                                    busyAction === "rename" ||
                                    !editorRenameValue.trim() ||
                                    editorRenameValue.trim() === pageLeafName(loadedPage.page_id)
                                  }
                                >
                                  <svg viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
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
                                  className="stream-create-action editor-title-action"
                                  type="button"
                                  aria-label="Cancel title edit"
                                  title="Cancel title edit"
                                  onClick={() => {
                                    resetEditorRenameValue();
                                    editorTitleInputRef.current?.blur();
                                  }}
                                  disabled={busyAction === "rename"}
                                >
                                  <svg viewBox="0 0 16 16" width="13" height="13" fill="none" aria-hidden="true">
                                    <path
                                      d="M4 4 12 12M12 4 4 12"
                                      stroke="currentColor"
                                      strokeWidth="1.8"
                                      strokeLinecap="round"
                                    />
                                  </svg>
                                </button>
                              </div>
                            </form>
                          ) : (
                            <h1 className="editor-title-static">
                              {loadedPage.title || pageLeafName(loadedPage.page_id) || loadedPage.page_id}
                            </h1>
                          )}
                          <Editor
                            pageId={loadedPageId}
                            text={selectedPageText}
                            revision={selectedPageRevision}
                            key={loadedPageEditorKey}
                            pages={regularPages}
                            onNavigate={handleSelectPage}
                            onConflict={() => void handleEditorConflict()}
                          />
                          {loadedPageIsRegular ? (
                            <LinkedReferences
                              entries={linkedRefs}
                              pages={pages}
                              diaryBlurEnabled={diaryBlurEnabled}
                              onNavigate={(sourcePageId) => {
                                const sourcePage = pagesById.get(sourcePageId);
                                if (sourcePage && readStreamName(sourcePage.location) === null) {
                                  handleSelectPage(sourcePageId);
                                }
                              }}
                              onReload={() => loadPageLinkedRefs(loadedPageId)}
                              onNotice={(message) => showNotice(message, "linked_refs_reload")}
                            />
                          ) : null}
                        </div>
                      ) : null}
                    </div>
                  </section>
                }
                onSelectStreamDual={handleSelectStreamDual}
                onSelectStreamSingle={handleSelectStreamSingle}
                onCreateStream={handleCreateStream}
                onDeleteStream={handleDeleteStream}
                onRenameStream={openRenameStreamModal}
                onReorderStreams={handleReorderStreams}
                onNavigatePage={handleSelectPage}
                onError={(error) => setActionError(normalizeError(error))}
                onRefresh={() => void refreshStreamWorkspace(true)}
              />
            </div>

            {dragState?.active ? (
              <div
                className="page-tree-drag-ghost"
                style={{
                  left: dragState.clientX + 14,
                  top: dragState.clientY + 14,
                }}
              >
                <span className="page-tree-drag-ghost-title">{dragState.sourceLabel}</span>
              </div>
            ) : null}
          </section>

          {modal && (
            <div className="modal-overlay" onClick={closeModal}>
              <div
                className={`modal${modal.type === "search" ? " modal--search" : ""}`}
                onClick={(e) => e.stopPropagation()}
              >
                {modal.type === "rename" && (
                  <>
                    <h3>Rename page</h3>
                    <div className="field">
                      <input
                        type="text"
                        value={renameValue}
                        onChange={(e) => setRenameValue(e.target.value)}
                        autoFocus
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            void handleConfirmRename(renameValue);
                          }
                          if (e.key === "Escape") {
                            closeModal();
                          }
                        }}
                      />
                    </div>
                    <div className="modal-actions">
                      <button
                        className="secondary-button"
                        type="button"
                        onClick={closeModal}
                      >
                        Cancel
                      </button>
                      <button
                        className="primary-button"
                        type="button"
                        disabled={
                          !renameValue.trim() ||
                          renameValue.trim() === pageLeafName(modal.pageId)
                        }
                        onClick={() => void handleConfirmRename(renameValue)}
                      >
                        {busyAction === "rename" ? "Renaming..." : "Rename"}
                      </button>
                    </div>
                  </>
                )}

                {modal.type === "rename_stream" && (
                  <>
                    <h3>Rename stream</h3>
                    <div className="field">
                      <input
                        type="text"
                        value={renameValue}
                        onChange={(e) => setRenameValue(e.target.value)}
                        autoFocus
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            void handleConfirmRenameStream(renameValue);
                          }
                          if (e.key === "Escape") {
                            closeModal();
                          }
                        }}
                      />
                    </div>
                    <div className="modal-actions">
                      <button className="secondary-button" type="button" onClick={closeModal}>
                        Cancel
                      </button>
                      <button
                        className="primary-button"
                        type="button"
                        disabled={
                          busyAction === "rename_stream" ||
                          !renameValue.trim() ||
                          renameValue.trim() === modal.streamName
                        }
                        onClick={() => void handleConfirmRenameStream(renameValue)}
                      >
                        {busyAction === "rename_stream" ? "Renaming..." : "Rename"}
                      </button>
                    </div>
                  </>
                )}

                {modal.type === "move" && (
                  <>
                    <h3>Move page</h3>
                    <p className="modal-hint">
                      Choose a new parent for <strong>{pageLabel(regularPages.find((p) => p.page_id === modal.pageId) ?? { page_id: modal.pageId })}</strong>
                    </p>
                    <div className="modal-tree-wrap">
                      <button
                        className={
                          moveTarget === ""
                            ? "page-tree-row page-tree-row--picked"
                            : "page-tree-row"
                        }
                        type="button"
                        style={{ "--page-tree-depth": 0 }}
                        onClick={() => setMoveTarget("")}
                      >
                        <span className="page-tree-toggle page-tree-toggle--placeholder" aria-hidden="true" />
                        <span className="page-tree-item" style={{ textAlign: "left" }}>
                          <span className="page-tree-title">Root (no parent)</span>
                        </span>
                      </button>
                      <PageTree
                        nodes={pageTree}
                        expandedPageIds={expandedPageIds}
                        onTogglePageTree={handleTogglePageTree}
                        pickerMode
                        pickerValue={moveTarget}
                        onPickerSelect={setMoveTarget}
                        disabledIds={new Set([
                          modal.pageId,
                          ...regularPages
                            .filter((p) => p.page_id.startsWith(modal.pageId + "/"))
                            .map((p) => p.page_id),
                        ])}
                        dragState={null}
                      />
                    </div>
                    <div className="modal-actions">
                      <button
                        className="secondary-button"
                        type="button"
                        onClick={closeModal}
                      >
                        Cancel
                      </button>
                      <button
                        className="primary-button"
                        type="button"
                        disabled={busyAction === "move"}
                        onClick={() => void handleConfirmMove(moveTarget)}
                      >
                        {busyAction === "move" ? "Moving..." : "Move"}
                      </button>
                    </div>
                  </>
                )}

                {modal.type === "new_page" && (
                  <>
                    <h3>{modal.parentPageId ? "New subpage" : "New page"}</h3>
                    <div className="field">
                      <input
                        type="text"
                        value={renameValue}
                        placeholder="Page name"
                        onChange={(e) => setRenameValue(e.target.value)}
                        autoFocus
                        onKeyDown={(e) => {
                          if (e.key === "Enter") {
                            e.preventDefault();
                            void handleCreatePage(renameValue, modal.parentPageId);
                          }
                          if (e.key === "Escape") {
                            closeModal();
                          }
                        }}
                      />
                    </div>
                    <div className="modal-actions">
                      <button
                        className="secondary-button"
                        type="button"
                        onClick={closeModal}
                      >
                        Cancel
                      </button>
                      <button
                        className="primary-button"
                        type="button"
                        disabled={busyAction === "create" || !renameValue.trim()}
                        onClick={() => void handleCreatePage(renameValue, modal.parentPageId)}
                      >
                        {busyAction === "create" ? "Creating..." : "Create"}
                      </button>
                    </div>
                  </>
                )}

                {modal.type === "merge_page" && (
                  <>
                    <h3>Merge page</h3>
                    <p>
                      <strong>{modal.sourceTitle}</strong> will be appended to{" "}
                      <strong>{modal.targetTitle}</strong>, all references updated, and the
                      original page deleted. This cannot be undone.
                    </p>
                    <div className="modal-actions">
                      <button className="secondary-button" type="button" onClick={closeModal}>
                        Cancel
                      </button>
                      <button
                        className="primary-button"
                        type="button"
                        disabled={busyAction === "merge"}
                        onClick={() => void handleConfirmMergePage()}
                      >
                        {busyAction === "merge" ? "Merging..." : "Merge"}
                      </button>
                    </div>
                  </>
                )}

                {modal.type === "search" && (
                  <>
                    <h3>Search</h3>
                    <div className="field">
                      <input
                        className="topbar-search-input"
                        type="search"
                        value={searchQuery}
                        placeholder="Search pages and content"
                        autoFocus
                        onChange={(e) => setSearchQuery(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" && searchResults.length > 0) {
                            e.preventDefault();
                            openSearchResult(searchResults[0]);
                          }
                          if (e.key === "Escape") {
                            closeModal();
                          }
                        }}
                      />
                    </div>
                    <div className="modal-list topbar-search-results">
                      {!searchQuery.trim() ? (
                        <p className="topbar-search-empty">Search titles, page ids, and note content.</p>
                      ) : searchLoading ? (
                        <p className="topbar-search-empty">Searching...</p>
                      ) : searchResults.length === 0 ? (
                        <p className="topbar-search-empty">No results.</p>
                      ) : (
                        searchResults.map((result) => {
                          const streamName = readStreamName(result.location);
                          return (
                            <button
                              key={`${result.page_id}:${result.matched_field}:${result.snippet ?? ""}`}
                              className="topbar-search-result"
                              type="button"
                              onClick={() => openSearchResult(result)}
                            >
                              <div className="topbar-search-result-head">
                                <span className="topbar-search-result-title">{searchResultLabel(result)}</span>
                                <span className="topbar-search-result-match">
                                  {describeSearchMatch(result.matched_field)}
                                </span>
                              </div>
                              <div className="topbar-search-result-meta">
                                {streamName ? (
                                  <span className="topbar-search-result-stream">{streamName}</span>
                                ) : null}
                                <span className="topbar-search-result-id">{result.page_id}</span>
                              </div>
                              {result.snippet ? (
                                <div className="topbar-search-result-snippet">{result.snippet}</div>
                              ) : null}
                            </button>
                          );
                        })
                      )}
                    </div>
                    <div className="modal-actions">
                      <button className="secondary-button" type="button" onClick={closeModal}>
                        Close
                      </button>
                    </div>
                  </>
                )}

                {modal.type === "delete" && (
                  <>
                    <h3>Delete page</h3>
                    <p>
                      Delete <strong>{pageLabel(regularPages.find((p) => p.page_id === modal.pageId) ?? { page_id: modal.pageId })}</strong> and all its subpages? This cannot be undone.
                    </p>
                    <div className="modal-actions">
                      <button
                        className="secondary-button"
                        type="button"
                        onClick={closeModal}
                      >
                        Cancel
                      </button>
                      <button
                        className="primary-button"
                        type="button"
                        disabled={busyAction === "delete"}
                        onClick={() => void handleConfirmDelete()}
                      >
                        {busyAction === "delete" ? "Deleting..." : "Delete"}
                      </button>
                    </div>
                  </>
                )}
              </div>
            </div>
          )}

          {isKeyboardVisible && (
            <MobileKeyboardBar keyboardHeight={keyboardHeight} />
          )}
        </main>
      </WorkspaceContext.Provider>
    );
  }

  return (
    <main className="app-shell">
      <div className="onboard-topbar" onMouseDown={handleWindowDragMouseDown}>
        {showDesktopWindowControls ? renderWindowControls() : null}
      </div>
      <section className="hero-panel minimal-panel">
        <img src="/uniseq.svg" alt="Uniseq" className="onboard-logo" />

        {visibleError ? (
          <div className="snackbar" role="alert" aria-live="assertive">
            <span>{formatError(visibleError)}</span>
            <button
              className="snackbar-dismiss"
              type="button"
              aria-label="Dismiss error"
              onClick={() => { setActionError(null); setStartupError(null); }}
            >
              Dismiss
            </button>
          </div>
        ) : null}

        {isMobile ? (
          <div className="onboard-panel">
            <button
              className="primary-button"
              type="button"
              onClick={handleOpenDefaultWorkspace}
              disabled={busyAction === "open"}
            >
              {busyAction === "open" ? "Opening..." : "Open My Notes"}
            </button>
          </div>
        ) : (
          <>
            <div className="onboard-tabs" role="tablist">
              <button
                className={`onboard-tab${onboardingTab === "create" ? " onboard-tab--active" : ""}`}
                type="button"
                role="tab"
                aria-selected={onboardingTab === "create"}
                onClick={() => setOnboardingTab("create")}
              >
                Make new
              </button>
              <button
                className={`onboard-tab${onboardingTab === "open" ? " onboard-tab--active" : ""}`}
                type="button"
                role="tab"
                aria-selected={onboardingTab === "open"}
                onClick={() => setOnboardingTab("open")}
              >
                Open existing
              </button>
            </div>

            <div className="onboard-panel">
              {onboardingTab === "open" ? (
                <button
                  className="primary-button"
                  type="button"
                  onClick={handleOpenWorkspace}
                  disabled={busyAction === "open"}
                >
                  {busyAction === "open" ? "Opening..." : "Choose folder"}
                </button>
              ) : (
                <form className="create-form" onSubmit={handleCreateWorkspace}>
                  <div className="field">
                    <span>Location</span>
                    <div className="inline-field">
                      <input
                        type="text"
                        value={createState.parentPath}
                        readOnly
                        placeholder="Parent folder"
                        title={createState.parentPath}
                      />
                      <button
                        className="secondary-button"
                        type="button"
                        onClick={handleChooseCreateParent}
                        disabled={busyAction === "pick-parent"}
                      >
                        {busyAction === "pick-parent" ? "Choosing..." : "Browse"}
                      </button>
                    </div>
                  </div>
                  <div className="field">
                    <span>Workspace name</span>
                    <input
                      type="text"
                      value={createState.folderName}
                      onChange={(event) =>
                        setCreateState((current) => ({
                          ...current,
                          folderName: event.target.value,
                        }))
                      }
                      placeholder="My Notes"
                    />
                  </div>
                  <button className="primary-button" type="submit" disabled={createDisabled}>
                    {busyAction === "create" ? "Creating..." : "Create"}
                  </button>
                </form>
              )}
            </div>
          </>
        )}
      </section>
    </main>
  );
}


