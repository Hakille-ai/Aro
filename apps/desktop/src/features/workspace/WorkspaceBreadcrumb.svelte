<script lang="ts">
  import FolderTree from "@lucide/svelte/icons/folder-tree";
  import Folder from "@lucide/svelte/icons/folder";
  import HardDrive from "@lucide/svelte/icons/hard-drive";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import type { Project, Folder as FolderType } from "../../lib/types/projects-folders";

  export let project: Project | null = null;
  export let folder: FolderType | null = null;
  export let rootPath: string | null = null;
  export let language: "fr" | "en" = "fr";

  let copied = false;

  async function copyRootPath() {
    if (!rootPath) return;
    await navigator.clipboard.writeText(rootPath);
    copied = true;
    setTimeout(() => (copied = false), 2000);
  }
</script>

<div class="breadcrumb-container">
  <!-- Project Level -->
  <div class="crumb-item project" title={project?.description || project?.name || "Global Workspace"}>
    <FolderTree size={14} class="crumb-icon" />
    <span class="crumb-label">{project ? project.name : (language === "fr" ? "Workspace Global" : "Global Workspace")}</span>
  </div>

  {#if folder}
    <ChevronRight size={13} class="crumb-sep" />
    <div class="crumb-item folder" title={folder.name}>
      <Folder size={14} class="crumb-icon" />
      <span class="crumb-label">{folder.name}</span>
    </div>
  {/if}

  {#if rootPath}
    <ChevronRight size={13} class="crumb-sep" />
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="crumb-item root-path" title={rootPath} on:click={copyRootPath}>
      <HardDrive size={14} class="crumb-icon" />
      <span class="crumb-label path-text">{rootPath.split(/[\/\\]/).pop() || rootPath}</span>
      <span class="copy-badge">
        {#if copied}<Check size={11} />{:else}<Copy size={11} />{/if}
      </span>
    </div>
  {/if}
</div>

<style>
  .breadcrumb-container {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 0.78rem;
    color: #94a3b8;
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
  }

  :global(body.light-theme) .breadcrumb-container {
    background: rgba(0, 0, 0, 0.04);
    border-color: rgba(0, 0, 0, 0.08);
    color: #475569;
  }

  .crumb-item {
    display: flex;
    align-items: center;
    gap: 5px;
    font-weight: 500;
    white-space: nowrap;
  }

  .crumb-item.project {
    color: #3b82f6;
  }

  .crumb-item.folder {
    color: #a855f7;
  }

  .crumb-item.root-path {
    color: #10b981;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(16, 185, 129, 0.1);
    transition: all 0.15s ease;
  }

  .crumb-item.root-path:hover {
    background: rgba(16, 185, 129, 0.2);
  }

  :global(.crumb-sep) {
    color: #475569;
    flex-shrink: 0;
  }

  .copy-badge {
    margin-left: 2px;
    opacity: 0.7;
  }
</style>
