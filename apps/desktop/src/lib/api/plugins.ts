import { invoke } from "@tauri-apps/api/core";
import { isTauri, webFetch } from "./transport";
import type { UserSkill } from "../../features/skills/model";
import type { McpServer } from "../../features/integrations/model";

export interface PluginMcpServerSummary {
  name: string;
  transportType: "stdio" | "streamable-http" | "sse";
  command?: string;
  url?: string;
  status: string;
}

export interface PluginSkillSummary {
  id: string;
  name: string;
  description: string;
  icon?: string;
  tags: string[];
  hasScripts: boolean;
}

export interface InstalledPlugin {
  id: string;
  name: string;
  version?: string;
  description?: string;
  author?: string;
  homepage?: string;
  repository?: string;
  license?: string;
  keywords: string[];
  rootPath: string;
  dataPath: string;
  enabled: boolean;
  isSystem?: boolean;
  status: "active" | "inactive" | "error";
  statusMessage?: string;
  installedAt: string;
  source: "local" | "git" | "marketplace";
  mcpServers: PluginMcpServerSummary[];
  skills: PluginSkillSummary[];
  auth?: PluginAuthConfig;
}

export interface PluginAuthConfig {
  authType: "oauth2" | "api_key" | "pat" | "none";
  providerName?: string;
  scopes?: string[];
  instructions?: string;
  documentationUrl?: string;
  defaultLabel?: string;
  supportedMethods?: Array<"oauth2" | "api_key" | "pat">;
  apiKeyPrompt?: string;
  multiAccountSupported?: boolean;
}

export interface PluginAccount {
  id: string;
  pluginId: string;
  accountIdentifier: string;
  label: string;
  email?: string;
  displayName?: string;
  avatarUrl?: string;
  authMethod: "oauth2" | "api_key" | "pat";
  isDefault: boolean;
  status: "active" | "expired" | "revoked" | "error";
  createdAt: string;
  updatedAt: string;
  lastUsedAt?: string;
}

export interface MarketplacePlugin {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  category: string;
  icon: string;
  repository: string;
  keywords: string[];
  mcpServersCount: number;
  skillsCount: number;
  installed: boolean;
  sampleSkills: string[];
  sampleMcpServers: string[];
  auth?: PluginAuthConfig;
}

export interface InstallPluginRequest {
  source: "local" | "git" | "marketplace";
  target: string;
}

export interface CustomSkillInput {
  name: string;
  description: string;
  instructions: string;
  icon?: string;
  tags?: string[];
}

export interface CreateCustomPluginRequest {
  name: string;
  version?: string;
  description?: string;
  author?: string;
  license?: string;
  keywords?: string[];
  mcpServers?: Record<string, any>;
  skills?: CustomSkillInput[];
}

export async function listInstalledPlugins(): Promise<InstalledPlugin[]> {
  if (isTauri()) {
    try {
      return await invoke<InstalledPlugin[]>("plugins_list_installed");
    } catch (err) {
      console.warn("Failed to list installed plugins via Tauri IPC:", err);
      return [];
    }
  }
  try {
    return await webFetch<InstalledPlugin[]>("GET", "/plugins");
  } catch (err) {
    console.warn("Failed to fetch installed plugins:", err);
    return [];
  }
}

