<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import Globe from "@lucide/svelte/icons/globe";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Lock from "@lucide/svelte/icons/lock";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Hand from "@lucide/svelte/icons/hand";
  import Bot from "@lucide/svelte/icons/bot";
  import MousePointerClick from "@lucide/svelte/icons/mouse-pointer-click";
  import Key from "@lucide/svelte/icons/key";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import {
    liveBrowserAvailable,
    liveTabsList,
    liveTabOpen,
    liveTabClose,
    liveNavigate,
    liveSnapshot,
    liveFrame,
    liveClick,
    liveClickRef,
    liveTypeText,
    livePressKey,
    liveWheel,
    liveAutofillUser,
    vaultAccounts,
    type LiveBrowserTab,
    type LivePageSnapshot,
  } from "./browser-store";

  export let language: "fr" | "en" = "fr";
  export let onSendToAi: ((url: string, title: string) => void) | undefined = undefined;

  interface EventTab {
    kind: "event";
    url: string;
    title: string;
    content: string;
    stepTitle: string;
  }

  let liveTabs: LiveBrowserTab[] = [];
  let activeLiveId: string | null = null;
  let eventTabs: EventTab[] = [];
  let selectedEventUrl: string | null = null;
  let snapshot: LivePageSnapshot | null = null;
  let frameBase64 = "";
  let frameMime = "image/jpeg";
  let frameLoading = false;
  let backendError: string | null = null;
  let booting = true;
  let addressInput = "";
  let typeInput = "";
  let userControl = false;
  let acting = false;
  let autofillAccount = "";
  let autofillDone: string | null = null;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let frameImg: HTMLImageElement | null = null;
  let wheelCooldown = false;

  $: activeLiveTab = liveTabs.find((t) => t.id === activeLiveId) ?? null;
  $: viewingEvent = selectedEventUrl !== null;
  $: currentEvent = eventTabs.find((t) => t.url === selectedEventUrl) ?? null;
  $: refs = (snapshot?.elements ?? []).filter((e) => e.inViewport).slice(0, 24);
  $: passwordRef = snapshot?.elements.find((e) => e.inputType === "password")?.id;
  $: usernameRef = snapshot?.elements.find(
    (e) => (e.inputType === "text" || e.inputType === "email") && /user|login|email|mail|ident/i.test(e.name)
  )?.id;

  function t(fr: string, en: string): string {
    return language === "fr" ? fr : en;
  }

  async function refreshTabs(selectId?: string): Promise<void> {
    if (!liveBrowserAvailable()) {
      backendError = t(
        "Moteur navigateur indisponible (mode web). Les contenus envoyés par l'agent restent lisibles ci-dessous.",
        "Browser engine unavailable (web mode). Agent-provided content stays readable below."
      );
      booting = false;
      return;
    }
    try {
      liveTabs = await liveTabsList();
      backendError = null;
      if (selectId) {
        activeLiveId = selectId;
        selectedEventUrl = null;
      } else if (!activeLiveId && liveTabs.length > 0) {
        activeLiveId = liveTabs[0].id;
      } else if (activeLiveId && !liveTabs.some((tab) => tab.id === activeLiveId)) {
        activeLiveId = liveTabs.length > 0 ? liveTabs[0].id : null;
      }
      if (activeLiveId) {
        const tab = liveTabs.find((x) => x.id === activeLiveId);
        if (tab) addressInput = tab.url;
      }
    } catch (err) {
      backendError = err instanceof Error ? err.message : String(err);
    } finally {
      booting = false;
    }
  }

  async function refreshSnapshot(): Promise<void> {
    if (!activeLiveId || !liveBrowserAvailable()) return;
    try {
      snapshot = await liveSnapshot(activeLiveId);
      addressInput = snapshot.url;
    } catch {
      // Le frame polling continuera d'essayer ; pas de bruit.
    }
  }

  async function pollFrame(): Promise<void> {
    if (!activeLiveId || frameLoading || viewingEvent || !liveBrowserAvailable()) return;
    if (typeof document !== "undefined" && document.hidden) return;
    frameLoading = true;
    try {
      const frame = await liveFrame(activeLiveId);
      frameBase64 = frame.imageBase64;
      frameMime = frame.mime || "image/jpeg";
    } catch {
      // Onglet fermé côté moteur ou Chromium arrêté : on resynchronise.
      await refreshTabs();
    } finally {
      frameLoading = false;
    }
  }

  async function runAct(label: string, fn: () => Promise<LivePageSnapshot>): Promise<void> {
    if (!activeLiveId || acting) return;
    acting = true;
    autofillDone = null;
    try {
      snapshot = await fn();
      addressInput = snapshot.url;
      frameBase64 = snapshot.screenshotBase64;
      await refreshTabs(activeLiveId);
    } catch (err) {
      backendError = `${label} : ${err instanceof Error ? err.message : String(err)}`;
    } finally {
      acting = false;
    }
  }

  async function handleGo(): Promise<void> {
    const url = addressInput.trim();
    if (!url) return;
    await runAct(t("Navigation", "Navigate"), () => liveNavigate(activeLiveId, url));
  }

  async function handleNewTab(): Promise<void> {
    if (!liveBrowserAvailable()) return;
    acting = true;
    try {
      const tab = await liveTabOpen("https://duckduckgo.com");
      await refreshTabs(tab.id);
      await refreshSnapshot();
    } catch (err) {
      backendError = err instanceof Error ? err.message : String(err);
    } finally {
      acting = false;
    }
  }

  async function handleCloseTab(id: string): Promise<void> {
    try {
      await liveTabClose(id);
    } catch {
      // Même en échec, on resynchronise l'état local.
    }
    await refreshTabs();
    snapshot = activeLiveId ? snapshot : null;
  }

  function selectLiveTab(id: string): void {
    activeLiveId = id;
    selectedEventUrl = null;
    snapshot = null;
    frameBase64 = "";
    void refreshSnapshot();
    void pollFrame();
  }

  function selectEventTab(url: string): void {
    selectedEventUrl = url;
  }

  async function handleFrameClick(e: MouseEvent): Promise<void> {
    if (!frameImg || !activeLiveId || !snapshot) return;
    const rect = frameImg.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const x = ((e.clientX - rect.left) / rect.width) * snapshot.screenshotWidth;
    const y = ((e.clientY - rect.top) / rect.height) * snapshot.screenshotHeight;
    await runAct(t("Clic", "Click"), () => liveClick(activeLiveId as string, x, y));
  }

  function handleFrameWheel(e: WheelEvent): void {
    if (!activeLiveId || wheelCooldown) return;
    wheelCooldown = true;
    setTimeout(() => (wheelCooldown = false), 350);
    e.preventDefault();
    void runAct(t("Défilement", "Scroll"), () =>
      liveWheel(activeLiveId as string, e.deltaX, e.deltaY)
    );
  }

  async function handleTypeSend(): Promise<void> {
    const text = typeInput.trim();
    if (!text || !activeLiveId) return;
    typeInput = "";
    await tick();
    await runAct(t("Saisie", "Type"), () => liveTypeText(activeLiveId as string, text));
  }

  async function handleKeyDown(e: KeyboardEvent): Promise<void> {
    if (!activeLiveId) return;
    const target = e.target as HTMLElement | null;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) return;
    const mapped: Record<string, string> = {
      Enter: "Enter",
      Escape: "Escape",
      Tab: "Tab",
      Backspace: "Backspace",
      Delete: "Delete",
      ArrowLeft: "ArrowLeft",
      ArrowUp: "ArrowUp",
      ArrowRight: "ArrowRight",
      ArrowDown: "ArrowDown",
      Home: "Home",
      End: "End",
      PageUp: "PageUp",
      PageDown: "PageDown",
    };
    const key = mapped[e.key];
    if (!key) return;
    e.preventDefault();
    await runAct(t("Touche", "Key"), () => livePressKey(activeLiveId as string, key));
  }

  async function handleRefClick(refId: string): Promise<void> {
    if (!activeLiveId) return;
    await runAct(t("Clic", "Click"), () => liveClickRef(activeLiveId as string, refId));
  }

  async function handleAutofill(): Promise<void> {
    if (!activeLiveId || !usernameRef || !passwordRef || !autofillAccount) return;
    await runAct(t("Remplissage", "Autofill"), () =>
      liveAutofillUser(activeLiveId as string, usernameRef, passwordRef, autofillAccount)
    );
    autofillDone = autofillAccount;
  }

  function handleTakeControl(): void {
    userControl = true;
    if (typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("aro:browser-user-control", { detail: { controlled: true } }));
    }
  }

  function handleReleaseToAi(): void {
    userControl = false;
    if (typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent("aro:browser-user-control", { detail: { controlled: false } }));
    }
    void refreshSnapshot();
  }

  function handleAgentStep(e: Event): void {
    const detail = (e as CustomEvent).detail as {
      url?: string;
      title?: string;
      stepTitle?: string;
      content?: string;
      tabId?: string;
    };
    if (!detail?.url) return;
    const existing = eventTabs.find((tab) => tab.url === detail.url);
    if (existing) {
      existing.title = detail.title || existing.title;
      if (detail.content) existing.content = detail.content;
      existing.stepTitle = detail.stepTitle || existing.stepTitle;
      eventTabs = [...eventTabs];
    } else {
      eventTabs = [
        ...eventTabs,
        {
          kind: "event",
          url: detail.url,
          title: detail.title || detail.url,
          content: detail.content || "",
          stepTitle: detail.stepTitle || "",
        },
      ];
    }
    if (!userControl) {
      const live =
        (detail.tabId && liveTabs.find((tab) => tab.id === detail.tabId)) ||
        liveTabs.find((tab) => tab.url === detail.url);
      if (live) {
        selectLiveTab(live.id);
      } else {
        selectedEventUrl = detail.url;
      }
    }
  }

  function extractDomain(url: string): string {
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch {
      return url;
    }
  }

  onMount(() => {
    void refreshTabs();
    pollTimer = setInterval(() => void pollFrame(), 900);
    if (typeof window !== "undefined") {
      window.addEventListener("aro:agent-browser-step", handleAgentStep);
    }
    return () => {
      if (pollTimer) clearInterval(pollTimer);
      if (typeof window !== "undefined") {
        window.removeEventListener("aro:agent-browser-step", handleAgentStep);
      }
    };
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
  });
