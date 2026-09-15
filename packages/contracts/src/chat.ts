export type ChatMessageRole = "system" | "user" | "assistant" | "tool";

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown> | string;
}

export interface ToolResult {
  toolCallId: string;
  name?: string;
  content: string;
  isError?: boolean;
}

export interface ChatMessageAttachment {
  id: string;
  name: string;
  size?: number;
  mimeType?: string;
  uri?: string;
  dataUrl?: string;
}

export interface ChatMessage {
  id: string;
  conversationId: string;
  role: ChatMessageRole;
  content: string;
  thought?: string | null;
  toolCalls?: ToolCall[] | null;
  toolResults?: ToolResult[] | null;
  attachments?: ChatMessageAttachment[] | null;
  model?: string;
  tokensUsed?: number;
  durationMs?: number;
  isStreaming?: boolean;
  error?: string | null;
  createdAt: string;
  updatedAt?: string;
}

export interface SendMessageRequest {
  conversationId: string;
  content: string;
  mode?: "chat" | "think" | "code" | "summarize" | "quiet" | string;
  model?: string;
  modelId?: string;
  provider?: string;
  systemPrompt?: string;
  contextFiles?: string[];
  attachments?: ChatMessageAttachment[];
  webAccess?: "off" | "search" | "deep" | string;
  searchSettings?: Record<string, unknown>;
  temperature?: number;
  stream?: boolean;
}

export interface SendMessageResponse {
  message?: ChatMessage;
  reply?: ChatMessage;
  conversation?: any;
  userMessage?: ChatMessage;
  assistantMessage?: ChatMessage;
}

export type StreamEventType =
  | "text-delta"
  | "thought-delta"
  | "chunk"
  | "tool-call"
  | "tool-result"
  | "artifact"
  | "status"
  | "error"
  | "done";

export interface StreamEvent {
  type?: StreamEventType;
  text?: string;
  content?: string;
  thought?: string;
  toolCall?: ToolCall;
  toolResult?: ToolResult;
  error?: string;
  conversationId?: string;
  messageId?: string;
  conversation?: any;
  userMessage?: any;
  assistantMessage?: any;
  metadata?: Record<string, unknown>;
}
