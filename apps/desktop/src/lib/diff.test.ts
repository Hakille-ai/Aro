import { describe, expect, it } from "vitest";
import {
  computeLineDiff,
  generateSplitDiffRows,
  parseUnifiedDiff,
  type DiffLine,
} from "./diff";

describe("diff engine: computeLineDiff", () => {
  it("handles empty strings", () => {
    expect(computeLineDiff("", "")).toEqual([]);
    expect(computeLineDiff("   ", "   ")).toEqual([
      { type: "context", oldLineNo: 1, newLineNo: 1, content: "   " },
    ]);
  });

  it("handles pure additions", () => {
    const result = computeLineDiff("", "first\nsecond\nthird");
    expect(result).toHaveLength(3);
    expect(result.every((l) => l.type === "added")).toBe(true);
    expect(result[0]).toEqual({ type: "added", newLineNo: 1, content: "first" });
    expect(result[1]).toEqual({ type: "added", newLineNo: 2, content: "second" });
    expect(result[2]).toEqual({ type: "added", newLineNo: 3, content: "third" });
  });

  it("handles pure deletions", () => {
    const result = computeLineDiff("line 1\nline 2", "");
    expect(result).toHaveLength(2);
    expect(result.every((l) => l.type === "removed")).toBe(true);
    expect(result[0]).toEqual({ type: "removed", oldLineNo: 1, content: "line 1" });
    expect(result[1]).toEqual({ type: "removed", oldLineNo: 2, content: "line 2" });
  });

  it("handles identical content as context", () => {
    const text = "const a = 1;\nconst b = 2;\nconsole.log(a + b);";
    const result = computeLineDiff(text, text);
    expect(result).toHaveLength(3);
    expect(result.every((l) => l.type === "context")).toBe(true);
    expect(result[0].oldLineNo).toBe(1);
    expect(result[0].newLineNo).toBe(1);
    expect(result[2].oldLineNo).toBe(3);
    expect(result[2].newLineNo).toBe(3);
  });

  it("handles line replacement in the middle", () => {
    const oldText = "first\nmiddle-old\nlast";
    const newText = "first\nmiddle-new\nlast";
    const result = computeLineDiff(oldText, newText);

    expect(result).toHaveLength(4);
    expect(result[0]).toEqual({ type: "context", oldLineNo: 1, newLineNo: 1, content: "first" });
    expect(result[1]).toEqual({ type: "removed", oldLineNo: 2, content: "middle-old" });
    expect(result[2]).toEqual({ type: "added", newLineNo: 2, content: "middle-new" });
    expect(result[3]).toEqual({ type: "context", oldLineNo: 3, newLineNo: 3, content: "last" });
  });

  it("handles additions and deletions at extremities", () => {
    const oldText = "item 2";
    const newText = "item 1\nitem 2\nitem 3";
    const result = computeLineDiff(oldText, newText);

    expect(result).toEqual([
      { type: "added", newLineNo: 1, content: "item 1" },
      { type: "context", oldLineNo: 1, newLineNo: 2, content: "item 2" },
      { type: "added", newLineNo: 3, content: "item 3" },
    ]);
  });

  it("normalizes carriage returns (\\r\\n)", () => {
    const oldText = "line 1\r\nline 2\r\n";
    const newText = "line 1\nline 2-mod\n";
    const result = computeLineDiff(oldText, newText);

    expect(result).toHaveLength(3);
    expect(result[0]).toEqual({ type: "context", oldLineNo: 1, newLineNo: 1, content: "line 1" });
    expect(result[1]).toEqual({ type: "removed", oldLineNo: 2, content: "line 2" });
    expect(result[2]).toEqual({ type: "added", newLineNo: 2, content: "line 2-mod" });
  });
});

