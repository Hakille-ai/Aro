<script lang="ts">
  import Globe from "@lucide/svelte/icons/globe";
  import Lock from "@lucide/svelte/icons/lock";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Copy from "@lucide/svelte/icons/copy";
  import Check from "@lucide/svelte/icons/check";
  import FileText from "@lucide/svelte/icons/file-text";
  import FileSpreadsheet from "@lucide/svelte/icons/file-spreadsheet";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Search from "@lucide/svelte/icons/search";
  import Brain from "@lucide/svelte/icons/brain";
  import Eye from "@lucide/svelte/icons/eye";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronUp from "@lucide/svelte/icons/chevron-up";
  import GitCompare from "@lucide/svelte/icons/git-compare";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import Plug from "@lucide/svelte/icons/plug";
  import BrandLogo from "../../lib/BrandLogo.svelte";
  import type { AgentStep } from "../../lib/types";

  export let step: AgentStep;
  export let messageId: string = "";
  export let isExpanded: boolean = false;
  export let language: "fr" | "en" = "fr";
  export let theme: "light" | "dark" = "dark";
  export let onToggleDetails: () => void = () => {};

  $: void messageId;

  let copiedUrl = false;
  let copiedOutput = false;

  $: toolId = (step.input?.toolId || "").toLowerCase();
  $: stepTitle = (step.title || "").toLowerCase();

  $: isBrowser =
    toolId.includes("browser") ||
    stepTitle.includes("browser") ||
    stepTitle.includes("navigate") ||
    toolId.includes("web.page.read") ||
    toolId.includes("web.fetch") ||
    Boolean(step.input?.url) ||
    Boolean(step.output?.url);

  $: isSearch =
    toolId.includes("search") ||
    stepTitle.includes("search") ||
    stepTitle.includes("recherche");

  $: isMemory =
    toolId.startsWith("memory") ||
    stepTitle.includes("memory") ||
    stepTitle.includes("mémoire");

  $: isDocument =
    toolId.includes("document") ||
    toolId.includes("artifact") ||
    stepTitle.includes("document") ||
    Boolean(step.output?.format) ||
    Boolean(step.output?.artifact) ||
    Boolean(step.input?.format);

  $: isShell =
    toolId.includes("shell") ||
    toolId.includes("command") ||
    toolId.includes("code.execute") ||
    stepTitle.includes("terminal") ||
    stepTitle.includes("commande");

  $: isDiff =
    toolId.includes("diff") ||
    Boolean(step.input?.diff) ||
    Boolean(step.output?.diff);

  $: isConnector =
    toolId.includes("connector") ||
    toolId.includes("plugin") ||
    toolId.includes("mcp") ||
    stepTitle.includes("connector") ||
    stepTitle.includes("connecteur") ||
    stepTitle.includes("plugin") ||
    stepTitle.includes("listed") && stepTitle.includes("plugin");

  function getConnectorList(rawStep: any): any[] {
    if (!rawStep) return [];
    const candidates = [
      rawStep.output?.connectors,
      rawStep.output?.output?.connectors,
      rawStep.output?.result?.connectors,
      rawStep.output?.data?.connectors,
    ];
    for (const c of candidates) {
      if (Array.isArray(c) && c.length > 0) return c;
    }
    return [];
  }

  $: connectorList = getConnectorList(step);
  $: connectorCount =
    step.output?.count ??
    step.output?.output?.count ??
    connectorList.length;

  function connectorDisplayName(c: any): string {
    return c.displayName || c.pluginName || c.name || c.connectorId || "Connecteur";
  }

  function handleConnectConnector(c: any) {
    const name = connectorDisplayName(c);
    const detail = { text: `Connecte ${name} (${c.connectorId || name})` };
    window.dispatchEvent(new CustomEvent("aro:send-prompt", { detail }));
  }

  function handleOpenPlugins() {
    window.dispatchEvent(new CustomEvent("aro:open-settings", { detail: { page: "plugins" } }));
    window.dispatchEvent(new CustomEvent("aro:navigate-settings", { detail: { page: "plugins" } }));
  }

  $: browserUrl = step.input?.url || step.output?.url || "";
  $: browserTitle = step.output?.title || step.input?.title || extractDomain(browserUrl);
  $: browserContent = step.output?.content || step.output?.text || step.output?.summary || "";
  $: browserStatus = step.output?.status || (step.status === "completed" ? 200 : null);

  function extractDomain(urlStr: string): string {
    if (!urlStr) return "";
    try {
      return new URL(urlStr).hostname;
    } catch {
      return urlStr.replace(/^https?:\/\//i, "").split("/")[0];
    }
  }

  function handleOpenBrowser(targetUrl: string, newTab = false) {
    if (!targetUrl) return;
    window.dispatchEvent(
      new CustomEvent("aro:open-browser", {
        detail: newTab ? { url: targetUrl, newTab: true, takeControl: false } : { url: targetUrl },
      })
    );
  }

  function handleTakeControl(targetUrl: string) {
    if (!targetUrl) return;
    window.dispatchEvent(
      new CustomEvent("aro:open-browser", {
        detail: { url: targetUrl, takeControl: true, newTab: false },
      })
    );
  }

  function handleCopyUrl(urlStr: string) {
    navigator.clipboard.writeText(urlStr).then(() => {
      copiedUrl = true;
      setTimeout(() => (copiedUrl = false), 2000);
    });
  }

  function handleCopyOutput(text: string) {
    navigator.clipboard.writeText(text).then(() => {
      copiedOutput = true;
      setTimeout(() => (copiedOutput = false), 2000);
    });
  }

  function handlePreviewGeneratedFile(docOutput: any) {
    const artifact = docOutput?.artifact || docOutput;
    const name = artifact?.title || artifact?.name || artifact?.path || "document";
    const mimeType = artifact?.mimeType || docOutput?.mimeType || "application/octet-stream";
    const content = artifact?.content || docOutput?.content;

    window.dispatchEvent(
      new CustomEvent("aro:preview-file", {
        detail: {
          name,
          mimeType,
          content,
          url: artifact?.uri || "",
        },
      })
    );
  }

  function handlePreviewDiff(filePath: string, diff: string) {
    window.dispatchEvent(new CustomEvent("aro:preview-diff", { detail: { filePath, diff } }));
    window.dispatchEvent(new CustomEvent("aro:select-artifact", { detail: { filePath, diff } }));
  }
</script>

<div class="agent-step-card" class:running={step.status === "running"} class:failed={step.status === "failed"} class:dark={theme === "dark"}>
  <!-- Step Header Row -->
  <div class="step-card-header" on:click={onToggleDetails} role="button" tabindex="0" on:keydown={(e) => e.key === "Enter" && onToggleDetails()}>
    <div class="step-icon-badge">
      {#if step.status === "running"}
        <div class="step-spinner"></div>
      {:else if step.status === "completed"}
        {#if isBrowser}
          <Globe size={14} class="type-icon browser" />
        {:else if isConnector}
          <Puzzle size={14} class="type-icon connector" />
        {:else if isDocument}
          <FileSpreadsheet size={14} class="type-icon doc" />
        {:else if isSearch}
          <Search size={14} class="type-icon search" />
        {:else if isMemory}
          <Brain size={14} class="type-icon memory" />
        {:else if isShell}
          <Terminal size={14} class="type-icon shell" />
        {:else if isDiff}
          <GitCompare size={14} class="type-icon diff" />
        {:else}
          <span class="step-check-icon">✓</span>
        {/if}
      {:else if step.status === "failed"}
        <AlertCircle size={14} class="type-icon error" />
      {:else}
        <span class="step-bullet-icon">•</span>
      {/if}
    </div>

    <div class="step-title-area">
      <span class="step-title-text">{step.title}</span>
      {#if isBrowser && browserUrl}
        <span class="step-domain-pill">{extractDomain(browserUrl)}</span>
      {/if}
      {#if isConnector && connectorList.length > 0}
        <span class="connector-stack" title={connectorList.map(connectorDisplayName).join(", ")}>
          {#each connectorList.slice(0, 3) as c}
            <BrandLogo
              pluginId={c.connectorId || c.pluginName || ""}
              icon={c.icon}
              logo={c.logo}
              logoKind={c.logoKind}
              brandColor={c.brandColor}
              serverName={c.server}
              size={20}
              radius={10}
            />
          {/each}
          <span class="connector-count-pill">{connectorCount} connecteur{connectorCount > 1 ? "s" : ""}</span>
        </span>
      {/if}
    </div>

    <div class="step-header-actions">
      {#if isBrowser && browserUrl}
        <button
          class="step-take-control-pill"
          type="button"
          on:click|stopPropagation={() => handleTakeControl(browserUrl)}
          title={language === "fr" ? "Prendre le contrôle immédiat du navigateur" : "Take immediate browser control"}
        >
          <Sliders size={11} />
          <span>{language === "fr" ? "Prendre le contrôle" : "Take Control"}</span>
        </button>
        <button
          class="step-open-tab-pill"
          type="button"
          on:click|stopPropagation={() => handleOpenBrowser(browserUrl, true)}
          title={language === "fr" ? "Ouvrir dans un nouvel onglet du navigateur" : "Open in new browser tab"}
        >
          <ExternalLink size={11} />
          <span>{language === "fr" ? "Ouvrir l'onglet" : "Open tab"}</span>
        </button>
      {/if}
      {#if step.kind === "tool" && step.output}
        <button class="step-toggle-btn" type="button" on:click|stopPropagation={onToggleDetails}>
          {#if isExpanded}
            <ChevronUp size={13} />
            <span>{language === "fr" ? "Masquer" : "Hide"}</span>
          {:else}
            <ChevronDown size={13} />
            <span>{language === "fr" ? "Détails" : "Details"}</span>
          {/if}
        </button>
      {/if}
    </div>
  </div>

  <!-- Expanded Details Panel -->
  {#if isExpanded}
    <div class="step-card-body">
      <!-- 1. INLINE BROWSER PREVIEW CARD -->
      {#if isBrowser && browserUrl}
        <div class="inline-browser-card">
          <div class="browser-chrome-header">
            <div class="traffic-lights">
              <span class="dot red"></span>
              <span class="dot yellow"></span>
              <span class="dot green"></span>
            </div>

            <div class="browser-address-bar">
              <Lock size={11} class="lock-icon" />
              <span class="browser-url-text" title={browserUrl}>{browserUrl}</span>
            </div>

            <div class="browser-actions">
              <button
                class="browser-act-btn take-control-btn"
                type="button"
                title={language === "fr" ? "Prendre le contrôle manuel immédiat" : "Take manual control"}
                on:click={() => handleTakeControl(browserUrl)}
              >
                <Sliders size={12} />
                <span>{language === "fr" ? "Prendre le contrôle" : "Take Control"}</span>
              </button>

              <button
                class="browser-act-btn"
                type="button"
                title={language === "fr" ? "Copier le lien" : "Copy link"}
                on:click={() => handleCopyUrl(browserUrl)}
              >
                {#if copiedUrl}
                  <Check size={12} style="color: #30d158;" />
                {:else}
                  <Copy size={12} />
                {/if}
              </button>

              <button
                class="browser-act-btn primary"
                type="button"
                title={language === "fr" ? "Ouvrir dans le navigateur intégré" : "Open in integrated browser"}
                on:click={() => handleOpenBrowser(browserUrl)}
              >
                <ExternalLink size={12} />
                <span>{language === "fr" ? "Naviguer" : "Browse"}</span>
              </button>
            </div>
          </div>

          <!-- Page Meta & Summary -->
          <div class="browser-card-content">
            {#if browserTitle}
              <h4 class="page-title">{browserTitle}</h4>
            {/if}

            {#if browserStatus}
              <span class="status-code-badge" class:ok={browserStatus >= 200 && browserStatus < 300}>
                HTTP {browserStatus} OK
              </span>
            {/if}

            {#if browserContent}
              <div class="browser-excerpt-box">
                <p class="excerpt-text">{browserContent.slice(0, 420)}{browserContent.length > 420 ? "..." : ""}</p>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <!-- 2. DOCUMENT CREATION & OUTPUT CARD -->
      {#if isDocument && (step.output || step.input)}
        {@const docData = step.output || step.input}
        {@const fmt = (docData.format || docData.artifact?.kind || "doc").toLowerCase()}
        <div class="document-output-card" class:excel={fmt === "excel" || fmt === "xlsx" || fmt === "csv"} class:word={fmt === "word" || fmt === "docx"}>
          <div class="doc-card-top">
            <div class="doc-icon-wrapper">
              {#if fmt === "excel" || fmt === "xlsx" || fmt === "csv"}
                <FileSpreadsheet size={24} class="doc-badge-icon excel" />
              {:else if fmt === "word" || fmt === "docx"}
                <FileText size={24} class="doc-badge-icon word" />
              {:else}
                <FileText size={24} class="doc-badge-icon default" />
              {/if}
            </div>

            <div class="doc-info-col">
              <span class="doc-name">{docData.title || docData.artifact?.title || "Document généré"}</span>
              <span class="doc-sub">
                <span class="doc-format-tag">{fmt.toUpperCase()}</span>
                {#if docData.sizeBytes}
                  • {Math.round(docData.sizeBytes / 1024)} KB
                {/if}
              </span>
            </div>

            <button
              class="visualize-btn"
              type="button"
              on:click={() => handlePreviewGeneratedFile(docData)}
            >
              <Eye size={13} />
              <span>{language === "fr" ? "Visualiser" : "View"}</span>
            </button>
          </div>
        </div>
      {/if}

      <!-- 3. WEB SEARCH CARD -->
      {#if isSearch}
        {#if step.input?.query}
          <div class="detail-row">
            <span class="detail-label">{language === "fr" ? "Recherche :" : "Query:"}</span>
            <span class="detail-val quote">"{step.input.query}"</span>
          </div>
        {/if}

        {#if step.output?.summary}
          <div class="search-summary-box">
            <span class="summary-label">{language === "fr" ? "Synthèse de recherche :" : "Search Summary:"}</span>
            <p class="summary-text">{step.output.summary}</p>
          </div>
        {/if}

        {#if step.output?.results && step.output.results.length > 0}
          <div class="search-results-list">
            {#each step.output.results.slice(0, 4) as res}
              <a class="search-result-item" href={res.url} target="_blank" rel="noopener noreferrer">
                <Globe size={13} class="res-icon" />
                <div class="res-texts">
                  <span class="res-title">{res.title}</span>
                  <span class="res-domain">{extractDomain(res.url)}</span>
                </div>
                <ExternalLink size={11} class="res-link-arrow" />
              </a>
            {/each}
          </div>
        {/if}
      {/if}

      <!-- 4. MEMORY & COGNITIVE CARD -->
      {#if isMemory}
        <div class="memory-cognitive-card">
          <div class="memory-card-header">
            <Brain size={14} class="brain-icon" />
            <span>{language === "fr" ? "Mémoire Cognitive Hybride (RRF + Ebbinghaus)" : "Hybrid Cognitive Memory"}</span>
          </div>
          {#if step.input?.query || step.input?.text}
            <div class="memory-query">"{step.input.query || step.input.text}"</div>
          {/if}
          {#if step.output?.summary}
            <div class="memory-summary">{step.output.summary}</div>
          {/if}
        </div>
      {/if}

      <!-- 5. SHELL / CODE EXECUTION CARD -->
      {#if isShell}
        <div class="shell-output-card">
          <div class="shell-topbar">
            <Terminal size={12} />
            <span class="shell-cmd">{step.input?.command || step.input?.code || "shell command"}</span>
            <button
              class="shell-copy-btn"
              type="button"
              on:click={() => handleCopyOutput(step.output?.stdout || step.output?.output || "")}
            >
              {#if copiedOutput}
                <Check size={11} />
              {:else}
                <Copy size={11} />
              {/if}
            </button>
          </div>
          {#if step.output?.stdout || step.output?.output}
            <pre class="shell-stdout"><code>{step.output.stdout || step.output.output}</code></pre>
          {/if}
        </div>
      {/if}

      <!-- 6. DIFF CARD -->
      {#if isDiff && (step.input?.diff || step.output?.diff)}
        {@const diffText = step.input?.diff || step.output?.diff}
        {@const filePath = step.input?.path || step.input?.filePath || "file"}
        <div class="diff-card">
          <div class="diff-header">
            <GitCompare size={13} />
            <span class="diff-path">{filePath}</span>
            <button
              class="diff-inspect-btn"
              type="button"
              on:click={() => handlePreviewDiff(filePath, diffText)}
            >
              {language === "fr" ? "Examiner le Diff" : "Inspect Diff"}
            </button>
          </div>
        </div>
      {/if}

      <!-- 7. CONNECTOR / PLUGIN GRID — affichage pro noms + icônes -->
      {#if isConnector}
        {#if connectorList.length > 0}
          <div class="connector-panel">
            <div class="connector-panel-header">
              <div class="connector-panel-title">
                <Plug size={13} class="connector-plug-icon" />
                <span>{connectorCount} connecteur{connectorCount > 1 ? "s" : ""} {language === "fr" ? "installé" : "installed"}{connectorCount > 1 ? (language === "fr" ? "s" : "s") : ""}</span>
              </div>
              <button class="connector-manage-btn" type="button" on:click={handleOpenPlugins}>
                {language === "fr" ? "Gérer" : "Manage"}
              </button>
            </div>
            <div class="connector-grid">
              {#each connectorList as c}
                <div class="connector-card" class:disabled={!c.enabled}>
                  <div class="connector-card-top">
                    <BrandLogo
                      pluginId={c.connectorId || c.pluginName || ""}
                      icon={c.icon}
                      logo={c.logo}
                      logoKind={c.logoKind}
                      brandColor={c.brandColor}
                      serverName={c.server}
                      size={38}
                      radius={10}
                    />
                    <div class="connector-meta">
                      <div class="connector-name-row">
                        <span class="connector-name">{connectorDisplayName(c)}</span>
                        {#if c.version}
                          <span class="connector-version">v{c.version}</span>
                        {/if}
                      </div>
                      <div class="connector-sub-row">
                        <span class="connector-server-pill">{c.server || c.connectorId}</span>
                        {#if c.category}
                          <span class="connector-category">{c.category}</span>
                        {/if}
                      </div>
                    </div>
                    <span class="connector-status-dot" class:active={c.enabled} class:inactive={!c.enabled} title={c.enabled ? (language === "fr" ? "Actif" : "Active") : (language === "fr" ? "Inactif" : "Inactive")}></span>
                  </div>
                  {#if c.description}
                    <p class="connector-desc">{c.description}</p>
                  {/if}
                  <div class="connector-card-footer">
                    <div class="connector-account">
                      {#if c.activeAccount?.label || c.activeAccount?.email || (c.accounts && c.accounts.length > 0)}
                        <span class="account-dot"></span>
                        <span class="account-label">{c.activeAccount?.label || c.activeAccount?.email || `${c.accounts.length} compte${c.accounts.length > 1 ? "s" : ""}`}</span>
                      {:else}
                        <span class="account-label muted">{language === "fr" ? "Aucun compte lié" : "No linked account"}</span>
                      {/if}
                      {#if c.transport}
                        <span class="transport-tag">{c.transport}</span>
                      {/if}
                    </div>
                    <button class="connector-connect-btn" type="button" on:click={() => handleConnectConnector(c)}>
                      <Plug size={11} />
                      <span>{language === "fr" ? "Connecter" : "Connect"}</span>
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {:else}
          <div class="connector-empty">
            <Puzzle size={14} />
            <span>{language === "fr" ? "Aucun connecteur installé. Ouvre Réglages > Plugins pour en ajouter." : "No connectors installed. Open Settings > Plugins to add some."}</span>
            <button class="connector-manage-btn" type="button" on:click={handleOpenPlugins}>
              {language === "fr" ? "Ouvrir les plugins" : "Open plugins"}
            </button>
          </div>
        {/if}
      {/if}

      <!-- Error message if step failed -->
      {#if step.error}
        <div class="step-error-banner">
          <AlertCircle size={13} />
          <span>{step.error}</span>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .agent-step-card {
    border-radius: 9px;
    border: 1px solid rgba(0, 0, 0, 0.07);
    background: rgba(0, 0, 0, 0.015);
    margin-bottom: 6px;
    overflow: hidden;
    transition: all 0.2s ease-in-out;
  }

  .agent-step-card.dark {
    border-color: rgba(255, 255, 255, 0.07);
    background: rgba(255, 255, 255, 0.02);
  }

  .agent-step-card.running {
    border-color: rgba(0, 113, 227, 0.3);
    background: rgba(0, 113, 227, 0.03);
  }

  .agent-step-card.failed {
    border-color: rgba(255, 69, 58, 0.3);
  }

  .step-card-header {
    display: flex;
    align-items: center;
    padding: 7px 12px;
    gap: 8px;
    cursor: pointer;
    user-select: none;
  }

  .step-card-header:hover {
    background: rgba(0, 0, 0, 0.02);
  }

  .dark .step-card-header:hover {
    background: rgba(255, 255, 255, 0.03);
  }

  .step-icon-badge {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    flex-shrink: 0;
  }

  .step-spinner {
    width: 12px;
    height: 12px;
    border: 2px solid rgba(0, 113, 227, 0.2);
    border-top-color: #0071e3;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .step-check-icon {
    color: #30d158;
    font-size: 13px;
    font-weight: 700;
  }

  :global(.type-icon.browser) { color: #0071e3; }
  :global(.type-icon.doc) { color: #107c41; }
  :global(.type-icon.search) { color: #5856d6; }
  :global(.type-icon.memory) { color: #bf5af2; }
  :global(.type-icon.shell) { color: #ff9500; }
  :global(.type-icon.diff) { color: #0071e3; }
  :global(.type-icon.error) { color: #ff453a; }

  .step-title-area {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    overflow: hidden;
  }

  .step-title-text {
    font-size: 12px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .step-domain-pill {
    padding: 1px 6px;
    border-radius: 4px;
    background: rgba(0, 113, 227, 0.1);
    color: #0071e3;
    font-size: 10px;
    font-weight: 600;
  }

  .step-toggle-btn {
    display: flex;
    align-items: center;
    gap: 3px;
    padding: 3px 8px;
    border-radius: 5px;
    border: none;
    background: rgba(0, 0, 0, 0.05);
    font-size: 11px;
    color: inherit;
    cursor: pointer;
    transition: background 0.15s;
  }

  .dark .step-toggle-btn {
    background: rgba(255, 255, 255, 0.08);
  }

  .step-card-body {
    padding: 10px 12px;
    border-top: 1px solid rgba(0, 0, 0, 0.05);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .dark .step-card-body {
    border-top-color: rgba(255, 255, 255, 0.05);
  }

  /* Inline Browser Card */
  .inline-browser-card {
    border-radius: 8px;
    border: 1px solid rgba(0, 0, 0, 0.1);
    background: #ffffff;
    overflow: hidden;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.04);
  }

  .dark .inline-browser-card {
    background: #252528;
    border-color: rgba(255, 255, 255, 0.1);
  }

  .browser-chrome-header {
    display: flex;
    align-items: center;
    padding: 6px 10px;
    background: rgba(0, 0, 0, 0.03);
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    gap: 8px;
  }

  .dark .browser-chrome-header {
    background: rgba(255, 255, 255, 0.04);
    border-bottom-color: rgba(255, 255, 255, 0.06);
  }

  .traffic-lights {
    display: flex;
    gap: 4px;
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .dot.red { background: #ff5f56; }
  .dot.yellow { background: #ffbd2e; }
  .dot.green { background: #27c93f; }

  .browser-address-bar {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px;
    background: rgba(0, 0, 0, 0.04);
    border-radius: 5px;
    font-size: 11px;
    overflow: hidden;
  }

  .dark .browser-address-bar {
    background: rgba(255, 255, 255, 0.06);
  }

  :global(.lock-icon) {
    color: #30d158;
    flex-shrink: 0;
  }

  .browser-url-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #86868b;
  }

  .browser-actions {
    display: flex;
    gap: 4px;
  }

  .browser-act-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 7px;
    border-radius: 5px;
    border: none;
    background: rgba(0, 0, 0, 0.05);
    font-size: 10px;
    cursor: pointer;
    color: inherit;
  }

  .dark .browser-act-btn {
    background: rgba(255, 255, 255, 0.08);
  }

  .browser-act-btn.primary {
    background: #0071e3;
    color: #ffffff;
    font-weight: 500;
  }

  .browser-card-content {
    padding: 10px 12px;
  }

  .page-title {
    margin: 0 0 6px 0;
    font-size: 13px;
    font-weight: 600;
  }

  .status-code-badge {
    display: inline-block;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    background: rgba(0, 0, 0, 0.06);
    color: #86868b;
    margin-bottom: 8px;
  }

  .status-code-badge.ok {
    background: rgba(48, 209, 88, 0.12);
    color: #30d158;
  }

  .browser-excerpt-box {
    background: rgba(0, 0, 0, 0.02);
    padding: 8px 10px;
    border-radius: 6px;
    font-size: 11.5px;
    line-height: 1.45;
  }

  .dark .browser-excerpt-box {
    background: rgba(255, 255, 255, 0.03);
  }

  .excerpt-text {
    margin: 0;
    color: #666666;
  }

  .dark .excerpt-text {
    color: #a1a1a6;
  }

  /* Document Output Card */
  .document-output-card {
    display: flex;
    flex-direction: column;
    padding: 10px 14px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.02);
    border: 1px solid rgba(0, 0, 0, 0.08);
  }

  .dark .document-output-card {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .document-output-card.excel {
    border-left: 3px solid #107c41;
  }

  .document-output-card.word {
    border-left: 3px solid #2b579a;
  }

  .doc-card-top {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  :global(.doc-badge-icon.excel) { color: #107c41; }
  :global(.doc-badge-icon.word) { color: #2b579a; }
  :global(.doc-badge-icon.default) { color: #86868b; }

  .doc-info-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .doc-name {
    font-size: 12.5px;
    font-weight: 600;
  }

  .doc-sub {
    font-size: 11px;
    color: #86868b;
  }

  .doc-format-tag {
    font-weight: 700;
    color: #0071e3;
  }

  .visualize-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: 6px;
    border: 1px solid rgba(0, 0, 0, 0.1);
    background: #ffffff;
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    color: inherit;
    transition: all 0.15s;
  }

  .dark .visualize-btn {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.12);
  }

  .visualize-btn:hover {
    background: rgba(0, 113, 227, 0.1);
    color: #0071e3;
    border-color: #0071e3;
  }

  /* Memory Cognitive Card */
  .memory-cognitive-card {
    border-left: 3px solid #bf5af2;
    padding-left: 10px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .memory-card-header {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    color: #bf5af2;
  }

  .memory-query {
    font-size: 11.5px;
    font-style: italic;
    color: inherit;
  }

  .memory-summary {
    font-size: 11.5px;
    color: #86868b;
  }

  /* Search Card */
  .search-summary-box {
    background: rgba(0, 0, 0, 0.02);
    padding: 8px 10px;
    border-radius: 6px;
    font-size: 11.5px;
  }

  .dark .search-summary-box {
    background: rgba(255, 255, 255, 0.03);
  }

  .summary-label {
    font-weight: 600;
    font-size: 11px;
    color: #86868b;
    display: block;
    margin-bottom: 2px;
  }

  .summary-text {
    margin: 0;
  }

  .search-results-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .search-result-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 6px;
    text-decoration: none;
    color: inherit;
    background: rgba(0, 0, 0, 0.02);
    transition: background 0.15s;
  }

  .dark .search-result-item {
    background: rgba(255, 255, 255, 0.03);
  }

  .search-result-item:hover {
    background: rgba(0, 113, 227, 0.08);
  }

  :global(.res-icon) {
    color: #86868b;
    flex-shrink: 0;
  }

  .res-texts {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .res-title {
    font-size: 11.5px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .res-domain {
    font-size: 10px;
    color: #86868b;
  }

  :global(.res-link-arrow) {
    color: #86868b;
  }

  /* Shell Output */
  .shell-output-card {
    border-radius: 6px;
    background: #1e1e1e;
    color: #f5f5f7;
    overflow: hidden;
    font-family: monospace;
    font-size: 11px;
  }

  .shell-topbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    background: rgba(255, 255, 255, 0.08);
  }

  .shell-cmd {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .shell-copy-btn {
    background: transparent;
    border: none;
    color: #8e8e93;
    cursor: pointer;
    padding: 2px;
  }

  .shell-stdout {
    margin: 0;
    padding: 8px;
    max-height: 160px;
    overflow-y: auto;
    white-space: pre-wrap;
  }

  /* Diff Card */
  .diff-card {
    display: flex;
    align-items: center;
    padding: 6px 10px;
    background: rgba(0, 0, 0, 0.02);
    border-radius: 6px;
  }

  .dark .diff-card {
    background: rgba(255, 255, 255, 0.03);
  }

  .diff-header {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .diff-path {
    flex: 1;
    font-size: 11.5px;
    font-family: monospace;
  }

  .diff-inspect-btn {
    padding: 3px 8px;
    border-radius: 5px;
    border: 1px solid rgba(0, 113, 227, 0.3);
    background: rgba(0, 113, 227, 0.08);
    color: #0071e3;
    font-size: 11px;
    cursor: pointer;
  }

  .step-error-banner {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    border-radius: 6px;
    background: rgba(255, 69, 58, 0.1);
    color: #ff453a;
    font-size: 11.5px;
  }

  .step-take-control-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 8px;
    border-radius: 11px;
    border: 1px solid rgba(0, 113, 227, 0.4);
    background: linear-gradient(135deg, rgba(0, 113, 227, 0.18), rgba(175, 82, 222, 0.18));
    color: #2997ff;
    font-size: 10.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .step-take-control-pill:hover {
    background: linear-gradient(135deg, rgba(0, 113, 227, 0.35), rgba(175, 82, 222, 0.35));
    color: #ffffff;
    transform: scale(1.02);
  }

  .dark .step-take-control-pill {
    color: #70b4ff;
    border-color: rgba(112, 180, 255, 0.35);
  }

  .step-open-tab-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 8px;
    border-radius: 11px;
    border: 1px solid rgba(0, 0, 0, 0.12);
    background: rgba(0, 0, 0, 0.05);
    color: #3a3a3c;
    font-size: 10.5px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .step-open-tab-pill:hover {
    background: rgba(0, 0, 0, 0.1);
    color: #000000;
    transform: scale(1.02);
  }

  .dark .step-open-tab-pill {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.14);
    color: #e4e4e7;
  }

  .dark .step-open-tab-pill:hover {
    background: rgba(255, 255, 255, 0.15);
    color: #ffffff;
  }

  .browser-act-btn.take-control-btn {
    background: linear-gradient(135deg, rgba(0, 113, 227, 0.25), rgba(175, 82, 222, 0.25));
    border: 1px solid rgba(0, 113, 227, 0.4);
    color: #2997ff;
    font-weight: 600;
  }

  .browser-act-btn.take-control-btn:hover {
    background: #0071e3;
    color: #ffffff;
  }

  .dark .browser-act-btn.take-control-btn {
    color: #8ec5ff;
    border-color: rgba(142, 197, 255, 0.4);
  }

  :global(.type-icon.connector) { color: #e67e22; }

  .connector-stack {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: 6px;
  }
  .connector-stack :global(.brand-tile) {
    margin-left: -6px;
    box-shadow: 0 0 0 2px rgba(255,255,255,0.9);
  }
  .connector-stack :global(.brand-tile:first-child) { margin-left: 0; }
  .connector-count-pill {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: 10px;
    background: rgba(230,126,34,0.12);
    color: #e67e22;
    white-space: nowrap;
  }

  .connector-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .connector-panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .connector-panel-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
  }
  :global(.connector-plug-icon) { color: #e67e22; }
  .connector-manage-btn {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 9px;
    border-radius: 6px;
    border: 1px solid rgba(0,0,0,0.1);
    background: transparent;
    cursor: pointer;
    color: inherit;
  }
  .dark .connector-manage-btn { border-color: rgba(255,255,255,0.14); }
  .connector-manage-btn:hover {
    background: rgba(0,113,227,0.1);
    color: #0071e3;
    border-color: #0071e3;
  }
  .connector-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 8px;
  }
  .connector-card {
    border-radius: 12px;
    border: 1px solid rgba(0,0,0,0.08);
    background: #ffffff;
    padding: 10px 11px;
    display: flex;
    flex-direction: column;
    gap: 7px;
    transition: transform 0.15s, box-shadow 0.15s;
  }
  .dark .connector-card {
    background: rgba(255,255,255,0.03);
    border-color: rgba(255,255,255,0.09);
  }
  .connector-card:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 14px rgba(0,0,0,0.08);
  }
  .connector-card.disabled { opacity: 0.65; }
  .connector-card-top {
    display: flex;
    align-items: flex-start;
    gap: 9px;
  }
  .connector-card-top :global(.brand-tile) {
    box-shadow: 0 2px 8px rgba(0,0,0,0.1);
  }
  .connector-meta { flex: 1; min-width: 0; }
  .connector-name-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .connector-name {
    font-size: 12.5px;
    font-weight: 700;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .connector-version {
    font-size: 10px;
    font-weight: 600;
    color: #86868b;
    background: rgba(0,0,0,0.05);
    padding: 1px 5px;
    border-radius: 4px;
    flex-shrink: 0;
  }
  .dark .connector-version { background: rgba(255,255,255,0.08); }
  .connector-sub-row {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 3px;
    flex-wrap: wrap;
  }
  .connector-server-pill {
    font-size: 10px;
    font-weight: 600;
    font-family: ui-monospace, monospace;
    padding: 1px 6px;
    border-radius: 5px;
    background: rgba(0,113,227,0.1);
    color: #0071e3;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .connector-category {
    font-size: 10px;
    color: #86868b;
  }
  .connector-status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    margin-top: 4px;
  }
  .connector-status-dot.active { background: #30d158; box-shadow: 0 0 0 3px rgba(48,209,88,0.18); }
  .connector-status-dot.inactive { background: #8e8e93; }
  .connector-desc {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.45;
    color: #6e6e73;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .dark .connector-desc { color: #a1a1a6; }
  .connector-card-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: auto;
  }
  .connector-account {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 10.5px;
    min-width: 0;
  }
  .account-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #30d158;
    flex-shrink: 0;
  }
  .account-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 110px;
  }
  .account-label.muted { color: #86868b; font-style: italic; }
  .transport-tag {
    font-size: 9.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: #86868b;
    background: rgba(0,0,0,0.04);
    padding: 1px 5px;
    border-radius: 4px;
  }
  .dark .transport-tag { background: rgba(255,255,255,0.07); }
  .connector-connect-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
    border-radius: 7px;
    border: none;
    background: #0071e3;
    color: #fff;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
  }
  .connector-connect-btn:hover { background: #0077ed; }
  .connector-empty {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: #86868b;
    padding: 8px;
    border-style: dashed;
    border-width: 1px;
    border-color: rgba(0,0,0,0.12);
    border-radius: 8px;
  }
</style>
