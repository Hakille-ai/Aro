import type { AgentStep } from "./agents-permissions-arena";
import type { SearchSettings } from "./settings-models-runtime";

export type AssistantMode = "chat" | "think" | "code" | "summarize" | "quiet";

export type MessageRole = "user" | "assistant" | "system";

export type WebAccessMode = "off" | "auto" | "on";

export type AttachmentMode = "cloud-object" | "local-reference";

export type FileStatus = "pending" | "available" | "quarantined" | "failed" | "deleted";

export type FileScanStatus = "pending" | "clean" | "blocked" | "skipped";

export interface Conversation {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  mode: AssistantMode;
  projectId?: string | null;
  folderId?: string | null;
  rootPath?: string | null;
  organizationId?: string | null;
}

export interface ChatMessage {
  id: string;
  conversationId: string;
  role: MessageRole;
  content: string;
  attachments?: MessageAttachment[];
  createdAt: string;
  tokenEstimate?: number | null;
  modelId?: string | null;
  isGenerating?: boolean;
  steps?: AgentStep[];
  agentRunId?: string | null;
}

export interface AttachmentRef {
  fileId?: string | null;
  mode: AttachmentMode;
  displayName: string;
  sizeBytes: number;
  mimeType: string;
}

export interface MessageAttachment {
  fileId?: string | null;
  mode: AttachmentMode;
  displayName: string;
  sizeBytes: number;
  mimeType: string;
}

export interface FileObject {
  id: string;
  organizationId: string;
  ownerUserId: string;
  originalName: string;
  mimeType: string;
  sizeBytes: number;
  sha256: string;
  status: FileStatus;
  scanStatus: FileScanStatus;
  createdAt: string;
  updatedAt: string;
}

export type MemoryCategory = "personal" | "technical" | "system" | "preference";

export type MemoryScope = "user" | "conversation" | "organization" | "project";

export type MemoryStatus = "approved" | "candidate" | "archived";

export interface LongTermMemoryItem {
  id: string;
  cloudId?: string;
  clientId?: string | null;
  content: string;
  category: MemoryCategory;
  scope: MemoryScope;
  status: MemoryStatus;
  sourceConversationId?: string | null;
  sourceMessageIds: string[];
  pinned: boolean;
  salience: number;
  recallCount?: number;
  lastUsedAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface EpisodeItem {
  id: string;
  conversationId: string;
  turnStart: number;
  turnEnd: number;
  summary: string;
  keyDecisions: string[];
  entities: string[];
  tokenCount: number;
  createdAt: string;
  updatedAt: string;
}

export type VectorMemoryMode = "disabled" | "auto" | "required";

export interface MemoryIndexStatus {
  mode: VectorMemoryMode;
  state: "active" | "degraded" | "disabled" | string;
  qdrantUrl: string;
  collectionName?: string | null;
  embeddingProvider: string;
  embeddingModel: string;
  dimension?: number | null;
  indexedCount?: number | null;
  message?: string | null;
}

export interface MemoryReindexReport {
  status: MemoryIndexStatus;
  indexedCount: number;
  skippedCount: number;
}

export interface SendMessageRequest {
  conversationId?: string | null;
  content: string;
  mode: AssistantMode;
  systemPrompt?: string | null;
  modelId?: string | null;
  provider?: string | null;
  attachments?: AttachmentRef[];
  webAccess?: WebAccessMode;
  searchSettings?: SearchSettings | null;
}

export interface SendMessageResponse {
  conversation: Conversation;
  userMessage: ChatMessage;
  /**
   * Null when the server could not generate (nothing persisted): the UI must
   * show an ephemeral notice + retry instead of a persisted error message.
   */
  assistantMessage: ChatMessage | null;
  unavailable?: boolean;
  modelId?: string | null;
  agentRunId?: string | null;
}

export interface PreviewFile {
  name: string;
  mimeType: string;
  url: string;
  content?: string;
  data?: ArrayBuffer | Uint8Array;
  sizeBytes?: number;
}
