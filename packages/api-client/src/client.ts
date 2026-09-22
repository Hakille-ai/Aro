import type {
  AuthResponse,
  CloudLoginRequest,
  CloudRegisterRequest,
  CloudSessionView,
  Conversation,
  ConversationCreateRequest,
  ConversationPatch,
  ChatMessage,
  SendMessageRequest,
  SendMessageResponse,
  StreamEvent,
  Plan,
  PlanCreateRequest,
  PlanUpdateRequest,
  AgentRun,
  AgentLaneView,
  AgentLiveLog,
  WorkspaceArtifact,
  FileObject,
  FileQuotaView,
  VoiceStatus,
  SynthesisResult,
  TranscriptionResult,
  ModelOption,
  AppSettings,
  Organization,
  OrganizationMember,
  TotpSetupResponse,
  MoveConversationPayload,
} from "@aro/contracts";
import { TokenStorage, MemoryTokenStorage } from "./storage";
import { ApiError } from "./errors";

export interface AroApiClientOptions {
  baseUrl?: string;
  tokenStorage?: TokenStorage;
  onAuthExpired?: () => void;
  enableMockFallback?: boolean;
}

export interface StreamCallbacks {
  onChunk?: (text: string) => void;
  onThought?: (thought: string) => void;
  onToolCall?: (toolCall: any) => void;
  onError?: (error: Error) => void;
  onDone?: () => void;
}

export class AroApiClient {
  private baseUrl: string;
  private tokenStorage: TokenStorage;
  private onAuthExpired?: () => void;
  private enableMockFallback: boolean;
  private refreshPromise: Promise<string | null> | null = null;

  constructor(options: AroApiClientOptions = {}) {
    this.baseUrl = (options.baseUrl || "http://127.0.0.1:8710").replace(/\/+$/, "");
    this.tokenStorage = options.tokenStorage || new MemoryTokenStorage();
    this.onAuthExpired = options.onAuthExpired;
    this.enableMockFallback = options.enableMockFallback ?? false;
  }

  public getBaseUrl(): string {
    return this.baseUrl;
  }

  public setBaseUrl(url: string) {
    this.baseUrl = url.replace(/\/+$/, "");
  }

  public getTokenStorage(): TokenStorage {
    return this.tokenStorage;
  }

  private getApiUrl(endpoint: string): string {
    const cleanEndpoint = endpoint.startsWith("/") ? endpoint : `/${endpoint}`;
    // Health / well-known endpoints are root-level, standard API routes are under /v1 or root
    if (cleanEndpoint.startsWith("/v1/") || cleanEndpoint === "/health" || cleanEndpoint.startsWith("/.well-known/")) {
      return `${this.baseUrl}${cleanEndpoint}`;
    }
    return `${this.baseUrl}/v1${cleanEndpoint}`;
  }

