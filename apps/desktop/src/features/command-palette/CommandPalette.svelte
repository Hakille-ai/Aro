<script lang="ts">
  import Bot from "@lucide/svelte/icons/bot";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Download from "@lucide/svelte/icons/download";
  import Folder from "@lucide/svelte/icons/folder";
  import FolderTree from "@lucide/svelte/icons/folder-tree";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import Mic from "@lucide/svelte/icons/mic";
  import Search from "@lucide/svelte/icons/search";
  import Settings from "@lucide/svelte/icons/settings";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Trash2 from "@lucide/svelte/icons/trash-2";

  type CommandPaletteItem = {
    id: string;
    title: string;
    subtitle?: string;
    category: string;
    icon: string;
    action: () => void;
    shortcut?: string;
  };

  export let language: "fr" | "en";
  export let search: string;
  export let selectedIndex: number;
  export let results: CommandPaletteItem[];
  export let inputElement: HTMLInputElement;
  export let onClose: () => void;
  export let onKeydown: (event: KeyboardEvent) => void;
  export let onExecute: (item: CommandPaletteItem) => void;
  export let formatShortcut: (shortcut: string) => string;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="command-palette-overlay" on:click={onClose}>
  <div class="command-palette-modal" on:click|stopPropagation>
    <div class="command-palette-search">
      <Search size={18} class="search-icon" />
      <input
        bind:this={inputElement}
        bind:value={search}
        placeholder={language === "fr" ? "Rechercher une action, projet, dossier ou discussion..." : "Search actions, projects, folders or chats..."}
        on:keydown={onKeydown}
      />
    </div>

    <div class="command-palette-results">
      {#each results as item, index}
        <div
          class="command-palette-item"
          class:selected={index === selectedIndex}
          on:click={() => onExecute(item)}
          on:mouseenter={() => (selectedIndex = index)}
        >
          <div class="item-icon">
            {#if item.icon === "chat" || item.icon === "chat-item"}
              <MessageSquare size={16} />
            {:else if item.icon === "project"}
              <FolderTree size={16} />
            {:else if item.icon === "folder"}
              <Folder size={16} />
            {:else if item.icon === "export"}
              <Download size={16} />
            {:else if item.icon === "settings"}
              <Settings size={16} />
            {:else if item.icon === "theme"}
              <Sliders size={16} />
            {:else if item.icon === "voice"}
              <Mic size={16} />
            {:else if item.icon === "trash"}
              <Trash2 size={16} />
            {:else if item.icon === "mode"}
              <Bot size={16} />
            {:else}
              <Cpu size={16} />
            {/if}
          </div>
          <div class="item-details">
            <span class="item-title">{item.title}</span>
            {#if item.subtitle}
              <span class="item-subtitle">{item.subtitle}</span>
            {/if}
          </div>
          {#if item.shortcut}
            <div class="item-shortcut">{formatShortcut(item.shortcut)}</div>
          {/if}
        </div>
      {/each}
      {#if results.length === 0}
        <div class="command-palette-empty">
          {language === "fr" ? "Aucun résultat trouvé" : "No results found"}
        </div>
      {/if}
    </div>
    <div class="command-palette-footer">
      <div class="footer-legend">
        <span><kbd>↑↓</kbd> {language === "fr" ? "Naviguer" : "Navigate"}</span>
        <span><kbd>Enter</kbd> {language === "fr" ? "Ouvrir" : "Open"}</span>
        <span><kbd>Esc</kbd> {language === "fr" ? "Fermer" : "Close"}</span>
      </div>
    </div>
  </div>
</div>
