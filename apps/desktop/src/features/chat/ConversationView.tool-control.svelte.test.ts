import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ConversationView from "./ConversationView.svelte";
import type { ChatMessage } from "../../lib/types";

const labels = new Proxy<Record<string, string>>({}, { get: (_target, key) => String(key) });
const noop = vi.fn();

function makeProps(overrides: Record<string, unknown> = {}) {
  return {
    conversationContainer: undefined,
    messagesEnd: undefined,
    loading: false,
    messages: [],
    labels,
    language: "fr",
    theme: "light",
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
    onToggleRecording: noop,
    onStopSpeaking: noop,
    onSetVoiceInputMode: noop,
    onSelectPersonality: noop,
    onToggleMessageSteps: noop,
    onToggleStepDetails: noop,
    onPreviewAttachment: noop,
    onCopyMessage: noop,
    onRememberMessage: noop,
    onFeedback: noop,
    onStartEdit: noop,
    onCancelEdit: noop,
    onSaveEdit: noop,
    ...overrides,
  } as any;
}

describe("ConversationView In-Message Tool Execution & Control", () => {
  it("displays live tool strip when assistant message has a browser tool step", () => {
    const messages: ChatMessage[] = [
      {
        id: "msg-1",
        conversationId: "conv-1",
        role: "assistant",
        content: "Je consulte le site Hacker News pour extraire les titres.",
        createdAt: "2026-09-17T05:00:00Z",
        isGenerating: false,
        steps: [
          {
            id: "step-1",
            runId: "run-1",
            startedAt: "2026-09-17T05:00:00Z",
            sequence: 1,
            kind: "tool",
            status: "completed",
            title: "Navigation vers news.ycombinator.com",
            input: {
              toolId: "browser.navigate",
              url: "https://news.ycombinator.com",
            },
            output: {
              url: "https://news.ycombinator.com",
              title: "Hacker News",
            },
          },
        ],
      },
    ];

    render(ConversationView, makeProps({ messages }));

    expect(screen.getByText("Navigation vers news.ycombinator.com")).toBeInTheDocument();
    expect(screen.getByText("https://news.ycombinator.com")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Prendre le contrôle" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Ouvrir l'onglet" })).toBeInTheDocument();
  });

  it("dispatches aro:open-browser with takeControl: true when Prendre le contrôle is clicked", async () => {
    const messages: ChatMessage[] = [
      {
        id: "msg-2",
        conversationId: "conv-1",
        role: "assistant",
        content: "Analyse en cours...",
        createdAt: "2026-09-17T05:01:00Z",
        isGenerating: true,
        steps: [
          {
            id: "step-2",
            runId: "run-1",
            startedAt: "2026-09-17T05:01:00Z",
            sequence: 1,
            kind: "tool",
            status: "running",
            title: "Recherche sur le web",
            input: {
              toolId: "browser.open",
              url: "https://news.ycombinator.com",
            },
            output: null,
          },
        ],
      },
    ];

    const dispatchSpy = vi.spyOn(window, "dispatchEvent");
    render(ConversationView, makeProps({ messages }));

    const takeControlBtn = screen.getByRole("button", { name: "Prendre le contrôle" });
    await fireEvent.click(takeControlBtn);

    expect(dispatchSpy).toHaveBeenCalled();
    const event = dispatchSpy.mock.calls.find(
      ([e]) => e instanceof CustomEvent && e.type === "aro:open-browser"
    )?.[0] as CustomEvent;

    expect(event).toBeDefined();
    expect(event.detail).toEqual({
      url: "https://news.ycombinator.com",
      takeControl: true,
      newTab: false,
    });
  });

  it("dispatches aro:open-browser with newTab: true when Ouvrir l'onglet is clicked", async () => {
    const messages: ChatMessage[] = [
      {
        id: "msg-3",
        conversationId: "conv-1",
        role: "assistant",
        content: "Page ouverte",
        createdAt: "2026-09-17T05:02:00Z",
        isGenerating: false,
        steps: [
          {
            id: "step-3",
            runId: "run-1",
            startedAt: "2026-09-17T05:02:00Z",
            sequence: 1,
            kind: "tool",
            status: "completed",
            title: "Lecture de documentation",
            input: {
              toolId: "browser.tab",
              url: "https://tauri.app/docs",
            },
            output: {
              url: "https://tauri.app/docs",
            },
          },
        ],
      },
    ];

    const dispatchSpy = vi.spyOn(window, "dispatchEvent");
    render(ConversationView, makeProps({ messages }));

    const openTabBtn = screen.getByRole("button", { name: "Ouvrir l'onglet" });
    await fireEvent.click(openTabBtn);

    const event = dispatchSpy.mock.calls.find(
      ([e]) => e instanceof CustomEvent && e.type === "aro:open-browser"
    )?.[0] as CustomEvent;

    expect(event.detail).toEqual({
      url: "https://tauri.app/docs",
      takeControl: false,
      newTab: true,
    });
  });

  it("renders both Prendre le contrôle and Ouvrir l'onglet in expanded AgentStepCard", async () => {
    const step = {
      id: "step-card-test",
      runId: "run-card-1",
      startedAt: "2026-09-17T05:05:00Z",
      sequence: 1,
      kind: "tool" as const,
      status: "completed" as const,
      title: "Inspection web",
      input: {
        toolId: "core.browser.navigate",
        url: "https://svelte.dev",
      },
      output: {
        url: "https://svelte.dev",
      },
    };

    const messages: ChatMessage[] = [
      {
        id: "msg-expanded",
        conversationId: "conv-1",
        role: "assistant",
        content: "Analyse terminée",
        createdAt: "2026-09-17T05:05:00Z",
        isGenerating: false,
        steps: [step],
      },
    ];

    const dispatchSpy = vi.spyOn(window, "dispatchEvent");
    const { container } = render(
      ConversationView,
      makeProps({
        messages,
        expandedMessageSteps: { "msg-expanded": true },
      })
    );

    const takeControlBtn = container.querySelector<HTMLButtonElement>(".step-take-control-pill");
    const openTabBtn = container.querySelector<HTMLButtonElement>(".step-open-tab-pill");

    expect(takeControlBtn).toBeDefined();
    expect(openTabBtn).toBeDefined();

    if (openTabBtn) {
      await fireEvent.click(openTabBtn);
      const event = dispatchSpy.mock.calls.find(
        ([e]) => e instanceof CustomEvent && e.type === "aro:open-browser"
      )?.[0] as CustomEvent;
      expect(event?.detail).toEqual({ url: "https://svelte.dev", newTab: true, takeControl: false });
    }
  });
});
