import type { Conversation, Folder, Project } from "../../lib/types";

export interface ProjectWithStats extends Project {
  conversationCount: number;
  folderCount: number;
}

export function computeProjectStats(
  projects: Project[],
  folders: Folder[],
  conversations: Conversation[],
): ProjectWithStats[] {
  return projects.map((project) => {
    const projFolders = folders.filter((f) => f.projectId === project.id);
    const folderIds = new Set(projFolders.map((f) => f.id));
    const projConversations = conversations.filter(
      (c) => c.projectId === project.id || (c.folderId && folderIds.has(c.folderId)),
    );
    return {
      ...project,
      conversationCount: projConversations.length,
      folderCount: projFolders.length,
    };
  });
}

export function filterConversationsForProject(
  conversations: Conversation[],
  projectId: string | null,
  folders: Folder[],
): Conversation[] {
  if (!projectId) return conversations;
  const projectFolderIds = new Set(folders.filter((f) => f.projectId === projectId).map((f) => f.id));
  return conversations.filter(
    (c) => c.projectId === projectId || (c.folderId && projectFolderIds.has(c.folderId)),
  );
}

export function getUnassignedConversations(
  conversations: Conversation[],
  folders: Folder[],
): Conversation[] {
  const allFolderIds = new Set(folders.map((f) => f.id));
  return conversations.filter((c) => !c.projectId && (!c.folderId || !allFolderIds.has(c.folderId)));
}
