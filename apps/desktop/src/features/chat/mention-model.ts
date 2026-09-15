/**
 * M4 — Pure @ mention model for the chat Composer.
 *
 * Pure logic (no Svelte, no DOM, no IPC) so it stays unit-testable:
 * - F4.1: trigger detection, entry filtering/ranking, token insertion, path extraction
 * - F4.3: workspace tree flattening -> mention entries, context injection helpers
 *
 * Contract mirrors PROJECT.md "Mention Model Contract".
 */

export interface MentionTrigger {
  active: boolean;
  query: string;
  startIndex: number;
  endIndex: number;
}

export interface WorkspaceMentionEntry {
  name: string;
  path: string;
  isDir: boolean;
  size?: number;
  relativePath: string;
  extension: string;
}

export interface RawWorkspaceNode {
  name?: string;
  path?: string;
  relativePath?: string;
  isDir?: boolean;
  size?: number;
  children?: RawWorkspaceNode[];
}

/** Email addresses must never trigger a mention popover. */
const EMAIL_GUARD_RE = /[A-Za-z0-9._%+-]@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$/;

/**
 * Detects an active `@query` trigger immediately before `cursorIndex`.
 * Trigger starts at line start or after whitespace/newline, never mid-email.
 */
export function detectMentionQuery(text: string, cursorIndex: number): MentionTrigger | null {
  const safeCursor = Math.max(0, Math.min(cursorIndex, text.length));
  const beforeCursor = text.slice(0, safeCursor);
  const match = beforeCursor.match(/(?:^|\s)@([a-zA-Z0-9_\-./\\]*)$/);
  if (!match) return null;
  const query = match[1] ?? "";
  const token = `@${query}`;
  const startIndex = beforeCursor.lastIndexOf(token);
  if (startIndex < 0) return null;
  // Reject `user@example.com` style sequences: char before `@` is email-local char.
  const charBefore = startIndex > 0 ? beforeCursor[startIndex - 1] : "";
  if (charBefore && /[A-Za-z0-9._%+-]/.test(charBefore)) return null;
  if (EMAIL_GUARD_RE.test(beforeCursor.slice(0, startIndex + 1))) return null;
  return { active: true, query, startIndex, endIndex: safeCursor };
}

/**
 * Filters + ranks workspace entries for a query (exact > prefix > substring > path).
 * Empty query returns the first `limit` entries (stable order).
 */
export function filterWorkspaceEntries(
  entries: WorkspaceMentionEntry[],
  query: string,
  limit = 12,
): WorkspaceMentionEntry[] {
  const list = entries ?? [];
  if (!query.trim()) return list.slice(0, limit);
  const q = query.toLowerCase().replace(/\\/g, "/");
  return list
    .map((entry) => {
      let score = 0;
      const nameLower = (entry.name ?? "").toLowerCase();
      const pathLower = (entry.relativePath ?? "").toLowerCase().replace(/\\/g, "/");
      if (nameLower === q) score += 100;
      else if (nameLower.startsWith(q)) score += 60;
      else if (nameLower.includes(q)) score += 30;
      else if (pathLower.includes(q)) score += 10;
      return { entry, score };
    })
    .filter((item) => item.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, Math.max(0, limit))
    .map((item) => item.entry);
}

/** Replaces the trigger range with `@selectedPath ` and returns the new caret. */
export function applyMentionSelection(
  text: string,
  trigger: MentionTrigger,
  selectedPath: string,
): { newText: string; newCursor: number } {
  const before = text.slice(0, trigger.startIndex);
  const after = text.slice(trigger.endIndex);
  const inserted = `@${selectedPath} `;
  return {
    newText: `${before}${inserted}${after}`,
    newCursor: before.length + inserted.length,
  };
}

/** Extracts every `@path` token from a message (dedup callers decide). */
export function extractMentionedPaths(text: string): string[] {
  if (!text) return [];
  const matches = text.matchAll(/(?:^|\s)@([a-zA-Z0-9_\-./\\]+)/g);
  return Array.from(matches, (m) => m[1]);
}

