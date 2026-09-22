import { describe, expect, it } from "vitest";
import {
  applyMentionSelection,
  buildUnifiedMentionItems,
  dedupeMentionedPaths,
  detectMentionQuery,
  extractMentionedPaths,
  filterUnifiedMentionItems,
  filterWorkspaceEntries,
  flattenWorkspaceTreeToMentions,
  resolveMentionedEntries,
  stripMentionTokens,
  type WorkspaceMentionEntry,
} from "./mention-model";

const entries: WorkspaceMentionEntry[] = [
  { name: "App.svelte", path: "/w/src/App.svelte", isDir: false, relativePath: "src/App.svelte", extension: "svelte" },
  { name: "app-store.ts", path: "/w/src/app-store.ts", isDir: false, relativePath: "src/app-store.ts", extension: "ts" },
  { name: "src", path: "/w/src", isDir: true, relativePath: "src", extension: "" },
  { name: "main.rs", path: "/w/src-tauri/main.rs", isDir: false, relativePath: "src-tauri/main.rs", extension: "rs" },
  { name: "Dockerfile", path: "/w/Dockerfile", isDir: false, relativePath: "Dockerfile", extension: "" },
];

describe("mention-model F4.1 — trigger detection", () => {
  it("detects @query at line start", () => {
    const t = detectMentionQuery("@App", 4);
    expect(t).toMatchObject({ active: true, query: "App", startIndex: 0, endIndex: 4 });
  });
  it("detects @query after whitespace and newline", () => {
    expect(detectMentionQuery("fix @main", 9)?.query).toBe("main");
    expect(detectMentionQuery("line1\n@src", 10)?.query).toBe("src");
  });
  it("rejects email addresses", () => {
    expect(detectMentionQuery("contact user@example.com", 24)).toBeNull();
  });
  it("returns null when cursor is before the token", () => {
    expect(detectMentionQuery("hello @App", 3)).toBeNull();
  });
  it("handles @ at end of very long text", () => {
    const text = `${"a".repeat(10_000)} @`;
    const t = detectMentionQuery(text, text.length);
    expect(t?.query).toBe("");
  });
});

describe("mention-model F4.1 — filtering & ranking", () => {
  it("returns first entries on empty query", () => {
    expect(filterWorkspaceEntries(entries, "", 2)).toHaveLength(2);
  });
  it("ranks exact match before prefix/substring matches", () => {
    const ranked = filterWorkspaceEntries(entries, "src", 10);
    expect(ranked[0].name).toBe("src");
  });
  it("matches dotted and dashed paths", () => {
    expect(filterWorkspaceEntries(entries, "app-store", 5)[0].name).toBe("app-store.ts");
  });
  it("normalizes windows backslash queries", () => {
    expect(filterWorkspaceEntries(entries, "src\\App", 5)[0].relativePath).toBe("src/App.svelte");
  });
  it("returns empty on 500-char query with no match", () => {
    expect(filterWorkspaceEntries(entries, "z".repeat(500), 5)).toHaveLength(0);
  });
});

describe("mention-model F4.1 — insertion & extraction", () => {
  it("applies mention replacement with trailing space", () => {
    const trigger = detectMentionQuery("review @Ap", 10)!;
    const { newText, newCursor } = applyMentionSelection("review @Ap", trigger, "src/App.svelte");
    expect(newText).toBe("review @src/App.svelte ");
    expect(newCursor).toBe(newText.length);
  });
  it("extracts all @paths and dedupes", () => {
    const paths = extractMentionedPaths("fix @src/App.svelte and @src/App.svelte plus @Dockerfile");
    expect(paths).toEqual(["src/App.svelte", "src/App.svelte", "Dockerfile"]);
    expect(dedupeMentionedPaths(paths)).toEqual(["src/App.svelte", "Dockerfile"]);
  });
});

