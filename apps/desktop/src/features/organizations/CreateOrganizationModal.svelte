<script lang="ts">
  import Building2 from "@lucide/svelte/icons/building-2";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import X from "@lucide/svelte/icons/x";
  import { fade } from "svelte/transition";

  export let language: "fr" | "en";
  export let name: string;
  export let busy: boolean;
  export let error: string;
  export let onClose: () => void;
  export let onCreate: () => Promise<void>;

  async function submit() {
    await onCreate();
    onClose();
  }
</script>

<div class="cloud-auth-backdrop" transition:fade={{ duration: 150 }}>
  <form class="cloud-auth-panel" on:submit|preventDefault={submit}>
    <div class="cloud-auth-header">
      <div>
        <span class="cloud-auth-kicker">{language === "fr" ? "Nouveau workspace" : "New workspace"}</span>
        <h2>{language === "fr" ? "Créer votre espace" : "Create your space"}</h2>
      </div>
      <button class="icon-button" type="button" aria-label="Fermer" on:click={onClose}>
        <X size={18} />
      </button>
    </div>

    <div class="cloud-auth-inputs-group" style="margin-top: 15px;">
      <p style="font-size: 12px; color: #86868b; margin-top: 0; margin-bottom: 15px; line-height: 1.45;">
        {language === "fr"
          ? "Saisissez un nom unique pour votre nouvel espace de travail. Cette action créera une nouvelle instance PostgreSQL multi-tenant dédiée."
          : "Enter a unique name for your new workspace. This will create a dedicated multi-tenant PostgreSQL instance."}
      </p>

      <div class="input-wrapper" style="position: relative; margin-bottom: 12px;">
        <input
          class="cloud-auth-input"
          style="padding-left: 36px;"
          bind:value={name}
          placeholder={language === "fr" ? "Nom de l'organisation / Workspace" : "Workspace / Organization name"}
          autocomplete="off"
          required
          disabled={busy}
        />
        <Building2 size={16} class="input-icon-left" />
      </div>
    </div>

    {#if error}
      <div class="cloud-auth-error">{error}</div>
    {/if}

    <button class="cloud-auth-submit" type="submit" disabled={busy || !name.trim()}>
      {#if busy}
        <RefreshCw size={16} class="spinning-icon" />
      {:else}
        <Building2 size={16} />
      {/if}
      <span>{language === "fr" ? "Confirmer la création" : "Confirm creation"}</span>
    </button>
  </form>
</div>
