<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { fade } from "svelte/transition";

  type SkillGroup = { id: string; name: string };

  export let currentTheme: "light" | "dark";
  export let editingSkillId: string | null;
  export let skillFormName: string;
  export let skillFormDesc: string;
  export let skillFormIcon: string;
  export let skillFormCategory: string;
  export let skillFormGroupId: string;
  export let skillFormContent: string;
  export let userSkillGroups: SkillGroup[];
  export let closeSkillForm: () => void;
  export let handleCreateOrUpdateSkill: () => void;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div 
  class="skill-modal-overlay"
  style="position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.5); backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px); display: flex; align-items: center; justify-content: center; z-index: 99999999;"
  on:click|self={closeSkillForm}
  transition:fade={{ duration: 200 }}
>
  <div 
    class="skill-modal"
    style="width: 580px; max-width: 90vw; background: {currentTheme === 'dark' ? '#1c1c1e' : '#f5f5f7'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)'}; border-radius: 16px; box-shadow: 0 20px 50px rgba(0,0,0,0.3); overflow: hidden; display: flex; flex-direction: column;"
  >
    <!-- Header -->
    <div style="display: flex; align-items: center; justify-content: space-between; padding: 18px 24px; border-bottom: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
      <h3 style="margin: 0; font-size: 16px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
        {editingSkillId ? 'Modifier la compétence' : 'Créer une compétence'}
      </h3>
      <button
        type="button"
        on:click={closeSkillForm}
        style="background: none; border: none; color: #86868b; cursor: pointer; display: flex; align-items: center; justify-content: center; padding: 4px; border-radius: 50%; hover:background: rgba(0,0,0,0.05);"
      >
        <X size={16} />
      </button>
    </div>

    <!-- Form Content -->
    <div style="padding: 24px; display: flex; flex-direction: column; gap: 16px; max-height: 70vh; overflow-y: auto; box-sizing: border-box;">
      
      <!-- Nom -->
      <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
        Nom de la compétence
        <input
          type="text"
          bind:value={skillFormName}
          placeholder="ex: Calculateur Local, Web Search"
          style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {currentTheme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
        />
      </label>

      <!-- Description -->
      <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
        Description
        <input
          type="text"
          bind:value={skillFormDesc}
          placeholder="A quoi sert ce skill ?"
          style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {currentTheme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px;"
        />
      </label>

      <!-- Sélection de l'icône -->
      <div style="display: flex; flex-direction: column; gap: 8px;">
        <span style="font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">Icône du Skill</span>
        <div style="display: flex; align-items: center; gap: 12px;">
          <div style="width: 44px; height: 44px; border-radius: 10px; background: {currentTheme === 'dark' ? '#2c2c2e' : '#ffffff'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; display: flex; align-items: center; justify-content: center; font-size: 24px; box-shadow: inset 0 1px 0 rgba(255,255,255,0.1); flex-shrink: 0;">
            {skillFormIcon}
          </div>
          <div style="display: flex; flex-wrap: wrap; gap: 6px;">
            {#each ["🧮", "✍️", "🌐", "💻", "🧠", "🔧", "🔍", "📊", "🎨", "🚀", "⚡", "🧩"] as emoji}
              <button
                type="button"
                on:click={() => (skillFormIcon = emoji)}
                style="width: 32px; height: 32px; border-radius: 8px; background: {skillFormIcon === emoji ? (currentTheme === 'dark' ? '#0071e3' : '#e3f2fd') : (currentTheme === 'dark' ? '#2c2c2e' : '#ffffff')}; border: 1px solid {skillFormIcon === emoji ? '#0071e3' : (currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.1)')}; font-size: 18px; cursor: pointer; display: flex; align-items: center; justify-content: center; transition: all 0.15s; outline: none;"
              >
                {emoji}
              </button>
            {/each}
          </div>
        </div>
      </div>

      <!-- Catégorie -->
      <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
        Catégorie
        <select
          bind:value={skillFormCategory}
          style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {currentTheme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px; color-scheme: {currentTheme === 'dark' ? 'dark' : 'light'};"
        >
          {#each ["Général", "Productivité", "Développement", "Rédaction", "Langues"] as catOption}
            <option value={catOption}>{catOption}</option>
          {/each}
        </select>
      </label>

      <!-- Groupe d'association -->
      <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'};">
        Groupe d'association
        <select
          bind:value={skillFormGroupId}
          style="width: 100%; padding: 8px 10px; border-radius: 8px; background: {currentTheme === 'dark' ? '#2c2c2e' : '#ffffff'}; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; outline: none; box-sizing: border-box; font-weight: normal; font-size: 13px; color-scheme: {currentTheme === 'dark' ? 'dark' : 'light'};"
        >
          {#each userSkillGroups as groupOption}
            <option value={groupOption.id}>{groupOption.name}</option>
          {/each}
        </select>
      </label>

      <!-- Code / Editor Content -->
      <label style="display: flex; flex-direction: column; gap: 6px; font-size: 13px; font-weight: 600; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'}; flex-grow: 1;">
        Instructions système de la compétence
        <textarea
          bind:value={skillFormContent}
          rows="8"
          placeholder="Instructions additionnelles pour guider le comportement du modèle..."
          style="width: 100%; padding: 10px 12px; border-radius: 8px; background: {currentTheme === 'dark' ? '#111112' : '#ffffff'}; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'}; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.08)' : 'rgba(0,0,0,0.12)'}; font-family: SFMono-Regular, Consolas, 'Liberation Mono', Menlo, monospace; font-size: 12px; line-height: 1.5; resize: vertical; outline: none; box-sizing: border-box; font-weight: normal;"
        ></textarea>
      </label>
    </div>

    <!-- Footer Buttons -->
    <div style="display: flex; align-items: center; justify-content: flex-end; gap: 12px; padding: 18px 24px; border-top: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.06)'};">
      <button
        type="button"
        on:click={closeSkillForm}
        style="background: none; border: 1px solid {currentTheme === 'dark' ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.15)'}; border-radius: 8px; padding: 8px 16px; font-size: 13px; font-weight: 500; color: {currentTheme === 'dark' ? '#ffffff' : '#1d1d1f'}; cursor: pointer;"
      >
        Annuler
      </button>
      <button
        type="button"
        on:click={handleCreateOrUpdateSkill}
        disabled={!skillFormName.trim() || !skillFormContent.trim()}
        style="background: {!skillFormName.trim() || !skillFormContent.trim() ? '#86868b' : 'linear-gradient(180deg, #2997ff 0%, #0071e3 100%)'}; color: white; border: none; border-radius: 8px; padding: 8px 16px; font-size: 13px; font-weight: 500; cursor: {!skillFormName.trim() || !skillFormContent.trim() ? 'not-allowed' : 'pointer'}; opacity: {!skillFormName.trim() || !skillFormContent.trim() ? '0.5' : '1'}; box-shadow: 0 1px 3px rgba(0,0,0,0.15);"
      >
        Enregistrer la compétence
      </button>
    </div>
  </div>
</div>

