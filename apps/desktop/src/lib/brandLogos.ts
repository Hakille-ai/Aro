/**
 * Brand logos for ARO plugins / connectors.
 *
 * Offline-first: all marks are vendored inline (no CDN hotlinking).
 * - GitHub + Google Drive paths: Simple Icons (CC0 1.0).
 * - Slack / OpenAI: original geometric approximations in official brand
 *   colors. Simple Icons v16 removed both slugs (Salesforce trademark,
 *   OpenAI permission), so exact upstream paths are intentionally avoided.
 * - Dev-tool tiles: hand-drawn 24px stroke glyphs (Lucide-style).
 *
 * Spec note (agent-plugins.org §5): `plugin.json` is a closed schema with
 * NO logo/icon field. Custom branding travels via
 * `extensions["com.aro.client"] = { logo, brandColor }` and via the
 * curated marketplace metadata. `getBrandLogo` prefers an explicit
 * `logo` override, then the curated mark, then the emoji fallback.
 */

export interface BrandLogo {
  /** Stable key: plugin id slug */
  id: string;
  /** CSS background for the tile */
  bg: string;
  /** Glyph color (SVG uses currentColor) */
  fg: string;
  /** Inner SVG markup (24x24 viewBox) or "" when emoji fallback applies */
  svg: string;
  /** Emoji fallback when svg === "" */
  emoji: string;
  label: string;
  /** Set when the plugin carries an uploaded file logo: bytes load lazily */
  filePluginId?: string;
}

export interface BrandLogoOpts {
  icon?: string;
  logo?: string;
  logoKind?: string;
  brandColor?: string;
  serverName?: string;
}

const STROKE_ATTRS =
  'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"';

const GITHUB_PATH =
  "M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12";

const DRIVE_PATH =
  "M12.01 1.485c-2.082 0-3.754.02-3.743.047.01.02 1.708 3.001 3.774 6.62l3.76 6.574h3.76c2.081 0 3.753-.02 3.742-.047-.005-.02-1.708-3.001-3.775-6.62l-3.76-6.574zm-4.76 1.73a789.828 789.861 0 0 0-3.63 6.319L0 15.868l1.89 3.298 1.885 3.297 3.62-6.335 3.618-6.33-1.88-3.287C8.1 4.704 7.255 3.22 7.25 3.214zm2.259 12.653-.203.348c-.114.198-.96 1.672-1.88 3.287a423.93 423.948 0 0 1-1.698 2.97c-.01.026 3.24.042 7.222.042h7.244l1.796-3.157c.992-1.734 1.85-3.23 1.906-3.323l.104-.167h-7.249z";

function strokeGlyph(inner: string): string {
  return `<svg viewBox="0 0 24 24" width="100%" height="100%" ${STROKE_ATTRS}>${inner}</svg>`;
}

function fillGlyph(path: string): string {
  return `<svg viewBox="0 0 24 24" width="100%" height="100%" fill="currentColor">${path}</svg>`;
}

const SLACK_GLYPH = `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect x="2" y="2" width="9" height="9" rx="4.5" fill="#36C5F0"/><rect x="13" y="2" width="9" height="9" rx="4.5" fill="#2EB67D"/><rect x="2" y="13" width="9" height="9" rx="4.5" fill="#ECB22E"/><rect x="13" y="13" width="9" height="9" rx="4.5" fill="#E01E5A"/><circle cx="12" cy="12" r="2.6" fill="#ffffff"/></svg>`;

const OPENAI_GLYPH = `<svg viewBox="0 0 24 24" width="100%" height="100%" fill="none" stroke="currentColor" stroke-width="1.8"><g><ellipse cx="12" cy="7.2" rx="2.6" ry="4.4"/><ellipse cx="12" cy="7.2" rx="2.6" ry="4.4" transform="rotate(60 12 12)"/><ellipse cx="12" cy="7.2" rx="2.6" ry="4.4" transform="rotate(120 12 12)"/><ellipse cx="12" cy="7.2" rx="2.6" ry="4.4" transform="rotate(180 12 12)"/><ellipse cx="12" cy="7.2" rx="2.6" ry="4.4" transform="rotate(240 12 12)"/><ellipse cx="12" cy="7.2" rx="2.6" ry="4.4" transform="rotate(300 12 12)"/></g></svg>`;

