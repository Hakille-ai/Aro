import type { Membership, Organization, SyncStatus, User, UserPreferences } from "./auth-organizations";
import type { Conversation } from "./conversations-memory-files";
import type { VoiceModelsStatus, VoiceSettings, VoiceStatus } from "./voice";

export type ModelProviderKind =
  | "mock"
  | "ollama"
  | "llama-cpp"
  | "openai"
  | "anthropic"
  | "google"
  | "mistral"
  | "openai-compatible";

export type ModelFallbackPolicy = "local-first";

export interface ModelSettings {
  providers: ModelProviderConnection[];
  activeModelRef: ModelRef;
  fallbackPolicy: ModelFallbackPolicy;
  // Legacy compatibility fields kept by Rust while the UI moves to ModelRef.
  provider: ModelProviderKind;
  modelId: string;
  ollamaEndpoint: string;
  llamaCppEndpoint: string;
  temperature: number;
  maxTokens: number;
}

export interface ModelRef {
  providerId: string;
  providerKind: ModelProviderKind;
  modelId: string;
  label: string;
  family?: string | null;
  local: boolean;
  installed: boolean;
  ready: boolean;
}

export interface ModelOption extends ModelRef {
  id: string;
  provider: ModelProviderKind;
}

export interface ModelProviderConnection {
  id: string;
  kind: ModelProviderKind;
  displayName: string;
  enabled: boolean;
  endpoint?: string | null;
  authConfigured: boolean;
  models: ModelRef[];
  createdAt: string;
  updatedAt: string;
}

export interface SearchSettings {
  provider: string;
  /** Write-only compatibility value. It is never returned or persisted. */
  apiKey?: string | null;
  authConfigured: boolean;
  endpoint?: string | null;
}

export type MemoryContextMode =
  | "auto"
  | "4k"
  | "8k"
  | "16k"
  | "32k"
  | "64k"
  | "128k"
  | "200k"
  | "1m"
  | "custom";

export interface MemoryConfigurationSettings {
  contextMode: MemoryContextMode;
  totalTokenCeiling: number;
  systemBudget: number;
  semanticBudget: number;
  episodicBudget: number;
  workingBudget: number;
  reserveBudget: number;
  compactionInterval: number;
  maxWorkingTurns: number;
  topK: number;
  minSalienceThreshold: number;
  decayHalfLifeDays: number;
  rrfFtsWeight: number;
  rrfVecWeight: number;
  rrfK: number;
  autoMemorize: boolean;
}

export interface AppSettings {
  model: ModelSettings;
  voice: VoiceSettings;
  search: SearchSettings;
  memory: MemoryConfigurationSettings;
  retainHistory: boolean;
  speakResponses: boolean;
}

export interface RuntimeStatus {
  modelProvider: ModelProviderKind;
  modelId: string;
  modelReady: boolean;
  voiceReady: boolean;
  voiceStatus?: VoiceStatus | null;
  endpoint?: string | null;
  detail: string;
  checkedAt: string;
}

export interface BootstrapPayload {
  settings: AppSettings;
  conversations: Conversation[];
  runtime: RuntimeStatus;
  voiceStatus?: VoiceStatus | null;
  voiceModelStatus?: VoiceModelsStatus | null;
  models: ModelOption[];
  modelProviders?: ModelProviderConnection[];
  currentUser?: User | null;
  activeOrganization?: Organization | null;
  memberships?: Membership[];
  preferences?: UserPreferences | null;
  syncStatus?: SyncStatus;
  clientState?: Record<string, unknown>;
  cloudAuthenticated?: boolean;
  apiBaseUrl?: string;
}
