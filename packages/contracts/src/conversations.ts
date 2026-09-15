export interface Project {
  id: string;
  name: string;
  description?: string | null;
  organizationId?: string | null;
  createdAt: string;
  updatedAt?: string;
}

export interface Folder {
  id: string;
  name: string;
  projectId?: string | null;
  parentId?: string | null;
  createdAt: string;
  updatedAt?: string;
}

export interface Conversation {
  id: string;
  title: string;
  organizationId?: string | null;
  projectId?: string | null;
  folderId?: string | null;
  pinned?: boolean;
  archived?: boolean;
  model?: string;
  systemPrompt?: string | null;
  messageCount?: number;
  lastMessagePreview?: string | null;
  mode?: "chat" | "think" | "code" | "summarize" | "quiet" | string;
  createdAt: string;
  updatedAt: string;
}

export interface ConversationCreateRequest {
  title?: string;
  mode?: "chat" | "think" | "code" | "summarize" | "quiet" | string;
  projectId?: string | null;
  folderId?: string | null;
  model?: string;
  systemPrompt?: string | null;
}

export interface ConversationPatch {
  title?: string;
  projectId?: string | null;
  folderId?: string | null;
  pinned?: boolean;
  archived?: boolean;
  model?: string;
  systemPrompt?: string | null;
}

export interface MoveConversationPayload {
  projectId?: string | null;
  folderId?: string | null;
}
