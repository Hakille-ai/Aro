import { invoke } from "@tauri-apps/api/core";
import { createSseParser } from "../sse";
import type {
  AppSettings,
  AgentContextItem,
  AgentLaneView,
  AgentOrchestratorSnapshot,
  AgentRun,
  AgentRunPriority,
  AgentRunStartRequest,
  AgentRunView,
  ApiKeyCreateResponse,
  ArenaRequest,
  AssistantMode,
  BootstrapPayload,
  ChatMessage,
  CloudLoginRequest,
  CloudRegisterRequest,
  CloudSessionView,
  Conversation,
  FileObject,
  Folder,
  EpisodeItem,
  LongTermMemoryItem,
  MemoryConfigurationSettings,
  MoveConversationPayload,
  Project,
  MemoryIndexStatus,
  MemoryReindexReport,
  MembershipCreateRequest,
  MembershipRole,
  ModelOption,
  ModelProviderConnection,
  ModelProviderKind,
  ModelRef,
  Organization,
  OrganizationCreateRequest,
  OrganizationInvitation,
  OrganizationInvitationPage,
  OrganizationInvitationReceipt,
  OrganizationMember,
  OrganizationPatch,
  PermissionProfile,
  PublicApiKey,
  RuntimeStatus,
  SendMessageRequest,
  SendMessageResponse,
  SynthesisResult,
  TranscriptionResult,
  User,
  UserPreferences,
  UserPreferencesPatch,
  UserProfilePatch,
  VoiceCapability,
  VoiceCapabilityStatus,
  VoiceModelCapability,
  VoiceModelReadiness,
  VoiceModelRuntimeKind,
  VoiceModelsStatus,
  VoiceReadinessIssue,
  VoiceReadinessIssueCode,
  VoiceRuntimeKind,
  VoiceSettingKey,
  VoiceStatus,
  WakeWordDetectionOptions,
  WakeWordDetectionResult,
} from "../types";

export const isTauri = () => Boolean(window.__TAURI_INTERNALS__);

// ---------------------------------------------------------------------------
// Web HTTP client – used when running as a plain browser page (not Tauri).
// Browser tokens intentionally stay in memory. Persistent browser sessions require
// an HttpOnly-cookie/BFF flow; localStorage is not an acceptable credential store.
// ---------------------------------------------------------------------------

const WEB_API_ROOT = (
  (typeof window !== "undefined" && (window as unknown as Record<string, string>).ARO_API_BASE_URL) ||
  (import.meta.env?.VITE_API_BASE_URL) ||
  (import.meta.env?.VITE_API_URL) ||
  "http://127.0.0.1:8710"
).replace(/\/+$/, "");
  const WEB_API_BASE = WEB_API_ROOT.endsWith("/v1") ? WEB_API_ROOT : `${WEB_API_ROOT}/v1`;

/** Racine API (sans le suffixe /v1) : utile pour `/health` qui est hors versionnage. */
export function apiRootUrl(): string {
  return WEB_API_BASE.endsWith("/v1") ? WEB_API_BASE.slice(0, -3) : WEB_API_BASE;
}
const WEB_TOKEN_KEY = "aro_web_access_token";
const WEB_REFRESH_TOKEN_KEY = "aro_web_refresh_token";
let webAccessToken: string | null = null;
let webRefreshTokenValue: string | null = null;
let webRefreshInFlight: Promise<WebAuthSession | null> | null = null;
let webSessionMutationTail: Promise<void> = Promise.resolve();

type WebAuthSession = CloudSessionView & {
  accessToken: string;
  refreshToken: string;
};

function purgeLegacyWebTokens() {
  try {
    localStorage.removeItem(WEB_TOKEN_KEY);
    localStorage.removeItem(WEB_REFRESH_TOKEN_KEY);
  } catch {
    // Storage can be unavailable in a restricted browser context.
  }
}

purgeLegacyWebTokens();

export function webToken(): string | null {
  return webAccessToken;
}
function webTokenSet(token: string | null) {
  webAccessToken = token;
}

function webRefreshToken(): string | null {
  return webRefreshTokenValue;
}
function webRefreshTokenSet(token: string | null) {
  webRefreshTokenValue = token;
}

function clearWebAuthSession() {
  webTokenSet(null);
  webRefreshTokenSet(null);
  demoCloudSession = null;
}

function withWebSessionMutation<T>(operation: () => Promise<T>): Promise<T> {
  const result = webSessionMutationTail.then(operation, operation);
  webSessionMutationTail = result.then(() => undefined, () => undefined);
  return result;
}

function currentWebAuthSession(): WebAuthSession | null {
  const accessToken = webToken();
  const refreshToken = webRefreshToken();
  if (!accessToken || !refreshToken || !demoCloudSession) return null;
  return { ...demoCloudSession, accessToken, refreshToken };
}

function applyWebAuthSession(session: WebAuthSession): WebAuthSession {
  if (!session.accessToken || !session.refreshToken) {
    clearWebAuthSession();
    throw new Error("Cloud authentication returned an incomplete session");
  }
  webTokenSet(session.accessToken);
  webRefreshTokenSet(session.refreshToken);
  demoCloudSession = session;
  return session;
}

async function refreshWebSessionLocked(expectedRefreshToken: string): Promise<WebAuthSession | null> {
  // Another serialized session mutation may already have replaced this credential.
  if (webRefreshToken() !== expectedRefreshToken) return currentWebAuthSession();
  try {
    const response = await fetch(`${WEB_API_BASE}/auth/refresh`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ refreshToken: expectedRefreshToken }),
    });
    if (!response.ok) {
      if (webRefreshToken() === expectedRefreshToken) {
        clearWebAuthSession();
      }
      return null;
    }
    const session = await response.json() as WebAuthSession;
    if (webRefreshToken() !== expectedRefreshToken) return currentWebAuthSession();
    return applyWebAuthSession(session);
  } catch (error) {
    console.error("Failed to auto-refresh cloud session:", error);
    // The request may have reached the server and consumed the credential. Retrying the
    // predecessor would look like theft/replay and revoke the whole token family.
    if (webRefreshToken() === expectedRefreshToken) clearWebAuthSession();
    return null;
  }
}

async function refreshWebSession(expectedRefreshToken: string): Promise<WebAuthSession | null> {
  if (webRefreshInFlight) return webRefreshInFlight;
  const refresh = withWebSessionMutation(() => refreshWebSessionLocked(expectedRefreshToken));
  webRefreshInFlight = refresh;
  try {
    return await refresh;
  } finally {
    if (webRefreshInFlight === refresh) webRefreshInFlight = null;
  }
}

async function readWebResponse<T>(resp: Response): Promise<T> {
  if (!resp.ok) {
    let msg = `${resp.status} ${resp.statusText}`;
    try { const j = await resp.json(); msg = j.error ?? msg; } catch { /* ignore */ }
    throw new Error(msg);
  }
  if (resp.status === 204) return undefined as T;
  return resp.json() as Promise<T>;
}

export async function webFetch<T>(method: string, path: string, body?: unknown, auth = true): Promise<T> {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  let authorizationToken = auth ? webToken() : null;
  let refreshAttempted = false;
  if (auth) {
    if (authorizationToken) headers["Authorization"] = `Bearer ${authorizationToken}`;
  }
  let resp = await fetch(`${WEB_API_BASE}${path}`, {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });

  if (resp.status === 401 && auth && !path.includes("/auth/refresh")) {
    const rToken = webRefreshToken();
    if (rToken) {
      refreshAttempted = true;
      const session = await refreshWebSession(rToken);
      if (session) {
        authorizationToken = session.accessToken;
        headers["Authorization"] = `Bearer ${session.accessToken}`;
        resp = await fetch(`${WEB_API_BASE}${path}`, {
          method,
          headers,
          body: body !== undefined ? JSON.stringify(body) : undefined,
        });
      }
    }
  }

  if (!resp.ok) {
    if (
      resp.status === 401
      && auth
      && webToken() === authorizationToken
      && (!refreshAttempted || webRefreshToken() === null)
    ) {
      clearWebAuthSession();
    }
  }
  return readWebResponse<T>(resp);
}

/** Returns true when the app is running in browser but NOT as Tauri (i.e. web mode). */
export const isWeb = () => !isTauri();

export interface AssistantStatusProvider {
  providerId: string;
  executable: boolean;
  reason: string;
}

export interface AssistantStatus {
  canGenerate: boolean;
  source: "local" | "server-cloud" | "demo" | "none";
  activeLabel: string;
  mockActive: boolean;
  ollamaReachable: boolean;
  ollamaModels: string[];
  llamacppReachable: boolean;
  serverCloudReady: boolean;
  guidance: "ok" | "start-local-engine" | "contact-admin" | "demo-mode";
  providers: AssistantStatusProvider[];
  runnableModelIds: string[];
}

export interface AiCloudConsentState {
  enabled: boolean;
  providerIds: string[];
  dataResidency: string | null;
  acceptedBy: string | null;
  acceptedAt: string | null;
  updatedAt: string;
}

export interface AiCloudKeyState {
  providerId: string;
  configured: boolean;
  updatedAt: string | null;
}

export interface AiCloudStatusState {
  consent: AiCloudConsentState;
  keys: AiCloudKeyState[];
}

/**
 * Pre-check UX (navigateur + mobile via son propre client) : le client sait
 * AVANT d'envoyer si le serveur peut generer. `null` en Tauri (generation
 * locale) ou sans session web.
 */
export async function fetchAssistantStatus(): Promise<AssistantStatus | null> {
  if (isTauri()) return null;
  if (isWeb() && webToken()) {
    try {
      return await webFetch<AssistantStatus>("GET", "/assistant/status");
    } catch (error) {
      console.warn("Assistant status unavailable:", error);
      return null;
    }
  }
  return null;
}

async function requireAiCloudAdmin(): Promise<"tauri" | "web"> {
  if (isTauri()) return "tauri";
  if (isWeb() && webToken()) return "web";
  throw new Error("Cloud connection required to manage server AI.");
}

export async function fetchAiCloudStatus(): Promise<AiCloudStatusState> {
  const mode = await requireAiCloudAdmin();
  if (mode === "tauri") {
    return invoke<AiCloudStatusState>("ai_cloud_status");
  }
  return webFetch<AiCloudStatusState>("GET", "/settings/ai-cloud/status");
}

export async function setAiCloudConsent(request: {
  enabled: boolean;
  providerIds: string[];
  dataResidency?: string | null;
}): Promise<AiCloudStatusState> {
  const mode = await requireAiCloudAdmin();
  if (mode === "tauri") {
    return invoke<AiCloudStatusState>("ai_cloud_set_consent", { request });
  }
  return webFetch<AiCloudStatusState>("PUT", "/settings/ai-cloud/consent", request);
}

export async function putAiCloudProviderKey(
  providerId: string,
  apiKey: string,
): Promise<AiCloudStatusState> {
  const mode = await requireAiCloudAdmin();
  if (mode === "tauri") {
    return invoke<AiCloudStatusState>("ai_cloud_put_key", {
      providerId,
      apiKey,
    });
  }
  return webFetch<AiCloudStatusState>(
    "PUT",
    `/settings/ai-cloud/keys/${encodeURIComponent(providerId)}`,
    { apiKey },
  );
}

export async function deleteAiCloudProviderKey(
  providerId: string,
): Promise<AiCloudStatusState> {
  const mode = await requireAiCloudAdmin();
  if (mode === "tauri") {
    return invoke<AiCloudStatusState>("ai_cloud_delete_key", { providerId });
  }
  return webFetch<AiCloudStatusState>(
    "DELETE",
    `/settings/ai-cloud/keys/${encodeURIComponent(providerId)}`,
  );
}

export function modelOptionKey(model: Pick<ModelRef, "providerId" | "modelId">): string {
  return `${model.providerId}::${model.modelId}`;
}

function normalizeProviderKind(kind: string | null | undefined): ModelProviderKind {
  if (kind === "open-ai") return "openai";
  if (kind === "open-ai-compatible") return "openai-compatible";
  const allowed = new Set<ModelProviderKind>([
    "mock",
    "ollama",
    "llama-cpp",
    "openai",
    "anthropic",
    "google",
    "mistral",
    "openai-compatible",
  ]);
  return allowed.has(kind as ModelProviderKind) ? (kind as ModelProviderKind) : "mock";
}

export function normalizeModelRef(model: ModelRef): ModelOption {
  const providerKind = normalizeProviderKind(model.providerKind);
  return {
    ...model,
    providerKind,
    id: modelOptionKey(model),
    provider: providerKind,
  };
}

function normalizeModels(models: ModelRef[] | ModelOption[] | undefined): ModelOption[] {
  return (models ?? []).map((model) => normalizeModelRef(model));
}

function normalizeBootstrap(payload: BootstrapPayload): BootstrapPayload {
  const settings = normalizeSettings(payload.settings);
  const voiceStatus = payload.voiceStatus ?? makeDemoVoiceStatus(settings);
  const voiceModelStatus = payload.voiceModelStatus ?? makeDemoVoiceModelStatus(settings, voiceStatus);
  return {
    ...payload,
    settings,
    runtime: normalizeRuntime(payload.runtime, voiceStatus),
    voiceStatus,
    voiceModelStatus,
    models: normalizeModels(payload.models),
    modelProviders: payload.modelProviders ?? settings.model.providers,
  };
}

