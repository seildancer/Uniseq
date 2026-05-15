import { createContext, useContext } from "react";

export const WorkspaceContext = createContext("");

export function useWorkspaceRoot() {
  return useContext(WorkspaceContext);
}
