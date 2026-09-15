import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Composer from "./Composer.svelte";
import type { WorkspaceMentionEntry } from "./mention-model";

const labels = new Proxy<Record<string, string>>({}, { get: (_target, key) => String(key) });
const noop = vi.fn();

const mentionEntries: WorkspaceMentionEntry[] = [
  { name: "App.svelte", path: "/w/src/App.svelte", isDir: false, relativePath: "src/App.svelte", extension: "svelte" },
  { name: "app-store.ts", path: "/w/src/app-store.ts", isDir: false, relativePath: "src/app-store.ts", extension: "ts" },
  { name: "src", path: "/w/src", isDir: true, relativePath: "src", extension: "" },
];

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
    formatFileSize: () => "1 KB", getWavePath: () => "M0 0", onSubmitMessage: noop,
    onFilesChange: noop, onPreviewAttachment: noop, onUploadAttachment: noop, onRemoveAttachment: noop,
    onResizeComposer: noop, onOpenFilePicker: noop, onToggleWebAccess: noop, onSetVoiceInputMode: noop,
    onToggleRecording: noop, onStopSpeaking: noop, onSelectModel: noop, onOpenModelSettings: noop,
    activePermissionMode: "standard", activePermissionLabel: "Standard", permissionProfiles: [], activePermissionProfileId: "",
    onSelectPermissionPreset: noop, onSelectPermissionProfile: noop, onOpenPermissionSettings: noop,
    workspaceMentionEntries: mentionEntries,
    ...overrides,
  } as any;
}

describe("Composer @ mentions (F4.2, shipped code)", () => {
  it("displays the popover on @ trigger with file and dir suggestions", async () => {
    render(Composer, composerProps());
    const textarea = screen.getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: "review @App" } });
    textarea.setSelectionRange(11, 11);
    await fireEvent.keyUp(textarea, { key: "p" });
    expect(screen.getByRole("listbox", { name: "Project files" })).toBeInTheDocument();
    expect(screen.getByText("App.svelte")).toBeInTheDocument();
  });

  it("navigates with ArrowDown (cyclic wrap) and confirms with Enter", async () => {
    render(Composer, composerProps());
    const textarea = screen.getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: "@" } });
    textarea.setSelectionRange(1, 1);
    await fireEvent.keyUp(textarea, { key: "@" });
    const options = screen.getAllByRole("option");
    expect(options.length).toBe(3);
    // Wrap: ArrowUp from first goes to last.
    await fireEvent.keyDown(textarea, { key: "ArrowUp" });
    expect(screen.getAllByRole("option")[2]).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(textarea, { key: "ArrowDown" });
    expect(screen.getAllByRole("option")[0]).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(textarea, { key: "Enter" });
    expect(textarea.value).toBe("@src/App.svelte ");
    expect(screen.queryByRole("listbox", { name: "Project files" })).not.toBeInTheDocument();
  });

  it("confirms with Tab and dismisses with Escape", async () => {
    render(Composer, composerProps());
    const textarea = screen.getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: "fix @src" } });
    textarea.setSelectionRange(8, 8);
    await fireEvent.keyUp(textarea, { key: "c" });
    await fireEvent.keyDown(textarea, { key: "Tab" });
    expect(textarea.value).toContain("@src ");
    await fireEvent.input(textarea, { target: { value: `${textarea.value}@a` } });
    textarea.setSelectionRange(textarea.value.length, textarea.value.length);
    await fireEvent.keyUp(textarea, { key: "a" });
    await fireEvent.keyDown(textarea, { key: "Escape" });
    expect(screen.queryByRole("listbox", { name: "Project files" })).not.toBeInTheDocument();
  });

  it("closes the popover when input is cleared externally (message sent)", async () => {
    const { rerender } = render(Composer, composerProps({ input: "@App" }));
    const textarea = screen.getByRole("textbox") as HTMLTextAreaElement;
    textarea.setSelectionRange(4, 4);
    await fireEvent.keyUp(textarea, { key: "p" });
    expect(screen.getByRole("listbox", { name: "Project files" })).toBeInTheDocument();
    await rerender(composerProps({ input: "" }));
    expect(screen.queryByRole("listbox", { name: "Project files" })).not.toBeInTheDocument();
  });

  it("shows an empty state when nothing matches and ignores emails", async () => {    render(Composer, composerProps());
    const textarea = screen.getByRole("textbox") as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: "contact user@example.com" } });
    textarea.setSelectionRange(24, 24);
    await fireEvent.keyUp(textarea, { key: "m" });
    expect(screen.queryByRole("listbox", { name: "Project files" })).not.toBeInTheDocument();
    await fireEvent.input(textarea, { target: { value: "contact user@example.com @zzz-no-match" } });
    const v = (screen.getByRole("textbox") as HTMLTextAreaElement).value;
    textarea.setSelectionRange(v.length, v.length);
    await fireEvent.keyUp(textarea, { key: "h" });
    expect(screen.getByText("No matching files.")).toBeInTheDocument();
  });
});