function normalizeRuntime(runtime: RuntimeStatus, voiceStatus?: VoiceStatus | null): RuntimeStatus {
  const resolvedVoiceStatus = voiceStatus ?? runtime.voiceStatus ?? null;
  return {
    ...runtime,
    voiceReady: resolvedVoiceStatus?.ready ?? runtime.voiceReady,
    voiceStatus: resolvedVoiceStatus,
  };
}

export function makeDefaultMemorySettings(): MemoryConfigurationSettings {
  return {
    contextMode: "auto",
    totalTokenCeiling: 8192,
    systemBudget: 800,
    semanticBudget: 1600,
    episodicBudget: 2000,
    workingBudget: 2400,
    reserveBudget: 1392,
    compactionInterval: 10,
    maxWorkingTurns: 8,
    topK: 5,
    minSalienceThreshold: 0.2,
    decayHalfLifeDays: 0,
    rrfFtsWeight: 0.4,
    rrfVecWeight: 0.6,
    rrfK: 60,
    autoMemorize: true,
  };
}

export function normalizeMemorySettings(
  raw?: Partial<MemoryConfigurationSettings> | null,
): MemoryConfigurationSettings {
  const def = makeDefaultMemorySettings();
  if (!raw) return def;
  return {
    contextMode: raw.contextMode ?? def.contextMode,
    totalTokenCeiling: Number(raw.totalTokenCeiling) || def.totalTokenCeiling,
    systemBudget: Number(raw.systemBudget) || def.systemBudget,
    semanticBudget: Number(raw.semanticBudget) || def.semanticBudget,
    episodicBudget: Number(raw.episodicBudget) || def.episodicBudget,
    workingBudget: Number(raw.workingBudget) || def.workingBudget,
    reserveBudget: Number(raw.reserveBudget) || def.reserveBudget,
    compactionInterval: Number(raw.compactionInterval) || def.compactionInterval,
    maxWorkingTurns: Number(raw.maxWorkingTurns) || def.maxWorkingTurns,
    topK: Number(raw.topK) || def.topK,
    minSalienceThreshold: typeof raw.minSalienceThreshold === "number" ? raw.minSalienceThreshold : def.minSalienceThreshold,
    decayHalfLifeDays: typeof raw.decayHalfLifeDays === "number" ? raw.decayHalfLifeDays : def.decayHalfLifeDays,
    rrfFtsWeight: typeof raw.rrfFtsWeight === "number" ? raw.rrfFtsWeight : def.rrfFtsWeight,
    rrfVecWeight: typeof raw.rrfVecWeight === "number" ? raw.rrfVecWeight : def.rrfVecWeight,
    rrfK: typeof raw.rrfK === "number" ? raw.rrfK : def.rrfK,
    autoMemorize: typeof raw.autoMemorize === "boolean" ? raw.autoMemorize : def.autoMemorize,
  };
}

function normalizeSettings(settings: AppSettings): AppSettings {
  const providers = settings.model.providers?.length
    ? settings.model.providers.map((provider) => ({
        ...provider,
        kind: normalizeProviderKind(provider.kind),
        models: provider.models.map((model) => ({
          ...model,
          providerKind: normalizeProviderKind(model.providerKind),
        })),
      }))
    : makeDefaultModelProviders();
  const activeModel =
    settings.model.activeModelRef ??
    providers[0]?.models[0] ?? {
      providerId: "mock-local",
      providerKind: "mock",
      modelId: settings.model.modelId || "mock",
      label: settings.model.modelId || "mock",
      family: "Mock",
      local: true,
      installed: true,
      ready: true,
    };
  const activeModelRef: ModelRef = {
    ...activeModel,
    providerKind: normalizeProviderKind(activeModel.providerKind),
  };
  return {
    ...settings,
    model: {
      ...settings.model,
      providers,
      activeModelRef,
      fallbackPolicy: settings.model.fallbackPolicy ?? "local-first",
      provider: normalizeProviderKind(activeModelRef.providerKind ?? settings.model.provider),
      modelId: activeModelRef.modelId ?? settings.model.modelId,
    },
    voice: {
      ...settings.voice,
      wakeWord: settings.voice.wakeWord ?? {
        enabled: false,
        runtime: "disabled",
        modelPath: null,
        threshold: 0.65,
      },
    },
    search: {
      ...(settings.search ?? {
        provider: "google-scrape",
        endpoint: null,
        authConfigured: false,
      }),
      apiKey: null,
      authConfigured: settings.search?.authConfigured ?? false,
    },
    notification: settings.notification ?? {
      desktopNotificationsEnabled: true,
      soundEnabled: true,
      agentCompletionNotifications: true,
      routineNotifications: true,
      emailNotificationsEnabled: false,
      emailOnAgentCompletion: false,
      emailOnRoutineSummary: false,
      emailRecipient: null,
      emailProvider: "smtp",
      smtpHost: null,
      smtpPort: 587,
      smtpUser: null,
      smtpPassword: null,
      smtpFrom: "noreply@aro-ai.com",
      smtpTlsMode: "starttls",
      apiKey: null,
      authConfigured: false,
    },
    memory: normalizeMemorySettings(settings.memory),
  };
}

function makeProvider(
  id: string,
  kind: ModelProviderKind,
  displayName: string,
  enabled: boolean,
  endpoint: string | null,
  modelIds: string[],
): ModelProviderConnection {
  const now = new Date().toISOString();
  return {
    id,
    kind,
    displayName,
    enabled,
    endpoint,
    authConfigured: false,
    models: modelIds.map((modelId) => ({
      providerId: id,
      providerKind: kind,
      modelId,
      label: modelId,
      family: displayName,
      local: kind === "mock" || kind === "ollama" || kind === "llama-cpp",
      installed: true,
      ready: true,
    })),
    createdAt: now,
    updatedAt: now,
  };
}

function isLocalProviderKind(kind: ModelProviderKind): boolean {
  return kind === "mock" || kind === "ollama" || kind === "llama-cpp";
}

function makeDefaultModelProviders(): ModelProviderConnection[] {
  return [
    makeProvider("mock-local", "mock", "Mock", true, null, ["mock", "gemma3:1b"]),
    makeProvider("ollama-local", "ollama", "Ollama", true, "http://127.0.0.1:11434", ["gemma3:1b"]),
    makeProvider("llama-cpp-local", "llama-cpp", "llama.cpp", true, "http://127.0.0.1:8080", ["local-model"]),
  ];
}

let demoSettings: AppSettings = {
  model: {
    providers: makeDefaultModelProviders(),
    activeModelRef: {
      providerId: "mock-local",
      providerKind: "mock",
      modelId: "gemma3:1b",
      label: "gemma3:1b",
      family: "Mock",
      local: true,
      installed: true,
      ready: true,
    },
    fallbackPolicy: "local-first",
    provider: "mock",
    modelId: "gemma3:1b",
    ollamaEndpoint: "http://127.0.0.1:11434",
    llamaCppEndpoint: "http://127.0.0.1:8080",
    temperature: 0.7,
    maxTokens: 768,
  },
  voice: {
    enabled: false,
    speechToText: "disabled",
    textToSpeech: "disabled",
    whisperBinary: null,
    whisperModelPath: null,
    piperBinary: null,
    piperVoicePath: null,
    wakeWord: {
      enabled: false,
      runtime: "disabled",
      modelPath: null,
      threshold: 0.65,
    },
  },
  search: {
    provider: "google-scrape",
    apiKey: null,
    authConfigured: false,
    endpoint: null,
  },
  notification: {
    desktopNotificationsEnabled: true,
    soundEnabled: true,
    agentCompletionNotifications: true,
    routineNotifications: true,
    emailNotificationsEnabled: false,
    emailOnAgentCompletion: false,
    emailOnRoutineSummary: false,
    emailRecipient: null,
    emailProvider: "smtp",
    smtpHost: null,
    smtpPort: 587,
    smtpUser: null,
    smtpPassword: null,
    smtpFrom: "noreply@aro-ai.com",
    smtpTlsMode: "starttls",
    apiKey: null,
    authConfigured: false,
  },
  retainHistory: true,
  speakResponses: false,
  memory: makeDefaultMemorySettings(),
};

function makeVoiceIssue(
  code: VoiceReadinessIssueCode,
  capability: VoiceCapability | null,
  setting: VoiceSettingKey | null,
  message: string,
  path: string | null = null,
): VoiceReadinessIssue {
  return { code, capability, setting, message, path };
}

function makeDemoCapabilityStatus(
  capability: "speech-to-text" | "text-to-speech",
  runtime: VoiceRuntimeKind,
  voiceEnabled: boolean,
): VoiceCapabilityStatus {
  const issues: VoiceReadinessIssue[] = [];

  if (runtime === "disabled") {
    issues.push(makeVoiceIssue(
      "runtime-disabled",
      capability,
      null,
      capability === "speech-to-text"
        ? "Speech-to-text is disabled in settings."
        : "Text-to-speech is disabled in settings.",
    ));
  } else if (voiceEnabled) {
    issues.push(makeVoiceIssue(
      "runtime-unavailable",
      capability,
      null,
      "Local voice runtimes are only available in the Tauri desktop app.",
    ));
  }

  return {
    capability,
    runtime,
    ready: false,
    issues,
  };
}

function makeDemoVoiceStatus(settings: AppSettings = demoSettings): VoiceStatus {
  const speechToText = makeDemoCapabilityStatus(
    "speech-to-text",
    settings.voice.speechToText,
    settings.voice.enabled,
  );
  const textToSpeech = makeDemoCapabilityStatus(
    "text-to-speech",
    settings.voice.textToSpeech,
    settings.voice.enabled,
  );
  const wakeWordIssues: VoiceReadinessIssue[] = [];
  if (!settings.voice.wakeWord.enabled || settings.voice.wakeWord.runtime === "disabled") {
    wakeWordIssues.push(makeVoiceIssue(
      "runtime-disabled",
      "wake-word",
      null,
      "Wake-word detection is disabled in settings.",
    ));
  } else if (settings.voice.enabled) {
    wakeWordIssues.push(makeVoiceIssue(
      "runtime-unavailable",
      "wake-word",
      null,
      "Local wake-word models are only available in the Tauri desktop app.",
    ));
  }
  const wakeWord = {
    enabled: settings.voice.wakeWord.enabled,
    runtime: settings.voice.wakeWord.runtime,
    ready: false,
    modelPath: settings.voice.wakeWord.modelPath ?? null,
    threshold: settings.voice.wakeWord.threshold,
    issues: wakeWordIssues,
  };
  const issues = [
    ...(settings.voice.enabled
      ? []
      : [makeVoiceIssue("voice-disabled", null, null, "Voice is disabled in settings.")]),
    ...speechToText.issues,
    ...textToSpeech.issues,
    ...wakeWord.issues,
  ];

  return {
    enabled: settings.voice.enabled,
    ready: false,
    speechToText,
    textToSpeech,
    wakeWord,
    issues,
    checkedAt: new Date().toISOString(),
  };
}

function makeVoiceModelReadiness(
  capability: VoiceModelCapability,
  runtime: VoiceModelRuntimeKind,
  ready: boolean,
  optional: boolean,
  binaryPath: string | null | undefined,
  modelPath: string | null | undefined,
  detail: string,
  issues: VoiceReadinessIssue[],
): VoiceModelReadiness {
  return {
    capability,
    runtime,
    ready,
    optional,
    binaryPath: binaryPath ?? null,
    modelPath: modelPath ?? null,
    detail,
    issues,
  };
}

function makeDemoVoiceModelStatus(
  settings: AppSettings = demoSettings,
  voiceStatus: VoiceStatus = makeDemoVoiceStatus(settings),
): VoiceModelsStatus {
  const speechToText = makeVoiceModelReadiness(
    "speech-to-text",
    settings.voice.speechToText,
    voiceStatus.speechToText.ready,
    false,
    settings.voice.whisperBinary,
    settings.voice.whisperModelPath,
    voiceStatus.speechToText.ready
      ? "whisper.cpp speech-to-text model is ready."
      : settings.voice.speechToText === "disabled"
        ? "Speech-to-text is disabled."
        : "whisper.cpp speech-to-text model needs configuration.",
    voiceStatus.speechToText.issues,
  );
  const textToSpeech = makeVoiceModelReadiness(
    "text-to-speech",
    settings.voice.textToSpeech,
    voiceStatus.textToSpeech.ready,
    false,
    settings.voice.piperBinary,
    settings.voice.piperVoicePath,
    voiceStatus.textToSpeech.ready
      ? "Piper text-to-speech voice model is ready."
      : settings.voice.textToSpeech === "disabled"
        ? "Text-to-speech is disabled."
        : "Piper text-to-speech voice model needs configuration.",
    voiceStatus.textToSpeech.issues,
  );
  const wakeWordIssues = [
    ...voiceStatus.issues.filter((issue) => issue.code === "voice-disabled"),
    ...voiceStatus.wakeWord.issues,
  ];
  const wakeWord = makeVoiceModelReadiness(
    "wake-word",
    settings.voice.wakeWord.runtime,
    voiceStatus.wakeWord.ready,
    true,
    null,
    settings.voice.wakeWord.modelPath,
    voiceStatus.wakeWord.ready
      ? "Wake-word local model is ready."
      : settings.voice.wakeWord.runtime === "disabled"
        ? "Wake-word detection is disabled."
        : "Wake-word local model needs configuration.",
    wakeWordIssues,
  );

  return {
    enabled: settings.voice.enabled,
    ready: voiceStatus.ready,
    voiceStatus,
    speechToText,
    textToSpeech,
    wakeWord,
    models: [speechToText, textToSpeech, wakeWord],
    checkedAt: voiceStatus.checkedAt,
  };
}

