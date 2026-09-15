<script lang="ts">
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Key from "@lucide/svelte/icons/key";
  import Check from "@lucide/svelte/icons/check";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";

  interface ApiKeyProvider {
    id: string;
    name: string;
    configured: boolean;
    obscuredKey?: string;
  }

  // Aucune clé exemple : afficher "configuré" sans vraie clé serait mensonger.
  export let providers: ApiKeyProvider[] = [
    { id: "openai", name: "OpenAI API Key", configured: false },
    { id: "anthropic", name: "Anthropic Claude Key", configured: false },
    { id: "gemini", name: "Google Gemini Key", configured: false },
    { id: "groq", name: "Groq Cloud Key", configured: false },
  ];

  export let language: "fr" | "en" = "fr";
  export let onClose: () => void = () => {};
  export let onSaveKey: (id: string, key: string) => Promise<void> = async () => {};

  let activeProvider: ApiKeyProvider | null = null;
  let newKey = "";
  let busy = false;
  let successMsg = "";
  let errorMsg = "";

  async function handleSave() {
    if (!activeProvider || !newKey.trim() || busy) return;
    busy = true;
    successMsg = "";
    errorMsg = "";
    try {
      await onSaveKey(activeProvider.id, newKey.trim());
      activeProvider.configured = true;
      activeProvider.obscuredKey = `${newKey.slice(0, 7)}...${newKey.slice(-4)}`;
      successMsg = "Clé d'API enregistrée de manière sécurisée dans le Keyring OS.";
      newKey = "";
      setTimeout(() => (successMsg = ""), 3000);
    } catch (err) {
      console.error("Failed to save API key:", err);
      errorMsg = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="vault-modal-overlay" on:click={onClose}>
  <div class="vault-modal glassmorphic-modal" on:click|stopPropagation>
    <div class="vault-header">
      <div class="title-wrap">
        <ShieldCheck size={20} class="vault-icon" />
        <div>
          <h3>{language === "fr" ? "Coffre-fort de Clés API (OS Vault)" : "API Key Vault"}</h3>
          <p>{language === "fr" ? "Stockage chiffré AES-256 dans le gestionnaire de d'identifiants OS." : "Encrypted storage in OS Credential Vault."}</p>
        </div>
      </div>
      <button class="close-btn" type="button" on:click={onClose}><X size={16} /></button>
    </div>

    <div class="vault-body">
      <div class="providers-list">
        {#each providers as prov}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="provider-row"
            class:active={activeProvider?.id === prov.id}
            on:click={() => (activeProvider = prov)}
          >
            <div class="prov-info">
              <Key size={16} class="key-icon" />
              <span class="prov-name">{prov.name}</span>
            </div>
            {#if prov.configured}
              <span class="configured-badge"><Check size={12} /> {prov.obscuredKey}</span>
            {:else}
              <span class="not-configured-badge">{language === "fr" ? "Non configurée" : "Not set"}</span>
            {/if}
          </div>
        {/each}
      </div>

      {#if activeProvider}
        <div class="key-edit-panel">
          <h4>{language === "fr" ? "Configurer" : "Configure"} {activeProvider.name}</h4>
          <input
            type="password"
            bind:value={newKey}
            placeholder={language === "fr" ? "Collez votre clé d'API sécurisée..." : "Paste secure API key..."}
          />
          {#if successMsg}
            <div class="success-banner">{successMsg}</div>
          {/if}
          {#if errorMsg}
            <div class="error-banner" role="alert">{errorMsg}</div>
          {/if}
          <button class="save-btn" type="button" disabled={busy || !newKey.trim()} on:click={handleSave}>
            {#if busy}<RefreshCw size={14} class="spinning" />{:else}<span>{language === "fr" ? "Chiffrer et Enregistrer" : "Encrypt & Save"}</span>{/if}
          </button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .vault-modal-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    z-index: 99995;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .vault-modal {
    width: 100%;
    max-width: 580px;
    background: rgba(15, 23, 42, 0.94);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 16px;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }

  .vault-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    background: rgba(255, 255, 255, 0.03);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .title-wrap {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  :global(.vault-icon) {
    color: #10b981;
  }

  .title-wrap h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 700;
    color: #f8fafc;
  }

  .title-wrap p {
    margin: 2px 0 0 0;
    font-size: 0.74rem;
    color: #64748b;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: #64748b;
    cursor: pointer;
  }

  .vault-body {
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .providers-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .provider-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .provider-row:hover, .provider-row.active {
    background: rgba(59, 130, 246, 0.15);
    border-color: #3b82f6;
  }

  .prov-info {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.86rem;
    font-weight: 600;
    color: #e2e8f0;
  }

  :global(.key-icon) {
    color: #3b82f6;
  }

  .configured-badge {
    font-size: 0.72rem;
    color: #10b981;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .not-configured-badge {
    font-size: 0.72rem;
    color: #64748b;
  }

  .key-edit-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .key-edit-panel h4 {
    margin: 0;
    font-size: 0.85rem;
    color: #f1f5f9;
  }

  .key-edit-panel input {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 8px 12px;
    color: #f8fafc;
    font-size: 0.85rem;
    outline: none;
  }

  .success-banner {
    color: #10b981;
    font-size: 0.76rem;
  }

  .error-banner {
    color: #f87171;
    font-size: 0.76rem;
  }

  .save-btn {
    align-self: flex-end;
    padding: 6px 14px;
    border-radius: 8px;
    background: #3b82f6;
    color: #ffffff;
    border: none;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }
</style>
