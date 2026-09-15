<script lang="ts">
  import X from "@lucide/svelte/icons/x";

  export let editingId: string | null;
  export let language: "fr" | "en";
  export let theme: "light" | "dark";
  export let name: string;
  export let description: string;
  export let path: string;
  export let speakerId: number | null;
  export let voiceLanguage: string;
  export let color: string;
  export let cancelLabel: string;
  export let writeLocked: boolean;
  export let writeDisabledTitle: string | null | undefined;
  export let onClose: () => void;
  export let onSubmit: () => void | Promise<void>;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" on:click={onClose}>
  <div class="modal-card" style="max-width: 520px;" on:click|stopPropagation>
    <header class="modal-header">
      <h2>{editingId ? (language === "fr" ? "Modifier la voix" : "Edit Voice") : (language === "fr" ? "Créer une voix" : "Create Voice")}</h2>
      <button class="modal-close-btn" type="button" on:click={onClose}>
        <X size={16} />
      </button>
    </header>
    <form class="modal-form" on:submit|preventDefault={onSubmit}>
      <div class="modal-body" style="display: flex; flex-direction: column; gap: 14px;">
        <!-- Name -->
        <div class="form-row">
          <label for="voice-name" class="modal-label">{language === "fr" ? "Nom de la voix" : "Voice Name"}</label>
          <input
            id="voice-name"
            type="text"
            placeholder={language === "fr" ? "ex. Français - Julie" : "e.g. US English - Clara"}
            bind:value={name}
            autocomplete="off"
            required
          />
        </div>

        <!-- Description -->
        <div class="form-row">
          <label for="voice-desc" class="modal-label">{language === "fr" ? "Description" : "Description"}</label>
          <input
            id="voice-desc"
            type="text"
            placeholder={language === "fr" ? "ex. Voix féminine naturelle" : "e.g. Natural female voice"}
            bind:value={description}
            autocomplete="off"
          />
        </div>

        <!-- Path -->
        <div class="form-row">
          <label for="voice-path" class="modal-label">{language === "fr" ? "Chemin d'accès au fichier ONNX" : "ONNX Model File Path"}</label>
          <input
            id="voice-path"
            type="text"
            placeholder={language === "fr" ? "ex. C:/models/voice.onnx" : "e.g. C:/models/voice.onnx"}
            bind:value={path}
            autocomplete="off"
            required
          />
        </div>

        <!-- Speaker ID -->
        <div class="form-row">
          <label for="voice-speaker" class="modal-label">{language === "fr" ? "ID du locuteur (Multi-locuteur, optionnel)" : "Speaker ID (Multi-speaker, optional)"}</label>
          <input
            id="voice-speaker"
            type="number"
            min="0"
            placeholder="0"
            bind:value={speakerId}
          />
        </div>

        <!-- Language -->
        <div class="form-row">
          <label for="voice-lang" class="modal-label">{language === "fr" ? "Langue" : "Language"}</label>
          <select
            id="voice-lang"
            class="settings-select"
            style="width: 100%; border-radius: 8px; font-size: 13px; padding: 8px 10px; color-scheme: {theme === 'dark' ? 'dark' : 'light'}; background: {theme === 'dark' ? 'rgba(255,255,255,0.05)' : '#ffffff'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.15)'};"
            bind:value={voiceLanguage}
          >
            <option value="fr">{language === "fr" ? "Français" : "French"}</option>
            <option value="en">{language === "fr" ? "Anglais" : "English"}</option>
            <option value="other">{language === "fr" ? "Autre" : "Other"}</option>
          </select>
        </div>

        <!-- Select Avatar Color Gradient -->
        <div class="form-row">
          <span class="modal-label" style="display: block; margin-bottom: 6px;">{language === "fr" ? "Dégradé de l'avatar" : "Avatar Gradient"}</span>
          <div style="display: flex; gap: 8px; padding: 4px;">
            {#each [
              "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)",
              "linear-gradient(135deg, #34A853 0%, #1A73E8 100%)",
              "linear-gradient(135deg, #F4B400 0%, #EA4335 100%)",
              "linear-gradient(135deg, #A224D6 0%, #FF6B6B 100%)",
              "linear-gradient(135deg, #FF6B6B 0%, #FF8E53 100%)",
              "linear-gradient(135deg, #2C3E50 0%, #000000 100%)"
            ] as grad}
              <button
                type="button"
                style="width: 28px; height: 28px; border-radius: 50%; background: {grad}; border: 2px solid {color === grad ? (theme === 'dark' ? '#ffffff' : '#1d1d1f') : 'transparent'}; cursor: pointer; box-shadow: 0 1px 4px rgba(0,0,0,0.15);"
                on:click={() => (color = grad)}
                aria-label={language === "fr" ? "Dégradé " + grad : "Gradient " + grad}
              ></button>
            {/each}
          </div>
        </div>
      </div>
      <footer class="modal-footer">
        <button class="modal-btn secondary" type="button" on:click={onClose}>
          {cancelLabel}
        </button>
        <button
          class="modal-btn primary"
          type="submit"
          disabled={writeLocked || !name.trim() || !path.trim()}
          title={writeDisabledTitle ?? (language === "fr" ? "Enregistrer" : "Save")}
        >
          {language === "fr" ? "Enregistrer" : "Save"}
        </button>
      </footer>
    </form>
  </div>
</div>