const LOGOS: Record<string, BrandLogo> = {
  "google-workspace": {
    id: "google-workspace",
    bg: "#ffffff",
    fg: "#1A73E8",
    svg: fillGlyph(`<path d="${DRIVE_PATH}"/>`),
    emoji: "📑",
    label: "Google Drive",
  },
  "github-developer": {
    id: "github-developer",
    bg: "#181717",
    fg: "#ffffff",
    svg: fillGlyph(`<path d="${GITHUB_PATH}"/>`),
    emoji: "🐙",
    label: "GitHub",
  },
  "slack-workspace": {
    id: "slack-workspace",
    bg: "#ffffff",
    fg: "#4A154B",
    svg: SLACK_GLYPH,
    emoji: "💬",
    label: "Slack",
  },
  "openai-ecosystem": {
    id: "openai-ecosystem",
    bg: "#000000",
    fg: "#ffffff",
    svg: OPENAI_GLYPH,
    emoji: "🤖",
    label: "OpenAI",
  },
  "filesystem-tools": {
    id: "filesystem-tools",
    bg: "linear-gradient(135deg,#0A84FF,#5E5CE6)",
    fg: "#ffffff",
    svg: strokeGlyph(
      `<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/>`
    ),
    emoji: "📁",
    label: "Filesystem",
  },
  "web-search-tools": {
    id: "web-search-tools",
    bg: "linear-gradient(135deg,#00C7BE,#0A84FF)",
    fg: "#ffffff",
    svg: strokeGlyph(
      `<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"/><path d="M2 12h20"/>`
    ),
    emoji: "🌐",
    label: "Web",
  },
  "git-assistant": {
    id: "git-assistant",
    bg: "linear-gradient(135deg,#F05032,#8E44AD)",
    fg: "#ffffff",
    svg: strokeGlyph(
      `<line x1="6" x2="6" y1="3" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>`
    ),
    emoji: "🐈",
    label: "Git",
  },
  "sqlite-database": {
    id: "sqlite-database",
    bg: "linear-gradient(135deg,#0F6CBD,#00C7BE)",
    fg: "#ffffff",
    svg: strokeGlyph(
      `<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5V19A9 3 0 0 0 21 19V5"/><path d="M3 12A9 3 0 0 0 21 12"/>`
    ),
    emoji: "🗄️",
    label: "SQL",
  },
  "python-analytics": {
    id: "python-analytics",
    bg: "linear-gradient(135deg,#3776AB,#FFD43B)",
    fg: "#ffffff",
    svg: strokeGlyph(`<polyline points="m4 17 6-6-6-6"/><line x1="12" x2="20" y1="19" y2="19"/>`),
    emoji: "🐍",
    label: "Python",
  },
  "code-reviewer": {
    id: "code-reviewer",
    bg: "linear-gradient(135deg,#30D158,#0A84FF)",
    fg: "#ffffff",
    svg: strokeGlyph(
      `<path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1 1 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/><path d="m9 12 2 2 4-4"/>`
    ),
    emoji: "🛡️",
    label: "Review",
  },
};

const GENERIC: BrandLogo = {
  id: "generic",
  bg: "linear-gradient(135deg,#0A84FF,#BF5AF2)",
  fg: "#ffffff",
  svg: "",
  emoji: "🧩",
  label: "Plugin",
};

function matchKey(raw: string): string | null {
  const id = (raw || "").toLowerCase();
  if (!id) return null;
  if (id.includes("google") || id.includes("drive") || id.includes("gmail") || id.includes("workspace"))
    return "google-workspace";
  if (id.includes("github")) return "github-developer";
  if (id.includes("slack")) return "slack-workspace";
  if (id.includes("openai") || id.includes("gpt")) return "openai-ecosystem";
  if (id.includes("filesystem") || id.includes("workspace") || id.includes("file"))
    return "filesystem-tools";
  if (id.includes("web") || id.includes("search") || id.includes("fetch") || id.includes("scrap"))
    return "web-search-tools";
  if (id === "git-assistant" || (id.includes("git") && !id.includes("github"))) return "git-assistant";
  if (id.includes("sqlite") || id.includes("sql") || id.includes("database")) return "sqlite-database";
  if (id.includes("python") || id.includes("analytic") || id.includes("data")) return "python-analytics";
  if (id.includes("review") || id.includes("secur") || id.includes("audit") || id.includes("lint"))
    return "code-reviewer";
  return null;
}

/**
 * Resolve the brand tile for a plugin/connector.
 * Priority: uploaded file logo > explicit short `logo` emoji override >
 * curated mark > emoji fallback. `brandColor` overrides the tile background.
 */
