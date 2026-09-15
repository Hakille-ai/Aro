<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import CheckCircle from "@lucide/svelte/icons/check-circle";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import X from "@lucide/svelte/icons/x";
  import XCircle from "@lucide/svelte/icons/x-circle";
  import { onMount } from "svelte";
  import { apiRootUrl, getWorkspaceTree, isTauri } from "../../lib/api/transport";

  interface HealthStatusItem {
    id: string;
    name: string;
    status: "ok" | "warning" | "error";
    detail: string;
    latencyMs?: number;
  }

  export let language: "fr" | "en" = "fr";
  export let onClose: () => void = () => {};

  let items: HealthStatusItem[] = [];
  let checking = true;

  const fr = (f: string, e: string) => (language === "fr" ? f : e);

  onMount(async () => {
    await runDiagnostics();
  });

  async function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      return await Promise.race([
        promise,
        new Promise<never>((_, reject) => {
          timer = setTimeout(() => reject(new Error("timeout")), ms);
        }),
      ]);
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  // Diagnostics réels (aucune valeur en dur) : chaque ligne correspond à
  // une vérification exécutée ici même, avec latence mesurée.
  async function runDiagnostics() {
    checking = true;
    items = [];
    const done: HealthStatusItem[] = [];
    try {
      // 1. Runtime : pont IPC Tauri chronométré, ou mode navigateur assumé.
      if (isTauri()) {
        const start = performance.now();
        try {
          const tree = await withTimeout(getWorkspaceTree(undefined), 8000);
          done.push({
            id: "ipc",
            name: "Tauri IPC Bridge",
            status: "ok",
            detail: fr(`Connecté et réactif (${tree.count} entrée(s) lues)`, `Connected (${tree.count} entries read)`),
            latencyMs: Math.round(performance.now() - start),
          });
        } catch (err) {
          done.push({
            id: "ipc",
            name: "Tauri IPC Bridge",
            status: "error",
            detail: err instanceof Error ? err.message : String(err),
          });
        }
      } else {
        done.push({
          id: "ipc",
          name: "Tauri IPC Bridge",
          status: "ok",
          detail: fr("Mode navigateur (API cloud utilisée)", "Browser mode (cloud API in use)"),
        });
      }

      // 2. API cloud : GET /health public, 6 s max.
      {
        const start = performance.now();
        try {
          const resp = await withTimeout(fetch(`${apiRootUrl()}/health`), 6000);
          if (resp.ok) {
            done.push({
              id: "api",
              name: "API Cloud",
              status: "ok",
              detail: fr("Joignable", "Reachable"),
              latencyMs: Math.round(performance.now() - start),
            });
          } else {
            done.push({
              id: "api",
              name: "API Cloud",
              status: "warning",
              detail: fr(`Répond ${resp.status} — vérifiez la configuration`, `HTTP ${resp.status} — check configuration`),
              latencyMs: Math.round(performance.now() - start),
            });
          }
        } catch (err) {
          done.push({
            id: "api",
            name: "API Cloud",
            status: "error",
            detail: fr(
              "Injoignable — démarrez-la (npm run api:dev)",
              "Unreachable — start it (npm run api:dev)"
            ),
          });
        }
      }

      // 3. Persistance locale : écriture/lecture/suppression d'une clé test.
      try {
        const key = "__aro_health_probe__";
        localStorage.setItem(key, "ok");
        const roundtrip = localStorage.getItem(key) === "ok";
        localStorage.removeItem(key);
        done.push({
          id: "storage",
          name: fr("Stockage local", "Local storage"),
          status: roundtrip ? "ok" : "error",
          detail: roundtrip
            ? fr("Lecture/écriture vérifiées", "Read/write verified")
            : fr("Écriture illisible", "Unreadable write"),
        });
      } catch (err) {
        done.push({
          id: "storage",
          name: fr("Stockage local", "Local storage"),
          status: "error",
          detail: err instanceof Error ? err.message : String(err),
        });
      }

      // 4. Microphone : périphériques d'entrée réellement énumérés.
      try {
        const devices = navigator.mediaDevices
          ? await withTimeout(navigator.mediaDevices.enumerateDevices(), 5000)
          : [];
        const mics = devices.filter((d) => d.kind === "audioinput").length;
        done.push({
          id: "mic",
          name: fr("Microphone", "Microphone"),
          status: mics > 0 ? "ok" : "warning",
          detail:
            mics > 0
              ? fr(`${mics} entrée(s) détectée(s)`, `${mics} input(s) detected`)
              : fr("Aucune entrée détectée ou permission refusée", "No input detected or permission denied"),
        });
      } catch (err) {
        done.push({
          id: "mic",
          name: fr("Microphone", "Microphone"),
          status: "warning",
          detail: err instanceof Error ? err.message : String(err),
        });
      }
    } finally {
      items = done;
      checking = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="health-modal-overlay" on:click={onClose}>
  <div class="health-modal glassmorphic-modal" on:click|stopPropagation>
    <div class="health-header">
      <div class="title-wrap">
        <Activity size={20} class="health-icon" />
        <div>
          <h3>{language === "fr" ? "Diagnostic de Santé Système ARO" : "ARO System Health Diagnostic"}</h3>
          <p>{language === "fr" ? "Vérification en temps réel des sous-systèmes Rust & Frontend" : "Real-time verification of Rust & Frontend subsystems"}</p>
        </div>
      </div>
      <button class="close-btn" type="button" on:click={onClose}><X size={16} /></button>
    </div>

    <div class="health-body">
      {#if checking}
        <div class="checking-state">
          <RefreshCw size={24} class="spinning" />
          <span>{language === "fr" ? "Analyse des composants système..." : "Analyzing system components..."}</span>
        </div>
      {:else}
        <div class="health-list">
          {#each items as item}
            <div class="health-item-row">
              <div class="item-left">
                {#if item.status === "ok"}
                  <CheckCircle size={18} class="ok-icon" />
                {:else if item.status === "warning"}
                  <AlertCircle size={18} class="warn-icon" />
                {:else}
                  <XCircle size={18} class="err-icon" />
                {/if}
                <div class="item-meta">
                  <span class="item-name">{item.name}</span>
                  <span class="item-detail">{item.detail}</span>
                </div>
              </div>

              {#if item.latencyMs !== undefined}
                <span class="latency-badge">{item.latencyMs} ms</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="health-footer">
      <button class="refresh-btn" type="button" on:click={runDiagnostics}>
        <RefreshCw size={13} />
        <span>{language === "fr" ? "Relancer l'analyse" : "Re-run analysis"}</span>
      </button>
    </div>
  </div>
</div>

<style>
  .health-modal-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    z-index: 99995;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .health-modal {
    width: 100%;
    max-width: 560px;
    background: rgba(15, 23, 42, 0.94);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 16px;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }

  .health-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    background: rgba(255, 255, 255, 0.03);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .title-wrap {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  :global(.health-icon) {
    color: #10b981;
  }

  .title-wrap h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 700;
    color: #f8fafc;
  }

  .title-wrap p {
    margin: 2px 0 0 0;
    font-size: 0.74rem;
    color: #64748b;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: #64748b;
    cursor: pointer;
  }

  .health-body {
    padding: 16px 20px;
    min-height: 220px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .checking-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: #3b82f6;
    font-size: 0.85rem;
  }

  .health-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .health-item-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .item-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  :global(.ok-icon) {
    color: #10b981;
  }

  :global(.warn-icon) {
    color: #f59e0b;
  }

  :global(.err-icon) {
    color: #ef4444;
  }

  .item-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .item-name {
    font-size: 0.86rem;
    font-weight: 600;
    color: #f1f5f9;
  }

  .item-detail {
    font-size: 0.74rem;
    color: #64748b;
  }

  .latency-badge {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 6px;
    background: rgba(16, 185, 129, 0.15);
    color: #34d399;
    border: 1px solid rgba(16, 185, 129, 0.3);
  }

  .health-footer {
    display: flex;
    justify-content: flex-end;
    padding: 10px 20px;
    background: rgba(0, 0, 0, 0.3);
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }

  .refresh-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 6px 12px;
    color: #e2e8f0;
    font-size: 0.78rem;
    cursor: pointer;
  }
</style>
