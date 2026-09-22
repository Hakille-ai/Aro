// richContent.ts
// Handles rendering of Chart.js and Mermaid diagrams inside markdown output.
import { getBrandLogo, brandTileCss, brandTileInner, ensureFileLogo } from "./brandLogos";

function hydrateConnectorFileLogos(root: HTMLElement) {
  const spots = root.querySelectorAll<HTMLElement>("[data-file-logo]");
  spots.forEach((spot) => {
    const pluginId = spot.getAttribute("data-file-logo") || "";
    if (!pluginId) return;
    ensureFileLogo(pluginId).then((url) => {
      if (!url || !spot.isConnected) return;
      spot.innerHTML =
        `<img src="${url}" alt="" draggable="false" style="width:100%;height:100%;object-fit:cover;border-radius:8px;" />`;
    });
  });
}

let chartJsPromise: Promise<any> | null = null;
async function loadChartJs() {
  if (!chartJsPromise) {
    chartJsPromise = (async () => {
      const { Chart, registerables } = await import("chart.js");
      Chart.register(...registerables);
      return Chart;
    })();
  }
  return chartJsPromise;
}

let mermaidPromise: Promise<any> | null = null;
async function loadMermaid(isDark: boolean) {
  if (!mermaidPromise) {
    mermaidPromise = (async () => {
      const mermaidModule = await import("mermaid");
      const mermaid = mermaidModule.default;
      mermaid.initialize({
        startOnLoad: false,
        theme: isDark ? "dark" : "default",
        fontFamily: "system-ui, -apple-system, sans-serif",
      });
      return mermaid;
    })();
  } else {
    // Re-initialize theme if it changed
    const mermaid = await mermaidPromise;
    mermaid.initialize({
      startOnLoad: false,
      theme: isDark ? "dark" : "default",
      fontFamily: "system-ui, -apple-system, sans-serif",
    });
  }
  return mermaidPromise;
}

// Store active chart instances to destroy them when elements are removed
const activeCharts = new Map<HTMLCanvasElement, any>();

