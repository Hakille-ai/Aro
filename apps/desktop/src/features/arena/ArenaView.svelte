<script context="module" lang="ts">
  import type { ArenaSlotState } from "../../lib/types";

  export type ArenaTurn = {
    userContent: string;
    slotA: ArenaSlotState;
    slotB: ArenaSlotState;
  };
  export type ArenaVote = "a" | "b" | "tie";
</script>

<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import X from "@lucide/svelte/icons/x";
  import type { ModelOption } from "../../lib/types";

  export let arenaMode: boolean;
  export let arenaModelA: string;
  export let arenaModelB: string;
  export let modelAMenuOpen: boolean;
  export let modelBMenuOpen: boolean;
  export let modelOptions: ModelOption[];
  export let arenaHistory: ArenaTurn[];
  export let missingLabel: string;
  export let onVote: (turnIndex: number, winner: ArenaVote) => void;
</script>

<!-- â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ ARENA MODE â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ -->
<div class="arena-wrapper">
  <!-- Arena header with close button -->
  <div class="arena-header-bar">
    <span class="arena-header-title">âš”ï¸ Arena Mode â€” Comparateur de ModÃ¨les</span>
    <button
      type="button"
      class="arena-close-btn"
      title="Quitter Arena Mode"
      aria-label="Quitter Arena Mode"
      on:click={() => (arenaMode = false)}
    >
      <X size={15} />
      <span>Quitter Arena</span>
    </button>
  </div>

  <!-- Model pickers row -->
  <div class="arena-pickers">
    <!-- Picker A -->
    <div class="arena-picker-slot">
      <span class="arena-slot-label">🤖 Model A</span>
      <div class="arena-custom-select">
        <button
          class="arena-select-button"
          type="button"
          title={arenaModelA ? `Model A: ${modelOptions.find((model) => model.id === arenaModelA)?.label}` : "Select Model A"}
          aria-expanded={modelAMenuOpen}
          on:click={() => {
            modelAMenuOpen = !modelAMenuOpen;
            modelBMenuOpen = false;
          }}
        >
          <span>{arenaModelA ? (modelOptions.find((model) => model.id === arenaModelA)?.label || arenaModelA) : "— pick model —"}</span>
          <ChevronDown size={14} />
        </button>

        {#if modelAMenuOpen}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="arena-select-backdrop" on:click={() => (modelAMenuOpen = false)}></div>
          <div class="arena-select-menu" role="listbox">
            {#each modelOptions as model}
              <button
                class:active={model.id === arenaModelA}
                type="button"
                role="option"
                aria-selected={model.id === arenaModelA}
                on:click={() => {
                  arenaModelA = model.id;
                  modelAMenuOpen = false;
                }}
              >
                <span class="model-name">{model.label}</span>
                <span class:missing={!model.installed} class="model-state">
                  {model.installed ? model.provider : missingLabel}
                </span>
                {#if model.id === arenaModelA}
                  <Check size={14} />
                {/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <div class="arena-vs-badge">VS</div>

    <!-- Picker B -->
    <div class="arena-picker-slot">
      <span class="arena-slot-label">🤖 Model B</span>
      <div class="arena-custom-select">
        <button
          class="arena-select-button"
          type="button"
          title={arenaModelB ? `Model B: ${modelOptions.find((model) => model.id === arenaModelB)?.label}` : "Select Model B"}
          aria-expanded={modelBMenuOpen}
          on:click={() => {
            modelBMenuOpen = !modelBMenuOpen;
            modelAMenuOpen = false;
          }}
        >
          <span>{arenaModelB ? (modelOptions.find((model) => model.id === arenaModelB)?.label || arenaModelB) : "— pick model —"}</span>
          <ChevronDown size={14} />
        </button>

        {#if modelBMenuOpen}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="arena-select-backdrop" on:click={() => (modelBMenuOpen = false)}></div>
          <div class="arena-select-menu" role="listbox">
            {#each modelOptions as model}
              <button
                class:active={model.id === arenaModelB}
                type="button"
                role="option"
                aria-selected={model.id === arenaModelB}
                on:click={() => {
                  arenaModelB = model.id;
                  modelBMenuOpen = false;
                }}
              >
                <span class="model-name">{model.label}</span>
                <span class:missing={!model.installed} class="model-state">
                  {model.installed ? model.provider : missingLabel}
                </span>
                {#if model.id === arenaModelB}
                  <Check size={14} />
                {/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- History of turns -->
  <div class="arena-history">
    {#each arenaHistory as turn, turnIndex}
      <!-- User prompt row -->
      <div class="arena-user-row">
        <div class="arena-user-bubble">{turn.userContent}</div>
      </div>

      <!-- Split response row -->
      <div class="arena-split-row">
        <!-- Slot A -->
        <div class="arena-slot" class:arena-winner={turn.slotA.vote === "winner"} class:arena-loser={turn.slotA.vote === "loser"} class:arena-tie={turn.slotA.vote === "tie"}>
          <div class="arena-slot-header">
            <span class="arena-model-tag">{turn.slotA.modelId || "Model A"}</span>
            <div class="arena-stats">
              {#if turn.slotA.ttft !== null}<span class="arena-stat" title="Time to first token">⚡ {Math.round(turn.slotA.ttft)}ms</span>{/if}
              {#if turn.slotA.done}<span class="arena-stat" title="Tokens per second">🚀 {turn.slotA.tokensPerSec} t/s</span>{:else if !turn.slotA.done}<span class="arena-stat arena-stat-streaming">â—</span>{/if}
              {#if turn.slotA.vote === "winner"}<span class="arena-badge-win">ðŸ† Winner</span>{:else if turn.slotA.vote === "tie"}<span class="arena-badge-tie">ðŸ¤ Tie</span>{:else if turn.slotA.vote === "loser"}<span class="arena-badge-lose">💀 Lost</span>{/if}
            </div>
          </div>
          <div class="arena-response">{#if turn.slotA.content}{turn.slotA.content}{:else if !turn.slotA.done}<span class="arena-thinking-dot">…</span>{/if}{#if turn.slotA.error}<span class="arena-error">⚠ {turn.slotA.error}</span>{/if}</div>
        </div>

        <!-- Slot B -->
        <div class="arena-slot" class:arena-winner={turn.slotB.vote === "winner"} class:arena-loser={turn.slotB.vote === "loser"} class:arena-tie={turn.slotB.vote === "tie"}>
          <div class="arena-slot-header">
            <span class="arena-model-tag">{turn.slotB.modelId || "Model B"}</span>
            <div class="arena-stats">
              {#if turn.slotB.ttft !== null}<span class="arena-stat" title="Time to first token">⚡ {Math.round(turn.slotB.ttft)}ms</span>{/if}
              {#if turn.slotB.done}<span class="arena-stat" title="Tokens per second">🚀 {turn.slotB.tokensPerSec} t/s</span>{:else if !turn.slotB.done}<span class="arena-stat arena-stat-streaming">â—</span>{/if}
              {#if turn.slotB.vote === "winner"}<span class="arena-badge-win">ðŸ† Winner</span>{:else if turn.slotB.vote === "tie"}<span class="arena-badge-tie">ðŸ¤ Tie</span>{:else if turn.slotB.vote === "loser"}<span class="arena-badge-lose">💀 Lost</span>{/if}
            </div>
          </div>
          <div class="arena-response">{#if turn.slotB.content}{turn.slotB.content}{:else if !turn.slotB.done}<span class="arena-thinking-dot">…</span>{/if}{#if turn.slotB.error}<span class="arena-error">⚠ {turn.slotB.error}</span>{/if}</div>
        </div>
      </div>

      <!-- Vote buttons â€” only show once both are done and not yet voted -->
      {#if turn.slotA.done && turn.slotB.done && turn.slotA.vote === null}
        <div class="arena-vote-row">
          <span class="arena-vote-label">Qui était le meilleur ?</span>
          <button type="button" class="arena-vote-btn vote-a" on:click={() => onVote(turnIndex, "a")}>ðŸ† A</button>
          <button type="button" class="arena-vote-btn vote-tie" on:click={() => onVote(turnIndex, "tie")}>ðŸ¤ Égalité</button>
          <button type="button" class="arena-vote-btn vote-b" on:click={() => onVote(turnIndex, "b")}>ðŸ† B</button>
        </div>
      {/if}
    {/each}
  </div>
</div>
