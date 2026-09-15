<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Folder from "@lucide/svelte/icons/folder";
  import FolderKanban from "@lucide/svelte/icons/folder-kanban";
  import Inbox from "@lucide/svelte/icons/inbox";
  import type { Folder as FolderType, Project as ProjectType } from "../../lib/types/projects-folders";

  export let projects: ProjectType[] = [];
  export let folders: FolderType[] = [];
  export let pendingProjectId: string | null = null;
  export let pendingFolderId: string | null = null;
  export let language: "fr" | "en" = "fr";
  export let disabled = false;
  export let onSelect: (projectId: string | null, folderId: string | null) => void = () => {};

  let open = false;
  let container: HTMLDivElement | null = null;

  $: selectedProject = projects.find((p) => p.id === pendingProjectId) ?? null;
  $: selectedFolder = folders.find((f) => f.id === pendingFolderId) ?? null;
  // Même héritage que le backend : dossier -> projet parent du dossier -> projet.
  $: effectiveRoot = (() => {
    if (selectedFolder?.rootPath) return selectedFolder.rootPath;
    if (selectedFolder?.projectId) {
      const parent = projects.find((p) => p.id === selectedFolder.projectId);
      if (parent?.rootPath) return parent.rootPath;
    }
    if (selectedProject?.rootPath) return selectedProject.rootPath;
    return null;
  })();
  $: hasInstructions = Boolean(selectedProject?.instructions);
  $: buttonLabel = (() => {
    if (!selectedProject && !selectedFolder) return language === "fr" ? "Sans classement" : "Unassigned";
    if (selectedFolder) {
      const parent = projects.find((p) => p.id === selectedFolder.projectId);
      return parent ? `${parent.name} / ${selectedFolder.name}` : selectedFolder.name;
    }
    return selectedProject?.name ?? "";
  })();

  function choose(projectId: string | null, folderId: string | null) {
    onSelect(projectId, folderId);
    open = false;
  }

  function handleWindowClick(event: MouseEvent) {
    if (open && container && !container.contains(event.target as Node)) {
      open = false;
    }
  }

  function handleWindowKey(event: KeyboardEvent) {
    if (event.key === "Escape") open = false;
  }

  function handleOpenPicker() {
    if (typeof window === "undefined") return;
    open = true;
  }

  onMount(() => {
    if (typeof window !== "undefined") {
      window.addEventListener("click", handleWindowClick);
      window.addEventListener("keydown", handleWindowKey);
      window.addEventListener("aro:open-destination-picker", handleOpenPicker);
    }
  });

  onDestroy(() => {
    if (typeof window !== "undefined") {
      window.removeEventListener("click", handleWindowClick);
      window.removeEventListener("keydown", handleWindowKey);
      window.removeEventListener("aro:open-destination-picker", handleOpenPicker);
    }
  });
</script>

