import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Composer from "./Composer.svelte";
import type { ModelOption } from "../../lib/types";

const labels = new Proxy<Record<string, string>>({}, { get: (_target, key) => String(key) });
const noop = vi.fn();

const mockModels: ModelOption[] = [
  {
    id: "ollama:llama3",
    modelId: "llama3",
    label: "Llama 3 8B",
    provider: "ollama",
    providerId: "ollama-local",
    providerKind: "ollama",
    local: true,
    installed: true,
    ready: true,
    family: "Llama",
  },
  {
    id: "ollama:mistral",
    modelId: "mistral",
    label: "Mistral 7B Local",
    provider: "ollama",
    providerId: "ollama-local",
    providerKind: "ollama",
    local: true,
    installed: true,
    ready: true,
    family: "Mistral",
  },
  {
    id: "openai:gpt-4o",
    modelId: "gpt-4o",
    label: "GPT-4o Omniscient",
    provider: "openai",
    providerId: "openai-cloud",
    providerKind: "openai",
    local: false,
    installed: false,
    ready: true,
    family: "GPT",
  },
  {
    id: "anthropic:claude-3-7-sonnet",
    modelId: "claude-3-7-sonnet",
    label: "Claude 3.7 Sonnet Hybrid",
    provider: "anthropic",
    providerId: "anthropic-cloud",
    providerKind: "anthropic",
    local: false,
    installed: false,
    ready: true,
    family: "Claude",
  },
  {
    id: "google:gemini-2-flash",
    modelId: "gemini-2-flash",
    label: "Gemini 2.0 Flash",
    provider: "google",
    providerId: "google-cloud",
    providerKind: "google",
    local: false,
    installed: false,
    ready: false,
    family: "Gemini",
  },
];

function composerProps(overrides: Record<string, unknown> = {}) {
  return {
    labels,
    language: "fr",
    errorMessage: "",
    input: "",
    fileInput: undefined,
    composerInput: undefined,
    attachedFiles: [],
    cloudWriteLocked: false,
    cloudAuthenticated: true,
    messagesCount: 1,
    webAccess: "auto",
    voiceModeOptions: [],
    voiceInputMode: "push-to-talk",
    voiceHandsFreeArmed: false,
    recording: false,
    assistantSpeaking: false,
    voiceVolume: 0,
    wakeWordEnabled: false,
    recordingHint: "",
    voiceStateLabel: "Prêt",
    voiceStateHint: "",
    modelRuntimeReady: true,
    voiceSpeechToTextReady: true,
    voiceWakeModelReady: false,
    modelReadinessLabel: "Prêt",
    speechReadinessLabel: "Prêt",
    wakeWordReadinessLabel: "Désactivé",
    settingsAvailable: true,
    changingModel: false,
    runtimeDetail: "Prêt",
    currentModelLabel: "Claude 3.7 Sonnet Hybrid",
    modelMenuOpen: false,
    modelSearchQuery: "",
    modelOptions: mockModels,
    activeModelKey: "anthropic:claude-3-7-sonnet",
    arenaMode: false,
    arenaSending: false,
    arenaModelA: "",
    arenaModelB: "",
    sending: false,
    cloudWriteDisabledTitle: () => undefined,
    webAccessTitle: () => "Web auto",
    webAccessAriaLabel: () => "Web auto",
    formatFileSize: () => "1 Ko",
    getWavePath: () => "M0 0",
    onSubmitMessage: noop,
    onFilesChange: noop,
    onPreviewAttachment: noop,
    onUploadAttachment: noop,
    onRemoveAttachment: noop,
    onResizeComposer: noop,
    onOpenFilePicker: noop,
    onToggleWebAccess: noop,
    onSetVoiceInputMode: noop,
    onToggleRecording: noop,
    onStopSpeaking: noop,
    onSelectModel: noop,
    onOpenModelSettings: noop,
    activePermissionMode: "standard",
    activePermissionLabel: "Standard",
    permissionProfiles: [],
    activePermissionProfileId: "",
    onSelectPermissionPreset: noop,
    onSelectPermissionProfile: noop,
    onOpenPermissionSettings: noop,
    workspaceMentionEntries: [],
    ...overrides,
  } as any;
}

