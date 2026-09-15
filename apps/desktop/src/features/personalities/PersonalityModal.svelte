<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import Bot from "@lucide/svelte/icons/bot";
  import Brain from "@lucide/svelte/icons/brain";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Check from "@lucide/svelte/icons/check";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import Search from "@lucide/svelte/icons/search";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Terminal from "@lucide/svelte/icons/terminal";
  import User from "@lucide/svelte/icons/user";
  import X from "@lucide/svelte/icons/x";
  import type { VoiceProfile } from "../../lib/types";

  export let editingId: string | null;
  export let language: "fr" | "en";
  export let theme: "light" | "dark";
  export let name: string;
  export let description: string;
  export let prompt: string;
  export let icon: string;
  export let color: string;
  export let temperature: number;
  export let voiceId: string;
  export let voices: VoiceProfile[];
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
      <h2>{editingId ? (language === "fr" ? "Modifier le profil" : "Edit Profile") : (language === "fr" ? "Créer un profil" : "Create Profile")}</h2>
      <button class="modal-close-btn" type="button" on:click={onClose}>
        <X size={16} />
      </button>
    </header>
    <form class="modal-form" on:submit|preventDefault={onSubmit}>
      <div class="modal-body" style="display: flex; flex-direction: column; gap: 14px;">
        <!-- Name -->
        <div class="form-row">
          <label for="pers-name" class="modal-label">{language === "fr" ? "Nom" : "Name"}</label>
          <input
            id="pers-name"
            type="text"
            placeholder={language === "fr" ? "ex. Mon assistant de code" : "e.g. My Coding Buddy"}
            bind:value={name}
            autocomplete="off"
            required
          />
        </div>

        <!-- Description -->
        <div class="form-row">
          <label for="pers-desc" class="modal-label">{language === "fr" ? "Description" : "Description"}</label>
          <input
            id="pers-desc"
            type="text"
            placeholder={language === "fr" ? "ex. Spécialiste des questions d'architecture logicielle" : "e.g. Specialist in software architecture"}
            bind:value={description}
            autocomplete="off"
          />
        </div>

        <!-- Prompt -->
        <div class="form-row">
          <label for="pers-prompt" class="modal-label">{language === "fr" ? "Consignes du profil / Prompt" : "Profile Instructions / Prompt"}</label>
          <textarea
            id="pers-prompt"
            class="settings-textarea"
            rows="4"
            style="min-height: 100px; font-family: inherit; font-size: 13px;"
            placeholder={language === "fr" ? "ex. Tu es un architecte logiciel senior..." : "e.g. You are a senior software architect..."}
            bind:value={prompt}
            required
          ></textarea>
        </div>

        <!-- Select Icon -->
        <div class="form-row">
          <span class="modal-label" style="display: block; margin-bottom: 6px;">{language === "fr" ? "Icône du profil" : "Profile Icon"}</span>
          <div style="display: flex; flex-wrap: wrap; gap: 8px; padding: 4px;">
            {#each ["bot", "code", "check", "edit-2", "brain", "cpu", "user", "activity", "sliders", "building-2", "search"] as iconName}
              <button
                type="button"
                style="width: 32px; height: 32px; border-radius: 6px; border: 2px solid {icon === iconName ? '#0071e3' : 'transparent'}; background: {theme === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.04)'}; display: flex; align-items: center; justify-content: center; cursor: pointer; color: {theme === 'dark' ? '#ffffff' : '#1d1d1f'};"
                on:click={() => (icon = iconName)}
                aria-label={language === "fr" ? "Icône " + iconName : "Icon " + iconName}
              >
                {#if iconName === "bot"}<Bot size={16} />
                {:else if iconName === "code"}<Terminal size={16} />
                {:else if iconName === "check"}<Check size={16} />
                {:else if iconName === "edit-2"}<Edit2 size={16} />
                {:else if iconName === "brain"}<Brain size={16} />
                {:else if iconName === "cpu"}<Cpu size={16} />
                {:else if iconName === "user"}<User size={16} />
                {:else if iconName === "activity"}<Activity size={16} />
                {:else if iconName === "sliders"}<Sliders size={16} />
                {:else if iconName === "building-2"}<Building2 size={16} />
                {:else}<Search size={16} />{/if}
              </button>
            {/each}
          </div>
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

        <!-- Temperature Slider -->
        <div class="form-row">
          <div class="flex-row justify-between" style="margin-bottom: 4px;">
            <label for="pers-temp" class="modal-label" style="margin-bottom: 0;">{language === "fr" ? "Température (Créativité)" : "Temperature (Creativity)"}</label>
            <span style="font-size: 12px; color: #86868b; font-weight: 500;">{temperature}</span>
          </div>
          <input
            id="pers-temp"
            type="range"
            min="0.0"
            max="1.0"
            step="0.1"
            bind:value={temperature}
            style="width: 100%; cursor: pointer;"
          />
          <div class="flex-row justify-between" style="font-size: 10px; color: #86868b; margin-top: 2px;">
            <span>{language === "fr" ? "Précis" : "Precise"}</span>
            <span>{language === "fr" ? "Équilibré" : "Balanced"}</span>
            <span>{language === "fr" ? "Créatif" : "Creative"}</span>
          </div>
        </div>

        <!-- Associated Voice Selector -->
        <div class="form-row">
          <label for="pers-voice" class="modal-label">{language === "fr" ? "Voix TTS associée" : "Associated TTS Voice"}</label>
          <select
            id="pers-voice"
            class="settings-select"
            style="width: 100%; border-radius: 8px; font-size: 13px; padding: 8px 10px; color-scheme: {theme === 'dark' ? 'dark' : 'light'}; background: {theme === 'dark' ? 'rgba(255,255,255,0.05)' : '#ffffff'}; border: 1px solid {theme === 'dark' ? 'rgba(255,255,255,0.1)' : 'rgba(0,0,0,0.15)'};"
            bind:value={voiceId}
          >
            <option value="default">{language === "fr" ? "Voix système par défaut" : "System default voice"}</option>
            {#each voices as voice}
              <option value={voice.id}>{voice.name} ({voice.language.toUpperCase()})</option>
            {/each}
          </select>
        </div>
      </div>
      <footer class="modal-footer">
        <button class="modal-btn secondary" type="button" on:click={onClose}>
          {cancelLabel}
        </button>
        <button
          class="modal-btn primary"
          type="submit"
          disabled={writeLocked || !name.trim() || !prompt.trim()}
          title={writeDisabledTitle ?? (language === "fr" ? "Enregistrer" : "Save")}
        >
          {language === "fr" ? "Enregistrer" : "Save"}
        </button>
      </footer>
    </form>
  </div>
</div>