export function getBrandLogo(pluginId: string, opts?: BrandLogoOpts): BrandLogo {
  const key =
    matchKey(pluginId) ||
    matchKey(opts?.serverName || "") ||
    matchKey(opts?.icon || "");
  const base: BrandLogo = key ? LOGOS[key] : GENERIC;
  const overrideLogo = (opts?.logo || "").trim();
  const tile: BrandLogo = {
    ...base,
    bg: opts?.brandColor?.trim() ? opts.brandColor.trim() : base.bg,
  };
  const isFileRef =
    opts?.logoKind === "file" || overrideLogo.startsWith("file:");
  if (isFileRef) {
    return { ...tile, svg: "", filePluginId: pluginId };
  }
  const isEmojiOverride =
    overrideLogo.length > 0 && overrideLogo.length <= 8 && !overrideLogo.startsWith("<");
  if (isEmojiOverride) {
    return { ...tile, svg: "", emoji: overrideLogo };
  }
  return tile;
}

/** CSS for a raw-HTML tile (chat bubbles, markdown widgets). */
export function brandTileCss(brand: BrandLogo, sizePx: number): string {
  const needsBorder = brand.bg === "#ffffff";
  return (
    `width:${sizePx}px;height:${sizePx}px;border-radius:10px;` +
    `display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;` +
    `background:${brand.bg};color:${brand.fg};` +
    (needsBorder ? "border:1px solid rgba(0,0,0,0.1);" : "border:1px solid rgba(0,0,0,0.06);") +
    `font-size:${Math.round(sizePx * 0.52)}px;`
  );
}

/** Inner HTML for a raw-HTML tile: real SVG mark or emoji fallback. */
export function brandTileInner(brand: BrandLogo, glyphPx?: number): string {
  if (brand.svg) {
    const s = glyphPx || 20;
    return `<span style="display:inline-flex;width:${s}px;height:${s}px;">${brand.svg}</span>`;
  }
  const esc = brand.emoji
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  return `<span>${esc}</span>`;
}

/* ── Uploaded file logos: lazy cache ─────────────────────────────── */

import { writable } from "svelte/store";

/** pluginId → dataUrl (string), null (no file logo), undefined (not loaded). */
export const fileLogoUrls = writable<Record<string, string | null>>({});

const fileLogoInflight = new Map<string, Promise<string | null>>();

/** Fetch + cache the uploaded logo bytes for a plugin (single-flight). */
export function ensureFileLogo(pluginId: string): Promise<string | null> {
  if (!pluginId) return Promise.resolve(null);
  let cached: Record<string, string | null> = {};
  fileLogoUrls.subscribe((v) => (cached = v))();
  if (pluginId in cached) return Promise.resolve(cached[pluginId] ?? null);
  const running = fileLogoInflight.get(pluginId);
  if (running) return running;
  const job = (async () => {
    try {
      const { readPluginLogo } = await import("./api/plugins");
      const payload = await readPluginLogo(pluginId);
      const url = payload?.dataUrl ?? null;
      fileLogoUrls.update((m) => ({ ...m, [pluginId]: url }));
      return url;
    } catch {
      fileLogoUrls.update((m) => ({ ...m, [pluginId]: null }));
      return null;
    } finally {
      fileLogoInflight.delete(pluginId);
    }
  })();
  fileLogoInflight.set(pluginId, job);
  return job;
}

/** Drop a cached entry (e.g. after uninstall / logo change). */
export function evictFileLogo(pluginId: string): void {
  fileLogoUrls.update((m) => {
    const next = { ...m };
    delete next[pluginId];
    return next;
  });
}

/* ── Creation-form validation (shared + unit-tested) ─────────────── */

export const LOGO_UPLOAD_MAX_BYTES = 2 * 1024 * 1024;
export const LOGO_UPLOAD_ACCEPT = ["image/png", "image/jpeg", "image/webp", "image/svg+xml"] as const;

export const LOGO_EMOJI_PRESETS = [
  "🚀", "⚡", "🧩", "🤖", "📦", "🛠️", "🌐", "📊",
  "🎨", "🔌", "💡", "🗂️",
];

export const LOGO_COLOR_PRESETS = [
  "#0A84FF", "#5E5CE6", "#BF5AF2", "#FF375F",
  "#FF9F0A", "#30D158", "#00C7BE", "#181717",
];

export function validateLogoUpload(file: File): { ok: boolean; error?: string } {
  if (!(LOGO_UPLOAD_ACCEPT as readonly string[]).includes(file.type)) {
    return { ok: false, error: "Format accepté : PNG, JPEG, WebP ou SVG." };
  }
  if (file.size <= 0 || file.size > LOGO_UPLOAD_MAX_BYTES) {
    return { ok: false, error: "Logo trop lourd (2 Mo max)." };
  }
  return { ok: true };
}

export function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(new Error("Lecture du fichier impossible"));
    reader.readAsDataURL(file);
  });
}

/** Kebab-case strict (miroir de la spec agent-plugins.org §5.5). */
export function isValidPluginSlug(name: string): boolean {
  if (!name || name.length > 64) return false;
  return /^(?!.*(?:--|\.\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/.test(name);
}
