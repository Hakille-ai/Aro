import { describe, it, expect } from "vitest";
import {
  addProviderDefaults,
  buildProviderDraft,
  defaultModelForKind,
  initialDraftForKind,
  isLocalProviderKind,
  providerOptions,
  slugifyProviderId,
  syncDraftNames,
} from "./modelProviders";

describe("modelProviders domain", () => {
  it("exposes one option per supported kind", () => {
    const values = providerOptions.map((o) => o.value).sort();
    expect(values).toEqual(
      ["anthropic", "google", "llama-cpp", "mistral", "mock", "ollama", "openai", "openai-compatible"].sort()
    );
  });

  it("covers every kind with name + endpoint defaults", () => {
    for (const option of providerOptions) {
      const defaults = addProviderDefaults[option.value as keyof typeof addProviderDefaults];
      expect(defaults?.name, option.value).toBeTruthy();
      expect(defaults?.endpoint, option.value).not.toBeUndefined();
    }
  });

  it("maps first models per kind (mock falls back to custom-model)", () => {
    expect(defaultModelForKind("openai")).toBe("gpt-5.4-mini");
    expect(defaultModelForKind("anthropic")).toBe("claude-sonnet-5");
    expect(defaultModelForKind("ollama")).toBe("gemma3:1b");
    expect(defaultModelForKind("llama-cpp")).toBe("local-model");
    expect(defaultModelForKind("mock")).toBe("custom-model");
    expect(defaultModelForKind("openai-compatible")).toBe("custom-model");
  });

  it("flags local kinds", () => {
    expect(isLocalProviderKind("ollama")).toBe(true);
    expect(isLocalProviderKind("llama-cpp")).toBe(true);
    expect(isLocalProviderKind("mock")).toBe(true);
    expect(isLocalProviderKind("openai")).toBe(false);
  });

  it("slugifies provider ids like the sheet", () => {
    expect(slugifyProviderId("openai")).toBe("openai");
    expect(slugifyProviderId("My Provider!")).toBe("my-provider");
    expect(slugifyProviderId("---")).toBe("provider");
    expect(slugifyProviderId("")).toBe("provider");
  });

  it("initializes drafts from defaults with an empty key", () => {
    expect(initialDraftForKind("openai")).toEqual({
      name: "OpenAI",
      endpoint: "https://api.openai.com/v1",
      apiKey: "",
    });
  });

  it("snaps empty or label-matching names back to defaults", () => {
    expect(syncDraftNames("openai", "", "https://x.test")).toEqual({
      name: "OpenAI",
      endpoint: "https://x.test",
    });
    // "OpenAI" is both the default and the option label: stays.
    expect(syncDraftNames("openai", "OpenAI", "")).toEqual({
      name: "OpenAI",
      endpoint: "https://api.openai.com/v1",
    });
    expect(
      syncDraftNames("mistral", "Perso", "https://perso.test")
    ).toEqual({ name: "Perso", endpoint: "https://perso.test" });
  });

  it("builds a submittable provider draft", () => {
    const draft = buildProviderDraft({
      kind: "openai",
      name: "  ",
      endpoint: "",
      apiKey: "sk-test",
      now: "2026-01-01T00:00:00.000Z",
    });
    expect(draft.kind).toBe("openai");
    expect(draft.displayName).toBe("OpenAI");
    expect(draft.endpoint).toBeNull();
    expect(draft.authConfigured).toBe(true);
    expect(draft.models).toHaveLength(1);
    expect(draft.models[0]).toMatchObject({
      providerKind: "openai",
      modelId: "gpt-5.4-mini",
      local: false,
      installed: true,
      ready: true,
    });
    expect(draft.models[0].providerId).toBe(draft.id);
    expect(draft.createdAt).toBe("2026-01-01T00:00:00.000Z");
  });

  it("marks local providers ready without a key", () => {
    const draft = buildProviderDraft({
      kind: "ollama",
      name: "Ollama",
      endpoint: "http://127.0.0.1:11434",
      apiKey: "",
      now: "2026-01-01T00:00:00.000Z",
    });
    expect(draft.authConfigured).toBe(true);
    expect(draft.models[0]).toMatchObject({ local: true, installed: true, ready: true });
  });

  it("uses the custom name as id base for openai-compatible", () => {
    const draft = buildProviderDraft({
      kind: "openai-compatible",
      name: "Mon Proxy",
      endpoint: "https://proxy.test/v1",
      apiKey: "",
      now: "2026-01-01T00:00:00.000Z",
    });
    expect(draft.id.startsWith("mon-proxy-")).toBe(true);
  });
});
