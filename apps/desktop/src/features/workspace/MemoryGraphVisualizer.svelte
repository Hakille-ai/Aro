<script lang="ts">
  import Database from "@lucide/svelte/icons/database";
  import Search from "@lucide/svelte/icons/search";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import { onMount } from "svelte";

  interface MemoryNode {
    id: string;
    label: string;
    category: string;
    x: number;
    y: number;
    color: string;
  }

  interface MemoryGraphItem {
    id?: string;
    title?: string;
    content?: string;
    category?: string;
  }

  export let memoryItems: MemoryGraphItem[] = [];
  export let language: "fr" | "en" = "fr";
  export let loading = false;

  let searchQuery = "";
  let selectedNode: MemoryNode | null = null;

  $: nodes = memoryItems.map((item, index) => {
    const angle = (index / Math.max(1, memoryItems.length)) * 2 * Math.PI;
    const radius = 120 + (index % 3) * 30;
    return {
      id: item.id || `node-${index}`,
      label: item.title || item.content || `Fact ${index + 1}`,
      category: item.category || "memory",
      x: 250 + Math.cos(angle) * radius,
      y: 200 + Math.sin(angle) * radius,
      color: item.category === "preference" ? "#a855f7" : item.category === "tech" ? "#3b82f6" : "#10b981",
    };
  });

  $: filteredNodes = nodes.filter((n) => !searchQuery.trim() || n.label.toLowerCase().includes(searchQuery.toLowerCase()));
</script>

<div class="memory-graph-container glassmorphic-panel">
  <div class="graph-header">
    <div class="graph-title">
      <Database size={16} class="graph-icon" />
      <span>{language === "fr" ? "Graphe de Mémoire Vectorielle" : "Vector Memory Graph"}</span>
    </div>

    <div class="graph-search">
      <Search size={13} class="search-icon" />
      <input
        type="text"
        bind:value={searchQuery}
        placeholder={language === "fr" ? "Rechercher un concept..." : "Search concept..."}
      />
    </div>
  </div>

  <div class="canvas-wrapper">
    {#if loading}
      <div class="graph-state">
        <RefreshCw size={20} class="spinning" />
        <span>{language === "fr" ? "Chargement de la mémoire…" : "Loading memory…"}</span>
      </div>
    {:else if memoryItems.length === 0}
      <div class="graph-state">
        <Database size={22} />
        <span>{language === "fr" ? "Aucune mémoire enregistrée pour l'instant." : "No memories stored yet."}</span>
      </div>
    {:else if filteredNodes.length === 0}
      <div class="graph-state">
        <Search size={20} />
        <span>{language === "fr" ? "Aucun concept correspondant." : "No matching concept."}</span>
      </div>
    {/if}
    <svg class="graph-svg" viewBox="0 0 500 400">
      <!-- Connection Lines to Center -->
      {#each filteredNodes as node}
        <line x1="250" y1="200" x2={node.x} y2={node.y} stroke="rgba(255, 255, 255, 0.08)" stroke-width="1.5" />
      {/each}

      <!-- Central Node -->
      <circle cx="250" cy="200" r="18" fill="#3b82f6" opacity="0.9" />
      <text x="250" y="204" text-anchor="middle" fill="#ffffff" font-size="10" font-weight="bold">ARO</text>

      <!-- Memory Nodes -->
      {#each filteredNodes as node}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <g class="node-group" on:click={() => (selectedNode = node)}>
          <circle cx={node.x} cy={node.y} r="12" fill={node.color} opacity="0.85" class="node-circle" />
          <text x={node.x} y={node.y + 24} text-anchor="middle" fill="#cbd5e1" font-size="9" class="node-label">
            {node.label.length > 18 ? node.label.slice(0, 15) + "..." : node.label}
          </text>
        </g>
      {/each}
    </svg>
  </div>

  {#if selectedNode}
    <div class="node-preview">
      <span class="preview-title">{selectedNode.label}</span>
      <span class="preview-cat" style="color: {selectedNode.color};">Catégorie: {selectedNode.category}</span>
    </div>
  {/if}
</div>

<style>
  .memory-graph-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-radius: 12px;
    background: rgba(10, 15, 30, 0.9);
    border: 1px solid rgba(255, 255, 255, 0.1);
    overflow: hidden;
  }

  .graph-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    background: rgba(255, 255, 255, 0.03);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .graph-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.82rem;
    font-weight: 600;
    color: #e2e8f0;
  }

  :global(.graph-icon) {
    color: #a855f7;
  }

  .graph-search {
    position: relative;
  }

  .graph-search :global(.search-icon) {
    position: absolute;
    left: 8px;
    top: 50%;
    transform: translateY(-50%);
    color: #64748b;
  }

  .graph-search input {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    padding: 4px 8px 4px 26px;
    color: #e2e8f0;
    font-size: 0.74rem;
    outline: none;
    width: 160px;
  }

  .canvas-wrapper {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: radial-gradient(circle at center, rgba(59, 130, 246, 0.05) 0%, transparent 70%);
  }

  .graph-state {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: #64748b;
    font-size: 0.8rem;
    text-align: center;
    padding: 20px;
    background: rgba(10, 15, 30, 0.6);
    z-index: 2;
  }

  .graph-state :global(.spinning) {
    animation: graph-spin 1s linear infinite;
  }

  @keyframes graph-spin {
    to { transform: rotate(360deg); }
  }

  .graph-svg {
    width: 100%;
    height: 100%;
    max-height: 380px;
  }

  .node-group {
    cursor: pointer;
    transition: transform 0.2s ease;
  }

  .node-group:hover .node-circle {
    transform: scale(1.2);
    filter: drop-shadow(0 0 8px currentColor);
  }

  .node-preview {
    padding: 8px 14px;
    background: rgba(0, 0, 0, 0.4);
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .preview-title {
    font-size: 0.8rem;
    font-weight: 600;
    color: #f8fafc;
  }

  .preview-cat {
    font-size: 0.7rem;
  }
</style>
