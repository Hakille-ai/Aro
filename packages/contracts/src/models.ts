export type ModelProviderKind =
  | "local"
  | "ollama"
  | "openai"
  | "anthropic"
  | "mistral"
  | "groq"
  | "custom";

export interface ModelOption {
  id: string;
  name: string;
  provider: ModelProviderKind;
  description?: string;
  contextWindow?: number;
  supportsStreaming?: boolean;
  supportsVision?: boolean;
  supportsTools?: boolean;
  isDefault?: boolean;
  isLocal?: boolean;
}

export interface ModelProviderConnection {
  provider: ModelProviderKind;
  enabled: boolean;
  endpointUrl?: string;
  hasApiKey?: boolean;
  activeModel?: string;
  status: "connected" | "disconnected" | "error";
  errorMessage?: string;
}

export interface AppSettings {
  defaultModel: string;
  serverUrl: string;
  autoSave: boolean;
  offlineMode: boolean;
  voiceAutoListen: boolean;
  theme: "light" | "dark" | "oled" | "system";
  accentColor: string;
  haptics: boolean;
}
