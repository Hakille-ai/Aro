import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import NotificationCenter from "../features/notifications/NotificationCenter.svelte";
import NotificationSettings from "../features/settings/pages/NotificationSettings.svelte";
import {
  playChimeSound,
  type NotificationItem,
  type NotificationSettings as NotificationSettingsType,
} from "../features/notifications/model";
import type { AppSettings } from "../lib/types";

// Mock Tauri invoke API
const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: any[]) => mockInvoke(...args),
}));

describe("Notification and Email System", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Web Audio Chime Synthesis", () => {
    it("safely generates a harmonic chime without throwing errors", () => {
      // Mock AudioContext in jsdom
      const mockOscillator = {
        type: "sine",
        frequency: { setValueAtTime: vi.fn(), exponentialRampToValueAtTime: vi.fn() },
        connect: vi.fn(),
        start: vi.fn(),
        stop: vi.fn(),
      };
      const mockGain = {
        gain: {
          setValueAtTime: vi.fn(),
          linearRampToValueAtTime: vi.fn(),
          exponentialRampToValueAtTime: vi.fn(),
        },
        connect: vi.fn(),
      };
      const mockContext = {
        currentTime: 0,
        state: "running",
        destination: {},
        createOscillator: vi.fn(() => mockOscillator),
        createGain: vi.fn(() => mockGain),
        resume: vi.fn(),
      };

      (window as any).AudioContext = vi.fn(function () {
        return mockContext;
      });

      expect(() => playChimeSound()).not.toThrow();
      expect(mockContext.createOscillator).toHaveBeenCalledTimes(2);
      expect(mockContext.createGain).toHaveBeenCalledTimes(2);
      expect(mockOscillator.start).toHaveBeenCalledTimes(2);
    });
  });

  describe("NotificationSettings Component", () => {
    const makeSettingsDraft = (): AppSettings & { notification: NotificationSettingsType } => ({
      model: {
        providers: [],
        activeModelRef: {
          providerId: "mock-local",
          providerKind: "mock",
          modelId: "mock",
          label: "Mock",
          family: "Mock",
          local: true,
          installed: true,
          ready: true,
        },
        fallbackPolicy: "local-first",
        provider: "mock",
        modelId: "mock",
        ollamaEndpoint: "http://127.0.0.1:11434",
        llamaCppEndpoint: "http://127.0.0.1:8080",
        temperature: 0.7,
        maxTokens: 768,
      },
      voice: {
        enabled: false,
        speechToText: "disabled",
        textToSpeech: "disabled",
        wakeWord: { enabled: false, runtime: "disabled", modelPath: null, threshold: 0.65 },
      },
      search: {
        provider: "google-scrape",
        authConfigured: false,
      },
      memory: {
        contextMode: "auto",
        totalTokenCeiling: 8192,
        systemBudget: 1024,
        semanticBudget: 2048,
        episodicBudget: 2048,
        workingBudget: 2048,
        reserveBudget: 1072,
        compactionInterval: 10,
        maxWorkingTurns: 30,
        topK: 5,
        minSalienceThreshold: 0.4,
        decayHalfLifeDays: 30,
        rrfFtsWeight: 1,
        rrfVecWeight: 1,
        rrfK: 60,
        autoMemorize: true,
      },
      notification: {
        desktopNotificationsEnabled: true,
        soundEnabled: true,
        agentCompletionNotifications: true,
        routineNotifications: true,
        emailNotificationsEnabled: true,
        emailOnAgentCompletion: true,
        emailOnRoutineSummary: false,
        emailRecipient: "admin@aro-ai.com",
        emailProvider: "smtp",
        smtpHost: "smtp.example.com",
        smtpPort: 587,
        smtpUser: "admin@aro-ai.com",
        smtpPassword: "",
        smtpFrom: "ARO Notifications <noreply@aro-ai.com>",
        smtpTlsMode: "starttls",
        apiKey: "",
        authConfigured: true,
      },
      retainHistory: true,
      speakResponses: false,
    });

    it("renders NotificationSettings and reflects configured values", () => {
      const draft = makeSettingsDraft();
      const { getByText, getByPlaceholderText } = render(NotificationSettings, {
        props: {
          settingsDraft: draft,
          language: "fr",
          onAutosave: vi.fn(),
        },
      });

      expect(getByText("Notifications & Envoi d'E-mails")).toBeTruthy();
      expect(getByText("Notifications du bureau OS")).toBeTruthy();
      expect(getByText("Carillon sonore cristallin")).toBeTruthy();
      expect(getByText("Achèvement de tâche par l'agent IA")).toBeTruthy();
      expect(getByText("Exécution de routines planifiées")).toBeTruthy();
      expect(getByText("Configuration du Serveur E-mail / SMTP")).toBeTruthy();
      expect(getByPlaceholderText("votre.email@domaine.com")).toBeTruthy();
    });

    it("triggers autosave on toggle change", async () => {
      const draft = makeSettingsDraft();
      const autosaveSpy = vi.fn();
      const { container } = render(NotificationSettings, {
        props: {
          settingsDraft: draft,
          language: "fr",
          onAutosave: autosaveSpy,
        },
      });

      const checkboxes = container.querySelectorAll("input[type='checkbox']");
      expect(checkboxes.length).toBeGreaterThan(0);
      await fireEvent.click(checkboxes[0]);
      expect(autosaveSpy).toHaveBeenCalled();
    });

    it("renders API key configuration when provider is set to Resend or SendGrid", () => {
      const draft = makeSettingsDraft();
      draft.notification.emailProvider = "resend";
      draft.notification.authConfigured = false;

      const { getByText, getByPlaceholderText } = render(NotificationSettings, {
        props: {
          settingsDraft: draft,
          language: "fr",
          onAutosave: vi.fn(),
        },
      });

      expect(getByText("Clé API / Token Sécurisé")).toBeTruthy();
      expect(getByPlaceholderText("re_123456789...")).toBeTruthy();
    });
  });

  describe("NotificationCenter Component", () => {
    const mockNotifications: NotificationItem[] = [
      {
        id: "notif-1",
        title: "✓ Agent ARO Terminé",
        body: "Pipeline d'analyse complété avec succès.",
        kind: "agent-completion",
        priority: "high",
        status: "unread",
        source: "agent",
        actionUrl: "conversation:conv-123",
        metadata: null,
        createdAt: new Date().toISOString(),
        readAt: null,
      },
      {
        id: "notif-2",
        title: "Routine Quotidienne",
        body: "Indexation vectorielle terminée.",
        kind: "routine",
        priority: "normal",
        status: "read",
        source: "routine",
        actionUrl: null,
        metadata: null,
        createdAt: new Date().toISOString(),
        readAt: new Date().toISOString(),
      },
    ];

    beforeEach(() => {
      (window as any).__TAURI_INTERNALS__ = {};
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "notification_list") return mockNotifications;
        if (cmd === "notification_unread_count") return 1;
        if (cmd === "notification_mark_read") return true;
        if (cmd === "notification_mark_all_read") return 1;
        if (cmd === "notification_delete") return true;
        if (cmd === "notification_clear_all") return 2;
        return null;
      });
    });

    it("renders the bell button with unread count badge", async () => {
      const { container } = render(NotificationCenter, {
        props: {
          activeOrganizationId: "org-1",
          language: "fr",
          soundEnabled: true,
        },
      });

      const bellBtn = container.querySelector(".bell-trigger-btn");
      expect(bellBtn).toBeTruthy();

      // Click to toggle flyout
      if (bellBtn) {
        await fireEvent.click(bellBtn);
      }
      expect(container.querySelector(".notification-flyout")).toBeTruthy();
    });

    it("reacts to workspace switching by refreshing scoped notifications", async () => {
      const { rerender } = render(NotificationCenter, {
        activeOrganizationId: "org-1",
        language: "fr",
        soundEnabled: true,
      });

      expect(mockInvoke).toHaveBeenCalledWith("notification_list", expect.objectContaining({
        filter: expect.objectContaining({ organizationId: "org-1" }),
      }));

      // Switch to personal workspace (null organization)
      await rerender({ activeOrganizationId: null });

      expect(mockInvoke).toHaveBeenCalledWith("notification_list", expect.objectContaining({
        filter: expect.objectContaining({ personalOnly: true }),
      }));
    });
  });
});
