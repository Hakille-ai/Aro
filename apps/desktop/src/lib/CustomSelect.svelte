<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { createEventDispatcher } from "svelte";

  const dispatch = createEventDispatcher();

  export let value: any;
  export let options: Array<{ value: any; label: string }> = [];
  export let disabled = false;

  let isOpen = false;

  $: selectedOption = options.find(o => o.value === value) || options[0];

  function toggleDropdown() {
    if (!disabled) {
      isOpen = !isOpen;
    }
  }

  function selectOption(optionVal: any) {
    value = optionVal;
    isOpen = false;
    dispatch("change", value);
  }
</script>

<div class="custom-select-container">
  <button
    type="button"
    class="select-trigger"
    class:open={isOpen}
    class:disabled
    on:click={toggleDropdown}
    {disabled}
  >
    <span class="select-label">{selectedOption ? selectedOption.label : ''}</span>
    <span class="select-arrow">
      <ChevronDown size={14} strokeWidth={2.5} />
    </span>
  </button>

  {#if isOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="select-backdrop" on:click={() => (isOpen = false)}></div>
    <div class="select-dropdown">
      {#each options as option}
        <button
          type="button"
          class="select-item"
          class:selected={option.value === value}
          on:click={() => selectOption(option.value)}
        >
          <span class="item-label">{option.label}</span>
          {#if option.value === value}
            <span class="item-check">
              <Check size={13} strokeWidth={3} />
            </span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .custom-select-container {
    position: relative;
    width: 220px;
    box-sizing: border-box;
  }

  .select-trigger {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 28px;
    padding: 0 10px;
    border: 1px solid rgba(0, 0, 0, 0.12);
    border-radius: 6px;
    background: #ffffff;
    color: #1d1d1f;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    outline: none;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
    transition: all 150ms ease;
  }

  .select-trigger:focus {
    border-color: #0071e3;
    box-shadow: 0 0 0 3px rgba(0, 113, 227, 0.25);
  }

  .select-trigger.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .select-arrow {
    display: flex;
    align-items: center;
    color: #86868b;
    transition: transform 150ms ease;
  }

  .select-trigger.open .select-arrow {
    transform: rotate(180deg);
  }

  .select-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    z-index: 999;
    background: transparent;
  }

  .select-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    width: 100%;
    max-height: 200px;
    overflow-y: auto;
    background: rgba(255, 255, 255, 0.85);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
    z-index: 1000;
    padding: 4px;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .select-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 6px 8px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #1d1d1f;
    font-size: 12px;
    font-weight: 500;
    text-align: left;
    cursor: pointer;
    box-sizing: border-box;
    transition: all 120ms ease;
  }

  .select-item:hover {
    background: #0071e3;
    color: #ffffff;
  }

  .select-item:hover :global(svg) {
    color: #ffffff !important;
  }

  .item-check {
    display: flex;
    align-items: center;
    color: #0071e3;
  }

  /* Dark Theme Styling */
  :global(body.dark-theme) .select-trigger {
    border-color: rgba(255, 255, 255, 0.15);
    background: #1e1e20;
    color: #ffffff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  }

  :global(body.dark-theme) .select-trigger:focus {
    border-color: #0a84ff;
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.35);
  }

  :global(body.dark-theme) .select-dropdown {
    background: rgba(36, 36, 38, 0.95);
    backdrop-filter: blur(25px);
    -webkit-backdrop-filter: blur(25px);
    border-color: rgba(255, 255, 255, 0.08);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }

  :global(body.dark-theme) .select-item {
    color: #e3e3e7;
  }

  :global(body.dark-theme) .select-item:hover {
    background: #0a84ff;
    color: #ffffff;
  }

  :global(body.dark-theme) .item-check {
    color: #0a84ff;
  }
</style>
