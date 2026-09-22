<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Brain from "@lucide/svelte/icons/brain";
  import Check from "@lucide/svelte/icons/check";
  import Clock from "@lucide/svelte/icons/clock";
  import Copy from "@lucide/svelte/icons/copy";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Database from "@lucide/svelte/icons/database";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import Layers from "@lucide/svelte/icons/layers";
  import Pin from "@lucide/svelte/icons/pin";
  import PinOff from "@lucide/svelte/icons/pin-off";
  import Plus from "@lucide/svelte/icons/plus";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Search from "@lucide/svelte/icons/search";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import Wand2 from "@lucide/svelte/icons/wand-2";
  import Zap from "@lucide/svelte/icons/zap";
  import type {
    EpisodeItem,
    LongTermMemoryItem,
    MemoryCategory,
    MemoryIndexStatus,
    MemoryConfigurationSettings,
    MemoryContextMode,
    ModelRef,
  } from "../../../lib/types";
  import { makeDefaultMemorySettings } from "../../../lib/api/transport";

  type MemoryFilter = "all" | MemoryCategory;
  type MemoryLabels = {
    memoryTitle: string;
    memoryDesc: string;
    memoryLimitLabel: string;
    memorySearchPlaceholder: string;
    addMemoryBtn: string;
    catPersonal: string;
    catTechnical: string;
    catSystem: string;
    catPreference: string;
    newMemoryPlaceholder: string;
    newMemoryCategoryLabel: string;
    cancelBtn: string;
    saveBtn: string;
    noMemoriesFound: string;
  };

  export let t: MemoryLabels;
  export let currentLanguage: "fr" | "en" = "fr";
  export let currentTheme: "light" | "dark" = "light";
  export let memoriesList: LongTermMemoryItem[] = [];
  export let episodesList: EpisodeItem[] = [];
  export let memoryIndex: MemoryIndexStatus | null = null;
  export let memoryIndexBusy = false;
  export let memoryEntryLimit = 500;
  $: void memoryEntryLimit;
  export let memorySearchQuery = "";
  export let selectedMemoryFilter: MemoryFilter = "all";
  export let showAddMemoryInline = false;
  export let newMemoryText = "";
  export let newMemoryCategory: MemoryCategory = "personal";
  export let newMemorySalience = 0.7;
  export let newMemoryPinned = false;
  export let editingMemoryId: string | null = null;
  export let editingMemoryText = "";

  export let refreshMemoryIndexStatus: () => void | Promise<void>;
  export let reindexMemories: () => void | Promise<void>;
  export let addMemory: () => void | Promise<void>;
  export let updateMemory: (memory: LongTermMemoryItem) => void | Promise<void>;
  export let deleteMemory: (memory: LongTermMemoryItem) => void | Promise<void>;
  export let togglePinMemory: ((memory: LongTermMemoryItem) => void | Promise<void>) | undefined = undefined;
  export let onRefreshEpisodes: (() => void | Promise<void>) | undefined = undefined;

  export let memorySettings: MemoryConfigurationSettings | undefined = undefined;
  export let onSaveMemorySettings: ((settings: MemoryConfigurationSettings) => void | Promise<void>) | undefined = undefined;
  export let activeModel: ModelRef | null = null;

  export function resolveAutoCeiling(model?: ModelRef | null): number {
    if (!model) return 32768;
    const id = (model.modelId || "").toLowerCase();
    const provider = (model.providerKind || "").toLowerCase();
    if (provider === "google" || id.includes("gemini")) {
      return 1000000;
    }
    if (provider === "anthropic" || id.includes("claude")) {
      return 200000;
    }
    if (provider === "openai" || id.includes("gpt-4") || id.includes("o1") || id.includes("o3") || id.includes("o4")) {
      return 128000;
    }
    if (id.includes("qwen") || id.includes("deepseek") || id.includes("mistral") || id.includes("llama-3")) {
      return 32768;
    }
    return 32768;
  }

  let activeSubTab: "semantic" | "episodic" | "config" | "architecture" = "config";
  let activeQuickFilter: "all" | "pinned" | "high_salience" | "frequent" = "all";
  let copiedId: string | null = null;

  let configDraft: MemoryConfigurationSettings = memorySettings
    ? { ...memorySettings }
    : makeDefaultMemorySettings();

  let prevMemorySettings = memorySettings;
  $: if (memorySettings && memorySettings !== prevMemorySettings) {
    prevMemorySettings = memorySettings;
    configDraft = { ...memorySettings };
  }

  let saveSuccess = false;
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let isSaving = false;

  async function handleSaveConfig() {
    if (onSaveMemorySettings) {
      isSaving = true;
      try {
        await onSaveMemorySettings({ ...configDraft });
        saveSuccess = true;
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
          saveSuccess = false;
        }, 2200);
      } finally {
        isSaving = false;
      }
    }
  }

  function handleResetConfig() {
    configDraft = makeDefaultMemorySettings();
    handleSaveConfig();
  }

  function applyContextPreset(preset: MemoryContextMode) {
    configDraft.contextMode = preset;
    let ceiling = 8192;
    if (preset === "128k") ceiling = 128000;
    else if (preset === "200k") ceiling = 200000;
    else if (preset === "64k") ceiling = 65536;
    else if (preset === "32k") ceiling = 32768;
    else if (preset === "16k") ceiling = 16384;
    else if (preset === "8k") ceiling = 8192;
    else if (preset === "4k") ceiling = 4096;
    else if (preset === "1m") ceiling = 1000000;
    else if (preset === "auto") ceiling = resolveAutoCeiling(activeModel);

    configDraft.totalTokenCeiling = ceiling;
    rebalancePartitions(ceiling);
    handleSaveConfig();
  }

  function rebalancePartitions(ceiling: number) {
    const total = Math.max(ceiling, 2048);
    configDraft.systemBudget = Math.min(Math.max(Math.round((total * 10) / 100), 400), 32000);
    configDraft.semanticBudget = Math.max(Math.round((total * 20) / 100), 600);
    configDraft.episodicBudget = Math.max(Math.round((total * 25) / 100), 800);
    configDraft.workingBudget = Math.max(Math.round((total * 30) / 100), 1000);
    const allocated =
      configDraft.systemBudget +
      configDraft.semanticBudget +
      configDraft.episodicBudget +
      configDraft.workingBudget;
    configDraft.reserveBudget = total > allocated ? total - allocated : 400;
  }

  $: totalCeiling = Math.max(configDraft.totalTokenCeiling, 2048);
  $: currentSum =
    configDraft.systemBudget +
    configDraft.semanticBudget +
    configDraft.episodicBudget +
    configDraft.workingBudget +
    configDraft.reserveBudget;
  $: systemPct = ((configDraft.systemBudget / totalCeiling) * 100).toFixed(1);
  $: semanticPct = ((configDraft.semanticBudget / totalCeiling) * 100).toFixed(1);
  $: episodicPct = ((configDraft.episodicBudget / totalCeiling) * 100).toFixed(1);
  $: workingPct = ((configDraft.workingBudget / totalCeiling) * 100).toFixed(1);
  $: reservePct = Math.max(
    0,
    100 -
      parseFloat(systemPct) -
      parseFloat(semanticPct) -
      parseFloat(episodicPct) -
      parseFloat(workingPct)
  ).toFixed(1);

  function formatNumber(n: number): string {
    return n.toLocaleString(currentLanguage === "fr" ? "fr-FR" : "en-US");
  }

  $: pinnedCount = memoriesList.filter((m) => m.pinned).length;
  $: avgSalience = memoriesList.length
    ? memoriesList.reduce((acc, m) => acc + (m.salience || 0.5), 0) / memoriesList.length
    : 0.5;

  $: totalDecisions = episodesList.reduce(
    (acc, ep) => acc + (ep.keyDecisions?.length || 0),
    0
  );
  $: totalEntities = episodesList.reduce(
    (acc, ep) => acc + (ep.entities?.length || 0),
    0
  );

  function handleCopy(id: string, text: string) {
    navigator.clipboard.writeText(text);
    copiedId = id;
    setTimeout(() => {
      if (copiedId === id) copiedId = null;
    }, 1800);
  }

  async function handleTogglePin(memory: LongTermMemoryItem) {
    if (togglePinMemory) {
      await togglePinMemory(memory);
    } else {
      await updateMemory({
        ...memory,
        pinned: !memory.pinned,
      });
    }
  }

  function getSalienceLabel(salience: number): { label: string; tone: string } {
    if (salience >= 0.8) return { label: currentLanguage === "fr" ? "Critique" : "Critical", tone: "#ff3b30" };
    if (salience >= 0.6) return { label: currentLanguage === "fr" ? "Élevée" : "High", tone: "#ff9500" };
    if (salience >= 0.4) return { label: currentLanguage === "fr" ? "Moyenne" : "Medium", tone: "#0071e3" };
    return { label: currentLanguage === "fr" ? "Faible" : "Low", tone: "#86868b" };
  }

  function formatRelativeDate(isoDate: string): string {
    try {
      const d = new Date(isoDate);
      const now = new Date();
      const diffMs = now.getTime() - d.getTime();
      const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
      if (diffHours < 1) return currentLanguage === "fr" ? "À l'instant" : "Just now";
      if (diffHours < 24) return currentLanguage === "fr" ? `Il y a ${diffHours}h` : `${diffHours}h ago`;
      const diffDays = Math.floor(diffHours / 24);
      if (diffDays < 7) return currentLanguage === "fr" ? `Il y a ${diffDays}j` : `${diffDays}d ago`;
      return d.toLocaleDateString(currentLanguage === "fr" ? "fr-FR" : "en-US", { month: "short", day: "numeric" });
    } catch {
      return isoDate.split("T")[0] || "";
    }
  }

  $: filteredMemories = memoriesList.filter((m) => {
    const q = memorySearchQuery.toLowerCase().trim();
    const matchSearch =
      !q ||
      m.content.toLowerCase().includes(q) ||
      m.category.toLowerCase().includes(q);

    const matchCategory =
      selectedMemoryFilter === "all" || m.category === selectedMemoryFilter;

    let matchQuick = true;
    if (activeQuickFilter === "pinned") matchQuick = m.pinned;
    else if (activeQuickFilter === "high_salience") matchQuick = m.salience >= 0.7;
    else if (activeQuickFilter === "frequent") matchQuick = (m.recallCount || 0) > 0;

    return matchSearch && matchCategory && matchQuick;
  });
