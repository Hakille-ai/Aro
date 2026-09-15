import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import CloudAuthPage from "../features/auth/CloudAuthPage.svelte";
import Composer from "../features/chat/Composer.svelte";
import ConversationTopbar from "../features/chat/ConversationTopbar.svelte";
import ConversationView from "../features/chat/ConversationView.svelte";
import PathsSettings from "../features/settings/pages/PathsSettings.svelte";
import MainSidebar from "../features/shell/MainSidebar.svelte";

const labels = new Proxy<Record<string, string>>({}, { get: (_target, key) => String(key) });
const noop = vi.fn();
const asyncNoop = vi.fn().mockResolvedValue(undefined);

const conversation = {
  id: "conversation-1",
  title: "Refactor safely",
  mode: "chat",
  createdAt: "2026-07-15T10:00:00Z",
  updatedAt: "2026-07-15T10:01:00Z",
};

const personality = {
  id: "default",
  name: "ARO",
  description: "Default",
  prompt: "Helpful",
  icon: "bot",
  avatarColor: "#0071e3",
  temperature: 0.7,
};

describe("CloudAuthPage", () => {
  it("binds credentials, toggles password visibility and submits", async () => {
    const onSubmit = vi.fn();
    render(CloudAuthPage, {
      language: "en", mode: "login", invitationToken: "", email: "", password: "", name: "",
      organizationName: "ARO", showPassword: false, busy: false, error: "", onSubmit, onToggleMode: noop,
    });

    const password = screen.getByPlaceholderText("Password (min. 10 chars)");
    await fireEvent.input(screen.getByPlaceholderText("Email address"), { target: { value: "user@aro.dev" } });
    await fireEvent.input(password, { target: { value: "long-password" } });
    expect(screen.getByRole("button", { name: "Sign In" })).toBeEnabled();
    await fireEvent.click(password.parentElement!.querySelector("button")!);
    expect(password).toHaveAttribute("type", "text");
    await fireEvent.submit(password.closest("form")!);
    expect(onSubmit).toHaveBeenCalledOnce();
  });
});

describe("MainSidebar", () => {
  it("searches, opens a conversation menu and forwards actions", async () => {
    const onOpenConversation = vi.fn();
    const onRenameConversation = vi.fn();
    render(MainSidebar, {
      labels, searchQuery: "query", activeSidebarMenuId: null, showCloudAuthPanel: false,
      conversations: [conversation], activeConversation: conversation, sendingByConversation: {}, cloudWriteLocked: false,
      cloudAuthenticated: false, cloudSyncStatus: { health: "offline-read-only", pendingEvents: 2, lastSyncedAt: null },
      cloudSession: null, cloudStatusShortLabel: "", runtime: null, isConversationSending: () => false,
      formatRelativeTime: () => "1m", cloudWriteDisabledTitle: () => undefined, onStartConversation: noop,
      onOpenConversation, onRenameConversation, onRemoveConversation: noop, onOpenSettings: noop,
    } as any);

    await fireEvent.click(screen.getByRole("button", { name: "Refactor safely" }));
    expect(onOpenConversation).toHaveBeenCalledWith(expect.objectContaining({ id: "conversation-1" }));
    await fireEvent.click(screen.getByRole("button", { name: "conversationOptions" }));
    await fireEvent.click(screen.getByRole("button", { name: "rename" }));
    expect(onRenameConversation).toHaveBeenCalledOnce();
    await fireEvent.click(screen.getByTitle("clearSearch"));
    expect(screen.getByPlaceholderText("searchPlaceholder")).toHaveValue("");
  });
});

