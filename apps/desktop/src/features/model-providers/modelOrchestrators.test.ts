import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  createModelOrchestrators,
  type ModelOpsContext,
  type ModelOpsState,
} from "./modelOrchestrators";
import type { AppSettings, ModelOption } from "../../lib/types";

function makeSettings(): AppSettings {
  return { model: { providers: [] } } as unknown as AppSettings;
}

function makeOption(id: string): ModelOption {
  return { id, providerId: "openai", modelId: "gpt" } as unknown as ModelOption;
}

function makeState(overrides: Partial<ModelOpsState> = {}): ModelOpsState {
  return {
    settings: makeSettings(),
    settingsDraft: makeSettings(),
    modelOptions: [makeOption("openai/gpt")],
    changingModel: false,
    activeModelKey: "other/model",
    modelMenuOpen: true,
    modelSearchQuery: "q",
    modelProviders: [],
    modelProviderBusy: {},
    modelProviderKeyDrafts: {},
    modelProviderStatus: {},
    errorMessage: "stale",
    ...overrides,
  };
}

function makeContext(
  state: ModelOpsState,
  api: Partial<ModelOpsContext["api"]> = {},
  helpers: Partial<ModelOpsContext["helpers"]> = {}
): { ctx: ModelOpsContext; state: ModelOpsState } {
  const ctx: ModelOpsContext = {
    getState: () => state,
    setState: (patch) => {
      Object.assign(state, patch);
    },
    api: {
      selectModelRef: vi.fn(async () => makeSettings()),
      listModels: vi.fn(async () => [makeOption("openai/gpt")]),
      testModelProvider: vi.fn(async () => ({ ok: true }) as never),
      refreshModelCatalog: vi.fn(async () => makeSettings()),
      setModelProviderApiKey: vi.fn(async () => makeSettings()),
      clearModelProviderApiKey: vi.fn(async () => makeSettings()),
      deleteModelProvider: vi.fn(async () => makeSettings()),
      ...api,
    },
    helpers: {
      ensureCloudWriteAllowed: () => true,
      cloneSettings: (s) => JSON.parse(JSON.stringify(s)),
      normalizeError: (e: unknown) => String((e as Error)?.message ?? e),
      refreshRuntimeState: vi.fn(async () => undefined),
      ensureCurrentModelOption: (options) => options,
      ...helpers,
    },
  };
  return { ctx, state };
}

describe("model orchestrators", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("changeModel is a no-op when already on that model", async () => {
    const { ctx, state } = makeContext(makeState({ activeModelKey: "openai/gpt" }));
    const ops = createModelOrchestrators(ctx);
    await ops.changeModel("openai/gpt");
    expect(ctx.api.selectModelRef).not.toHaveBeenCalled();
    expect(state.changingModel).toBe(false);
  });

  it("changeModel swaps settings and refreshes runtime", async () => {
    const next = makeSettings();
    const { ctx, state } = makeContext(makeState(), {
      selectModelRef: vi.fn(async () => next),
    });
    const ops = createModelOrchestrators(ctx);
    await ops.changeModel("openai/gpt");
    expect(state.settings).toBe(next);
    expect(state.changingModel).toBe(false);
    expect(state.errorMessage).toBe("");
    expect(ctx.helpers.refreshRuntimeState).toHaveBeenCalled();
  });

  it("changeModel surfaces API errors without getting stuck busy", async () => {
    const { ctx, state } = makeContext(
      makeState(),
      {
        selectModelRef: vi.fn(async () => {
          throw new Error("boom");
        }),
      }
    );
    const ops = createModelOrchestrators(ctx);
    await ops.changeModel("openai/gpt");
    expect(state.errorMessage).toBe("boom");
    expect(state.changingModel).toBe(false);
  });

  it("selectModel closes the menu before delegating", async () => {
    const { ctx, state } = makeContext(makeState());
    const ops = createModelOrchestrators(ctx);
    await ops.selectModel("openai/gpt");
    expect(state.modelMenuOpen).toBe(false);
    expect(state.modelSearchQuery).toBe("");
    expect(ctx.api.selectModelRef).toHaveBeenCalled();
  });

  it("testProvider saves a draft key first, then records status", async () => {
    const state = makeState({ modelProviderKeyDrafts: { p1: "sk-x" } });
    const { ctx } = makeContext(state, {
      testModelProvider: vi.fn(async () => ({ ready: true }) as never),
    });
    const ops = createModelOrchestrators(ctx);
    await ops.testProvider("p1");
    expect(ctx.api.setModelProviderApiKey).toHaveBeenCalledWith("p1", "sk-x");
    expect(ctx.api.testModelProvider).toHaveBeenCalledWith("p1");
    expect(state.modelProviderStatus["p1"]).toEqual({ ready: true });
    expect(state.modelProviderBusy["p1"]).toBe(false);
  });

  it("saveProviderKey refuses empty keys and records a failed status on error", async () => {
    const { ctx, state } = makeContext(makeState());
    const ops = createModelOrchestrators(ctx);
    await ops.saveProviderKey("p1");
    expect(ctx.api.setModelProviderApiKey).not.toHaveBeenCalled();

    const withKey = makeContext(makeState({ modelProviderKeyDrafts: { p1: "sk-x" } }), {
      setModelProviderApiKey: vi.fn(async () => {
        throw new Error("denied");
      }),
    });
    await createModelOrchestrators(withKey.ctx).saveProviderKey("p1");
    expect(withKey.state.errorMessage).toBe("denied");
    expect(withKey.state.modelProviderStatus["p1"].modelReady).toBe(false);
    expect(withKey.state.modelProviderKeyDrafts["p1"]).toBe("sk-x");
  });

  it("removeProvider refreshes state from the delete result", async () => {
    const next = makeSettings();
    const { ctx, state } = makeContext(makeState(), {
      deleteModelProvider: vi.fn(async () => next),
    });
    const ops = createModelOrchestrators(ctx);
    await ops.removeProvider("p1");
    expect(ctx.api.deleteModelProvider).toHaveBeenCalledWith("p1");
    expect(state.settings).toBe(next);
  });

  it("refreshModelsState is a no-op without settings", async () => {
    const { ctx } = makeContext(makeState({ settings: null }));
    const ops = createModelOrchestrators(ctx);
    await ops.refreshModelsState(null);
    expect(ctx.api.listModels).not.toHaveBeenCalled();
  });
});
