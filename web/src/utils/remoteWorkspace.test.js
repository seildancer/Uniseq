import test from "node:test";
import assert from "node:assert/strict";
import { resolveRemoteWorkspaceSelection, retainRemoteWorkspaceSelection } from "./remoteWorkspace.js";

const workspaces = [
  { id: "original", name: "My notes" },
  { id: "duplicate", name: "My notes" },
];

test("reconnect uses the selected ID even when names match and a fallback name is supplied", () => {
  assert.deepEqual(resolveRemoteWorkspaceSelection({
    workspaces, selectedWorkspaceId: "original", newWorkspaceName: "",
    fallbackWorkspaceName: "My notes",
  }), { workspace: workspaces[0] });
});

test("missing or stale selections never silently create a remote workspace", () => {
  for (const selectedWorkspaceId of ["", "deleted"]) {
    assert.throws(() => resolveRemoteWorkspaceSelection({
      workspaces, selectedWorkspaceId, newWorkspaceName: "My notes",
      fallbackWorkspaceName: "My notes",
    }), /Choose an existing/);
  }
});

test("creation requires the explicit new-workspace selection", () => {
  assert.deepEqual(resolveRemoteWorkspaceSelection({
    workspaces, selectedWorkspaceId: "__new__", newWorkspaceName: " New notes ",
  }), { name: "New notes" });
  assert.throws(() => resolveRemoteWorkspaceSelection({
    workspaces, selectedWorkspaceId: "__new__", newWorkspaceName: "",
  }), /Enter a name/);
});

test("reloading workspaces keeps the connected ID even if server ordering changes", () => {
  assert.equal(retainRemoteWorkspaceSelection([...workspaces].reverse(), "original"), "original");
  assert.equal(retainRemoteWorkspaceSelection(workspaces, "deleted"), "original");
  assert.equal(retainRemoteWorkspaceSelection([], "original"), "");
});
