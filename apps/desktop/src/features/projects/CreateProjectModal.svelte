<script lang="ts">
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Box from "@lucide/svelte/icons/box";
  import Briefcase from "@lucide/svelte/icons/briefcase";
  import Code from "@lucide/svelte/icons/code";
  import Cpu from "@lucide/svelte/icons/cpu";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import FolderTree from "@lucide/svelte/icons/folder-tree";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import Terminal from "@lucide/svelte/icons/terminal";
  import X from "@lucide/svelte/icons/x";
  import type { Project } from "../../lib/types";
  import { selectFolderDialog } from "../../lib/api";

  export let projectToEdit: Project | null = null;
  export let writeLocked = false;
  export let disabledTitle: string | undefined = undefined;
  export let onClose: () => void;
  export let onSave: (data: { name: string; description?: string; instructions?: string; rootPath?: string; color: string; icon: string }) => void | Promise<void>;

  let name = projectToEdit?.name ?? "";
  let description = projectToEdit?.description ?? "";
  let instructions = projectToEdit?.instructions ?? "";
  let rootPath = projectToEdit?.rootPath ?? "";
  let color = projectToEdit?.color ?? "#3b82f6";
  let icon = projectToEdit?.icon ?? "folder-tree";

  const colorPresets = ["#3b82f6", "#10b981", "#8b5cf6", "#f59e0b", "#ec4899", "#06b6d4", "#64748b"];
  const iconPresets = [
    { id: "folder-tree", label: "Projet / Arbo", comp: FolderTree },
    { id: "briefcase", label: "Travail", comp: Briefcase },
    { id: "code", label: "Code", comp: Code },
    { id: "terminal", label: "Terminal", comp: Terminal },
    { id: "sparkles", label: "IA / Innov", comp: Sparkles },
    { id: "cpu", label: "Système", comp: Cpu },
    { id: "box", label: "Produit", comp: Box },
    { id: "book-open", label: "Docs", comp: BookOpen },
  ];

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
        description: description.trim() || undefined,
        instructions: instructions.trim() || undefined,
        rootPath: rootPath.trim() || undefined,
        color,
        icon,
      });
      onClose();
    } catch (err) {
      console.error("Failed to save project:", err);
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
  <div class="modal-card project-modal" on:click|stopPropagation>
    <header class="modal-header">
      <h2>{projectToEdit ? "Modifier le Projet" : "Nouveau Projet"}</h2>
      <button class="modal-close-btn" type="button" on:click={onClose}>
        <X size={16} />
      </button>
    </header>
    <form class="modal-form" on:submit|preventDefault={handleSubmit}>
      <div class="modal-body">
        <div class="form-group">
          <label for="project-name" class="modal-label">Nom du Projet</label>
          <input
            id="project-name"
            type="text"
            placeholder="ex. ARO Desktop Engine"
            bind:value={name}
            autocomplete="off"
            disabled={writeLocked}
            required
            class="modal-input"
          />
        </div>

        <div class="form-group">
          <label for="project-desc" class="modal-label">Description (optionnel)</label>
          <input
            id="project-desc"
            type="text"
            placeholder="ex. Système d'IA locale synchronisée"
            bind:value={description}
            autocomplete="off"
            disabled={writeLocked}
            class="modal-input"
          />
        </div>

        <div class="form-group">
          <label for="project-inst" class="modal-label">Consignes du Projet / Context Vault (optionnel)</label>
          <textarea
            id="project-inst"
            placeholder="Consignes applicables à toutes les conversations du projet (ex: Répondre en français, adopter un style concis)..."
            bind:value={instructions}
            rows="3"
            disabled={writeLocked}
            class="modal-textarea"
          ></textarea>
        </div>

        <div class="form-group">
          <label for="project-root-path" class="modal-label">Dossier Racine de Travail (Root Path)</label>
          <div class="input-with-button-row">
            <input
              id="project-root-path"
              type="text"
              placeholder="ex. C:\Users\Stagiaire\Documents\ARO ou /home/user/project"
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
          <span class="field-hint">Dossier racine par défaut hérité par tous les dossiers et conversations du projet pour l'outillage Agent (lecture, écriture, grep, shell).</span>
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
            <input type="color" bind:value={color} class="custom-color-input" title="Couleur personnalisée" />
          </div>
        </div>

        <div class="form-group">
          <span class="modal-label">Icône du Projet</span>
          <div class="icon-selector-grid">
            {#each iconPresets as item}
              <button
                type="button"
                class="icon-preset-btn"
                class:selected={icon === item.id}
                on:click={() => (icon = item.id)}
                title={item.label}
              >
                <svelte:component this={item.comp} size={18} />
              </button>
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
          title={disabledTitle ?? (projectToEdit ? "Enregistrer les modifications" : "Créer le projet")}
        >
          {saving ? "…" : projectToEdit ? "Enregistrer" : "Créer le projet"}
        </button>
      </footer>
    </form>
  </div>
</div>

<style>
  .project-modal {
    max-width: 480px;
    width: 100%;
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
    background: #3b82f6;
    color: #ffffff;
    border: none;
    font-size: 0.8rem;
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  .browse-folder-btn:hover {
    background: #2563eb;
  }

  .browse-folder-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .modal-textarea {
    width: 100%;
    resize: vertical;
    min-height: 70px;
    font-family: inherit;
    font-size: 0.85rem;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid rgba(0, 0, 0, 0.15);
    background: #ffffff;
    color: #0f172a;
  }

  :global(body.dark-theme) .modal-textarea {
    border-color: rgba(255, 255, 255, 0.15);
    background: rgba(0, 0, 0, 0.3);
    color: #f8fafc;
  }

  .modal-textarea:focus {
    outline: none;
    border-color: #3b82f6;
  }

  .field-hint {
    display: block;
    font-size: 0.72rem;
    color: #64748b;
    margin-top: 4px;
    line-height: 1.3;
  }

  :global(body.dark-theme) .field-hint {
    color: rgba(255, 255, 255, 0.5);
  }

  .color-picker-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .color-preset-btn {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    transition: transform 0.15s ease, border-color 0.15s ease;
  }

  .color-preset-btn:hover {
    transform: scale(1.1);
  }

  .color-preset-btn.selected {
    border-color: #ffffff;
    box-shadow: 0 0 0 2px var(--color-primary, #3b82f6);
  }

  .custom-color-input {
    width: 30px;
    height: 30px;
    border: none;
    background: transparent;
    cursor: pointer;
    border-radius: 50%;
  }

  .icon-selector-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }

  .icon-preset-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 10px;
    border-radius: 8px;
    background: #f1f5f9;
    border: 1px solid #cbd5e1;
    color: #334155;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .icon-preset-btn {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
    color: #e2e8f0;
  }

  .icon-preset-btn:hover {
    background: #e2e8f0;
  }

  :global(body.dark-theme) .icon-preset-btn:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .icon-preset-btn.selected {
    border-color: #3b82f6;
    background: #eff6ff;
    color: #2563eb;
  }

  :global(body.dark-theme) .icon-preset-btn.selected {
    border-color: #3b82f6;
    background: rgba(59, 130, 246, 0.2);
    color: #60a5fa;
  }
</style>
