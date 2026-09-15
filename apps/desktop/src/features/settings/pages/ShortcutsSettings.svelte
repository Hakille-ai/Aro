<script lang="ts">
  import type { ShortcutKey, ShortcutMap } from "../types";
  export let language: "fr" | "en";
  export let shortcuts: ShortcutMap;
  export let recordingShortcutFor: ShortcutKey | null;
  export let formatShortcut: (shortcut: string) => string;
</script>

<div class="settings-tab-panel">
                <div class="panel-header">
                  <h2>{language === "fr" ? "Raccourcis Clavier" : "Keyboard Shortcuts"}</h2>
                  <p>{language === "fr" ? "Personnalisez vos raccourcis pour naviguer et contrôler ARO plus rapidement." : "Customize your keyboard shortcuts to navigate and control ARO faster."}</p>
                </div>

                <div class="settings-group">
                  <div class="shortcut-list-container">
                    
                    <div class="shortcut-row">
                      <div class="shortcut-info">
                        <span class="shortcut-title">{language === "fr" ? "Palette de commandes" : "Command Palette"}</span>
                        <span class="shortcut-desc">{language === "fr" ? "Ouvrir la palette de recherche Spotlight" : "Open the Spotlight search palette"}</span>
                      </div>
                      <div class="shortcut-action">
                        {#if recordingShortcutFor === "spotlight"}
                          <button class="kbd-record-btn active" type="button">
                            {language === "fr" ? "Écoute..." : "Listening..."}
                          </button>
                        {:else}
                          <button class="kbd-record-btn" type="button" on:click={() => (recordingShortcutFor = "spotlight")}>
                            {formatShortcut(shortcuts.spotlight)}
                          </button>
                        {/if}
                      </div>
                    </div>

                    <div class="shortcut-row">
                      <div class="shortcut-info">
                        <span class="shortcut-title">{language === "fr" ? "Activer le mode voix" : "Toggle Voice Dictation"}</span>
                        <span class="shortcut-desc">{language === "fr" ? "Démarrer ou arrêter l'enregistrement audio" : "Start or stop audio recording"}</span>
                      </div>
                      <div class="shortcut-action">
                        {#if recordingShortcutFor === "voice"}
                          <button class="kbd-record-btn active" type="button">
                            {language === "fr" ? "Écoute..." : "Listening..."}
                          </button>
                        {:else}
                          <button class="kbd-record-btn" type="button" on:click={() => (recordingShortcutFor = "voice")}>
                            {formatShortcut(shortcuts.voice)}
                          </button>
                        {/if}
                      </div>
                    </div>

                    <div class="shortcut-row">
                      <div class="shortcut-info">
                        <span class="shortcut-title">{language === "fr" ? "Ouvrir les Paramètres" : "Open Settings"}</span>
                        <span class="shortcut-desc">{language === "fr" ? "Accéder instantanément au panneau de configuration" : "Instantly open the settings panel"}</span>
                      </div>
                      <div class="shortcut-action">
                        {#if recordingShortcutFor === "settings"}
                          <button class="kbd-record-btn active" type="button">
                            {language === "fr" ? "Écoute..." : "Listening..."}
                          </button>
                        {:else}
                          <button class="kbd-record-btn" type="button" on:click={() => (recordingShortcutFor = "settings")}>
                            {formatShortcut(shortcuts.settings)}
                          </button>
                        {/if}
                      </div>
                    </div>

                    <div class="shortcut-row">
                      <div class="shortcut-info">
                        <span class="shortcut-title">{language === "fr" ? "Nouvelle discussion" : "New Chat"}</span>
                        <span class="shortcut-desc">{language === "fr" ? "Créer une nouvelle conversation vierge" : "Create a new blank conversation"}</span>
                      </div>
                      <div class="shortcut-action">
                        {#if recordingShortcutFor === "newChat"}
                          <button class="kbd-record-btn active" type="button">
                            {language === "fr" ? "Écoute..." : "Listening..."}
                          </button>
                        {:else}
                          <button class="kbd-record-btn" type="button" on:click={() => (recordingShortcutFor = "newChat")}>
                            {formatShortcut(shortcuts.newChat)}
                          </button>
                        {/if}
                      </div>
                    </div>

                  </div>
                </div>
              </div>