<div class="dest-picker" bind:this={container}>
  <button
    type="button"
    class="dest-button"
    class:placed={Boolean(selectedProject || selectedFolder)}
    disabled={disabled}
    title={language === "fr" ? "Choisir le projet et le dossier de cette conversation" : "Choose this conversation's project and folder"}
    aria-haspopup="listbox"
    aria-expanded={open}
    on:click={() => (open = !open)}
  >
    {#if selectedProject}
      <span class="dest-dot" style="background-color: {selectedProject.color};"></span>
    {:else}
      <Inbox size={14} />
    {/if}
    <span class="dest-label">{buttonLabel}</span>
    <ChevronDown size={13} />
  </button>

  {#if open}
    <div class="dest-popover" role="listbox">
      <button
        type="button"
        class="dest-option"
        class:selected={!pendingProjectId && !pendingFolderId}
        on:click={() => choose(null, null)}
      >
        <Inbox size={15} />
        <span>{language === "fr" ? "Conversation indépendante" : "Independent conversation"}</span>
        {#if !pendingProjectId && !pendingFolderId}<Check size={15} />{/if}
      </button>

      {#if projects.length > 0}
        <div class="dest-divider"><span>{language === "fr" ? "OU UN PROJET" : "OR A PROJECT"}</span></div>
        {#each projects as proj}
          {@const projFolders = folders.filter((f) => f.projectId === proj.id)}
          <div class="dest-project" class:selected={pendingProjectId === proj.id}>
            <button type="button" class="dest-project-head" on:click={() => choose(proj.id, null)}>
              <span class="dest-dot" style="background-color: {proj.color};"></span>
              <span class="dest-project-name">{proj.name}</span>
              {#if pendingProjectId === proj.id && !pendingFolderId}<Check size={15} />{/if}
            </button>
            <div class="dest-folder-pills">
              <button
                type="button"
                class="dest-pill"
                class:active={pendingProjectId === proj.id && !pendingFolderId}
                on:click={() => choose(proj.id, null)}
              >
                {language === "fr" ? "Racine du projet" : "Project root"}
              </button>
              {#each projFolders as fold}
                <button
                  type="button"
                  class="dest-pill"
                  class:active={pendingFolderId === fold.id}
                  on:click={() => choose(proj.id, fold.id)}
                >
                  <Folder size={12} />
                  <span>{fold.name}</span>
                </button>
              {/each}
            </div>
          </div>
        {/each}
      {:else}
        <div class="dest-empty">
          {language === "fr"
            ? "Aucun projet pour l'instant — créez-en un depuis la barre latérale."
            : "No projects yet — create one from the sidebar."}
        </div>
      {/if}

      <div class="dest-hint">
        {#if effectiveRoot}
          <FolderKanban size={12} />
          <span title={effectiveRoot}>
            {language === "fr" ? "Root : " : "Root: "}{effectiveRoot.length > 48 ? `…${effectiveRoot.slice(-47)}` : effectiveRoot}
          </span>
        {:else}
          <Inbox size={12} />
          <span>
            {language === "fr" ? "Aucun root — fichiers du dossier courant" : "No root — current folder files"}
          </span>
        {/if}
        {#if hasInstructions}
          <span class="dest-vault">Context Vault</span>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .dest-picker {
    position: relative;
    display: inline-flex;
  }

  .dest-button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 6px 14px;
    border-radius: 20px;
    border: 1px solid rgba(0, 0, 0, 0.1);
    background: rgba(255, 255, 255, 0.8);
    color: #1d1d1f;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
    max-width: 280px;
  }

  :global(body.dark-theme) .dest-button {
    border-color: rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: #f5f5f7;
  }

  .dest-button:hover:not(:disabled) {
    border-color: #0071e3;
  }

  .dest-button.placed {
    border-color: #0071e3;
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
  }

  .dest-button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .dest-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dest-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dest-popover {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
    width: 320px;
    max-height: 380px;
    overflow-y: auto;
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.1);
    border-radius: 14px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.16);
    padding: 10px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  :global(body.dark-theme) .dest-popover {
    background: #1c1c1e;
    border-color: rgba(255, 255, 255, 0.1);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
  }

  .dest-option {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 11px;
    border-radius: 10px;
    border: 1px solid rgba(148, 163, 184, 0.25);
    background: transparent;
    color: #1e293b;
    font-size: 0.82rem;
    font-weight: 500;
    cursor: pointer;
    width: 100%;
    text-align: left;
  }

  :global(body.dark-theme) .dest-option {
    color: #e2e8f0;
  }

  .dest-option.selected {
    border-color: #0071e3;
    background: rgba(0, 113, 227, 0.08);
  }

  .dest-option :global(svg:last-child) {
    margin-left: auto;
    color: #0071e3;
  }

  .dest-divider {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 10px;
    font-weight: 700;
    color: #86868b;
    letter-spacing: 0.05em;
    padding: 4px 2px 0;
  }

  .dest-divider::before,
  .dest-divider::after {
    content: "";
    flex: 1;
    height: 1px;
    background: rgba(148, 163, 184, 0.3);
  }

  .dest-project {
    border: 1px solid rgba(148, 163, 184, 0.25);
    border-radius: 10px;
    padding: 6px;
  }

  .dest-project.selected {
    border-color: #0071e3;
  }

  .dest-project-head {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    background: transparent;
    border: none;
    cursor: pointer;
    padding: 5px 6px;
    border-radius: 7px;
    color: inherit;
    font-size: 0.82rem;
    font-weight: 600;
  }

  .dest-project-head:hover {
    background: rgba(0, 113, 227, 0.07);
  }

  .dest-project-name {
    flex: 1;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #1e293b;
  }

  :global(body.dark-theme) .dest-project-name {
    color: #f1f5f9;
  }

  .dest-folder-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 4px 2px 2px;
  }

  .dest-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 10px;
    border-radius: 999px;
    border: 1px solid rgba(148, 163, 184, 0.3);
    background: transparent;
    color: #475569;
    font-size: 0.72rem;
    cursor: pointer;
  }

  :global(body.dark-theme) .dest-pill {
    color: #cbd5e1;
  }

  .dest-pill.active {
    border-color: #0071e3;
    background: rgba(0, 113, 227, 0.12);
    color: #0071e3;
    font-weight: 600;
  }

  .dest-empty {
    font-size: 0.76rem;
    color: #64748b;
    text-align: center;
    padding: 10px 6px;
  }

  .dest-hint {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.7rem;
    color: #64748b;
    border-top: 1px solid rgba(148, 163, 184, 0.25);
    padding: 8px 4px 2px;
    margin-top: 2px;
  }

  :global(body.dark-theme) .dest-hint {
    color: rgba(255, 255, 255, 0.45);
  }

  .dest-hint span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dest-vault {
    margin-left: auto;
    flex-shrink: 0;
    font-weight: 700;
    font-size: 0.64rem;
    color: #7c3aed;
    background: rgba(124, 58, 237, 0.1);
    border-radius: 999px;
    padding: 2px 8px;
  }
</style>
