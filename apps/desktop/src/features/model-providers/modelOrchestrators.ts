/**
 * Model-provider orchestrators, extracted from `App.svelte`.
 *
 * The domain logic (select/test/refresh/save/remove providers and models)
 * with zero Svelte reactivity: all component state flows through an
 * explicit `ModelOpsContext` (`getState` snapshot + `setState` patch), so
 * every orchestrator is unit-testable with fake state and a stubbed API.
 * `App.svelte` keeps thin wrappers under the historical names.
 */

import type {
  AppSettings,
  ModelOption,
  ModelProviderConnection,
  ModelProviderKind,
  RuntimeStatus,
} from "../../lib/types";

export interface ModelOpsState {
  settings: AppSettings | null;
  settingsDraft: AppSettings | null;
  modelOptions: ModelOption[];
  changingModel: boolean;
  activeModelKey: string;
  modelMenuOpen: boolean;
  modelSearchQuery: string;
  modelProviders: ModelProviderConnection[];
  modelProviderBusy: Record<string, boolean>;
  modelProviderKeyDrafts: Record<string, string>;
  modelProviderStatus: Record<string, RuntimeStatus>;
  errorMessage: string;
}

export interface ModelOpsApi {
  selectModelRef(providerId: string, modelId: string): Promise<AppSettings>;
  listModels(): Promise<ModelOption[]>;
  testModelProvider(providerId: string): Promise<RuntimeStatus>;
  refreshModelCatalog(providerId: string): Promise<AppSettings>;
  setModelProviderApiKey(providerId: string, apiKey: string): Promise<AppSettings>;
  clearModelProviderApiKey(providerId: string): Promise<AppSettings>;
  deleteModelProvider(providerId: string): Promise<AppSettings>;
}

export interface ModelOpsHelpers {
  ensureCloudWriteAllowed(action: string): boolean;
  cloneSettings(settings: AppSettings): AppSettings;
  normalizeError(error: unknown): string;
  refreshRuntimeState(): Promise<void>;
  ensureCurrentModelOption(options: ModelOption[], settings: AppSettings): ModelOption[];
}

export interface ModelOpsContext {
  getState(): ModelOpsState;
  setState(patch: Partial<ModelOpsState>): void;
  api: ModelOpsApi;
  helpers: ModelOpsHelpers;
}

export interface ModelOrchestrators {
  changeModel(modelKey: string): Promise<void>;
  selectModel(modelKey: string): Promise<void>;
  refreshModelsState(nextSettings?: AppSettings | null): Promise<void>;
  testProvider(providerId: string): Promise<void>;
  refreshProviderCatalog(providerId: string): Promise<void>;
  saveProviderKey(providerId: string): Promise<void>;
  clearProviderKey(providerId: string): Promise<void>;
  removeProvider(providerId: string): Promise<void>;
}

function setBusy(
  ctx: ModelOpsContext,
  providerId: string,
  busy: boolean
): void {
  const { modelProviderBusy } = ctx.getState();
  ctx.setState({ modelProviderBusy: { ...modelProviderBusy, [providerId]: busy } });
}

