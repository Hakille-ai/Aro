/**
 * Artifact extraction and management for ARO AI Workspace.
 *
 * Extracts deliverables (code files, unified diff patches) from
 * assistant chat messages and merges them with subagent artifacts.
 */

import { parseUnifiedDiff, type DiffLine } from "./diff";
import type { ChatMessage } from "./types";

export type ArtifactKind = "diff" | "code" | "file";
export type ArtifactStatus = "pending" | "applied" | "rejected";

export interface WorkspaceArtifact {
  id: string;                      // Stable unique ID: `art-${msgId}-${index}`
  conversationId?: string | null;  // Associated conversation ID
  messageId?: string;              // Source message ID
  title: string;                   // Display title: e.g. "src/App.svelte" or custom title
  filePath: string;                // Normalized relative workspace file path
  kind: ArtifactKind;              // "diff" for patch fences, "code" for complete files
  content: string;                 // Raw code block or diff patch string
  diffLines?: DiffLine[];          // Parsed DiffLine[] if diff
  additions: number;               // Lines added metric
  deletions: number;               // Lines removed metric
  status: ArtifactStatus;          // "pending" | "applied" | "rejected"
  createdAt: string;               // ISO timestamp of generation
  language?: string;               // Code language (ts, svelte, rust, css, etc.)
  uniqueKey?: string;              // De-duplication fallback key
}

/**
 * Normalizes relative workspace path:
 * replaces Windows backslashes with POSIX slashes and removes leading slashes and dot-slashes.
 */
function normalizeWorkspacePath(rawPath: string): string {
  if (!rawPath) return "";
  return rawPath
    .replace(/\\/g, "/")
    .replace(/^(\.\/)+/, "")
    .replace(/^\/+/, "")
    .trim();
}

/**
 * Extracts candidate artifacts from code blocks and diff fences in conversation messages.
 */
export function extractArtifactsFromMessages(
  messages: ChatMessage[],
  activeConversationId?: string | null
): WorkspaceArtifact[] {
  if (!messages || !Array.isArray(messages) || messages.length === 0) return [];

  const artifacts: WorkspaceArtifact[] = [];
  const seenPaths = new Map<string, number>();

  // Only scan assistant messages for proposed deliverables
  const assistantMessages = messages.filter((m) => m && m.role === "assistant");

  for (const msg of assistantMessages) {
    if (!msg || typeof msg.content !== "string" || !msg.content.trim()) continue;

    // Match all fenced code blocks: ```lang [attributes]\n content \n```
    const fenceRegex = /(?:^|\n)```([^\n]*)\n([\s\S]*?)(?:```|$)/g;
    let match: RegExpExecArray | null;
    let blockIndex = 0;

    while ((match = fenceRegex.exec(msg.content)) !== null) {
      blockIndex++;
      const header = (match[1] || "").trim();
      const rawContent = match[2] || "";

      // 1. Extract language identifier (first word)
      const langMatch = header.match(/^([a-zA-Z0-9_-]+)/);
      const rawLang = langMatch ? langMatch[1].toLowerCase() : "";

      // 2. Extract filepath attribute from header (filepath="...", path="...", filename="...", file="...")
      let filePath: string | undefined;
      const attrMatch = header.match(/(?:filepath|path|filename|file)=["']([^"']+)["']/i);
      if (attrMatch) {
        filePath = attrMatch[1];
      } else {
        // Fallback: If header has a second token that looks like a file path with an extension
        const tokens = header.split(/\s+/);
        if (tokens.length > 1 && tokens[1].includes(".") && !tokens[1].startsWith("-")) {
          filePath = tokens[1].replace(/["']/g, "");
        }
      }

      // 3. Process diff fences
      if (rawLang === "diff") {
        const parsed = parseUnifiedDiff(rawContent);
        const resolvedPath = filePath || parsed.filePath || `patch_${msg.id || "msg"}_${blockIndex}.diff`;
        const normalizedPath = normalizeWorkspacePath(resolvedPath);

        const additions = Math.max(0, parsed.additions || 0);
        const deletions = Math.max(0, parsed.deletions || 0);

        const id = `art-${msg.id || "msg"}-${blockIndex}`;
        const count = seenPaths.get(normalizedPath) ?? 0;
        seenPaths.set(normalizedPath, count + 1);

        artifacts.push({
          id,
          conversationId: activeConversationId ?? null,
          messageId: msg.id,
          title: normalizedPath.split("/").pop() || normalizedPath,
          filePath: normalizedPath,
          kind: "diff",
          content: rawContent,
          diffLines: parsed.diffLines,
          additions,
          deletions,
          status: "pending",
          createdAt: msg.createdAt || new Date().toISOString(),
          language: "diff",
          uniqueKey: `${id}-${count}`,
        });
      }
      // 4. Process full-file code fences with explicit filepath
      else if (filePath) {
        const normalizedPath = normalizeWorkspacePath(filePath);
        const normalized = rawContent.replace(/\r\n/g, "\n");
        const lines = normalized.split("\n");
        if (lines.length > 0 && lines[lines.length - 1] === "") {
          lines.pop();
        }
        const additions = lines.length;
        const deletions = 0;

        const id = `art-${msg.id || "msg"}-${blockIndex}`;
        const count = seenPaths.get(normalizedPath) ?? 0;
        seenPaths.set(normalizedPath, count + 1);

        artifacts.push({
          id,
          conversationId: activeConversationId ?? null,
          messageId: msg.id,
          title: normalizedPath.split("/").pop() || normalizedPath,
          filePath: normalizedPath,
          kind: "code",
          content: rawContent,
          additions,
          deletions,
          status: "pending",
          createdAt: msg.createdAt || new Date().toISOString(),
          language: rawLang || "code",
          uniqueKey: `${id}-${count}`,
        });
      }
    }
  }

  return artifacts;
}

/**
 * Merges agent artifacts with chat-extracted artifacts, deduplicating by normalized filePath.
 */
export function mergeArtifacts(
  agentArtifacts: any[],
  chatArtifacts: WorkspaceArtifact[]
): WorkspaceArtifact[] {
  const byPath = new Map<string, WorkspaceArtifact>();

  // Chat artifacts from active conversation
  if (Array.isArray(chatArtifacts)) {
    for (const art of chatArtifacts) {
      if (art && art.filePath) {
        byPath.set(art.filePath, art);
      }
    }
  }

  // Agent artifacts from active run
  if (Array.isArray(agentArtifacts)) {
    for (let i = 0; i < agentArtifacts.length; i++) {
      const a = agentArtifacts[i];
      if (!a) continue;
      const rawPath = a.uri || a.filePath || a.title || `deliverable-${i}`;
      const normalized = normalizeWorkspacePath(rawPath);
      byPath.set(normalized, {
        id: a.id || `agent-${normalized}-${i}`,
        title: a.title || normalized.split("/").pop() || normalized,
        filePath: normalized,
        kind: a.kind || "file",
        content: a.content || "",
        diffLines: a.diffLines,
        additions: Math.max(0, a.metadata?.additions ?? a.additions ?? 0),
        deletions: Math.max(0, a.metadata?.deletions ?? a.deletions ?? 0),
        status: a.status || "pending",
        createdAt: a.createdAt || new Date().toISOString(),
        uniqueKey: `agent-${normalized}-${i}`,
      });
    }
  }

  return Array.from(byPath.values());
}