describe("ConversationTopbar", () => {
  it("selects personalities and toggles both panels", async () => {
    const onSelect = vi.fn();
    render(ConversationTopbar, {
      activeConversation: conversation, conversationPersonalities: {}, selectedPersonalityId: "default",
      personalities: [personality, { ...personality, id: "code", name: "Code", icon: "code" }], theme: "light",
      language: "en", labels, showTopbarPersonalityDropdown: false, showConversationMenu: false,
      showContextPanel: false, showRightPanel: false, sidebarOpen: true, isTauriEnv: false, attachedFileCount: 2, cloudWriteLocked: false,
      cloudWriteDisabledTitle: () => undefined, ensureCloudWriteAllowed: () => true,
      onSelectConversationPersonality: onSelect, onRenameConversation: noop, onRemoveConversation: noop,
    } as any);

    await fireEvent.click(screen.getByTitle("ARO"));
    await fireEvent.click(screen.getByRole("button", { name: "Code" }));
    expect(onSelect).toHaveBeenCalledWith("conversation-1", "code");
    expect(screen.getByLabelText("Right panel")).toBeInTheDocument();
  });
});

function conversationViewProps(overrides: Record<string, unknown> = {}) {
  return {
    conversationContainer: undefined, messagesEnd: undefined, loading: false, messages: [], labels, language: "en",
    theme: "light", personalities: [personality], selectedPersonalityId: "default", cloudWriteLocked: false,
    cloudWriteDisabledTitle: () => undefined, recording: false, assistantSpeaking: false, voiceVolume: 0,
    wakeWordEnabled: false, voiceStateLabel: "Ready", recordingHint: "", voiceStateHint: "Speak",
    runtimeDetail: "Ready", modelRuntimeReady: true, voiceSpeechToTextReady: true, voiceWakeModelReady: false,
    voiceTextToSpeechReady: true, speakResponses: false, speechToTextIssue: "", textToSpeechIssue: "",
    modelReadinessLabel: "Model ready", speechReadinessLabel: "Speech ready", wakeWordReadinessLabel: "Wake off",
    textToSpeechReadinessLabel: "TTS ready", voiceModeOptions: [], voiceInputMode: "push-to-talk",
    voiceHandsFreeArmed: false, expandedMessageSteps: {}, expandedStepDetails: {}, copiedMessageId: null,
    messageFeedback: {}, editingMessageId: null, editingMessageText: "", sending: false,
    getWavePath: () => "M0 0", formatTime: () => "10:00", formatFileSize: () => "1 KB",
    onToggleRecording: noop, onStopSpeaking: noop, onSetVoiceInputMode: noop, onSelectPersonality: noop,
    onToggleMessageSteps: noop, onToggleStepDetails: noop, onPreviewAttachment: noop, onCopyMessage: noop,
    onRememberMessage: noop, onFeedback: noop, onStartEdit: noop, onCancelEdit: noop, onSaveEdit: noop,
    ...overrides,
  } as any;
}

describe("ConversationView", () => {
  it("covers empty personality selection and message actions", async () => {
    const onSelectPersonality = vi.fn();
    const { rerender } = render(ConversationView, conversationViewProps({ onSelectPersonality }));
    await fireEvent.click(screen.getByRole("button", { name: "ARO" }));
    expect(onSelectPersonality).toHaveBeenCalledWith("default");

    const onCopyMessage = vi.fn();
    await rerender(conversationViewProps({
      messages: [{ id: "m1", conversationId: "conversation-1", role: "user", content: "Hello", createdAt: "now" }],
      onCopyMessage,
    }));
    await fireEvent.click(screen.getByRole("button", { name: "copyBtn" }));
    expect(onCopyMessage).toHaveBeenCalledWith("m1", "Hello");
  });
});