export const CURATED_MARKETPLACE: MarketplacePlugin[] = [
  {
    id: "filesystem-tools",
    name: "Filesystem & Workspace",
    version: "1.0.0",
    description: "Standard filesystem operations, tree inspection, and search via Model Context Protocol.",
    author: "ARO Ecosystem",
    category: "Filesystem",
    icon: "📁",
    repository: "https://github.com/agentplugins/filesystem-tools",
    keywords: ["fs", "workspace", "files"],
    mcpServersCount: 1,
    skillsCount: 2,
    installed: false,
    sampleSkills: ["workspace-explorer", "file-finder"],
    sampleMcpServers: ["local-fs"],
  },
  {
    id: "web-search-tools",
    name: "Web Search & Fetch",
    version: "1.2.0",
    description: "Extract clean web page content, search live search engines, and fetch public documentation.",
    author: "ARO Ecosystem",
    category: "Web & Research",
    icon: "🌐",
    repository: "https://github.com/agentplugins/web-search-tools",
    keywords: ["web", "search", "scraper"],
    mcpServersCount: 1,
    skillsCount: 1,
    installed: false,
    sampleSkills: ["documentation-crawler"],
    sampleMcpServers: ["web-fetcher"],
  },
  {
    id: "git-assistant",
    name: "Git Assistant",
    version: "1.1.0",
    description: "Git repository automation, staging, commit formatting, and branch synchronization.",
    author: "ARO Ecosystem",
    category: "Development",
    icon: "🐈",
    repository: "https://github.com/agentplugins/git-assistant",
    keywords: ["git", "version-control", "commits"],
    mcpServersCount: 1,
    skillsCount: 2,
    installed: false,
    sampleSkills: ["smart-commit", "branch-cleaner"],
    sampleMcpServers: ["git-mcp"],
  },
  {
    id: "sqlite-database",
    name: "SQLite & SQL Explorer",
    version: "1.0.1",
    description: "Query, introspect schemas, and manage SQLite databases securely via MCP.",
    author: "ARO Ecosystem",
    category: "Database",
    icon: "🗄️",
    repository: "https://github.com/agentplugins/sqlite-database",
    keywords: ["sqlite", "sql", "database"],
    mcpServersCount: 1,
    skillsCount: 1,
    installed: false,
    sampleSkills: ["schema-auditor"],
    sampleMcpServers: ["sqlite-server"],
  },
  {
    id: "python-analytics",
    name: "Python Analytics",
    version: "1.0.0",
    description: "Run Python scripts, compute statistical calculations, and transform data frames.",
    author: "ARO Ecosystem",
    category: "Data & Code",
    icon: "🐍",
    repository: "https://github.com/agentplugins/python-analytics",
    keywords: ["python", "data", "analytics"],
    mcpServersCount: 1,
    skillsCount: 2,
    installed: false,
    sampleSkills: ["data-summarizer", "chart-generator"],
    sampleMcpServers: ["python-kernel"],
  },
  {
    id: "code-reviewer",
    name: "Code Review & Security",
    version: "1.3.0",
    description: "Automated architectural code reviews, linting validation, and security vulnerability scanning.",
    author: "ARO Ecosystem",
    category: "Code Quality",
    icon: "🛡️",
    repository: "https://github.com/agentplugins/code-reviewer",
    keywords: ["security", "review", "audit"],
    mcpServersCount: 1,
    skillsCount: 2,
    installed: false,
    sampleSkills: ["security-audit", "architecture-check"],
    sampleMcpServers: ["code-reviewer-server"],
  },
  {
    id: "google-workspace",
    name: "Google Workspace & Drive",
    version: "1.0.0",
    description: "Integrate Google Drive, Docs, Gmail, and Calendar into agent workflows via MCP.",
    author: "Google & ARO Ecosystem",
    category: "Productivity",
    icon: "📑",
    repository: "https://github.com/agentplugins/google-workspace",
    keywords: ["google", "workspace", "drive", "docs", "gmail"],
    mcpServersCount: 1,
    skillsCount: 2,
    installed: false,
    sampleSkills: ["workspace-organizer", "calendar-scheduler"],
    sampleMcpServers: ["google-workspace-mcp"],
    auth: {
      authType: "oauth2",
      providerName: "Google Workspace / Gmail",
      scopes: [
        "https://www.googleapis.com/auth/userinfo.email",
        "https://www.googleapis.com/auth/userinfo.profile",
        "https://www.googleapis.com/auth/gmail.modify",
        "https://www.googleapis.com/auth/drive",
      ],
      instructions: "Connectez un ou plusieurs comptes Google (ex: Personnel et Travail) pour permettre à ARO d'accéder à Gmail, Google Drive et Docs.",
      documentationUrl: "https://developers.google.com/identity/protocols/oauth2",
      defaultLabel: "Compte Google",
    },
  },
  {
    id: "openai-ecosystem",
    name: "OpenAI Assistants & Models",
    version: "1.2.0",
    description: "Connect OpenAI Assistants API, prompt evaluations, embeddings, and token analysis via MCP.",
    author: "OpenAI & ARO Ecosystem",
    category: "AI & Models",
    icon: "🤖",
    repository: "https://github.com/agentplugins/openai-ecosystem",
    keywords: ["openai", "chatgpt", "assistants", "embeddings"],
    mcpServersCount: 1,
    skillsCount: 1,
    installed: false,
    sampleSkills: ["prompt-optimizer"],
    sampleMcpServers: ["openai-mcp"],
    auth: {
      authType: "api_key",
      providerName: "OpenAI",
      scopes: [],
      instructions: "Saisissez votre clé secrète d'API OpenAI (sk-...) pour permettre aux outils du plugin d'interagir avec les modèles d'OpenAI.",
      documentationUrl: "https://platform.openai.com/api-keys",
      defaultLabel: "Clé OpenAI",
    },
  },
  {
    id: "github-developer",
    name: "GitHub Developer Suite",
    version: "1.1.0",
    description: "Inspect repositories, triage Pull Requests, manage issues, and audit GitHub Actions workflows via MCP.",
    author: "GitHub & ARO Ecosystem",
    category: "Development",
    icon: "🐙",
    repository: "https://github.com/agentplugins/github-developer",
    keywords: ["github", "git", "pull-request", "issues", "actions"],
    mcpServersCount: 1,
    skillsCount: 2,
    installed: false,
    sampleSkills: ["pr-reviewer", "issue-triager"],
    sampleMcpServers: ["github-mcp"],
    auth: {
      authType: "oauth2",
      providerName: "GitHub",
      scopes: ["repo", "read:user", "user:email"],
      instructions: "Connectez un ou plusieurs comptes GitHub (ou utilisez un Personal Access Token) pour inspecter vos dépôts et PRs.",
      documentationUrl: "https://github.com/settings/tokens",
      defaultLabel: "Compte GitHub",
    },
  },
  {
    id: "slack-workspace",
    name: "Slack Collaboration Hub",
    version: "1.0.0",
    description: "Post messages, query conversation channels, and automate team standup summaries via MCP.",
    author: "Slack & ARO Ecosystem",
    category: "Communication",
    icon: "💬",
    repository: "https://github.com/agentplugins/slack-workspace",
    keywords: ["slack", "messaging", "channels", "collaboration"],
    mcpServersCount: 1,
    skillsCount: 1,
    installed: false,
    sampleSkills: ["standup-reporter"],
    sampleMcpServers: ["slack-mcp"],
    auth: {
      authType: "oauth2",
      providerName: "Slack",
      scopes: ["chat:write", "channels:read"],
      instructions: "Connectez votre espace de travail Slack pour permettre l'envoi de messages et la lecture des canaux d'équipe.",
      documentationUrl: "https://api.slack.com/authentication",
      defaultLabel: "Espace Slack",
    },
  },
];

