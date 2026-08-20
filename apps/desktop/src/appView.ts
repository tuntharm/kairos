export type WorkspaceId = "atlas" | "next" | "lab" | "settings";
export type ViewId = "chat" | WorkspaceId;

export function expandedWorkspace(view: ViewId): WorkspaceId {
  return view === "chat" ? "atlas" : view;
}
