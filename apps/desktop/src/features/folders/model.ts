import type { Conversation, Folder } from "../../lib/types";

export interface FolderTreeItem {
  folder: Folder;
  conversations: Conversation[];
  childFolders: FolderTreeItem[];
}

export function buildFolderTree(
  folders: Folder[],
  conversations: Conversation[],
  projectId?: string | null,
): FolderTreeItem[] {
  const relevantFolders = projectId !== undefined
    ? folders.filter((f) => (f.projectId ?? null) === (projectId ?? null))
    : folders;

  return relevantFolders.map((folder) => {
    const folderConversations = conversations.filter((c) => c.folderId === folder.id);
    return {
      folder,
      conversations: folderConversations,
      childFolders: [],
    };
  });
}

export function countConversationsInFolder(
  folderId: string,
  conversations: Conversation[],
): number {
  return conversations.filter((c) => c.folderId === folderId).length;
}
