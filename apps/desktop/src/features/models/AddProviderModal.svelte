<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import type { ModelProviderKind } from "../../lib/types";

  type ProviderOption = { value: string; label: string };
  type ProviderDefaults = Record<string, { name: string; endpoint: string }>;

  export let language: "fr" | "en";
  export let kind: ModelProviderKind;
  export let name: string;
  export let endpoint: string;
  export let apiKey: string;
  export let writeLocked: boolean;
  export let options: ProviderOption[];
  export let defaults: ProviderDefaults;
  export let onClose: () => void;
  export let onSubmit: () => void | Promise<void>;

  function applyDefaults() {
    const selected = defaults[kind];
    name = selected.name;
    endpoint = selected.endpoint;
  }
</script>

<div class="cloud-auth-backdrop">
  <form class="cloud-auth-panel model-provider-sheet" on:submit|preventDefault={onSubmit}>
    <div class="cloud-auth-header">
      <div>
        <span class="cloud-auth-kicker">Private API</span>
        <h2>{language === "fr" ? "Ajouter un provider" : "Add provider"}</h2>
      </div>
      <button class="icon-button" type="button" aria-label="Fermer" on:click={onClose}>
        <X size={18} />
      </button>
    </div>
    <select class="cloud-auth-input" bind:value={kind} on:change={applyDefaults}>
      {#each options.filter((option) => option.value !== "mock") as option}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
    <input class="cloud-auth-input" bind:value={name} placeholder="Display name" />
    <input class="cloud-auth-input" bind:value={endpoint} placeholder="Endpoint" />
    {#if kind !== "ollama" && kind !== "llama-cpp"}
      <input class="cloud-auth-input" type="password" bind:value={apiKey} placeholder="API key saved locally" />
      <p class="cloud-auth-footnote">{language === "fr" ? "La clé reste dans le credential store de cet appareil." : "The key stays in this device's credential store."}</p>
    {/if}
    <button class="cloud-auth-submit" type="submit" disabled={writeLocked || !name.trim()}>
      <Plus size={16} />
      <span>{language === "fr" ? "Ajouter" : "Add"}</span>
    </button>
  </form>
</div>
