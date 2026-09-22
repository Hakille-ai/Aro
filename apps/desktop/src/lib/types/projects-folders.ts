export interface Project {
  id: string;
  name: string;
  description?: string | null;
  instructions?: string | null;
  rootPath?: string | null;
  color: string;
  icon: string;
  createdAt: string;
  updatedAt: string;
  organizationId?: string | null;
}

export interface Folder {
  id: string;
  projectId?: string | null;
  name: string;
  rootPath?: string | null;
  color?: string | null;
  icon?: string | null;
  createdAt: string;
  updatedAt: string;
  organizationId?: string | null;
}

export interface MoveConversationPayload {
  conversationId: string;
  projectId?: string | null;
  folderId?: string | null;
}
