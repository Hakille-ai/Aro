<script lang="ts">
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import X from "@lucide/svelte/icons/x";
  import type { Folder, Project } from "../../lib/types";
  import { selectFolderDialog } from "../../lib/api";

  export let folderToEdit: Folder | null = null;
  export let projects: Project[] = [];
  export let defaultProjectId: string | null = null;
  export let writeLocked = false;
  export let disabledTitle: string | undefined = undefined;
  export let onClose: () => void;
  export let onSave: (data: { name: string; projectId?: string | null; rootPath?: string; color?: string }) => void | Promise<void>;

  let name = folderToEdit?.name ?? "";
  let projectId = folderToEdit?.projectId ?? defaultProjectId ?? "";
  let rootPath = folderToEdit?.rootPath ?? "";
  let color = folderToEdit?.color ?? "#10b981";

  const colorPresets = ["#10b981", "#3b82f6", "#8b5cf6", "#f59e0b", "#ec4899", "#64748b"];

  let saveError: string | null = null;
  let saving = false;

  async function handleBrowseFolder() {
    try {
      const selected = await selectFolderDialog();
      if (selected) {
        rootPath = selected;
      }
    } catch (err) {
      console.error("Folder picker failed:", err);
      saveError = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleSubmit() {
    if (!name.trim() || saving) return;
    saving = true;
    saveError = null;
    try {
      await onSave({
        name: name.trim(),
        projectId: projectId || null,
        rootPath: rootPath.trim() || undefined,
        color: color || undefined,
      });
      onClose();
    } catch (err) {
      console.error("Failed to save folder:", err);
      saveError = err instanceof Error ? err.message : String(err);
    } finally {
      saving = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" on:click={onClose}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-card folder-modal" on:click|stopPropagation>
    <header class="modal-header">
      <h2>{folderToEdit ? "Modifier le Dossier" : "Nouveau Dossier"}</h2>
      <button class="modal-close-btn" type="button" on:click={onClose}>
        <X size={16} />
      </button>
    </header>
    <form class="modal-form" on:submit|preventDefault={handleSubmit}>
      <div class="modal-body">
        <div class="form-group">
          <label for="folder-name" class="modal-label">Nom du Dossier</label>
          <input
            id="folder-name"
            type="text"
            placeholder="ex. Documentation & Guides"
            bind:value={name}
            autocomplete="off"
            disabled={writeLocked}
            required
            class="modal-input"
          />
        </div>

        <div class="form-group">
          <label for="folder-project" class="modal-label">Projet Parent (Optionnel)</label>
          <select id="folder-project" bind:value={projectId} class="modal-select" disabled={writeLocked}>
            <option value="">-- Aucun (Dossier Global) --</option>
            {#each projects as proj}
              <option value={proj.id}>{proj.name}</option>
            {/each}
          </select>
        </div>

        <div class="form-group">
          <label for="folder-root-path" class="modal-label">Dossier Racine Spécifique (Optionnel)</label>
          <div class="input-with-button-row">
            <input
              id="folder-root-path"
              type="text"
              placeholder="Si vide, hérite du projet parent..."
              bind:value={rootPath}
              autocomplete="off"
              disabled={writeLocked}
              class="modal-input"
            />
            <button type="button" class="browse-folder-btn" on:click={handleBrowseFolder} disabled={writeLocked} title="Parcourir l'explorateur de l'ordinateur">
              <FolderOpen size={15} />
              <span>Parcourir</span>
            </button>
          </div>
        </div>

        <div class="form-group">
          <span class="modal-label">Couleur d'accentuation</span>
          <div class="color-picker-row">
            {#each colorPresets as preset}
              <button
                type="button"
                class="color-preset-btn"
                class:selected={color === preset}
                style="background-color: {preset};"
                on:click={() => (color = preset)}
                aria-label="Sélectionner la couleur {preset}"
              ></button>
            {/each}
          </div>
        </div>
      </div>
      {#if saveError}
        <p class="modal-form-error" role="alert">{saveError}</p>
      {/if}
      <footer class="modal-footer">
        <button class="modal-btn secondary" type="button" on:click={onClose}>Annuler</button>
        <button
          class="modal-btn primary"
          type="submit"
          disabled={writeLocked || saving || !name.trim()}
          title={disabledTitle ?? (folderToEdit ? "Enregistrer les modifications" : "Créer le dossier")}
        >
          {saving ? "…" : folderToEdit ? "Enregistrer" : "Créer le dossier"}
        </button>
      </footer>
    </form>
  </div>
</div>

<style>
  .folder-modal {
    max-width: 440px;
    width: 100%;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }

  .input-with-button-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .input-with-button-row input {
    flex: 1;
  }

  .browse-folder-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 8px;
    background: #10b981;
    color: #ffffff;
    border: none;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  .browse-folder-btn:hover {
    background: #059669;
  }

  .browse-folder-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .modal-select {
    padding: 8px 12px;
    border-radius: 8px;
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.15);
    color: #0f172a;
  }

  :global(body.dark-theme) .modal-select {
    background: rgba(0, 0, 0, 0.3);
    border-color: rgba(255, 255, 255, 0.15);
    color: #f8fafc;
  }

  .color-picker-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .color-preset-btn {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    transition: transform 0.15s ease;
  }

  .color-preset-btn:hover {
    transform: scale(1.1);
  }

  .color-preset-btn.selected {
    border-color: #ffffff;
    box-shadow: 0 0 0 2px var(--color-primary, #10b981);
  }

  .modal-form-error {
    margin: 0 0 10px;
    font-size: 0.76rem;
    color: #dc2626;
    background: rgba(220, 38, 38, 0.08);
    border: 1px solid rgba(220, 38, 38, 0.25);
    border-radius: 8px;
    padding: 8px 10px;
  }
</style>