</script>

<div class="browser-workspace" on:keydown={handleKeyDown} role="application" aria-label={t("Navigateur ARO", "ARO Browser")} tabindex="0">
  <!-- Barre d'onglets -->
  <div class="tabs-bar">
    <div class="tabs-list" role="tablist">
      {#each liveTabs as tab (tab.id)}
        <button
          type="button"
          role="tab"
          aria-selected={tab.id === activeLiveId && !viewingEvent}
          class="browser-tab"
          class:active={tab.id === activeLiveId && !viewingEvent}
          on:click={() => selectLiveTab(tab.id)}
          title={tab.url}
        >
          <Globe size={12} />
          <span class="tab-title">{tab.title || extractDomain(tab.url)}</span>
          {#if tab.aiControlled && !userControl}
            <span class="tab-ai-dot" title={tab.aiStatusMessage || t("Piloté par l'IA", "AI driven")}>●</span>
          {/if}
          <span
            class="tab-close"
            role="button"
            tabindex="0"
            aria-label={t("Fermer l'onglet", "Close tab")}
            on:click|stopPropagation={() => void handleCloseTab(tab.id)}
            on:keydown|stopPropagation={(e) => e.key === "Enter" && void handleCloseTab(tab.id)}
          >
            <X size={12} />
          </span>
        </button>
      {/each}
      {#each eventTabs as tab (tab.url)}
        <button
          type="button"
          role="tab"
          aria-selected={selectedEventUrl === tab.url}
          class="browser-tab event"
          class:active={selectedEventUrl === tab.url}
          on:click={() => selectEventTab(tab.url)}
          title={tab.url}
        >
          <Bot size={12} />
          <span class="tab-title">{tab.title}</span>
        </button>
      {/each}
    </div>
    <button type="button" class="icon-btn" on:click={() => void handleNewTab()} title={t("Nouvel onglet", "New tab")} disabled={acting}>
      <Plus size={14} />
    </button>
  </div>

  <!-- Barre d'adresse + contrôle -->
  <div class="nav-bar">
    <button type="button" class="icon-btn" title={t("Actualiser", "Reload")} on:click={() => activeLiveId && void runAct(t("Actualiser", "Reload"), () => liveNavigate(activeLiveId as string, addressInput))} disabled={!activeLiveId || acting}>
      <RotateCw size={14} />
    </button>
    <div class="url-box">
      <Lock size={12} />
      <input
        type="text"
        bind:value={addressInput}
        placeholder="https://…"
        on:keydown={(e) => e.key === "Enter" && void handleGo()}
        aria-label={t("Barre d'adresse", "Address bar")}
      />
    </div>
    <button type="button" class="go-btn" on:click={() => void handleGo()} disabled={acting}>{t("Aller", "Go")}</button>
    {#if userControl}
      <button type="button" class="control-btn ai" on:click={handleReleaseToAi}>
        <Bot size={13} /><span>{t("Rendre à l'IA", "Give back to AI")}</span>
      </button>
    {:else}
      <button type="button" class="control-btn user" on:click={handleTakeControl} title={t("Suspendre le pilotage IA et prendre la main", "Pause AI driving and take over")}>
        <Hand size={13} /><span>{t("Prendre le contrôle", "Take control")}</span>
      </button>
    {/if}
  </div>

  {#if userControl}
    <div class="control-banner user">
      <Hand size={14} /><span>{t("Vous pilotez : cliquez, tapez, faites défiler. L'IA est en pause visuelle sur cet onglet.", "You are driving: click, type, scroll. AI is visually paused on this tab.")}</span>
    </div>
  {:else if activeLiveTab?.aiControlled}
    <div class="control-banner ai">
      <Bot size={14} /><span>{activeLiveTab.aiStatusMessage || t("L'IA pilote cet onglet.", "AI is driving this tab.")}</span>
    </div>
  {/if}

  <!-- Viewport -->
  <div class="viewport">
    {#if booting}
      <div class="viewport-empty"><p>{t("Démarrage du navigateur…", "Starting browser…")}</p></div>
    {:else if backendError && liveTabs.length === 0 && eventTabs.length === 0}
      <div class="viewport-empty">
        <AlertCircle size={20} />
        <p>{backendError}</p>
        <p class="hint">{t("Vérifiez que Chrome, Edge ou Chromium est installé, ou définissez ARO_CHROME_EXECUTABLE.", "Make sure Chrome, Edge or Chromium is installed, or set ARO_CHROME_EXECUTABLE.")}</p>
      </div>
    {:else if viewingEvent && currentEvent}
      <div class="reader-view">
        <div class="reader-header">
          <h2>{currentEvent.title}</h2>
          <span class="reader-url">{currentEvent.url}</span>
          {#if currentEvent.stepTitle}<span class="reader-step">{currentEvent.stepTitle}</span>{/if}
          <div class="reader-tools">
            {#if onSendToAi}
              <button type="button" class="apple-btn secondary small" on:click={() => onSendToAi?.(currentEvent?.url ?? "", currentEvent?.title ?? "")}>
                {t("Envoyer à l'IA", "Send to AI")}
              </button>
            {/if}
            <a href={currentEvent.url} target="_blank" rel="noopener noreferrer" class="apple-btn primary small">
              <ExternalLink size={12} /><span>{t("Ouvrir externe", "Open external")}</span>
            </a>
          </div>
        </div>
        <div class="reader-body">
          {#if currentEvent.content}
            <div class="extracted-article-text">{currentEvent.content}</div>
          {:else}
            <p class="hint">{t("Contenu en cours d'extraction par l'agent…", "Content being extracted by the agent…")}</p>
          {/if}
        </div>
      </div>
    {:else if activeLiveTab}
      <div class="live-view">
        {#if frameBase64}
          <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
          <img
            bind:this={frameImg}
            class="live-frame"
            src={`data:${frameMime};base64,${frameBase64}`}
            alt={activeLiveTab.title}
            on:click={handleFrameClick}
            on:wheel={handleFrameWheel}
            draggable="false"
          />
        {:else}
          <div class="viewport-empty"><p>{t("Chargement de la page…", "Loading page…")}</p></div>
        {/if}
        {#if acting}
          <div class="acting-veil"><span>{t("Action en cours…", "Working…")}</span></div>
        {/if}
      </div>
    {:else}
      <div class="viewport-empty">
        <p>{t("Aucun onglet. Ouvrez-en un pour commencer.", "No tabs. Open one to start.")}</p>
        <button type="button" class="apple-btn primary small" on:click={() => void handleNewTab()}>{t("Nouvel onglet", "New tab")}</button>
      </div>
    {/if}
  </div>

  <!-- Interactions -->
  {#if activeLiveTab && !viewingEvent}
    <div class="interact-bar">
      <div class="type-row">
        <input
          type="text"
          bind:value={typeInput}
          placeholder={t("Taper dans le champ focalisé… (Entrée pour envoyer)", "Type into the focused field… (Enter to send)")}
          on:keydown={(e) => e.key === "Enter" && void handleTypeSend()}
          aria-label={t("Saisie clavier", "Keyboard input")}
        />
        <button type="button" class="apple-btn secondary small" on:click={() => void handleTypeSend()} disabled={acting}>{t("Taper", "Type")}</button>
      </div>
      {#if refs.length > 0}
        <div class="refs-row" aria-label={t("Éléments cliquables", "Clickable elements")}>
          <span class="refs-label"><MousePointerClick size={12} /> {t("Cliquer :", "Click:")}</span>
          {#each refs as el}
            <button type="button" class="ref-chip" on:click={() => void handleRefClick(el.id)} title={`${el.role} — ${el.name}`}>
              {el.id} · {(el.name || el.role).slice(0, 28)}
            </button>
          {/each}
        </div>
      {/if}
      {#if passwordRef && usernameRef && $vaultAccounts.length > 0}
        <div class="autofill-row">
          <Key size={13} />
          <select bind:value={autofillAccount} aria-label={t("Compte du coffre", "Vault account")}>
            <option value="">{t("Choisir un compte…", "Choose an account…")}</option>
            {#each $vaultAccounts as account}
              <option value={account}>{account}</option>
            {/each}
          </select>
          <button type="button" class="apple-btn secondary small" on:click={() => void handleAutofill()} disabled={!autofillAccount || acting}>
            {t("Remplir depuis le coffre", "Fill from vault")}
          </button>
          {#if autofillDone}
            <span class="autofill-ok">✓ {autofillDone}</span>
          {/if}
        </div>
      {/if}
      {#if snapshot?.textExcerpt}
        <details class="snapshot-text">
          <summary>{t("Texte extrait", "Extracted text")}</summary>
          <div class="extracted-article-text">{snapshot.textExcerpt.slice(0, 4000)}</div>
        </details>
      {/if}
    </div>
  {/if}

  <!-- Pied : recherche rapide -->
  <div class="browser-footer">
    <div class="quick-chips">
      <span class="chips-label">{t("Recherche rapide :", "Quick search:")}</span>
      <button type="button" class="quick-chip" on:click={() => { addressInput = "https://github.com"; void handleGo(); }}>GitHub</button>
      <button type="button" class="quick-chip" on:click={() => { addressInput = "https://duckduckgo.com"; void handleGo(); }}>DuckDuckGo</button>
      <button type="button" class="quick-chip" on:click={() => { addressInput = "https://en.wikipedia.org"; void handleGo(); }}>Wikipedia</button>
      <button type="button" class="quick-chip" on:click={() => { addressInput = "https://news.ycombinator.com"; void handleGo(); }}>HackerNews</button>
    </div>
    <div class="browser-meta-info">
      {#if backendError}
        <span class="engine-off" title={backendError}>● {t("Moteur arrêté", "Engine off")}</span>
      {:else}
        <span class="engine-on">● {t("Chromium live", "Live Chromium")}</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .browser-workspace { display: flex; flex-direction: column; height: 100%; width: 100%; background: #18181b; color: #f4f4f5; overflow: hidden; outline: none; }
  .tabs-bar { display: flex; align-items: center; gap: 6px; padding: 8px 8px 4px; background: #101013; }
  .tabs-list { display: flex; gap: 6px; overflow-x: auto; flex: 1; }
  .browser-tab { display: inline-flex; align-items: center; gap: 6px; max-width: 220px; padding: 6px 8px; border-radius: 8px; background: #232329; border: 1px solid transparent; color: #d4d4d8; font-size: 12px; cursor: pointer; white-space: nowrap; }
  .browser-tab.active { background: #2e2e36; border-color: #52525b; color: #fff; }
  .browser-tab.event { border-style: dashed; }
  .tab-title { overflow: hidden; text-overflow: ellipsis; }
  .tab-ai-dot { color: #c084fc; font-size: 10px; }
  .tab-close { display: inline-flex; border-radius: 4px; padding: 2px; }
  .tab-close:hover { background: #52525b; }
  .icon-btn { display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: 8px; background: transparent; border: 0; color: #d4d4d8; cursor: pointer; }
  .icon-btn:hover { background: #2e2e36; }
  .icon-btn:disabled { opacity: 0.4; cursor: default; }
  .nav-bar { display: flex; align-items: center; gap: 8px; padding: 6px 8px; background: #101013; }
  .url-box { display: flex; align-items: center; gap: 6px; flex: 1; background: #232329; border-radius: 8px; padding: 6px 10px; }
  .url-box input { flex: 1; background: transparent; border: 0; color: #fff; font-size: 12px; outline: none; }
  .go-btn, .control-btn { border: 0; border-radius: 8px; padding: 7px 12px; font-size: 12px; font-weight: 600; cursor: pointer; display: inline-flex; align-items: center; gap: 6px; }
  .go-btn { background: #2563eb; color: #fff; }
  .control-btn.user { background: #14532d; color: #bbf7d0; }
  .control-btn.ai { background: #581c87; color: #e9d5ff; }
  .control-banner { display: flex; align-items: center; gap: 8px; padding: 6px 12px; font-size: 12px; }
  .control-banner.user { background: rgba(20, 83, 45, 0.5); color: #bbf7d0; }
  .control-banner.ai { background: rgba(88, 28, 135, 0.5); color: #e9d5ff; }
  .viewport { flex: 1; overflow: auto; position: relative; background: #0c0c0e; }
  .viewport-empty { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; height: 100%; color: #a1a1aa; font-size: 13px; text-align: center; padding: 24px; }
  .viewport-empty .hint { font-size: 12px; opacity: 0.7; max-width: 420px; }
  .live-view { position: relative; min-height: 100%; }
  .live-frame { display: block; width: 100%; cursor: crosshair; user-select: none; }
  .acting-veil { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; background: rgba(0, 0, 0, 0.35); color: #fff; font-size: 13px; }
  .reader-view { padding: 20px; max-width: 760px; margin: 0 auto; }
  .reader-header h2 { font-size: 18px; margin: 0 0 4px; }
  .reader-url { display: block; font-size: 12px; color: #a1a1aa; word-break: break-all; }
  .reader-step { display: inline-block; margin-top: 6px; font-size: 12px; color: #c084fc; }
  .reader-tools { display: flex; gap: 8px; margin-top: 12px; }
  .reader-body { margin-top: 16px; font-size: 14px; line-height: 1.7; }
  .extracted-article-text { white-space: pre-wrap; font-size: 13px; line-height: 1.7; color: #e4e4e7; }
  .interact-bar { border-top: 1px solid #27272a; padding: 8px; display: flex; flex-direction: column; gap: 8px; background: #101013; max-height: 42%; overflow-y: auto; }
  .type-row { display: flex; gap: 8px; }
  .type-row input { flex: 1; background: #232329; border: 0; border-radius: 8px; color: #fff; font-size: 12px; padding: 8px 10px; outline: none; }
  .refs-row { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .refs-label { display: inline-flex; align-items: center; gap: 4px; font-size: 12px; color: #a1a1aa; }
  .ref-chip { background: #27272a; border: 1px solid #3f3f46; color: #e4e4e7; border-radius: 999px; font-size: 11px; padding: 4px 10px; cursor: pointer; }
  .ref-chip:hover { background: #3f3f46; }
  .autofill-row { display: flex; align-items: center; gap: 8px; font-size: 12px; color: #a1a1aa; }
  .autofill-row select { background: #232329; color: #fff; border: 0; border-radius: 8px; padding: 6px 8px; font-size: 12px; }
  .autofill-ok { color: #4ade80; }
  .snapshot-text { font-size: 12px; color: #a1a1aa; }
  .snapshot-text summary { cursor: pointer; }
  .browser-footer { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 6px 8px; background: #101013; border-top: 1px solid #27272a; }
  .quick-chips { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .chips-label { font-size: 11px; color: #71717a; }
  .quick-chip { background: #232329; border: 0; color: #d4d4d8; border-radius: 999px; font-size: 11px; padding: 4px 10px; cursor: pointer; display: inline-flex; align-items: center; gap: 4px; }
  .quick-chip:hover { background: #3f3f46; }
  .browser-meta-info { display: flex; align-items: center; font-size: 11px; }
  .engine-on { color: #4ade80; }
  .engine-off { color: #f87171; }
  .apple-btn { border: 0; border-radius: 8px; padding: 7px 12px; font-size: 12px; font-weight: 600; cursor: pointer; display: inline-flex; align-items: center; gap: 6px; text-decoration: none; }
  .apple-btn.primary { background: #2563eb; color: #fff; }
  .apple-btn.secondary { background: #27272a; color: #e4e4e7; }
  .apple-btn.small { padding: 5px 10px; font-size: 11px; }
</style>
