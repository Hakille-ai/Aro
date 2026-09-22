<script lang="ts">
  import Building2 from "@lucide/svelte/icons/building-2";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import User from "@lucide/svelte/icons/user";
  import X from "@lucide/svelte/icons/x";
  import { fly } from "svelte/transition";
  import type { CloudSessionView, Organization } from "../../lib/types";

  export let language: "fr" | "en";
  export let organizations: Organization[];
  export let session: CloudSessionView | null;
  export let busy: boolean;
  export let error: string;
  export let sidebarOpen: boolean;
  export let sidebarWidth: number;
  export let activeScope: "personal" | "organization" = "organization";
  export let onClose: () => void;
  export let onCreate: () => void;
  export let onSwitch: (organizationId: string) => void | Promise<void>;
</script>

<div
  class="workspace-popover glassmorphic-popover"
  style="left: {sidebarOpen ? '14px' : '92px'}; bottom: {sidebarOpen ? '74px' : '18px'}; width: {sidebarOpen ? (sidebarWidth - 28) + 'px' : '300px'};"
  transition:fly={{ y: 8, duration: 180 }}
>
  <div class="popover-header">
    <span class="popover-title">{language === "fr" ? "Espaces de travail" : "Workspaces"}</span>
    <button class="popover-close-btn" type="button" aria-label="Fermer" on:click={onClose}>
      <X size={14} />
    </button>
  </div>

  <div class="popover-body">
    <!-- Personal Workspace Option -->
    <div class="workspace-section-label">
      {language === "fr" ? "Compte Personnel" : "Personal Account"}
    </div>
    <button
      class="org-item-btn personal-btn"
      class:active={activeScope === "personal"}
      type="button"
      disabled={busy}
      on:click={() => onSwitch("personal")}
    >
      <div class="org-item-left">
        <User size={14} class="org-icon personal-icon" />
        <div class="workspace-meta">
          <span class="org-name">{language === "fr" ? "Espace Personnel" : "Personal Workspace"}</span>
          <span class="workspace-hint">{language === "fr" ? "Privé & modèles locaux" : "Private & local models"}</span>
        </div>
      </div>
      {#if activeScope === "personal"}
        <Check size={14} class="check-icon" />
      {/if}
    </button>

    <!-- Organizations Section -->
    <div class="workspace-section-label" style="margin-top: 10px;">
      {language === "fr" ? "Organisations & Équipes" : "Organizations & Teams"}
    </div>
    <div class="org-list">
      {#each organizations as organization}
        <button
          class="org-item-btn"
          class:active={activeScope === "organization" && organization.id === session?.activeOrganization.id}
          type="button"
          disabled={busy}
          on:click={() => onSwitch(organization.id)}
        >
          <div class="org-item-left">
            <Building2 size={14} class="org-icon" />
            <div class="workspace-meta">
              <span class="org-name">{organization.name}</span>
            </div>
          </div>
          {#if activeScope === "organization" && organization.id === session?.activeOrganization.id}
            <Check size={14} class="check-icon" />
          {/if}
        </button>
      {/each}
    </div>

    <button class="create-org-trigger-btn" type="button" disabled={busy} on:click={onCreate}>
      <Plus size={14} style="margin-right: 4px;" />
      <span>{language === "fr" ? "Créer un workspace" : "Create a workspace"}</span>
    </button>

    {#if error}
      <div class="cloud-auth-error">{error}</div>
    {/if}
  </div>
</div>

<style>
  .workspace-section-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #8e8e93;
    padding: 4px 6px 2px 6px;
  }

  .workspace-meta {
    display: flex;
    flex-direction: column;
    text-align: left;
  }

  .workspace-hint {
    font-size: 10px;
    color: #8e8e93;
  }

  .personal-btn {
    margin-bottom: 6px;
  }

  .personal-icon {
    color: #af52de !important;
  }
</style>
