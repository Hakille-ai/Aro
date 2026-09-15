<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import type { Action } from "svelte/action";

  export let title: string;
  export let label: string;
  export let placeholder: string;
  export let cancelLabel: string;
  export let saveLabel: string;
  export let value: string;
  export let writeLocked: boolean;
  export let disabledTitle: string | undefined;
  export let focusOnMount: Action<HTMLInputElement>;
  export let onClose: () => void;
  export let onSave: () => void | Promise<void>;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" on:click={onClose}>
  <div class="modal-card" on:click|stopPropagation>
    <header class="modal-header">
      <h2>{title}</h2>
      <button class="modal-close-btn" type="button" on:click={onClose}>
        <X size={16} />
      </button>
    </header>
    <form class="modal-form" on:submit|preventDefault={onSave}>
      <div class="modal-body">
        <label for="rename-input" class="modal-label">{label}</label>
        <input
          id="rename-input"
          type="text"
          {placeholder}
          bind:value
          autocomplete="off"
          disabled={writeLocked}
          required
          use:focusOnMount
        />
      </div>
      <footer class="modal-footer">
        <button class="modal-btn secondary" type="button" on:click={onClose}>{cancelLabel}</button>
        <button class="modal-btn primary" type="submit" disabled={writeLocked || !value.trim()} title={disabledTitle ?? saveLabel}>
          {saveLabel}
        </button>
      </footer>
    </form>
  </div>
</div>