</script>

<div class="settings-tab-panel cognitive-memory-container">
  <!-- Hero Cognitive Header -->
  <div class="cognitive-hero-card">
    <div class="cognitive-hero-header">
      <div class="cognitive-hero-title-group">
        <h2>
          <Brain size={22} style="color: #af52de;" />
          <span>{currentLanguage === "fr" ? "Mémoire Cognitive ARO" : "ARO Cognitive Memory"}</span>
        </h2>
        <p style="margin: 0; font-size: 13px; color: #86868b; line-height: 1.5; max-width: 640px;">
          {currentLanguage === "fr"
            ? "Architecture multi-niveaux d'inspiration AGI : mémoire de travail active bornée, consolidation épisodique continue par rollups et mémoire sémantique à long terme avec utilité dynamique d'Ebbinghaus."
            : "Multi-tiered cognitive architecture inspired by AGI: bounded working memory, continuous episodic rollup consolidation, and long-term semantic knowledge with Ebbinghaus dynamic utility."}
        </p>
      </div>
      <div style="display: flex; align-items: center; gap: 8px;">
        <span class="cognitive-status-pill">
          <span class="pulse-indicator"></span>
          <span>{currentLanguage === "fr" ? "Cognition AGI Active" : "AGI Cognition Active"}</span>
        </span>
        <button
          type="button"
          class="apple-btn primary"
          style="font-size: 12px; padding: 6px 12px; height: 32px; display: flex; align-items: center; gap: 6px;"
          on:click={() => {
            activeSubTab = "semantic";
            showAddMemoryInline = true;
          }}
        >
          <Plus size={14} />
          <span>{t.addMemoryBtn}</span>
        </button>
      </div>
    </div>

    <!-- 4 Real-time Metrics Cards (Unlimited Capacity) -->
    <div class="cognitive-metrics-grid">
      <!-- Card 1: Long-Term Semantic (Unlimited Capacity) -->
      <div class="cognitive-metric-card">
        <div class="metric-card-top">
          <span>{currentLanguage === "fr" ? "Long Terme · Sémantique" : "Long-Term · Semantic"}</span>
          <Brain size={14} style="color: #0071e3;" />
        </div>
        <div class="metric-card-value" style="display: flex; align-items: baseline; gap: 6px;">
          <span>{memoriesList.length}</span>
          <span style="font-size: 11px; font-weight: 700; color: #34c759; background: rgba(52,199,89,0.12); padding: 2px 7px; border-radius: 12px; letter-spacing: -0.2px;">
            ∞ {currentLanguage === "fr" ? "Illimité" : "Unlimited"}
          </span>
        </div>
        <div class="metric-card-sub">
          {pinnedCount} {currentLanguage === "fr" ? "épinglés" : "pinned"} · {currentLanguage === "fr" ? "Saillance moy." : "Avg salience"} {(avgSalience * 100).toFixed(0)}%
        </div>
        <div style="margin-top: 6px; font-size: 11px; color: #34c759; display: flex; align-items: center; gap: 5px; font-weight: 600;">
          <ShieldCheck size={12} />
          <span>{currentLanguage === "fr" ? "Stockage permanent SQLite WAL" : "Permanent SQLite WAL"}</span>
        </div>
      </div>

      <!-- Card 2: Medium-Term Episodic -->
      <div class="cognitive-metric-card">
        <div class="metric-card-top">
          <span>{currentLanguage === "fr" ? "Moyen Terme · Épisodique" : "Medium-Term · Episodic"}</span>
          <BookOpen size={14} style="color: #af52de;" />
        </div>
        <div class="metric-card-value">
          {episodesList.length} <span style="font-size: 13px; color: #86868b; font-weight: 500;">{currentLanguage === "fr" ? "rollups" : "rollups"}</span>
        </div>
        <div class="metric-card-sub">
          {totalDecisions} {currentLanguage === "fr" ? "décisions" : "decisions"} · {totalEntities} {currentLanguage === "fr" ? "entités clés" : "entities"}
        </div>
        <div style="margin-top: 6px; font-size: 11px; color: #af52de; display: flex; align-items: center; gap: 5px; font-weight: 600;">
          <Activity size={12} />
          <span>{currentLanguage === "fr" ? `Compaction tous les ${configDraft.compactionInterval} tours` : `Compaction every ${configDraft.compactionInterval} turns`}</span>
        </div>
      </div>

      <!-- Card 3: Dynamic Token Ceiling & Context Window -->
      <div class="cognitive-metric-card">
        <div class="metric-card-top">
          <span>{currentLanguage === "fr" ? "Contexte IA & Modèle" : "AI Context & Model"}</span>
          <Cpu size={14} style="color: #0071e3;" />
        </div>
        <div class="metric-card-value">
          {formatNumber(configDraft.totalTokenCeiling)} <span style="font-size: 13px; color: #86868b; font-weight: 500;">tok</span>
        </div>
        <div class="metric-card-sub">
          {configDraft.contextMode === "auto" ? (currentLanguage === "fr" ? "Mode Auto-Adaptatif" : "Auto-Adaptive") : (currentLanguage === "fr" ? "Mode Spécifique Modèle" : "Model-Specific")} · 5 {currentLanguage === "fr" ? "partitions" : "partitions"}
        </div>
        <div style="margin-top: 6px; font-size: 11px; color: #0071e3; display: flex; align-items: center; gap: 5px; font-weight: 600;">
          <Sliders size={12} />
          <span>{currentLanguage === "fr" ? "Plafond dynamique configurable" : "Configurable dynamic ceiling"}</span>
        </div>
      </div>

      <!-- Card 4: RRF Hybrid Engine -->
      <div class="cognitive-metric-card">
        <div class="metric-card-top">
          <span>{currentLanguage === "fr" ? "Recherche Hybride RRF" : "RRF Hybrid Search"}</span>
          <Database size={14} style="color: #ff9500;" />
        </div>
        <div class="metric-card-value" style="font-size: 16px; display: flex; align-items: center; gap: 6px;">
          <span>FTS5 + Vecteurs</span>
        </div>
        <div class="metric-card-sub" style="display: flex; justify-content: space-between; align-items: center;">
          <span>Qdrant: {memoryIndex?.state === "active" ? (currentLanguage === "fr" ? "Actif" : "Active") : (currentLanguage === "fr" ? "Local" : "Local")} · Top-{configDraft.topK}</span>
          <button
            type="button"
            class="icon-button"
            title={currentLanguage === "fr" ? "Réindexer la mémoire" : "Reindex memory"}
            disabled={memoryIndexBusy}
            style="padding: 2px 6px; font-size: 10px; height: 20px;"
            on:click={reindexMemories}
          >
            <RefreshCw size={11} class={memoryIndexBusy ? "spin" : ""} />
          </button>
        </div>
        <div style="margin-top: 6px; font-size: 11px; color: #ff9500; display: flex; align-items: center; gap: 5px; font-weight: 600;">
          <Sparkles size={12} />
          <span>{currentLanguage === "fr" ? `Poids FTS ${(configDraft.rrfFtsWeight * 100).toFixed(0)}% / Vec ${(configDraft.rrfVecWeight * 100).toFixed(0)}%` : `Weights FTS ${(configDraft.rrfFtsWeight * 100).toFixed(0)}% / Vec ${(configDraft.rrfVecWeight * 100).toFixed(0)}%`}</span>
        </div>
      </div>
    </div>
  </div>

  <!-- Subtabs Navigation -->
  <div style="display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap; margin-top: 4px;">
    <div class="memory-subtabs-nav">
      <button
        type="button"
        class="memory-subtab-btn"
        class:active={activeSubTab === "config"}
        on:click={() => (activeSubTab = "config")}
      >
        <Sliders size={14} />
        <span>{currentLanguage === "fr" ? "Paramètres & Moteur" : "Parameters & Engine"}</span>
        <span style="font-size: 10px; font-weight: 700; color: #0071e3; background: rgba(0,113,227,0.12); padding: 1px 6px; border-radius: 10px;">
          {formatNumber(configDraft.totalTokenCeiling)} tok
        </span>
      </button>

      <button
        type="button"
        class="memory-subtab-btn"
        class:active={activeSubTab === "semantic"}
        on:click={() => (activeSubTab = "semantic")}
      >
        <Brain size={14} />
        <span>{currentLanguage === "fr" ? "Faits Sémantiques (Long Terme)" : "Semantic Facts (Long-Term)"}</span>
        <span style="font-size: 10px; opacity: 0.8; background: rgba(0,0,0,0.06); padding: 1px 6px; border-radius: 10px;">{memoriesList.length}</span>
      </button>

      <button
        type="button"
        class="memory-subtab-btn"
        class:active={activeSubTab === "episodic"}
        on:click={() => (activeSubTab = "episodic")}
      >
        <BookOpen size={14} />
        <span>{currentLanguage === "fr" ? "Épisodes & Sessions (Moyen Terme)" : "Episodes & Sessions (Medium-Term)"}</span>
        <span style="font-size: 10px; opacity: 0.8; background: rgba(0,0,0,0.06); padding: 1px 6px; border-radius: 10px;">{episodesList.length}</span>
      </button>

      <button
        type="button"
        class="memory-subtab-btn"
        class:active={activeSubTab === "architecture"}
        on:click={() => (activeSubTab = "architecture")}
      >
        <Layers size={14} />
        <span>{currentLanguage === "fr" ? "Diagnostics & Architecture" : "Diagnostics & Architecture"}</span>
      </button>
    </div>
  </div>

  <!-- ========================================================================= -->
  <!-- TAB 0: Configuration & Dynamic Token Window (Apple macOS Sequoia Style)  -->
  <!-- ========================================================================= -->
  {#if activeSubTab === "config"}
    <div style="display: flex; flex-direction: column; gap: 16px;">
      <!-- Section 1: Modèle & Plafond de Contexte -->
      <div class="apple-settings-section">
        <div class="apple-section-header">
          <div>
            <div class="apple-section-title">
              <Cpu size={16} style="color: #0071e3;" />
              <span>{currentLanguage === "fr" ? "Fenêtre de Contexte & Modèle IA" : "Context Window & AI Model"}</span>
            </div>
            <div class="apple-section-subtitle">
              {currentLanguage === "fr"
                ? "Adaptez dynamiquement le contexte de mémoire selon votre modèle (local ou cloud) de 4k à 1M tokens."
                : "Dynamically adapt memory context window to your model (local or cloud) from 4k to 1M tokens."}
            </div>
          </div>
          <button
            type="button"
            class="apple-btn secondary"
            style="font-size: 11px; padding: 4px 10px; height: 26px; display: flex; align-items: center; gap: 4px;"
            on:click={() => {
              rebalancePartitions(configDraft.totalTokenCeiling);
              handleSaveConfig();
            }}
          >
            <Wand2 size={12} style="color: #0071e3;" />
            <span>{currentLanguage === "fr" ? "Rééquilibrer les 5 partitions" : "Auto-rebalance 5 partitions"}</span>
          </button>
        </div>

        <!-- Preset Chips -->
        <div class="apple-preset-grid">
          <button
            type="button"
            class="apple-preset-chip"
            class:active={configDraft.contextMode === "auto"}
            on:click={() => applyContextPreset("auto")}
          >
            <span>⚡ Auto (Modèle actif)</span>
          </button>
          <button
            type="button"
            class="apple-preset-chip"
            class:active={configDraft.contextMode === "128k"}
            on:click={() => applyContextPreset("128k")}
          >
            <span>GPT-4o (128k)</span>
          </button>
          <button
            type="button"
            class="apple-preset-chip"
            class:active={configDraft.contextMode === "200k"}
            on:click={() => applyContextPreset("200k")}
          >
            <span>Claude 3.5 / Gemini (200k)</span>
          </button>
          <button
            type="button"
            class="apple-preset-chip"
            class:active={configDraft.contextMode === "32k"}
            on:click={() => applyContextPreset("32k")}
          >
            <span>Llama 3 / Mistral (32k)</span>
          </button>
          <button
            type="button"
            class="apple-preset-chip"
            class:active={configDraft.contextMode === "8k"}
            on:click={() => applyContextPreset("8k")}
          >
            <span>Local Standard (8k)</span>
          </button>
          <button
            type="button"
            class="apple-preset-chip"
            class:active={configDraft.contextMode === "4k"}
            on:click={() => applyContextPreset("4k")}
          >
            <span>Éco (4k)</span>
          </button>
          <button
            type="button"
            class="apple-preset-chip"
            class:active={configDraft.contextMode === "1m"}
            on:click={() => applyContextPreset("1m")}
          >
            <span>Gemini Pro (1M)</span>
          </button>
          <button
            type="button"
            class="apple-preset-chip"
            class:active={configDraft.contextMode === "custom"}
            on:click={() => {
              configDraft.contextMode = "custom";
              handleSaveConfig();
            }}
          >
            <span>Personnalisé</span>
          </button>
        </div>

        <!-- Ceiling Slider Row -->
        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Plafond Total de Tokens" : "Total Token Ceiling"}</h4>
            <p class="apple-row-desc">
              {currentLanguage === "fr"
                ? "Taille totale allouée pour le prompt et la mémoire dans chaque tour de dialogue."
                : "Total prompt and memory size permitted per turn."}
            </p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="2048"
                max="1000000"
                step="1024"
                bind:value={configDraft.totalTokenCeiling}
                on:change={() => {
                  configDraft.contextMode = "custom";
                  rebalancePartitions(configDraft.totalTokenCeiling);
                  handleSaveConfig();
                }}
              />
              <span class="apple-value-pill">{formatNumber(configDraft.totalTokenCeiling)} tok</span>
            </div>
          </div>
        </div>

        <!-- 5-Partition Visualizer Preview -->
        <div style="padding: 12px 18px 16px 18px; background: rgba(0,0,0,0.015); border-top: 1px solid rgba(0,0,0,0.03);">
          <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px; font-size: 11px; font-weight: 600;">
            <span style="color: #1d1d1f;">{currentLanguage === "fr" ? "Répartition des 5 Compartiments Mémoire" : "5 Memory Partitions Distribution"}</span>
            <span style="color: #86868b;">{formatNumber(currentSum)} / {formatNumber(configDraft.totalTokenCeiling)} tok</span>
          </div>

          <div class="partition-bar-track" style="height: 14px; border-radius: 7px;">
            <div class="partition-segment" style="width: {systemPct}%; background: #0071e3;" title="Système ({configDraft.systemBudget} tok)"></div>
            <div class="partition-segment" style="width: {semanticPct}%; background: #af52de;" title="Sémantique ({configDraft.semanticBudget} tok)"></div>
            <div class="partition-segment" style="width: {episodicPct}%; background: #ff9500;" title="Épisodique ({configDraft.episodicBudget} tok)"></div>
            <div class="partition-segment" style="width: {workingPct}%; background: #34c759;" title="Travail ({configDraft.workingBudget} tok)"></div>
            <div class="partition-segment" style="width: {reservePct}%; background: #86868b;" title="Réserve ({configDraft.reserveBudget} tok)"></div>
          </div>

          <div class="partition-legend-grid" style="margin-top: 10px;">
            <div class="partition-legend-item">
              <span class="partition-color-dot" style="background: #0071e3;"></span>
              <span>Système: {formatNumber(configDraft.systemBudget)} ({systemPct}%)</span>
            </div>
            <div class="partition-legend-item">
              <span class="partition-color-dot" style="background: #af52de;"></span>
              <span>Sémantique: {formatNumber(configDraft.semanticBudget)} ({semanticPct}%)</span>
            </div>
            <div class="partition-legend-item">
              <span class="partition-color-dot" style="background: #ff9500;"></span>
              <span>Épisodique: {formatNumber(configDraft.episodicBudget)} ({episodicPct}%)</span>
            </div>
            <div class="partition-legend-item">
              <span class="partition-color-dot" style="background: #34c759;"></span>
              <span>Travail: {formatNumber(configDraft.workingBudget)} ({workingPct}%)</span>
            </div>
            <div class="partition-legend-item">
              <span class="partition-color-dot" style="background: #86868b;"></span>
              <span>Réserve: {formatNumber(configDraft.reserveBudget)} ({reservePct}%)</span>
            </div>
          </div>
        </div>

        <!-- 5 Partitions Sliders Rows -->
        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">1. {currentLanguage === "fr" ? "Budget Prompt Système & Outils" : "System Prompt & Tools Budget"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Instructions de base, personnalité et signatures des outils." : "Base persona, system instructions, and tool definitions."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="400"
                max={Math.min(32000, Math.max(4000, Math.round(configDraft.totalTokenCeiling * 0.3)))}
                step="50"
                bind:value={configDraft.systemBudget}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{formatNumber(configDraft.systemBudget)} tok</span>
            </div>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">2. {currentLanguage === "fr" ? "Budget Faits Sémantiques (Long Terme)" : "Semantic Facts Budget (Long-Term)"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Faits mémorisés réinjectés par similarité cosinus et FTS5." : "Recalled long-term facts retrieved via hybrid search."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="600"
                max={Math.round(configDraft.totalTokenCeiling * 0.4)}
                step="100"
                bind:value={configDraft.semanticBudget}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{formatNumber(configDraft.semanticBudget)} tok</span>
            </div>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">3. {currentLanguage === "fr" ? "Budget Synthèses Épisodiques (Moyen Terme)" : "Episodic Rollups Budget (Medium-Term)"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Résumés de sessions passées, décisions clés et entités." : "Summaries of previous sessions, key decisions and entities."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="600"
                max={Math.round(configDraft.totalTokenCeiling * 0.4)}
                step="100"
                bind:value={configDraft.episodicBudget}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{formatNumber(configDraft.episodicBudget)} tok</span>
            </div>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">4. {currentLanguage === "fr" ? "Budget Mémoire de Travail Active" : "Active Working Memory Buffer"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Messages bruts récents dans la conversation active." : "Raw recent chat turns in the active thread."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="800"
                max={Math.round(configDraft.totalTokenCeiling * 0.5)}
                step="100"
                bind:value={configDraft.workingBudget}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{formatNumber(configDraft.workingBudget)} tok</span>
            </div>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">5. {currentLanguage === "fr" ? "Marge de Réserve Dynamique" : "Dynamic Reserve Buffer"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Marge pour les tokens de réponse générés et la flexibilité du modèle." : "Margin for response completion tokens."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="400"
                max={Math.round(configDraft.totalTokenCeiling * 0.4)}
                step="100"
                bind:value={configDraft.reserveBudget}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{formatNumber(configDraft.reserveBudget)} tok</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Section 2: Compaction Continue -->
      <div class="apple-settings-section">
        <div class="apple-section-header">
          <div class="apple-section-title">
            <Activity size={16} style="color: #af52de;" />
            <span>{currentLanguage === "fr" ? "Consolidation Continue & Mémoire de Travail" : "Continuous Compaction & Working Memory"}</span>
          </div>
          <div class="apple-section-subtitle">
            {currentLanguage === "fr"
              ? "Prévient tout débordement de contexte en transformant automatiquement les tours anciens en rollups épisodiques."
              : "Prevents context overflow by automatically compacting older conversation turns into episodic rollups."}
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Intervalle de Compaction Continue" : "Compaction Interval"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Déclenche une synthèse automatique en arrière-plan tous les N tours de dialogue." : "Triggers background episodic rollup every N dialog turns."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="5"
                max="30"
                step="1"
                bind:value={configDraft.compactionInterval}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{configDraft.compactionInterval} {currentLanguage === "fr" ? "tours" : "turns"}</span>
            </div>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Tours Récents Conservés Intacts" : "Preserved Working Turns"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Nombre de tours récents gardés textuellement dans le prompt sans compaction." : "Recent messages retained verbatim in prompt before compaction."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="2"
                max="20"
                step="1"
                bind:value={configDraft.maxWorkingTurns}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{configDraft.maxWorkingTurns} {currentLanguage === "fr" ? "tours" : "turns"}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Section 3: Rappel & Filtrage de Saillance -->
      <div class="apple-settings-section">
        <div class="apple-section-header">
          <div class="apple-section-title">
            <Sparkles size={16} style="color: #ff9500;" />
            <span>{currentLanguage === "fr" ? "Rappel Sémantique & Mémorisation" : "Semantic Recall & Memorization"}</span>
          </div>
          <div class="apple-section-subtitle">
            {currentLanguage === "fr"
              ? "Contrôle la sensibilité de détection des faits pertinents et le volume injecté par tour."
              : "Controls sensitivity of relevant facts retrieval and volume injected per turn."}
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Mémorisation Cognitive Automatique" : "Autonomous Cognitive Memorization"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Extrait et sauvegarde automatiquement les faits, préférences et décisions de chaque échange." : "Automatically extracts and persists facts, preferences, and decisions."}</p>
          </div>
          <div class="apple-row-control">
            <label class="apple-toggle">
              <input
                type="checkbox"
                bind:checked={configDraft.autoMemorize}
                on:change={handleSaveConfig}
              />
              <span class="apple-toggle-slider"></span>
            </label>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Top-K Souvenirs Référés" : "Top-K Recalled Memories"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Nombre maximal de souvenirs sémantiques injectés dans le contexte par tour." : "Maximum number of semantic memories injected per prompt."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="1"
                max="25"
                step="1"
                bind:value={configDraft.topK}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">Top {configDraft.topK}</span>
            </div>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Seuil Minimal de Saillance" : "Minimum Salience Threshold"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Les faits ayant un score d'importance inférieur à ce seuil ne seront pas injectés." : "Memories with salience below this threshold are filtered out."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="0.0"
                max="0.9"
                step="0.05"
                bind:value={configDraft.minSalienceThreshold}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{(configDraft.minSalienceThreshold * 100).toFixed(0)}%</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Section 4: Courbe d'Oubli d'Ebbinghaus -->
      <div class="apple-settings-section">
        <div class="apple-section-header">
          <div class="apple-section-title">
            <Clock size={16} style="color: #34c759;" />
            <span>{currentLanguage === "fr" ? "Rétention & Décroissance d'Ebbinghaus" : "Retention & Ebbinghaus Decay"}</span>
          </div>
          <div class="apple-section-subtitle">
            {currentLanguage === "fr"
              ? "Modélise l'utilité temporelle selon R = exp(-t / S). Les faits épinglés restent 100% permanents."
              : "Models temporal utility R = exp(-t / S). Pinned memories remain 100% permanent."}
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Demi-vie de Décroissance" : "Decay Half-Life"}</h4>
            <p class="apple-row-desc">
              {#if configDraft.decayHalfLifeDays === 0}
                {currentLanguage === "fr" ? "✨ Illimitée : aucun fait n'est jamais affaibli par le temps." : "✨ Unlimited: memories never fade over time."}
              {:else}
                {currentLanguage === "fr"
                  ? `Les souvenirs non réutilisés perdent 50% d'utilité après ${configDraft.decayHalfLifeDays} jours.`
                  : `Unused memories lose 50% utility after ${configDraft.decayHalfLifeDays} days.`}
              {/if}
            </p>
          </div>
          <div class="apple-row-control">
            <div style="display: flex; gap: 6px; flex-wrap: wrap;">
              {#each [{ val: 0, label: currentLanguage === "fr" ? "Illimitée (Jamais d'oubli)" : "Unlimited (Never fade)" }, { val: 7, label: "7j" }, { val: 30, label: "30j" }, { val: 90, label: "90j" }, { val: 365, label: "1 an" }] as opt}
                <button
                  type="button"
                  class="apple-preset-chip"
                  class:active={configDraft.decayHalfLifeDays === opt.val}
                  on:click={() => {
                    configDraft.decayHalfLifeDays = opt.val;
                    handleSaveConfig();
                  }}
                >
                  <span>{opt.label}</span>
                </button>
              {/each}
            </div>
          </div>
        </div>
      </div>

      <!-- Section 5: Moteur Hybride RRF -->
      <div class="apple-settings-section">
        <div class="apple-section-header">
          <div class="apple-section-title">
            <Database size={16} style="color: #0071e3;" />
            <span>{currentLanguage === "fr" ? "Consensus Hybride RRF (Reciprocal Rank Fusion)" : "RRF Hybrid Search Consensus"}</span>
          </div>
          <div class="apple-section-subtitle">
            {currentLanguage === "fr"
              ? "Combine la recherche lexicale SQLite FTS5 et la recherche vectorielle sémantique Qdrant."
              : "Blends SQLite FTS5 lexical BM25 matching and Qdrant semantic vector similarity."}
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Poids Recherche Lexicale FTS5" : "FTS5 Lexical Search Weight"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Priorise la correspondance exacte de mots-clés, noms propres et termes techniques." : "Favors exact keyword matches and identifiers."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="0.0"
                max="1.0"
                step="0.05"
                bind:value={configDraft.rrfFtsWeight}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{configDraft.rrfFtsWeight.toFixed(2)}</span>
            </div>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Poids Recherche Vectorielle Qdrant" : "Vector Search Weight"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Priorise la proximité sémantique et conceptuelle multilingue." : "Favors conceptual and semantic embedding similarity."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="0.0"
                max="1.0"
                step="0.05"
                bind:value={configDraft.rrfVecWeight}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">{configDraft.rrfVecWeight.toFixed(2)}</span>
            </div>
          </div>
        </div>

        <div class="apple-settings-row">
          <div class="apple-row-label-group">
            <h4 class="apple-row-title">{currentLanguage === "fr" ? "Facteur d'Amortissement RRF (k)" : "RRF Smoothing Factor (k)"}</h4>
            <p class="apple-row-desc">{currentLanguage === "fr" ? "Constante k dans score = sum(w / (k + rank)). Standard industriel : 60." : "Constant k in score = sum(w / (k + rank)). Industry standard: 60."}</p>
          </div>
          <div class="apple-row-control">
            <div class="apple-slider-wrap">
              <input
                type="range"
                class="apple-slider"
                min="10"
                max="100"
                step="5"
                bind:value={configDraft.rrfK}
                on:change={handleSaveConfig}
              />
              <span class="apple-value-pill">k = {configDraft.rrfK}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer Buttons & Feedback -->
      <div style="display: flex; justify-content: space-between; align-items: center; padding: 8px 0 24px 0; flex-wrap: wrap; gap: 12px;">
        <button
          type="button"
          class="apple-btn secondary"
          style="font-size: 12px; padding: 7px 14px; display: flex; align-items: center; gap: 6px;"
          on:click={handleResetConfig}
        >
          <RotateCcw size={13} />
          <span>{currentLanguage === "fr" ? "Rétablir la configuration recommandée Apple" : "Restore Apple Recommended Defaults"}</span>
        </button>

        <div style="display: flex; align-items: center; gap: 12px;">
          {#if saveSuccess}
            <span style="font-size: 12px; color: #34c759; display: flex; align-items: center; gap: 5px; font-weight: 600;">
              <Check size={14} />
              <span>{currentLanguage === "fr" ? "Configuration synchronisée" : "Configuration synced"}</span>
            </span>
          {/if}

          <button
            type="button"
            class="apple-btn primary"
            style="font-size: 12px; padding: 7px 18px; display: flex; align-items: center; gap: 6px;"
            disabled={isSaving}
            on:click={handleSaveConfig}
          >
            {#if isSaving}
              <RefreshCw size={13} class="spin" />
              <span>{currentLanguage === "fr" ? "Enregistrement..." : "Saving..."}</span>
            {:else}
              <Check size={13} />
              <span>{currentLanguage === "fr" ? "Appliquer les Réglages" : "Apply Settings"}</span>
            {/if}
          </button>
        </div>
      </div>
    </div>

  <!-- ========================================================================= -->
  <!-- TAB 1: Semantic Long-Term Memories -->
  <!-- ========================================================================= -->
  {:else if activeSubTab === "semantic"}
    <!-- Search and Controls Row -->
    <div class="memory-controls-row" style="display: flex; gap: 12px; align-items: center; flex-wrap: wrap;">
      <div class="search-input-wrapper" style="flex: 1; min-width: 220px;">
        <Search size={14} class="search-icon" />
        <input
          type="text"
          class="settings-input search-input"
          placeholder={t.memorySearchPlaceholder}
          bind:value={memorySearchQuery}
        />
        {#if memorySearchQuery}
          <button
            type="button"
            class="icon-button"
            style="position: absolute; right: 8px; top: 50%; transform: translateY(-50%); padding: 4px;"
            on:click={() => (memorySearchQuery = "")}
          >
            ×
          </button>
        {/if}
      </div>

      <!-- Quick filters: All, Pinned, High Salience, Frequent -->
      <div style="display: flex; gap: 6px; flex-wrap: wrap;">
        <button
          type="button"
          class="filter-chip"
          class:active={activeQuickFilter === "all"}
          on:click={() => (activeQuickFilter = "all")}
        >
          {currentLanguage === "fr" ? "Tous les faits" : "All facts"}
        </button>
        <button
          type="button"
          class="filter-chip"
          class:active={activeQuickFilter === "pinned"}
          on:click={() => (activeQuickFilter = "pinned")}
        >
          <Pin size={10} style="display: inline; vertical-align: middle; margin-right: 3px;" />
          {currentLanguage === "fr" ? `Épinglés (${pinnedCount})` : `Pinned (${pinnedCount})`}
        </button>
        <button
          type="button"
          class="filter-chip"
          class:active={activeQuickFilter === "high_salience"}
          on:click={() => (activeQuickFilter = "high_salience")}
        >
          <Sparkles size={10} style="display: inline; vertical-align: middle; margin-right: 3px;" />
          {currentLanguage === "fr" ? "Haute saillance" : "High salience"}
        </button>
        <button
          type="button"
          class="filter-chip"
          class:active={activeQuickFilter === "frequent"}
          on:click={() => (activeQuickFilter = "frequent")}
        >
          <Zap size={10} style="display: inline; vertical-align: middle; margin-right: 3px;" />
          {currentLanguage === "fr" ? "Fréquents" : "Frequently recalled"}
        </button>
      </div>
    </div>

    <!-- Category Filter Chips -->
    <div class="filter-chips" style="display: flex; gap: 8px; overflow-x: auto; padding-bottom: 2px;">
      <button
        type="button"
        class="filter-chip"
        class:active={selectedMemoryFilter === "all"}
        on:click={() => (selectedMemoryFilter = "all")}
      >
        {currentLanguage === "fr" ? "Toutes catégories" : "All categories"}
      </button>
      <button
        type="button"
        class="filter-chip"
        class:active={selectedMemoryFilter === "personal"}
        on:click={() => (selectedMemoryFilter = "personal")}
      >
        {t.catPersonal}
      </button>
      <button
        type="button"
        class="filter-chip"
        class:active={selectedMemoryFilter === "technical"}
        on:click={() => (selectedMemoryFilter = "technical")}
      >
        {t.catTechnical}
      </button>
      <button
        type="button"
        class="filter-chip"
        class:active={selectedMemoryFilter === "system"}
        on:click={() => (selectedMemoryFilter = "system")}
      >
        {t.catSystem}
      </button>
      <button
        type="button"
        class="filter-chip"
        class:active={selectedMemoryFilter === "preference"}
        on:click={() => (selectedMemoryFilter = "preference")}
      >
        {t.catPreference}
      </button>
    </div>

    <!-- Inline Add Memory Card -->
    {#if showAddMemoryInline}
      <div class="memory-card-modern" style="border: 1.5px solid #0071e3; background: rgba(0, 113, 227, 0.03);">
        <div style="display: flex; justify-content: space-between; align-items: center;">
          <h3 style="margin: 0; font-size: 13px; font-weight: 700; color: #0071e3; display: flex; align-items: center; gap: 6px;">
            <Brain size={15} />
            <span>{currentLanguage === "fr" ? "Mémoriser un nouveau fait permanent" : "Remember New Permanent Fact"}</span>
          </h3>
          <button
            type="button"
            class="icon-button"
            style="padding: 2px;"
            on:click={() => {
              showAddMemoryInline = false;
              newMemoryText = "";
            }}
          >
            ×
          </button>
        </div>

        <textarea
          class="settings-textarea"
          rows="3"
          placeholder={t.newMemoryPlaceholder}
          bind:value={newMemoryText}
          style="min-height: 60px; font-size: 13px; padding: 10px 12px;"
        ></textarea>

        <div style="display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap;">
          <div style="display: flex; align-items: center; gap: 14px; flex-wrap: wrap;">
            <!-- Category Selection -->
            <div style="display: flex; align-items: center; gap: 6px;">
              <span style="font-size: 11px; color: #86868b; font-weight: 500;">{t.newMemoryCategoryLabel} :</span>
              <select class="role-select" bind:value={newMemoryCategory} style="font-size: 11px; padding: 4px 8px;">
                <option value="personal">{t.catPersonal}</option>
                <option value="technical">{t.catTechnical}</option>
                <option value="system">{t.catSystem}</option>
                <option value="preference">{t.catPreference}</option>
              </select>
            </div>

            <!-- Salience Slider -->
            <div style="display: flex; align-items: center; gap: 8px;">
              <span style="font-size: 11px; color: #86868b; font-weight: 500;">
                {currentLanguage === "fr" ? "Saillance :" : "Salience :"}
              </span>
              <input
                type="range"
                min="0.1"
                max="1.0"
                step="0.05"
                bind:value={newMemorySalience}
                style="width: 80px; accent-color: #0071e3; cursor: pointer;"
              />
              <span class="salience-gauge" style="color: {getSalienceLabel(newMemorySalience).tone};">
                {(newMemorySalience * 100).toFixed(0)}% ({getSalienceLabel(newMemorySalience).label})
              </span>
            </div>

            <!-- Pinned Checkbox -->
            <label style="display: flex; align-items: center; gap: 6px; font-size: 11px; cursor: pointer; color: #86868b;">
              <input type="checkbox" bind:checked={newMemoryPinned} style="accent-color: #ff9500;" />
              <span>{currentLanguage === "fr" ? "Épingler immédiatement" : "Pin immediately"}</span>
            </label>
          </div>

          <div style="display: flex; gap: 8px;">
            <button
              type="button"
              class="apple-btn secondary"
              style="font-size: 11px; padding: 6px 12px; height: 28px;"
              on:click={() => {
                showAddMemoryInline = false;
                newMemoryText = "";
              }}
            >
              {t.cancelBtn}
            </button>
            <button
              type="button"
              class="apple-btn primary"
              style="font-size: 11px; padding: 6px 14px; height: 28px;"
              disabled={!newMemoryText.trim()}
              on:click={async () => {
                await addMemory();
                showAddMemoryInline = false;
              }}
            >
              {t.saveBtn}
            </button>
          </div>
        </div>
      </div>
    {/if}

    <!-- Memories Grid -->
    <div style="display: flex; flex-direction: column; gap: 10px;">
      {#each filteredMemories as memory (memory.id)}
        <div class="memory-card-modern" class:pinned-active={memory.pinned}>
          <div class="memory-card-top-row">
            <div class="memory-card-tags-left">
              <span class="memory-tag {memory.category}">
                {#if memory.category === "personal"}{t.catPersonal}
                {:else if memory.category === "technical"}{t.catTechnical}
                {:else if memory.category === "system"}{t.catSystem}
                {:else}{t.catPreference}{/if}
              </span>

              {#if memory.pinned}
                <span class="pinned-indicator">
                  <Pin size={10} />
                  <span>{currentLanguage === "fr" ? "Épinglé" : "Pinned"}</span>
                </span>
              {/if}

              <span class="salience-gauge" title={currentLanguage === "fr" ? "Score de saillance cognitive" : "Cognitive salience score"}>
                <Sparkles size={10} style="color: {getSalienceLabel(memory.salience || 0.5).tone};" />
                <span>{((memory.salience || 0.5) * 100).toFixed(0)}% {getSalienceLabel(memory.salience || 0.5).label}</span>
              </span>

              {#if (memory.recallCount || 0) > 0}
                <span class="recall-count-badge" title={currentLanguage === "fr" ? "Nombre de fois rappelé par l'IA" : "Number of times recalled by AI"}>
                  <Zap size={10} />
                  <span>{memory.recallCount} {currentLanguage === "fr" ? "rappels" : "recalls"}</span>
                </span>
              {/if}

              <span style="font-size: 11px; color: #86868b; display: flex; align-items: center; gap: 4px; margin-left: 4px;">
                <Clock size={11} />
                <span>{formatRelativeDate(memory.createdAt)}</span>
              </span>
            </div>

            <!-- Card Actions -->
            <div style="display: flex; align-items: center; gap: 4px;">
              <button
                type="button"
                class="member-delete-btn"
                style="padding: 4px 6px; font-size: 11px; display: flex; align-items: center; gap: 4px;"
                title={memory.pinned ? (currentLanguage === "fr" ? "Désépingler" : "Unpin") : (currentLanguage === "fr" ? "Épingler" : "Pin")}
                on:click={() => handleTogglePin(memory)}
              >
                {#if memory.pinned}
                  <PinOff size={12} style="color: #ff9500;" />
                {:else}
                  <Pin size={12} />
                {/if}
              </button>

              <button
                type="button"
                class="member-delete-btn"
                style="padding: 4px 6px;"
                title={currentLanguage === "fr" ? "Copier le texte" : "Copy text"}
                on:click={() => handleCopy(memory.id, memory.content)}
              >
                {#if copiedId === memory.id}
                  <Check size={12} style="color: #34c759;" />
                {:else}
                  <Copy size={12} />
                {/if}
              </button>

              {#if editingMemoryId !== memory.id}
                <button
                  type="button"
                  class="member-delete-btn"
                  style="padding: 4px 6px;"
                  title={currentLanguage === "fr" ? "Modifier ce fait" : "Edit fact"}
                  on:click={() => {
                    editingMemoryId = memory.id;
                    editingMemoryText = memory.content;
                  }}
                >
                  <Edit2 size={12} />
                </button>
                <button
                  type="button"
                  class="member-delete-btn"
                  style="padding: 4px 6px;"
                  title={currentLanguage === "fr" ? "Oublier ce fait" : "Forget fact"}
                  on:click={() => deleteMemory(memory)}
                >
                  <Trash2 size={12} />
                </button>
              {/if}
            </div>
          </div>

          <!-- Content or Inline Editor -->
          {#if editingMemoryId === memory.id}
            <div style="width: 100%; margin-top: 4px;">
              <textarea
                class="settings-textarea"
                rows="2"
                bind:value={editingMemoryText}
                style="min-height: 48px; font-size: 13px; padding: 8px 10px; width: 100%;"
              ></textarea>
              <div style="display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px;">
                <button
                  type="button"
                  class="apple-btn secondary"
                  style="font-size: 11px; padding: 4px 10px; height: 26px;"
                  on:click={() => (editingMemoryId = null)}
                >
                  {t.cancelBtn}
                </button>
                <button
                  type="button"
                  class="apple-btn primary"
                  style="font-size: 11px; padding: 4px 12px; height: 26px;"
                  disabled={!editingMemoryText.trim()}
                  on:click={async () => {
                    await updateMemory({
                      ...memory,
                      content: editingMemoryText.trim(),
                    });
                    editingMemoryId = null;
                  }}
                >
                  {t.saveBtn}
                </button>
              </div>
            </div>
          {:else}
            <p style="margin: 0; font-size: 13px; line-height: 1.5; color: {currentTheme === 'dark' ? '#f5f5f7' : '#1d1d1f'}; word-break: break-word;">
              {memory.content}
            </p>
          {/if}
        </div>
      {:else}
        <div class="empty-memory-state" style="text-align: center; padding: 48px 24px; border-radius: 14px; background: rgba(0,0,0,0.01); border: 1.5px dashed rgba(0,0,0,0.08);">
          <Brain size={32} style="color: #86868b; margin-bottom: 12px; display: inline-block;" />
          <h4 style="margin: 0 0 6px 0; font-size: 14px; font-weight: 600; color: #1d1d1f;">
            {t.noMemoriesFound}
          </h4>
          <p style="margin: 0; font-size: 12px; color: #86868b; max-width: 380px; display: inline-block;">
            {currentLanguage === "fr"
              ? "Ajoutez des informations clés que vous souhaitez qu'ARO retienne toujours au fil des conversations."
              : "Add key information you want ARO to always remember across conversations."}
          </p>
          <div style="margin-top: 16px;">
            <button
              type="button"
              class="apple-btn primary"
              style="font-size: 12px; padding: 6px 14px;"
              on:click={() => (showAddMemoryInline = true)}
            >
              <Plus size={13} style="margin-right: 4px; display: inline; vertical-align: middle;" />
              <span>{t.addMemoryBtn}</span>
            </button>
          </div>
        </div>
      {/each}
    </div>

  <!-- ========================================================================= -->
  <!-- TAB 2: Episodic Rollups (Medium-Term) -->
  <!-- ========================================================================= -->
  {:else if activeSubTab === "episodic"}
    <div style="display: flex; flex-direction: column; gap: 14px;">
      <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 10px;">
        <div>
          <h3 style="margin: 0; font-size: 15px; font-weight: 700;">
            {currentLanguage === "fr" ? "Synthèses Épisodiques de Sessions" : "Episodic Session Rollups"}
          </h3>
          <p style="margin: 2px 0 0 0; font-size: 12px; color: #86868b;">
            {currentLanguage === "fr"
              ? "Le démon de compaction extrait et consolide automatiquement l'historique par tranches de 10 tours pour préserver un contexte sans perte."
              : "The continuous compaction daemon automatically consolidates history into 10-turn rollups to preserve lossless context."}
          </p>
        </div>
        <div style="display: flex; align-items: center; gap: 8px;">
          {#if onRefreshEpisodes}
            <button
              type="button"
              class="apple-btn secondary"
              style="padding: 4px 10px; font-size: 11px; height: 28px;"
              on:click={() => onRefreshEpisodes?.()}
              title={currentLanguage === "fr" ? "Actualiser les synthèses" : "Refresh rollups"}
            >
              <RefreshCw size={12} />
              <span>{currentLanguage === "fr" ? "Actualiser" : "Refresh"}</span>
            </button>
          {/if}
          <span class="salience-gauge" style="padding: 4px 10px; font-size: 11px;">
            <Layers size={12} />
            <span>{episodesList.length} {currentLanguage === "fr" ? "rollups consolidés" : "consolidated rollups"}</span>
          </span>
        </div>
      </div>

      {#if episodesList.length > 0}
        <div style="display: flex; flex-direction: column; gap: 12px;">
          {#each episodesList as episode (episode.id)}
            <div class="episode-timeline-card">
              <div class="episode-header-row">
                <div style="display: flex; align-items: center; gap: 8px;">
                  <span class="episode-turns-badge">
                    <Clock size={12} />
                    <span>{currentLanguage === "fr" ? `Tours ${episode.turnStart} → ${episode.turnEnd}` : `Turns ${episode.turnStart} → ${episode.turnEnd}`}</span>
                  </span>
                  <span style="font-size: 11px; color: #86868b;">
                    {formatRelativeDate(episode.createdAt)}
                  </span>
                </div>
                <span style="font-size: 11px; color: #86868b; font-weight: 600;">
                  ~{episode.tokenCount || 0} tokens
                </span>
              </div>

              <!-- Summary text -->
              <p style="margin: 0; font-size: 13px; line-height: 1.5; color: {currentTheme === 'dark' ? '#f5f5f7' : '#1d1d1f'};">
                {episode.summary}
              </p>

              <!-- Decisions & Entities -->
              {#if (episode.keyDecisions && episode.keyDecisions.length > 0) || (episode.entities && episode.entities.length > 0)}
                <div style="display: flex; flex-direction: column; gap: 8px; border-top: 1px solid rgba(0,0,0,0.04); padding-top: 10px;">
                  {#if episode.keyDecisions && episode.keyDecisions.length > 0}
                    <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                      <span style="font-size: 11px; font-weight: 600; color: #34c759; display: flex; align-items: center; gap: 4px;">
                        <Check size={11} />
                        <span>{currentLanguage === "fr" ? "Décisions clés :" : "Key decisions:"}</span>
                      </span>
                      <div class="episode-chips-wrap">
                        {#each episode.keyDecisions as decision}
                          <span class="decision-pill">{decision}</span>
                        {/each}
                      </div>
                    </div>
                  {/if}

                  {#if episode.entities && episode.entities.length > 0}
                    <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                      <span style="font-size: 11px; font-weight: 600; color: #af52de; display: flex; align-items: center; gap: 4px;">
                        <ShieldCheck size={11} />
                        <span>{currentLanguage === "fr" ? "Entités préservées :" : "Preserved entities:"}</span>
                      </span>
                      <div class="episode-chips-wrap">
                        {#each episode.entities as entity}
                          <span class="entity-pill">{entity}</span>
                        {/each}
                      </div>
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {:else}
        <div class="empty-memory-state" style="text-align: center; padding: 48px 24px; border-radius: 14px; background: rgba(0,0,0,0.01); border: 1.5px dashed rgba(0,0,0,0.08);">
          <BookOpen size={32} style="color: #86868b; margin-bottom: 12px; display: inline-block;" />
          <h4 style="margin: 0 0 6px 0; font-size: 14px; font-weight: 600; color: #1d1d1f;">
            {currentLanguage === "fr" ? "Aucune session n'a encore été consolidée" : "No sessions consolidated yet"}
          </h4>
          <p style="margin: 0; font-size: 12px; color: #86868b; max-width: 440px; display: inline-block; line-height: 1.5;">
            {currentLanguage === "fr"
              ? "Dès qu'une conversation atteint 10 tours, le compacteur automatique en arrière-plan génère un rollup chronologique qui apparaîtra ici."
              : "As soon as a conversation reaches 10 turns, the continuous background compactor automatically generates a chronological rollup that will appear here."}
          </p>
        </div>
      {/if}
    </div>

  <!-- ========================================================================= -->
  <!-- TAB 3: Cognitive Architecture & Health -->
  <!-- ========================================================================= -->
  {:else if activeSubTab === "architecture"}
    <div style="display: flex; flex-direction: column; gap: 16px;">
      <div>
        <h3 style="margin: 0; font-size: 15px; font-weight: 700;">
          {currentLanguage === "fr" ? "Architecture Cognitive & Budget de Contexte" : "Cognitive Architecture & Context Budget"}
        </h3>
        <p style="margin: 2px 0 0 0; font-size: 12px; color: #86868b;">
          {currentLanguage === "fr"
            ? `Plafond actif de ${formatNumber(configDraft.totalTokenCeiling)} tokens garantissant une allocation stricte sans dépassement de fenêtre.`
            : `Active ceiling of ${formatNumber(configDraft.totalTokenCeiling)} tokens guaranteeing strict allocation without overflow.`}
        </p>
      </div>

      <!-- 5-Partition Token Visualizer (Dynamic) -->
      <div class="partition-visualizer">
        <div style="display: flex; justify-content: space-between; align-items: center; font-size: 12px; font-weight: 600;">
          <span>{currentLanguage === "fr" ? `Budget Total du Prompt : ${formatNumber(configDraft.totalTokenCeiling)} Tokens` : `Total Prompt Budget: ${formatNumber(configDraft.totalTokenCeiling)} Tokens`}</span>
          <span style="color: #34c759;">100% {currentLanguage === "fr" ? "Partitionné" : "Partitioned"}</span>
        </div>

        <div class="partition-bar-track" style="height: 14px; border-radius: 7px;">
          <div class="partition-segment" style="width: {systemPct}%; background: #0071e3;" title="System ({formatNumber(configDraft.systemBudget)} tok)"></div>
          <div class="partition-segment" style="width: {semanticPct}%; background: #af52de;" title="Semantic ({formatNumber(configDraft.semanticBudget)} tok)"></div>
          <div class="partition-segment" style="width: {episodicPct}%; background: #ff9500;" title="Episodic ({formatNumber(configDraft.episodicBudget)} tok)"></div>
          <div class="partition-segment" style="width: {workingPct}%; background: #34c759;" title="Working Buffer ({formatNumber(configDraft.workingBudget)} tok)"></div>
          <div class="partition-segment" style="width: {reservePct}%; background: #86868b;" title="Reserve ({formatNumber(configDraft.reserveBudget)} tok)"></div>
        </div>

        <div class="partition-legend-grid">
          <div class="partition-legend-item">
            <span class="partition-color-dot" style="background: #0071e3;"></span>
            <span>Système ({formatNumber(configDraft.systemBudget)} max)</span>
          </div>
          <div class="partition-legend-item">
            <span class="partition-color-dot" style="background: #af52de;"></span>
            <span>Sémantique ({formatNumber(configDraft.semanticBudget)} max)</span>
          </div>
          <div class="partition-legend-item">
            <span class="partition-color-dot" style="background: #ff9500;"></span>
            <span>Épisodique ({formatNumber(configDraft.episodicBudget)} max)</span>
          </div>
          <div class="partition-legend-item">
            <span class="partition-color-dot" style="background: #34c759;"></span>
            <span>Mémoire Travail ({formatNumber(configDraft.workingBudget)} max)</span>
          </div>
          <div class="partition-legend-item">
            <span class="partition-color-dot" style="background: #86868b;"></span>
            <span>Réserve Dynamique ({formatNumber(configDraft.reserveBudget)})</span>
          </div>
        </div>
      </div>

      <!-- Theoretical Models & Foundations Card -->
      <div class="memory-card-modern" style="gap: 12px;">
        <h4 style="margin: 0; font-size: 13px; font-weight: 700; display: flex; align-items: center; gap: 8px;">
          <Cpu size={15} style="color: #0071e3;" />
          <span>{currentLanguage === "fr" ? "Fondations Algorithmiques AGI" : "AGI Algorithmic Foundations"}</span>
        </h4>
        <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 14px; font-size: 12px; line-height: 1.5; color: #86868b;">
          <div style="background: rgba(0,0,0,0.02); padding: 12px; border-radius: 10px;">
            <strong style="color: #1d1d1f; display: block; margin-bottom: 4px;">
              📉 {currentLanguage === "fr" ? "Décroissance d'Ebbinghaus" : "Ebbinghaus Forgetting Curve"}
            </strong>
            <code>R = e^(-t / S)</code>
            <p style="margin: 4px 0 0 0;">
              {currentLanguage === "fr"
                ? `Rétention temporelle configurée : ${configDraft.decayHalfLifeDays === 0 ? "Illimitée (aucun affaiblissement)" : `${configDraft.decayHalfLifeDays} jours de demi-vie`}.`
                : `Configured retention: ${configDraft.decayHalfLifeDays === 0 ? "Unlimited (never fades)" : `${configDraft.decayHalfLifeDays} days half-life`}.`}
            </p>
          </div>

          <div style="background: rgba(0,0,0,0.02); padding: 12px; border-radius: 10px;">
            <strong style="color: #1d1d1f; display: block; margin-bottom: 4px;">
              ⚡ {currentLanguage === "fr" ? "Utilité Dynamique U(m, q)" : "Dynamic Utility U(m, q)"}
            </strong>
            <code>U = wr·R + ws·S + wu·log(1 + N_recall)</code>
            <p style="margin: 4px 0 0 0;">
              {currentLanguage === "fr"
                ? `Chaque rappel atomique (Top-${configDraft.topK}) augmente le score d'utilité du souvenir et renforce sa mobilisation.`
                : `Each atomic recall (Top-${configDraft.topK}) increases utility score and reinforces future activation.`}
            </p>
          </div>

          <div style="background: rgba(0,0,0,0.02); padding: 12px; border-radius: 10px;">
            <strong style="color: #1d1d1f; display: block; margin-bottom: 4px;">
              🔍 {currentLanguage === "fr" ? `Fusion Hybride RRF (k=${configDraft.rrfK})` : `RRF Hybrid Fusion (k=${configDraft.rrfK})`}
            </strong>
            <code>RRF = {configDraft.rrfFtsWeight.toFixed(2)}·FTS5 + {configDraft.rrfVecWeight.toFixed(2)}·Vector</code>
            <p style="margin: 4px 0 0 0;">
              {currentLanguage === "fr"
                ? "Combine la précision lexicale BM25 de SQLite et la similarité conceptuelle des vecteurs Qdrant pour un consensus de rappel optimal."
                : "Combines SQLite BM25 lexical precision with Qdrant vector semantic similarity for optimal recall consensus."}
            </p>
          </div>
        </div>
      </div>

      <!-- Storage & Index Engine Card -->
      <div class="settings-group" style="padding: 16px;">
        <div style="display: flex; justify-content: space-between; align-items: center; gap: 12px; flex-wrap: wrap;">
          <div style="display: flex; align-items: center; gap: 10px;">
            <Database size={20} style="color: {memoryIndex?.state === 'active' ? '#34c759' : '#0071e3'};" />
            <div>
              <div style="font-size: 13px; font-weight: 600;">
                SQLite WAL (Write-Ahead Logging) & Qdrant Engine
              </div>
              <div style="font-size: 11px; color: #86868b; margin-top: 2px;">
                {currentLanguage === "fr"
                  ? "Concurrence immédiate, tables virtuelles FTS5 et transactions ACID sans verrouillage"
                  : "Immediate concurrency, FTS5 virtual tables, and lock-free ACID transactions"}
              </div>
            </div>
          </div>
          <div style="display: flex; gap: 8px;">
            <button
              type="button"
              class="apple-btn secondary"
              style="font-size: 11px; padding: 6px 12px; height: 30px;"
              disabled={memoryIndexBusy}
              on:click={refreshMemoryIndexStatus}
            >
              <RefreshCw size={12} class={memoryIndexBusy ? "spin" : ""} style="margin-right: 4px; display: inline;" />
              <span>{currentLanguage === "fr" ? "Vérifier le statut" : "Check status"}</span>
            </button>
            <button
              type="button"
              class="apple-btn primary"
              style="font-size: 11px; padding: 6px 14px; height: 30px;"
              disabled={memoryIndexBusy}
              on:click={reindexMemories}
            >
              <span>{memoryIndexBusy ? (currentLanguage === "fr" ? "Indexation..." : "Indexing...") : (currentLanguage === "fr" ? "Réindexer l'index vectoriel" : "Reindex Vector Engine")}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>
