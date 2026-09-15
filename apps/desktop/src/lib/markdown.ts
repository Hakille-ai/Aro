import { marked, Renderer } from "marked";

export type MarkdownLanguage = "fr" | "en";

// Lightweight syntax highlighting used by Markdown code blocks. The generated
// markup is intentionally kept stable because the application styles these
// classes directly.
export function highlightCode(code: string, lang: string): string {
  const escaped = code
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");

  if (!lang) return escaped;
  const l = lang.toLowerCase().trim();

  if (l === "json") {
    let html = escaped;
    html = html.replace(/("(\\u[a-zA-Z0-9]{4}|\\[^u]|[^\\"])*")/g, '<span class="hl-string">$1</span>');
    html = html.replace(/\b(true|false|null)\b/g, '<span class="hl-keyword">$1</span>');
    html = html.replace(/\b(-?\d+(?:\.\d*)?(?:[eE][+-]?\d+)?)\b/g, '<span class="hl-number">$1</span>');
    html = html.replace(/&lt;span class="hl-string"&gt;(.+?)&lt;\/span&gt;(?=\s*:)/g, '<span class="hl-key">$1</span>');
    return html;
  }

  if (
    l === "js" || l === "javascript" || l === "ts" || l === "typescript" ||
    l === "rust" || l === "rs" || l === "go" || l === "golang" ||
    l === "c" || l === "cpp" || l === "c++" || l === "cs" || l === "csharp" ||
    l === "java" || l === "python" || l === "py" || l === "bash" || l === "sh" || l === "shell"
  ) {
    let html = escaped;
    const tokens: { placeholder: string; html: string }[] = [];
    const protect = (str: string, className: string) => {
      const placeholder = `___HL_TOKEN_${tokens.length}___`;
      tokens.push({ placeholder, html: `<span class="${className}">${str}</span>` });
      return placeholder;
    };

    html = html.replace(/\/\*[\s\S]*?\*\//g, (match) => protect(match, "hl-comment"));
    if (l === "python" || l === "py" || l === "bash" || l === "sh" || l === "shell") {
      html = html.replace(/#.*/g, (match) => protect(match, "hl-comment"));
    } else {
      html = html.replace(/\/\/.*/g, (match) => protect(match, "hl-comment"));
    }

    html = html.replace(/(["'`])(\\?.)*?\1/g, (match) => protect(match, "hl-string"));

    const keywords = (l === "python" || l === "py")
      ? /\b(def|class|import|from|as|if|elif|else|while|for|in|try|except|finally|with|return|yield|pass|break|continue|lambda|and|or|not|is|None|True|False)\b/g
      : (l === "rust" || l === "rs")
        ? /\b(fn|let|mut|const|static|impl|struct|enum|trait|use|mod|pub|crate|self|Self|if|else|while|loop|for|in|match|return|break|continue|async|await|type|as|ref|unsafe|where)\b/g
        : /\b(const|let|var|function|class|constructor|extends|super|import|export|default|from|as|if|else|switch|case|while|do|for|in|of|try|catch|finally|throw|return|break|continue|await|async|yield|new|delete|typeof|instanceof|void|debugger|true|false|null|undefined|this|interface|type|implements|private|protected|public|readonly|namespace|any|string|number|boolean|unknown|never|void)\b/g;

    html = html.replace(keywords, '<span class="hl-keyword">$1</span>');
    html = html.replace(/\b(0x[a-fA-F0-9]+|\d+(?:\.\d*)?)\b/g, '<span class="hl-number">$1</span>');
    html = html.replace(/\b([a-zA-Z_$][a-zA-Z0-9_$]*)(?=\s*\()/g, '<span class="hl-function">$1</span>');

    for (let index = tokens.length - 1; index >= 0; index -= 1) {
      html = html.replace(tokens[index].placeholder, tokens[index].html);
    }
    return html;
  }

  if (l === "html" || l === "xml" || l === "svelte" || l === "vue") {
    let html = escaped;
    const tokens: { placeholder: string; html: string }[] = [];
    const protect = (str: string, className: string) => {
      const placeholder = `___HL_TOKEN_${tokens.length}___`;
      tokens.push({ placeholder, html: `<span class="${className}">${str}</span>` });
      return placeholder;
    };

    html = html.replace(/&lt;!--[\s\S]*?--&gt;/g, (match) => protect(match, "hl-comment"));
    html = html.replace(/(&lt;\/?[a-zA-Z0-9:-]+)(\s|&gt;)/g, (_match, tag, suffix) => {
      return protect(tag, "hl-keyword") + suffix;
    });
    html = html.replace(/(\s[a-zA-Z0-9:-]+=)(["'])(.*?)\2/g, (_match, name, quote, value) => {
      return " " + protect(name.slice(1, -1), "hl-attr") + "=" + protect(quote + value + quote, "hl-string");
    });

    for (let index = tokens.length - 1; index >= 0; index -= 1) {
      html = html.replace(tokens[index].placeholder, tokens[index].html);
    }
    return html;
  }

  if (l === "css") {
    let html = escaped;
    const tokens: { placeholder: string; html: string }[] = [];
    const protect = (str: string, className: string) => {
      const placeholder = `___HL_TOKEN_${tokens.length}___`;
      tokens.push({ placeholder, html: `<span class="${className}">${str}</span>` });
      return placeholder;
    };

    html = html.replace(/\/\*[\s\S]*?\*\//g, (match) => protect(match, "hl-comment"));
    html = html.replace(/(["'])(.*?)\1/g, (match) => protect(match, "hl-string"));
    html = html.replace(/([^{]+)(?=\s*\{)/g, (match) => {
      if (match.trim().startsWith("@")) return match;
      return `<span class="hl-keyword">${match}</span>`;
    });
    html = html.replace(/([a-zA-Z-_\s]+)(?=\s*:)/g, (match) => {
      return `<span class="hl-attr">${match}</span>`;
    });

    for (let index = tokens.length - 1; index >= 0; index -= 1) {
      html = html.replace(tokens[index].placeholder, tokens[index].html);
    }
    return html;
  }

  if (l === "diff") {
    const lines = escaped.split("\n");
    return lines.map((line) => {
      if (line.startsWith("+") && !line.startsWith("+++")) {
        return `<span class="hl-diff-add">${line}</span>`;
      }
      if (line.startsWith("-") && !line.startsWith("---")) {
        return `<span class="hl-diff-del">${line}</span>`;
      }
      if (line.startsWith("@@")) {
        return `<span class="hl-diff-hunk">${line}</span>`;
      }
      return line;
    }).join("\n");
  }

  return escaped;
}

export function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function safeMarkdownUrl(value: string): string | null {
  try {
    const url = new URL(value.trim());
    return url.protocol === "https:" || url.protocol === "http:" ? url.toString() : null;
  } catch {
    return null;
  }
}

function createRenderer(language: MarkdownLanguage): Renderer {
  const renderer = new Renderer();
  const defaultLinkRenderer = renderer.link.bind(renderer);
  const defaultImageRenderer = renderer.image.bind(renderer);

  // Raw HTML and non-HTTP(S) links are rendered as text so model output cannot
  // execute code in the desktop WebView or open a privileged URI scheme.
  renderer.html = ({ text }) => escapeHtml(text);
  renderer.link = function(token) {
    const href = safeMarkdownUrl(token.href);
    if (!href) {
      return this.parser.parseInline(token.tokens);
    }
    return defaultLinkRenderer({ ...token, href });
  };
  renderer.image = function(token) {
    const href = safeMarkdownUrl(token.href);
    if (!href) {
      return escapeHtml(token.text);
    }
    const escapedAlt = escapeHtml(token.text);
    const escapedHref = escapeHtml(href);
    return `
      <div class="chat-image-card">
        <img src="${escapedHref}" alt="${escapedAlt}" data-alt="${encodeURIComponent(token.text)}" onerror="window.__handleImageError(this)" onclick="window.__openImageLightbox(this)" />
        <div class="image-hover-toolbar">
          <button type="button" class="img-download-btn" title="Télécharger l'image" onclick="window.__downloadImage(this)">
            ⬇️ Télécharger
          </button>
        </div>
      </div>
    `;
  };
  renderer.code = function({ text, lang }) {
    const rawLang = (lang || "code").trim();
    const langTokens = rawLang.split(/\s+/);
    const codeLanguage = langTokens[0].toLowerCase();
    let effectiveLang = codeLanguage;

    if (codeLanguage === "diff") {
      const fileMatch = rawLang.match(/(?:filepath|path|filename|file)=["']([^"']+)["']/i);
      let filePath = fileMatch ? fileMatch[1] : undefined;
      if (!filePath && langTokens.length > 1 && langTokens[1].includes(".")) {
        filePath = langTokens[1];
      }
      if (!filePath) {
        const pMatch = text.match(/\+\+\+\s+(?:b\/|"?)(.+?)"?$/m);
        if (pMatch && pMatch[1] !== "/dev/null" && pMatch[1] !== "dev/null") {
          filePath = pMatch[1].trim();
        }
      }

      const displayPath = filePath || (language === "fr" ? "Patch Diff" : "Diff Patch");
      const escapedCode = encodeURIComponent(text);
      const escapedPath = encodeURIComponent(displayPath);

      // Compute syntax highlighted diff rows
      const lines = text.split("\n");
      let diffRowsHtml = "";
      let additions = 0;
      let deletions = 0;

      for (const line of lines) {
        if (line.startsWith("+") && !line.startsWith("+++")) {
          additions++;
          diffRowsHtml += `<div class="diff-line added"><span class="line-sign">+</span><span class="line-text">${escapeHtml(line.substring(1))}</span></div>`;
        } else if (line.startsWith("-") && !line.startsWith("---")) {
          deletions++;
          diffRowsHtml += `<div class="diff-line removed"><span class="line-sign">-</span><span class="line-text">${escapeHtml(line.substring(1))}</span></div>`;
        } else if (line.startsWith("@@")) {
          diffRowsHtml += `<div class="diff-line hunk-header"><span class="line-sign"> </span><span class="line-text">${escapeHtml(line)}</span></div>`;
        } else {
          const content = line.startsWith(" ") ? line.substring(1) : line;
          diffRowsHtml += `<div class="diff-line context"><span class="line-sign"> </span><span class="line-text">${escapeHtml(content)}</span></div>`;
        }
      }

      return `
        <div class="code-block-container chat-diff-container" data-code="${escapedCode}" data-filepath="${escapedPath}">
          <div class="code-block-header diff-block-header">
            <div class="diff-header-left" style="display: flex; align-items: center; gap: 6px;">
              <span class="diff-file-icon">📝</span>
              <span class="diff-file-path" style="font-family: ui-monospace, monospace; font-size: 11px; font-weight: 600;">${escapeHtml(displayPath)}</span>
              <span class="diff-delta-badge" style="font-size: 10px; font-weight: 600;"><span class="delta-add" style="color: #34c759;">+${additions}</span> <span class="delta-del" style="color: #ff3b30;">-${deletions}</span></span>
            </div>
            <div class="diff-header-actions" style="display: flex; align-items: center; gap: 6px;">
              <button class="diff-action-btn preview-btn" type="button" onclick="window.__previewChatDiff ? window.__previewChatDiff('${escapedPath}', '${escapedCode}') : window.dispatchEvent(new CustomEvent('aro:preview-diff', { detail: { filePath: decodeURIComponent('${escapedPath}'), diff: decodeURIComponent('${escapedCode}') } }))" title="${language === "fr" ? "Ouvrir dans le visualiseur de diff" : "Open in Diff Viewer"}">
                🔍 ${language === "fr" ? "Aperçu Diff" : "Preview Diff"}
              </button>
              <button class="diff-action-btn apply-btn" type="button" onclick="window.__applyChatDiff ? window.__applyChatDiff('${escapedPath}', '${escapedCode}') : window.dispatchEvent(new CustomEvent('aro:apply-diff', { detail: { filePath: decodeURIComponent('${escapedPath}'), diff: decodeURIComponent('${escapedCode}') } }))" title="${language === "fr" ? "Appliquer directement au projet" : "Apply to project files"}">
                ⚡ ${language === "fr" ? "Appliquer" : "Apply"}
              </button>
              <button class="code-block-copy-btn" type="button" onclick="window.__copyCodeBlock(this)" title="${language === "fr" ? "Copier le patch" : "Copy patch"}">
                ${language === "fr" ? "Copier" : "Copy"}
              </button>
            </div>
          </div>
          <div class="chat-diff-body">
            <pre class="diff-code-pre"><code>${diffRowsHtml}</code></pre>
          </div>
        </div>
      `;
    }

    // Smart JSON Auto-Detection for any code block (json, text, code, etc.)
    if (codeLanguage === "json" || codeLanguage === "code" || codeLanguage === "" || codeLanguage === "javascript") {
      try {
        const parsed = JSON.parse(text.trim());
        if (parsed && typeof parsed === "object") {
          if (parsed.type === "form" || parsed.fields || parsed.questions) {
            effectiveLang = "form";
          } else if (parsed.tools || (Array.isArray(parsed) && parsed[0]?.category && parsed[0]?.name)) {
            effectiveLang = "tools";
          } else if (parsed.type === "bar" || parsed.type === "line" || parsed.type === "pie" || parsed.type === "doughnut" || parsed.data?.datasets) {
            effectiveLang = "chart";
          } else if (parsed.tasks || (Array.isArray(parsed) && typeof parsed[0]?.done !== "undefined")) {
            effectiveLang = "tasks";
          } else if (parsed.metrics || parsed.stats) {
            effectiveLang = "metrics";
          } else if (parsed.images || (Array.isArray(parsed) && parsed[0]?.url)) {
            effectiveLang = "gallery";
          }
        }
      } catch {
        // Not valid JSON
      }
    }

    if (effectiveLang === "chart") {
      return `
        <div class="code-block-container chart-block-container" data-code="${encodeURIComponent(text)}" data-chart="${encodeURIComponent(text)}">
          <div class="code-block-header">
            <span class="code-block-lang">${language === "fr" ? "Graphique" : "Chart"}</span>
            <button class="code-block-copy-btn" type="button" onclick="window.__copyCodeBlock(this)">
              ${language === "fr" ? "Copier JSON" : "Copy JSON"}
            </button>
          </div>
          <div class="chart-wrapper">
            <canvas></canvas>
          </div>
        </div>
      `;
    }

    if (effectiveLang === "tools") {
      return `
        <div class="tools-block-container" data-code="${encodeURIComponent(text)}" data-tools="${encodeURIComponent(text)}">
          <div class="tools-grid-rendered"></div>
        </div>
      `;
    }

    if (effectiveLang === "gallery" || effectiveLang === "media" || effectiveLang === "images") {
      return `
        <div class="gallery-block-container" data-code="${encodeURIComponent(text)}" data-gallery="${encodeURIComponent(text)}">
          <div class="gallery-rendered"></div>
        </div>
      `;
    }

    if (effectiveLang === "tasks" || effectiveLang === "kanban" || effectiveLang === "todo") {
      return `
        <div class="tasks-block-container" data-code="${encodeURIComponent(text)}" data-tasks="${encodeURIComponent(text)}">
          <div class="tasks-card-rendered"></div>
        </div>
      `;
    }

    if (effectiveLang === "metrics" || effectiveLang === "kpi" || effectiveLang === "stats") {
      return `
        <div class="metrics-block-container" data-code="${encodeURIComponent(text)}" data-metrics="${encodeURIComponent(text)}">
          <div class="metrics-grid-rendered"></div>
        </div>
      `;
    }

    if (effectiveLang === "form" || effectiveLang === "question" || effectiveLang === "questions" || effectiveLang === "interactive") {
      let innerCardHtml = "";
      try {
        const data = JSON.parse(text);
        const title = data.title || (language === "fr" ? "Bienvenue dans ARO" : "Welcome to ARO");
        const fields = data.fields || data.questions || [];
        innerCardHtml = `
          <div class="futuristic-form-card">
            <div class="form-card-title">💡 ${escapeHtml(title)}</div>
            <div class="form-card-body">
        `;
        fields.forEach((field: any, idx: number) => {
          const label = field.label || field.question || `Option ${idx + 1}`;
          const options = field.options || [];
          innerCardHtml += `
            <div class="form-field-group">
              <label class="form-field-label">${escapeHtml(label)}</label>
          `;
          if (options.length > 0) {
            innerCardHtml += `<div class="form-options-row">`;
            options.forEach((opt: string) => {
              innerCardHtml += `<button type="button" class="form-option-chip" data-option="${encodeURIComponent(opt)}">${escapeHtml(opt)}</button>`;
            });
            innerCardHtml += `</div>`;
          } else {
            const inputId = `form-input-${Math.random().toString(36).slice(2, 6)}`;
            innerCardHtml += `
              <div class="form-input-row">
                <input type="text" id="${inputId}" class="form-text-input" placeholder="${language === "fr" ? "Votre réponse..." : "Your answer..."}" />
                <button type="button" class="form-submit-btn">Envoyer</button>
              </div>
            `;
          }
          innerCardHtml += `</div>`;
        });
        innerCardHtml += `</div></div>`;
      } catch {
        innerCardHtml = "";
      }

      return `
        <div class="form-block-container" data-code="${encodeURIComponent(text)}" data-form="${encodeURIComponent(text)}">
          <div class="form-card-rendered">${innerCardHtml}</div>
        </div>
      `;
    }

    if (codeLanguage === "mermaid") {
      return `
        <div class="code-block-container mermaid-block-container" data-code="${encodeURIComponent(text)}" data-mermaid="${encodeURIComponent(text)}">
          <div class="code-block-header">
            <span class="code-block-lang">Mermaid</span>
            <button class="code-block-copy-btn" type="button" onclick="window.__copyCodeBlock(this)">
              ${language === "fr" ? "Copier le code" : "Copy code"}
            </button>
          </div>
          <div class="mermaid-wrapper">
            <div class="mermaid-preview-rendered"></div>
            <div class="mermaid-loading-placeholder">${language === "fr" ? "Génération..." : "Generating..."}</div>
          </div>
        </div>
      `;
    }

    const highlighted = highlightCode(text, codeLanguage);
    const showPreview = codeLanguage === "html" || codeLanguage === "svg";

    return `
      <div class="code-block-container" data-code="${encodeURIComponent(text)}">
        <div class="code-block-header">
          <span class="code-block-lang">${codeLanguage}</span>
          <div style="display: flex; gap: 8px; align-items: center;">
            ${showPreview ? `
              <button class="code-block-preview-btn" type="button" onclick="window.__previewCodeBlock(this)">
                ${language === "fr" ? "Aperçu" : "Preview"}
              </button>
            ` : ""}
            <button class="code-block-copy-btn" type="button" onclick="window.__copyCodeBlock(this)">
              ${language === "fr" ? "Copier" : "Copy"}
            </button>
          </div>
        </div>
        <pre><code class="language-${codeLanguage}">${highlighted}</code></pre>
      </div>
    `;
  };

  return renderer;
}

export function renderMarkdown(content: string, language: MarkdownLanguage): string {
  try {
    return marked.parse(content, { async: false, renderer: createRenderer(language) }) as string;
  } catch {
    return content;
  }
}