describe("diff engine: generateSplitDiffRows", () => {
  it("returns empty array for empty diffLines", () => {
    expect(generateSplitDiffRows([])).toEqual([]);
  });

  it("aligns context lines symmetrically", () => {
    const lines: DiffLine[] = [
      { type: "context", oldLineNo: 1, newLineNo: 1, content: "hello" },
      { type: "context", oldLineNo: 2, newLineNo: 2, content: "world" },
    ];
    const rows = generateSplitDiffRows(lines);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toEqual({
      oldLine: { lineNo: 1, content: "hello", type: "context" },
      newLine: { lineNo: 1, content: "hello", type: "context" },
    });
    expect(rows[1]).toEqual({
      oldLine: { lineNo: 2, content: "world", type: "context" },
      newLine: { lineNo: 2, content: "world", type: "context" },
    });
  });

  it("aligns replaced lines side-by-side", () => {
    const lines: DiffLine[] = [
      { type: "context", oldLineNo: 1, newLineNo: 1, content: "start" },
      { type: "removed", oldLineNo: 2, content: "old code" },
      { type: "added", newLineNo: 2, content: "new code" },
      { type: "context", oldLineNo: 3, newLineNo: 3, content: "end" },
    ];
    const rows = generateSplitDiffRows(lines);
    expect(rows).toHaveLength(3);
    expect(rows[1]).toEqual({
      oldLine: { lineNo: 2, content: "old code", type: "removed" },
      newLine: { lineNo: 2, content: "new code", type: "added" },
    });
  });

  it("handles unbalanced replacement blocks", () => {
    const lines: DiffLine[] = [
      { type: "removed", oldLineNo: 1, content: "remove 1" },
      { type: "removed", oldLineNo: 2, content: "remove 2" },
      { type: "added", newLineNo: 1, content: "add 1" },
    ];
    const rows = generateSplitDiffRows(lines);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toEqual({
      oldLine: { lineNo: 1, content: "remove 1", type: "removed" },
      newLine: { lineNo: 1, content: "add 1", type: "added" },
    });
    expect(rows[1]).toEqual({
      oldLine: { lineNo: 2, content: "remove 2", type: "removed" },
      newLine: undefined,
    });
  });

  it("handles pure additions and pure deletions", () => {
    const lines: DiffLine[] = [
      { type: "added", newLineNo: 1, content: "brand new 1" },
      { type: "added", newLineNo: 2, content: "brand new 2" },
    ];
    const rows = generateSplitDiffRows(lines);
    expect(rows).toHaveLength(2);
    expect(rows[0].oldLine).toBeUndefined();
    expect(rows[0].newLine?.content).toBe("brand new 1");
    expect(rows[1].oldLine).toBeUndefined();
    expect(rows[1].newLine?.content).toBe("brand new 2");
  });
});

describe("diff engine: parseUnifiedDiff", () => {
  it("handles empty input", () => {
    const result = parseUnifiedDiff("");
    expect(result.diffLines).toEqual([]);
    expect(result.filePath).toBeUndefined();
    expect(result.additions).toBe(0);
    expect(result.deletions).toBe(0);
  });

  it("parses standard git unified diff with hunks", () => {
    const patch = `diff --git a/src/App.svelte b/src/App.svelte
index 45ab12..89cd34 100644
--- a/src/App.svelte
+++ b/src/App.svelte
@@ -10,4 +10,5 @@
 import Header from "./Header.svelte";
-import OldComponent from "./Old.svelte";
+import NewComponent from "./New.svelte";
+import ExtraComponent from "./Extra.svelte";
 export let title = "App";
`;
    const result = parseUnifiedDiff(patch);
    expect(result.filePath).toBe("src/App.svelte");
    expect(result.additions).toBe(2);
    expect(result.deletions).toBe(1);

    expect(result.diffLines[0]).toEqual({
      type: "context",
      oldLineNo: 10,
      newLineNo: 10,
      content: 'import Header from "./Header.svelte";',
    });
    expect(result.diffLines[1]).toEqual({
      type: "removed",
      oldLineNo: 11,
      content: 'import OldComponent from "./Old.svelte";',
    });
    expect(result.diffLines[2]).toEqual({
      type: "added",
      newLineNo: 11,
      content: 'import NewComponent from "./New.svelte";',
    });
    expect(result.diffLines[3]).toEqual({
      type: "added",
      newLineNo: 12,
      content: 'import ExtraComponent from "./Extra.svelte";',
    });
  });

  it("parses simplified diff without hunks", () => {
    const patch = `+let x = 10;
-let x = 5;
 let y = 20;`;
    const result = parseUnifiedDiff(patch);
    expect(result.additions).toBe(1);
    expect(result.deletions).toBe(1);
    expect(result.diffLines).toHaveLength(3);
    expect(result.diffLines[0]).toEqual({ type: "added", newLineNo: 1, content: "let x = 10;" });
    expect(result.diffLines[1]).toEqual({ type: "removed", oldLineNo: 1, content: "let x = 5;" });
    expect(result.diffLines[2]).toEqual({ type: "context", oldLineNo: 2, newLineNo: 2, content: "let y = 20;" });
  });
});
