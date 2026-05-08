import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

const INITIAL_CREATE_STATE = {
  parentPath: "",
  folderName: "",
};

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

export default function App() {
  const [mode, setMode] = useState("booting");
  const [workspace, setWorkspace] = useState(null);
  const [pages, setPages] = useState([]);
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
    setActionError(null);
    setMode("onboarding");
  }

  const createDisabled =
    busyAction === "create" ||
    !createState.parentPath ||
    !createState.folderName.trim();

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
      <main className="app-shell">
        <section className="workspace-panel">
          <div className="workspace-header">
            <div>
              <p className="eyebrow">Workspace Open</p>
              <h1>Uniseq</h1>
              <p className="body-copy">
                {workspace.root_path}
              </p>
            </div>
            <button className="secondary-button" type="button" onClick={handleCloseWorkspace}>
              Close
            </button>
          </div>

          <dl className="workspace-meta">
            <div>
              <dt>Pages</dt>
              <dd>{pages.length}</dd>
            </div>
            <div>
              <dt>Watcher</dt>
              <dd>{workspace.watcher_status.mode ?? "starting"}</dd>
            </div>
          </dl>

          <section className="page-list-panel">
            <div className="section-heading">
              <h2>Discovered Pages</h2>
              <button className="ghost-button" type="button" onClick={loadWorkspacePages}>
                Refresh
              </button>
            </div>

            {pages.length === 0 ? (
              <p className="empty-state">
                This workspace is ready. Add files under <code>pages/</code> to start building it.
              </p>
            ) : (
              <ul className="page-list">
                {pages.map((page) => (
                  <li key={page.page_id}>
                    <strong>{page.title || page.page_id}</strong>
                    <span>{page.workspace_path}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
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