let demoConversations: Conversation[] = [];
let demoProjects: Project[] = [];
let demoFolders: Folder[] = [];
let demoMessages: Record<string, ChatMessage[]> = {};
let demoMemories: LongTermMemoryItem[] = [];
let demoModels: ModelOption[] = [
  normalizeModelRef({
    providerId: "mock-local",
    providerKind: "mock",
    modelId: "gemma3:1b",
    label: "gemma3:1b",
    family: "Mock",
    local: true,
    installed: true,
    ready: true,
  }),
  normalizeModelRef({
    providerId: "ollama-local",
    providerKind: "ollama",
    modelId: "phi3:mini",
    label: "phi3:mini",
    family: "Ollama",
    local: true,
    installed: true,
    ready: true,
  }),
];
let demoCloudSession: CloudSessionView | null = null;
let demoMembers: OrganizationMember[] = [];
let demoOrganizations: Organization[] = [];
let demoPreferences: UserPreferences | null = null;
let demoAgentRuns: AgentRun[] = [];
let demoAgentRunViews: Record<string, AgentRunView> = {};
let demoAgentLanes: AgentLaneView[] = [];
let demoPermissionProfiles: PermissionProfile[] = [];


export async function bootstrap(): Promise<BootstrapPayload> {
  if (isTauri()) return normalizeBootstrap(await invoke("app_bootstrap"));

  // Web mode: try real API first if a token exists
  if (isWeb() && webToken()) {
    try {
      const cloud = await webFetch<BootstrapPayload>("GET", "/bootstrap");
      // Persist token-carried conversations locally so UI stays consistent
      demoConversations = cloud.conversations ?? [];
      return normalizeBootstrap({
        ...cloud,
        cloudAuthenticated: true,
        apiBaseUrl: WEB_API_BASE,
      });
    } catch (err) {
      // If unauthorized clear token and fall through to local demo
      if (String(err).includes("401") || String(err).includes("403")) webTokenSet(null);
    }
  }

  return normalizeBootstrap({
    settings: demoSettings,
    conversations: demoConversations,
    runtime: demoRuntime(),
    models: demoModels,
    modelProviders: demoSettings.model.providers,
    currentUser: demoCloudSession?.user ?? null,
    activeOrganization: demoCloudSession?.activeOrganization ?? null,
    memberships: demoCloudSession?.memberships ?? [],
    preferences: demoPreferences,
    syncStatus: { health: "online", pendingEvents: 0, lastSyncedAt: new Date().toISOString() },
    clientState: {},
    cloudAuthenticated: Boolean(demoCloudSession),
    apiBaseUrl: "web-demo",
  });
}

export async function getCloudSession(): Promise<CloudSessionView | null> {
  if (isTauri()) return invoke("cloud_session_get");
  // In web mode, we derive the session from the bootstrap payload if a token is stored
  if (isWeb() && webToken()) {
    try {
      const payload = await webFetch<BootstrapPayload>("GET", "/bootstrap");
      return {
        user: payload.currentUser!,
        activeOrganization: payload.activeOrganization!,
        memberships: payload.memberships ?? [],
        expiresAt: new Date(Date.now() + 15 * 60 * 1000).toISOString(),
      };
    } catch {
      return null;
    }
  }
  return demoCloudSession;
}

export async function switchCloudOrganization(organizationId: string): Promise<CloudSessionView> {
  if (isTauri()) return invoke("cloud_organization_switch", { organizationId });
  if (isWeb() && webToken()) {
    return withWebSessionMutation(async () => {
      let accessToken = webToken();
      let refreshToken = webRefreshToken();
      if (!accessToken || !refreshToken) throw new Error("Cloud session required");

      const sendSwitch = () => fetch(`${WEB_API_BASE}/auth/switch-organization`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Authorization": `Bearer ${accessToken}`,
        },
        body: JSON.stringify({ organizationId, refreshToken }),
      });

      let response: Response | null = null;
      try {
        response = await sendSwitch();
        if (response.status === 401) {
          const refreshed = await refreshWebSessionLocked(refreshToken);
          if (!refreshed) throw new Error("Cloud session expired");
          accessToken = refreshed.accessToken;
          refreshToken = refreshed.refreshToken;
          response = null;
          response = await sendSwitch();
        }
        if (response.status === 401 && webRefreshToken() === refreshToken) {
          clearWebAuthSession();
        }
        const session = await readWebResponse<WebAuthSession>(response);
        return applyWebAuthSession(session);
      } catch (error) {
        // A transport or decoding failure is ambiguous after submitting a rotation. Keeping the
        // predecessor would make the next request look like credential theft/replay.
        if (response === null || response.ok) clearWebAuthSession();
        throw error;
      }
    });
  }
  if (!demoCloudSession) throw new Error("Cloud session required");
  const organization = demoOrganizations.find((item) => item.id === organizationId)
    ?? demoCloudSession.activeOrganization;
  demoCloudSession = { ...demoCloudSession, activeOrganization: organization };
  return demoCloudSession;
}


export async function registerCloud(
  request: CloudRegisterRequest,
): Promise<CloudSessionView> {
  if (isTauri()) return invoke("cloud_auth_register", { request });
  if (isWeb()) {
    return withWebSessionMutation(async () => applyWebAuthSession(
      await webFetch<WebAuthSession>("POST", "/auth/register", {
        email: request.email,
        password: request.password,
        name: request.name,
        organizationName: request.organizationName,
        organizationDomain: request.organizationDomain ?? undefined,
      }, false),
    ));
  }
  demoCloudSession = makeDemoSession(request.email, request.name, request.organizationName ?? "ARO Demo");
  return demoCloudSession;
}

export async function loginCloud(request: CloudLoginRequest): Promise<CloudSessionView> {
  if (isTauri()) return invoke("cloud_auth_login", { request });
  if (isWeb()) {
    return withWebSessionMutation(async () => applyWebAuthSession(
      await webFetch<WebAuthSession>("POST", "/auth/login", {
        email: request.email,
        password: request.password,
      }, false),
    ));
  }
  demoCloudSession = makeDemoSession(request.email, request.email.split("@")[0] || "ARO User", "ARO Demo");
  return demoCloudSession;
}

export async function requestPasswordReset(email: string): Promise<{ success: boolean; message: string; devTokenUrl?: string }> {
  if (isTauri()) return invoke("auth_password_reset_request", { email });
  if (isWeb() && webToken()) {
    try {
      return await webFetch<{ success: boolean; message: string; devTokenUrl?: string }>("POST", "/auth/password-reset/request", { email });
    } catch {
      /* fallthrough */
    }
  }
  const token = `demo-reset-${Date.now()}`;
  return {
    success: true,
    message: `Un e-mail de réinitialisation avec template HTML responsive a été simulé pour ${email}.`,
    devTokenUrl: `aro://auth/reset-password?token=${token}`,
  };
}

export async function confirmPasswordReset(token: string, newPassword: string): Promise<{ success: boolean; message: string }> {
  if (isTauri()) return invoke("auth_password_reset_confirm", { token, newPassword });
  if (isWeb() && webToken()) {
    try {
      return await webFetch<{ success: boolean; message: string }>("POST", "/auth/password-reset/confirm", { token, newPassword });
    } catch {
      /* fallthrough */
    }
  }
  return {
    success: true,
    message: "Votre mot de passe a été mis à jour avec succès !",
  };
}

export async function sendDesktopNotification(title: string, body: string, conversationId?: string): Promise<void> {
  if (isTauri()) {
    return invoke("notify_desktop_os", { title, body, conversationId });
  }
  if (typeof Notification !== "undefined" && Notification.permission === "granted") {
    new Notification(title, { body, icon: "/logo.png" });
  } else if (typeof Notification !== "undefined" && Notification.permission !== "denied") {
    Notification.requestPermission().then((perm) => {
      if (perm === "granted") new Notification(title, { body, icon: "/logo.png" });
    });
  }
}

export async function acceptCloudInvitation(
  token: string,
  email: string,
  password: string,
): Promise<CloudSessionView> {
  if (isTauri()) return invoke("cloud_invitation_accept", { token, email, password });
  if (isWeb()) {
    return withWebSessionMutation(async () => applyWebAuthSession(
      await webFetch<WebAuthSession>(
        "POST",
        "/auth/invitations/accept-account",
        { token, email, password },
        false,
      ),
    ));
  }
  throw new Error("Invitation acceptance requires ARO Cloud");
}

export async function logoutCloud(): Promise<void> {
  if (isTauri()) return invoke("cloud_auth_logout");
  if (isWeb()) {
    return withWebSessionMutation(async () => {
      const refreshToken = webRefreshToken();
      try {
        if (refreshToken) {
          await webFetch<void>("POST", "/auth/logout", { refreshToken }, false);
        }
      } finally {
        clearWebAuthSession();
        demoConversations = [];
        demoMessages = {};
      }
    });
  }
  demoCloudSession = null;
  demoMembers = [];
  demoOrganizations = [];
  demoPreferences = null;
}

export async function setCloudState(key: string, value: unknown): Promise<void> {
  if (isTauri()) return invoke("cloud_state_set", { key, value });
  if (isWeb() && webToken()) {
    await webFetch<void>("PUT", `/client-state/${encodeURIComponent(key)}`, { value });
  }
}

export async function deleteCloudState(key: string): Promise<void> {
  if (isTauri()) return invoke("cloud_state_delete", { key });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/client-state/${encodeURIComponent(key)}`);
  }
}

export async function listCloudOrganizations(): Promise<Organization[]> {
  if (isTauri()) return invoke("cloud_organizations_list");
  if (isWeb() && webToken()) {
    try { return await webFetch<Organization[]>("GET", "/organizations"); } catch { /* fallthrough */ }
  }
  return demoOrganizations;
}


export async function createCloudOrganization(
  request: OrganizationCreateRequest,
): Promise<CloudSessionView> {
  if (isTauri()) return invoke("cloud_organization_create", { request });
  if (isWeb() && webToken()) {
    // Create org then switch to it
    await webFetch<Organization>("POST", "/organizations", request);
    const orgs = await webFetch<Organization[]>("GET", "/organizations");
    const created = orgs.find((o) => o.name === request.name) ?? orgs[0];
    if (created) {
      return switchCloudOrganization(created.id);
    }
  }
  if (!demoCloudSession) {
    demoCloudSession = makeDemoSession("demo@aro.local", "ARO User", request.name);
    return demoCloudSession;
  }
  const now = new Date().toISOString();
  const organization: Organization = {
    id: crypto.randomUUID(),
    name: request.name,
    domain: request.domain ?? null,
    description: request.description ?? null,
    createdAt: now,
    updatedAt: now,
  };
  const membership = {
    id: crypto.randomUUID(),
    userId: demoCloudSession.user.id,
    organizationId: organization.id,
    role: "owner" as const,
    status: "active" as const,
    createdAt: now,
    updatedAt: now,
  };
  demoCloudSession = {
    ...demoCloudSession,
    activeOrganization: organization,
    memberships: [...demoCloudSession.memberships, membership],
  };
  demoOrganizations = [...demoOrganizations, organization];
  demoMembers = [{
    id: membership.id,
    userId: demoCloudSession.user.id,
    organizationId: organization.id,
    name: demoCloudSession.user.name,
    email: demoCloudSession.user.email,
    role: "owner",
    status: "active",
    createdAt: now,
    updatedAt: now,
  }];
  return demoCloudSession;
}

export async function getCloudCollection<T = unknown>(collection: string): Promise<T> {
  if (isTauri()) return invoke("cloud_collection_get", { collection });
  if (isWeb() && webToken()) {
    try { return await webFetch<T>("GET", `/collections/${collection}`); } catch { /* fallthrough */ }
  }
  return [] as T;
}

export async function createCloudCollectionItem<T = unknown>(
  collection: string,
  payload: unknown,
): Promise<T> {
  if (isTauri()) return invoke("cloud_collection_create", { collection, payload });
  if (isWeb() && webToken()) {
    return webFetch<T>("POST", `/collections/${collection}`, payload);
  }
  return payload as T;
}

export async function updateCloudCollectionItem<T = unknown>(
  collection: string,
  itemId: string,
  payload: unknown,
): Promise<T> {
  if (isTauri()) {
    return invoke("cloud_collection_update", { collection, itemId, payload });
  }
  if (isWeb() && webToken()) {
    return webFetch<T>("PATCH", `/collections/${collection}/${itemId}`, payload);
  }
  return { ...(payload as object), id: itemId } as T;
}

export async function deleteCloudCollectionItem(
  collection: string,
  itemId: string,
): Promise<void> {
  if (isTauri()) return invoke("cloud_collection_delete", { collection, itemId });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/collections/${collection}/${itemId}`);
  }
}

export type TaskStepStatus = "pending" | "in_progress" | "completed" | "error";

