import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

const INITIAL_CREATE_STATE = {
  parentPath: "",
  folderName: "",
};

const appWindow = getCurrentWindow();

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

  if (error.path) {
    return `${error.message} (${error.path})`;
  }

  return error.message;
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

function flattenBlocks(blocks, depth = 0) {
  return blocks.flatMap((block) => [
    {
      depth,
      kind: block.kind,
      content: block.content,
    },
    ...flattenBlocks(block.children ?? [], depth + 1),
  ]);
}

export default function App() {
  const [mode, setMode] = useState("booting");
  const [workspace, setWorkspace] = useState(null);
  const [pages, setPages] = useState([]);
  const [selectedPageId, setSelectedPageId] = useState("");
  const [selectedPageBlocks, setSelectedPageBlocks] = useState(null);
  const [startupError, setStartupError] = useState(null);
  const [actionError, setActionError] = useState(null);
  const [busyAction, setBusyAction] = useState("");
  const [createState, setCreateState] = useState(INITIAL_CREATE_STATE);

  useEffect(() => {
    let cancelled = false;

    async function boot() {
      setMode("booting");
      setStartupError(null);

      try {
        const lastWorkspacePath = await invoke("get_last_workspace_path");
        if (!lastWorkspacePath) {
          if (!cancelled) {
            setMode("onboarding");
          }
          return;
        }

        const openedWorkspace = await invoke("open_workspace", {
          rootPath: lastWorkspacePath,
        });
        const allPages = await invoke("all_pages");

        if (!cancelled) {
          setWorkspace(openedWorkspace);
          setPages(allPages);
          setMode("workspace");
        }
      } catch (error) {
        await invoke("clear_last_workspace_path").catch(() => undefined);

        if (!cancelled) {
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

    boot();

    return () => {
      cancelled = true;
    };
  }, []);

  async function loadWorkspacePages() {
    const allPages = await invoke("all_pages");
    setPages(allPages);
  }

  async function loadPageBlocks(pageId) {
    if (!pageId) {
      setSelectedPageBlocks(null);
      return;
    }

    const blocks = await invoke("page_blocks", { pageId });
    setSelectedPageBlocks(blocks);
  }

  async function openWorkspaceRoot(rootPath) {
    const openedWorkspace = await invoke("open_workspace", { rootPath });
    setWorkspace(openedWorkspace);
    await loadWorkspacePages();
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
      await loadWorkspacePages();
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
    setSelectedPageId("");
    setSelectedPageBlocks(null);
    setActionError(null);
    setMode("onboarding");
  }

  async function handleSelectPage(pageId) {
    setSelectedPageId(pageId);
    setActionError(null);

    try {
      await loadPageBlocks(pageId);
    } catch (error) {
      setActionError(normalizeError(error));
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
    if (mode !== "workspace") {
      return;
    }

    if (pages.length === 0) {
      setSelectedPageId("");
      setSelectedPageBlocks(null);
      return;
    }

    const hasSelectedPage = pages.some((page) => page.page_id === selectedPageId);
    const nextPageId = hasSelectedPage ? selectedPageId : pages[0].page_id;

    if (nextPageId !== selectedPageId) {
      setSelectedPageId(nextPageId);
    }

    loadPageBlocks(nextPageId).catch((error) => {
      setActionError(normalizeError(error));
    });
  }, [mode, pages, selectedPageId]);

  const createDisabled =
    busyAction === "create" ||
    !createState.parentPath ||
    !createState.folderName.trim();

  const regularPages = pages.filter((page) => readStreamName(page.location) === null);
  const streamGroups = pages.reduce((groups, page) => {
    const streamName = readStreamName(page.location);
    if (!streamName) {
      return groups;
    }

    if (!groups[streamName]) {
      groups[streamName] = [];
    }

    groups[streamName].push(page);
    return groups;
  }, {});
  const selectedPage = pages.find((page) => page.page_id === selectedPageId) ?? null;
  const editorRows = selectedPageBlocks ? flattenBlocks(selectedPageBlocks.blocks ?? []) : [];

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
              <button
                className="window-control-button"
                type="button"
                aria-label="Minimize window"
                onClick={handleMinimizeWindow}
              >
                _
              </button>
              <button
                className="window-control-button"
                type="button"
                aria-label="Maximize window"
                onClick={handleToggleMaximizeWindow}
              >
                □
              </button>
              <button
                className="window-control-button window-control-button--close"
                type="button"
                aria-label="Close window"
                onClick={handleCloseWindow}
              >
                ×
              </button>
            </div>
          </header>

          {(startupError || actionError) && (
            <div className="error-banner" role="alert">
              <span>
                {startupError?.cause
                  ? formatError(startupError.cause)
                  : actionError
                    ? formatError(actionError)
                    : startupError?.message}
              </span>
            </div>
          )}

          <div className="workspace-body">
            <aside className="workspace-sidebar">
              <div className="sidebar-section">
                <div className="section-heading">
                  <h2>Streams</h2>
                </div>

                {Object.keys(streamGroups).length === 0 ? (
                  <p className="empty-state">No stream pages yet.</p>
                ) : (
                  <div className="nav-groups">
                    {Object.entries(streamGroups).map(([streamName, streamPages]) => (
                      <section key={streamName} className="nav-group">
                        <p className="nav-group-title">{streamName}</p>
                        <ul className="nav-list">
                          {streamPages.map((page) => (
                            <li key={page.page_id}>
                              <button
                                className={
                                  page.page_id === selectedPageId
                                    ? "nav-item nav-item--active"
                                    : "nav-item"
                                }
                                type="button"
                                onClick={() => handleSelectPage(page.page_id)}
                              >
                                <strong>{page.title || page.page_id}</strong>
                                <span>{page.workspace_path}</span>
                              </button>
                            </li>
                          ))}
                        </ul>
                      </section>
                    ))}
                  </div>
                )}
              </div>

              <div className="sidebar-section">
                <div className="section-heading">
                  <h2>Pages</h2>
                  <button className="ghost-button" type="button" onClick={loadWorkspacePages}>
                    Refresh
                  </button>
                </div>

                {regularPages.length === 0 ? (
                  <p className="empty-state">No regular pages yet.</p>
                ) : (
                  <ul className="nav-list">
                    {regularPages.map((page) => (
                      <li key={page.page_id}>
                        <button
                          className={
                            page.page_id === selectedPageId
                              ? "nav-item nav-item--active"
                              : "nav-item"
                          }
                          type="button"
                          onClick={() => handleSelectPage(page.page_id)}
                        >
                          <strong>{page.title || page.page_id}</strong>
                          <span>{page.workspace_path}</span>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              <div className="sidebar-footer">
                <dl className="workspace-stats">
                  <div>
                    <dt>Pages</dt>
                    <dd>{pages.length}</dd>
                  </div>
                  <div>
                    <dt>Watcher</dt>
                    <dd>{workspace.watcher_status.mode ?? "starting"}</dd>
                  </div>
                </dl>
                <button className="secondary-button" type="button" onClick={handleCloseWorkspace}>
                  Close workspace
                </button>
              </div>
            </aside>

            <section className="editor-panel">
              {selectedPage ? (
                <>
                  <div className="editor-header">
                    <div>
                      <p className="eyebrow">Editor</p>
                      <h1>{selectedPage.title || selectedPage.page_id}</h1>
                      <p className="body-copy">{selectedPage.workspace_path}</p>
                    </div>
                  </div>

                  {editorRows.length === 0 ? (
                    <div className="editor-empty">
                      <p className="empty-state">This page is empty. Editor surface comes next.</p>
                    </div>
                  ) : (
                    <div className="editor-surface">
                      {editorRows.map((row, index) => (
                        <div
                          key={`${row.kind}-${row.content}-${index}`}
                          className="editor-row"
                          style={{ paddingLeft: `${16 + row.depth * 18}px` }}
                        >
                          <span className="editor-row-kind">{row.kind}</span>
                          <span>{row.content || "Untitled block"}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </>
              ) : (
                <div className="editor-empty">
                  <h1>Workspace ready</h1>
                  <p className="body-copy">
                    Add files under <code>pages/</code> or in a stream folder like{" "}
                    <code>journal/2026_05_08.md</code>.
                  </p>
                </div>
              )}
            </section>
          </div>
        </section>
      </main>
    );
  }

  return (
    <main className="app-shell">
      <section className="hero-panel minimal-panel">
        <div className="hero-copy compact-copy">
          <h1>Uniseq</h1>
        </div>

        {(startupError || actionError) && (
          <div className="error-banner" role="alert">
            <span>
              {startupError?.cause
                ? formatError(startupError.cause)
                : actionError
                  ? formatError(actionError)
                  : startupError?.message}
            </span>
          </div>
        )}

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
