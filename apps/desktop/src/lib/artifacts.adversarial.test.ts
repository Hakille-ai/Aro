import { describe, it, expect } from "vitest";
import {
  extractArtifactsFromMessages,
  mergeArtifacts,
  type WorkspaceArtifact,
} from "./artifacts";
import type { ChatMessage } from "./types";

describe("Adversarial Stress Suite: artifacts.ts", () => {
  describe("1. Malformed Markdown & Unclosed Code Fences", () => {
    it("handles unclosed diff code fence at end of message", () => {
      const messages: ChatMessage[] = [
        {
          id: "m1",
          conversationId: "conv-1",
          role: "assistant",
          content: 'Here is an incomplete response:\n```diff filepath="src/broken.ts"\n--- a/src/broken.ts\n+++ b/src/broken.ts\n@@ -1,1 +1,2 @@\n-old\n+new\n+extra',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/broken.ts");
      expect(artifacts[0].kind).toBe("diff");
      expect(artifacts[0].additions).toBe(2);
      expect(artifacts[0].deletions).toBe(1);
    });

    it("handles unclosed code fence with code block", () => {
      const messages: ChatMessage[] = [
        {
          id: "m2",
          conversationId: "conv-1",
          role: "assistant",
          content: '```ts filepath="src/unclosed.ts"\nconst a = 1;\nconst b = 2;\n',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/unclosed.ts");
      expect(artifacts[0].kind).toBe("code");
      expect(artifacts[0].additions).toBe(2);
    });

    it("handles empty fence blocks", () => {
      const messages: ChatMessage[] = [
        {
          id: "m3",
          conversationId: "conv-1",
          role: "assistant",
          content: '```diff filepath="empty.diff"\n```\n\n```ts filepath="empty.ts"\n```',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(2);
      expect(artifacts[0].filePath).toBe("empty.diff");
      expect(artifacts[0].additions).toBe(0);
      expect(artifacts[0].deletions).toBe(0);
      expect(artifacts[1].filePath).toBe("empty.ts");
      expect(artifacts[1].additions).toBe(0);
    });

    it("handles nested code fences inside markdown content", () => {
      const messages: ChatMessage[] = [
        {
          id: "m4",
          conversationId: "conv-1",
          role: "assistant",
          content:
            '```markdown filepath="README.md"\n# Guide\n\nUse code like this:\n```js\nconsole.log("hello");\n```\nEnd guide.\n```',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts.length).toBeGreaterThanOrEqual(1);
      expect(artifacts[0].filePath).toBe("README.md");
    });

    it("handles CRLF line endings in code fences and headers", () => {
      const messages: ChatMessage[] = [
        {
          id: "m5",
          conversationId: "conv-1",
          role: "assistant",
          content:
            '```diff filepath="src/crlf.ts"\r\n--- a/src/crlf.ts\r\n+++ b/src/crlf.ts\r\n@@ -1 +1 @@\r\n-old\r\n+new\r\n```',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/crlf.ts");
      expect(artifacts[0].additions).toBe(1);
      expect(artifacts[0].deletions).toBe(1);
    });
  });

  describe("2. Header Attribute Variations & Fallbacks", () => {
    it("extracts path from varied attribute names: filepath, path, filename, file", () => {
      const messages: ChatMessage[] = [
        {
          id: "m6",
          conversationId: "conv-1",
          role: "assistant",
          content: [
            '```ts filepath="src/f1.ts"\nexport const a = 1;\n```',
            '```ts path="src/f2.ts"\nexport const b = 2;\n```',
            '```ts filename="src/f3.ts"\nexport const c = 3;\n```',
            '```ts file="src/f4.ts"\nexport const d = 4;\n```',
          ].join("\n\n"),
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(4);
      expect(artifacts.map((a) => a.filePath)).toEqual([
        "src/f1.ts",
        "src/f2.ts",
        "src/f3.ts",
        "src/f4.ts",
      ]);
    });

    it("extracts path from unquoted second token if it has extension", () => {
      const messages: ChatMessage[] = [
        {
          id: "m7",
          conversationId: "conv-1",
          role: "assistant",
          content: '```typescript src/components/Badge.svelte\n<div>badge</div>\n```',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/components/Badge.svelte");
      expect(artifacts[0].title).toBe("Badge.svelte");
    });

    it("extracts fallback path from git diff header if no attribute provided", () => {
      const messages: ChatMessage[] = [
        {
          id: "m8",
          conversationId: "conv-1",
          role: "assistant",
          content:
            '```diff\n--- a/src/lib/helper.ts\n+++ b/src/lib/helper.ts\n@@ -1 +1 @@\n-a\n+b\n```',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/lib/helper.ts");
      expect(artifacts[0].title).toBe("helper.ts");
    });

    it("generates synthetic patch path if diff has neither attribute nor git header", () => {
      const messages: ChatMessage[] = [
        {
          id: "msg-99",
          conversationId: "conv-1",
          role: "assistant",
          content: '```diff\n@@ -1,2 +1,2 @@\n-old line\n+new line\n```',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toMatch(/^patch_msg-99_\d+\.diff$/);
    });
  });

  describe("3. Path Normalization, Nested, Traversal, and Unicode", () => {
    it("normalizes leading slashes, dot-slashes, and Windows backslashes", () => {
      const messages: ChatMessage[] = [
        {
          id: "m9",
          conversationId: "conv-1",
          role: "assistant",
          content: [
            '```ts filepath="././src/a.ts"\nconst a = 1;\n```',
            '```ts filepath="///src/b.ts"\nconst b = 1;\n```',
            '```ts filepath="src\\components\\nested\\Button.svelte"\n<button />\n```',
          ].join("\n"),
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(3);
      expect(artifacts[0].filePath).toBe("src/a.ts");
      expect(artifacts[1].filePath).toBe("src/b.ts");
      expect(artifacts[2].filePath).toBe("src/components/nested/Button.svelte");
    });

    it("handles deeply nested paths (10+ directory levels)", () => {
      const deepPath = "packages/core/src/internal/engine/pipeline/stages/parser/lexer/token.ts";
      const messages: ChatMessage[] = [
        {
          id: "m10",
          conversationId: "conv-1",
          role: "assistant",
          content: `\`\`\`ts filepath="${deepPath}"\nexport interface Token {}\n\`\`\``,
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe(deepPath);
      expect(artifacts[0].title).toBe("token.ts");
    });

    it("handles unicode, accented, and non-latin paths", () => {
      const unicodePath = "src/fonctionnalités/modèles/élèves.ts";
      const messages: ChatMessage[] = [
        {
          id: "m11",
          conversationId: "conv-1",
          role: "assistant",
          content: `\`\`\`ts filepath="${unicodePath}"\nexport const x = 42;\n\`\`\``,
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe(unicodePath);
      expect(artifacts[0].title).toBe("élèves.ts");
    });
  });

  describe("4. 100+ Simulated Messages & Scalability", () => {
    it("processes 120 messages with multiple code blocks under 500ms", () => {
      const messages: ChatMessage[] = [];
      const roles: ("user" | "assistant" | "system")[] = ["user", "assistant", "system"];

      for (let i = 0; i < 120; i++) {
        const role = roles[i % 3];
        const content =
          role === "assistant"
            ? `Message ${i}\n\`\`\`diff filepath="src/file_${i}.ts"\n--- a/src/file_${i}.ts\n+++ b/src/file_${i}.ts\n@@ -1,2 +1,3 @@\n-line\n+line A\n+line B\n\`\`\`\n` +
              `And also:\n\`\`\`json filepath="config/file_${i}.json"\n{"id": ${i}}\n\`\`\``
            : `User comment ${i} with \`\`\`ts filepath="ignored.ts"\nconst x = 1;\n\`\`\``;

        messages.push({
          id: `msg-${i}`,
          conversationId: "stress-conv",
          role,
          content,
          createdAt: new Date(1700000000000 + i * 1000).toISOString(),
        });
      }

      const start = performance.now();
      const artifacts = extractArtifactsFromMessages(messages, "stress-conv");
      const elapsed = performance.now() - start;

      // 40 assistant messages * 2 artifacts each = 80 artifacts
      expect(artifacts).toHaveLength(80);
      expect(elapsed).toBeLessThan(500); // 500ms budget
    });

    it("survives 100,000-character single message without regex freeze (ReDoS test)", () => {
      const longText = "a".repeat(100000);
      const messages: ChatMessage[] = [
        {
          id: "large-msg",
          conversationId: "conv-1",
          role: "assistant",
          content: `Leading text ${longText}\n\`\`\`ts filepath="src/huge.ts"\nconst x = 1;\n\`\`\`\nTrailing text ${longText}`,
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const start = performance.now();
      const artifacts = extractArtifactsFromMessages(messages);
      const elapsed = performance.now() - start;

      expect(artifacts).toHaveLength(1);
      expect(artifacts[0].filePath).toBe("src/huge.ts");
      expect(elapsed).toBeLessThan(1000); // under 1s
    });
  });

  describe("5. mergeArtifacts Robustness & Contract Invariants", () => {
    it("handles null, undefined, empty inputs gracefully", () => {
      expect(mergeArtifacts(null as any, null as any)).toEqual([]);
      expect(mergeArtifacts([] as any, [] as any)).toEqual([]);
      expect(mergeArtifacts(undefined as any, [] as any)).toEqual([]);
      expect(mergeArtifacts([null, undefined, 123, "string"] as any, [])).toHaveLength(2); // 123 -> deliverable-2, string -> deliverable-3
    });

    it("deduplicates artifacts when chat and agent report same filePath", () => {
      const chatArtifacts: WorkspaceArtifact[] = [
        {
          id: "chat-1",
          title: "App.svelte",
          filePath: "src/App.svelte",
          kind: "diff",
          content: "diff content",
          additions: 2,
          deletions: 1,
          status: "pending",
          createdAt: "2026-09-04T10:00:00Z",
        },
      ];

      const agentArtifacts = [
        {
          id: "agent-1",
          filePath: "src/App.svelte",
          title: "App.svelte",
          kind: "file",
          content: "new code",
          metadata: { additions: 5, deletions: 0 },
        },
      ];

      const merged = mergeArtifacts(agentArtifacts, chatArtifacts);
      expect(merged).toHaveLength(1);
      expect(merged[0].filePath).toBe("src/App.svelte");
      // Agent deliverable updates/takes precedence or overwrites byPath
      expect(merged[0].id).toBe("agent-1");
      expect(merged[0].additions).toBe(5);
    });

    it("extractArtifactsFromMessages and WorkspaceArtifact interface contract inspection", () => {
      const messages: ChatMessage[] = [
        {
          id: "diff-msg",
          conversationId: "conv-1",
          role: "assistant",
          content:
            '```diff filepath="src/contract.ts"\n--- a/src/contract.ts\n+++ b/src/contract.ts\n@@ -1 +1 @@\n-old\n+new\n```',
          createdAt: "2026-09-04T12:00:00Z",
        },
      ];

      const artifacts = extractArtifactsFromMessages(messages);
      expect(artifacts).toHaveLength(1);
      const art = artifacts[0];

      // CRITICAL CONTRACT CHECK:
      // Does WorkspaceArtifact have diffText property?
      // Notice: In artifacts.ts, art.kind === "diff", but art.content contains the diff text.
      // If RightPanel checks `if (artifact.diffText)`:
      const hasExplicitDiffText = "diffText" in art && (art as any).diffText !== undefined;
      // We document this empirical fact:
      expect(hasExplicitDiffText).toBe(false);
      expect(art.content).toContain("@@ -1 +1 @@");
    });
  });
});
