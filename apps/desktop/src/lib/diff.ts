/**
 * Pure, robust Diff Engine for ARO AI Workspace.
 *
 * Implements:
 * - Line-by-line diff computation using Longest Common Subsequence (LCS).
 * - Side-by-side (Split) diff alignment with synchronized rows.
 * - Unified diff patch parser supporting Git and standard unidiff formats.
 */

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
  additions?: number;
  deletions?: number;
}

/**
 * Split text into an array of normalized lines.
 * Strips \r and drops trailing empty line from standard final newline.
 */
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

  if (m === 0 && n === 0) {
    return [];
  }

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

  // Compute LCS table
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));

  for (let i = 0; i < m; i++) {
    for (let j = 0; j < n; j++) {
      if (oldLines[i] === newLines[j]) {
        dp[i + 1][j + 1] = dp[i][j] + 1;
      } else {
        dp[i + 1][j + 1] = Math.max(dp[i + 1][j], dp[i][j + 1]);
      }
    }
  }

  // Backtrack to reconstruct diff lines
  let i = m;
  let j = n;
  const reversedDiff: DiffLine[] = [];

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      reversedDiff.push({
        type: "context",
        oldLineNo: i,
        newLineNo: j,
        content: oldLines[i - 1],
      });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      reversedDiff.push({
        type: "added",
        newLineNo: j,
        content: newLines[j - 1],
      });
      j--;
    } else if (i > 0 && (j === 0 || dp[i][j - 1] < dp[i - 1][j])) {
      reversedDiff.push({
        type: "removed",
        oldLineNo: i,
        content: oldLines[i - 1],
      });
      i--;
    }
  }

  return reversedDiff.reverse();
}

/**
 * Generates aligned side-by-side (Split) diff rows from a unified list of DiffLines.
 * Pairs corresponding removed and added lines across matching modification blocks.
 */
export function generateSplitDiffRows(diffLines: DiffLine[]): SplitDiffRow[] {
  if (!diffLines || !Array.isArray(diffLines)) return [];
  const rows: SplitDiffRow[] = [];
  let i = 0;

  while (i < diffLines.length) {
    const line = diffLines[i];
    if (!line) {
      i++;
      continue;
    }

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
      i++;
    } else {
      // Collect contiguous block of modifications (removals and additions)
      const removedBlock: DiffLine[] = [];
      const addedBlock: DiffLine[] = [];

      while (i < diffLines.length && diffLines[i] && diffLines[i].type !== "context") {
        if (diffLines[i].type === "removed") {
          removedBlock.push(diffLines[i]);
        } else if (diffLines[i].type === "added") {
          addedBlock.push(diffLines[i]);
        }
        i++;
      }

      const maxLen = Math.max(removedBlock.length, addedBlock.length);
      for (let k = 0; k < maxLen; k++) {
        const row: SplitDiffRow = {};
        if (k < removedBlock.length) {
          row.oldLine = {
            lineNo: removedBlock[k].oldLineNo ?? 0,
            content: removedBlock[k].content,
            type: "removed",
          };
        }
        if (k < addedBlock.length) {
          row.newLine = {
            lineNo: addedBlock[k].newLineNo ?? 0,
            content: addedBlock[k].content,
            type: "added",
          };
        }
        rows.push(row);
      }
    }
  }

  return rows;
}