describe("Composer Model Selector Overhaul", () => {
  it("renders model button and opens menu popover on click", async () => {
    const { container } = render(Composer, composerProps());
    const modelButton = screen.getByRole("button", { name: "chooseModelTitle" });
    expect(modelButton).toBeInTheDocument();
    expect(screen.getByText("Claude 3.7 Sonnet Hybrid")).toBeInTheDocument();

    await fireEvent.click(modelButton);
    const modelMenu = container.querySelector(".model-menu");
    expect(modelMenu).toBeInTheDocument();
  });

  it("renders dynamic provider filter pills and counts", async () => {
    render(Composer, composerProps({ modelMenuOpen: true }));

    const tabsBar = screen.getByRole("tablist", { name: "Fournisseurs de modèles" });
    expect(tabsBar).toBeInTheDocument();

    // "Tous" (5 models), "Local / Ollama" (2 models), "OpenAI" (1), "Anthropic" (1), "Gemini" (1)
    expect(within(tabsBar).getByText("Tous")).toBeInTheDocument();
    expect(within(tabsBar).getByText("Local / Ollama")).toBeInTheDocument();
    expect(within(tabsBar).getByText("OpenAI")).toBeInTheDocument();
    expect(within(tabsBar).getByText("Anthropic")).toBeInTheDocument();
    expect(within(tabsBar).getByText("Gemini")).toBeInTheDocument();
  });

  it("filters models when provider pill is selected", async () => {
    const { container } = render(Composer, composerProps({ modelMenuOpen: true }));
    const menu = container.querySelector(".model-menu") as HTMLElement;

    // By default "Tous" is selected, all 5 models are visible
    expect(within(menu).getByText("Llama 3 8B")).toBeInTheDocument();
    expect(within(menu).getByText("GPT-4o Omniscient")).toBeInTheDocument();

    // Click OpenAI tab
    const openAiTab = within(menu).getByRole("tab", { name: /OpenAI/i });
    await fireEvent.click(openAiTab);

    // Now only OpenAI model is displayed in menu
    expect(within(menu).getByText("GPT-4o Omniscient")).toBeInTheDocument();
    expect(within(menu).queryByText("Llama 3 8B")).not.toBeInTheDocument();
    expect(within(menu).queryByText("Claude 3.7 Sonnet Hybrid")).not.toBeInTheDocument();
  });

  it("filters models via search input", async () => {
    const { container } = render(Composer, composerProps({ modelMenuOpen: true }));
    const menu = container.querySelector(".model-menu") as HTMLElement;

    const searchInput = within(menu).getByPlaceholderText("Rechercher un modèle ou fournisseur...");
    await fireEvent.input(searchInput, { target: { value: "claude" } });

    expect(within(menu).getByText("Claude 3.7 Sonnet Hybrid")).toBeInTheDocument();
    expect(within(menu).queryByText("Llama 3 8B")).not.toBeInTheDocument();
    expect(within(menu).queryByText("GPT-4o Omniscient")).not.toBeInTheDocument();
  });

  it("selects model on click and calls onSelectModel", async () => {
    const onSelectModel = vi.fn();
    render(Composer, composerProps({ modelMenuOpen: true, onSelectModel }));

    const llamaOption = screen.getByRole("option", { name: /Llama 3 8B/i });
    await fireEvent.click(llamaOption);

    expect(onSelectModel).toHaveBeenCalledWith("ollama:llama3");
  });

  it("opens settings when footer manage button is clicked", async () => {
    const onOpenModelSettings = vi.fn();
    render(Composer, composerProps({ modelMenuOpen: true, onOpenModelSettings }));

    const manageBtn = screen.getByRole("button", { name: /Gérer les fournisseurs et modèles\.\.\./i });
    await fireEvent.click(manageBtn);

    expect(onOpenModelSettings).toHaveBeenCalledOnce();
  });

  it("selects model on Enter in search box without submitting composer form", async () => {
    const onSelectModel = vi.fn();
    const onSubmitMessage = vi.fn();
    const { container } = render(
      Composer,
      composerProps({ modelMenuOpen: true, onSelectModel, onSubmitMessage })
    );
    const menu = container.querySelector(".model-menu") as HTMLElement;
    const searchInput = within(menu).getByPlaceholderText("Rechercher un modèle ou fournisseur...");

    await fireEvent.input(searchInput, { target: { value: "llama" } });
    await fireEvent.keyDown(searchInput, { key: "Enter" });

    expect(onSubmitMessage).not.toHaveBeenCalled();
    expect(onSelectModel).toHaveBeenCalledWith("ollama:llama3");
  });

  it("opens settings when clicking a model that needs setup", async () => {
    const onOpenModelSettings = vi.fn();
    render(Composer, composerProps({ modelMenuOpen: true, onOpenModelSettings }));

    // Gemini 2.0 Flash is mockModels[4] with ready: false
    const geminiOption = screen.getByRole("option", { name: /Gemini 2\.0 Flash/i });
    await fireEvent.click(geminiOption);

    expect(onOpenModelSettings).toHaveBeenCalledOnce();
  });

  describe("server-side executability (browser)", () => {
    it("reroutes a server-blocked cloud model to settings with a browser hint", async () => {
      const onSelectModel = vi.fn();
      const onOpenModelSettings = vi.fn();
      render(
        Composer,
        composerProps({
          modelMenuOpen: true,
          onSelectModel,
          onOpenModelSettings,
          serverRunnableIds: ["llama3", "mistral"],
        })
      );

      // Le menu se referme au clic : on verifie les bandeaux avant.
      // GPT-4o, Claude (bloques serveur) + Gemini (pas pret) : 3 hints.
      expect(
        screen.getAllByText("Non exécutable dans le navigateur")
      ).toHaveLength(3);
      const gptOption = screen.getByRole("option", { name: /GPT-4o Omniscient/i });
      await fireEvent.click(gptOption);

      expect(onSelectModel).not.toHaveBeenCalled();
      expect(onOpenModelSettings).toHaveBeenCalledOnce();
    });

    it("keeps local models selectable when the server list is known", async () => {
      const onSelectModel = vi.fn();
      render(
        Composer,
        composerProps({
          modelMenuOpen: true,
          onSelectModel,
          serverRunnableIds: ["llama3", "mistral"],
        })
      );

      const llamaOption = screen.getByRole("option", { name: /Llama 3 8B/i });
      await fireEvent.click(llamaOption);

      expect(onSelectModel).toHaveBeenCalledWith("ollama:llama3");
    });

    it("selects a cloud model covered by the server opt-in", async () => {
      const onSelectModel = vi.fn();
      render(
        Composer,
        composerProps({
          modelMenuOpen: true,
          onSelectModel,
          serverRunnableIds: ["llama3", "gpt-4o"],
        })
      );

      const gptOption = screen.getByRole("option", { name: /GPT-4o Omniscient/i });
      await fireEvent.click(gptOption);

      expect(onSelectModel).toHaveBeenCalledWith("openai:gpt-4o");
    });
  });
});
