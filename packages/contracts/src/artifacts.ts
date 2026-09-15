export type ArtifactKind = "diff" | "code" | "file";
export type ArtifactStatus = "pending" | "applied" | "rejected";

export interface DiffLine {
  type: "added" | "removed" | "context";
  oldLineNo?: number;
  newLineNo?: number;
  content: string;
}

export interface SplitDiffRow {
  oldLine?: {
    lineNo: number;
    content: string;
    type: "removed" | "context";
  };
  newLine?: {
    lineNo: number;
    content: string;
    type: "added" | "context";
  };
}

export interface ParsedUnifiedDiff {
  filePath?: string;
  diffLines: DiffLine[];
  additions: number;
  deletions: number;
}

export interface WorkspaceArtifact {
  id: string;
  conversationId?: string | null;
  messageId?: string;
  title: string;
  filePath: string;
  kind: ArtifactKind;
  content: string;
  diffLines?: DiffLine[];
  additions: number;
  deletions: number;
  status: ArtifactStatus;
  createdAt: string;
  language?: string;
}

function splitLines(text: string): string[] {
  if (!text) return [];
  const normalized = text.replace(/\r\n/g, "\n");
  const lines = normalized.split("\n");
  if (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop();
  }
  return lines;
}

/**
 * Computes a line-by-line diff between two strings using Longest Common Subsequence (LCS).
 */
export function computeLineDiff(oldText: string, newText: string): DiffLine[] {
  const oldLines = splitLines(oldText);
  const newLines = splitLines(newText);

  const m = oldLines.length;
  const n = newLines.length;

  if (m === 0 && n === 0) return [];

  if (m === 0) {
    return newLines.map((content, idx) => ({
      type: "added" as const,
      newLineNo: idx + 1,
      content,
    }));
  }

  if (n === 0) {
    return oldLines.map((content, idx) => ({
      type: "removed" as const,
      oldLineNo: idx + 1,
      content,
    }));
  }

  // DP table for LCS
  const dp: number[][] = Array.from({ length: m + 1 }, () =>
    new Array<number>(n + 1).fill(0)
  );

  for (let i = 0; i < m; i++) {
    for (let j = 0; j < n; j++) {
      if (oldLines[i] === newLines[j]) {
        dp[i + 1][j + 1] = dp[i][j] + 1;
      } else {
        dp[i + 1][j + 1] = Math.max(dp[i][j + 1], dp[i + 1][j]);
      }
    }
  }

  // Backtrack to build diff
  const result: DiffLine[] = [];
  let i = m;
  let j = n;

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      result.unshift({
        type: "context",
        oldLineNo: i,
        newLineNo: j,
        content: oldLines[i - 1],
      });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      result.unshift({
        type: "added",
        newLineNo: j,
        content: newLines[j - 1],
      });
      j--;
    } else if (i > 0 && (j === 0 || dp[i][j - 1] < dp[i - 1][j])) {
      result.unshift({
        type: "removed",
        oldLineNo: i,
        content: oldLines[i - 1],
      });
      i--;
    }
  }

  return result;
}

/**
 * Builds side-by-side (split) diff rows from unified DiffLine list.
 */
export function buildSplitDiffRows(diffLines: DiffLine[]): SplitDiffRow[] {
  const rows: SplitDiffRow[] = [];
  let idx = 0;

  while (idx < diffLines.length) {
    const line = diffLines[idx];

    if (line.type === "context") {
      rows.push({
        oldLine: {
          lineNo: line.oldLineNo ?? 0,
          content: line.content,
          type: "context",
        },
        newLine: {
          lineNo: line.newLineNo ?? 0,
          content: line.content,
          type: "context",
        },
      });
      idx++;
    } else if (line.type === "removed") {
      // Look ahead for matching additions to align side-by-side
      const removedChunk: DiffLine[] = [];
      while (idx < diffLines.length && diffLines[idx].type === "removed") {
        removedChunk.push(diffLines[idx]);
        idx++;
      }

      const addedChunk: DiffLine[] = [];
      while (idx < diffLines.length && diffLines[idx].type === "added") {
        addedChunk.push(diffLines[idx]);
        idx++;
      }

      const maxLen = Math.max(removedChunk.length, addedChunk.length);
      for (let k = 0; k < maxLen; k++) {
        const rem = removedChunk[k];
        const add = addedChunk[k];
        rows.push({
          oldLine: rem
            ? {
                lineNo: rem.oldLineNo ?? 0,
                content: rem.content,
                type: "removed",
              }
            : undefined,
          newLine: add
            ? {
                lineNo: add.newLineNo ?? 0,
                content: add.content,
                type: "added",
              }
            : undefined,
        });
      }
    } else if (line.type === "added") {
      rows.push({
        newLine: {
          lineNo: line.newLineNo ?? 0,
          content: line.content,
          type: "added",
        },
      });
      idx++;
    }
  }

  return rows;
}

/**
 * Parses a standard Git unified diff patch string into DiffLines and statistics.
 */
export function parseUnifiedDiff(rawPatch: string): ParsedUnifiedDiff {
  if (!rawPatch) return { diffLines: [], additions: 0, deletions: 0 };

  const lines = rawPatch.replace(/\r\n/g, "\n").split("\n");
  const diffLines: DiffLine[] = [];
  let additions = 0;
  let deletions = 0;
  let filePath: string | undefined;

  let oldNo = 0;
  let newNo = 0;

  for (const line of lines) {
    if (line.startsWith("diff --git") || line.startsWith("index ") || line.startsWith("--- ")) {
      if (!filePath && line.startsWith("--- a/")) {
        filePath = line.substring(6).trim();
      }
      continue;
    }

    if (line.startsWith("+++ b/")) {
      filePath = line.substring(6).trim();
      continue;
    }

    if (line.startsWith("@@")) {
      // Hunk header: @@ -oldStart,oldCount +newStart,newCount @@
      const match = line.match(/@@\s+-(\d+)(?:,\d+)?\s+\+(\d+)(?:,\d+)?\s+@@/);
      if (match) {
        oldNo = parseInt(match[1], 10);
        newNo = parseInt(match[2], 10);
      }
      continue;
    }

    if (line.startsWith("+")) {
      additions++;
      diffLines.push({
        type: "added",
        newLineNo: newNo++,
        content: line.substring(1),
      });
    } else if (line.startsWith("-")) {
      deletions++;
      diffLines.push({
        type: "removed",
        oldLineNo: oldNo++,
        content: line.substring(1),
      });
    } else if (line.startsWith(" ") || line === "") {
      diffLines.push({
        type: "context",
        oldLineNo: oldNo++,
        newLineNo: newNo++,
        content: line.startsWith(" ") ? line.substring(1) : line,
      });
    }
  }

  return { filePath, diffLines, additions, deletions };
}