export interface TaskStep {
  id: string;
  text: string;
  completed: boolean;
  status?: TaskStepStatus;
  error?: string | null;
}

export interface Plan {
  id: string;
  conversationId: string;
  title: string;
  description?: string;
  tasks: TaskStep[];
  status: "active" | "completed" | "archived";
  createdAt: string;
  updatedAt: string;
}

export async function listPlans(conversationId: string): Promise<Plan[]> {
  if (isTauri()) {
    try {
      return await invoke<Plan[]>("plans_list", { conversationId });
    } catch (e) {
      console.error("Tauri plans_list failed:", e);
      return [];
    }
  }
  return getCloudCollection<Plan[]>(`plans?conversation_id=${conversationId}`);
}

export async function createPlan(plan: Omit<Plan, "id" | "status" | "createdAt" | "updatedAt">): Promise<void> {
  const fullPlan: Plan = {
    ...plan,
    id: crypto.randomUUID(),
    status: "active",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };

  if (isTauri()) {
    return invoke("plan_create", { plan: fullPlan });
  }
  await createCloudCollectionItem("plans", fullPlan);
}

export async function updatePlan(plan: Plan): Promise<void> {
  const updatedPlan = {
    ...plan,
    updatedAt: new Date().toISOString(),
  };

  if (isTauri()) {
    return invoke("plan_update", { plan: updatedPlan });
  }
  await updateCloudCollectionItem("plans", plan.id, updatedPlan);
}

export async function deletePlan(id: string): Promise<void> {
  if (isTauri()) {
    return invoke("plan_delete", { id });
  }
  await deleteCloudCollectionItem("plans", id);
}

export async function updateCloudUserProfile(request: UserProfilePatch): Promise<User> {
  if (isTauri()) return invoke("cloud_user_profile_update", { request });
  if (isWeb() && webToken()) {
    return webFetch<User>("PATCH", "/users/me", request);
  }
  const updated = {
    ...(demoCloudSession?.user ?? makeDemoSession("demo@aro.local", "ARO User", "ARO Demo").user),
    name: request.name ?? demoCloudSession?.user.name ?? "ARO User",
    roleTitle: request.roleTitle ?? demoCloudSession?.user.roleTitle ?? null,
    avatarColor: request.avatarColor ?? demoCloudSession?.user.avatarColor ?? null,
    updatedAt: new Date().toISOString(),
  };
  if (demoCloudSession) demoCloudSession = { ...demoCloudSession, user: updated };
  demoMembers = demoMembers.map((member) =>
    member.userId === updated.id ? { ...member, name: updated.name, email: updated.email } : member,
  );
  return updated;
}


export async function updateCloudOrganization(
  organizationId: string,
  request: OrganizationPatch,
): Promise<Organization> {
  if (isTauri()) return invoke("cloud_organization_update", { organizationId, request });
  if (isWeb() && webToken()) {
    return webFetch<Organization>("PATCH", `/organizations/${organizationId}`, request);
  }
  const current = demoCloudSession?.activeOrganization ?? makeDemoSession("demo@aro.local", "ARO User", "ARO Demo").activeOrganization;
  const updated = {
    ...current,
    name: request.name ?? current.name,
    domain: request.domain ?? current.domain ?? null,
    description: request.description ?? current.description ?? null,
    updatedAt: new Date().toISOString(),
  };
  if (demoCloudSession && demoCloudSession.activeOrganization.id === organizationId) {
    demoCloudSession = { ...demoCloudSession, activeOrganization: updated };
  }
  return updated;
}


export async function listCloudApiKeys(): Promise<PublicApiKey[]> {
  if (isTauri()) return invoke("cloud_api_keys_list");
  if (isWeb() && webToken()) {
    try { return await webFetch<PublicApiKey[]>("GET", "/api-keys"); } catch { /* fallthrough */ }
  }
  return [];
}

export async function createCloudApiKey(name: string): Promise<ApiKeyCreateResponse> {
  if (isTauri()) return invoke("cloud_api_key_create", { name });
  if (isWeb() && webToken()) {
    return webFetch<ApiKeyCreateResponse>("POST", "/api-keys", { name });
  }
  const now = new Date().toISOString();
  const secret = `aro_live_${crypto.randomUUID().replaceAll("-", "")}`;
  return {
    key: {
      id: crypto.randomUUID(),
      organizationId: demoCloudSession?.activeOrganization.id ?? crypto.randomUUID(),
      name,
      prefix: secret.slice(0, 14),
      createdAt: now,
      lastUsedAt: null,
    },
    secret,
  };
}

export async function revokeCloudApiKey(apiKeyId: string): Promise<void> {
  if (isTauri()) return invoke("cloud_api_key_revoke", { apiKeyId });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/api-keys/${apiKeyId}`);
  }
}


export async function listCloudMembers(): Promise<OrganizationMember[]> {
  if (isTauri()) return invoke("cloud_members_list");
  if (isWeb() && webToken()) {
    try { return await webFetch<OrganizationMember[]>("GET", "/memberships"); } catch { /* fallthrough */ }
  }
  return demoMembers;
}

export async function listCloudInvitations(
  cursor: string | null = null,
  limit = 100,
  includeClosed = false,
): Promise<OrganizationInvitationPage> {
  const boundedLimit = Math.max(1, Math.min(200, Math.trunc(limit)));
  if (isTauri()) {
    return invoke("cloud_invitations_list", { cursor, limit: boundedLimit, includeClosed });
  }
  if (isWeb() && webToken()) {
    const query = new URLSearchParams({
      limit: String(boundedLimit),
      includeClosed: String(includeClosed),
    });
    if (cursor) query.set("cursor", cursor);
    return webFetch<OrganizationInvitationPage>("GET", `/invitations?${query.toString()}`);
  }
  return { items: [], nextCursor: null };
}

export async function listAllCloudInvitations(
  includeClosed = false,
): Promise<OrganizationInvitation[]> {
  const invitations: OrganizationInvitation[] = [];
  const invitationIds = new Set<string>();
  const seenCursors = new Set<string>();
  let cursor: string | null = null;

  // A hard page bound prevents a faulty or hostile server from keeping the client in an
  // unbounded startup loop while still allowing large organizations to reach older invites.
  for (let pageNumber = 0; pageNumber < 100; pageNumber += 1) {
    const page = await listCloudInvitations(cursor, 200, includeClosed);
    for (const invitation of page.items) {
      if (!invitationIds.has(invitation.id)) {
        invitationIds.add(invitation.id);
        invitations.push(invitation);
      }
    }
    if (!page.nextCursor) return invitations;
    if (seenCursors.has(page.nextCursor)) {
      throw new Error("invitation pagination returned a repeated cursor");
    }
    seenCursors.add(page.nextCursor);
    cursor = page.nextCursor;
  }

  throw new Error("invitation pagination exceeded the safety limit");
}

export async function revokeCloudInvitation(invitationId: string): Promise<void> {
  if (isTauri()) return invoke("cloud_invitation_revoke", { invitationId });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/invitations/${encodeURIComponent(invitationId)}`);
  }
}

export async function inviteCloudMember(
  request: MembershipCreateRequest,
): Promise<OrganizationInvitationReceipt> {
  if (isTauri()) return invoke("cloud_member_invite", { request });
  if (isWeb() && webToken()) {
    return webFetch<OrganizationInvitationReceipt>("POST", "/memberships", request);
  }
  const now = new Date().toISOString();
  const member: OrganizationMember = {
    id: crypto.randomUUID(),
    userId: crypto.randomUUID(),
    organizationId: demoCloudSession?.activeOrganization.id ?? crypto.randomUUID(),
    name: request.name,
    email: request.email,
    role: request.role,
    status: "invited",
    createdAt: now,
    updatedAt: now,
  };
  demoMembers = [member, ...demoMembers];
  return {
    invitationId: member.id,
    organizationId: member.organizationId,
    email: member.email,
    name: member.name,
    role: request.role,
    status: "pending",
    expiresAt: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(),
    member,
  };
}

export async function updateCloudMemberRole(
  membershipId: string,
  role: Exclude<MembershipRole, "owner">,
): Promise<OrganizationMember> {
  if (isTauri()) return invoke("cloud_member_role_update", { membershipId, role });
  if (isWeb() && webToken()) {
    return webFetch<OrganizationMember>("PATCH", `/memberships/${membershipId}`, { role });
  }
  const member = demoMembers.find((item) => item.id === membershipId);
  if (!member) throw new Error("Member not found");
  member.role = role;
  member.updatedAt = new Date().toISOString();
  return member;
}

export async function removeCloudMember(membershipId: string): Promise<void> {
  if (isTauri()) return invoke("cloud_member_remove", { membershipId });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/memberships/${membershipId}`);
    return;
  }
  demoMembers = demoMembers.filter((item) => item.id !== membershipId);
}


export async function createConversation(
  title: string,
  mode: AssistantMode,
  projectId?: string | null,
  folderId?: string | null,
  organizationId?: string | null,
): Promise<Conversation> {
  const placement = {
    projectId: projectId ?? null,
    folderId: folderId ?? null,
  };
  if (isTauri()) {
    const conv = await invoke<Conversation>("conversation_create", {
      request: { title, mode, projectId: placement.projectId, folderId: placement.folderId },
    });
    if (organizationId) conv.organizationId = organizationId;
    return conv;
  }
  if (isWeb() && webToken()) {
    const conv = await webFetch<Conversation>("POST", "/conversations", { title, mode, ...placement });
    if (organizationId) conv.organizationId = organizationId;
    demoConversations = [conv, ...demoConversations];
    demoMessages[conv.id] = [];
    return conv;
  }
  const conversation = makeConversation(title, mode, placement.projectId, placement.folderId, organizationId);
  demoConversations = [conversation, ...demoConversations];
  demoMessages[conversation.id] = [];
  return conversation;
}

export async function listConversations(): Promise<Conversation[]> {
  if (isTauri()) return invoke("conversation_list");
  if (isWeb() && webToken()) {
    try {
      const list = await webFetch<Conversation[]>("GET", "/conversations");
      demoConversations = list;
      return list;
    } catch { /* fallthrough */ }
  }
  return demoConversations;
}

export async function deleteConversation(conversationId: string): Promise<void> {
  if (isTauri()) return invoke("conversation_delete", { conversationId });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/conversations/${conversationId}`);
  }
  demoConversations = demoConversations.filter((item) => item.id !== conversationId);
  delete demoMessages[conversationId];
}

export async function deleteEmptyConversations(): Promise<string[]> {
  if (isTauri()) return invoke<string[]>("conversations_delete_empty");
  const emptyIds = demoConversations
    .filter((c) => (demoMessages[c.id] ?? []).length === 0)
    .map((c) => c.id);
  demoConversations = demoConversations.filter((item) => !emptyIds.includes(item.id));
  for (const id of emptyIds) delete demoMessages[id];
  return emptyIds;
}

export async function updateConversationTitle(
  conversationId: string,
  title: string,
): Promise<Conversation> {
  if (isTauri()) return invoke("conversation_update_title", { conversationId, title });
  if (isWeb() && webToken()) {
    return webFetch<Conversation>("PATCH", `/conversations/${conversationId}`, { title });
  }
  const conversation = demoConversations.find((item) => item.id === conversationId);
  if (!conversation) throw new Error("Conversation not found");
  conversation.title = title;
  conversation.updatedAt = new Date().toISOString();
  return conversation;
}

export async function exportConversation(
  conversationId: string,
  format: "markdown" | "json" = "markdown",
): Promise<Blob> {
  if (isTauri()) {
    try {
      const bytes: number[] = await invoke("conversation_export", { conversationId, format });
      return new Blob([new Uint8Array(bytes)]);
    } catch {
      // Fallback if IPC is not wired up
    }
  }
  if (isWeb() && webToken()) {
    const token = webToken();
    const headers: Record<string, string> = {};
    if (token) headers["Authorization"] = `Bearer ${token}`;
    const resp = await fetch(`${WEB_API_BASE}/conversations/${conversationId}/export?format=${format}`, { headers });
    if (!resp.ok) throw new Error("Failed to export conversation");
    return resp.blob();
  }
  const conv = demoConversations.find((c) => c.id === conversationId);
  const title = conv?.title ?? "Conversation";
  const msgs = demoMessages[conversationId] ?? [];
  if (format === "json") {
    return new Blob([JSON.stringify({ conversation: conv, messages: msgs }, null, 2)], { type: "application/json" });
  }
  let md = `# ${title}\n\n`;
  for (const m of msgs) {
    md += `### **${m.role}**\n\n${m.content}\n\n`;
  }
  return new Blob([md], { type: "text/markdown" });
}

export async function exportProject(projectId: string): Promise<Blob> {
  const proj = demoProjects.find((p) => p.id === projectId);
  const projectConversations = demoConversations.filter((c) => c.projectId === projectId);
  const data = {
    project: proj,
    conversations: projectConversations,
    exportedAt: new Date().toISOString(),
  };
  return new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
}

export async function importConversations(jsonData: string): Promise<number> {
  try {
    const parsed = JSON.parse(jsonData);
    let count = 0;
    if (parsed.conversation && Array.isArray(parsed.messages)) {
      const conv = parsed.conversation;
      demoConversations = [conv, ...demoConversations];
      demoMessages[conv.id] = parsed.messages;
      count++;
    } else if (Array.isArray(parsed.conversations)) {
      for (const conv of parsed.conversations) {
        demoConversations = [conv, ...demoConversations];
        count++;
      }
    }
    return count;
  } catch {
    throw new Error("Invalid JSON format for conversation import");
  }
}