function cleanCandidatePath(candidate: string | undefined): string | undefined {
  if (!candidate) return undefined;
  let cleaned = candidate.trim().replace(/\t.*$/, ""); // strip timestamps
  cleaned = cleaned.replace(/^"|"$/g, ""); // strip quotes
  cleaned = cleaned.replace(/^[ab]\//, ""); // strip git a/ or b/ prefixes
  if (cleaned === "dev/null" || cleaned === "/dev/null" || cleaned === "") {
    return undefined;
  }
  return cleaned;
}

/**
 * Parses a unified diff string into structured DiffLines, extracting file path and delta metrics.
 */
export function parseUnifiedDiff(patch: string): ParsedUnifiedDiff {
  if (!patch || !patch.trim()) {
    return { filePath: undefined, diffLines: [], additions: 0, deletions: 0 };
  }

  const rawLines = splitLines(patch);
  while (rawLines.length > 0 && rawLines[rawLines.length - 1] === "") {
    rawLines.pop();
  }
  let filePath: string | undefined = undefined;
  const diffLines: DiffLine[] = [];
  let additions = 0;
  let deletions = 0;

  let inHunk = false;
  let oldLineCursor = 1;
  let newLineCursor = 1;
  let oldRemaining = 0;
  let newRemaining = 0;
  let hasCount = false;

  for (let i = 0; i < rawLines.length; i++) {
    const line = rawLines[i];

    // Detect file path from git or standard headers
    if (line.startsWith("diff --git ")) {
      const match =
        line.match(/^diff --git (?:a\/|"(?:a\/)?)(.+?)"? (?:b\/|"(?:b\/)?)(.+)"?$/) ||
        line.match(/^diff --git (.+?) (.+)$/);
      if (match) {
        const p1 = cleanCandidatePath(match[1]);
        const p2 = cleanCandidatePath(match[2]);
        filePath = p2 || p1 || filePath;
      }
    } else if (line.startsWith("+++ ")) {
      const p = cleanCandidatePath(line.substring(4));
      if (p) filePath = p;
    } else if (line.startsWith("--- ")) {
      const p = cleanCandidatePath(line.substring(4));
      if (p && !filePath) filePath = p;
    }

    // Detect hunk header @@ -oldStart,oldCount +newStart,newCount @@
    if (line.startsWith("@@ ")) {
      inHunk = true;
      hasCount = false;
      oldRemaining = 0;
      newRemaining = 0;
      const parts = line.split("@@");
      if (parts.length >= 3) {
        const header = parts[1].trim();
        const ranges = header.split(" ");
        for (const range of ranges) {
          if (range.startsWith("-")) {
            const val = range.substring(1);
            const [startStr, countStr] = val.split(",");
            const parsedStart = parseInt(startStr, 10);
            if (!isNaN(parsedStart)) oldLineCursor = parsedStart;
            if (countStr !== undefined) {
              const parsedCount = parseInt(countStr, 10);
              if (!isNaN(parsedCount)) {
                oldRemaining = parsedCount;
                hasCount = true;
              }
            } else if (!isNaN(parsedStart)) {
              oldRemaining = parsedStart === 0 ? 0 : 1;
              hasCount = true;
            }
          } else if (range.startsWith("+")) {
            const val = range.substring(1);
            const [startStr, countStr] = val.split(",");
            const parsedStart = parseInt(startStr, 10);
            if (!isNaN(parsedStart)) newLineCursor = parsedStart;
            if (countStr !== undefined) {
              const parsedCount = parseInt(countStr, 10);
              if (!isNaN(parsedCount)) {
                newRemaining = parsedCount;
                hasCount = true;
              }
            } else if (!isNaN(parsedStart)) {
              newRemaining = parsedStart === 0 ? 0 : 1;
              hasCount = true;
            }
          }
        }
      }
      continue;
    }

    if (inHunk) {
      if (line.startsWith("+") && !line.startsWith("+++")) {
        const content = line.substring(1);
        diffLines.push({
          type: "added",
          newLineNo: newLineCursor++,
          content,
        });
        additions++;
        if (hasCount) newRemaining--;
      } else if (line.startsWith("-") && !line.startsWith("---")) {
        const content = line.substring(1);
        diffLines.push({
          type: "removed",
          oldLineNo: oldLineCursor++,
          content,
        });
        deletions++;
        if (hasCount) oldRemaining--;
      } else if (line.startsWith(" ") || line === "") {
        const content = line.startsWith(" ") ? line.substring(1) : line;
        diffLines.push({
          type: "context",
          oldLineNo: oldLineCursor++,
          newLineNo: newLineCursor++,
          content,
        });
        if (hasCount) {
          oldRemaining--;
          newRemaining--;
        }
      } else if (line.startsWith("\\ No newline at end of file") || line.startsWith("\\\\ No newline at end of file")) {
        // Standard diff marker, skip
        continue;
      } else if (line.startsWith("diff --git ") || line.startsWith("index ")) {
        inHunk = false;
      }

      if (hasCount && oldRemaining <= 0 && newRemaining <= 0) {
        inHunk = false;
      }
    }
  }

  // Fallback: If no @@ hunks were present but lines start with + / -
  if (diffLines.length === 0 && rawLines.some((l) => l.startsWith("+") || l.startsWith("-"))) {
    let oLine = 1;
    let nLine = 1;
    for (const line of rawLines) {
      if (line.startsWith("+++") || line.startsWith("---")) continue;
      if (line.startsWith("+")) {
        diffLines.push({
          type: "added",
          newLineNo: nLine++,
          content: line.substring(1),
        });
        additions++;
      } else if (line.startsWith("-")) {
        diffLines.push({
          type: "removed",
          oldLineNo: oLine++,
          content: line.substring(1),
        });
        deletions++;
      } else {
        const content = line.startsWith(" ") ? line.substring(1) : line;
        diffLines.push({
          type: "context",
          oldLineNo: oLine++,
          newLineNo: nLine++,
          content,
        });
      }
    }
  }

  return {
    filePath,
    diffLines,
    additions,
    deletions,
  };
}