/** Deduplicates while preserving first-seen order. */
export function dedupeMentionedPaths(paths: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const p of paths ?? []) {
    const key = p.replace(/\\/g, "/");
    if (!seen.has(key)) {
      seen.add(key);
      out.push(p);
    }
  }
  return out;
}

function extensionOf(fileName: string): string {
  const base = fileName.split(/[/\\]/).pop() ?? fileName;
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) return "";
  return base.slice(dot + 1).toLowerCase();
}

/** Builds a mention entry from a flat tree record. */
export function toMentionEntry(node: RawWorkspaceNode): WorkspaceMentionEntry {
  const relativePath = (node.relativePath || node.path || node.name || "").replace(/\\/g, "/");
  const name = node.name || relativePath.split("/").filter(Boolean).pop() || relativePath;
  return {
    name,
    path: node.path || relativePath,
    isDir: !!node.isDir,
    size: typeof node.size === "number" ? node.size : undefined,
    relativePath,
    extension: node.isDir ? "" : extensionOf(name),
  };
}

/**
 * Flattens a workspace tree (flat backend list or nested `{ children }`)
 * into mention entries. Directories are kept (popover shows folder icons);
 * callers can filter with `includeDirs: false` for file-only contexts.
 */
export function flattenWorkspaceTreeToMentions(
  nodes: RawWorkspaceNode[] | null | undefined,
  options: { includeDirs?: boolean; maxEntries?: number } = {},
): WorkspaceMentionEntry[] {
  const { includeDirs = true, maxEntries = 2000 } = options;
  const out: WorkspaceMentionEntry[] = [];
  const seen = new Set<string>();
  function push(node: RawWorkspaceNode) {
    const entry = toMentionEntry(node);
    const key = entry.relativePath || entry.path;
    if (!key || seen.has(key)) return;
    seen.add(key);
    if (!entry.isDir || includeDirs) out.push(entry);
  }
  function walk(list: RawWorkspaceNode[] | null | undefined, depth = 0) {
    if (depth > 64) return;
    for (const node of list ?? []) {
      if (out.length >= maxEntries) return;
      const key = (node.relativePath || node.path || node.name || "").replace(/\\/g, "/");
      const alreadySeen = Boolean(key && seen.has(key));
      push(node);
      if (!alreadySeen && node.children?.length) {
        walk(node.children, depth + 1);
      }
    }
  }
  walk(nodes);
  return out;
}

/** Resolves `@paths` against known entries; unknown ghost paths are skipped. */
export function resolveMentionedEntries(
  text: string,
  entries: WorkspaceMentionEntry[],
): WorkspaceMentionEntry[] {
  const wanted = dedupeMentionedPaths(extractMentionedPaths(text));
  if (wanted.length === 0) return [];
  const byPath = new Map<string, WorkspaceMentionEntry>();
  for (const e of entries ?? []) {
    byPath.set(e.relativePath.replace(/\\/g, "/"), e);
    byPath.set(e.path.replace(/\\/g, "/"), e);
  }
  const out: WorkspaceMentionEntry[] = [];
  for (const w of wanted) {
    const key = w.replace(/\\/g, "/").replace(/^\.\//, "");
    let hit = byPath.get(key);
    if (!hit) {
      const nameMatches = (entries ?? []).filter(
        (e) => e.name.toLowerCase() === key.toLowerCase(),
      );
      if (nameMatches.length === 1) {
        hit = nameMatches[0];
      }
    }
    if (hit && !out.includes(hit)) out.push(hit);
  }
  return out;
}

/** Strips `@path` tokens but keeps surrounding prose readable. */
export function stripMentionTokens(text: string): string {
  return text
    .replace(/(^|\s)@[a-zA-Z0-9_\-./\\]+/g, "$1")
    .replace(/[ \t]{2,}/g, " ")
    .trim();
}