export async function searchConversationContent(query: string, limit = 20): Promise<ChatMessage[]> {
  if (!query.trim()) return [];
  if (isTauri()) {
    return invoke("conversation_search_content", { query, limit });
  }
  const allMsgs: ChatMessage[] = [];
  for (const list of Object.values(demoMessages)) {
    allMsgs.push(...list);
  }
  return allMsgs.filter((m) => m.content.toLowerCase().includes(query.toLowerCase())).slice(0, limit);
}

export async function listProjects(): Promise<Project[]> {
  if (isTauri()) return invoke("project_list");
  if (isWeb() && webToken()) {
    try {
      const list = await webFetch<Project[]>("GET", "/projects");
      demoProjects = list;
      return list;
    } catch { /* fallthrough */ }
  }
  return demoProjects;
}

export async function createProject(
  name: string,
  description?: string | null,
  instructions?: string | null,
  rootPath?: string | null,
  color?: string | null,
  icon?: string | null,
  organizationId?: string | null,
): Promise<Project> {
  if (isTauri()) return invoke("project_create", { name, description, instructions, rootPath, color, icon, organizationId });
  if (isWeb() && webToken()) {
    const project = await webFetch<Project>("POST", "/projects", { name, description, instructions, rootPath, color, icon, organizationId });
    demoProjects = [project, ...demoProjects];
    return project;
  }
  const now = new Date().toISOString();
  const project: Project = {
    id: `project-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
    name,
    description: description ?? null,
    instructions: instructions ?? null,
    rootPath: rootPath ?? null,
    color: color || "#3b82f6",
    icon: icon || "folder-tree",
    createdAt: now,
    updatedAt: now,
    organizationId: organizationId ?? null,
  };
  demoProjects = [project, ...demoProjects];
  return project;
}

export async function updateProject(
  id: string,
  updates: Partial<Pick<Project, "name" | "description" | "instructions" | "rootPath" | "color" | "icon">>,
): Promise<Project> {
  if (isTauri()) return invoke("project_update", { id, ...updates });
  if (isWeb() && webToken()) {
    return webFetch<Project>("PATCH", `/projects/${id}`, updates);
  }
  const project = demoProjects.find((item) => item.id === id);
  if (!project) throw new Error("Project not found");
  if (updates.name !== undefined) project.name = updates.name;
  if (updates.description !== undefined) project.description = updates.description;
  if (updates.instructions !== undefined) project.instructions = updates.instructions;
  if (updates.rootPath !== undefined) project.rootPath = updates.rootPath;
  if (updates.color !== undefined) project.color = updates.color;
  if (updates.icon !== undefined) project.icon = updates.icon;
  project.updatedAt = new Date().toISOString();
  return project;
}

export async function deleteProject(id: string): Promise<void> {
  if (isTauri()) return invoke("project_delete", { id });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/projects/${id}`);
  }
  demoProjects = demoProjects.filter((item) => item.id !== id);
  demoFolders = demoFolders.map((f) => (f.projectId === id ? { ...f, projectId: null } : f));
  demoConversations = demoConversations.map((c) => (c.projectId === id ? { ...c, projectId: null } : c));
}

export async function listFolders(): Promise<Folder[]> {
  if (isTauri()) return invoke("folder_list");
  if (isWeb() && webToken()) {
    try {
      const list = await webFetch<Folder[]>("GET", "/folders");
      demoFolders = list;
      return list;
    } catch { /* fallthrough */ }
  }
  return demoFolders;
}

export async function createFolder(
  name: string,
  projectId?: string | null,
  rootPath?: string | null,
  color?: string | null,
  icon?: string | null,
  organizationId?: string | null,
): Promise<Folder> {
  if (isTauri()) return invoke("folder_create", { name, projectId, rootPath, color, icon, organizationId });
  if (isWeb() && webToken()) {
    const folder = await webFetch<Folder>("POST", "/folders", { name, projectId, rootPath, color, icon, organizationId });
    demoFolders = [folder, ...demoFolders];
    return folder;
  }
  const now = new Date().toISOString();
  const folder: Folder = {
    id: `folder-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
    projectId: projectId ?? null,
    name,
    rootPath: rootPath ?? null,
    color: color ?? null,
    icon: icon ?? "folder",
    createdAt: now,
    updatedAt: now,
    organizationId: organizationId ?? null,
  };
  demoFolders = [folder, ...demoFolders];
  return folder;
}

export async function updateFolder(
  id: string,
  updates: Partial<Pick<Folder, "name" | "projectId" | "rootPath" | "color" | "icon">>,
): Promise<Folder> {
  if (isTauri()) return invoke("folder_update", { id, ...updates });
  if (isWeb() && webToken()) {
    return webFetch<Folder>("PATCH", `/folders/${id}`, updates);
  }
  const folder = demoFolders.find((item) => item.id === id);
  if (!folder) throw new Error("Folder not found");
  if (updates.name !== undefined) folder.name = updates.name;
  if (updates.projectId !== undefined) folder.projectId = updates.projectId;
  if (updates.rootPath !== undefined) folder.rootPath = updates.rootPath;
  if (updates.color !== undefined) folder.color = updates.color;
  if (updates.icon !== undefined) folder.icon = updates.icon;
  folder.updatedAt = new Date().toISOString();
  return folder;
}

export async function setConversationRootPath(
  conversationId: string,
  rootPath: string | null,
): Promise<Conversation> {
  if (isTauri()) return invoke("conversation_set_root_path", { conversationId, rootPath });
  if (isWeb() && webToken()) {
    return webFetch<Conversation>("PATCH", `/conversations/${conversationId}/root-path`, { rootPath });
  }
  const conv = demoConversations.find((c) => c.id === conversationId);
  if (!conv) throw new Error("Conversation not found");
  conv.rootPath = rootPath;
  conv.updatedAt = new Date().toISOString();
  return conv;
}

export async function getEffectiveRootPath(conversationId: string): Promise<string | null> {
  if (isTauri()) return invoke("conversation_effective_root_path", { conversationId });
  if (isWeb() && webToken()) {
    try {
      const res = await webFetch<{ rootPath: string | null }>("GET", `/conversations/${conversationId}/root-path`);
      return res.rootPath;
    } catch {
      return null;
    }
  }
  const conv = demoConversations.find((c) => c.id === conversationId);
  if (conv?.rootPath) return conv.rootPath;
  if (conv?.folderId) {
    const folder = demoFolders.find((f) => f.id === conv.folderId);
    if (folder?.rootPath) return folder.rootPath;
    if (folder?.projectId) {
      const parent = demoProjects.find((p) => p.id === folder.projectId);
      if (parent?.rootPath) return parent.rootPath;
    }
  }
  if (conv?.projectId) {
    const project = demoProjects.find((p) => p.id === conv.projectId);
    if (project?.rootPath) return project.rootPath;
  }
  return null;
}

export interface WorkspaceTreeEntry {
  name: string;
  path: string;
  relativePath: string;
  isDir: boolean;
  size: number;
}

export interface WorkspaceTreeResult {
  rootPath: string;
  entries: WorkspaceTreeEntry[];
  count: number;
  readError?: string | null;
}

interface RawTreeEntry {
  name?: string;
  path?: string;
  relativePath?: string;
  isDir?: boolean;
  size?: number;
}

function normalizeTreeEntries(rootPath: string, raw: RawTreeEntry[]): WorkspaceTreeEntry[] {
  return (raw ?? []).map((e) => {
    const abs: string = e.path ?? e.name ?? "";
    let rel = e.relativePath ?? abs;
    if (rootPath && abs.startsWith(rootPath)) {
      rel = abs.slice(rootPath.length).replace(/^[/\\]+/, "");
    }
    rel = (rel || e.name || "").replace(/\\/g, "/");
    if (!rel) rel = e.name ?? "";
    return {
      name: e.name ?? rel.split("/").pop() ?? "",
      path: abs,
      relativePath: rel || (e.name ?? ""),
      isDir: !!e.isDir,
      size: typeof e.size === "number" ? e.size : 0,
    };
  });
}

export async function selectFolderDialog(): Promise<string | null> {
  if (isTauri()) {
    return invoke<string | null>("select_folder_dialog");
  }
  if (typeof (window as any).showDirectoryPicker === "function") {
    try {
      const handle = await (window as any).showDirectoryPicker();
      return handle.name;
    } catch {
      return null;
    }
  }
  return null;
}

export async function getWorkspaceTree(conversationId?: string, path?: string): Promise<WorkspaceTreeResult> {
  if (isTauri()) {
    const raw = await invoke<{ rootPath: string; entries: RawTreeEntry[]; count: number; readError?: string | null }>("workspace_tree_get", {
      conversationId,
      path,
    });
    return { rootPath: raw.rootPath, entries: normalizeTreeEntries(raw.rootPath, raw.entries), count: raw.count, readError: raw.readError ?? null };
  }
  if (isWeb() && webToken()) {
    try {
      const raw = await webFetch<{ rootPath: string; entries: RawTreeEntry[]; count: number; readError?: string | null }>(
        "GET",
        `/workspace/tree?conversationId=${conversationId ?? ""}&path=${encodeURIComponent(path ?? "")}`
      );
      return { rootPath: raw.rootPath, entries: normalizeTreeEntries(raw.rootPath, raw.entries), count: raw.count, readError: raw.readError ?? null };
    } catch {
      return { rootPath: path || ".", entries: [], count: 0 };
    }
  }
  return { rootPath: path || ".", entries: [], count: 0 };
}

export async function readWorkspaceFile(path: string, conversationId?: string): Promise<string> {
  if (isTauri()) return invoke<string>("workspace_file_read", { path, conversationId });
  if (isWeb() && webToken()) {
    return webFetch<string>("GET", `/workspace/file?path=${encodeURIComponent(path)}&conversationId=${conversationId ?? ""}`);
  }
  return "";
}

export async function writeWorkspaceFile(path: string, content: string, conversationId?: string): Promise<void> {
  if (isTauri()) return invoke<void>("workspace_file_write", { path, content, conversationId });
  if (isWeb() && webToken()) {
    await webFetch<void>("PUT", `/workspace/file`, { path, content, conversationId });
  }
}

export async function applyWorkspaceDiff(path: string, diffPatch: string, conversationId?: string): Promise<void> {
  if (isTauri()) return invoke<void>("workspace_diff_apply", { path, diffPatch, conversationId });
  if (isWeb() && webToken()) {
    await webFetch<void>("POST", `/workspace/diff/apply`, { path, diffPatch, conversationId });
  }
}

export async function getWorkspaceGitDiff(path?: string, conversationId?: string): Promise<string> {
  if (isTauri()) return invoke<string>("workspace_git_diff", { path, conversationId });
  if (isWeb() && webToken()) {
    return webFetch<string>("GET", `/workspace/git/diff?path=${encodeURIComponent(path ?? "")}&conversationId=${conversationId ?? ""}`);
  }
  return "";
}

export async function deleteFolder(id: string): Promise<void> {
  if (isTauri()) return invoke("folder_delete", { id });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/folders/${id}`);
  }
  demoFolders = demoFolders.filter((item) => item.id !== id);
  demoConversations = demoConversations.map((c) => (c.folderId === id ? { ...c, folderId: null } : c));
}

export async function moveConversation(
  conversationId: string,
  projectId?: string | null,
  folderId?: string | null,
): Promise<Conversation> {
  if (isTauri()) return invoke("conversation_move", { conversationId, projectId, folderId });
  if (isWeb() && webToken()) {
    return webFetch<Conversation>("PATCH", `/conversations/${conversationId}/move`, { projectId, folderId });
  }
  const conversation = demoConversations.find((item) => item.id === conversationId);
  if (!conversation) throw new Error("Conversation not found");
  conversation.projectId = projectId ?? null;
  conversation.folderId = folderId ?? null;
  conversation.updatedAt = new Date().toISOString();
  return conversation;
}

export async function listMessages(conversationId: string): Promise<ChatMessage[]> {
  if (isTauri()) return invoke("message_list", { conversationId });
  if (isWeb() && webToken()) {
    try {
      const msgs = await webFetch<ChatMessage[]>("GET", `/conversations/${conversationId}/messages`);
      demoMessages[conversationId] = msgs;
      return msgs;
    } catch { /* fallthrough */ }
  }
  return demoMessages[conversationId] ?? [];
}

