import { describe, it, expect } from "vitest";
import { extractArtifactsFromMessages, mergeArtifacts } from "./artifacts";
import type { ChatMessage } from "./types";

describe("artifacts.ts", () => {
  it("returns empty array when messages are empty or null", () => {
    expect(extractArtifactsFromMessages([])).toEqual([]);
    expect(extractArtifactsFromMessages(null as any)).toEqual([]);
  });

  it("extracts diff blocks from assistant messages", () => {
    const messages: ChatMessage[] = [
      {
        id: "msg-1",
        conversationId: "conv-1",
        role: "user",
        content: "Please update App.svelte",
        createdAt: "2026-09-04T12:00:00Z",
      },
      {
        id: "msg-2",
        conversationId: "conv-1",
        role: "assistant",
        content: 'Here is the patch:\n```diff filepath="src/App.svelte"\n--- a/src/App.svelte\n+++ b/src/App.svelte\n@@ -1,3 +1,3 @@\n-const x = 1;\n+const x = 2;\n```\nDone!',
        createdAt: "2026-09-04T12:01:00Z",
      },
    ];

    const artifacts = extractArtifactsFromMessages(messages, "conv-1");
    expect(artifacts).toHaveLength(1);
    expect(artifacts[0].filePath).toBe("src/App.svelte");
    expect(artifacts[0].kind).toBe("diff");
    expect(artifacts[0].additions).toBe(1);
    expect(artifacts[0].deletions).toBe(1);
    expect(artifacts[0].conversationId).toBe("conv-1");
    expect(artifacts[0].status).toBe("pending");
    expect(artifacts[0].diffLines).toBeDefined();
  });

  it("extracts code blocks with explicit filepath", () => {
    const messages: ChatMessage[] = [
      {
        id: "msg-assistant",
        conversationId: "conv-1",
        role: "assistant",
        content: '```typescript filepath="src/utils/math.ts"\nexport function add(a: number, b: number): number {\n  return a + b;\n}\n```',
        createdAt: "2026-09-04T12:02:00Z",
      },
    ];

    const artifacts = extractArtifactsFromMessages(messages);
    expect(artifacts).toHaveLength(1);
    expect(artifacts[0].filePath).toBe("src/utils/math.ts");
    expect(artifacts[0].kind).toBe("code");
    expect(artifacts[0].additions).toBe(3);
    expect(artifacts[0].deletions).toBe(0);
  });

  it("ignores user messages with code blocks", () => {
    const messages: ChatMessage[] = [
      {
        id: "msg-user",
        conversationId: "conv-1",
        role: "user",
        content: '```typescript filepath="src/index.ts"\nconst hello = true;\n```',
        createdAt: "2026-09-04T12:00:00Z",
      },
    ];

    const artifacts = extractArtifactsFromMessages(messages);
    expect(artifacts).toHaveLength(0);
  });

  it("normalizes Windows backslashes in paths", () => {
    const messages: ChatMessage[] = [
      {
        id: "msg-win",
        conversationId: "conv-1",
        role: "assistant",
        content: '```diff path="src\\components\\Button.svelte"\n@@ -1 +1 @@\n-old\n+new\n```',
        createdAt: "2026-09-04T12:00:00Z",
      },
    ];

    const artifacts = extractArtifactsFromMessages(messages);
    expect(artifacts[0].filePath).toBe("src/components/Button.svelte");
  });

  it("merges agent artifacts and chat artifacts deduplicating by path", () => {
    const chatArtifacts = [
      {
        id: "chat-1",
        title: "App.svelte",
        filePath: "src/App.svelte",
        kind: "diff" as const,
        content: "diff text",
        additions: 5,
        deletions: 2,
        status: "pending" as const,
        createdAt: "2026-09-04T12:00:00Z",
      },
    ];

    const agentArtifacts = [
      {
        id: "agent-1",
        uri: "src/App.svelte",
        title: "App.svelte",
        kind: "file",
        content: "agent content",
        metadata: { additions: 10, deletions: 0 },
      },
      {
        id: "agent-2",
        uri: "src/NewFile.ts",
        title: "NewFile.ts",
        kind: "file",
        content: "new content",
      },
    ];

    const merged = mergeArtifacts(agentArtifacts, chatArtifacts);
    expect(merged).toHaveLength(2);
    expect(merged.map((m) => m.filePath).sort()).toEqual(["src/App.svelte", "src/NewFile.ts"]);
  });
});
