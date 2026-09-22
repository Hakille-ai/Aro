/**
 * Model-provider domain logic, extracted from `App.svelte`.
 *
 * Pure functions + catalog constants only: no Svelte reactivity, no API
 * calls, no component state. The sheet UI lives in
 * `features/models/AddProviderModal.svelte`; `App.svelte` keeps the reactive
 * glue (draft `let`s, template bindings, API orchestration).
 */

import type {
  ModelProviderConnection,
  ModelProviderKind,
} from "../../lib/types";

export interface ProviderOption {
  value: string;
  label: string;
}

export interface ProviderDefault {
  name: string;
  endpoint: string;
}

export const providerOptions: ProviderOption[] = [
  { value: "mock", label: "Mock" },
  { value: "ollama", label: "Ollama" },
  { value: "llama-cpp", label: "llama.cpp" },
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
  { value: "google", label: "Google Gemini" },
  { value: "mistral", label: "Mistral" },
  { value: "openai-compatible", label: "OpenAI-compatible" },
];

export const addProviderDefaults: Record<ModelProviderKind, ProviderDefault> = {
  openai: { name: "OpenAI", endpoint: "https://api.openai.com/v1" },
  anthropic: { name: "Anthropic", endpoint: "https://api.anthropic.com/v1" },
  google: { name: "Google Gemini", endpoint: "https://generativelanguage.googleapis.com/v1beta" },
  mistral: { name: "Mistral", endpoint: "https://api.mistral.ai/v1" },
  "openai-compatible": { name: "Custom Provider", endpoint: "https://api.example.com/v1" },
  mock: { name: "Mock", endpoint: "" },
  ollama: { name: "Ollama", endpoint: "http://127.0.0.1:11434" },
  "llama-cpp": { name: "llama.cpp", endpoint: "http://127.0.0.1:8080" },
};

/** First model seeded for a freshly created provider, by kind. */
export function defaultModelForKind(kind: ModelProviderKind): string {
  switch (kind) {
    case "anthropic":
      return "claude-sonnet-5";
    case "google":
      return "gemini-3.5-flash";
    case "mistral":
      return "mistral-small-4";
    case "openai":
      return "gpt-5.4-mini";
    case "ollama":
      return "gemma3:1b";
    case "llama-cpp":
      return "local-model";
    default:
      return "custom-model";
  }
}

export function isLocalProviderKind(kind: ModelProviderKind): boolean {
  return kind === "mock" || kind === "ollama" || kind === "llama-cpp";
}

/** Slug used for the provider id (`openai-compatible` uses the given name). */
export function slugifyProviderId(base: string): string {
  return (
    String(base).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "provider"
  );
}

/** Fresh draft values when the sheet opens for a kind. */
export function initialDraftForKind(kind: ModelProviderKind): {
  name: string;
  endpoint: string;
  apiKey: string;
} {
  const defaults = addProviderDefaults[kind];
  return { name: defaults.name, endpoint: defaults.endpoint, apiKey: "" };
}

/**
 * Name/endpoint auto-sync while the sheet is open (mirrors the legacy `$:`
 * rule): an empty or option-label-matching name snaps back to the default,
 * an empty endpoint snaps back to the default endpoint.
 */
export function syncDraftNames(
  kind: ModelProviderKind,
  name: string,
  endpoint: string
): { name: string; endpoint: string } {
  const defaults = addProviderDefaults[kind];
  let nextName = name;
  if (
    !nextName.trim() ||
    providerOptions.some((option) => option.value === kind && nextName === option.label)
  ) {
    nextName = defaults.name;
  }
  const nextEndpoint = !endpoint.trim() ? defaults.endpoint : endpoint;
  return { name: nextName, endpoint: nextEndpoint };
}

export interface ProviderDraftInput {
  kind: ModelProviderKind;
  name: string;
  endpoint: string;
  apiKey: string;
  now?: string;
}

/** Build the `ModelProviderConnection` payload submitted from the sheet. */
export function buildProviderDraft(input: ProviderDraftInput): ModelProviderConnection {
  const now = input.now ?? new Date().toISOString();
  const idBase = input.kind === "openai-compatible" ? input.name : input.kind;
  const slug = slugifyProviderId(idBase);
  // NOTE: `Date.now()` keeps ids unique across rapid submissions; tests pass
  // an explicit suffix via `now` only for timestamps, never for the id.
  const id = `${slug}-${Date.now().toString(36)}`;
  const isLocal = isLocalProviderKind(input.kind);
  const modelId = defaultModelForKind(input.kind);
  const displayName = input.name.trim() || addProviderDefaults[input.kind].name;
  const hasKey = Boolean(input.apiKey.trim());
  return {
    id,
    kind: input.kind,
    displayName,
    enabled: true,
    endpoint: input.endpoint.trim() || null,
    authConfigured: hasKey || isLocal,
    models: [
      {
        providerId: id,
        providerKind: input.kind,
        modelId,
        label: modelId,
        family: displayName,
        local: isLocal,
        installed: isLocal || hasKey,
        ready: isLocal || hasKey,
      },
    ],
    createdAt: now,
    updatedAt: now,
  };
}