function normalizeMemoryItem(value: unknown): LongTermMemoryItem {
  const row = (value && typeof value === "object" ? value : {}) as Record<string, unknown>;
  const now = new Date().toISOString();
  const category = row.category === "technical" || row.category === "system" || row.category === "preference" || row.category === "personal"
    ? row.category
    : "personal";
  const scope = row.scope === "conversation" || row.scope === "organization" || row.scope === "project" || row.scope === "user"
    ? row.scope
    : "user";
  const status = row.status === "candidate" || row.status === "archived" || row.status === "approved"
    ? row.status
    : "approved";
  return {
    id: String(row.id ?? crypto.randomUUID()),
    cloudId: row.cloudId ? String(row.cloudId) : undefined,
    clientId: (row.clientId ?? row.client_id ?? null) as string | null,
    content: String(row.content ?? ""),
    category,
    scope,
    status,
    sourceConversationId: (row.sourceConversationId ?? row.source_conversation_id ?? null) as string | null,
    sourceMessageIds: Array.isArray(row.sourceMessageIds)
      ? row.sourceMessageIds.map(String)
      : Array.isArray(row.source_message_ids)
        ? row.source_message_ids.map(String)
        : [],
    pinned: Boolean(row.pinned ?? false),
    salience: Number(row.salience ?? 0.5),
    recallCount: Number(row.recallCount ?? row.recall_count ?? 0),
    lastUsedAt: (row.lastUsedAt ?? row.last_used_at ?? null) as string | null,
    createdAt: String(row.createdAt ?? row.created_at ?? now),
    updatedAt: String(row.updatedAt ?? row.updated_at ?? now),
  };
}

let demoEpisodes: EpisodeItem[] = [
  {
    id: "demo-ep-1",
    conversationId: "demo-conv-1",
    turnStart: 1,
    turnEnd: 10,
    summary: "Configuration initiale du pipeline de mémoire cognitive et définition des variables d'environnement.",
    keyDecisions: ["Adoption du mode WAL pour SQLite", "Découpage du prompt en 5 partitions étanches"],
    entities: ["port: 9464", "cluster: k8s-eu-west-3", "database: SQLite"],
    tokenCount: 1840,
    createdAt: new Date(Date.now() - 3600000 * 2).toISOString(),
    updatedAt: new Date(Date.now() - 3600000 * 2).toISOString(),
  },
  {
    id: "demo-ep-2",
    conversationId: "demo-conv-1",
    turnStart: 11,
    turnEnd: 20,
    summary: "Mise en place de l'algorithme Reciprocal Rank Fusion et intégration des outils de mémoire pour l'agent.",
    keyDecisions: ["RRF k=60 avec pondération FTS5 0.40 et Vector 0.60", "Double dispatch pour memory.search"],
    entities: ["algorithm: RRF", "vector_engine: Qdrant", "model: nomic-embed-text"],
    tokenCount: 2150,
    createdAt: new Date(Date.now() - 3600000).toISOString(),
    updatedAt: new Date(Date.now() - 3600000).toISOString(),
  },
];

function normalizeEpisodeItem(value: unknown): EpisodeItem {
  const row = (value && typeof value === "object" ? value : {}) as Record<string, unknown>;
  const now = new Date().toISOString();
  return {
    id: String(row.id ?? crypto.randomUUID()),
    conversationId: String(row.conversationId ?? row.conversation_id ?? ""),
    turnStart: Number(row.turnStart ?? row.turn_start ?? 0),
    turnEnd: Number(row.turnEnd ?? row.turn_end ?? 0),
    summary: String(row.summary ?? ""),
    keyDecisions: Array.isArray(row.keyDecisions)
      ? row.keyDecisions.map(String)
      : Array.isArray(row.key_decisions)
        ? row.key_decisions.map(String)
        : [],
    entities: Array.isArray(row.entities) ? row.entities.map(String) : [],
    tokenCount: Number(row.tokenCount ?? row.token_count ?? 0),
    createdAt: String(row.createdAt ?? row.created_at ?? now),
    updatedAt: String(row.updatedAt ?? row.updated_at ?? now),
  };
}

export async function listEpisodes(
  conversationId?: string,
  limit = 100
): Promise<EpisodeItem[]> {
  if (isTauri()) {
    return (
      await invoke<unknown[]>("memory_episodes", {
        conversationId: conversationId || null,
        limit,
      })
    ).map(normalizeEpisodeItem);
  }
  if (isWeb() && webToken()) {
    try {
      const q = conversationId ? `?conversation_id=${encodeURIComponent(conversationId)}` : "";
      const list = await webFetch<unknown[]>("GET", `/episodes${q}`);
      return list.map(normalizeEpisodeItem);
    } catch {
      /* fallthrough */
    }
  }
  if (conversationId) {
    return demoEpisodes.filter((ep) => ep.conversationId === conversationId);
  }
  return demoEpisodes;
}

export async function listMemoryItems(): Promise<LongTermMemoryItem[]> {
  if (isTauri()) {
    return (await invoke<unknown[]>("memory_list")).map(normalizeMemoryItem);
  }
  if (isWeb() && webToken()) {
    try {
      const list = await webFetch<unknown[]>("GET", "/memories");
      return list.map(normalizeMemoryItem);
    } catch { /* fallthrough */ }
  }
  return demoMemories;
}

export async function searchMemoryItems(
  query: string,
  limit = 20,
): Promise<LongTermMemoryItem[]> {
  if (isTauri()) {
    return (await invoke<unknown[]>("memory_search", { query, limit })).map(normalizeMemoryItem);
  }
  if (isWeb() && webToken()) {
    try {
      const q = encodeURIComponent(query);
      const list = await webFetch<unknown[]>("GET", `/memories/search?q=${q}&limit=${limit}`);
      return list.map(normalizeMemoryItem);
    } catch { /* fallthrough */ }
  }
  const normalized = query.trim().toLowerCase();
  return demoMemories
    .filter((memory) => memory.status === "approved")
    .filter((memory) => !normalized || memory.content.toLowerCase().includes(normalized) || memory.category.includes(normalized))
    .sort((left, right) => Number(right.pinned) - Number(left.pinned) || right.updatedAt.localeCompare(left.updatedAt))
    .slice(0, limit);
}

export async function upsertMemoryItem(
  memory: LongTermMemoryItem,
): Promise<LongTermMemoryItem> {
  const normalized = normalizeMemoryItem(memory);
  if (isTauri()) {
    return normalizeMemoryItem(await invoke("memory_upsert", { memory: normalized }));
  }
  if (isWeb() && webToken()) {
    try {
      const body = {
        content: normalized.content,
        category: normalized.category,
        pinned: normalized.pinned,
        scope: normalized.scope,
        status: normalized.status,
        sourceConversationId: normalized.sourceConversationId ?? null,
        sourceMessageIds: normalized.sourceMessageIds,
        salience: normalized.salience,
      };
      if (normalized.cloudId) {
        const updated = await webFetch<unknown>("PATCH", `/memories/${normalized.cloudId}`, body);
        return normalizeMemoryItem(updated);
      }
      const created = await webFetch<unknown>("POST", "/memories", body);
      return normalizeMemoryItem(created);
    } catch { /* fallthrough */ }
  }
  demoMemories = [
    normalized,
    ...demoMemories.filter((item) => item.id !== normalized.id),
  ];
  return normalized;
}

export async function deleteMemoryItem(memoryId: string): Promise<void> {
  if (isTauri()) return invoke("memory_delete", { memoryId });
  if (isWeb() && webToken()) {
    await webFetch<void>("DELETE", `/memories/${memoryId}`);
    return;
  }
  demoMemories = demoMemories.filter((item) => item.id !== memoryId);
}

export async function memoryIndexStatus(): Promise<MemoryIndexStatus> {
  if (isTauri()) return invoke("memory_index_status");
  if (isWeb() && webToken()) {
    try { return await webFetch<MemoryIndexStatus>("GET", "/memory-index/status"); } catch { /* fallthrough */ }
  }
  return {
    mode: "disabled",
    state: "disabled",
    qdrantUrl: "demo",
    collectionName: null,
    embeddingProvider: "mock",
    embeddingModel: "demo",
    dimension: null,
    indexedCount: demoMemories.length,
    message: null,
  };
}

export async function memoryIndexReindex(): Promise<MemoryReindexReport> {
  if (isTauri()) return invoke("memory_index_reindex");
  if (isWeb() && webToken()) {
    try { return await webFetch<MemoryReindexReport>("POST", "/memory-index/reindex", {}); } catch { /* fallthrough */ }
  }
  return {
    status: await memoryIndexStatus(),
    indexedCount: demoMemories.filter((memory) => memory.status === "approved").length,
    skippedCount: demoMemories.filter((memory) => memory.status !== "approved").length,
  };
}


export async function uploadCloudFile(file: File): Promise<FileObject> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  const sha256 = await sha256Hex(bytes);
  if (isTauri()) {
    return invoke("file_upload", {
      request: {
        originalName: file.name,
        mimeType: file.type || "application/octet-stream",
        bytes: Array.from(bytes),
        sha256: sha256 || null,
      },
    });
  }
  if (isWeb() && webToken()) {
    // Use multipart upload via the REST API
    const token = webToken();
    const formInit = await webFetch<{ uploadId: string; expiresAt: string }>("POST", "/files/uploads", {
      originalName: file.name,
      mimeType: file.type || "application/octet-stream",
      sizeBytes: file.size,
      sha256: sha256 || null,
    });
    const headers: Record<string, string> = {};
    if (token) headers["Authorization"] = `Bearer ${token}`;
    await fetch(`${WEB_API_BASE}/files/uploads/${formInit.uploadId}/content`, {
      method: "PUT",
      headers: { ...headers, "Content-Type": file.type || "application/octet-stream" },
      body: bytes,
    });
    return webFetch<FileObject>("GET", `/files/${formInit.uploadId}`);
  }
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    organizationId: "demo-org",
    ownerUserId: "demo-user",
    originalName: file.name,
    mimeType: file.type || "application/octet-stream",
    sizeBytes: file.size,
    sha256,
    status: "available",
    scanStatus: "skipped",
    createdAt: now,
    updatedAt: now,
  };
}


export async function downloadCloudFile(fileId: string): Promise<Blob> {
  if (isTauri()) {
    const bytes: number[] = await invoke("file_download", { fileId });
    return new Blob([new Uint8Array(bytes)]);
  }
  if (isWeb() && webToken()) {
    const token = webToken();
    const headers: Record<string, string> = {};
    if (token) headers["Authorization"] = `Bearer ${token}`;
    const resp = await fetch(`${WEB_API_BASE}/files/${fileId}/content`, { headers });
    if (!resp.ok) throw new Error("Failed to download file content");
    return resp.blob();
  }
  throw new Error("Cloud connection required to download file.");
}

export async function sendMessageStream(
  request: SendMessageRequest,
  tempAssistantMessageId: string,
): Promise<SendMessageResponse> {
  if (isTauri()) return invoke("message_send_stream", { request, tempAssistantMessageId });

  if (isWeb() && webToken()) {
    const response = await fetch(`${WEB_API_BASE}/assistant/stream`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Authorization": `Bearer ${webToken()}`,
      },
      body: JSON.stringify({
        conversationId: request.conversationId || null,
        content: request.content,
        mode: request.mode,
        systemPrompt: request.systemPrompt || null,
        modelId: request.modelId || null,
        provider: request.provider || null,
        attachments: request.attachments || [],
        webAccess: request.webAccess || "off",
        searchSettings: request.searchSettings || null,
        // Client epais : prompt systeme deja compile par le harnais
        // desktop (personnalite + souvenirs + skills + permissions).
        // Le serveur l'utilise tel quel, sans enrichissement.
        promptScope: "full",
      }),
    });

    if (!response.ok) {
      let msg = `${response.status} ${response.statusText}`;
      try {
        const j = await response.json();
        msg = j.error ?? msg;
      } catch { /* ignore */ }
      // 409 = génération déjà en cours : erreur typée (jamais de fausse
      // réponse vide qui corromprait l'état local comme avant).
      const err = new Error(msg) as Error & { status?: number; alreadyRunning?: boolean };
      err.status = response.status;
      if (response.status === 409) {
        err.alreadyRunning = true;
      }
      throw err;
    }

    const reader = response.body?.getReader();
    if (!reader) throw new Error("No response body");
    const decoder = new TextDecoder();
    // The parser owns buffering + event-type state across reads: an
    // `event:` line and its `data:` line may land in different TCP chunks.
    const parser = createSseParser((e, raw) =>
      console.error("Failed to parse SSE data:", e, raw)
    );
    let doneResult: SendMessageResponse | null = null;

    const handleSseEvent = (kind: string, data: any) => {
      if (kind === "chunk") {
        if (typeof window !== "undefined") {
          window.dispatchEvent(
            new CustomEvent("aro-chat-stream-chunk", {
              detail: {
                conversationId: request.conversationId || data.conversationId || "",
                messageId: tempAssistantMessageId,
                content: data.content || "",
                done: false,
              },
            })
          );
        }
      } else if (kind === "step") {
        if (typeof window !== "undefined") {
          window.dispatchEvent(
            new CustomEvent("aro-agent-step-update", {
              detail: {
                conversationId: request.conversationId || data.conversationId || "",
                step: data.step,
              },
            })
          );
        }
      } else if (kind === "done") {
        doneResult = data as SendMessageResponse;
      }
    };

    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      for (const event of parser.feed(decoder.decode(value, { stream: true }))) {
        handleSseEvent(event.kind, event.data);
      }
    }
    for (const event of parser.flush()) {
      handleSseEvent(event.kind, event.data);
    }
    if (doneResult) return doneResult;
    throw new Error("Stream closed without done event");
  }

  return sendMessage(request);
}

