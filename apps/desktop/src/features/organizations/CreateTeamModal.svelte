<script lang="ts">
  import X from "@lucide/svelte/icons/x";

  type Labels = Record<string, string>;
  type Member = { id: string; name: string; email: string };

  export let labels: Labels;
  export let name: string;
  export let description: string;
  export let selectedMemberIds: string[];
  export let members: Member[];
  export let writeLocked: boolean;
  export let disabledTitle: string | undefined;
  export let onClose: () => void;
  export let onCreate: (name: string, description: string, memberIds: string[]) => void | Promise<void>;

  function toggleMember(memberId: string, checked: boolean) {
    selectedMemberIds = checked
      ? [...selectedMemberIds, memberId]
      : selectedMemberIds.filter((id) => id !== memberId);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" on:click={onClose}>
  <div class="modal-card" on:click|stopPropagation>
    <header class="modal-header">
      <h2>{labels.createTeamBtn}</h2>
      <button class="modal-close-btn" type="button" on:click={onClose}><X size={16} /></button>
    </header>
    <form class="modal-form" on:submit|preventDefault={() => onCreate(name, description, selectedMemberIds)}>
      <div class="modal-body">
        <div class="form-row">
          <label for="team-name" class="modal-label">{labels.teamNameLabel}</label>
          <input id="team-name" type="text" placeholder="e.g. Frontend Team" bind:value={name} autocomplete="off" disabled={writeLocked} required />
        </div>
        <div class="form-row" style="margin-top: 12px;">
          <label for="team-desc" class="modal-label">{labels.teamDescLabel}</label>
          <input id="team-desc" type="text" placeholder="e.g. UI development and optimization" bind:value={description} autocomplete="off" disabled={writeLocked} />
        </div>
        <div class="form-row" style="margin-top: 12px;">
          <span class="modal-label" style="font-weight: 500; font-size: 12px; margin-bottom: 6px; display: block; color: var(--text-color, inherit);">{labels.teamMembersLabel}</span>
          <div class="modal-members-checklist" style="max-height: 120px; overflow-y: auto; border: 1px solid rgba(0,0,0,0.1); border-radius: 8px; padding: 8px; background: rgba(255,255,255,0.02);">
            {#each members as member}
              <label class="checklist-item" style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px; font-size: 12px; cursor: pointer; color: var(--text-color, inherit);">
                <input
                  type="checkbox"
                  value={member.id}
                  checked={selectedMemberIds.includes(member.id)}
                  disabled={writeLocked}
                  on:change={(event) => toggleMember(member.id, (event.target as HTMLInputElement).checked)}
                />
                <span>{member.name} ({member.email})</span>
              </label>
            {/each}
          </div>
        </div>
      </div>
      <footer class="modal-footer">
        <button class="modal-btn secondary" type="button" on:click={onClose}>{labels.cancel}</button>
        <button class="modal-btn primary" type="submit" disabled={writeLocked || !name.trim()} title={disabledTitle ?? labels.save}>{labels.save}</button>
      </footer>
    </form>
  </div>
</div>
