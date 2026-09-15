<script lang="ts">
  import { onMount } from "svelte";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Code from "@lucide/svelte/icons/code";
  import FileCode from "@lucide/svelte/icons/file-code";
  import FileText from "@lucide/svelte/icons/file-text";
  import Folder from "@lucide/svelte/icons/folder";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import FolderTree from "@lucide/svelte/icons/folder-tree";
  import FolderSearch from "@lucide/svelte/icons/folder-search";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Search from "@lucide/svelte/icons/search";
  import {
    getWorkspaceTree,
    selectFolderDialog,
    setConversationRootPath,
    type WorkspaceTreeEntry,
  } from "../../lib/api";

  export let activeConversationId: string | null = null;
  export let language: "fr" | "en" = "fr";
  export let onSelectFileToPrompt: (path: string) => void = () => {};
  export let onOpenFile: (entry: WorkspaceTreeEntry, rootPath: string) => void = () => {};
  export let activeFilePath: string | null = null;
  export let scopeLabel: string | null = null;

  interface DirNode {
    kind: "dir";
    name: string;
    relativePath: string;
    children: Array<DirNode | FileNode>;
  }
  interface FileNode {
    kind: "file";
    entry: WorkspaceTreeEntry;
  }

  let rootPath: string = ".";
  let entries: WorkspaceTreeEntry[] = [];
  let loading = false;
  let loadError: string | null = null;
  let searchQuery = "";
  let pickingRoot = false;
  let expandedDirs: Record<string, boolean> = {};

  $: hasRoot = rootPath !== "." && rootPath !== "" && rootPath !== null;

  async function loadTree() {
    loading = true;
    loadError = null;
    try {
      const data = await getWorkspaceTree(activeConversationId || undefined);
      rootPath = data.rootPath || ".";
      entries = data.entries || [];
      if (data.readError && entries.length === 0) {
        loadError = data.readError;
      }
      for (const e of entries) {
        if (e.isDir && !(e.relativePath in expandedDirs)) {
          expandedDirs[e.relativePath] = e.relativePath.split("/").length <= 1;
        }
      }
    } catch (err) {
      console.error("Failed to load workspace tree:", err);
      loadError = language === "fr" ? "Impossible de charger les fichiers." : "Failed to load files.";
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    loadTree();
  });

  $: if (activeConversationId) {
    loadTree();
  }

  function toggleDir(path: string) {
    expandedDirs[path] = !expandedDirs[path];
  }

  async function handlePickRoot() {
    if (pickingRoot) return;
    pickingRoot = true;
    try {
      const picked = await selectFolderDialog();
      if (picked && activeConversationId) {
        await setConversationRootPath(activeConversationId, picked);
      } else if (picked && !activeConversationId) {
        rootPath = picked;
      }
      await loadTree();
    } catch (err) {
      console.error("Failed to set workspace root:", err);
    } finally {
      pickingRoot = false;
    }
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function getFileExtension(name: string): string {
    const parts = name.split(".");
    return parts.length > 1 ? parts.pop()!.toLowerCase() : "";
  }

  function getBadgeColor(ext: string): string {
    switch (ext) {
      case "ts":
      case "js":
        return "#3178c6";
      case "rs":
        return "#dea584";
      case "svelte":
        return "#ff3e00";
      case "json":
        return "#f59e0b";
      case "md":
        return "#10b981";
      case "css":
      case "scss":
        return "#ec4899";
      case "html":
        return "#e34f26";
      case "py":
        return "#3572a5";
      case "sql":
        return "#8b5cf6";
      default:
        return "#64748b";
    }
  }

  // L'affichage groupe sur 2 niveaux de dossiers ; les fichiers plus profonds
  // sont rattachés au dossier visible le plus proche (jamais perdus).
  const MAX_DIR_DEPTH = 2;

  function visibleAncestor(rel: string | null): string | null {
    if (!rel) return null;
    const segs = rel.split("/");
    if (segs.length <= MAX_DIR_DEPTH) return rel;
    return segs.slice(0, MAX_DIR_DEPTH).join("/");
  }

  function buildTree(all: WorkspaceTreeEntry[]): Array<DirNode | FileNode> {
    const dirMap = new Map<string, DirNode>();
    const roots: Array<DirNode | FileNode> = [];
    const ensureDir = (rel: string): DirNode | null => {
      const vis = visibleAncestor(rel);
      if (!vis) return null;
      const existing = dirMap.get(vis);
      if (existing) return existing;
      const node: DirNode = { kind: "dir", name: vis.split("/").pop() ?? vis, relativePath: vis, children: [] };
      dirMap.set(vis, node);
      const parentRel = vis.includes("/") ? vis.slice(0, vis.lastIndexOf("/")) : null;
      if (parentRel) {
        ensureDir(parentRel)?.children.push(node);
      } else {
        roots.push(node);
      }
      return node;
    };
    const files: FileNode[] = [];
    for (const e of all) {
      if (e.isDir) {
        const rel = e.relativePath || e.name;
        if (rel.split("/").length > MAX_DIR_DEPTH) continue;
        const node = ensureDir(rel);
        if (node) node.name = e.name || node.name;
      } else {
        files.push({ kind: "file", entry: e });
      }
    }
    for (const f of files) {
      const rel = f.entry.relativePath || f.entry.name;
      const parentRel = rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : null;
      const visParent = visibleAncestor(parentRel);
      if (visParent) {
        ensureDir(visParent)?.children.push(f);
      } else {
        roots.push(f);
      }
    }
    const sortNodes = (nodes: Array<DirNode | FileNode>) => {
      nodes.sort((a, b) => {
        if (a.kind !== b.kind) return a.kind === "dir" ? -1 : 1;
        const an = a.kind === "dir" ? a.name : a.entry.name;
        const bn = b.kind === "dir" ? b.name : b.entry.name;
        return an.localeCompare(bn);
      });
      for (const n of nodes) if (n.kind === "dir") sortNodes(n.children);
    };
    sortNodes(roots);
    return roots;
  }

  $: treeRoots = buildTree(entries);

  $: query = searchQuery.trim().toLowerCase();
  $: searchResults = !query
    ? []
    : entries.filter(
        (e) =>
          !e.isDir &&
          (e.name.toLowerCase().includes(query) || (e.relativePath || "").toLowerCase().includes(query))
      );
</script>

<div class="workspace-tree-explorer animate-fade-in">
  <!-- Header Card -->
  <div class="tree-header">
    <div class="header-title-row">
      <span class="header-icon"><FolderTree size={16} /></span>
      <span class="header-label">{language === "fr" ? "Arborescence du Workspace" : "Workspace File Tree"}</span>
      <button class="refresh-btn" class:spinning={loading} type="button" on:click={loadTree} title="Rafraîchir">
        <RefreshCw size={13} />
      </button>
    </div>
    {#if scopeLabel}
      <div class="scope-badge" title={scopeLabel}>{scopeLabel}</div>
    {/if}
    <div class="root-badge" title={rootPath}>
      <span class="root-dot"></span>
      <span class="root-text">{rootPath}</span>
    </div>
  </div>

  <!-- Search Filter -->
  <div class="tree-search-bar">
    <Search size={13} class="search-icon" />
    <input
      type="text"
      placeholder={language === "fr" ? "Chercher dans les fichiers..." : "Filter workspace files..."}
      bind:value={searchQuery}
    />
  </div>

  <!-- File Tree List -->
  <div class="tree-content">
    {#if loading}
      <div class="tree-skeleton">
        <div class="skeleton-item"></div>
        <div class="skeleton-item"></div>
        <div class="skeleton-item"></div>
      </div>
    {:else if loadError}
      <div class="tree-empty">
        <FileCode size={24} />
        <span>{loadError}</span>
        <button type="button" class="pick-root-btn" on:click={loadTree}>
          {language === "fr" ? "Réessayer" : "Retry"}
        </button>
      </div>
    {:else if !hasRoot && entries.length === 0}
      <div class="tree-empty">
        <FolderSearch size={24} />
        <span>
          {language === "fr"
            ? "Aucun dossier racine. Rattachez cette conversation à un projet ou dossier avec un root, ou choisissez-en un."
            : "No root folder. Attach this conversation to a project/folder with a root, or pick one."}
        </span>
        <button type="button" class="pick-root-btn" disabled={pickingRoot} on:click={handlePickRoot}>
          {pickingRoot
            ? language === "fr" ? "Sélection…" : "Picking…"
            : language === "fr" ? "Parcourir…" : "Browse…"}
        </button>
      </div>
    {:else if query}
      {#if searchResults.length === 0}
        <div class="tree-empty">
          <FileCode size={24} />
          <span>{language === "fr" ? "Aucun fichier correspondant" : "No matching files found"}</span>
        </div>
      {:else}
        <div class="tree-list">
          {#each searchResults as item (item.relativePath)}
            {@const ext = getFileExtension(item.name)}
            <div class="tree-node" class:active={activeFilePath === item.relativePath}>
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="node-content" on:click={() => onOpenFile(item, rootPath)}>
                <span class="node-icon file" style="color: {getBadgeColor(ext)};">
                  <FileText size={14} />
                </span>
                <span class="node-name" title={item.relativePath}>{item.relativePath}</span>
                {#if ext}
                  <span class="ext-badge" style="background: {getBadgeColor(ext)}22; color: {getBadgeColor(ext)};">
                    .{ext}
                  </span>
                {/if}
                <span class="node-size">{formatSize(item.size)}</span>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {:else if treeRoots.length === 0}
      <div class="tree-empty">
        <FileCode size={24} />
        <span>{language === "fr" ? "Dossier vide" : "Empty folder"}</span>
      </div>
    {:else}
      <div class="tree-list">
        {#each treeRoots as node (node.kind === "dir" ? `d:${node.relativePath}` : `f:${node.entry.relativePath}`)}
          {#if node.kind === "dir"}
            <div class="tree-node is-dir">
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="node-content" style="padding-left: 8px;" on:click={() => toggleDir(node.relativePath)}>
                <span class="chevron" class:open={expandedDirs[node.relativePath]}><ChevronRight size={12} /></span>
                <span class="node-icon dir">
                  {#if expandedDirs[node.relativePath]}
                    <FolderOpen size={14} />
                  {:else}
                    <Folder size={14} />
                  {/if}
                </span>
                <span class="node-name" title={node.relativePath}>{node.name}</span>
                <span class="node-count">{node.children.length}</span>
              </div>
              {#if expandedDirs[node.relativePath]}
                <div class="node-children">
                  {#each node.children as child (child.kind === "dir" ? `d:${child.relativePath}` : `f:${child.entry.relativePath}`)}
                    {#if child.kind === "dir"}
                      <div class="tree-node is-dir">
                        <!-- svelte-ignore a11y_click_events_have_key_events -->
                        <!-- svelte-ignore a11y_no_static_element_interactions -->
                        <div class="node-content nested" on:click={() => toggleDir(child.relativePath)}>
                          <span class="chevron" class:open={expandedDirs[child.relativePath]}><ChevronRight size={12} /></span>
                          <span class="node-icon dir">
                            {#if expandedDirs[child.relativePath]}
                              <FolderOpen size={14} />
                            {:else}
                              <Folder size={14} />
                            {/if}
                          </span>
                          <span class="node-name" title={child.relativePath}>{child.name}</span>
                          <span class="node-count">{child.children.length}</span>
                        </div>
                        {#if expandedDirs[child.relativePath]}
                          <div class="node-children">
                            {#each child.children as grand (grand.kind === "dir" ? `d:${grand.relativePath}` : `f:${grand.entry.relativePath}`)}
                              {#if grand.kind === "file"}
                                {@const gext = getFileExtension(grand.entry.name)}
                                <div class="tree-node" class:active={activeFilePath === grand.entry.relativePath}>
                                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                                  <div class="node-content nested2" on:click={() => onOpenFile(grand.entry, rootPath)}>
                                    <span class="node-icon file" style="color: {getBadgeColor(gext)};">
                                      <FileText size={14} />
                                    </span>
                                    <span class="node-name" title={grand.entry.relativePath}>{grand.entry.name}</span>
                                    {#if gext}
                                      <span class="ext-badge" style="background: {getBadgeColor(gext)}22; color: {getBadgeColor(gext)};">
                                        .{gext}
                                      </span>
                                    {/if}
                                    <span class="node-size">{formatSize(grand.entry.size)}</span>
                                    <button
                                      type="button"
                                      class="action-btn"
                                      title="Examiner avec l'Agent"
                                      on:click={(e) => {
                                        e.stopPropagation();
                                        onSelectFileToPrompt(grand.entry.relativePath);
                                      }}
                                    >
                                      <Code size={12} />
                                    </button>
                                  </div>
                                </div>
                              {:else}
                                <div class="tree-node is-dir">
                                  <div class="node-content nested2">
                                    <span class="node-icon dir"><Folder size={14} /></span>
                                    <span class="node-name" title={grand.relativePath}>{grand.name}</span>
                                  </div>
                                </div>
                              {/if}
                            {/each}
                          </div>
                        {/if}
                      </div>
                    {:else}
                      {@const ext = getFileExtension(child.entry.name)}
                      <div class="tree-node" class:active={activeFilePath === child.entry.relativePath}>
                        <!-- svelte-ignore a11y_click_events_have_key_events -->
                        <!-- svelte-ignore a11y_no_static_element_interactions -->
                        <div class="node-content nested" on:click={() => onOpenFile(child.entry, rootPath)}>
                          <span class="node-icon file" style="color: {getBadgeColor(ext)};">
                            <FileText size={14} />
                          </span>
                          <span class="node-name" title={child.entry.relativePath}>{child.entry.name}</span>
                          {#if ext}
                            <span class="ext-badge" style="background: {getBadgeColor(ext)}22; color: {getBadgeColor(ext)};">
                              .{ext}
                            </span>
                          {/if}
                          <span class="node-size">{formatSize(child.entry.size)}</span>
                          <button
                            type="button"
                            class="action-btn"
                            title="Examiner avec l'Agent"
                            on:click={(e) => {
                              e.stopPropagation();
                              onSelectFileToPrompt(child.entry.relativePath);
                            }}
                          >
                            <Code size={12} />
                          </button>
                        </div>
                      </div>
                    {/if}
                  {/each}
                </div>
              {/if}
            </div>
          {:else}
            {@const ext = getFileExtension(node.entry.name)}
            <div class="tree-node" class:active={activeFilePath === node.entry.relativePath}>
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="node-content" on:click={() => onOpenFile(node.entry, rootPath)}>
                <span class="node-icon file" style="color: {getBadgeColor(ext)};">
                  <FileText size={14} />
                </span>
                <span class="node-name" title={node.entry.relativePath}>{node.entry.name}</span>
                {#if ext}
                  <span class="ext-badge" style="background: {getBadgeColor(ext)}22; color: {getBadgeColor(ext)};">
                    .{ext}
                  </span>
                {/if}
                <span class="node-size">{formatSize(node.entry.size)}</span>
                <button
                  type="button"
                  class="action-btn"
                  title="Examiner avec l'Agent"
                  on:click={(e) => {
                    e.stopPropagation();
                    onSelectFileToPrompt(node.entry.relativePath);
                  }}
                >
                  <Code size={12} />
                </button>
              </div>
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .workspace-tree-explorer {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: #ffffff;
    border-radius: 12px;
    padding: 12px;
    gap: 10px;
    border: 1px solid #cbd5e1;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .workspace-tree-explorer {
    background: rgba(255, 255, 255, 0.02);
    border-color: rgba(255, 255, 255, 0.05);
    box-shadow: none;
  }

  .tree-header {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .header-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    font-weight: 600;
    color: #0f172a;
  }

  :global(body.dark-theme) .header-title-row {
    color: #f8fafc;
  }

  .header-icon {
    color: #3b82f6;
    display: flex;
    align-items: center;
  }

  .header-label {
    flex: 1;
  }

  .refresh-btn {
    background: transparent;
    border: none;
    color: #64748b;
    cursor: pointer;
    padding: 4px;
    border-radius: 6px;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .refresh-btn {
    color: rgba(255, 255, 255, 0.5);
  }

  .refresh-btn:hover {
    color: #0f172a;
    background: #f1f5f9;
  }

  :global(body.dark-theme) .refresh-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.1);
  }

  .refresh-btn.spinning {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .scope-badge {
    font-size: 0.72rem;
    font-weight: 600;
    color: #7c3aed;
    background: #f5f3ff;
    border: 1px solid #ddd6fe;
    border-radius: 6px;
    padding: 4px 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(body.dark-theme) .scope-badge {
    background: rgba(139, 92, 246, 0.1);
    border-color: rgba(139, 92, 246, 0.25);
    color: #a78bfa;
  }

  .root-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    border-radius: 6px;
    background: #eff6ff;
    border: 1px solid #bfdbfe;
    font-size: 0.72rem;
    font-weight: 500;
    color: #1d4ed8;
    overflow: hidden;
  }

  :global(body.dark-theme) .root-badge {
    background: rgba(59, 130, 246, 0.1);
    border-color: rgba(59, 130, 246, 0.2);
    color: #60a5fa;
  }

  .root-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #3b82f6;
    flex-shrink: 0;
  }

  .root-text {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tree-search-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    background: #f8fafc;
    border: 1px solid #cbd5e1;
    border-radius: 8px;
    padding: 5px 10px;
  }

  :global(body.dark-theme) .tree-search-bar {
    background: rgba(0, 0, 0, 0.2);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .tree-search-bar input {
    width: 100%;
    background: transparent;
    border: none;
    color: #0f172a;
    font-size: 0.78rem;
    outline: none;
  }

  :global(body.dark-theme) .tree-search-bar input {
    color: #f8fafc;
  }

  .tree-content {
    flex: 1;
    overflow-y: auto;
    border-radius: 8px;
    background: #f8fafc;
    border: 1px solid #e2e8f0;
    padding: 6px;
    min-height: 120px;
  }

  :global(body.dark-theme) .tree-content {
    background: rgba(0, 0, 0, 0.1);
    border-color: transparent;
  }

  .tree-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .tree-node {
    border-radius: 6px;
    transition: background 0.12s ease;
  }

  .tree-node:hover {
    background: #e2e8f0;
  }

  :global(body.dark-theme) .tree-node:hover {
    background: rgba(255, 255, 255, 0.05);
  }

  .tree-node.active {
    background: #dbeafe;
    outline: 1px solid #93c5fd;
  }

  :global(body.dark-theme) .tree-node.active {
    background: rgba(59, 130, 246, 0.15);
    outline: 1px solid rgba(59, 130, 246, 0.4);
  }

  .node-content {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    font-size: 0.78rem;
    cursor: pointer;
    user-select: none;
  }

  .node-content.nested {
    padding-left: 22px;
  }

  .node-content.nested2 {
    padding-left: 38px;
  }

  .chevron {
    display: flex;
    align-items: center;
    color: #94a3b8;
    transition: transform 0.12s ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .node-children {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .node-icon {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .node-icon.dir {
    color: #d97706;
  }

  :global(body.dark-theme) .node-icon.dir {
    color: #f59e0b;
  }

  .node-name {
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: #1e293b;
    font-weight: 500;
    min-width: 0;
  }

  :global(body.dark-theme) .node-name {
    color: #e2e8f0;
    font-weight: normal;
  }

  .node-count {
    font-size: 0.66rem;
    color: #94a3b8;
    background: #e2e8f0;
    border-radius: 999px;
    padding: 0 6px;
  }

  :global(body.dark-theme) .node-count {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.45);
  }

  .ext-badge {
    font-size: 0.65rem;
    font-weight: 600;
    padding: 1px 5px;
    border-radius: 4px;
    text-transform: uppercase;
    flex-shrink: 0;
  }

  .node-size {
    font-size: 0.68rem;
    color: #64748b;
    flex-shrink: 0;
  }

  :global(body.dark-theme) .node-size {
    color: rgba(255, 255, 255, 0.4);
  }

  .action-btn {
    opacity: 0;
    background: #e2e8f0;
    border: none;
    color: #0f172a;
    border-radius: 4px;
    padding: 2px 4px;
    cursor: pointer;
    transition: all 0.15s ease;
    flex-shrink: 0;
  }

  :global(body.dark-theme) .action-btn {
    background: rgba(255, 255, 255, 0.1);
    color: #f8fafc;
  }

  .tree-node:hover .action-btn {
    opacity: 1;
  }

  .action-btn:hover {
    background: #3b82f6;
    color: #ffffff;
  }

  .tree-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 120px;
    gap: 8px;
    color: #64748b;
    font-size: 0.78rem;
    text-align: center;
    padding: 16px;
  }

  :global(body.dark-theme) .tree-empty {
    color: rgba(255, 255, 255, 0.4);
  }

  .pick-root-btn {
    background: #3b82f6;
    color: #fff;
    border: none;
    border-radius: 8px;
    padding: 6px 14px;
    font-size: 0.76rem;
    font-weight: 600;
    cursor: pointer;
  }

  .pick-root-btn:disabled {
    opacity: 0.6;
    cursor: wait;
  }

  .tree-skeleton {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
  }

  .skeleton-item {
    height: 20px;
    border-radius: 4px;
    background: #e2e8f0;
    animation: pulse 1.5s infinite ease-in-out;
  }

  :global(body.dark-theme) .skeleton-item {
    background: rgba(255, 255, 255, 0.05);
  }

  @keyframes pulse {
    0%, 100% { opacity: 0.4; }
    50% { opacity: 0.8; }
  }
</style>