export async function sendMessage(
  request: SendMessageRequest,
): Promise<SendMessageResponse> {
  if (isTauri()) return invoke("message_send", { request });

  if (isWeb() && webToken()) {
    const tempId = crypto.randomUUID();
    return sendMessageStream(request, tempId);
  }

  const conversation =
    demoConversations.find((item) => item.id === request.conversationId) ??
    makeConversation(request.content.split(/\s+/).slice(0, 6).join(" "), request.mode);
  if (!demoConversations.some((item) => item.id === conversation.id)) {
    demoConversations = [conversation, ...demoConversations];
  }

  const userMessage = {
    ...makeMessage(conversation.id, "user", request.content),
    attachments: request.attachments ?? [],
  };
  const assistantMessage = makeMessage(
    conversation.id,
    "assistant",
    demoAssistantResponse(request.content, request.mode),
  );
  demoMessages[conversation.id] = [
    ...(demoMessages[conversation.id] ?? []),
    userMessage,
    assistantMessage,
  ];
  return { conversation, userMessage, assistantMessage };
}

function demoAssistantResponse(content: string, mode: AssistantMode): string {
  const normalized = content.toLowerCase();
  const french = ["bonjour", "salut", "merci", "analyse", "projet", "aide", "fais"].some((word) =>
    normalized.includes(word),
  );
  const excerpt = content.split(/\s+/).join(" ").slice(0, 180);
  if (french) {
    return `ARO est prêt en mode démo local. J'utilise les consignes ${mode} et j'ai bien reçu : "${excerpt}". Lance Tauri avec Ollama ou llama.cpp pour une vraie réponse modèle.`;
  }
  return `ARO is ready in local demo mode. I am using the ${mode} instructions and received: "${excerpt}". Run Tauri with Ollama or llama.cpp for a real model response.`;
}

export async function startAgentRun(request: AgentRunStartRequest): Promise<AgentRunView> {
  if (isTauri()) return invoke("agent_run_start", { request });
  if (isWeb() && webToken()) {
    return webFetch<AgentRunView>("POST", "/agent/runs", request);
  }
  const now = new Date().toISOString();
  const laneId = request.laneId ?? request.conversationId ?? "demo-lane";
  const existingLane = demoAgentLanes.find((item) => item.lane.id === laneId);
  const laneView: AgentLaneView = existingLane ?? {
    lane: {
      id: laneId,
      conversationId: request.conversationId ?? null,
      title: request.goal.split(/\s+/).slice(0, 6).join(" ") || "Agent lane",
      status: "active",
      priority: request.priority ?? "normal",
      maxConcurrentRuns: 1,
      createdAt: now,
      updatedAt: now,
    },
    queuedCount: 0,
    runningCount: 0,
    waitingCount: 0,
    latestRuns: [],
  };
  const run: AgentRun = {
    id: request.runId ?? crypto.randomUUID(),
    laneId,
    conversationId: request.conversationId ?? null,
    goal: request.goal,
    mode: request.mode,
    status: "running",
    priority: request.priority ?? laneView.lane.priority,
    modelProviderId: request.provider ?? null,
    modelId: request.modelId ?? null,
    autonomyProfileId: null,
    checkpointSummary: "Demo run created in web fallback.",
    lastError: null,
    createdAt: now,
    updatedAt: now,
    heartbeatAt: now,
    completedAt: null,
  };
  const view: AgentRunView = {
    run,
    steps: [
      {
        id: crypto.randomUUID(),
        runId: run.id,
        sequence: 1,
        kind: "run-started",
        status: "completed",
        title: "Run started",
        input: { goal: run.goal },
        output: { status: run.status },
        error: null,
        startedAt: now,
        finishedAt: now,
      },
    ],
    artifacts: [],
    contextPack: null,
  };
  demoAgentRuns = [run, ...demoAgentRuns.filter((item) => item.id !== run.id)];
  demoAgentRunViews[run.id] = view;
  const nextLaneView: AgentLaneView = {
    ...laneView,
    runningCount: demoAgentRuns.filter((item) => item.laneId === laneId && item.status === "running").length,
    queuedCount: demoAgentRuns.filter((item) => item.laneId === laneId && item.status === "queued").length,
    waitingCount: demoAgentRuns.filter((item) => item.laneId === laneId && (item.status === "waiting" || item.status === "paused")).length,
    latestRuns: demoAgentRuns.filter((item) => item.laneId === laneId).slice(0, 4),
  };
  demoAgentLanes = [nextLaneView, ...demoAgentLanes.filter((item) => item.lane.id !== laneId)];
  return view;
}

export async function listAgentRuns(): Promise<AgentRun[]> {
  if (isTauri()) return invoke("agent_run_list");
  if (isWeb() && webToken()) {
    try { return await webFetch<AgentRun[]>("GET", "/agent/runs"); } catch { /* fallthrough */ }
  }
  return demoAgentRuns;
}

export async function getAgentRun(runId: string): Promise<AgentRunView> {
  if (isTauri()) return invoke("agent_run_get", { runId });
  if (isWeb() && webToken()) {
    return webFetch<AgentRunView>("GET", `/agent/runs/${runId}`);
  }
  const view = demoAgentRunViews[runId];
  if (!view) throw new Error("Agent run not found");
  return view;
}

export async function pauseAgentRun(runId: string): Promise<AgentRun> {
  if (isTauri()) return invoke("agent_run_pause", { runId });
  if (isWeb() && webToken()) {
    return webFetch<AgentRun>("POST", `/agent/runs/${runId}/pause`, {});
  }
  return setDemoAgentRunStatus(runId, "paused");
}

export async function resumeAgentRun(runId: string): Promise<AgentRun> {
  if (isTauri()) return invoke("agent_run_resume", { runId });
  if (isWeb() && webToken()) {
    return webFetch<AgentRun>("POST", `/agent/runs/${runId}/resume`, {});
  }
  return setDemoAgentRunStatus(runId, "running");
}

export async function cancelAgentRun(runId: string): Promise<AgentRun> {
  if (isTauri()) return invoke("agent_run_cancel", { runId });
  if (isWeb() && webToken()) {
    return webFetch<AgentRun>("POST", `/agent/runs/${runId}/cancel`, {});
  }
  return setDemoAgentRunStatus(runId, "cancelled");
}

export async function getAgentOrchestratorSnapshot(): Promise<AgentOrchestratorSnapshot> {
  if (isTauri()) return invoke("agent_orchestrator_snapshot");
  if (isWeb() && webToken()) {
    try {
      const [runs, lanes] = await Promise.all([
        webFetch<AgentRun[]>("GET", "/agent/runs"),
        webFetch<AgentLaneView[]>("GET", "/agent/lanes"),
      ]);
      const runningCount = runs.filter((r) => r.status === "running").length;
      const queuedCount = runs.filter((r) => r.status === "queued").length;
      return { maxGlobalRunning: 3, runningCount, queuedCount, lanes };
    } catch { /* fallthrough */ }
  }
  const runningCount = demoAgentRuns.filter((run) => run.status === "running").length;
  const queuedCount = demoAgentRuns.filter((run) => run.status === "queued").length;
  return {
    maxGlobalRunning: 3,
    runningCount,
    queuedCount,
    lanes: demoAgentLanes,
  };
}

export async function listAgentLanes(): Promise<AgentLaneView[]> {
  if (isTauri()) return invoke("agent_lane_list");
  if (isWeb() && webToken()) {
    try { return await webFetch<AgentLaneView[]>("GET", "/agent/lanes"); } catch { /* fallthrough */ }
  }
  return demoAgentLanes;
}

export async function pauseAgentLane(laneId: string): Promise<AgentLaneView> {
  if (isTauri()) return invoke("agent_lane_pause", { laneId });
  if (isWeb() && webToken()) {
    return webFetch<AgentLaneView>("POST", `/agent/lanes/${laneId}/pause`, {});
  }
  return setDemoAgentLane(laneId, { status: "paused" });
}

export async function resumeAgentLane(laneId: string): Promise<AgentLaneView> {
  if (isTauri()) return invoke("agent_lane_resume", { laneId });
  if (isWeb() && webToken()) {
    return webFetch<AgentLaneView>("POST", `/agent/lanes/${laneId}/resume`, {});
  }
  return setDemoAgentLane(laneId, { status: "active" });
}

export async function setAgentLanePriority(
  laneId: string,
  priority: AgentRunPriority,
): Promise<AgentLaneView> {
  if (isTauri()) return invoke("agent_lane_set_priority", { laneId, priority });
  if (isWeb() && webToken()) {
    return webFetch<AgentLaneView>("POST", `/agent/lanes/${laneId}/priority`, { priority });
  }
  return setDemoAgentLane(laneId, { priority });
}

export async function searchAgentContext(
  query: string,
  limit = 20,
): Promise<AgentContextItem[]> {
  if (isTauri()) return invoke("agent_context_search", { query, limit });
  if (isWeb() && webToken()) {
    try {
      return await webFetch<AgentContextItem[]>("GET", `/agent/context/search?q=${encodeURIComponent(query)}&limit=${limit}`);
    } catch { /* fallthrough */ }
  }
  return [];
}

export async function listPermissionProfiles(): Promise<PermissionProfile[]> {
  if (isTauri()) return invoke("permission_profiles_list");
  if (isWeb() && webToken()) {
    // An authenticated 401/403/5xx is security-relevant and must not be disguised as demo data.
    return webFetch<PermissionProfile[]>("GET", "/agent/permission-profiles");
  }
  return demoPermissionProfiles;
}

export async function upsertPermissionProfile(
  profile: PermissionProfile,
): Promise<PermissionProfile> {
  if (isTauri()) return invoke("permission_profile_upsert", { profile });
  if (isWeb() && webToken()) {
    return webFetch<PermissionProfile>("PUT", "/agent/permission-profiles", profile);
  }
  const now = new Date().toISOString();
  const saved = { ...profile, updatedAt: now, createdAt: profile.createdAt || now };
  demoPermissionProfiles = [
    saved,
    ...demoPermissionProfiles.filter((item) => item.id !== saved.id),
  ];
  return saved;
}

function setDemoAgentRunStatus(runId: string, status: AgentRun["status"]): AgentRun {
  const run = demoAgentRuns.find((item) => item.id === runId);
  if (!run) throw new Error("Agent run not found");
  run.status = status;
  run.updatedAt = new Date().toISOString();
  if (status === "cancelled" || status === "completed" || status === "failed") {
    run.completedAt = run.updatedAt;
  }
  demoAgentRunViews[runId] = { ...demoAgentRunViews[runId], run };
  return run;
}

function setDemoAgentLane(
  laneId: string,
  patch: Partial<AgentLaneView["lane"]>,
): AgentLaneView {
  const view = demoAgentLanes.find((item) => item.lane.id === laneId);
  if (!view) throw new Error("Agent lane not found");
  const updated = {
    ...view,
    lane: {
      ...view.lane,
      ...patch,
      updatedAt: new Date().toISOString(),
    },
  };
  demoAgentLanes = demoAgentLanes.map((item) => item.lane.id === laneId ? updated : item);
  return updated;
}

export async function sendArenaStream(request: ArenaRequest): Promise<void> {
  if (isTauri()) return invoke("message_arena_stream", { request });
  // Pas de simulation silencieuse : l'appelant affiche l'erreur (slots en
  // échec) au lieu d'un tour vide qui ressemble à un bug.
  throw new Error("Arena comparison requires the desktop app (local model runtimes).");
}

export async function getSettings(): Promise<AppSettings> {
  if (isTauri()) return invoke("settings_get");
  if (isWeb() && webToken()) {
    try {
      const s = await webFetch<AppSettings>("GET", "/settings");
      demoSettings = normalizeSettings(s);
      return demoSettings;
    } catch { /* fallthrough */ }
  }
  return demoSettings;
}

export async function updateSettings(settings: AppSettings): Promise<AppSettings> {
  if (isTauri()) return normalizeSettings(await invoke("settings_update", { settings }));
  if (isWeb() && webToken()) {
    const s = await webFetch<AppSettings>("PUT", "/settings", settings);
    demoSettings = normalizeSettings(s);
    return demoSettings;
  }
  demoSettings = normalizeSettings(settings);
  return demoSettings;
}

export async function setSearchProviderApiKey(
  providerId: string,
  apiKey: string,
): Promise<AppSettings> {
  if (!isTauri()) throw new Error("Search credentials require the secure desktop keyring.");
  return normalizeSettings(
    await invoke("search_provider_set_api_key", { providerId, apiKey }),
  );
}

export async function clearSearchProviderApiKey(providerId: string): Promise<AppSettings> {
  if (!isTauri()) throw new Error("Search credentials require the secure desktop keyring.");
  return normalizeSettings(await invoke("search_provider_clear_api_key", { providerId }));
}

export async function listModelProviders(): Promise<ModelProviderConnection[]> {
  if (isTauri()) return invoke("model_provider_list");
  if (isWeb() && webToken()) {
    try {
      const s = await webFetch<AppSettings>("GET", "/settings");
      return normalizeSettings(s).model.providers;
    } catch { /* fallthrough */ }
  }
  return demoSettings.model.providers;
}

