<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import FolderTree from "@lucide/svelte/icons/folder-tree";
  import Layers from "@lucide/svelte/icons/layers";
  import X from "@lucide/svelte/icons/x";
  import type { Conversation, Folder, Project } from "../../lib/types";

  export let conversation: Conversation;
  export let projects: Project[] = [];
  export let folders: Folder[] = [];
  export let writeLocked = false;
  export let onClose: () => void;
  export let onMove: (conversationId: string, projectId: string | null, folderId: string | null) => void | Promise<void>;

  let selectedProjectId = conversation.projectId ?? "";
  let selectedFolderId = conversation.folderId ?? "";

  $: availableFolders = folders.filter((f) => !selectedProjectId || !f.projectId || f.projectId === selectedProjectId);

  function selectProject(projId: string) {
    if (selectedProjectId === projId) {
      selectedProjectId = "";
      selectedFolderId = "";
    } else {
      selectedProjectId = projId;
      selectedFolderId = "";
    }
  }

  function selectUnassigned() {
    selectedProjectId = "";
    selectedFolderId = "";
  }

  async function handleSave() {
    await onMove(
      conversation.id,
      selectedProjectId || null,
      selectedFolderId || null,
    );
    onClose();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" on:click={onClose}>
  <div class="modal-card move-modal" on:click|stopPropagation>
    <header class="modal-header">
      <h2>Organiser la Conversation</h2>
      <button class="modal-close-btn" type="button" on:click={onClose}>
        <X size={16} />
      </button>
    </header>
    <form class="modal-form" on:submit|preventDefault={handleSave}>
      <div class="modal-body">
        <p class="target-title">
          Classer <strong>"{conversation.title}"</strong>
        </p>

        <!-- Unassigned Option -->
        <button
          type="button"
          class="unassigned-card-btn"
          class:selected={!selectedProjectId && !selectedFolderId}
          on:click={selectUnassigned}
        >
          <div class="card-left">
            <Layers size={16} />
            <span>Conversation Indépendante (Aucun Projet)</span>
          </div>
          {#if !selectedProjectId && !selectedFolderId}
            <Check size={16} class="check-icon" />
          {/if}
        </button>

        <div class="section-divider">
          <span>OU SÉLECTIONNER UN PROJET</span>
        </div>

        <!-- Project Grid -->
        <div class="projects-visual-list">
          {#each projects as proj}
            <div class="project-card" class:selected={selectedProjectId === proj.id}>
              <button
                type="button"
                class="project-card-header"
                on:click={() => selectProject(proj.id)}
              >
                <div class="card-left">
                  <span class="project-color-badge" style="background-color: {proj.color};"></span>
                  <span class="project-card-name">{proj.name}</span>
                </div>
                {#if selectedProjectId === proj.id}
                  <Check size={15} class="check-icon" />
                {/if}
              </button>

              <!-- Folders under selected project -->
              {#if selectedProjectId === proj.id}
                {@const projFolders = folders.filter((f) => f.projectId === proj.id || (!f.projectId && selectedProjectId === proj.id))}
                {#if projFolders.length > 0}
                  <div class="folder-options-nested">
                    <span class="folder-nested-label">Dossier :</span>
                    <div class="folder-pills">
                      <button
                        type="button"
                        class="folder-pill"
                        class:active={!selectedFolderId}
                        on:click={() => (selectedFolderId = "")}
                      >
                        Racine du Projet
                      </button>
                      {#each projFolders as fold}
                        <button
                          type="button"
                          class="folder-pill"
                          class:active={selectedFolderId === fold.id}
                          on:click={() => (selectedFolderId = fold.id)}
                        >
                          <FolderIcon size={12} />
                          <span>{fold.name}</span>
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}
              {/if}
            </div>
          {/each}

          {#if projects.length === 0}
            <div class="empty-projects-notice">
              Aucun projet créé pour l'instant. Vous pouvez en créer un depuis la barre latérale.
            </div>
          {/if}
        </div>
      </div>

      <footer class="modal-footer">
        <button class="modal-btn secondary" type="button" on:click={onClose}>Annuler</button>
        <button class="modal-btn primary" type="submit" disabled={writeLocked}>
          Appliquer
        </button>
      </footer>
    </form>
  </div>
</div>

<style>
  .move-modal {
    max-width: 480px;
    width: 100%;
  }

  .target-title {
    font-size: 0.88rem;
    color: var(--text-color-muted, #64748b);
    margin-bottom: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(body.dark-theme) .target-title {
    color: #94a3b8;
  }

  .unassigned-card-btn {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    border-radius: 10px;
    border: 1px solid rgba(148, 163, 184, 0.25);
    background: rgba(0, 0, 0, 0.02);
    color: #1e293b;
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    margin-bottom: 14px;
  }

  :global(body.dark-theme) .unassigned-card-btn {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.1);
    color: #f1f5f9;
  }

  .unassigned-card-btn:hover {
    border-color: #3b82f6;
    background: rgba(59, 130, 246, 0.06);
  }

  .unassigned-card-btn.selected {
    border-color: #3b82f6;
    background: rgba(59, 130, 246, 0.1);
    color: #2563eb;
  }

  :global(body.dark-theme) .unassigned-card-btn.selected {
    color: #60a5fa;
  }

  .card-left {
    display: flex;
    align-items: center;
    gap: 8px;
    overflow: hidden;
  }

  :global(.check-icon) {
    color: #3b82f6;
    flex-shrink: 0;
  }

  .section-divider {
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #64748b;
    margin-bottom: 10px;
  }

  :global(body.dark-theme) .section-divider {
    color: #94a3b8;
  }

  .projects-visual-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-height: 260px;
    overflow-y: auto;
    padding-right: 2px;
  }

  .project-card {
    border-radius: 10px;
    border: 1px solid rgba(148, 163, 184, 0.25);
    background: rgba(0, 0, 0, 0.015);
    transition: all 0.15s ease;
    overflow: hidden;
  }

  :global(body.dark-theme) .project-card {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .project-card.selected {
    border-color: #3b82f6;
    background: rgba(59, 130, 246, 0.05);
  }

  .project-card-header {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 12px;
    background: transparent;
    border: none;
    cursor: pointer;
    color: #0f172a;
    font-size: 0.85rem;
    font-weight: 600;
  }

  :global(body.dark-theme) .project-card-header {
    color: #f8fafc;
  }

  .project-color-badge {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .project-card-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .folder-options-nested {
    padding: 8px 12px 10px;
    border-top: 1px dashed rgba(148, 163, 184, 0.2);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .folder-nested-label {
    font-size: 0.72rem;
    font-weight: 600;
    color: #64748b;
  }

  :global(body.dark-theme) .folder-nested-label {
    color: #94a3b8;
  }

  .folder-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .folder-pill {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    border-radius: 6px;
    font-size: 0.78rem;
    font-weight: 500;
    border: 1px solid rgba(148, 163, 184, 0.3);
    background: transparent;
    color: #475569;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .folder-pill {
    color: #cbd5e1;
    border-color: rgba(255, 255, 255, 0.15);
  }

  .folder-pill:hover {
    border-color: #3b82f6;
    color: #2563eb;
  }

  .folder-pill.active {
    background: #3b82f6;
    border-color: #3b82f6;
    color: #ffffff;
  }

  .empty-projects-notice {
    font-size: 0.8rem;
    color: #64748b;
    text-align: center;
    padding: 20px 10px;
  }
</style>