export async function initRichContent(container: HTMLElement, isDark: boolean) {
  if (!container) return;

  // 1. Process Charts
  const chartContainers = container.querySelectorAll(".chart-block-container");
  if (chartContainers.length > 0) {
    try {
      const Chart = await loadChartJs();
      chartContainers.forEach((el) => {
        const canvas = el.querySelector("canvas");
        if (!canvas) return;

        // Skip if already rendered
        if (activeCharts.has(canvas)) return;

        const rawData = decodeURIComponent(el.getAttribute("data-chart") || "");
        try {
          const chartConfig = JSON.parse(rawData);
          
          // Apply some premium defaults for chart styling
          if (chartConfig.options === undefined) {
            chartConfig.options = {};
          }
          
          const textAndBorderColor = isDark ? "rgba(255, 255, 255, 0.7)" : "rgba(0, 0, 0, 0.7)";
          const gridColor = isDark ? "rgba(255, 255, 255, 0.08)" : "rgba(0, 0, 0, 0.05)";
          
          // Premium default configurations
          chartConfig.options = {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
              legend: {
                labels: {
                  color: textAndBorderColor,
                  font: { family: "system-ui, -apple-system, sans-serif", size: 11, weight: "bold" }
                }
              },
              tooltip: {
                backgroundColor: isDark ? "rgba(30, 30, 32, 0.95)" : "rgba(255, 255, 255, 0.95)",
                titleColor: isDark ? "#ffffff" : "#000000",
                bodyColor: isDark ? "rgba(255, 255, 255, 0.8)" : "rgba(0, 0, 0, 0.8)",
                borderColor: isDark ? "rgba(255, 255, 255, 0.1)" : "rgba(0, 0, 0, 0.06)",
                borderWidth: 1,
                padding: 10,
                cornerRadius: 8,
              }
            },
            scales: chartConfig.type === "pie" || chartConfig.type === "doughnut" ? {} : {
              x: {
                grid: { color: gridColor },
                ticks: { color: textAndBorderColor, font: { family: "system-ui, sans-serif" } }
              },
              y: {
                grid: { color: gridColor },
                ticks: { color: textAndBorderColor, font: { family: "system-ui, sans-serif" } }
              }
            },
            ...chartConfig.options
          };

          // Premium Apple-inspired gradients or color palette default if not provided
          if (chartConfig.data && chartConfig.data.datasets) {
            chartConfig.data.datasets.forEach((dataset: any, index: number) => {
              // Add nice default colors if none specified
              if (!dataset.backgroundColor) {
                const colors = [
                  "rgba(10, 132, 255, 0.85)", // Blue
                  "rgba(48, 209, 88, 0.85)",  // Green
                  "rgba(191, 90, 242, 0.85)", // Purple
                  "rgba(255, 159, 10, 0.85)", // Orange
                  "rgba(255, 69, 58, 0.85)",  // Red
                  "rgba(100, 210, 255, 0.85)" // Teal
                ];
                dataset.backgroundColor = colors[index % colors.length];
              }
              if (!dataset.borderColor) {
                const borderColors = [
                  "rgba(10, 132, 255, 1)",
                  "rgba(48, 209, 88, 1)",
                  "rgba(191, 90, 242, 1)",
                  "rgba(255, 159, 10, 1)",
                  "rgba(255, 69, 58, 1)",
                  "rgba(100, 210, 255, 1)"
                ];
                dataset.borderColor = borderColors[index % borderColors.length];
                dataset.borderWidth = dataset.borderWidth || 2;
              }
            });
          }

          const chartInstance = new Chart(canvas, chartConfig);
          activeCharts.set(canvas, chartInstance);
        } catch (err) {
          console.error("Error parsing/rendering chart JSON:", err);
          el.innerHTML = `<div class="chart-error-msg">⚠️ Graphique invalide: ${err instanceof Error ? err.message : String(err)}</div>`;
        }
      });
    } catch (e) {
      console.error("Failed to load Chart.js:", e);
    }
  }

  // 2. Process Mermaid Diagrams
  const mermaidContainers = container.querySelectorAll(".mermaid-block-container");
  if (mermaidContainers.length > 0) {
    try {
      const mermaid = await loadMermaid(isDark);
      
      for (const el of Array.from(mermaidContainers)) {
        const previewDiv = el.querySelector(".mermaid-preview-rendered") as HTMLElement;
        const loadingDiv = el.querySelector(".mermaid-loading-placeholder") as HTMLElement;
        if (!previewDiv) continue;

        // Skip if already rendered (has SVG child)
        if (previewDiv.querySelector("svg")) continue;

        const rawCode = decodeURIComponent(el.getAttribute("data-mermaid") || "");
        const id = `mermaid-${Math.random().toString(36).substr(2, 9)}`;
        
        try {
          const { svg, bindFunctions } = await mermaid.render(id, rawCode);
          previewDiv.innerHTML = svg;
          if (bindFunctions) {
            bindFunctions(previewDiv);
          }
          if (loadingDiv) {
            loadingDiv.style.display = "none";
          }
        } catch (err) {
          console.error("Mermaid render error:", err);
          if (loadingDiv) {
            loadingDiv.innerHTML = `<div class="mermaid-error-msg">⚠️ Diagramme invalide</div>`;
            loadingDiv.style.color = "#ff453a";
          }
        }
      }
    } catch (e) {
      console.error("Failed to load Mermaid:", e);
    }
  }

  // 3. Process Interactive Tools Grid
  const toolsContainers = container.querySelectorAll(".tools-block-container");
  toolsContainers.forEach((el) => {
    const renderedDiv = el.querySelector(".tools-grid-rendered") as HTMLElement;
    if (!renderedDiv || renderedDiv.children.length > 0) return;

    const rawData = decodeURIComponent(el.getAttribute("data-tools") || "");
    try {
      let data = JSON.parse(rawData);
      let toolsArray = Array.isArray(data) ? data : data.tools || [];
      if (!Array.isArray(toolsArray)) return;

      let html = `
        <div class="futuristic-tools-header">
          <span class="tools-title">⚡ Équipement & Outils ARO (${toolsArray.length})</span>
        </div>
        <div class="futuristic-tools-grid">
      `;

      toolsArray.forEach((tool: any) => {
        const name = tool.name || "Tool";
        const desc = tool.description || "";
        const category = tool.category || "search";

        html += `
          <div class="tool-card-item">
            <div class="tool-card-head">
              <span class="tool-card-badge">${category}</span>
              <span class="tool-card-name">${name}</span>
            </div>
            <p class="tool-card-desc">${desc}</p>
            <button class="tool-card-act-btn" data-tool="${encodeURIComponent(name)}" onclick="window.__useToolPrompt(this)">
              Exécuter / Tester
            </button>
          </div>
        `;
      });

      html += `</div>`;
      renderedDiv.innerHTML = html;
    } catch (e) {
      console.error("Failed to parse tools json:", e);
    }
  });

  // 3b. Process Connectors / Plugins Grid — pro cards nom + icône
  const connectorsContainers = container.querySelectorAll(".connectors-block-container");
  connectorsContainers.forEach((el) => {
    const renderedDiv = el.querySelector(".connectors-grid-rendered") as HTMLElement;
    if (!renderedDiv || renderedDiv.children.length > 0) return;
    const rawData = decodeURIComponent(el.getAttribute("data-connectors") || "");
    try {
      const data = JSON.parse(rawData);
      const list = Array.isArray(data)
        ? data
        : data.connectors || data.plugins || [];
      if (!Array.isArray(list)) return;
      const esc = (s: any) =>
        String(s ?? "")
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;")
          .replace(/"/g, "&quot;");
      let html = `<div class="chat-connectors-panel"><div class="chat-connectors-header"><span>🔌 ${list.length} connecteur${list.length > 1 ? "s" : ""} installé${list.length > 1 ? "s" : ""}</span></div><div class="chat-connectors-grid">`;
      list.forEach((c: any) => {
        const name = esc(c.displayName || c.pluginName || c.name || c.connectorId || "Connecteur");
        const server = esc(c.server || c.connectorId || "");
        const desc = esc(c.description || "");
        const version = c.version ? `<span class="chat-conn-version">v${esc(c.version)}</span>` : "";
        const account = esc(c.activeAccount?.label || c.activeAccount?.email || "");
        const pluginKey = String(c.connectorId || c.pluginName || c.name || "");
        const brand = getBrandLogo(pluginKey, {
          icon: c.icon,
          logo: c.logo,
          logoKind: c.logoKind,
          brandColor: c.brandColor,
          serverName: c.server,
        });
        const tile = brand.filePluginId
          ? `<span class="chat-conn-icon" data-file-logo="${esc(pluginKey)}" style="${brandTileCss(brand, 38)}">${brandTileInner(brand, 21)}</span>`
          : `<span class="chat-conn-icon" style="${brandTileCss(brand, 38)}">${brandTileInner(brand, 21)}</span>`;
        html += `<div class="chat-connector-card">${tile}<div class="chat-conn-meta"><div class="chat-conn-name-row"><span class="chat-conn-name">${name}</span>${version}</div><div class="chat-conn-server">${server}</div>${desc ? `<p class="chat-conn-desc">${desc}</p>` : ""}${account ? `<div class="chat-conn-account">● ${account}</div>` : ""}</div><button class="chat-conn-btn" data-option="${encodeURIComponent("Connecte " + name)}" onclick="window.__sendChatOption ? window.__sendChatOption(this) : null">Connecter</button></div>`;
      });
      html += `</div></div>`;
      renderedDiv.innerHTML = html;
      // Hydrate uploaded file logos lazily (bytes stay out of the LLM
      // context; tiles upgrade in place once fetched).
      hydrateConnectorFileLogos(renderedDiv);
    } catch (e) {
      console.error("Failed to parse connectors json:", e);
    }
  });

  // 4. Process Interactive Form Card
  const formContainers = container.querySelectorAll(".form-block-container");
  formContainers.forEach((el) => {
    const renderedDiv = el.querySelector(".form-card-rendered") as HTMLElement;
    if (!renderedDiv) return;

    if (renderedDiv.children.length > 0) {
      renderedDiv.querySelectorAll<HTMLButtonElement>(".form-option-chip").forEach((btn) => {
        if ((btn as any).__bound) return;
        (btn as any).__bound = true;
        btn.addEventListener("click", (e) => {
          e.preventDefault();
          e.stopPropagation();
          const raw = btn.getAttribute("data-option");
          let opt = "";
          if (raw) {
            try { opt = decodeURIComponent(raw); } catch { opt = raw; }
          }
          if (!opt && btn.textContent) opt = btn.textContent.trim();
          if (opt) {
            btn.style.opacity = "0.6";
            btn.style.transform = "scale(0.96)";
            setTimeout(() => { btn.style.opacity = ""; btn.style.transform = ""; }, 300);
            if (typeof (window as any).__sendChatOption === "function") {
              (window as any).__sendChatOption(opt);
            } else {
              window.dispatchEvent(new CustomEvent("aro:send-prompt", { detail: { text: opt } }));
            }
          }
        });
      });
      return;
    }

    const rawData = decodeURIComponent(el.getAttribute("data-form") || "");
    try {
      let data = JSON.parse(rawData);
      let title = data.title || "Formulaire de Décision AGI";
      let fields = data.fields || data.questions || [];

      let html = `
        <div class="futuristic-form-card">
          <div class="form-card-title">💡 ${title}</div>
          <div class="form-card-body">
      `;

      fields.forEach((field: any, idx: number) => {
        const label = field.label || field.question || `Option ${idx + 1}`;
        const options = field.options || [];

        html += `
          <div class="form-field-group">
            <label class="form-field-label">${label}</label>
        `;

        if (options.length > 0) {
          html += `<div class="form-options-row">`;
          options.forEach((opt: string) => {
            html += `<button type="button" class="form-option-chip" data-option="${encodeURIComponent(opt)}">${opt}</button>`;
          });
          html += `</div>`;
        } else {
          const inputId = `form-input-${Math.random().toString(36).slice(2, 6)}`;
          html += `
            <div class="form-input-row">
              <input type="text" id="${inputId}" class="form-text-input" placeholder="Votre réponse..." />
              <button type="button" class="form-submit-btn" onclick="window.__sendChatInput('${inputId}')">Envoyer</button>
            </div>
          `;
        }
        html += `</div>`;
      });

      html += `</div></div>`;
      renderedDiv.innerHTML = html;

      // Attach direct event listeners
      renderedDiv.querySelectorAll<HTMLButtonElement>(".form-option-chip").forEach((btn) => {
        (btn as any).__bound = true;
        btn.addEventListener("click", (e) => {
          e.preventDefault();
          e.stopPropagation();
          const raw = btn.getAttribute("data-option");
          let opt = "";
          if (raw) {
            try { opt = decodeURIComponent(raw); } catch { opt = raw; }
          }
          if (!opt && btn.textContent) opt = btn.textContent.trim();
          if (opt) {
            btn.style.opacity = "0.6";
            btn.style.transform = "scale(0.96)";
            setTimeout(() => { btn.style.opacity = ""; btn.style.transform = ""; }, 300);
            if (typeof (window as any).__sendChatOption === "function") {
              (window as any).__sendChatOption(opt);
            } else {
              window.dispatchEvent(new CustomEvent("aro:send-prompt", { detail: { text: opt } }));
            }
          }
        });
      });
    } catch (e) {
      console.error("Failed to parse form json:", e);
    }
  });

  // 5. Process Interactive Image Gallery
  const galleryContainers = container.querySelectorAll(".gallery-block-container");
  galleryContainers.forEach((el) => {
    const renderedDiv = el.querySelector(".gallery-rendered") as HTMLElement;
    if (!renderedDiv || renderedDiv.children.length > 0) return;

    const rawData = decodeURIComponent(el.getAttribute("data-gallery") || "");
    try {
      let data = JSON.parse(rawData);
      let imagesArray = Array.isArray(data) ? data : data.images || [];
      if (!Array.isArray(imagesArray)) return;

      let html = `
        <div class="gallery-layout-toolbar">
          <span class="gallery-title">🖼️ Galerie d'Images (${imagesArray.length})</span>
          <div class="layout-toggle-btns">
            <button class="layout-btn active" type="button" onclick="window.__setGalleryLayout(this, 'horizontal')">⇄ Horizontale</button>
            <button class="layout-btn" type="button" onclick="window.__setGalleryLayout(this, 'vertical')">⇕ Verticale</button>
          </div>
        </div>
        <div class="futuristic-gallery-grid horizontal">
      `;
      imagesArray.forEach((img: any) => {
        const url = typeof img === "string" ? img : img.url || "";
        const caption = typeof img === "string" ? "" : img.caption || img.title || "";

        html += `
          <div class="gallery-card-item" onclick="window.__openImageLightbox(this)">
            <img src="${url}" alt="${caption}" data-alt="${encodeURIComponent(caption)}" class="gallery-img" onerror="window.__handleImageError(this)" />
            <div class="gallery-hover-overlay">
              <button type="button" class="img-download-btn" title="Télécharger" onclick="event.stopPropagation(); window.__downloadImage(this)">
                ⬇️ Télécharger
              </button>
            </div>
            ${caption ? `<span class="gallery-caption">${caption}</span>` : ""}
          </div>
        `;
      });
      html += `</div>`;
      renderedDiv.innerHTML = html;
    } catch (e) {
      console.error("Failed to parse gallery json:", e);
    }
  });

  // 6. Process Interactive Tasks / Checklist
  const tasksContainers = container.querySelectorAll(".tasks-block-container");
  tasksContainers.forEach((el) => {
    const renderedDiv = el.querySelector(".tasks-card-rendered") as HTMLElement;
    if (!renderedDiv || renderedDiv.children.length > 0) return;

    const rawData = decodeURIComponent(el.getAttribute("data-tasks") || "");
    try {
      let data = JSON.parse(rawData);
      let title = data.title || "Feuille de Route & Tâches";
      let tasksArray = Array.isArray(data) ? data : data.tasks || [];
      if (!Array.isArray(tasksArray)) return;

      let html = `
        <div class="futuristic-tasks-card">
          <div class="tasks-card-header">📋 ${title} (${tasksArray.length})</div>
          <div class="tasks-card-list">
      `;

      tasksArray.forEach((task: any, idx: number) => {
        const text = typeof task === "string" ? task : task.text || task.label || `Tâche ${idx + 1}`;
        const done = typeof task === "object" ? Boolean(task.done || task.completed) : false;

        html += `
          <label class="task-row-item">
            <input type="checkbox" ${done ? "checked" : ""} class="task-checkbox" />
            <span class="task-text ${done ? "completed" : ""}">${text}</span>
          </label>
        `;
      });

      html += `</div></div>`;
      renderedDiv.innerHTML = html;
    } catch (e) {
      console.error("Failed to parse tasks json:", e);
    }
  });

  // 7. Process KPI / Metrics Scorecards
  const metricsContainers = container.querySelectorAll(".metrics-block-container");
  metricsContainers.forEach((el) => {
    const renderedDiv = el.querySelector(".metrics-grid-rendered") as HTMLElement;
    if (!renderedDiv || renderedDiv.children.length > 0) return;

    const rawData = decodeURIComponent(el.getAttribute("data-metrics") || "");
    try {
      let data = JSON.parse(rawData);
      let metricsArray = Array.isArray(data) ? data : data.metrics || data.stats || [];
      if (!Array.isArray(metricsArray)) return;

      let html = `<div class="futuristic-metrics-grid">`;
      metricsArray.forEach((m: any) => {
        const label = m.label || m.name || "Métrique";
        const val = m.value || m.val || "0";
        const change = m.change || m.trend || "";
        const isUp = change.includes("+") || m.status === "up";

        html += `
          <div class="metric-card-item">
            <span class="metric-label">${label}</span>
            <div class="metric-val-row">
              <span class="metric-value">${val}</span>
              ${change ? `<span class="metric-change ${isUp ? "up" : "down"}">${change}</span>` : ""}
            </div>
          </div>
        `;
      });
      html += `</div>`;
      renderedDiv.innerHTML = html;
    } catch (e) {
      console.error("Failed to parse metrics json:", e);
    }
  });
}

// Cleanup function to avoid memory leaks
export function destroyCharts(container: HTMLElement) {
  if (!container) return;
  const canvases = container.querySelectorAll("canvas");
  canvases.forEach((canvas) => {
    const chart = activeCharts.get(canvas);
    if (chart) {
      chart.destroy();
      activeCharts.delete(canvas);
    }
  });
}
