import { useEffect, useRef, useState } from "react";
import Editor from "./Editor.jsx";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

const INITIAL_CREATE_STATE = {
  parentPath: "",
  folderName: "",
};

const NOTICE_AUTO_DISMISS_MS = 4000;

const appWindow = getCurrentWindow();

function readPageLeafName(pageId) {
  if (typeof pageId !== "string") {
    return "";
  }

  if (pageId.startsWith("pages:")) {
    return pageId.slice("pages:".length).split("/").at(-1) ?? pageId;
  }

  if (pageId.startsWith("stream:")) {
    return pageId.slice("stream:".length).split("/").at(-1) ?? pageId;
  }

  return pageId;
}

function pageLabel(page) {
  return page.title || readPageLeafName(page.page_id) || page.page_id;
}

function buildPageTree(pages) {
  const childrenByParent = new Map();

  for (const page of pages) {
    const parentId = page.parent_page_id ?? null;
    const siblings = childrenByParent.get(parentId) ?? [];
    siblings.push(page);
    childrenByParent.set(parentId, siblings);
  }

  const comparePages = (left, right) => left.page_id.localeCompare(right.page_id);

  const buildNodes = (parentId = null) => {
    const nodes = (childrenByParent.get(parentId) ?? []).map((page) => {
      const children = buildNodes(page.page_id);
      const subtreeModifiedAt = Math.max(
        page.modified_at ?? Number.NEGATIVE_INFINITY,
        ...children.map((child) => child.subtreeModifiedAt),
      );

      return {
        page,
        children,
        subtreeModifiedAt,
      };
    });

    nodes.sort((left, right) => {
      if (left.subtreeModifiedAt !== right.subtreeModifiedAt) {
        return right.subtreeModifiedAt - left.subtreeModifiedAt;
      }

      return comparePages(left.page, right.page);
    });

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

function formatError(error) {
  if (!error) {
    return "";
  }

  return error.path ? `${error.message} (${error.path})` : error.message;
}

function readStreamName(location) {
  if (!location || typeof location !== "object") {
    return null;
  }

  if ("stream" in location && location.stream?.stream_name) {
    return location.stream.stream_name;
  }

  if ("Stream" in location && location.Stream?.stream_name) {
    return location.Stream.stream_name;
  }

  return null;
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
  pickerMode = false,
  pickerValue = "",
  onPickerSelect,
  disabledIds = new Set(),
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

        return (
          <li key={page.page_id} className="page-tree-node">
            <div
              className={
                isActive
                  ? "page-tree-row page-tree-row--active"
                  : isPicked
                    ? "page-tree-row page-tree-row--picked"
                    : isDisabled
                      ? "page-tree-row page-tree-row--disabled"
                      : "page-tree-row"
              }
              style={{ "--page-tree-depth": depth }}
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
                    &gt;
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
                      ⋯
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
                  >
                    +
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
                pickerMode={pickerMode}
                pickerValue={pickerValue}
                onPickerSelect={onPickerSelect}
                disabledIds={disabledIds}
              />
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}

export default function App() {
  const didAttemptBootRef = useRef(false);
  const isBootEffectMountedRef = useRef(false);

  const [darkMode, setDarkMode] = useState(() => localStorage.getItem("theme") === "dark");
  const [mode, setMode] = useState("booting");
  const [workspace, setWorkspace] = useState(null);
  const [pages, setPages] = useState([]);
  const [streamNames, setStreamNames] = useState([]);
  const [selectedPageId, setSelectedPageId] = useState("");
  const [selectedPageText, setSelectedPageText] = useState("");
  const [selectedPageRevision, setSelectedPageRevision] = useState(null);
  const [loadedPageId, setLoadedPageId] = useState(null);
  const [startupError, setStartupError] = useState(null);
  const [actionError, setActionError] = useState(null);
  const [notice, setNotice] = useState(null);
  const [busyAction, setBusyAction] = useState("");
  const [createState, setCreateState] = useState(INITIAL_CREATE_STATE);
  const [expandedPageIds, setExpandedPageIds] = useState({});
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef(null);
  const [pageMenuOpenId, setPageMenuOpenId] = useState(null);
  const [modal, setModal] = useState(null);
  const [renameValue, setRenameValue] = useState("");
  const [moveTarget, setMoveTarget] = useState("");

  const regularPages = pages.filter((page) => readStreamName(page.location) === null);
  const pageTree = buildPageTree(regularPages);
  const selectedPage = regularPages.find((page) => page.page_id === selectedPageId) ?? null;
  const loadedPage = regularPages.find((page) => page.page_id === loadedPageId) ?? null;
  const loadedPageEditorKey = loadedPageId && selectedPageRevision
    ? `${loadedPageId}:${selectedPageRevision.len_bytes}:${selectedPageRevision.content_hash}`
    : loadedPageId;
  const createDisabled =
    busyAction === "create" ||
    !createState.parentPath ||
    !createState.folderName.trim();

  async function loadWorkspaceLists() {
    const [allPages, allStreamNames] = await Promise.all([
      invoke("all_pages"),
      invoke("all_streams"),
    ]);
    setPages(allPages);
    setStreamNames(allStreamNames);
  }

  const loadPageContentSeqRef = useRef(0);

  async function loadPageContent(pageId) {
    if (!pageId) {
      setSelectedPageText("");
      setSelectedPageRevision(null);
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

  async function handleEditorConflict() {
    if (!loadedPageId) return;
    await loadPageContent(loadedPageId).catch(() => {});
    setNotice({
      id: Date.now(),
      code: "stale_page_reload",
      message: "Page changed while the editor was open. Reloaded the latest content.",
    });
  }

  async function openWorkspaceRoot(rootPath) {
    const openedWorkspace = await invoke("open_workspace", { rootPath });
    setWorkspace(openedWorkspace);
    await loadWorkspaceLists();
    setMode("workspace");
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
    setStreamNames([]);
    setSelectedPageId("");
    setSelectedPageText("");
    setSelectedPageRevision(null);
    setStartupError(null);
    setActionError(null);
    setExpandedPageIds({});
    setMode("onboarding");
  }

  function handleSelectPage(pageId) {
    setSelectedPageId(pageId);
    setActionError(null);
  }

  function handleTogglePageTree(pageId) {
    setExpandedPageIds((current) => ({
      ...current,
      [pageId]: !current[pageId],
    }));
  }

  function openRenameModal(pageId) {
    setPageMenuOpenId(null);
    setRenameValue(readPageLeafName(pageId));
    setModal({ type: "rename", pageId });
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
  }

  async function handleConfirmRename(newTitle) {
    if (!modal?.pageId || !newTitle.trim()) return;
    setBusyAction("rename");
    setActionError(null);
    try {
      await invoke("rename_page", { pageId: modal.pageId, newTitle: newTitle.trim() });
      const prefix = modal.pageId.lastIndexOf("/");
      const newPageId =
        prefix >= 0
          ? modal.pageId.slice(0, prefix + 1) + newTitle.trim()
          : "pages:" + newTitle.trim();
      setSelectedPageId((current) => remapSubtreePageId(current, modal.pageId, newPageId));
      setLoadedPageId((current) => remapSubtreePageId(current, modal.pageId, newPageId));
      await loadWorkspaceLists();
      closeModal();
    } catch (error) {
      setActionError(normalizeError(error));
    } finally {
      setBusyAction("");
    }
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
      const leafName = readPageLeafName(modal.pageId);
      const newPageId = newParentPageId
        ? newParentPageId + "/" + leafName
        : "pages:" + leafName;
      setSelectedPageId((current) => remapSubtreePageId(current, modal.pageId, newPageId));
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
      if (isPageInSubtree(selectedPageId, modal.pageId)) {
        setSelectedPageId("");
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

  async function handleMinimizeWindow() {
    await appWindow.minimize();
  }

  async function handleToggleMaximizeWindow() {
    await appWindow.toggleMaximize();
  }

  async function handleCloseWindow() {
    await appWindow.close();
  }

  function handleTopbarMouseDown(event) {
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
    if (!notice) {
      return undefined;
    }

    const timeoutId = setTimeout(() => {
      setNotice((current) => (current?.id === notice.id ? null : current));
    }, NOTICE_AUTO_DISMISS_MS);

    return () => clearTimeout(timeoutId);
  }, [notice]);

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
            setMode("onboarding");
          }
          return;
        }

        await openWorkspaceRoot(lastWorkspacePath);
      } catch (error) {
        await invoke("clear_last_workspace_path").catch(() => undefined);

        if (isBootEffectMountedRef.current) {
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

    if (regularPages.length === 0) {
      setSelectedPageId("");
      setSelectedPageText("");
      return;
    }

    if (!regularPages.some((page) => page.page_id === selectedPageId)) {
      setSelectedPageId(regularPages[0].page_id);
    }
  }, [mode, pages]); // eslint-disable-line react-hooks/exhaustive-deps

  // Load blocks when the selected page changes.
  // `selectedPageId` is a string — React compares it by value, so this fires exactly once per navigation.
  useEffect(() => {
    if (mode !== "workspace" || !selectedPageId) {
      return;
    }

    loadPageContent(selectedPageId).catch((error) => {
      setActionError(normalizeError(error));
    });
  }, [mode, selectedPageId]);

  // Poll watcher events to detect external file changes.
  useEffect(() => {
    if (mode !== "workspace") return;

    const id = setInterval(async () => {
      const events = await invoke("drain_workspace_events").catch(() => []);
      for (const event of events) {
        if (event.type === "workspace_reloaded") {
          await loadWorkspaceLists().catch(() => {});
        } else if (event.type === "pages_changed") {
          await loadWorkspaceLists().catch(() => {});
          if (loadedPageId && event.page_ids.includes(loadedPageId)) {
            await loadPageContent(loadedPageId).catch(() => {});
          }
        } else if (event.type === "page_removed") {
          await loadWorkspaceLists().catch(() => {});
          if (event.page_id === loadedPageId) {
            setSelectedPageText("");
            setSelectedPageRevision(null);
            setLoadedPageId(null);
            setSelectedPageId("");
          }
        }
      }
    }, 250);

    return () => clearInterval(id);
  }, [mode, loadedPageId]); // eslint-disable-line react-hooks/exhaustive-deps

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

  const visibleError =
    startupError?.cause ?? actionError ?? (startupError ? normalizeError(startupError) : null);

  if (mode === "booting") {
    return (
      <main className="app-shell">
        <section className="boot-panel minimal-panel">
          <h1>Uniseq</h1>
          <p className="status-copy">Opening last workspace...</p>
        </section>
      </main>
    );
  }

  if (mode === "workspace" && workspace) {
    return (
      <main className="app-shell app-shell--workspace">
        <section className="workspace-shell">
          <header className="app-topbar" onMouseDown={handleTopbarMouseDown}>
            <div className="topbar-brand">
              <strong>Uniseq</strong>
              <span>{workspace.root_path}</span>
            </div>

            <div className="topbar-tabs">
              <span className="topbar-tab topbar-tab--placeholder">Tabs later</span>
            </div>

            <div className="window-controls" data-no-window-drag="true">
              <div className="topbar-menu" ref={menuRef} data-no-window-drag="true">
                <button
                  className="window-control-button"
                  type="button"
                  aria-label="Menu"
                  aria-expanded={menuOpen}
                  onClick={() => setMenuOpen((open) => !open)}
                >
                  ⋮
                </button>
                {menuOpen && (
                  <div className="topbar-menu-dropdown">
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
              <button
                className="window-control-button"
                type="button"
                aria-label="Minimize window"
                onClick={handleMinimizeWindow}
              >
                -
              </button>
              <button
                className="window-control-button"
                type="button"
                aria-label="Maximize window"
                onClick={handleToggleMaximizeWindow}
              >
                []
              </button>
              <button
                className="window-control-button window-control-button--close"
                type="button"
                aria-label="Close window"
                onClick={handleCloseWindow}
              >
                x
              </button>
            </div>
          </header>

          {visibleError ? (
            <div className="error-banner" role="alert">
              <span>{formatError(visibleError)}</span>
            </div>
          ) : null}

          {notice ? (
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

          <div className="workspace-body">
            <aside className="workspace-sidebar">
              <div className="sidebar-section">
                <div className="section-heading">
                  <h2>Streams</h2>
                </div>

                {streamNames.length === 0 ? (
                  <p className="empty-state">No stream pages yet.</p>
                ) : (
                  <ul className="stream-list">
                    {streamNames.map((streamName) => (
                      <li key={streamName} className="stream-list-item">
                        <span className="stream-list-label">{streamName}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              <div className="sidebar-section sidebar-section--pages">
                <div className="section-heading">
                  <h2>Pages</h2>
                </div>

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
                  />
                )}
              </div>
            </aside>

            <section className="editor-panel">
              {loadedPage && (
                <>
                  <p className="eyebrow">Editor</p>
                  <h1>{loadedPage.title || loadedPage.page_id}</h1>
                  <p className="body-copy">{loadedPage.workspace_path}</p>
                  <Editor
                    pageId={loadedPageId}
                    text={selectedPageText}
                    revision={selectedPageRevision}
                    key={loadedPageEditorKey}
                    pages={regularPages}
                    onNavigate={handleSelectPage}
                    onConflict={() => void handleEditorConflict()}
                  />
                </>
              )}
            </section>
          </div>
        </section>

        {modal && (
          <div className="modal-overlay" onClick={closeModal}>
            <div className="modal" onClick={(e) => e.stopPropagation()}>
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
                        renameValue.trim() === readPageLeafName(modal.pageId)
                      }
                      onClick={() => void handleConfirmRename(renameValue)}
                    >
                      {busyAction === "rename" ? "Renaming..." : "Rename"}
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
      </main>
    );
  }

  return (
    <main className="app-shell">
      <section className="hero-panel minimal-panel">
        <div className="hero-copy compact-copy">
          <h1>Uniseq</h1>
        </div>

        {visibleError ? (
          <div className="error-banner" role="alert">
            <span>{formatError(visibleError)}</span>
          </div>
        ) : null}

        <div className="minimal-stack">
          <section className="minimal-section">
            <div className="section-copy">
              <h2>Open existing workspace</h2>
              <p>Select the workspace folder.</p>
            </div>

            <button
              className="primary-button"
              type="button"
              onClick={handleOpenWorkspace}
              disabled={busyAction === "open"}
            >
              {busyAction === "open" ? "Opening..." : "Open workspace folder"}
            </button>
          </section>

          <section className="minimal-section">
            <div className="section-copy">
              <h2>Create new workspace</h2>
              <p>Select where to create the workspace folder.</p>
            </div>

            <form className="create-form compact-form" onSubmit={handleCreateWorkspace}>
              <div className="inline-field">
                <input
                  type="text"
                  value={createState.parentPath}
                  readOnly
                  placeholder="Parent folder"
                />
                <button
                  className="secondary-button"
                  type="button"
                  onClick={handleChooseCreateParent}
                  disabled={busyAction === "pick-parent"}
                >
                  {busyAction === "pick-parent" ? "Choosing..." : "Choose location"}
                </button>
              </div>

              <label className="field">
                <span>Name</span>
                <input
                  type="text"
                  value={createState.folderName}
                  onChange={(event) =>
                    setCreateState((current) => ({
                      ...current,
                      folderName: event.target.value,
                    }))
                  }
                  placeholder="Workspace name"
                />
              </label>

              <button className="primary-button" type="submit" disabled={createDisabled}>
                {busyAction === "create" ? "Creating..." : "Create workspace"}
              </button>
            </form>
          </section>
        </div>
      </section>
    </main>
  );
}
