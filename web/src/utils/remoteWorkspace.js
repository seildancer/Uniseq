export function resolveRemoteWorkspaceSelection({
  workspaces,
  selectedWorkspaceId,
  newWorkspaceName,
  fallbackWorkspaceName = "",
}) {
  if (selectedWorkspaceId === "__new__") {
    const name = (newWorkspaceName || fallbackWorkspaceName).trim();
    if (!name) throw new Error("Enter a name for the new remote workspace.");
    return { name };
  }
  const workspace = workspaces.find((item) => item.id === selectedWorkspaceId);
  if (!workspace) throw new Error("Choose an existing remote workspace or explicitly create a new one.");
  return { workspace };
}

export function retainRemoteWorkspaceSelection(workspaces, selectedWorkspaceId) {
  return workspaces.some((item) => item.id === selectedWorkspaceId)
    ? selectedWorkspaceId
    : workspaces[0]?.id ?? "";
}
