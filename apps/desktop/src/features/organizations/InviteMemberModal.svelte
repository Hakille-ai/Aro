<script lang="ts">
  import X from "@lucide/svelte/icons/x";

  type InviteRole = "admin" | "manager" | "member" | "guest";
  type Labels = Record<string, string>;

  export let labels: Labels;
  export let name: string;
  export let email: string;
  export let role: InviteRole;
  export let writeLocked: boolean;
  export let disabledTitle: string | undefined;
  export let onClose: () => void;
  export let onInvite: (name: string, email: string, role: InviteRole) => void | Promise<void>;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" on:click={onClose}>
  <div class="modal-card" on:click|stopPropagation>
    <header class="modal-header">
      <h2>{labels.inviteModalTitle}</h2>
      <button class="modal-close-btn" type="button" on:click={onClose}><X size={16} /></button>
    </header>
    <form class="modal-form" on:submit|preventDefault={() => onInvite(name, email, role)}>
      <div class="modal-body">
        <div class="form-row">
          <label for="invite-name" class="modal-label">{labels.inviteNameLabel}</label>
          <input id="invite-name" type="text" placeholder="e.g. Marie Dupont" bind:value={name} autocomplete="off" disabled={writeLocked} required />
        </div>
        <div class="form-row" style="margin-top: 12px;">
          <label for="invite-email" class="modal-label">{labels.inviteEmailLabel}</label>
          <input id="invite-email" type="email" placeholder="e.g. marie.dupont@aro.ai" bind:value={email} autocomplete="off" disabled={writeLocked} required />
        </div>
        <div class="form-row" style="margin-top: 12px;">
          <label for="invite-role" class="modal-label">{labels.inviteRoleLabel}</label>
          <div class="select-wrapper">
            <select id="invite-role" class="settings-select" bind:value={role} disabled={writeLocked}>
              <option value="admin">{labels.roleAdminLabel} - {labels.roleAdminDesc}</option>
              <option value="manager">{labels.roleManagerLabel}</option>
              <option value="member">{labels.roleMemberLabel} - {labels.roleMemberDesc}</option>
              <option value="guest">{labels.roleGuestLabel}</option>
            </select>
          </div>
        </div>
      </div>
      <footer class="modal-footer">
        <button class="modal-btn secondary" type="button" on:click={onClose}>{labels.cancel}</button>
        <button class="modal-btn primary" type="submit" disabled={writeLocked || !name.trim() || !email.trim()} title={disabledTitle ?? labels.inviteSendBtn}>
          {labels.inviteSendBtn}
        </button>
      </footer>
    </form>
  </div>
</div>