describe("mention-model F4.3 — tree flattening & context", () => {
  it("flattens nested children and deep levels", () => {
    const flat = flattenWorkspaceTreeToMentions([
      { name: "a", path: "/w/a", relativePath: "a", isDir: true, children: [{ name: "b.ts", path: "/w/a/b.ts", relativePath: "a/b.ts" }] },
    ]);
    expect(flat.map((e) => e.relativePath).sort()).toEqual(["a", "a/b.ts"]);
  });
  it("keeps extensionless files with empty extension", () => {
    const flat = flattenWorkspaceTreeToMentions([{ name: "Dockerfile", path: "/w/Dockerfile", relativePath: "Dockerfile" }]);
    expect(flat[0].extension).toBe("");
  });
  it("resolves mentioned entries and skips ghost files", () => {
    const resolved = resolveMentionedEntries("look at @src/App.svelte and @ghost/missing.ts", entries);
    expect(resolved.map((e) => e.relativePath)).toEqual(["src/App.svelte"]);
  });
  it("dedupes 20 distinct mentions in one prompt", () => {
    const text = Array.from({ length: 20 }, (_, i) => `@file${i}.ts`).join(" ");
    expect(extractMentionedPaths(text)).toHaveLength(20);
  });
  it("strips mention tokens for clean prompts", () => {
    expect(stripMentionTokens("review @src/App.svelte now")).toBe("review now");
  });
  it("safely handles circular tree references without crashing", () => {
    const cyclicNode: any = { name: "cycle", path: "/w/cycle", relativePath: "cycle", isDir: true };
    cyclicNode.children = [cyclicNode];
    const flat = flattenWorkspaceTreeToMentions([cyclicNode]);
    expect(flat).toHaveLength(1);
    expect(flat[0].name).toBe("cycle");
  });
  it("normalizes leading ./ in mention paths", () => {
    const resolved = resolveMentionedEntries("examine @./src/App.svelte", entries);
    expect(resolved.map((e) => e.relativePath)).toEqual(["src/App.svelte"]);
  });
  it("resolves unique basename mentions when typed without directory prefix", () => {
    const resolved = resolveMentionedEntries("see @App.svelte and @main.rs", entries);
    expect(resolved.map((e) => e.relativePath)).toEqual(["src/App.svelte", "src-tauri/main.rs"]);
  });
});

describe("mention-model — Multi-Entity Unified Mentions (Files, Skills, Plugins, MCP, Agents, Models)", () => {
  it("builds unified mention items across all 6 categories", () => {
    const unified = buildUnifiedMentionItems({
      files: entries,
      skills: [{ id: "s1", name: "WebSearch", description: "Search web", category: "Recherche" }],
      plugins: [{ id: "p1", name: "GitHub Integration", description: "Git sync", version: "1.2.0" }],
      mcpServers: [{ id: "m1", name: "Local Postgres", url: "http://localhost:5432", status: "connected" }],
      agents: [{ id: "a1", name: "Senior Architect", role: "Architecture", description: "Design systems" }],
      models: [{ id: "deepseek-r1", label: "DeepSeek R1", provider: "Ollama", details: "Reasoning model" }],
    });

    expect(unified.length).toBe(entries.length + 5);
    expect(unified.find((i) => i.category === "skill")?.insertToken).toBe("skill:websearch");
    expect(unified.find((i) => i.category === "plugin")?.insertToken).toBe("plugin:github-integration");
    expect(unified.find((i) => i.category === "mcp")?.insertToken).toBe("mcp:local-postgres");
    expect(unified.find((i) => i.category === "agent")?.insertToken).toBe("agent:senior-architect");
    expect(unified.find((i) => i.category === "model")?.insertToken).toBe("model:deepseek-r1");
  });

  it("filters unified items by query and category filter", () => {
    const unified = buildUnifiedMentionItems({
      files: entries,
      skills: [{ id: "s1", name: "WebSearch", description: "Search web" }],
      agents: [{ id: "a1", name: "Code Reviewer", role: "Reviewer" }],
    });

    const skillOnly = filterUnifiedMentionItems(unified, "", "skill");
    expect(skillOnly).toHaveLength(1);
    expect(skillOnly[0].title).toBe("WebSearch");

    const searchHits = filterUnifiedMentionItems(unified, "Reviewer", "all");
    expect(searchHits).toHaveLength(1);
    expect(searchHits[0].title).toBe("Code Reviewer");
  });

  it("detects mention triggers with colons (e.g. @skill:web or @agent:coder)", () => {
    const t1 = detectMentionQuery("ask @skill:web", 14);
    expect(t1?.query).toBe("skill:web");

    const t2 = detectMentionQuery("ping @agent:architect", 22);
    expect(t2?.query).toBe("agent:architect");
  });
});
