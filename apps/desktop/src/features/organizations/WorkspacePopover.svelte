<script lang="ts">
  import Building2 from "@lucide/svelte/icons/building-2";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
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
    <span class="popover-title">{language === "fr" ? "Espace de travail" : "Workspace"}</span>
    <button class="popover-close-btn" type="button" aria-label="Fermer" on:click={onClose}>
      <X size={14} />
    </button>
  </div>

  <div class="popover-body">
    <div class="org-list">
      {#each organizations as organization}
        <button
          class="org-item-btn"
          class:active={organization.id === session?.activeOrganization.id}
          type="button"
          disabled={busy}
          on:click={() => onSwitch(organization.id)}
        >
          <div class="org-item-left">
            <Building2 size={14} class="org-icon" />
            <span class="org-name">{organization.name}</span>
          </div>
          {#if organization.id === session?.activeOrganization.id}
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
