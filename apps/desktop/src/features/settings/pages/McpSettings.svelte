<script lang="ts">
  import { onMount } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import Database from "@lucide/svelte/icons/database";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Link from "@lucide/svelte/icons/link";
  import Loader2 from "@lucide/svelte/icons/loader-2";
  import Network from "@lucide/svelte/icons/network";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import Settings from "@lucide/svelte/icons/settings";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { fade } from "svelte/transition";
  import { MCP_SERVER_PRESETS } from "../../integrations/model";
  import type { McpServer, McpServerPreset } from "../../integrations/model";
  import {
    listInstalledPlugins,
    pluginMcpServersToMcpServers,
    testPluginMcpServer,
  } from "../../../lib/api/plugins";

  type McpEnvironmentEntry = { key: string; value: string };

  const [filesystemPreset, gitPreset, postgresPreset, slackPreset] = MCP_SERVER_PRESETS;

  export let currentTheme: "light" | "dark";
  export let mcpServers: McpServer[] = [];
  export let selectedMcpServerId: string | null = null;
  export let showMcpModal: boolean = false;
  export let mcpModalMode: "add" | "edit" = "add";
  export let mcpFormName: string = "";
  export let mcpFormType: "stdio" | "sse" = "stdio";
  export let mcpFormCommand: string = "";
  export let mcpFormArgs: string = "";
  export let mcpFormUrl: string = "";
  export let mcpFormEnv: McpEnvironmentEntry[] = [];

  export let openAddMcpModal: () => void;
  export let openEditMcpModal: (server: McpServer) => void;
  export let deleteMcpServer: (id: string) => void | Promise<void>;
  export let toggleMcpServer: (id: string) => void;
  export let addPresetMcpServer: (preset: McpServerPreset) => void;
  export let addMcpEnvVar: () => void;
  export let removeMcpEnvVar: (index: number) => void;
  export let handleSaveMcpServer: () => void;

  // Local state for fallback plugin MCP loading & testing
  let localPluginMcpServers: McpServer[] = [];
  let testingServerId: string | null = null;
  let testResults: Record<string, { success: boolean; message: string }> = {};
  // Secrets d'environnement masqués par défaut (anti shoulder-surfing).
  let revealedEnv: Record<string, boolean> = {};

  function envRevealKey(server: McpServer, key: string): string {
    return `${server.id}::${key}`;
  }

  function toggleEnvReveal(server: McpServer, key: string) {
    const k = envRevealKey(server, key);
    revealedEnv = { ...revealedEnv, [k]: !revealedEnv[k] };
  }

  $: isDark = currentTheme === "dark";

  // Merge passed mcpServers with local fallback
  $: allMcpServersList = (() => {
    const existingIds = new Set(mcpServers.map((s) => s.id));
    const merged = [...mcpServers];
    for (const ps of localPluginMcpServers) {
      if (!existingIds.has(ps.id)) {
        merged.push(ps);
      }
    }
    return merged;
  })();

  $: pluginServersCount = allMcpServersList.filter((s) => s.isPlugin).length;
  $: manualServersCount = allMcpServersList.filter((s) => !s.isPlugin).length;

  // Les boutons d'action ne doivent pas replier/déplier la ligne :
  // stopPropagation nommé (explicite, testable) plutôt qu'une flèche vide.
  function stopRowToggle(event: Event) {
    event.stopPropagation();
  }

  async function handleTestMcp(server: McpServer) {
    if (!server.pluginId) return;
    testingServerId = server.id;
    delete testResults[server.id];
    testResults = testResults;

    try {
      const tools = await testPluginMcpServer(
        server.pluginId,
        server.serverName || server.name
      );
      const toolCount = Array.isArray(tools) ? tools.length : 0;
      testResults[server.id] = {
        success: true,
        message: `Serveur MCP opérationnel. ${toolCount} outil${toolCount > 1 ? "s" : ""} détecté${toolCount > 1 ? "s" : ""}.`,
      };
    } catch (err: any) {
      testResults[server.id] = {
        success: false,
        message: err?.message || "Échec de la communication avec le serveur MCP.",
      };
    } finally {
      testingServerId = null;
      testResults = testResults;
      setTimeout(() => {
        delete testResults[server.id];
        testResults = testResults;
      }, 7000);
    }
  }

  onMount(async () => {
    try {
      const plugins = await listInstalledPlugins();
      localPluginMcpServers = pluginMcpServersToMcpServers(plugins);
    } catch (err) {
      console.warn("Failed to load plugin MCP fallback:", err);
    }
  });