export async function upsertModelProvider(
  provider: ModelProviderConnection,
): Promise<AppSettings> {
  if (isTauri()) return normalizeSettings(await invoke("model_provider_upsert", { provider }));
  if (isWeb() && webToken()) {
    const current = await getSettings();
    const providers = current.model.providers.some((p) => p.id === provider.id)
      ? current.model.providers.map((p) => p.id === provider.id ? provider : p)
      : [...current.model.providers, provider];
    return updateSettings({ ...current, model: { ...current.model, providers } });
  }
  const next = normalizeSettings(demoSettings);
  const existingIndex = next.model.providers.findIndex((item) => item.id === provider.id);
  if (existingIndex >= 0) next.model.providers[existingIndex] = provider;
  else next.model.providers = [...next.model.providers, provider];
  demoSettings = next;
  demoModels = normalizeModels(next.model.providers.flatMap((item) => item.models));
  return demoSettings;
}

export async function deleteModelProvider(providerId: string): Promise<AppSettings> {
  if (isTauri()) return normalizeSettings(await invoke("model_provider_delete", { providerId }));
  if (isWeb() && webToken()) {
    const current = await getSettings();
    const providers = current.model.providers.filter((p) => p.id !== providerId);
    return updateSettings({ ...current, model: { ...current.model, providers } });
  }
  demoSettings.model.providers = demoSettings.model.providers.filter((item) => item.id !== providerId);
  demoModels = normalizeModels(demoSettings.model.providers.flatMap((item) => item.models));
  return normalizeSettings(demoSettings);
}

export async function setModelProviderApiKey(
  providerId: string,
  apiKey: string,
): Promise<AppSettings> {
  if (isTauri()) {
    return normalizeSettings(
      await invoke("model_provider_set_api_key", { request: { providerId, apiKey } }),
    );
  }
  if (isWeb() && webToken()) {
    const current = await getSettings();
    const providers = current.model.providers.map((p) =>
      p.id === providerId ? { ...p, authConfigured: Boolean(apiKey.trim()) } : p,
    );
    return updateSettings({ ...current, model: { ...current.model, providers } });
  }
  demoSettings.model.providers = demoSettings.model.providers.map((provider) =>
    provider.id === providerId ? { ...provider, authConfigured: Boolean(apiKey.trim()) } : provider,
  );
  return normalizeSettings(demoSettings);
}

export async function clearModelProviderApiKey(providerId: string): Promise<AppSettings> {
  if (isTauri()) {
    return normalizeSettings(await invoke("model_provider_clear_api_key", { providerId }));
  }
  if (isWeb() && webToken()) {
    const current = await getSettings();
    const providers = current.model.providers.map((p) =>
      p.id === providerId ? { ...p, authConfigured: false } : p,
    );
    return updateSettings({ ...current, model: { ...current.model, providers } });
  }
  demoSettings.model.providers = demoSettings.model.providers.map((provider) =>
    provider.id === providerId ? { ...provider, authConfigured: false } : provider,
  );
  return normalizeSettings(demoSettings);
}

export async function testModelProvider(providerId: string): Promise<RuntimeStatus> {
  if (isTauri()) {
    const [runtime, voiceStatus] = await Promise.all([
      invoke<RuntimeStatus>("model_provider_test", { providerId }),
      checkVoiceStatus(),
    ]);
    return normalizeRuntime(runtime, voiceStatus);
  }
  const provider = demoSettings.model.providers.find((item) => item.id === providerId);
  const voiceStatus = makeDemoVoiceStatus();
  return {
    modelProvider: provider?.kind ?? "mock",
    modelId: provider?.models[0]?.modelId ?? "mock",
    modelReady: Boolean(provider && (isLocalProviderKind(provider.kind) || provider.authConfigured)),
    voiceReady: voiceStatus.ready,
    voiceStatus,
    endpoint: provider?.endpoint ?? null,
    detail: provider
      ? `${provider.displayName} ${provider.authConfigured || isLocalProviderKind(provider.kind) ? "is ready" : "needs an API key"}.`
      : "Provider not found.",
    checkedAt: new Date().toISOString(),
  };
}

export async function refreshModelCatalog(providerId: string): Promise<AppSettings> {
  if (isTauri()) return normalizeSettings(await invoke("model_catalog_refresh", { providerId }));
  // In web mode, re-fetch the latest settings as a proxy for catalog refresh
  if (isWeb() && webToken()) {
    try {
      const s = await webFetch<AppSettings>("GET", "/settings");
      demoSettings = normalizeSettings(s);
      demoModels = normalizeModels(demoSettings.model.providers.flatMap((p) => p.models));
      return demoSettings;
    } catch { /* fallthrough */ }
  }
  const next = normalizeSettings(demoSettings);
  demoSettings = next;
  demoModels = normalizeModels(next.model.providers.flatMap((provider) => provider.models));
  return next;
}

export async function selectModelRef(providerId: string, modelId: string): Promise<AppSettings> {
  if (isTauri()) {
    return normalizeSettings(await invoke("model_select", { request: { providerId, modelId } }));
  }
  if (isWeb() && webToken()) {
    const current = await getSettings();
    const provider = current.model.providers.find((p) => p.id === providerId);
    const model = provider?.models.find((m) => m.modelId === modelId);
    if (provider && model) {
      const updated = normalizeSettings({
        ...current,
        model: { ...current.model, activeModelRef: model, provider: provider.kind, modelId },
      });
      return updateSettings(updated);
    }
    return current;
  }
  const provider = demoSettings.model.providers.find((item) => item.id === providerId);
  const model = provider?.models.find((item) => item.modelId === modelId);
  if (provider && model) {
    demoSettings = normalizeSettings({
      ...demoSettings,
      model: {
        ...demoSettings.model,
        activeModelRef: model,
        provider: provider.kind,
        modelId,
      },
    });
  }
  return demoSettings;
}

export async function updatePreferences(
  preferences: UserPreferencesPatch,
): Promise<UserPreferences> {
  if (isTauri()) return invoke("preferences_update", { preferences });
  if (isWeb() && webToken()) {
    return webFetch<UserPreferences>("PUT", "/preferences", preferences);
  }
  const now = new Date().toISOString();
  demoPreferences = {
    userId: demoCloudSession?.user.id ?? "demo",
    theme: preferences.theme ?? demoPreferences?.theme ?? "light",
    language: preferences.language ?? demoPreferences?.language ?? "fr",
    wakeWordEnabled: preferences.wakeWordEnabled ?? demoPreferences?.wakeWordEnabled ?? false,
    inferenceMode: preferences.inferenceMode ?? demoPreferences?.inferenceMode ?? "local",
    updatedAt: now,
  };
  return demoPreferences;
}


export async function checkRuntime(): Promise<RuntimeStatus> {
  if (isTauri()) {
    const [runtime, voiceStatus] = await Promise.all([
      invoke<RuntimeStatus>("runtime_check"),
      checkVoiceStatus(),
    ]);
    return normalizeRuntime(runtime, voiceStatus);
  }
  return demoRuntime();
}

export async function checkVoiceStatus(): Promise<VoiceStatus> {
  if (isTauri()) return invoke("voice_status");
  return makeDemoVoiceStatus();
}

export async function checkVoiceModelStatus(): Promise<VoiceModelsStatus> {
  if (isTauri()) return invoke<VoiceModelsStatus>("voice_model_status");
  return makeDemoVoiceModelStatus();
}

export async function listModels(): Promise<ModelOption[]> {
  if (isTauri()) return normalizeModels(await invoke("model_list"));
  return normalizeModels(demoModels);
}

export async function resetMemory(): Promise<void> {
  if (isTauri()) return invoke("memory_reset");
  demoConversations = [];
  demoMessages = {};
}

export async function updateMessage(messageId: string, content: string): Promise<void> {
  if (isTauri()) return invoke("message_update", { messageId, content });
  if (isWeb() && webToken()) {
    await webFetch<void>("PATCH", `/messages/${messageId}`, { content });
    return;
  }
  // Local demo fallback
  for (const conversationId in demoMessages) {
    const list = demoMessages[conversationId];
    const idx = list.findIndex((m) => m.id === messageId);
    if (idx !== -1) {
      list[idx].content = content;
      demoMessages[conversationId] = list.slice(0, idx + 1);
      break;
    }
  }
}

export async function regenerateMessage(conversationId: string, systemPrompt?: string | null): Promise<ChatMessage> {
  if (isTauri()) return invoke("message_regenerate", { conversationId, systemPrompt });
  if (isWeb() && webToken()) {
    return webFetch<ChatMessage>("POST", `/conversations/${conversationId}/regenerate`, { systemPrompt: systemPrompt ?? null });
  }
  // Local demo fallback
  const list = demoMessages[conversationId] || [];
  const lastUserMsg = list[list.length - 1];
  if (!lastUserMsg || lastUserMsg.role !== "user") {
    throw new Error("No user message to regenerate from");
  }
  const assistantMessage = makeMessage(
    conversationId,
    "assistant",
    `[Régénération] ARO est en mode local de développement. J'ai bien reçu votre message modifié : "${lastUserMsg.content}".`
  );
  list.push(assistantMessage);
  return assistantMessage;
}

export async function regenerateMessageStream(
  conversationId: string,
  systemPrompt: string | null | undefined,
  tempAssistantMessageId: string,
): Promise<ChatMessage> {
  if (isTauri()) {
    return invoke("message_regenerate_stream", {
      conversationId,
      systemPrompt,
      tempAssistantMessageId,
    });
  }
  return regenerateMessage(conversationId, systemPrompt);
}


export async function transcribeAudio(
  audioBytes: number[],
  mimeType: string,
): Promise<TranscriptionResult> {
  if (isTauri()) {
    return invoke("voice_transcribe", {
      request: { audioBytes, mimeType, language: "fr" },
    });
  }

  return {
    text: "",
    runtimeDetail: "voice runtime disabled in web fallback",
  };
}

export async function detectWakeWord(
  audioBytes: number[],
  mimeType: string,
  options: WakeWordDetectionOptions = {},
): Promise<WakeWordDetectionResult> {
  if (isTauri()) {
    return invoke<WakeWordDetectionResult>("voice_wake_word_detect", {
      request: {
        audioBytes,
        mimeType,
        language: options.language ?? "fr",
        variants: options.variants ?? null,
        allowEmbeddedAro: options.allowEmbeddedAro ?? false,
      },
    });
  }

  return {
    detected: false,
    text: "",
    matchedVariant: null,
    runtimeDetail: "voice runtime disabled in web fallback",
    score: null,
    threshold: null,
    checkedAt: new Date().toISOString(),
  };
}

export async function synthesizeSpeech(
  text: string,
  voicePath?: string | null,
  speakerId?: number | null
): Promise<SynthesisResult> {
  if (isTauri()) return invoke("voice_synthesize", { request: { text, voicePath, speakerId } });
  return { audioBytes: [], mimeType: "audio/wav", runtimeDetail: "disabled" };
}

function demoRuntime(): RuntimeStatus {
  const active = demoSettings.model.activeModelRef;
  const provider = demoSettings.model.providers.find((item) => item.id === active.providerId);
  const voiceStatus = makeDemoVoiceStatus();
  return {
    modelProvider: active.providerKind,
    modelId: active.modelId,
    modelReady: Boolean(active.local || provider?.authConfigured),
    voiceReady: voiceStatus.ready,
    voiceStatus,
    endpoint: provider?.endpoint ?? null,
    detail: "Mode web local. Lance Tauri pour utiliser les commandes Rust.",
    checkedAt: new Date().toISOString(),
  };
}

function makeConversation(
  title: string,
  mode: AssistantMode,
  projectId?: string | null,
  folderId?: string | null,
  organizationId?: string | null,
): Conversation {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    title: title || "New conversation",
    createdAt: now,
    updatedAt: now,
    mode,
    projectId: projectId ?? null,
    folderId: folderId ?? null,
    organizationId: organizationId ?? null,
  };
}

async function sha256Hex(bytes: Uint8Array): Promise<string> {
  if (!globalThis.crypto?.subtle) return "";
  const digestInput = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(digestInput).set(bytes);
  const digest = await crypto.subtle.digest("SHA-256", digestInput);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function makeMessage(
  conversationId: string,
  role: "user" | "assistant",
  content: string,
): ChatMessage {
  return {
    id: crypto.randomUUID(),
    conversationId,
    role,
    content,
    createdAt: new Date().toISOString(),
    tokenEstimate: null,
  };
}

function makeDemoSession(email: string, name: string, organizationName: string): CloudSessionView {
  const now = new Date().toISOString();
  const userId = crypto.randomUUID();
  const organizationId = crypto.randomUUID();
  const membership = {
    id: crypto.randomUUID(),
    userId,
    organizationId,
    role: "owner" as const,
    status: "active" as const,
    createdAt: now,
    updatedAt: now,
  };
  const session = {
    user: {
      id: userId,
      email,
      name,
      roleTitle: null,
      avatarColor: null,
      createdAt: now,
      updatedAt: now,
    },
    activeOrganization: {
      id: organizationId,
      name: organizationName,
      domain: null,
      description: null,
      createdAt: now,
      updatedAt: now,
    },
    memberships: [membership],
    expiresAt: new Date(Date.now() + 15 * 60 * 1000).toISOString(),
  };
  demoMembers = [
    {
      id: membership.id,
      userId,
      organizationId,
      name,
      email,
      role: membership.role,
      status: membership.status,
      createdAt: now,
      updatedAt: now,
    },
  ];
  demoOrganizations = [session.activeOrganization];
  demoPreferences = {
    userId,
    theme: "light",
    language: "fr",
    wakeWordEnabled: false,
    inferenceMode: "local",
    updatedAt: now,
  };
  return session;
}
