<script lang="ts">
  import CustomSelect from "../../../lib/CustomSelect.svelte";
  type MaybeAsync = void | Promise<void>;
  export let labels: Record<string, string>;
  export let themeDraft: "light" | "dark";
  export let languageDraft: "fr" | "en";
  export let themeOptions: Array<{ value: string; label: string }>;
  export let languageOptions: Array<{ value: string; label: string }>;
  export let onAutosave: () => MaybeAsync;
</script>

<div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{labels.preferencesTabTitle}</h2>
                  <p>{labels.preferencesTabDesc}</p>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.themeTitle}</span>
                      <span class="settings-desc">{labels.themeDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <CustomSelect bind:value={themeDraft} options={themeOptions} on:change={onAutosave} />
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.languageTitle}</span>
                      <span class="settings-desc">{labels.languageDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <CustomSelect bind:value={languageDraft} options={languageOptions} on:change={onAutosave} />
                    </div>
                  </div>
                </div>
              </div>
