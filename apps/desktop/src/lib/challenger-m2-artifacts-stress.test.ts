import { describe, it, expect } from "vitest";
import {
  extractArtifactsFromMessages,
  mergeArtifacts,
  type WorkspaceArtifact,
} from "./artifacts";
import type { ChatMessage } from "./types";

describe("Empirical Challenger M2 Iteration 2: Extreme Artifacts Stress Suite", () => {
  describe("1. Extreme Scaling & ReDoS Resistance", () => {
    it("processes 1,000 messages with 2,000 code blocks within 1,000ms", () => {
      const messages: ChatMessage[] = [];
      for (let i = 0; i < 1000; i++) {
        const isAssistant = i % 2 === 0;
        messages.push({
          id: `msg-${i}`,
          conversationId: "extreme-scale",
          role: isAssistant ? "assistant" : "user",
          content: isAssistant
            ? `Prefix text ${i}\n\`\`\`diff filepath="src/file_${i}.ts"\n--- a/src/file_${i}.ts\n+++ b/src/file_${i}.ts\n@@ -1,1 +1,2 @@\n-old_${i}\n+new_${i}\n+extra_${i}\n\`\`\`\n` +
              `And also a code block:\n\`\`\`ts filepath="src/code_${i}.ts"\nexport const v${i} = ${i};\n\`\`\``
            : `User says ${i} with \`\`\`ts filepath="ignored_${i}.ts"\nconst x = ${i};\n\`\`\``,
          createdAt: "2026-09-05T00:00:00Z",
        });
      }

      const start = performance.now();
      const artifacts = extractArtifactsFromMessages(messages, "extreme-scale");
      const duration = performance.now() - start;

      // 500 assistant messages * 2 artifacts each = 1,000 artifacts
      expect(artifacts).toHaveLength(1000);
      expect(duration).toBeLessThan(1000); // Strict performance budget
    });

    it("survives 1,000,000-character payload with unclosed fence without catastrophic backtracking", () => {
      // Adversarial payload designed to trigger regex backtracking
      const repetition = "```diff filepath=\"src/test.ts\"\n" + "+addition\n".repeat(50000);
      const messages: ChatMessage[] = [
        {
          id: "huge-redos",
          conversationId: "conv-redos",
          role: "assistant",
          content: repetition, // no closing fence
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const start = performance.now();
      const artifacts = extractArtifactsFromMessages(messages, "conv-redos");
      const duration = performance.now() - start;

      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/test.ts");
      expect(artifacts[0].additions).toBe(50000);
      expect(duration).toBeLessThan(1500);
    });

    it("processes 100 code blocks inside a single message", () => {
      const blocks: string[] = [];
      for (let i = 0; i < 100; i++) {
        blocks.push(
          `\`\`\`ts filepath="src/block_${i}.ts"\nexport const item${i} = "${i}";\n\`\`\``
        );
      }
      const messages: ChatMessage[] = [
        {
          id: "multi-block",
          conversationId: "conv-multi",
          role: "assistant",
          content: blocks.join("\n\n"),
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(100);
      expect(artifacts[0].filePath).toBe("src/block_0.ts");
      expect(artifacts[99].filePath).toBe("src/block_99.ts");
    });
  });

  describe("2. Malformed & Adversarial Markdown Delimiters", () => {
    it("handles 0-byte, whitespace-only, and non-string messages gracefully", () => {
      const invalidMessages: any[] = [
        null,
        undefined,
        {},
        { role: "assistant", content: "" },
        { role: "assistant", content: "   \n\t  " },
        { role: "assistant", content: null },
        { role: "assistant", content: 12345 },
        { role: "system", content: "system message" },
      ];

      expect(extractArtifactsFromMessages(invalidMessages)).toEqual([]);
    });

    it("handles unclosed single quotes and double quotes in filepath attribute", () => {
      const messages: ChatMessage[] = [
        {
          id: "bad-quotes",
          conversationId: "conv-1",
          role: "assistant",
          content: '```ts filepath="unclosed.ts\nconst x = 1;\n```',
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      // Does not throw
      const artifacts = extractArtifactsFromMessages(messages);
      // If regex requires matching closing quote, it falls back or skips safely
      expect(Array.isArray(artifacts)).toBe(true);
    });

    it("handles uppercase and mixed case FILEPATH attributes", () => {
      const messages: ChatMessage[] = [
        {
          id: "case-msg",
          conversationId: "conv-1",
          role: "assistant",
          content: '```ts FILEPATH="src/upper.ts"\nconst x = 1;\n```\n```ts Path="src/mixed.ts"\nconst y = 2;\n```',
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(2);
      expect(artifacts[0].filePath).toBe("src/upper.ts");
      expect(artifacts[1].filePath).toBe("src/mixed.ts");
    });

    it("handles Windows absolute and backslash paths safely", () => {
      const messages: ChatMessage[] = [
        {
          id: "win-path-msg",
          conversationId: "conv-1",
          role: "assistant",
          content: '```ts filepath="C:\\Users\\ARO\\src\\core\\engine.ts"\nexport const v = 1;\n```',
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      // Backslashes must be normalized to POSIX slashes
      expect(artifacts[0].filePath).toBe("C:/Users/ARO/src/core/engine.ts");
      expect(artifacts[0].title).toBe("engine.ts");
    });

    it("handles relative path navigation prefixes (././ and /)", () => {
      const messages: ChatMessage[] = [
        {
          id: "prefix-msg",
          conversationId: "conv-1",
          role: "assistant",
          content: '```ts filepath="./././src/nested/app.ts"\nexport const z = 99;\n```',
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/nested/app.ts");
    });

    it("handles unicode, emoji, and special characters in paths", () => {
      const messages: ChatMessage[] = [
        {
          id: "unicode-msg",
          conversationId: "conv-1",
          role: "assistant",
          content: '```ts filepath="src/📁 dossiers/modèle_élève_#1.ts"\nexport const a = "test";\n```',
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/📁 dossiers/modèle_élève_#1.ts");
      expect(artifacts[0].title).toBe("modèle_élève_#1.ts");
    });
  });

  describe("3. Diff Extraction & Metrics Calculation", () => {
    it("handles git diff headers without explicit attribute", () => {
      const messages: ChatMessage[] = [
        {
          id: "git-diff-msg",
          conversationId: "conv-1",
          role: "assistant",
          content:
            '```diff\ndiff --git a/packages/core/index.ts b/packages/core/index.ts\n--- a/packages/core/index.ts\n+++ b/packages/core/index.ts\n@@ -1,2 +1,3 @@\n-const a = 1;\n+const a = 2;\n+const b = 3;\n```',
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("packages/core/index.ts");
      expect(artifacts[0].title).toBe("index.ts");
      expect(artifacts[0].additions).toBe(2);
      expect(artifacts[0].deletions).toBe(1);
    });

    it("handles git binary patch headers without crash", () => {
      const messages: ChatMessage[] = [
        {
          id: "binary-diff-msg",
          conversationId: "conv-1",
          role: "assistant",
          content:
            '```diff filepath="assets/logo.png"\nBinary files a/assets/logo.png and b/assets/logo.png differ\n```',
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("assets/logo.png");
      expect(artifacts[0].additions).toBe(0);
      expect(artifacts[0].deletions).toBe(0);
    });

    it("handles completely empty diff content", () => {
      const messages: ChatMessage[] = [
        {
          id: "empty-diff",
          conversationId: "conv-1",
          role: "assistant",
          content: '```diff\n\n```',
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].kind).toBe("diff");
      expect(artifacts[0].additions).toBe(0);
      expect(artifacts[0].deletions).toBe(0);
    });
  });

  describe("4. mergeArtifacts Robustness & Anomaly Invariants", () => {
    it("handles negative and extreme deltas in agent artifacts safely", () => {
      const agentArtifacts = [
        {
          id: "ag-1",
          filePath: "src/bad-metrics.ts",
          metadata: { additions: -50, deletions: -10 },
        },
        {
          id: "ag-2",
          filePath: "src/inf-metrics.ts",
          metadata: { additions: 999999, deletions: 0 },
        },
      ];

      const merged = mergeArtifacts(agentArtifacts, []);
      expect(merged).toHaveLength(2);
      // Negative deltas are clamped to 0 by Math.max(0, ...)
      expect(merged[0].additions).toBe(0);
      expect(merged[0].deletions).toBe(0);
      expect(merged[1].additions).toBe(999999);
    });

    it("handles sparse arrays and prototype property pollution safely", () => {
      const sparseAgent: any[] = new Array(10);
      sparseAgent[2] = { id: "valid-1", filePath: "src/valid.ts" };
      sparseAgent[5] = null;
      sparseAgent[8] = { __proto__: { evil: true } };

      const merged = mergeArtifacts(sparseAgent, []);
      expect(Array.isArray(merged)).toBe(true);
      expect(merged.length).toBeGreaterThanOrEqual(1);
      expect(merged.some((m) => m.filePath === "src/valid.ts")).toBe(true);
    });

    it("guarantees unique keys for identical filepaths across duplicate blocks", () => {
      const messages: ChatMessage[] = [
        {
          id: "msg-dup",
          conversationId: "conv-1",
          role: "assistant",
          content: [
            '```ts filepath="src/dup.ts"\nconst x = 1;\n```',
            '```ts filepath="src/dup.ts"\nconst x = 2;\n```',
            '```ts filepath="src/dup.ts"\nconst x = 3;\n```',
          ].join("\n\n"),
          createdAt: "2026-09-05T00:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(3);
      const uniqueKeys = new Set(artifacts.map((a) => a.uniqueKey));
      expect(uniqueKeys.size).toBe(3);
    });
  });
});
