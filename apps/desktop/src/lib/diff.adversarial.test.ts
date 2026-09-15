import { describe, expect, it } from "vitest";
import {
  computeLineDiff,
  generateSplitDiffRows,
  parseUnifiedDiff,
  type DiffLine,
} from "./diff";

describe("Adversarial Stress Suite: diff.ts", () => {
  // Oracle Helper 1: Reconstruct oldLines from diffLines
  function reconstructOldLines(diff: DiffLine[]): string[] {
    return diff
      .filter((l) => l.type === "context" || l.type === "removed")
      .map((l) => l.content);
  }

  // Oracle Helper 2: Reconstruct newLines from diffLines
  function reconstructNewLines(diff: DiffLine[]): string[] {
    return diff
      .filter((l) => l.type === "context" || l.type === "added")
      .map((l) => l.content);
  }

  // Oracle Helper 3: Split text as diff.ts does
  function split(text: string): string[] {
    if (!text) return [];
    const normalized = text.replace(/\r\n/g, "\n");
    const lines = normalized.split("\n");
    if (lines.length > 0 && lines[lines.length - 1] === "") {
      lines.pop();
    }
    return lines;
  }

  describe("1. Oracle Invariants on computeLineDiff", () => {
    it("satisfies reconstruction oracle on arbitrary inputs", () => {
      const cases: [string, string][] = [
        ["", ""],
        ["hello", ""],
        ["", "world"],
        ["a\nb\nc", "a\nx\nc"],
        ["line1\nline2\nline3\nline4", "line0\nline1\nline3-mod\nline5"],
        ["same\nsame\nsame", "same\nchanged\nsame"],
        ["alpha\nbeta\ngamma", "delta\nepsilon\nzeta"],
        ["\n\n\n", "\n"],
      ];

      for (const [oldT, newT] of cases) {
        const diff = computeLineDiff(oldT, newT);
        const expectedOld = split(oldT);
        const expectedNew = split(newT);

        expect(reconstructOldLines(diff)).toEqual(expectedOld);
        expect(reconstructNewLines(diff)).toEqual(expectedNew);
      }
    });

    it("ensures line numbers are strictly monotonic and sequential", () => {
      const oldT = "a\nb\nc\nd\ne\nf";
      const newT = "a\nb-mod\nd\ne\ng\nh";
      const diff = computeLineDiff(oldT, newT);

      let lastOld = 0;
      let lastNew = 0;
      for (const line of diff) {
        if (line.oldLineNo !== undefined) {
          expect(line.oldLineNo).toBe(lastOld + 1);
          lastOld = line.oldLineNo;
        }
        if (line.newLineNo !== undefined) {
          expect(line.newLineNo).toBe(lastNew + 1);
          lastNew = line.newLineNo;
        }
      }
      expect(lastOld).toBe(split(oldT).length);
      expect(lastNew).toBe(split(newT).length);
    });
  });

  describe("2. Extreme & Boundary Inputs", () => {
    it("handles whitespace variations without crashing or corrupting lines", () => {
      const t1 = "   \n\t\t\n  ";
      const t2 = "   \n\t \n  ";
      const diff = computeLineDiff(t1, t2);

      expect(diff).toHaveLength(4); // line 1 context, line 2 removed, line 2 added, line 3 context
      expect(diff[0].type).toBe("context");
      expect(diff[1].type).toBe("removed");
      expect(diff[2].type).toBe("added");
      expect(diff[3].type).toBe("context");
    });

    it("handles complete replacement of 500 lines", () => {
      const oldLines = Array.from({ length: 500 }, (_, i) => `old-line-${i + 1}`);
      const newLines = Array.from({ length: 500 }, (_, i) => `new-line-${i + 1}`);
      const oldText = oldLines.join("\n");
      const newText = newLines.join("\n");

      const diff = computeLineDiff(oldText, newText);

      expect(diff).toHaveLength(1000);
      const removed = diff.filter((l) => l.type === "removed");
      const added = diff.filter((l) => l.type === "added");
      const context = diff.filter((l) => l.type === "context");

      expect(removed).toHaveLength(500);
      expect(added).toHaveLength(500);
      expect(context).toHaveLength(0);
      expect(reconstructOldLines(diff)).toEqual(oldLines);
      expect(reconstructNewLines(diff)).toEqual(newLines);
    });

    it("handles large identical file (1000 lines) efficiently", () => {
      const lines = Array.from({ length: 1000 }, (_, i) => `function codeBlock${i}() { return ${i}; }`);
      const text = lines.join("\n");

      const start = Date.now();
      const diff = computeLineDiff(text, text);
      const duration = Date.now() - start;

      expect(diff).toHaveLength(1000);
      expect(diff.every((l) => l.type === "context")).toBe(true);
      expect(duration).toBeLessThan(1500); // Must complete within 1.5s
    });

    it("handles CRLF vs LF seamlessly", () => {
      const crlf = "line 1\r\nline 2\r\nline 3\r\n";
      const lf = "line 1\nline 2\nline 3\n";

      const diff = computeLineDiff(crlf, lf);
      // Normalized lines are identical, so all 3 lines must be context
      expect(diff).toHaveLength(3);
      expect(diff.every((l) => l.type === "context")).toBe(true);
    });

    it("handles extremely long single lines (minified JS - 50,000 chars)", () => {
      const long1 = "var a=" + "x".repeat(50000) + ";";
      const long2 = "var a=" + "x".repeat(50000) + "1;";
      const diff = computeLineDiff(long1, long2);
      expect(diff).toHaveLength(2);
      expect(diff[0].type).toBe("removed");
      expect(diff[1].type).toBe("added");
    });

    it("handles 2000 disjoint lines under DP table memory and time limits", () => {
      const oldLines = Array.from({ length: 2000 }, (_, i) => `old_prefix_${i}`);
      const newLines = Array.from({ length: 2000 }, (_, i) => `new_prefix_${i}`);
      const t1 = oldLines.join("\n");
      const t2 = newLines.join("\n");

      const start = Date.now();
      const diff = computeLineDiff(t1, t2);
      const elapsed = Date.now() - start;

      expect(diff).toHaveLength(4000);
      expect(elapsed).toBeLessThan(3000); // 3s budget
    });

    it("handles lone CR (\\r without \\n)", () => {
      const oldText = "line1\rline2";
      const newText = "line1\nline2";
      // \r without \n is not replaced by regex /\r\n/g, so splitLines treats oldText as 1 line containing \r
      const diff = computeLineDiff(oldText, newText);
      expect(diff.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe("3. Unicode, Emojis, and Special Characters", () => {
    it("handles multibyte emojis and complex unicode correctly", () => {
      const oldText = "Hello 🌍!\nRocket 🚀\nFamily 👨‍👩‍👧‍👦\nFlag 🇨🇵";
      const newText = "Hello 🌎!\nRocket 🚀\nFamily 👨‍👩‍👦\nFlag 🇨🇵\nBonus 🎉";

      const diff = computeLineDiff(oldText, newText);

      expect(reconstructOldLines(diff)).toEqual(split(oldText));
      expect(reconstructNewLines(diff)).toEqual(split(newText));

      expect(diff[0].type).toBe("removed");
      expect(diff[1].type).toBe("added");
      expect(diff[2].type).toBe("context"); // Rocket unchanged
    });

    it("handles RTL, CJK, and accented characters", () => {
      const oldText = "مرحبا بالعالم\n你好世界\nBonjour café";
      const newText = "مرحبا بالجميع\n你好世界\nBonjour thé";

      const diff = computeLineDiff(oldText, newText);
      expect(reconstructOldLines(diff)).toEqual(split(oldText));
      expect(reconstructNewLines(diff)).toEqual(split(newText));

      const contextLines = diff.filter((l) => l.type === "context");
      expect(contextLines).toHaveLength(1);
      expect(contextLines[0].content).toBe("你好世界");
    });

    it("handles code with HTML, script tags, backticks, quotes, and symbols", () => {
      const oldText = '<script>alert("XSS & `escape`")</script>\n<div class="test">\\n</div>';
      const newText = '<script>console.log("Safe & `escape`")</script>\n<div class="test">\\n</div>';

      const diff = computeLineDiff(oldText, newText);
      expect(reconstructOldLines(diff)).toEqual(split(oldText));
      expect(reconstructNewLines(diff)).toEqual(split(newText));
      expect(diff[2].type).toBe("context");
    });
  });

  describe("4. generateSplitDiffRows Oracle Invariants", () => {
    it("maintains strict split diff alignment and row invariants", () => {
      const oldT = "const a = 1;\nconst b = 2;\nconst c = 3;\nconst d = 4;";
      const newT = "const a = 1;\nconst b = 20;\nconst b_extra = 21;\nconst d = 4;\nconst e = 5;";

      const diff = computeLineDiff(oldT, newT);
      const rows = generateSplitDiffRows(diff);

      expect(rows.length).toBeGreaterThanOrEqual(4);

      // Invariant A: Reconstructing old column matches original old lines
      const leftCol = rows.filter((r) => r.oldLine).map((r) => r.oldLine!.content);
      expect(leftCol).toEqual(split(oldT));

      // Invariant B: Reconstructing new column matches original new lines
      const rightCol = rows.filter((r) => r.newLine).map((r) => r.newLine!.content);
      expect(rightCol).toEqual(split(newT));

      // Invariant C: Line numbers in both columns are strictly monotonically increasing
      let prevOld = 0;
      let prevNew = 0;
      for (const row of rows) {
        expect(row.oldLine !== undefined || row.newLine !== undefined).toBe(true);
        if (row.oldLine) {
          expect(row.oldLine.lineNo).toBe(prevOld + 1);
          prevOld = row.oldLine.lineNo;
        }
        if (row.newLine) {
          expect(row.newLine.lineNo).toBe(prevNew + 1);
          prevNew = row.newLine.lineNo;
        }

        if (row.oldLine?.type === "context" || row.newLine?.type === "context") {
          expect(row.oldLine?.type).toBe("context");
          expect(row.newLine?.type).toBe("context");
          expect(row.oldLine?.content).toBe(row.newLine?.content);
        }
      }
    });

    it("handles massive pure additions in split view", () => {
      const diff: DiffLine[] = Array.from({ length: 100 }, (_, i) => ({
        type: "added",
        newLineNo: i + 1,
        content: `new line ${i + 1}`,
      }));

      const rows = generateSplitDiffRows(diff);
      expect(rows).toHaveLength(100);
      for (let i = 0; i < 100; i++) {
        expect(rows[i].oldLine).toBeUndefined();
        expect(rows[i].newLine).toBeDefined();
        expect(rows[i].newLine?.lineNo).toBe(i + 1);
        expect(rows[i].newLine?.type).toBe("added");
      }
    });

    it("handles massive pure deletions in split view", () => {
      const diff: DiffLine[] = Array.from({ length: 100 }, (_, i) => ({
        type: "removed",
        oldLineNo: i + 1,
        content: `deleted line ${i + 1}`,
      }));

      const rows = generateSplitDiffRows(diff);
      expect(rows).toHaveLength(100);
      for (let i = 0; i < 100; i++) {
        expect(rows[i].newLine).toBeUndefined();
        expect(rows[i].oldLine).toBeDefined();
        expect(rows[i].oldLine?.lineNo).toBe(i + 1);
        expect(rows[i].oldLine?.type).toBe("removed");
      }
    });
  });

  describe("5. parseUnifiedDiff Adversarial Robustness", () => {
    it("handles diffs creating brand new files (/dev/null)", () => {
      const patch = `diff --git a/dev/null b/src/brand_new.ts
new file mode 100644
--- /dev/null
+++ b/src/brand_new.ts
@@ -0,0 +1,3 @@
+export const X = 1;
+export const Y = 2;
+export const Z = 3;
`;
      const parsed = parseUnifiedDiff(patch);
      expect(parsed.filePath).toBe("src/brand_new.ts");
      expect(parsed.additions).toBe(3);
      expect(parsed.deletions).toBe(0);
      expect(parsed.diffLines).toHaveLength(3);
      expect(parsed.diffLines.every((l) => l.type === "added")).toBe(true);
    });

    it("handles diffs deleting files to /dev/null", () => {
      const patch = `diff --git a/src/obsolete.ts b/dev/null
deleted file mode 100644
--- a/src/obsolete.ts
+++ /dev/null
@@ -1,2 +0,0 @@
-const dead = true;
-export default dead;
-`;
      const parsed = parseUnifiedDiff(patch);
      expect(parsed.filePath).toBe("src/obsolete.ts");
      expect(parsed.deletions).toBe(2);
      expect(parsed.additions).toBe(0);
      expect(parsed.diffLines).toHaveLength(2);
      expect(parsed.diffLines.every((l) => l.type === "removed")).toBe(true);
    });

    it("gracefully ignores \\ No newline at end of file markers and does not add phantom line", () => {
      const patch = `@@ -1,2 +1,2 @@\n-old line\n\\ No newline at end of file\n+new line\n\\ No newline at end of file`;
      const parsed = parseUnifiedDiff(patch);
      expect(parsed.diffLines).toHaveLength(2);
      expect(parsed.diffLines[0].type).toBe("removed");
      expect(parsed.diffLines[1].type).toBe("added");
    });

    it("survives malformed headers and corrupted patch strings without throwing", () => {
      const corruptedPatches = [
        "not a diff at all",
        "@@ -invalid,header @@\n+foo\n-bar",
        "@@ @@ @@\n+added line",
        "--- missing plus\n@@ -1,1 +1,1 @@\n context",
        "+++ \n--- \n@@ @@",
        "",
        "   \n\n\t",
      ];

      for (const badPatch of corruptedPatches) {
        expect(() => parseUnifiedDiff(badPatch)).not.toThrow();
      }
    });
  });

  describe("6. Property-Based Fuzzing Invariant Check", () => {
    it("passes 50 random fuzzing permutations with 100% oracle reconstruction", () => {
      const vocabulary = [
        "import { something } from 'somewhere';",
        "const x = 42;",
        "let flag = false;",
        "function compute(a: number, b: number): number {",
        "  return a + b;",
        "}",
        "export default compute;",
        "// this is a comment",
        "console.log('debug output');",
        "if (flag) { throw new Error('boom'); }",
      ];

      // Pseudorandom generator with fixed seed for determinism
      let seed = 123456789;
      function random() {
        seed = (seed * 1664525 + 1013904223) % 4294967296;
        return seed / 4294967296;
      }

      function generateRandomFile(numLines: number): string {
        const lines: string[] = [];
        for (let i = 0; i < numLines; i++) {
          const idx = Math.floor(random() * vocabulary.length);
          lines.push(`${vocabulary[idx]} // line ${i}`);
        }
        return lines.join("\n");
      }

      for (let iteration = 0; iteration < 50; iteration++) {
        const len1 = Math.floor(random() * 25) + 1;
        const len2 = Math.floor(random() * 25) + 1;
        const oldText = generateRandomFile(len1);
        const newText = generateRandomFile(len2);

        const diff = computeLineDiff(oldText, newText);
        const splitRows = generateSplitDiffRows(diff);

        // Oracle check on diff
        expect(reconstructOldLines(diff)).toEqual(split(oldText));
        expect(reconstructNewLines(diff)).toEqual(split(newText));

        // Oracle check on split rows
        const splitOld = splitRows.filter((r) => r.oldLine).map((r) => r.oldLine!.content);
        const splitNew = splitRows.filter((r) => r.newLine).map((r) => r.newLine!.content);
        expect(splitOld).toEqual(split(oldText));
        expect(splitNew).toEqual(split(newText));
      }
    });
  });
});