export async function listMarketplacePlugins(): Promise<MarketplacePlugin[]> {
  if (isTauri()) {
    try {
      const list = await invoke<MarketplacePlugin[]>("plugins_list_marketplace");
      if (Array.isArray(list) && list.length > 0) {
        return list;
      }
    } catch (err) {
      console.warn("Failed to fetch marketplace plugins via Tauri IPC:", err);
    }
  } else {
    try {
      const list = await webFetch<MarketplacePlugin[]>("GET", "/plugins/marketplace");
      if (Array.isArray(list) && list.length > 0) {
        return list;
      }
    } catch (err) {
      console.warn("Failed to fetch marketplace plugins, using curated fallback:", err);
    }
  }
  return CURATED_MARKETPLACE;
}

export async function installPlugin(request: InstallPluginRequest): Promise<InstalledPlugin> {
  if (isTauri()) {
    return await invoke<InstalledPlugin>("plugins_install", { request });
  }
  return await webFetch<InstalledPlugin>("POST", "/plugins/install", request);
}

export async function createCustomPlugin(request: CreateCustomPluginRequest): Promise<InstalledPlugin> {
  if (isTauri()) {
    return await invoke<InstalledPlugin>("plugins_custom_create", { request });
  }
  return await webFetch<InstalledPlugin>("POST", "/plugins/custom", request);
}

export async function getPlugin(pluginId: string): Promise<InstalledPlugin> {
  if (isTauri()) {
    const plugin = await invoke<InstalledPlugin | null>("plugins_get", { pluginId });
    if (!plugin) throw new Error(`Plugin '${pluginId}' not found`);
    return plugin;
  }
  return await webFetch<InstalledPlugin>("GET", `/plugins/${encodeURIComponent(pluginId)}`);
}