export function createModelOrchestrators(ctx: ModelOpsContext): ModelOrchestrators {
  const { api, helpers } = ctx;

  async function refreshModelsState(nextSettings?: AppSettings | null): Promise<void> {
    const fallback = nextSettings === undefined ? ctx.getState().settings : nextSettings;
    if (!fallback) return;
    ctx.setState({
      settings: fallback,
      settingsDraft: helpers.cloneSettings(fallback),
      modelProviders: fallback.model.providers,
    });
    const modelOptions = helpers.ensureCurrentModelOption(
      await api.listModels(),
      fallback
    );
    ctx.setState({ modelOptions });
    await helpers.refreshRuntimeState();
  }

  async function changeModel(modelKey: string): Promise<void> {
    const state = ctx.getState();
    const model = state.modelOptions.find((item) => item.id === modelKey);
    if (
      !state.settings ||
      !state.settingsDraft ||
      !model ||
      state.changingModel ||
      modelKey === state.activeModelKey
    ) {
      return;
    }
    if (!helpers.ensureCloudWriteAllowed("changer de modèle")) return;
    ctx.setState({ changingModel: true, errorMessage: "" });
    try {
      const settings = await api.selectModelRef(model.providerId, model.modelId);
      ctx.setState({
        settings,
        settingsDraft: helpers.cloneSettings(settings),
      });
      await helpers.refreshRuntimeState();
      const modelOptions = helpers.ensureCurrentModelOption(await api.listModels(), settings);
      ctx.setState({ modelOptions, modelProviders: settings.model.providers });
    } catch (error) {
      ctx.setState({ errorMessage: helpers.normalizeError(error) });
    } finally {
      ctx.setState({ changingModel: false });
    }
  }

  async function selectModel(modelKey: string): Promise<void> {
    ctx.setState({ modelMenuOpen: false, modelSearchQuery: "" });
    await changeModel(modelKey);
  }

  async function testProvider(providerId: string): Promise<void> {
    setBusy(ctx, providerId, true);
    ctx.setState({ errorMessage: "" });
    try {
      const draftKey = ctx.getState().modelProviderKeyDrafts[providerId]?.trim() ?? "";
      if (draftKey) {
        await saveProviderKey(providerId);
      }
      const status = await api.testModelProvider(providerId);
      const { modelProviderStatus } = ctx.getState();
      ctx.setState({ modelProviderStatus: { ...modelProviderStatus, [providerId]: status } });
    } catch (error) {
      ctx.setState({ errorMessage: helpers.normalizeError(error) });
    } finally {
      setBusy(ctx, providerId, false);
    }
  }

  async function refreshProviderCatalog(providerId: string): Promise<void> {
    if (!helpers.ensureCloudWriteAllowed("rafraîchir les modèles")) return;
    setBusy(ctx, providerId, true);
    ctx.setState({ errorMessage: "" });
    try {
      const draftKey = ctx.getState().modelProviderKeyDrafts[providerId]?.trim() ?? "";
      if (draftKey) {
        await saveProviderKey(providerId);
      }
      await refreshModelsState(await api.refreshModelCatalog(providerId));
    } catch (error) {
      ctx.setState({ errorMessage: helpers.normalizeError(error) });
    } finally {
      setBusy(ctx, providerId, false);
    }
  }

  async function saveProviderKey(providerId: string): Promise<void> {
    const apiKey = ctx.getState().modelProviderKeyDrafts[providerId]?.trim() ?? "";
    if (!apiKey || !helpers.ensureCloudWriteAllowed("ajouter une cle provider")) return;
    setBusy(ctx, providerId, true);
    ctx.setState({ errorMessage: "" });
    try {
      await refreshModelsState(await api.setModelProviderApiKey(providerId, apiKey));
      const { modelProviderKeyDrafts } = ctx.getState();
      ctx.setState({ modelProviderKeyDrafts: { ...modelProviderKeyDrafts, [providerId]: "" } });
    } catch (error) {
      const errorMessage = helpers.normalizeError(error);
      const { modelProviderStatus } = ctx.getState();
      ctx.setState({
        errorMessage,
        modelProviderStatus: {
          ...modelProviderStatus,
          [providerId]: {
            modelProvider: "mock",
            modelId: "unknown",
            modelReady: false,
            voiceReady: false,
            voiceStatus: null,
            endpoint: null,
            detail: errorMessage,
            checkedAt: new Date().toISOString(),
          },
        },
      });
    } finally {
      setBusy(ctx, providerId, false);
    }
  }

  async function clearProviderKey(providerId: string): Promise<void> {
    if (!helpers.ensureCloudWriteAllowed("retirer une cle provider")) return;
    setBusy(ctx, providerId, true);
    ctx.setState({ errorMessage: "" });
    try {
      await refreshModelsState(await api.clearModelProviderApiKey(providerId));
    } catch (error) {
      ctx.setState({ errorMessage: helpers.normalizeError(error) });
    } finally {
      setBusy(ctx, providerId, false);
    }
  }

  async function removeProvider(providerId: string): Promise<void> {
    if (!helpers.ensureCloudWriteAllowed("supprimer un provider")) return;
    setBusy(ctx, providerId, true);
    ctx.setState({ errorMessage: "" });
    try {
      await refreshModelsState(await api.deleteModelProvider(providerId));
    } catch (error) {
      ctx.setState({ errorMessage: helpers.normalizeError(error) });
    } finally {
      setBusy(ctx, providerId, false);
    }
  }

  return {
    changeModel,
    selectModel,
    refreshModelsState,
    testProvider,
    refreshProviderCatalog,
    saveProviderKey,
    clearProviderKey,
    removeProvider,
  };
}