function composerProps(overrides: Record<string, unknown> = {}) {
  return {
    labels, language: "en", errorMessage: "", input: "", fileInput: undefined, composerInput: undefined,
    attachedFiles: [], cloudWriteLocked: false, cloudAuthenticated: true, messagesCount: 1, webAccess: "auto",
    voiceModeOptions: [], voiceInputMode: "push-to-talk", voiceHandsFreeArmed: false, recording: false,
    assistantSpeaking: false, voiceVolume: 0, wakeWordEnabled: false, recordingHint: "", voiceStateLabel: "Ready",
    voiceStateHint: "", modelRuntimeReady: true, voiceSpeechToTextReady: true, voiceWakeModelReady: false,
    modelReadinessLabel: "Ready", speechReadinessLabel: "Ready", wakeWordReadinessLabel: "Off",
    settingsAvailable: true, changingModel: false, runtimeDetail: "Ready", currentModelLabel: "Local",
    modelMenuOpen: false, modelSearchQuery: "", modelOptions: [], activeModelKey: "", arenaMode: false,
    arenaSending: false, arenaModelA: "", arenaModelB: "", sending: false,
    cloudWriteDisabledTitle: () => undefined, webAccessTitle: () => "Web auto", webAccessAriaLabel: () => "Web auto",
    formatFileSize: () => "1 KB", getWavePath: () => "M0 0", onSubmitMessage: asyncNoop,
    onFilesChange: noop, onPreviewAttachment: noop, onUploadAttachment: noop, onRemoveAttachment: noop,
    onResizeComposer: noop, onOpenFilePicker: noop, onToggleWebAccess: noop, onSetVoiceInputMode: noop,
    onToggleRecording: noop, onStopSpeaking: noop, onSelectModel: noop, onOpenModelSettings: noop,
    activePermissionMode: "standard", activePermissionLabel: "Standard", permissionProfiles: [], activePermissionProfileId: "",
    onSelectPermissionPreset: noop, onSelectPermissionProfile: noop, onOpenPermissionSettings: noop,
    ...overrides,
  } as any;
}

describe("Composer", () => {
  it("binds text, submits by shortcut, and exposes attachment actions", async () => {
    const onSubmitMessage = vi.fn();
    const onUploadAttachment = vi.fn();
    render(Composer, composerProps({
      onSubmitMessage, onUploadAttachment,
      attachedFiles: [{ id: "f1", name: "brief.pdf", size: 10, type: "application/pdf", file: new File([], "brief.pdf"), mode: "local-reference", uploadStatus: "local" }],
    }));
    const textarea = screen.getByRole("textbox");
    await fireEvent.input(textarea, { target: { value: "Ship it" } });
    await fireEvent.keyDown(textarea, { key: "Enter", ctrlKey: true });
    expect(onSubmitMessage).toHaveBeenCalledOnce();
    await fireEvent.click(screen.getByRole("button", { name: /Uploader brief\.pdf/ }));
    expect(onUploadAttachment).toHaveBeenCalledWith("f1");
  });

  it("opens permission menu and allows selecting permission preset", async () => {
    const onSelectPermissionPreset = vi.fn();
    render(Composer, composerProps({ onSelectPermissionPreset }));
    const permBtn = screen.getByRole("button", { name: "Permissions" });
    expect(permBtn).toBeInTheDocument();
    await fireEvent.click(permBtn);
    const readOnlyOption = screen.getByText("Read-only");
    expect(readOnlyOption).toBeInTheDocument();
    await fireEvent.click(readOnlyOption);
    expect(onSelectPermissionPreset).toHaveBeenCalledWith("read-only");
  });
});

describe("PathsSettings", () => {
  it("binds runtime paths and autosaves on blur", async () => {
    const onAutosave = vi.fn();
    render(PathsSettings, {
      settingsDraft: { voice: { whisperBinary: "whisper-cli", whisperModelPath: "", piperBinary: "piper", piperVoicePath: "", wakeWord: { modelPath: "", threshold: 0.5 } } },
      language: "en", labels, writeLocked: false, onAutosave,
    } as any);
    const whisper = screen.getByPlaceholderText("whisper-cli");
    await fireEvent.input(whisper, { target: { value: "C:/aro/whisper.exe" } });
    await fireEvent.blur(whisper);
    expect(whisper).toHaveValue("C:/aro/whisper.exe");
    expect(onAutosave).toHaveBeenCalledOnce();
  });
});