export async function togglePlugin(pluginId: string, enabled: boolean): Promise<InstalledPlugin> {
  if (isTauri()) {
    return await invoke<InstalledPlugin>("plugins_toggle", { pluginId, enabled });
  }
  return await webFetch<InstalledPlugin>(
    "POST",
    `/plugins/${encodeURIComponent(pluginId)}/toggle`,
    { enabled }
  );
}

export async function uninstallPlugin(pluginId: string): Promise<{ success: boolean }> {
  if (isTauri()) {
    return await invoke<{ success: boolean }>("plugins_uninstall", { pluginId });
  }
  return await webFetch<{ success: boolean }>(
    "DELETE",
    `/plugins/${encodeURIComponent(pluginId)}`
  );
}

export async function testPluginMcpServer(pluginId: string, serverName: string): Promise<any[]> {
  if (isTauri()) {
    return await invoke<any[]>("plugins_mcp_test", { pluginId, serverName });
  }
  return await webFetch<any[]>(
    "POST",
    `/plugins/${encodeURIComponent(pluginId)}/mcp/${encodeURIComponent(serverName)}/test`,
    {}
  );
}

export async function callPluginMcpTool(
  pluginId: string,
  serverName: string,
  toolName: string,
  args: Record<string, any> = {}
): Promise<any> {
  if (isTauri()) {
    return await invoke<any>("plugins_mcp_call", {
      pluginId,
      serverName,
      toolName,
      arguments: args,
    });
  }
  return await webFetch<any>(
    "POST",
    `/plugins/${encodeURIComponent(pluginId)}/mcp/${encodeURIComponent(serverName)}/call`,
    { toolName, arguments: args }
  );
}

export async function invokePluginSkill(
  pluginId: string,
  skillId: string,
  input: Record<string, any> = {}
): Promise<any> {
  if (isTauri()) {
    return await invoke<any>("plugins_skill_invoke", {
      pluginId,
      skillId,
      input,
    });
  }
  return await webFetch<any>(
    "POST",
    `/plugins/${encodeURIComponent(pluginId)}/skills/${encodeURIComponent(skillId)}/invoke`,
    input
  );
}

export async function listPluginAccounts(pluginId?: string): Promise<PluginAccount[]> {
  if (isTauri()) {
    try {
      return await invoke<PluginAccount[]>("plugins_accounts_list", { pluginId });
    } catch (err) {
      console.warn("Failed to list plugin accounts via Tauri IPC:", err);
      return [];
    }
  }
  try {
    const query = pluginId ? `?pluginId=${encodeURIComponent(pluginId)}` : "";
    return await webFetch<PluginAccount[]>("GET", `/plugins/accounts${query}`);
  } catch (err) {
    console.warn("Failed to fetch plugin accounts:", err);
    return [];
  }
}

export async function startPluginOAuthConnect(request: {
  pluginId: string;
  label?: string;
  clientId?: string;
  clientSecret?: string;
  scopes?: string[];
  openBrowser?: boolean;
}): Promise<{ authUrl: string; state: string; port: number; redirectUri: string }> {
  if (isTauri()) {
    return await invoke<any>("plugins_account_connect_oauth_start", { request });
  }
  return await webFetch<any>("POST", "/plugins/accounts/oauth/start", request);
}

export async function connectPluginApiKey(request: {
  pluginId: string;
  apiKey: string;
  label?: string;
  accountIdentifier?: string;
  authMethod?: string;
}): Promise<PluginAccount> {
  if (isTauri()) {
    return await invoke<PluginAccount>("plugins_account_connect_api_key", { request });
  }
  return await webFetch<PluginAccount>("POST", "/plugins/accounts/api-key", request);
}

export async function setDefaultPluginAccount(accountId: string): Promise<PluginAccount> {
  if (isTauri()) {
    return await invoke<PluginAccount>("plugins_account_set_default", { accountId });
  }
  return await webFetch<PluginAccount>("POST", `/plugins/accounts/${encodeURIComponent(accountId)}/default`);
}

export async function updatePluginAccountLabel(accountId: string, label: string): Promise<PluginAccount> {
  if (isTauri()) {
    return await invoke<PluginAccount>("plugins_account_update_label", { accountId, label });
  }
  return await webFetch<PluginAccount>("PATCH", `/plugins/accounts/${encodeURIComponent(accountId)}/label`, { label });
}

