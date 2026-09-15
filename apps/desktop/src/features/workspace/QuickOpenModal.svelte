<script lang="ts">
  import Search from "@lucide/svelte/icons/search";
  import FileCode from "@lucide/svelte/icons/file-code";
  import FileText from "@lucide/svelte/icons/file-text";
  import Folder from "@lucide/svelte/icons/folder";
  import X from "@lucide/svelte/icons/x";
  import { onMount, tick } from "svelte";
  import { getWorkspaceTree, type WorkspaceTreeEntry } from "../../lib/api/transport";

  interface WorkspaceFileItem {
    path: string;
    relativePath: string;
    isDir: boolean;
    size?: number;
  }

  export let conversationId: string | undefined = undefined;
  export let language: "fr" | "en" = "fr";
  export let onClose: () => void = () => {};
  export let onSelectFile: (path: string) => void = () => {};

  let query = "";
  let items: WorkspaceFileItem[] = [];
  let loading = true;
  let selectedIndex = 0;
  let inputElement: HTMLInputElement;

  onMount(async () => {
    try {
      const res = await getWorkspaceTree(conversationId);
      items = flattenEntries(res.entries);
    } catch (err) {
      console.error("Failed to load workspace files for Quick Open", err);
    } finally {
      loading = false;
    }
    await tick();
    if (inputElement) inputElement.focus();
  });

  function toItem(entry: WorkspaceTreeEntry): WorkspaceFileItem {
    const relativePath = entry.relativePath || entry.name || entry.path;
    return {
      path: entry.path || relativePath,
      relativePath,
      isDir: !!entry.isDir,
      size: entry.size,
    };
  }

  // Le backend renvoie une liste plate ; on accepte aussi des nœuds
  // imbriqués ({ children }) pour rester compatible.
  function flattenEntries(entries: WorkspaceTreeEntry[]): WorkspaceFileItem[] {
    const list: WorkspaceFileItem[] = [];
    function walk(nodes: Array<WorkspaceTreeEntry & { children?: WorkspaceTreeEntry[] }>) {
      for (const node of nodes || []) {
        if (!node.isDir) {
          list.push(toItem(node));
        }
        if (node.children && node.children.length) {
          walk(node.children);
        }
      }
    }
    walk(entries || []);
    return list;
  }

  $: filteredFiles = items
    .filter(
      (file) =>
        !query.trim() ||
        file.path.toLowerCase().includes(query.toLowerCase()) ||
        file.relativePath.toLowerCase().includes(query.toLowerCase())
    )
    .slice(0, 30);

  $: if (filteredFiles.length) {
    if (selectedIndex >= filteredFiles.length) selectedIndex = 0;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % (filteredFiles.length || 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + (filteredFiles.length || 1)) % (filteredFiles.length || 1);
    } else if (e.key === "Enter" && filteredFiles[selectedIndex]) {
      e.preventDefault();
      onSelectFile(filteredFiles[selectedIndex].relativePath || filteredFiles[selectedIndex].path);
      onClose();
    }
  }

  function getFileName(path: string): string {
    return path.split(/[\/\\]/).pop() || path;
  }

  function getDirName(path: string): string {
    const parts = path.split(/[\/\\]/);
    parts.pop();
    return parts.join("/") || ".";
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="quick-open-overlay" on:click={onClose}>
  <div class="quick-open-modal glassmorphic-modal" on:click|stopPropagation>
    <div class="quick-open-header">
      <Search size={18} class="search-icon" />
      <input
        bind:this={inputElement}
        bind:value={query}
        placeholder={language === "fr" ? "Aller au fichier... (ex: main.rs, App.svelte)" : "Go to file... (e.g. main.rs, App.svelte)"}
        on:keydown={handleKeydown}
      />
      <button class="close-btn" type="button" on:click={onClose}>
        <X size={16} />
      </button>
    </div>

    <div class="quick-open-body">
      {#if loading}
        <div class="quick-open-empty">
          <span>{language === "fr" ? "Chargement des fichiers..." : "Loading workspace files..."}</span>
        </div>
      {:else if filteredFiles.length === 0}
        <div class="quick-open-empty">
          <span>{language === "fr" ? "Aucun fichier correspondant trouvé." : "No matching files found."}</span>
        </div>
      {:else}
        <div class="file-results-list">
          {#each filteredFiles as file, index}
            <div
              class="file-item-row"
              class:selected={index === selectedIndex}
              on:click={() => {
                onSelectFile(file.relativePath || file.path);
                onClose();
              }}
              on:mouseenter={() => (selectedIndex = index)}
            >
              <div class="file-icon">
                {#if file.relativePath.endsWith(".ts") || file.relativePath.endsWith(".rs") || file.relativePath.endsWith(".svelte") || file.relativePath.endsWith(".js")}
                  <FileCode size={16} class="code-icon" />
                {:else}
                  <FileText size={16} class="text-icon" />
                {/if}
              </div>

              <div class="file-info" title={file.path}>
                <span class="file-name">{getFileName(file.relativePath)}</span>
                <span class="file-dir">{getDirName(file.relativePath)}</span>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="quick-open-footer">
      <span><kbd>↑</kbd> <kbd>↓</kbd> Naviguer</span>
      <span><kbd>↵</kbd> Ouvrir</span>
      <span><kbd>ESC</kbd> Fermer</span>
    </div>
  </div>
</div>

<style>
  .quick-open-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 99990;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
  }

  .quick-open-modal {
    width: 100%;
    max-width: 620px;
    background: rgba(15, 23, 42, 0.94);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 16px;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    animation: quick-open-pop 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes quick-open-pop {
    from { opacity: 0; transform: scale(0.96) translateY(-10px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }

  .quick-open-header {
    display: flex;
    align-items: center;
    padding: 14px 18px;
    gap: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
  }

  .quick-open-header :global(.search-icon) {
    color: #3b82f6;
  }

  .quick-open-header input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #f8fafc;
    font-size: 1rem;
    font-weight: 500;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: #64748b;
    cursor: pointer;
    padding: 4px;
    border-radius: 6px;
    transition: all 0.15s ease;
  }

  .close-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.1);
  }

  .quick-open-body {
    max-height: 380px;
    overflow-y: auto;
    padding: 8px;
  }

  .quick-open-empty {
    padding: 24px;
    text-align: center;
    color: #64748b;
    font-size: 0.85rem;
  }

  .file-results-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .file-item-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .file-item-row:hover, .file-item-row.selected {
    background: rgba(59, 130, 246, 0.15);
    border-left: 3px solid #3b82f6;
  }

  .file-icon {
    display: flex;
    align-items: center;
  }

  :global(.code-icon) { color: #60a5fa; }
  :global(.text-icon) { color: #94a3b8; }

  .file-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
  }

  .file-name {
    font-size: 0.88rem;
    font-weight: 600;
    color: #f1f5f9;
  }

  .file-dir {
    font-size: 0.74rem;
    color: #64748b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .quick-open-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 16px;
    padding: 8px 18px;
    background: rgba(0, 0, 0, 0.3);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    font-size: 0.72rem;
    color: #64748b;
  }

  kbd {
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 0.68rem;
    color: #cbd5e1;
  }
</style>