</script>

<div class="settings-tab-panel">
  <!-- Header -->
  <div class="panel-header" style="display: flex; justify-content: space-between; align-items: flex-start; width: 100%; margin-bottom: 24px;">
    <div>
      <div style="display: flex; align-items: center; gap: 10px; margin-bottom: 4px;">
        <h2 style="margin: 0; font-size: 22px; font-weight: 600; letter-spacing: -0.5px; color: {isDark ? '#ffffff' : '#1d1d1f'};">
          Model Context Protocol
        </h2>
        {#if pluginServersCount > 0}
          <span
            style="font-size: 11px; font-weight: 600; padding: 2px 8px; border-radius: 980px; background: rgba(0, 113, 227, 0.1); color: #0071e3; border: 1px solid rgba(0, 113, 227, 0.2); display: inline-flex; align-items: center; gap: 4px;"
          >
            <Puzzle size={11} />
            {pluginServersCount} via Plugins
          </span>
        {/if}
      </div>
      <p style="color: #86868b; font-size: 13px; margin: 0; max-width: 620px; line-height: 1.5;">
        Connectez des serveurs MCP pour doter l'assistant d'outils d'accès aux fichiers, bases de données, terminal et APIs tierces.
      </p>
    </div>
    <button
      class="primary-btn"
      style="background: linear-gradient(180deg, #2997ff 0%, #0071e3 100%); color: white; border: none; border-radius: 980px; padding: 8px 16px; font-size: 13px; font-weight: 500; cursor: pointer; box-shadow: 0 4px 12px rgba(0, 113, 227, 0.2); display: flex; align-items: center; gap: 6px; white-space: nowrap; flex-shrink: 0; transition: all 0.2s ease;"
      type="button"
      on:click={openAddMcpModal}
    >
      <Plus size={15} />
      <span>Ajouter un serveur</span>
    </button>
  </div>

  <!-- Presets / Quick Setup -->
  <div style="margin-bottom: 32px;">
    <h3 style="font-size: 14px; font-weight: 600; color: {isDark ? '#f5f5f7' : '#1d1d1f'}; margin-bottom: 12px; letter-spacing: -0.2px;">
      Configurations Rapides
    </h3>
    <div style="display: grid; grid-template-columns: repeat(auto-fill, minmax(190px, 1fr)); gap: 12px;">
      <!-- Filesystem Preset -->
      <div style="background: {isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
        <div>
          <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
            <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(0, 122, 255, 0.12); display: flex; align-items: center; justify-content: center; color: #007aff;">
              <Terminal size={14} />
            </div>
            <span style="font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">Accès Fichiers</span>
          </div>
          <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Permet à l'assistant de lire et écrire des fichiers locaux en toute sécurité.</p>
        </div>
        <button
          type="button"
          on:click={() => addPresetMcpServer(filesystemPreset)}
          style="width: 100%; border: none; background: rgba(0,122,255,0.1); color: #007aff; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
        >
          Configurer
        </button>
      </div>

      <!-- Git Preset -->
      <div style="background: {isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
        <div>
          <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
            <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(255, 69, 58, 0.12); display: flex; align-items: center; justify-content: center; color: #ff453a;">
              <Network size={14} />
            </div>
            <span style="font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">Dépôt Git</span>
          </div>
          <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Ajoute le suivi d'historique git, les commits et le diffing.</p>
        </div>
        <button
          type="button"
          on:click={() => addPresetMcpServer(gitPreset)}
          style="width: 100%; border: none; background: rgba(255,69,58,0.1); color: #ff453a; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
        >
          Configurer
        </button>
      </div>

      <!-- Postgres Preset -->
      <div style="background: {isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
        <div>
          <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
            <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(52, 199, 89, 0.12); display: flex; align-items: center; justify-content: center; color: #34c759;">
              <Database size={14} />
            </div>
            <span style="font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">PostgreSQL</span>
          </div>
          <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Permet d'analyser et d'interroger directement vos schémas de BDD.</p>
        </div>
        <button
          type="button"
          on:click={() => addPresetMcpServer(postgresPreset)}
          style="width: 100%; border: none; background: rgba(52,199,89,0.1); color: #34c759; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
        >
          Configurer
        </button>
      </div>

      <!-- Slack SSE Preset -->
      <div style="background: {isDark ? 'rgba(255,255,255,0.03)' : 'rgba(0,0,0,0.02)'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'}; border-radius: 12px; padding: 14px; display: flex; flex-direction: column; justify-content: space-between; min-height: 120px; transition: all 0.2s ease;">
        <div>
          <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
            <div style="width: 24px; height: 24px; border-radius: 6px; background: rgba(175, 82, 222, 0.12); display: flex; align-items: center; justify-content: center; color: #af52de;">
              <Link size={14} />
            </div>
            <span style="font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">Slack SSE</span>
          </div>
          <p style="font-size: 11px; color: #86868b; line-height: 1.4; margin: 0;">Connecte un serveur de diffusion réseau pour vos espaces Slack.</p>
        </div>
        <button
          type="button"
          on:click={() => addPresetMcpServer(slackPreset)}
          style="width: 100%; border: none; background: rgba(175,82,222,0.1); color: #af52de; font-size: 11px; font-weight: 600; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; margin-top: 10px;"
        >
          Configurer
        </button>
      </div>
    </div>
  </div>

  <!-- Server List -->
  <div>
    <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;">
      <h3 style="font-size: 14px; font-weight: 600; color: {isDark ? '#f5f5f7' : '#1d1d1f'}; margin: 0; letter-spacing: -0.2px;">
        Serveurs MCP Configurés
      </h3>
      {#if allMcpServersList.length > 0}
        <span style="font-size: 12px; color: #86868b;">
          {allMcpServersList.length} serveur{allMcpServersList.length > 1 ? 's' : ''} ({pluginServersCount} via plugins)
        </span>
      {/if}
    </div>

    {#if allMcpServersList.length === 0}
      <div style="text-align: center; padding: 48px 24px; background: {isDark ? 'rgba(255,255,255,0.02)' : 'rgba(0,0,0,0.01)'}; border: 1px dashed {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; border-radius: 16px;">
        <Network size={32} style="color: #86868b; margin-bottom: 12px; display: inline-block;" />
        <p style="font-size: 13px; color: #86868b; margin: 0;">Aucun serveur MCP configuré pour le moment.</p>
        <button
          type="button"
          on:click={openAddMcpModal}
          style="margin-top: 12px; background: none; border: 1px solid #007aff; color: #007aff; border-radius: 980px; padding: 6px 14px; font-size: 12px; font-weight: 500; cursor: pointer;"
        >
          Ajouter un premier serveur
        </button>
      </div>
    {:else}
      <div style="display: flex; flex-direction: column; gap: 8px;">
        {#each allMcpServersList as server}
          {@const isSelected = selectedMcpServerId === server.id}
          {@const isConnected = server.enabled && (server.status === "connected" || (server.status as string) === "ready" || Boolean(server.isPlugin))}
          {@const result = testResults[server.id]}

          <div
            role="button"
            tabindex="0"
            on:click={() => (selectedMcpServerId = isSelected ? null : server.id)}
            on:keydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                selectedMcpServerId = isSelected ? null : server.id;
              }
            }}
            style="background: {isSelected ? (isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.03)') : (isDark ? 'rgba(255,255,255,0.02)' : '#ffffff')}; border: 1px solid {isSelected ? '#007aff' : (isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.08)')}; border-radius: 12px; padding: 14px 18px; display: flex; align-items: center; justify-content: space-between; cursor: pointer; transition: all 0.2s ease;"
          >
            <div style="display: flex; align-items: center; gap: 14px;">
              <!-- Connection Status Dot -->
              <div style="position: relative;">
                <div style="width: 34px; height: 34px; border-radius: 50%; background: {isDark ? 'rgba(255,255,255,0.04)' : 'rgba(0,0,0,0.03)'}; display: flex; align-items: center; justify-content: center; color: #86868b;">
                  {#if server.type === "stdio"}
                    <Terminal size={15} />
                  {:else}
                    <Link size={15} />
                  {/if}
                </div>
                <span
                  style="position: absolute; bottom: -2px; right: -2px; width: 10px; height: 10px; border-radius: 50%; border: 2px solid {isDark ? '#1c1c1e' : '#ffffff'}; background: {isConnected ? '#34c759' : server.status === 'connecting' ? '#007aff' : server.status === 'error' ? '#ff453a' : '#86868b'};"
                  class:pulse-loading={server.status === 'connecting'}
                ></span>
              </div>

              <div>
                <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                  <span style="font-size: 14px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">
                    {server.name}
                  </span>
                  <span style="font-size: 10px; text-transform: uppercase; background: {isDark ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.05)'}; color: #86868b; padding: 2px 6px; border-radius: 980px; font-weight: 600;">
                    {server.type}
                  </span>
                  {#if server.isPlugin}
                    <span style="font-size: 10px; font-weight: 600; background: rgba(0, 113, 227, 0.08); color: #0071e3; border: 1px solid rgba(0, 113, 227, 0.18); padding: 1px 7px; border-radius: 6px; display: inline-flex; align-items: center; gap: 3px;">
                      <Puzzle size={10} />
                      {server.pluginName || 'Plugin'}
                    </span>
                  {/if}
                </div>
                <div style="font-size: 11px; color: #86868b; margin-top: 2px; max-width: 450px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                  {#if server.type === "stdio"}
                    <code>{server.command || "stdio"} {(server.args || []).join(' ')}</code>
                  {:else}
                    <code>{server.url || "sse"}</code>
                  {/if}
                </div>
              </div>
            </div>

            <div style="display: flex; align-items: center; gap: 14px;" on:click|stopPropagation={stopRowToggle} on:keydown|stopPropagation={stopRowToggle} role="none">
              <!-- Test MCP button for plugins -->
              {#if server.isPlugin && server.pluginId}
                <button
                  type="button"
                  disabled={testingServerId === server.id}
                  on:click={() => handleTestMcp(server)}
                  style="background: {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.08)'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; border-radius: 6px; padding: 4px 8px; font-size: 11px; font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 4px; transition: all 0.2s;"
                  title="Tester la connectivité MCP"
                >
                  {#if testingServerId === server.id}
                    <Loader2 size={12} class="animate-spin" />
                    <span>Test...</span>
                  {:else}
                    <Play size={11} style="color: #34c759;" />
                    <span>Tester</span>
                  {/if}
                </button>
              {/if}

              <!-- Edit/Delete or Plugin origin badge -->
              {#if server.isPlugin}
                <span style="font-size: 11px; color: #86868b; font-style: italic;">Plugin</span>
              {:else}
                <div style="display: flex; align-items: center; gap: 8px;">
                  <button
                    type="button"
                    on:click={() => openEditMcpModal(server)}
                    style="background: none; border: none; color: #86868b; cursor: pointer; padding: 4px;"
                    title="Modifier"
                  >
                    <Settings size={14} />
                  </button>
                  <button
                    type="button"
                    on:click={() => deleteMcpServer(server.id)}
                    style="background: none; border: none; color: #ff453a; cursor: pointer; padding: 4px;"
                    title="Supprimer"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              {/if}

              <!-- iOS Style toggle switch -->
              <label class="ios-switch" style="margin-left: 6px;">
                <input
                  type="checkbox"
                  checked={server.enabled}
                  on:change={() => toggleMcpServer(server.id)}
                />
                <span class="ios-slider"></span>
              </label>
            </div>
          </div>

          <!-- Test result banner if available -->
          {#if result}
            <div
              style="padding: 8px 14px; border-radius: 8px; font-size: 12px; margin-top: -4px; margin-bottom: 6px; display: flex; align-items: center; gap: 8px;
                background: {result.success ? 'rgba(52, 199, 89, 0.1)' : 'rgba(255, 69, 58, 0.1)'};
                color: {result.success ? '#34c759' : '#ff453a'}; border: 1px solid {result.success ? 'rgba(52, 199, 89, 0.2)' : 'rgba(255, 69, 58, 0.2)'};"
            >
              {#if result.success}
                <Check size={13} />
              {:else}
                <X size={13} />
              {/if}
              <span>{result.message}</span>
            </div>
          {/if}

          <!-- Selected Detail Pane -->
          {#if isSelected}
            <div style="margin-top: -4px; margin-bottom: 8px; background: {isDark ? 'rgba(255,255,255,0.01)' : 'rgba(0,0,0,0.005)'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.05)'}; border-top: none; border-radius: 0 0 12px 12px; padding: 18px 24px; transition: all 0.2s ease;">
              {#if server.isPlugin}
                <div style="display: flex; flex-direction: column; gap: 12px;">
                  <div style="display: flex; align-items: center; justify-content: space-between;">
                    <div>
                      <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 4px;">Fourni par Plugin</span>
                      <span style="font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">{server.pluginName} ({server.pluginId})</span>
                    </div>
                    <span style="font-size: 11px; padding: 3px 8px; border-radius: 6px; background: rgba(52, 199, 89, 0.1); color: #34c759; font-weight: 600;">
                      {isConnected ? '✓ Prêt et Actif' : 'Désactivé'}
                    </span>
                  </div>

                  <div>
                    <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 4px;">Transport & Commande</span>
                    <div style="background: {isDark ? '#1c1c1e' : '#f5f5f7'}; border-radius: 8px; padding: 8px 12px; font-family: monospace; font-size: 11px; color: {isDark ? '#ffffff' : '#1d1d1f'};">
                      {#if server.type === "stdio"}
                        <code>{server.command || "cmd"} {(server.args || []).join(' ')}</code>
                      {:else}
                        <code>{server.url}</code>
                      {/if}
                    </div>
                  </div>
                </div>
              {:else}
                <!-- Connection variables/details for manual servers -->
                {#if server.env && Object.keys(server.env).length > 0}
                  <div style="margin-bottom: 16px;">
                    <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 6px;">Variables d'environnement</span>
                    <div style="background: {isDark ? '#1c1c1e' : '#f5f5f7'}; border-radius: 8px; padding: 8px 12px; font-family: monospace; font-size: 11px; display: grid; gap: 4px;">
                      {#each Object.entries(server.env) as [k, v]}
                        <div style="display: flex; align-items: center; gap: 6px;">
                          <span style="color: #ff453a;">{k}</span>=<span style="color: #30b0c7;">{revealedEnv[envRevealKey(server, k)] ? v : "••••••••"}</span>
                          <button
                            type="button"
                            on:click={() => toggleEnvReveal(server, k)}
                            title={revealedEnv[envRevealKey(server, k)] ? "Masquer" : "Révéler"}
                            style="background: transparent; border: none; cursor: pointer; color: #86868b; font-size: 10px; padding: 0 4px;"
                          >{revealedEnv[envRevealKey(server, k)] ? "Masquer" : "Voir"}</button>
                        </div>
                      {/each}
                    </div>
                  </div>
                {/if}

                <!-- Exposed Tools -->
                <div style="margin-bottom: 16px;">
                  <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 8px;">Outils Exposés (Tools)</span>
                  {#if !server.tools || server.tools.length === 0}
                    <span style="font-size: 12px; color: #86868b; font-style: italic;">Aucun outil déclaré par ce serveur.</span>
                  {:else}
                    <div style="display: flex; flex-direction: column; gap: 8px;">
                      {#each server.tools as tool}
                        <div style="background: {isDark ? 'rgba(255,255,255,0.02)' : '#ffffff'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.05)'}; border-radius: 8px; padding: 10px 12px;">
                          <div style="display: flex; align-items: center; justify-content: space-between;">
                            <code style="font-size: 12px; color: #ff9500; font-weight: 600;">{tool.name}</code>
                            <span style="font-size: 9px; text-transform: uppercase; background: rgba(255, 149, 0, 0.1); color: #ff9500; padding: 1px 6px; border-radius: 4px; font-weight: 600;">Tool</span>
                          </div>
                          <p style="font-size: 12px; color: #86868b; margin: 6px 0 0 0; line-height: 1.4;">{tool.description || 'Aucune description fournie.'}</p>
                          {#if tool.inputSchema && tool.inputSchema.properties}
                            <div style="margin-top: 8px; display: flex; flex-wrap: wrap; gap: 4px;">
                              {#each Object.keys(tool.inputSchema.properties) as prop}
                                <span style="font-size: 10px; font-family: monospace; background: {isDark ? '#2c2c2e' : '#f5f5f7'}; border-radius: 4px; padding: 1px 6px; color: #86868b;">{prop}</span>
                              {/each}
                            </div>
                          {/if}
                        </div>
                      {/each}
                    </div>
                  {/if}
                </div>

                <!-- Exposed Resources -->
                {#if server.resources && server.resources.length > 0}
                  <div>
                    <span style="font-size: 11px; font-weight: 600; text-transform: uppercase; color: #86868b; display: block; margin-bottom: 8px;">Ressources Exposées</span>
                    <div style="display: flex; flex-direction: column; gap: 6px;">
                      {#each server.resources as resource}
                        <div style="background: {isDark ? 'rgba(255,255,255,0.02)' : '#ffffff'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.05)'}; border-radius: 8px; padding: 10px 12px; display: flex; align-items: center; gap: 8px;">
                          <div style="color: #007aff;"><FolderOpen size={14} /></div>
                          <div>
                            <span style="font-size: 12px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'}; display: block;">{resource.name}</span>
                            <code style="font-size: 10px; color: #86868b;">{resource.uri}</code>
                          </div>
                        </div>
                      {/each}
                    </div>
                  </div>
                {/if}
              {/if}
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>

<!-- MCP Server Configuration Modal -->
{#if showMcpModal}
  <div
    class="skill-modal-overlay"
    style="position: fixed; inset: 0; background: rgba(0,0,0,0.4); backdrop-filter: blur(12px); display: flex; align-items: center; justify-content: center; z-index: 10000;"
    transition:fade={{ duration: 200 }}
  >
    <div
      class="skill-modal"
      style="width: 500px; max-width: 95vw; background: {isDark ? '#1c1c1e' : '#f5f5f7'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; border-radius: 16px; box-shadow: 0 20px 50px rgba(0,0,0,0.35); overflow: hidden; display: flex; flex-direction: column;"
    >
      <!-- Header -->
      <div style="display: flex; align-items: center; justify-content: space-between; padding: 18px 24px; border-bottom: 1px solid {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
        <h3 style="margin: 0; font-size: 16px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">
          {mcpModalMode === 'add' ? 'Ajouter un serveur MCP' : 'Modifier le serveur MCP'}
        </h3>
        <button
          type="button"
          on:click={() => (showMcpModal = false)}
          style="background: none; border: none; color: #86868b; cursor: pointer; display: flex; align-items: center; justify-content: center; padding: 4px; border-radius: 50%;"
        >
          <X size={16} />
        </button>
      </div>

      <!-- Form Content -->
      <div style="padding: 24px; display: flex; flex-direction: column; gap: 16px; max-height: 60vh; overflow-y: auto;">
        <!-- Segmented Control for Type -->
        <div style="display: flex; background: {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.05)'}; border-radius: 8px; padding: 2px;">
          <button
            type="button"
            on:click={() => (mcpFormType = "stdio")}
            style="flex: 1; border: none; background: {mcpFormType === 'stdio' ? (isDark ? '#3a3a3c' : '#ffffff') : 'none'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; font-weight: 500; font-size: 12px; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; box-shadow: {mcpFormType === 'stdio' ? '0 1px 3px rgba(0,0,0,0.1)' : 'none'};"
          >
            Stdio (Local)
          </button>
          <button
            type="button"
            on:click={() => (mcpFormType = "sse")}
            style="flex: 1; border: none; background: {mcpFormType === 'sse' ? (isDark ? '#3a3a3c' : '#ffffff') : 'none'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; font-weight: 500; font-size: 12px; padding: 6px 0; border-radius: 6px; cursor: pointer; transition: all 0.2s ease; box-shadow: {mcpFormType === 'sse' ? '0 1px 3px rgba(0,0,0,0.1)' : 'none'};"
          >
            SSE (Réseau)
          </button>
        </div>

        <!-- Server Name -->
        <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">
          Nom du serveur
          <input
            type="text"
            bind:value={mcpFormName}
            placeholder="Ex: Fichiers de projet"
            style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {isDark ? '#2c2c2e' : '#ffffff'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
          />
        </label>

        {#if mcpFormType === "stdio"}
          <!-- Command -->
          <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">
            Commande d'exécution (Command)
            <input
              type="text"
              bind:value={mcpFormCommand}
              placeholder="Ex: npx, node, python"
              style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {isDark ? '#2c2c2e' : '#ffffff'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
            />
          </label>

          <!-- Arguments -->
          <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">
            Arguments (séparés par un espace)
            <input
              type="text"
              bind:value={mcpFormArgs}
              placeholder="Ex: -y @modelcontextprotocol/server-git"
              style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {isDark ? '#2c2c2e' : '#ffffff'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
            />
          </label>

          <!-- Environment Variables -->
          <div>
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
              <span style="font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">Variables d'environnement</span>
              <button
                type="button"
                on:click={addMcpEnvVar}
                style="background: none; border: none; color: #007aff; font-size: 12px; font-weight: 500; cursor: pointer; display: flex; align-items: center; gap: 4px;"
              >
                <Plus size={12} />
                <span>Ajouter</span>
              </button>
            </div>

            {#if mcpFormEnv.length === 0}
              <p style="font-size: 11px; color: #86868b; margin: 0; font-style: italic;">Aucune variable d'environnement définie.</p>
            {:else}
              <div style="display: flex; flex-direction: column; gap: 8px;">
                {#each mcpFormEnv as item, i}
                  <div style="display: flex; align-items: center; gap: 8px;">
                    <input
                      type="text"
                      bind:value={item.key}
                      placeholder="Clé (ex: API_KEY)"
                      style="flex: 1; padding: 6px 8px; border-radius: 6px; background: {isDark ? '#2c2c2e' : '#ffffff'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; font-size: 12px;"
                    />
                    <span style="color: #86868b;">=</span>
                    <input
                      type="text"
                      bind:value={item.value}
                      placeholder="Valeur"
                      style="flex: 1; padding: 6px 8px; border-radius: 6px; background: {isDark ? '#2c2c2e' : '#ffffff'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; font-size: 12px;"
                    />
                    <button
                      type="button"
                      on:click={() => removeMcpEnvVar(i)}
                      style="background: none; border: none; color: #ff453a; cursor: pointer; padding: 4px;"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {:else}
          <!-- URL (SSE) -->
          <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {isDark ? '#ffffff' : '#1d1d1f'};">
            URL du Endpoint SSE
            <input
              type="url"
              bind:value={mcpFormUrl}
              placeholder="https://localhost:3000/sse"
              style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {isDark ? '#2c2c2e' : '#ffffff'}; color: {isDark ? '#ffffff' : '#1d1d1f'}; border: 1px solid {isDark ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
            />
          </label>
        {/if}
      </div>

      <!-- Footer Buttons -->
      <div style="display: flex; align-items: center; justify-content: flex-end; gap: 12px; padding: 18px 24px; border-top: 1px solid {isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
        <button
          type="button"
          on:click={() => (showMcpModal = false)}
          style="background: none; border: 1px solid {isDark ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.15)'}; border-radius: 8px; padding: 8px 16px; font-size: 13px; font-weight: 500; color: {isDark ? '#ffffff' : '#1d1d1f'}; cursor: pointer;"
        >
          Annuler
        </button>
        <button
          type="button"
          on:click={handleSaveMcpServer}
          style="background: linear-gradient(180deg, #2997ff 0%, #0071e3 100%); color: white; border: none; border-radius: 8px; padding: 8px 16px; font-size: 13px; font-weight: 500; cursor: pointer; box-shadow: 0 1px 3px rgba(0,0,0,0.15);"
        >
          Enregistrer
        </button>
      </div>
    </div>
  </div>
{/if}
