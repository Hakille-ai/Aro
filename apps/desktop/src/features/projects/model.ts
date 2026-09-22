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

/**
 * Returns the effective organization ID of a project.
 * Supports retro-compatibility: if no explicit organizationId tag is set on the project,
 * infers it from contained conversations (direct or via folders), otherwise defaults to null (personal).
 */
export function getProjectOrganizationId(
  project: Project,
  conversations?: Conversation[],
  folders?: Folder[],
): string | null {
  if (project.organizationId !== undefined && project.organizationId !== null && project.organizationId !== "") {
    return project.organizationId;
  }
  if (conversations && conversations.length > 0) {
    const projFolderIds = folders
      ? new Set(folders.filter((f) => f.projectId === project.id).map((f) => f.id))
      : new Set<string>();
    const matchedConv = conversations.find(
      (c) =>
        (c.projectId === project.id || (c.folderId && projFolderIds.has(c.folderId))) &&
        Boolean(c.organizationId && c.organizationId !== "personal"),
    );
    if (matchedConv?.organizationId) {
      return matchedConv.organizationId;
    }
  }
  return null;
}

/**
 * Returns the effective organization ID of a folder.
 * Supports retro-compatibility: if no explicit organizationId tag is set on the folder,
 * infers it from its parent project, or from contained conversations, otherwise defaults to null (personal).
 */
export function getFolderOrganizationId(
  folder: Folder,
  projects?: Project[],
  conversations?: Conversation[],
  folders?: Folder[],
): string | null {
  if (folder.organizationId !== undefined && folder.organizationId !== null && folder.organizationId !== "") {
    return folder.organizationId;
  }
  if (folder.projectId && projects && projects.length > 0) {
    const parentProject = projects.find((p) => p.id === folder.projectId);
    if (parentProject) {
      const parentOrg = getProjectOrganizationId(parentProject, conversations, folders);
      if (parentOrg) return parentOrg;
    }
  }
  if (conversations && conversations.length > 0) {
    const matchedConv = conversations.find(
      (c) => c.folderId === folder.id && Boolean(c.organizationId && c.organizationId !== "personal"),
    );
    if (matchedConv?.organizationId) {
      return matchedConv.organizationId;
    }
  }
  return null;
}

/**
 * Returns the effective organization ID of a conversation.
 * Supports retro-compatibility: if no explicit organizationId tag is set on the conversation,
 * infers it from its parent project or folder, otherwise defaults to null (personal).
 */
export function getConversationOrganizationId(
  conversation: Conversation,
  projects?: Project[],
  folders?: Folder[],
): string | null {
  if (
    conversation.organizationId !== undefined &&
    conversation.organizationId !== null &&
    conversation.organizationId !== ""
  ) {
    return conversation.organizationId;
  }
  if (conversation.projectId && projects && projects.length > 0) {
    const parentProject = projects.find((p) => p.id === conversation.projectId);
    if (parentProject) {
      const parentOrg = getProjectOrganizationId(parentProject, undefined, folders);
      if (parentOrg) return parentOrg;
    }
  }
  if (conversation.folderId && folders && folders.length > 0) {
    const parentFolder = folders.find((f) => f.id === conversation.folderId);
    if (parentFolder) {
      const parentOrg = getFolderOrganizationId(parentFolder, projects, undefined, folders);
      if (parentOrg) return parentOrg;
    }
  }
  return null;
}

/**
 * Filters projects strictly for the current workspace scope.
 */
export function filterProjectsForScope(
  projects: Project[],
  scope: "personal" | "organization",
  activeOrgId?: string | null,
  conversations?: Conversation[],
  folders?: Folder[],
): Project[] {
  return projects.filter((project) => {
    const orgId = getProjectOrganizationId(project, conversations, folders);
    if (scope === "personal") {
      return !orgId || orgId === "personal";
    } else {
      return Boolean(activeOrgId && orgId === activeOrgId);
    }
  });
}

/**
 * Filters folders strictly for the current workspace scope.
 */
export function filterFoldersForScope(
  folders: Folder[],
  scope: "personal" | "organization",
  activeOrgId?: string | null,
  projects?: Project[],
  conversations?: Conversation[],
): Folder[] {
  return folders.filter((folder) => {
    const orgId = getFolderOrganizationId(folder, projects, conversations, folders);
    if (scope === "personal") {
      return !orgId || orgId === "personal";
    } else {
      return Boolean(activeOrgId && orgId === activeOrgId);
    }
  });
}

/**
 * Filters conversations strictly for the current workspace scope.
 */
export function filterConversationsForScope(
  conversations: Conversation[],
  scope: "personal" | "organization",
  activeOrgId?: string | null,
  projects?: Project[],
  folders?: Folder[],
): Conversation[] {
  return conversations.filter((conversation) => {
    const orgId = getConversationOrganizationId(conversation, projects, folders);
    if (scope === "personal") {
      return !orgId || orgId === "personal";
    } else {
      return Boolean(activeOrgId && orgId === activeOrgId);
    }
  });
}