  private async getAuthHeaders(): Promise<Record<string, string>> {
    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      Accept: "application/json",
    };
    const token = await this.tokenStorage.getAccessToken();
    if (token) {
      headers["Authorization"] = `Bearer ${token}`;
    }
    return headers;
  }

  public async request<T>(
    endpoint: string,
    options: RequestInit = {},
    isRetry = false
  ): Promise<T> {
    const url = this.getApiUrl(endpoint);
    const headers = {
      ...(await this.getAuthHeaders()),
      ...((options.headers as Record<string, string>) || {}),
    };

    try {
      const response = await fetch(url, {
        ...options,
        headers,
      });

      if (response.status === 401 && !isRetry) {
        const refreshed = await this.tryRefreshToken();
        if (refreshed) {
          return this.request<T>(endpoint, options, true);
        } else {
          this.onAuthExpired?.();
        }
      }

      if (!response.ok) {
        let errData: unknown;
        try {
          errData = await response.json();
        } catch {
          errData = await response.text();
        }
        throw ApiError.fromResponse(response.status, errData);
      }

      if (response.status === 204) {
        return undefined as unknown as T;
      }

      return (await response.json()) as T;
    } catch (err) {
      if (err instanceof ApiError) {
        throw err;
      }

      if (this.enableMockFallback && !/^\/?(?:v1\/)?billing(?:\/|$)/.test(endpoint)) {
        return this.generateMockFallback<T>(endpoint, options);
      }

      throw new ApiError(
        err instanceof Error ? err.message : "Network request failed",
        0,
        "NETWORK_ERROR",
        err
      );
    }
  }

  private async tryRefreshToken(): Promise<boolean> {
    if (this.refreshPromise) {
      const token = await this.refreshPromise;
      return Boolean(token);
    }

    this.refreshPromise = (async () => {
      try {
        const refreshToken = await this.tokenStorage.getRefreshToken();
        if (!refreshToken) return null;

        const refreshUrl = this.getApiUrl("/auth/refresh");
        const res = await fetch(refreshUrl, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ refreshToken }),
        });

        if (!res.ok) {
          await this.tokenStorage.clear();
          return null;
        }

        const data = (await res.json()) as AuthResponse;
        if (data?.tokens?.accessToken) {
          await this.tokenStorage.setAccessToken(data.tokens.accessToken);
          if (data.tokens.refreshToken) {
            await this.tokenStorage.setRefreshToken(data.tokens.refreshToken);
          }
          return data.tokens.accessToken;
        }
        return null;
      } catch {
        await this.tokenStorage.clear();
        return null;
      } finally {
        this.refreshPromise = null;
      }
    })();

    const result = await this.refreshPromise;
    return Boolean(result);
  }

  // -------------------------------------------------------------------------
  // Auth API
  // -------------------------------------------------------------------------
  public async login(req: CloudLoginRequest): Promise<AuthResponse> {
    const res = await this.request<AuthResponse>("/auth/login", {
      method: "POST",
      body: JSON.stringify(req),
    });
    if (res?.tokens) {
      await this.tokenStorage.setAccessToken(res.tokens.accessToken);
      await this.tokenStorage.setRefreshToken(res.tokens.refreshToken);
    }
    return res;
  }

  public async register(req: CloudRegisterRequest): Promise<AuthResponse> {
    const res = await this.request<AuthResponse>("/auth/register", {
      method: "POST",
      body: JSON.stringify(req),
    });
    if (res?.tokens) {
      await this.tokenStorage.setAccessToken(res.tokens.accessToken);
      await this.tokenStorage.setRefreshToken(res.tokens.refreshToken);
    }
    return res;
  }

  public async logout(): Promise<void> {
    try {
      await this.request<void>("/auth/logout", { method: "POST" });
    } finally {
      await this.tokenStorage.clear();
    }
  }

  public async getBootstrap(): Promise<{ session: CloudSessionView }> {
    return this.request<{ session: CloudSessionView }>("/bootstrap");
  }

  public async switchOrganization(organizationId: string): Promise<AuthResponse> {
    const res = await this.request<AuthResponse>("/auth/switch-organization", {
      method: "POST",
      body: JSON.stringify({ organizationId }),
    });
    if (res?.tokens) {
      await this.tokenStorage.setAccessToken(res.tokens.accessToken);
      await this.tokenStorage.setRefreshToken(res.tokens.refreshToken);
    }
    return res;
  }

  public async setupTotp(): Promise<TotpSetupResponse> {
    return this.request<TotpSetupResponse>("/auth/mfa/totp/setup", { method: "POST" });
  }

  public async enableTotp(code: string): Promise<void> {
    return this.request<void>("/auth/mfa/totp/enable", {
      method: "POST",
      body: JSON.stringify({ code }),
    });
  }

  public async disableTotp(code: string): Promise<void> {
    return this.request<void>("/auth/mfa/totp/disable", {
      method: "POST",
      body: JSON.stringify({ code }),
    });
  }

  // -------------------------------------------------------------------------
  // Organizations API
  // -------------------------------------------------------------------------
  public async listOrganizations(): Promise<Organization[]> {
    return this.request<Organization[]>("/organizations");
  }

  public async createOrganization(name: string): Promise<Organization> {
    return this.request<Organization>("/organizations", {
      method: "POST",
      body: JSON.stringify({ name }),
    });
  }

  public async listMembers(): Promise<OrganizationMember[]> {
    return this.request<OrganizationMember[]>("/memberships");
  }

  // -------------------------------------------------------------------------
  // Conversations API
  // -------------------------------------------------------------------------
  public async listConversations(params?: { search?: string }): Promise<Conversation[]> {
    const query = params?.search ? `?search=${encodeURIComponent(params.search)}` : "";
    return this.request<Conversation[]>(`/conversations${query}`);
  }

  public async getConversation(id: string): Promise<Conversation> {
    return this.request<Conversation>(`/conversations/${id}`);
  }

  private formatAssistantPayload(req: SendMessageRequest, stream: boolean) {
    return {
      conversationId: req.conversationId || null,
      content: req.content,
      mode: req.mode || "chat",
      systemPrompt: req.systemPrompt || null,
      modelId: req.modelId || req.model || null,
      provider: req.provider || null,
      attachments: req.attachments || [],
      webAccess: req.webAccess || "off",
      searchSettings: req.searchSettings || null,
      stream,
    };
  }

  public async createConversation(req: ConversationCreateRequest): Promise<Conversation> {
    const payload = {
      title: req.title || "Nouvelle discussion",
      mode: req.mode || "chat",
      projectId: req.projectId || null,
      folderId: req.folderId || null,
    };
    return this.request<Conversation>("/conversations", {
      method: "POST",
      body: JSON.stringify(payload),
    });
  }

  public async updateConversation(id: string, patch: ConversationPatch): Promise<Conversation> {
    return this.request<Conversation>(`/conversations/${id}`, {
      method: "PATCH",
      body: JSON.stringify(patch),
    });
  }

  public async deleteConversation(id: string): Promise<void> {
    return this.request<void>(`/conversations/${id}`, { method: "DELETE" });
  }

  public async moveConversation(id: string, payload: MoveConversationPayload): Promise<void> {
    return this.request<void>(`/conversations/${id}/move`, {
      method: "PATCH",
      body: JSON.stringify(payload),
    });
  }

  public async getMessages(conversationId: string): Promise<ChatMessage[]> {
    return this.request<ChatMessage[]>(`/conversations/${conversationId}/messages`);
  }

  // -------------------------------------------------------------------------
  // Messages & Streaming API
  // -------------------------------------------------------------------------
  public async sendMessage(req: SendMessageRequest): Promise<SendMessageResponse> {
    return this.request<SendMessageResponse>("/assistant/stream", {
      method: "POST",
      body: JSON.stringify(this.formatAssistantPayload(req, false)),
    });
  }

  public streamAssistant(
    req: SendMessageRequest,
    callbacks: StreamCallbacks
  ): { abort: () => void } {
    const controller = new AbortController();

    (async () => {
      try {
        const url = this.getApiUrl("/assistant/stream");
        const headers = await this.getAuthHeaders();

        const response = await fetch(url, {
          method: "POST",
          headers,
          body: JSON.stringify(this.formatAssistantPayload(req, true)),
          signal: controller.signal,
        });

        if (!response.ok) {
          throw new ApiError(`Stream failed: ${response.status}`, response.status);
        }

        if (!response.body) {
          // Fallback if environment doesn't support body stream
          const fullJson = await response.json();
          const text = fullJson?.reply?.content || fullJson?.assistantMessage?.content;
          if (text) {
            callbacks.onChunk?.(text);
          }
          callbacks.onDone?.();
          return;
        }

        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";
        let currentEvent = "";

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split("\n");
          buffer = lines.pop() || "";

          for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed || trimmed.startsWith(":")) continue;

            if (trimmed.startsWith("event:")) {
              currentEvent = trimmed.slice(6).trim();
              continue;
            }

            if (trimmed.startsWith("data:")) {
              const dataStr = trimmed.slice(5).trim();
              if (dataStr === "[DONE]" || currentEvent === "done") {
                callbacks.onDone?.();
                continue;
              }

              try {
                const parsed = JSON.parse(dataStr) as StreamEvent;
                const chunkText = parsed.content ?? parsed.text;
                if (chunkText !== undefined && chunkText !== "") {
                  callbacks.onChunk?.(chunkText);
                }
                if (parsed.thought) callbacks.onThought?.(parsed.thought);
                if (parsed.toolCall) callbacks.onToolCall?.(parsed.toolCall);
                if (parsed.error) callbacks.onError?.(new Error(parsed.error));
              } catch {
                callbacks.onChunk?.(dataStr);
              }
            }
          }
        }

        callbacks.onDone?.();
      } catch (err) {
        if ((err as Error)?.name !== "AbortError") {
          callbacks.onError?.(err instanceof Error ? err : new Error(String(err)));
        }
      }
    })();

    return {
      abort: () => controller.abort(),
    };
  }

  // -------------------------------------------------------------------------
  // Plans & Roadmap API
  // -------------------------------------------------------------------------
  public async listPlans(conversationId?: string): Promise<Plan[]> {
    const query = conversationId ? `?conversationId=${conversationId}` : "";
    return this.request<Plan[]>(`/plans${query}`);
  }

  public async getPlan(id: string): Promise<Plan> {
    return this.request<Plan>(`/plans/${id}`);
  }

  public async createPlan(req: PlanCreateRequest): Promise<Plan> {
    return this.request<Plan>("/plans", {
      method: "POST",
      body: JSON.stringify(req),
    });
  }

  public async updatePlan(id: string, patch: PlanUpdateRequest): Promise<Plan> {
    return this.request<Plan>(`/plans/${id}`, {
      method: "PATCH",
      body: JSON.stringify(patch),
    });
  }

  public async deletePlan(id: string): Promise<void> {
    return this.request<void>(`/plans/${id}`, { method: "DELETE" });
  }

  public async generateRoadmap(conversationId: string, prompt?: string): Promise<Plan> {
    return this.request<Plan>("/plans/generate", {
      method: "POST",
      body: JSON.stringify({ conversationId, prompt }),
    });
  }

  // -------------------------------------------------------------------------
  // Agents & Multi-Agent Orchestrator API
  // -------------------------------------------------------------------------
  public async listAgentRuns(conversationId?: string): Promise<AgentRun[]> {
    const query = conversationId ? `?conversationId=${conversationId}` : "";
    return this.request<AgentRun[]>(`/agent/runs${query}`);
  }

  public async createAgentRun(req: {
    conversationId: string;
    agentName: string;
    goal: string;
  }): Promise<AgentRun> {
    return this.request<AgentRun>("/agent/runs", {
      method: "POST",
      body: JSON.stringify(req),
    });
  }

  public async getAgentRun(id: string): Promise<AgentRun> {
    return this.request<AgentRun>(`/agent/runs/${id}`);
  }

  public async pauseAgentRun(id: string): Promise<void> {
    return this.request<void>(`/agent/runs/${id}/pause`, { method: "POST" });
  }

  public async resumeAgentRun(id: string): Promise<void> {
    return this.request<void>(`/agent/runs/${id}/resume`, { method: "POST" });
  }

  public async cancelAgentRun(id: string): Promise<void> {
    return this.request<void>(`/agent/runs/${id}/cancel`, { method: "POST" });
  }

  public async listAgentLanes(): Promise<AgentLaneView[]> {
    return this.request<AgentLaneView[]>("/agent/lanes");
  }

  public async getLiveLogs(runId?: string): Promise<AgentLiveLog[]> {
    const query = runId ? `?runId=${runId}` : "";
    return this.request<AgentLiveLog[]>(`/agent/logs${query}`);
  }

  // -------------------------------------------------------------------------
  // Files & Artifacts API
  // -------------------------------------------------------------------------
  public async listFiles(): Promise<FileObject[]> {
    return this.request<FileObject[]>("/files");
  }

  public async getFileQuota(): Promise<FileQuotaView> {
    return this.request<FileQuotaView>("/files/quota");
  }

  public async listArtifacts(conversationId?: string): Promise<WorkspaceArtifact[]> {
    const query = conversationId ? `?conversationId=${conversationId}` : "";
    return this.request<WorkspaceArtifact[]>(`/artifacts${query}`);
  }

  public async applyDiffPatch(artifactId: string): Promise<void> {
    return this.request<void>(`/artifacts/${artifactId}/apply`, { method: "POST" });
  }

  public async rejectDiffPatch(artifactId: string): Promise<void> {
    return this.request<void>(`/artifacts/${artifactId}/reject`, { method: "POST" });
  }

  // -------------------------------------------------------------------------
  // Voice API
  // -------------------------------------------------------------------------
  public async getVoiceStatus(): Promise<VoiceStatus> {
    return this.request<VoiceStatus>("/voice/status");
  }

  public async synthesizeSpeech(text: string, voice?: string): Promise<SynthesisResult> {
    return this.request<SynthesisResult>("/voice/synthesize", {
      method: "POST",
      body: JSON.stringify({ text, voice }),
    });
  }

  public async transcribeSpeech(audioBase64: string): Promise<TranscriptionResult> {
    return this.request<TranscriptionResult>("/voice/transcribe", {
      method: "POST",
      body: JSON.stringify({ audioBase64 }),
    });
  }

  // -------------------------------------------------------------------------
  // Models & Settings API
  // -------------------------------------------------------------------------
  public async listModels(): Promise<ModelOption[]> {
    return this.request<ModelOption[]>("/models");
  }

  public async getSettings(): Promise<AppSettings> {
    return this.request<AppSettings>("/settings");
  }

  public async updateSettings(patch: Partial<AppSettings>): Promise<AppSettings> {
    return this.request<AppSettings>("/settings", {
      method: "PATCH",
      body: JSON.stringify(patch),
    });
  }

  public async checkHealth(): Promise<{ status: string; version: string; uptimeSeconds: number }> {
    return this.request<{ status: string; version: string; uptimeSeconds: number }>("/health");
  }

  // -------------------------------------------------------------------------
  // Mock Data Fallback (for preview / offline resilience)
  // -------------------------------------------------------------------------
  private generateMockFallback<T>(endpoint: string, options: RequestInit): T {
    const method = options.method?.toUpperCase() || "GET";

    if (endpoint.includes("/conversations") && method === "GET") {
      return [
        {
          id: "conv-1",
          title: "Refonte Mobile ARO Apple Design",
          messageCount: 14,
          lastMessagePreview: "L'architecture modulaire avec Expo 57 et Zustand est en place.",
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },
        {
          id: "conv-2",
          title: "Intégration Visual Diff & Panneau Droit",
          messageCount: 8,
          lastMessagePreview: "L'application du patch unidiff a réussi sur 3 fichiers.",
          createdAt: new Date(Date.now() - 3600000).toISOString(),
          updatedAt: new Date(Date.now() - 3600000).toISOString(),
        },
      ] as unknown as T;
    }

    if (endpoint.includes("/plans") && method === "GET") {
      return [
        {
          id: "plan-1",
          conversationId: "conv-1",
          title: "Feuille de Route Mobile ARO",
          description: "Déploiement complet de l'application mobile native avec design Apple",
          status: "active",
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          tasks: [
            {
              id: "task-1",
              text: "Architecture Monorepo (@aro/contracts, @aro/ui-tokens, @aro/api-client)",
              completed: true,
              status: "completed",
            },
            {
              id: "task-2",
              text: "Composants d'interface Apple (Header, GlassCard, SpringButton, CodeDiffViewer)",
              completed: true,
              status: "completed",
            },
            {
              id: "task-3",
              text: "Animations physiques fluides Reanimated et gestes tactiles",
              completed: true,
              status: "in_progress",
            },
            {
              id: "task-4",
              text: "Studio Vocal ARO & Reconnaissance de parole",
              completed: false,
              status: "pending",
            },
          ],
        },
      ] as unknown as T;
    }

    if (endpoint.includes("/agent/runs") && method === "GET") {
      return [
        {
          id: "run-1",
          conversationId: "conv-1",
          agentName: "ARO Lead Architect",
          role: "leader",
          goal: "Structurer l'application mobile avec Expo 57 et React 19",
          status: "running",
          priority: "urgent",
          stepCount: 7,
          currentThought: "Validation des tokens d'interface et de la navigation",
          createdAt: new Date().toISOString(),
        },
        {
          id: "run-2",
          conversationId: "conv-1",
          agentName: "Visual Designer",
          role: "ui-specialist",
          goal: "Alignement des palettes light/dark/oled et des flous verre",
          status: "completed",
          priority: "normal",
          stepCount: 4,
          createdAt: new Date(Date.now() - 120000).toISOString(),
        },
      ] as unknown as T;
    }

    if (endpoint.includes("/artifacts") && method === "GET") {
      return [
        {
          id: "art-1",
          conversationId: "conv-1",
          title: "apps/mobile/app/(tabs)/index.tsx",
          filePath: "apps/mobile/app/(tabs)/index.tsx",
          kind: "diff",
          content: "@@ -1,5 +1,6 @@\n export function ChatScreen() {\n+  const theme = useTheme();\n   return <View />;\n }",
          additions: 1,
          deletions: 0,
          status: "pending",
          createdAt: new Date().toISOString(),
        },
      ] as unknown as T;
    }

    if (endpoint.includes("/models") && method === "GET") {
      return [
        {
          id: "claude-3-7-sonnet",
          name: "Claude 3.7 Sonnet (Thinking)",
          provider: "anthropic",
          isDefault: true,
          supportsStreaming: true,
          supportsTools: true,
        },
        {
          id: "gpt-4o",
          name: "GPT-4o Omnimodal",
          provider: "openai",
          supportsStreaming: true,
          supportsTools: true,
        },
        {
          id: "ollama-qwen-2.5-coder",
          name: "Qwen 2.5 Coder 14B (Local)",
          provider: "local",
          isLocal: true,
          supportsStreaming: true,
        },
      ] as unknown as T;
    }

    if (endpoint.includes("/settings") && method === "GET") {
      return {
        defaultModel: "claude-3-7-sonnet",
        serverUrl: "http://127.0.0.1:8710",
        autoSave: true,
        offlineMode: false,
        voiceAutoListen: false,
        theme: "dark",
        accentColor: "#0a84ff",
        haptics: true,
      } as unknown as T;
    }

    return {} as unknown as T;
  }
}
