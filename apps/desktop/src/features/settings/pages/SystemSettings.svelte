<script lang="ts">
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import type { RuntimeStatus } from "../../../lib/types";
  type MaybeAsync = void | Promise<void>;
  export let labels: Record<string, string>;
  export let runtime: RuntimeStatus | null;
  export let writeLocked: boolean;
  export let writeDisabledTitle: (action?: string) => string | null | undefined;
  export let onRefreshRuntime: () => MaybeAsync;
  export let onClearMemory: () => MaybeAsync;
</script>

<div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{labels.maintenanceTabTitle}</h2>
                  <p>{labels.maintenanceTabDesc}</p>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.runtimeStatusTitle}</span>
                      <span class="settings-desc">{labels.runtimeStatusDesc}</span>
                    </div>
                    <div class="settings-control-col system-info">
                      <pre><code>{runtime?.detail ?? labels.noRuntimeInfo}</code></pre>
                    </div>
                  </div>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.checkStatusTitle}</span>
                      <span class="settings-desc">{labels.checkStatusDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <button class="apple-btn secondary" type="button" on:click={onRefreshRuntime}>
                        <RefreshCw size={14} />
                        <span>{labels.checkBtn}</span>
                      </button>
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.resetMemoryTitle}</span>
                      <span class="settings-desc text-danger">{labels.resetMemoryDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <button
                        class="apple-btn danger-action"
                        type="button"
                        disabled={writeLocked}
                        title={writeDisabledTitle("reinitialiser la memoire") ?? labels.resetMemoryTitle}
                        on:click={onClearMemory}
                      >
                        <Trash2 size={14} />
                        <span>{labels.resetBtn}</span>
                      </button>
                    </div>
                  </div>
                </div>
              </div>
