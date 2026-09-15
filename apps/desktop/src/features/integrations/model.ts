export type McpTransport = "stdio" | "sse";
export type McpConnectionStatus = "connected" | "disconnected" | "connecting" | "error";

export interface McpTool {
  name: string;
  description: string;
  inputSchema?: {
    properties?: Record<string, unknown>;
    [key: string]: unknown;
  };
}

export interface McpResource {
  uri: string;
  name: string;
  description?: string;
}

export interface McpServer {
  id: string;
  cloudId?: string;
  name: string;
  type: McpTransport;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  enabled: boolean;
  status: McpConnectionStatus;
  error?: string;
  tools?: McpTool[];
  resources?: McpResource[];
  pluginId?: string;
  pluginName?: string;
  serverName?: string;
  isPlugin?: boolean;
}

export interface McpServerPreset {
  id: string;
  name: string;
  type: McpTransport;
  command?: string;
  args?: string[];
  url?: string;
}

export type HookStatus = "idle" | "testing" | "success" | "error";
export type HookEventId = "message.sent" | "task.completed" | "conversation.created";

export interface AroHook {
  id: string;
  cloudId?: string;
  name: string;
  url: string;
  secret?: string;
  events: string[];
  enabled: boolean;
  createdAt: string;
  lastTriggered?: string;
  status?: HookStatus;
}

export interface HookEventOption {
  id: HookEventId;
  label: string;
  description: string;
}

export interface HookPreset {
  id: string;
  name: string;
  url: string;
  events: HookEventId[];
}

export type SchedulerType = "timer" | "cron";
export type SchedulerStatus = "idle" | "running" | "completed" | "error";

export interface ScheduledTask {
  id: string;
  cloudId?: string;
  name: string;
  prompt: string;
  type: SchedulerType;
  durationMinutes?: number;
  cronExpression?: string;
  enabled: boolean;
  createdAt: string;
  lastRun?: string;
  status: SchedulerStatus;
}

export interface SchedulerPreset {
  id: string;
  name: string;
  prompt: string;
  type: SchedulerType;
  durationMinutes?: number;
  cronExpression?: string;
}

export const MCP_SERVER_PRESETS: readonly McpServerPreset[] = [
  {
    id: "filesystem-local",
    name: "Fichiers Locaux",
    type: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-filesystem", "C:/Users/Stagiaire/Documents"],
  },
  {
    id: "git-controller",
    name: "Contrôleur Git",
    type: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-git"],
  },
  {
    id: "postgresql",
    name: "Base PostgreSQL",
    type: "stdio",
    command: "npx",
    args: ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost:5432/db"],
  },
  {
    id: "slack-remote",
    name: "Slack Remote MCP",
    type: "sse",
    url: "https://mcp-slack.fly.dev/sse",
  },
] as const;

export const HOOK_EVENT_CATALOG: readonly HookEventOption[] = [
  {
    id: "message.sent",
    label: "Message Envoyé",
    description: "Se déclenche chaque fois que l'assistant ou l'utilisateur envoie un message.",
  },
  {
    id: "task.completed",
    label: "Tâche Complétée",
    description: "Se déclenche lorsqu'une tâche asynchrone ou d'arrière-plan se termine avec succès.",
  },
  {
    id: "conversation.created",
    label: "Conversation Créée",
    description: "Se déclenche lorsqu'un nouveau fil de discussion commence.",
  },
] as const;

export const HOOK_PRESETS: readonly HookPreset[] = [
  {
    id: "slack-alert",
    name: "Slack Alert",
    url: "https://hooks.slack.com/services/T000/B000/XXXX",
    events: ["task.completed"],
  },
  {
    id: "discord-channel",
    name: "Discord Channel",
    url: "https://discord.com/api/webhooks/0000/XXXX",
    events: ["message.sent", "task.completed"],
  },
  {
    id: "custom-http",
    name: "",
    url: "",
    events: ["message.sent"],
  },
] as const;

export const SCHEDULER_PRESETS: readonly SchedulerPreset[] = [
  {
    id: "coffee-break",
    name: "Pause Café",
    prompt: "Rappelle-moi de faire une pause étirement et de boire un verre d'eau.",
    type: "timer",
    durationMinutes: 10,
  },
  {
    id: "weekday-standup",
    name: "Standup 9h",
    prompt: "Résume mes priorités d'aujourd'hui et rédige un brouillon de message de standup.",
    type: "cron",
    cronExpression: "0 9 * * 1-5",
  },
  {
    id: "friday-review",
    name: "Bilan du vendredi",
    prompt: "Rédige un bilan de mes commits et tâches accomplies cette semaine.",
    type: "cron",
    cronExpression: "0 17 * * 5",
  },
] as const;

export const DEFAULT_MCP_FORM = {
  type: "stdio" as McpTransport,
  name: "",
  command: "",
  args: "",
  url: "",
  env: [] as { key: string; value: string }[],
};

export const DEFAULT_HOOK_FORM = {
  name: "",
  url: "",
  secret: "",
  events: ["message.sent"] as HookEventId[],
};

export const DEFAULT_SCHEDULER_FORM = {
  name: "",
  prompt: "",
  type: "timer" as SchedulerType,
  duration: 15,
  durationMinutes: 15,
  cron: "*/30 * * * *",
  cronExpression: "*/30 * * * *",
};
