import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import AgentMicroPills from "./AgentMicroPills.svelte";
import ConversationTopbar from "./ConversationTopbar.svelte";
import ConversationView from "./ConversationView.svelte";
import type { SubAgentInfo, ChatMessage } from "../../lib/types";

describe("AgentMicroPills Component", () => {
  it("renders micro pills with name and responds to click", async () => {
    const onSelect = vi.fn();
    const testAgents: SubAgentInfo[] = [
      {
        id: "agent-1",
        name: "Agent Recherche",
        role: "research",
        icon: "search",
        status: "running",
        goal: "Chercher les informations récentes",
        stepCount: 2,
      },
      {
        id: "agent-2",
        name: "Agent Code",
        role: "code",
        icon: "terminal",
        status: "completed",
        goal: "Implémenter la fonction",
        stepCount: 5,
      },
    ];

    render(AgentMicroPills, {
      agents: testAgents,
      language: "fr",
      onSelectAgent: onSelect,
    });

    expect(screen.getByText("Agent Recherche")).toBeInTheDocument();
    expect(screen.getByText("Agent Code")).toBeInTheDocument();

    const firstBtn = screen.getByRole("button", { name: /Agent Recherche/ });
    await fireEvent.click(firstBtn);
    expect(onSelect).toHaveBeenCalledWith(testAgents[0]);
  });

  it("handles keyboard navigation with Enter and Space keys", async () => {
    const onSelect = vi.fn();
    const testAgents: SubAgentInfo[] = [
      {
        id: "agent-1",
        name: "Agent Spécialiste",
        role: "think",
        icon: "🧠",
        status: "running",
      },
    ];

    render(AgentMicroPills, {
      agents: testAgents,
      language: "fr",
      onSelectAgent: onSelect,
    });

    const pillBtn = screen.getByRole("button", { name: /Agent Spécialiste/ });
    expect(screen.getByText("🧠")).toBeInTheDocument();

    await fireEvent.keyDown(pillBtn, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith(testAgents[0]);

    await fireEvent.keyDown(pillBtn, { key: " " });
    expect(onSelect).toHaveBeenCalledTimes(2);
  });
});

describe("ConversationTopbar Sub-Agent Breadcrumb", () => {
  it("renders breadcrumb <Title> / agents / <SubAgent> and calls onExitSubAgent", async () => {
    const onExit = vi.fn();
    const activeSubAgent: SubAgentInfo = {
      id: "sa-1",
      name: "Sous-Agent Data",
      role: "worker",
      icon: "cpu",
      status: "running",
    };

    render(ConversationTopbar, {
      activeConversation: {
        id: "c-1",
        title: "Projet Alpha",
        createdAt: "2026-09-24T00:00:00Z",
        updatedAt: "2026-09-24T00:00:00Z",
        mode: "chat",
      },
      activeSubAgent,
      onExitSubAgent: onExit,
      conversationPersonalities: {},
      selectedPersonalityId: "default",
      personalities: [],
      theme: "dark",
      language: "fr",
      labels: {} as any,
      showTopbarPersonalityDropdown: false,
      showConversationMenu: false,
      attachedFileCount: 0,
      cloudWriteLocked: false,
      cloudWriteDisabledTitle: () => null,
      ensureCloudWriteAllowed: () => true,
      onSelectConversationPersonality: vi.fn(),
      onRenameConversation: vi.fn(),
      onRemoveConversation: vi.fn(),
    });

    expect(screen.getByText("Projet Alpha")).toBeInTheDocument();
    expect(screen.getByText("agents")).toBeInTheDocument();
    expect(screen.getByText("Sous-Agent Data")).toBeInTheDocument();

    const parentBtn = screen.getByRole("button", { name: /Projet Alpha/ });
    await fireEvent.click(parentBtn);
    expect(onExit).toHaveBeenCalled();
  });
});

describe("ConversationView Unified Sub-Agent Mode", () => {
  it("renders identical chat message stream with sub-agent avatar, name, and thinking", () => {
    const activeSubAgent: SubAgentInfo = {
      id: "agent-active",
      name: "Agent En Ligne",
      role: "worker",
      icon: "search",
      avatarColor: "linear-gradient(135deg, #0071e3 0%, #005bb5 100%)",
      status: "completed",
      goal: "Tâche de recherche en ligne",
    };

    const subAgentMessages: ChatMessage[] = [
      {
        id: "sa-task",
        conversationId: "conv-1",
        role: "user",
        content: "Mission assignée à Agent En Ligne",
        createdAt: "2026-09-24T00:00:00Z",
      },
      {
        id: "sa-reply",
        conversationId: "conv-1",
        role: "assistant",
        content: "<think>Recherche des documents...</think>Exécution terminée avec succès.",
        createdAt: "2026-09-24T00:01:00Z",
      },
    ];

    const labels = new Proxy<Record<string, string>>({}, { get: (_t, k) => String(k) });

    const { container } = render(ConversationView, {
      conversationContainer: undefined as any,
      messagesEnd: undefined as any,
      loading: false,
      messages: subAgentMessages,
      labels,
      language: "fr",
      theme: "dark",
      personalities: [],
      selectedPersonalityId: "default",
      cloudWriteLocked: false,
      cloudWriteDisabledTitle: () => undefined,
      recording: false,
      assistantSpeaking: false,
      voiceVolume: 0,
      wakeWordEnabled: false,
      voiceStateLabel: "Prêt",
      recordingHint: "",
      voiceStateHint: "",
      runtimeDetail: "Prêt",
      modelRuntimeReady: true,
      voiceSpeechToTextReady: true,
      voiceWakeModelReady: false,
      voiceTextToSpeechReady: true,
      speakResponses: false,
      speechToTextIssue: "",
      textToSpeechIssue: "",
      modelReadinessLabel: "Prêt",
      speechReadinessLabel: "Prêt",
      wakeWordReadinessLabel: "Off",
      textToSpeechReadinessLabel: "Prêt",
      voiceModeOptions: [],
      voiceInputMode: "push-to-talk",
      voiceHandsFreeArmed: false,
      expandedMessageSteps: {},
      expandedStepDetails: {},
      copiedMessageId: null,
      messageFeedback: {},
      editingMessageId: null,
      editingMessageText: "",
      sending: false,
      getWavePath: () => "M0 0",
      formatTime: () => "10:00",
      formatFileSize: () => "1 Ko",
      onToggleRecording: vi.fn(),
      onStopSpeaking: vi.fn(),
      onSetVoiceInputMode: vi.fn(),
      onSelectPersonality: vi.fn(),
      onToggleMessageSteps: vi.fn(),
      onToggleStepDetails: vi.fn(),
      onPreviewAttachment: vi.fn(),
      onCopyMessage: vi.fn(),
      onRememberMessage: vi.fn(),
      onFeedback: vi.fn(),
      onStartEdit: vi.fn(),
      onCancelEdit: vi.fn(),
      onSaveEdit: vi.fn(),
      activeSubAgent,
      allAgentRuns: [],
    } as any);

    // Uses the EXACT SAME .message-stream component and layout
    expect(container.querySelector(".message-stream")).toBeInTheDocument();

    // Renders the sub-agent's name as sender
    expect(screen.getByText("Agent En Ligne")).toBeInTheDocument();

    // Renders the user directive and assistant response in identical bubbles
    expect(screen.getByText("Mission assignée à Agent En Ligne")).toBeInTheDocument();
    expect(screen.getByText(/Exécution terminée avec succès/)).toBeInTheDocument();

    // Renders reasoning with sub-agent context
    expect(screen.getByText(/Réflexion \(Agent En Ligne\)/)).toBeInTheDocument();
  });
});