export async function disconnectPluginAccount(accountId: string): Promise<{ success: boolean }> {
  if (isTauri()) {
    return await invoke<{ success: boolean }>("plugins_account_disconnect", { accountId });
  }
  return await webFetch<{ success: boolean }>("DELETE", `/plugins/accounts/${encodeURIComponent(accountId)}`);
}

export async function testPluginAccountHealth(accountId: string): Promise<{
  healthy: boolean;
  status: string;
  message: string;
}> {
  if (isTauri()) {
    return await invoke<any>("plugins_account_test_health", { accountId });
  }
  return await webFetch<any>("POST", `/plugins/accounts/${encodeURIComponent(accountId)}/test-health`);
}

export async function executeCode(request: {
  language: string;
  code: string;
  conversationId?: string;
  args?: string[];
  cwd?: string;
  rootPath?: string;
}): Promise<any> {
  if (isTauri()) {
    return await invoke<any>("code_execute", { request });
  }
  return await webFetch<any>("POST", "/tools/code/execute", request);
}

export async function createDocument(request: {
  documentType: string;
  title: string;
  conversationId?: string;
  description?: string;
  fileName?: string;
  columns?: string[];
  rows?: any[][];
  sections?: any[];
  content?: string;
  metadata?: Record<string, any>;
}): Promise<any> {
  if (isTauri()) {
    return await invoke<any>("document_create", { request });
  }
  return await webFetch<any>("POST", "/tools/document/create", request);
}

export function getPluginFriendlyName(plugin: { id: string; name?: string }): string {
  const match = CURATED_MARKETPLACE.find((m) => m.id === plugin.id);
  if (match?.name) return match.name;
  if (plugin.name && plugin.name !== plugin.id) return plugin.name;
  return plugin.id
    .split("-")
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");
}

export function pluginSkillsToUserSkills(plugins: InstalledPlugin[]): UserSkill[] {
  const skills: UserSkill[] = [];
  for (const plugin of plugins) {
    if (!plugin.skills || !Array.isArray(plugin.skills)) continue;
    const friendlyPluginName = getPluginFriendlyName(plugin);
    const marketplaceItem = CURATED_MARKETPLACE.find((m) => m.id === plugin.id);
    const category = marketplaceItem?.category || "Plugins";

    for (const skill of plugin.skills) {
      skills.push({
        id: `plugin:${plugin.id}:${skill.id}`,
        name: skill.name || skill.id,
        description:
          skill.description ||
          `Compétence fournie par le plugin ${friendlyPluginName}.`,
        icon: skill.icon || marketplaceItem?.icon || "🧩",
        category: category,
        groupId: "plugins",
        triggers: (skill.tags || []).join(", "),
        type: "api",
        content: `Plugin: ${friendlyPluginName} (${plugin.id})\nSkill: ${skill.name}`,
        enabled: plugin.enabled,
        createdAt: plugin.installedAt || new Date().toISOString(),
        pluginId: plugin.id,
        pluginName: friendlyPluginName,
        tags: skill.tags || [],
        isPlugin: true,
      });
    }
  }
  return skills;
}

export function pluginMcpServersToMcpServers(plugins: InstalledPlugin[]): McpServer[] {
  const servers: McpServer[] = [];
  for (const plugin of plugins) {
    if (!plugin.mcpServers || !Array.isArray(plugin.mcpServers)) continue;
    const friendlyPluginName = getPluginFriendlyName(plugin);

    for (const srv of plugin.mcpServers) {
      const isStdio = srv.transportType === "stdio";
      servers.push({
        id: `plugin:${plugin.id}:${srv.name}`,
        name: srv.name,
        serverName: srv.name,
        type: isStdio ? "stdio" : "sse",
        command: srv.command || (isStdio ? "cmd" : undefined),
        url: srv.url || undefined,
        enabled: plugin.enabled,
        status: (plugin.enabled && (srv.status === "ready" || srv.status === "active")
          ? "connected"
          : srv.status === "error"
          ? "error"
          : "disconnected") as "connected" | "disconnected" | "connecting" | "error",
        pluginId: plugin.id,
        pluginName: friendlyPluginName,
        isPlugin: true,
      });
    }
  }
  return servers;
}
