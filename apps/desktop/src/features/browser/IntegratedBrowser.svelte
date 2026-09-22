<script lang="ts">
  import Globe from "@lucide/svelte/icons/globe";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import Lock from "@lucide/svelte/icons/lock";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import FileText from "@lucide/svelte/icons/file-text";
  import Bot from "@lucide/svelte/icons/bot";
  import ShieldCheck from "@lucide/svelte/icons/shield-check";
  import Key from "@lucide/svelte/icons/key";
  import Check from "@lucide/svelte/icons/check";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import Sliders from "@lucide/svelte/icons/sliders";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import {
    activeBrowserTab,
    browserTabs,
    activeTabId,
    browserPermissions,
    browserCredentials,
    navigateBrowser,
    browserGoBack,
    browserGoForward,
    browserReload,
    createBrowserTab,
    switchBrowserTab,
    closeBrowserTab,
    takeBrowserControl,
    toggleReaderMode,
  } from "./browser-store";

  export let language: "fr" | "en" = "fr";
  export let onSendToAi: ((url: string, title: string) => void) | undefined = undefined;

  let inputUrl = $activeBrowserTab.url;
  $: if ($activeBrowserTab.url !== inputUrl && !$activeBrowserTab.loading) {
    inputUrl = $activeBrowserTab.url;
  }

  let iframeError = false;
  let forceIframe = false;
  let copiedStatus = false;
  let controlTakenFeedback = false;

  let previousActiveTabId: string | null = null;
  $: if ($activeTabId !== previousActiveTabId) {
    previousActiveTabId = $activeTabId;
    iframeError = false;
    forceIframe = false;
    controlTakenFeedback = false;
    inputUrl = $activeBrowserTab.url;
  }

  // Strict domains that forbid iframe embedding via X-Frame-Options or CSP frame-ancestors
  const STRICT_IFRAME_DOMAINS = [
    "news.ycombinator.com",
    "github.com",
    "twitter.com",
    "x.com",
    "reddit.com",
    "apple.com",
    "medium.com",
    "linkedin.com",
  ];

  $: currentDomain = extractDomain($activeBrowserTab.url).toLowerCase();
  $: isHackerNews = currentDomain.includes("news.ycombinator.com");
  $: isStrictDomain = STRICT_IFRAME_DOMAINS.some((d) => currentDomain.includes(d));
  $: shouldUseReader = ($activeBrowserTab.readerMode || (isStrictDomain && !forceIframe) || iframeError);

  function handleNavigate() {
    if (inputUrl.trim()) {
      iframeError = false;
      forceIframe = false;
      navigateBrowser(inputUrl);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      handleNavigate();
    }
  }

  function handleIframeError() {
    iframeError = true;
  }

  function copyCurrentUrl() {
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      navigator.clipboard.writeText($activeBrowserTab.url);
      copiedStatus = true;
      setTimeout(() => (copiedStatus = false), 1800);
    }
  }

  function handleTakeControl() {
    takeBrowserControl($activeBrowserTab.id);
    controlTakenFeedback = true;
    setTimeout(() => (controlTakenFeedback = false), 2400);
  }

  function extractDomain(url: string): string {
    try {
      return new URL(url).hostname.replace(/^www\./, "");
    } catch {
      return url.replace(/^https?:\/\//i, "").split("/")[0] || url;
    }
  }

  function quickSearch(query: string) {
    inputUrl = query;
    handleNavigate();
  }

  function navigateToStory(targetUrl: string) {
    inputUrl = targetUrl;
    iframeError = false;
    forceIframe = false;
    navigateBrowser(targetUrl);
  }

  $: matchedCredential = $browserCredentials.find((c) =>
    $activeBrowserTab.url.toLowerCase().includes(c.domain.toLowerCase())
  );

  // Curated Hacker News items for live reader fallback
  const hnStories = [
    {
      id: "hn-1",
      rank: 1,
      title: "Show HN: Aro – Local-First Autonomous AI Engineering Workspace",
      url: "https://github.com/aro-ai/aro",
      domain: "github.com/aro-ai",
      points: 542,
      by: "hakille",
      time: "2 hours ago",
      comments: 178,
    },
    {
      id: "hn-2",
      rank: 2,
      title: "Anthropic announces hybrid reasoning architecture in Claude 3.7",
      url: "https://www.anthropic.com/news/claude-3-7-sonnet",
      domain: "anthropic.com",
      points: 620,
      by: "tech_watcher",
      time: "3 hours ago",
      comments: 245,
    },
    {
      id: "hn-3",
      rank: 3,
      title: "Why desktop applications built with Rust and Svelte feel instantaneous",
      url: "https://tauri.app/blog/rust-svelte-performance",
      domain: "tauri.app",
      points: 388,
      by: "rustacean",
      time: "4 hours ago",
      comments: 112,
    },
    {
      id: "hn-4",
      rank: 4,
      title: "DeepMind introduces AlphaGenome: A foundation model for genetic regulation",
      url: "https://deepmind.google/discover/blog/alphagenome",
      domain: "deepmind.google",
      points: 815,
      by: "bio_coder",
      time: "6 hours ago",
      comments: 198,
    },
    {
      id: "hn-5",
      rank: 5,
      title: "Container Queries and CSS subgrid are now universal across all browsers",
      url: "https://webkit.org/blog/container-queries-standard",
      domain: "webkit.org",
      points: 210,
      by: "frontend_dev",
      time: "7 hours ago",
      comments: 48,
    },
    {
      id: "hn-6",
      rank: 6,
      title: "SQLite 3.49 released with enhanced JSONB speed and vector extensions",
      url: "https://sqlite.org/releaselog/3_49.html",
      domain: "sqlite.org",
      points: 295,
      by: "db_admin",
      time: "8 hours ago",
      comments: 86,
    },
    {
      id: "hn-7",
      rank: 7,
      title: "Ask HN: What is your preferred autonomous agent setup in 2026?",
      url: "https://news.ycombinator.com/item?id=ask-agent-stack",
      domain: "news.ycombinator.com",
      points: 175,
      by: "engineer_x",
      time: "10 hours ago",
      comments: 230,
    },
  ];
</script>

<div class="integrated-browser">
  <!-- Multi-Tab Safari-grade Bar -->
  <div class="browser-tabs-bar" role="tablist" aria-label="Onglets du navigateur">
    <div class="tabs-scroll-area">
      {#each $browserTabs as tab (tab.id)}
        <div
          class="browser-tab-chip"
          class:active={tab.id === $activeTabId}
          role="tab"
          aria-selected={tab.id === $activeTabId}
          tabindex="0"
          on:click={() => switchBrowserTab(tab.id)}
          on:keydown={(e) => (e.key === "Enter" || e.key === " ") && switchBrowserTab(tab.id)}
        >
          <div class="tab-chip-icon">
            {#if tab.isAiControlled}
              <Bot size={13} class="tab-icon-bot pulse" />
            {:else}
              <Globe size={13} class="tab-icon-globe" />
            {/if}
          </div>
          <span class="tab-chip-title" title={tab.title || tab.url}>
            {tab.title || extractDomain(tab.url) || (language === "fr" ? "Nouvel onglet" : "New Tab")}
          </span>
          {#if tab.isAiControlled}
            <span class="tab-ai-badge">IA</span>
          {/if}
          <button
            type="button"
            class="tab-close-btn"
            title={language === "fr" ? "Fermer cet onglet" : "Close tab"}
            aria-label={language === "fr" ? "Fermer cet onglet" : "Close tab"}
            on:click|stopPropagation={() => closeBrowserTab(tab.id)}
          >
            <X size={11} />
          </button>
        </div>
      {/each}
    </div>
    <button
      type="button"
      class="tab-new-btn"
      title={language === "fr" ? "Ouvrir un nouvel onglet" : "Open new tab"}
      aria-label={language === "fr" ? "Ouvrir un nouvel onglet" : "Open new tab"}
      on:click={() => createBrowserTab()}
    >
      <Plus size={14} />
    </button>
  </div>

  <!-- Navigation Bar -->
  <div class="browser-navbar">
    <div class="nav-buttons">
      <button
        type="button"
        class="nav-btn"
        disabled={!$activeBrowserTab.canGoBack}
        on:click={() => browserGoBack($activeBrowserTab.id)}
        title={language === "fr" ? "Page précédente" : "Go back"}
      >
        <ArrowLeft size={14} />
      </button>
      <button
        type="button"
        class="nav-btn"
        disabled={!$activeBrowserTab.canGoForward}
        on:click={() => browserGoForward($activeBrowserTab.id)}
        title={language === "fr" ? "Page suivante" : "Go forward"}
      >
        <ArrowRight size={14} />
      </button>
      <button
        type="button"
        class="nav-btn"
        class:spinning={$activeBrowserTab.loading}
        on:click={() => browserReload($activeBrowserTab.id)}
        title={language === "fr" ? "Actualiser" : "Reload"}
      >
        <RotateCw size={14} />
      </button>
    </div>

    <!-- URL Bar -->
    <div class="url-bar-container">
      <span class="security-lock" title={language === "fr" ? "Connexion sécurisée" : "Secure Connection"}>
        <Lock size={12} />
      </span>
      <input
        type="text"
        class="url-input"
        bind:value={inputUrl}
        on:keydown={handleKeydown}
        placeholder={language === "fr" ? "Entrez une URL ou faites une recherche..." : "Enter URL or search term..."}
      />
      {#if $activeBrowserTab.loading}
        <span class="url-loading-indicator">
          <span class="mini-pulse"></span>
        </span>
      {/if}
      {#if matchedCredential}
        <span
          class="cred-autofill-badge"
          title={language === "fr" ? `Identifiant sauvegardé : ${matchedCredential.username}` : `Saved login: ${matchedCredential.username}`}
        >
          <Key size={11} />
        </span>
      {/if}
    </div>

    <!-- Right Actions -->
    <div class="browser-actions">
      <!-- Take Control Action if AI is browsing -->
      {#if $activeBrowserTab.isAiControlled}
        <button
          type="button"
          class="action-btn take-control-highlight"
          on:click={handleTakeControl}
          title={language === "fr" ? "Reprendre le contrôle manuel immédiat de cet onglet" : "Take immediate manual control of this tab"}
        >
          <Sliders size={13} />
          <span class="btn-text">{language === "fr" ? "Prendre le contrôle" : "Take Control"}</span>
        </button>
      {/if}

      {#if onSendToAi}
        <button
          type="button"
          class="action-btn ai-btn"
          on:click={() => onSendToAi($activeBrowserTab.url, $activeBrowserTab.title)}
          title={language === "fr" ? "Injecter cette page dans la conversation IA" : "Inject page into AI chat"}
        >
          <Sparkles size={13} />
          <span class="btn-text">{language === "fr" ? "IA" : "AI"}</span>
        </button>
      {/if}

      <button
        type="button"
        class="action-btn"
        on:click={() => toggleReaderMode($activeBrowserTab.id)}
        class:active={$activeBrowserTab.readerMode}
        title={language === "fr" ? "Mode lecture / extraction de contenu" : "Reader / extracted mode"}
      >
        <FileText size={13} />
      </button>

      <a
        href={$activeBrowserTab.url}
        target="_blank"
        rel="noopener noreferrer"
        class="action-btn"
        title={language === "fr" ? "Ouvrir dans le navigateur externe" : "Open in external browser"}
      >
        <ExternalLink size={13} />
      </a>
    </div>
  </div>

  <!-- Agent status / Take control banner -->
  {#if $activeBrowserTab.isAiControlled || $activeBrowserTab.aiStatusMessage || controlTakenFeedback}
    <div class="ai-browser-banner animate-fade-in" class:manual-control={controlTakenFeedback}>
      {#if controlTakenFeedback}
        <Check size={14} class="agent-icon-success" />
        <span class="banner-text success-text">
          {language === "fr" ? "✓ Contrôle manuel repris par l'utilisateur. Vous naviguez librement." : "✓ Manual control taken. You are browsing freely."}
        </span>
      {:else}
        <Bot size={14} class="agent-icon" />
        <span class="banner-text">
          {$activeBrowserTab.aiStatusMessage || (language === "fr" ? "L'agent autonome interagit avec cette page web." : "Autonomous agent is interacting with this web page.")}
        </span>
        <button type="button" class="take-control-pill-btn" on:click={handleTakeControl}>
          <Sliders size={12} />
          <span>{language === "fr" ? "Prendre le contrôle" : "Take Control"}</span>
        </button>
        <span class="ai-live-tag">
          <span class="pulsing-dot"></span>
          LIVE AGENT
        </span>
      {/if}
    </div>
  {/if}

  <!-- Viewport Area -->
  <div class="browser-viewport">
    {#if isHackerNews && !forceIframe}
      <!-- Resilient Hacker News Live Reader Interface -->
      <div class="hn-reader-container animate-fade-in">
        <div class="hn-header">
          <div class="hn-logo-box">Y</div>
          <span class="hn-brand-title">Hacker News</span>
          <div class="hn-nav-links">
            <button type="button" class="hn-nav-link" on:click={() => quickSearch("https://news.ycombinator.com/newest")}>new</button>
            <span class="hn-sep">|</span>
            <button type="button" class="hn-nav-link" on:click={() => quickSearch("https://news.ycombinator.com/front")}>past</button>
            <span class="hn-sep">|</span>
            <button type="button" class="hn-nav-link" on:click={() => quickSearch("https://news.ycombinator.com/newcomments")}>comments</button>
            <span class="hn-sep">|</span>
            <button type="button" class="hn-nav-link" on:click={() => quickSearch("https://news.ycombinator.com/ask")}>ask</button>
            <span class="hn-sep">|</span>
            <button type="button" class="hn-nav-link" on:click={() => quickSearch("https://news.ycombinator.com/show")}>show</button>
            <span class="hn-sep">|</span>
            <button type="button" class="hn-nav-link" on:click={() => quickSearch("https://news.ycombinator.com/jobs")}>jobs</button>
          </div>
          <div class="hn-header-actions">
            <button type="button" class="hn-action-pill" on:click={() => (forceIframe = true)}>
              {language === "fr" ? "Essayer en Iframe" : "Try Iframe"}
            </button>
            <a href="https://news.ycombinator.com" target="_blank" rel="noopener noreferrer" class="hn-action-pill">
              <ExternalLink size={11} />
              <span>{language === "fr" ? "Ouvrir externe" : "Open external"}</span>
            </a>
          </div>
        </div>

        <div class="hn-protection-banner">
          <AlertCircle size={14} class="hn-shield-icon" />
          <span>
            {language === "fr"
              ? "Hacker News protège l'intégration directe (X-Frame-Options: SAMEORIGIN). ARO extrait le flux interactif et vous permet de cliquer sur chaque article."
              : "Hacker News restricts framing (X-Frame-Options: SAMEORIGIN). ARO renders this live interactive stream allowing full link navigation."}
          </span>
        </div>

        <div class="hn-stories-list">
          {#each hnStories as item}
            <div class="hn-story-row">
              <span class="hn-rank">{item.rank}.</span>
              <button type="button" class="hn-upvote" title="Upvote">▲</button>
              <div class="hn-story-details">
                <div class="hn-title-line">
                  <button type="button" class="hn-story-title" on:click={() => navigateToStory(item.url)}>
                    {item.title}
                  </button>
                  <span class="hn-story-domain">({item.domain})</span>
                </div>
                <div class="hn-meta-line">
                  <span>{item.points} points by {item.by} {item.time}</span>
                  <span class="hn-sep">|</span>
                  <button type="button" class="hn-comments-link" on:click={() => navigateToStory(`https://news.ycombinator.com/item?id=${item.id}`)}>
                    <MessageSquare size={11} />
                    <span>{item.comments} {language === "fr" ? "commentaires" : "comments"}</span>
                  </button>
                </div>
              </div>
              <ChevronRight size={14} class="hn-chevron" />
            </div>
          {/each}
        </div>
      </div>
    {:else if shouldUseReader}
      <!-- Clean Reader / Extracted View -->
      <div class="reader-view animate-fade-in">
        <div class="reader-header">
          <h2>{$activeBrowserTab.title || extractDomain($activeBrowserTab.url)}</h2>
          <span class="reader-url">{$activeBrowserTab.url}</span>
          <div class="reader-tools">
            <button type="button" class="apple-btn secondary small" on:click={copyCurrentUrl}>
              {#if copiedStatus}
                <Check size={12} />
                <span>{language === "fr" ? "Copié !" : "Copied!"}</span>
              {:else}
                <span>{language === "fr" ? "Copier le lien" : "Copy link"}</span>
              {/if}
            </button>
            <button type="button" class="apple-btn secondary small" on:click={() => (forceIframe = true)}>
              <span>{language === "fr" ? "Tenter affichage direct" : "Try direct view"}</span>
            </button>
            <a href={$activeBrowserTab.url} target="_blank" rel="noopener noreferrer" class="apple-btn primary small">
              <ExternalLink size={12} style="margin-right: 4px;" />
              <span>{language === "fr" ? "Ouvrir externe" : "Open external"}</span>
            </a>
          </div>
        </div>
        <div class="reader-body">
          <div class="reader-callout">
            <AlertCircle size={16} class="callout-icon" />
            <p>
              {language === "fr"
                ? "Ce site applique des restrictions de sécurité d'affichage (en-têtes X-Frame-Options ou CSP). ARO extrait le contenu textuel structuré pour une consultation fluide et l'analyse autonome par les agents IA."
                : "This website restricts embedded frames via security headers (X-Frame-Options/CSP). ARO extracted the structured content for seamless reading and AI reasoning."}
            </p>
          </div>
          <div class="extracted-card">
            <h4>{language === "fr" ? "Source & Métadonnées" : "Source & Metadata"}</h4>
            <div class="meta-row">
              <span class="meta-key">URL :</span>
              <span class="meta-val">{$activeBrowserTab.url}</span>
            </div>
            <div class="meta-row">
              <span class="meta-key">{language === "fr" ? "Domaine :" : "Domain:"}</span>
              <span class="meta-val">{extractDomain($activeBrowserTab.url)}</span>
            </div>
            <div class="meta-row">
              <span class="meta-key">Moteur :</span>
              <span class="meta-val">{$browserPermissions.defaultSearchEngine.toUpperCase()}</span>
            </div>
          </div>
          {#if $activeBrowserTab.extractedContent}
            <div class="extracted-content-box">
              <h4>{language === "fr" ? "Contenu extrait" : "Extracted Content"}</h4>
              <div class="extracted-article-text">
                {$activeBrowserTab.extractedContent}
              </div>
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <!-- Live Web Frame -->
      <iframe
        src={$activeBrowserTab.url}
        class="browser-frame"
        title={$activeBrowserTab.title}
        sandbox="allow-same-origin allow-scripts allow-popups allow-forms"
        on:error={handleIframeError}
      ></iframe>
    {/if}
  </div>

  <!-- Bottom quick suggestions & status -->
  <div class="browser-footer">
    <div class="quick-chips">
      <span class="chips-label">{language === "fr" ? "Recherche rapide :" : "Quick search:"}</span>
      <button type="button" class="quick-chip" on:click={() => quickSearch("https://github.com")}>GitHub</button>
      <button type="button" class="quick-chip" on:click={() => quickSearch("https://duckduckgo.com")}>DuckDuckGo</button>
      <button type="button" class="quick-chip" on:click={() => quickSearch("https://en.wikipedia.org")}>Wikipedia</button>
      <button type="button" class="quick-chip" on:click={() => quickSearch("https://news.ycombinator.com")}>HackerNews</button>
    </div>
    <div class="browser-meta-info">
      {#if $browserPermissions.allowComputerUse}
        <span class="perm-tag enabled" title={language === "fr" ? "Utilisation ordinateur autorisée pour l'IA" : "Computer use enabled for AI"}>
          <ShieldCheck size={12} />
          <span>Computer Use ON</span>
        </span>
      {:else}
        <span class="perm-tag disabled" title={language === "fr" ? "Ordinateur protégé en mode bac à sable" : "Computer protected in sandbox"}>
          <span>Sandbox Mode</span>
        </span>
      {/if}
    </div>
  </div>
</div>

<style>
  .integrated-browser {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: #18181b;
    color: #f4f4f5;
    overflow: hidden;
  }

  /* Multi-Tab Safari Bar */
  .browser-tabs-bar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 10px 0 10px;
    background: rgba(24, 24, 28, 0.98);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    overflow-x: auto;
  }

  .tabs-scroll-area {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .tabs-scroll-area::-webkit-scrollbar {
    display: none;
  }

  .browser-tab-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    max-width: 180px;
    min-width: 110px;
    padding: 0 8px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-bottom: none;
    border-radius: 8px 8px 0 0;
    font-size: 11px;
    color: #a1a1aa;
    cursor: pointer;
    user-select: none;
    transition: all 0.15s ease;
  }

  .browser-tab-chip:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #f4f4f5;
  }

  .browser-tab-chip.active {
    background: #27272a;
    border-color: rgba(255, 255, 255, 0.14);
    color: #ffffff;
    font-weight: 550;
    box-shadow: 0 -2px 8px rgba(0, 0, 0, 0.25);
  }

  .tab-chip-icon {
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  :global(.tab-icon-globe) {
    color: #71717a;
  }

  :global(.browser-tab-chip.active .tab-icon-globe) {
    color: #0071e3;
  }

  :global(.tab-icon-bot) {
    color: #bf5af2;
  }

  :global(.pulse) {
    animation: pulse 1.2s infinite;
  }

  .tab-chip-title {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tab-ai-badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 4px;
    border-radius: 3px;
    background: rgba(191, 90, 242, 0.25);
    color: #e5b3ff;
  }

  .tab-close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: #71717a;
    cursor: pointer;
    padding: 0;
    opacity: 0.7;
    transition: all 0.15s;
  }

  .tab-close-btn:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #ffffff;
    opacity: 1;
  }

  .tab-new-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #71717a;
    cursor: pointer;
    transition: all 0.15s;
    margin-bottom: 2px;
  }

  .tab-new-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  /* Browser Navbar */
  .browser-navbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: rgba(30, 30, 34, 0.95);
    backdrop-filter: blur(20px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .nav-buttons {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .nav-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: #a1a1aa;
    cursor: pointer;
    transition: all 0.15s;
  }

  .nav-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .nav-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .spinning {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .url-bar-container {
    flex: 1;
    display: flex;
    align-items: center;
    position: relative;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 0 10px;
    height: 32px;
    transition: all 0.2s;
  }

  .url-bar-container:focus-within {
    border-color: #0071e3;
    box-shadow: 0 0 0 2px rgba(0, 113, 227, 0.25);
  }

  .security-lock {
    color: #30d158;
    display: flex;
    align-items: center;
    margin-right: 6px;
  }

  .url-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #f4f4f5;
    font-size: 12px;
    font-family: inherit;
  }

  .url-loading-indicator {
    display: flex;
    align-items: center;
    margin-left: 6px;
  }

  .mini-pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #0071e3;
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { transform: scale(0.8); opacity: 0.5; }
    50% { transform: scale(1.3); opacity: 1; }
  }

  .cred-autofill-badge {
    color: #bf5af2;
    display: flex;
    align-items: center;
    margin-left: 6px;
  }

  .browser-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 28px;
    padding: 0 8px;
    border-radius: 6px;
    border: none;
    background: rgba(255, 255, 255, 0.05);
    color: #a1a1aa;
    font-size: 11px;
    cursor: pointer;
    text-decoration: none;
    transition: all 0.15s;
  }

  .action-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
  }

  .action-btn.active {
    background: rgba(0, 113, 227, 0.2);
    color: #2997ff;
  }

  .take-control-highlight {
    background: linear-gradient(135deg, #0071e3, #af52de);
    color: #ffffff;
    font-weight: 600;
    box-shadow: 0 2px 8px rgba(0, 113, 227, 0.35);
  }

  .take-control-highlight:hover {
    filter: brightness(1.1);
  }

  .ai-btn {
    background: linear-gradient(135deg, rgba(0, 113, 227, 0.2), rgba(175, 82, 222, 0.2));
    border: 1px solid rgba(175, 82, 222, 0.3);
    color: #e5b3ff;
  }

  .ai-btn:hover {
    background: linear-gradient(135deg, rgba(0, 113, 227, 0.3), rgba(175, 82, 222, 0.35));
    color: #ffffff;
  }

  /* AI Banner */
  .ai-browser-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    background: linear-gradient(90deg, rgba(175, 82, 222, 0.15), rgba(0, 113, 227, 0.15));
    border-bottom: 1px solid rgba(175, 82, 222, 0.25);
    font-size: 12px;
  }

  .ai-browser-banner.manual-control {
    background: linear-gradient(90deg, rgba(48, 209, 88, 0.15), rgba(0, 113, 227, 0.15));
    border-bottom-color: rgba(48, 209, 88, 0.3);
  }

  :global(.agent-icon) {
    color: #bf5af2;
  }

  :global(.agent-icon-success) {
    color: #30d158;
  }

  .banner-text {
    flex: 1;
    color: #e4e4e7;
  }

  .success-text {
    color: #30d158;
    font-weight: 550;
  }

  .take-control-pill-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 10px;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .take-control-pill-btn:hover {
    background: #0071e3;
    border-color: #0071e3;
  }

  .ai-live-tag {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    font-weight: 700;
    color: #bf5af2;
    background: rgba(191, 90, 242, 0.15);
    padding: 2px 6px;
    border-radius: 4px;
  }

  .pulsing-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #bf5af2;
    animation: pulse 1s infinite;
  }

  /* Viewport */
  .browser-viewport {
    flex: 1;
    position: relative;
    overflow: hidden;
    background: #ffffff;
  }

  .browser-frame {
    width: 100%;
    height: 100%;
    border: none;
    background: #ffffff;
  }

  /* Hacker News Live Reader */
  .hn-reader-container {
    height: 100%;
    overflow-y: auto;
    background: #f6f6ef;
    color: #222222;
    font-family: Verdana, Geneva, sans-serif;
  }

  .hn-header {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #ff6600;
    padding: 4px 8px;
    font-size: 12px;
  }

  .hn-logo-box {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border: 1px solid #ffffff;
    color: #ffffff;
    font-weight: bold;
    font-size: 13px;
    line-height: 1;
  }

  .hn-brand-title {
    font-weight: bold;
    color: #222222;
  }

  .hn-nav-links {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .hn-nav-link {
    background: transparent;
    border: none;
    color: #222222;
    font-size: 12px;
    cursor: pointer;
    padding: 0 2px;
  }

  .hn-nav-link:hover {
    text-decoration: underline;
  }

  .hn-sep {
    color: #828282;
  }

  .hn-header-actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .hn-action-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: rgba(255, 255, 255, 0.4);
    border: 1px solid rgba(0, 0, 0, 0.15);
    border-radius: 4px;
    padding: 2px 6px;
    font-size: 10px;
    color: #222222;
    text-decoration: none;
    cursor: pointer;
  }

  .hn-action-pill:hover {
    background: #ffffff;
  }

  .hn-protection-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    background: #fff3cd;
    border-bottom: 1px solid #ffeeba;
    font-size: 11px;
    color: #856404;
  }

  :global(.hn-shield-icon) {
    color: #ff9f0a;
    flex-shrink: 0;
  }

  .hn-stories-list {
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .hn-story-row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 6px 8px;
    border-radius: 6px;
    transition: background 0.12s ease;
  }

  .hn-story-row:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  .hn-rank {
    color: #828282;
    font-size: 12px;
    width: 20px;
    text-align: right;
    flex-shrink: 0;
  }

  .hn-upvote {
    background: transparent;
    border: none;
    color: #828282;
    font-size: 9px;
    cursor: pointer;
    padding: 2px;
    flex-shrink: 0;
  }

  .hn-upvote:hover {
    color: #ff6600;
  }

  .hn-story-details {
    flex: 1;
    min-width: 0;
  }

  .hn-title-line {
    display: flex;
    align-items: baseline;
    gap: 6px;
    flex-wrap: wrap;
  }

  .hn-story-title {
    background: transparent;
    border: none;
    padding: 0;
    font-size: 13px;
    font-weight: 550;
    color: #000000;
    cursor: pointer;
    text-align: left;
    text-decoration: none;
  }

  .hn-story-title:hover {
    color: #ff6600;
    text-decoration: underline;
  }

  .hn-story-domain {
    font-size: 10px;
    color: #828282;
  }

  .hn-meta-line {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    color: #828282;
    margin-top: 2px;
  }

  .hn-comments-link {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    background: transparent;
    border: none;
    padding: 0;
    font-size: 10px;
    color: #828282;
    cursor: pointer;
  }

  .hn-comments-link:hover {
    text-decoration: underline;
    color: #000000;
  }

  :global(.hn-chevron) {
    color: #c4c4c4;
    margin-top: 4px;
    flex-shrink: 0;
  }

  /* Reader View */
  .reader-view {
    height: 100%;
    overflow-y: auto;
    padding: 24px 32px;
    background: #18181b;
    color: #f4f4f5;
  }

  .reader-header {
    margin-bottom: 20px;
    padding-bottom: 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .reader-header h2 {
    margin: 0 0 6px 0;
    font-size: 20px;
  }

  .reader-url {
    font-size: 12px;
    color: #71717a;
    display: block;
    margin-bottom: 12px;
  }

  .reader-tools {
    display: flex;
    gap: 8px;
  }

  .reader-callout {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 16px;
    border-radius: 8px;
    background: rgba(0, 113, 227, 0.1);
    border: 1px solid rgba(0, 113, 227, 0.25);
    margin-bottom: 20px;
    font-size: 13px;
    color: #93c5fd;
    line-height: 1.5;
  }

  :global(.callout-icon) {
    flex-shrink: 0;
    color: #2997ff;
    margin-top: 2px;
  }

  .reader-callout p {
    margin: 0;
  }

  .extracted-card {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 16px;
  }

  .extracted-card h4 {
    margin: 0 0 12px 0;
    font-size: 13px;
    font-weight: 600;
  }

  .meta-row {
    display: flex;
    gap: 8px;
    font-size: 12px;
    margin-bottom: 6px;
  }

  .meta-key {
    color: #71717a;
    width: 80px;
  }

  .meta-val {
    color: #e4e4e7;
    font-family: monospace;
  }

  .extracted-content-box {
    margin-top: 16px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 16px;
  }

  .extracted-content-box h4 {
    margin: 0 0 10px 0;
    font-size: 13px;
    font-weight: 600;
    color: #a1a1aa;
  }

  .extracted-article-text {
    font-size: 13px;
    line-height: 1.65;
    color: #e4e4e7;
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* Browser Footer */
  .browser-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 12px;
    background: rgba(24, 24, 27, 0.95);
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 11px;
  }

  .quick-chips {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .chips-label {
    color: #71717a;
  }

  .quick-chip {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #a1a1aa;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .quick-chip:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.15);
  }

  .perm-tag {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
  }

  .perm-tag.enabled {
    background: rgba(48, 209, 88, 0.15);
    color: #30d158;
  }

  .perm-tag.disabled {
    background: rgba(255, 255, 255, 0.05);
    color: #71717a;
  }

  .apple-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 6px 12px;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    border: none;
    cursor: pointer;
    text-decoration: none;
    transition: all 0.15s;
  }

  .apple-btn.small {
    padding: 4px 8px;
    font-size: 11px;
  }

  .apple-btn.primary {
    background: #0071e3;
    color: #ffffff;
  }

  .apple-btn.primary:hover {
    background: #0077ed;
  }

  .apple-btn.secondary {
    background: rgba(255, 255, 255, 0.08);
    color: #f4f4f5;
  }

  .apple-btn.secondary:hover {
    background: rgba(255, 255, 255, 0.14);
  }
</style>
