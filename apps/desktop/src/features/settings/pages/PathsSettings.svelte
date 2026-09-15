<script lang="ts">
  import type { AppSettings } from "../../../lib/types";
  type MaybeAsync = void | Promise<void>;
  export let settingsDraft: AppSettings;
  export let language: "fr" | "en";
  export let labels: Record<string, string>;
  export let writeLocked: boolean;
  export let onAutosave: () => MaybeAsync;
</script>

<div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{labels.pathsTabTitle}</h2>
                  <p>{labels.pathsTabDesc}</p>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.whisperBinaryTitle}</span>
                      <span class="settings-desc">{labels.whisperBinaryDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <input bind:value={settingsDraft.voice.whisperBinary} placeholder="whisper-cli" disabled={writeLocked} on:blur={onAutosave} on:keydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.whisperModelTitle}</span>
                      <span class="settings-desc">{labels.whisperModelDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <input bind:value={settingsDraft.voice.whisperModelPath} placeholder="model.bin" disabled={writeLocked} on:blur={onAutosave} on:keydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
                    </div>
                  </div>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.piperBinaryTitle}</span>
                      <span class="settings-desc">{labels.piperBinaryDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <input bind:value={settingsDraft.voice.piperBinary} placeholder="piper" disabled={writeLocked} on:blur={onAutosave} on:keydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{labels.piperVoiceTitle}</span>
                      <span class="settings-desc">{labels.piperVoiceDesc}</span>
                    </div>
                    <div class="settings-control-col">
                      <input bind:value={settingsDraft.voice.piperVoicePath} placeholder="voice.onnx" disabled={writeLocked} on:blur={onAutosave} on:keydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
                    </div>
                  </div>
                </div>

                <div class="settings-group">
                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{language === "fr" ? "Modele mot d'eveil" : "Wake-word model"}</span>
                      <span class="settings-desc">{language === "fr" ? "Chemin du modèle local qui décide si ARO a élabelsé prononcé." : "Local model path used to decide whether ARO was spoken."}</span>
                    </div>
                    <div class="settings-control-col">
                      <input bind:value={settingsDraft.voice.wakeWord.modelPath} placeholder="models/voice/wake-word/aro.wake" disabled={writeLocked} on:blur={onAutosave} on:keydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
                    </div>
                  </div>

                  <div class="settings-row">
                    <div class="settings-label-col">
                      <span class="settings-title">{language === "fr" ? "Seuil mot d'eveil" : "Wake-word threshold"}</span>
                      <span class="settings-desc">{language === "fr" ? "Plus haut = moins de faux declenchements, plus bas = detection plus sensible." : "Higher means fewer false triggers, lower means more sensitive detection."}</span>
                    </div>
                    <div class="settings-control-col">
                      <input type="number" min="0" max="1" step="0.01" bind:value={settingsDraft.voice.wakeWord.threshold} disabled={writeLocked} on:blur={onAutosave} on:keydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} />
                    </div>
                  </div>
                </div>
              </div>

