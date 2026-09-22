import { describe, expect, it } from "vitest";
import {
  INSTRUCTION_DEFAULTS_VERSION,
  INSTRUCTION_PROMPT_CHAR_LIMIT,
  buildDefaultSystemPrompts,
  compileSystemPrompt,
  estimateInstructionTokens,
  migrateInstructionDefaults,
  selectInstructionMemories,
  type InstructionMemory,
} from "./instructions";

const memories: InstructionMemory[] = [
  {
    id: "m1",
    content: "The project uses Svelte and Rust.",
    category: "technical",
    createdAt: "2026-07-01",
  },
  {
    id: "m2",
    content: "The user prefers concise French answers.",
    category: "preference",
    createdAt: "2026-07-01",
  },
  {
    id: "m3",
    content: "Ollama is configured on localhost.",
    category: "system",
    createdAt: "2026-07-01",
  },
];

describe("instructions", () => {
  it("combines canonical base, personality, memory, and environment", () => {
    const prompt = compileSystemPrompt({
      mode: "code",
      personality: {
        id: "developer",
        name: "Developer",
        description: "Code profile",
        prompt: "Prefer small, tested changes.",
        icon: "code",
        avatarColor: "blue",
        temperature: 0.2,
      },
      customSystemPromptsEnabled: false,
      customSystemPrompts: {},
      customInstructionsEnabled: true,
      memories,
      userInput: "Please improve the Svelte project",
      language: "fr",
      model: {
        providerId: "ollama-local",
        providerKind: "ollama",
        modelId: "gemma3:1b",
        label: "gemma3:1b",
        local: true,
        installed: true,
        ready: true,
      },
      runtimeDetail: "Ollama is ready.",
    });

    expect(prompt).toContain("You are ARO");
    expect(prompt).toContain("[MODE: CODE]");
    expect(prompt).toContain("[ACTIVE PERSONALITY PROFILE]");
    expect(prompt).toContain("Prefer small, tested changes.");
    expect(prompt).toContain("[USER MEMORY]");
    expect(prompt).toContain("Svelte and Rust");
    expect(prompt).toContain("gemma3:1b");
  });

  it("respects custom system prompts while still adding personality when enabled", () => {
    const prompt = compileSystemPrompt({
      mode: "chat",
      personality: {
        id: "writer",
        name: "Writer",
        description: "Writing profile",
        prompt: "Use vivid but concise language.",
        icon: "edit-2",
        avatarColor: "purple",
        temperature: 0.8,
      },
      customSystemPromptsEnabled: true,
      customSystemPrompts: { chat: "[CUSTOM]\nStay compact." },
      customInstructionsEnabled: true,
      memories: [],
    });

    expect(prompt).toContain("[CUSTOM]");
    expect(prompt).toContain("Use vivid but concise language.");
  });

  it("can disable personality while retaining canonical defaults", () => {
    const prompt = compileSystemPrompt({
      mode: "chat",
      personality: null,
      customSystemPromptsEnabled: false,
      customSystemPrompts: {},
      customInstructionsEnabled: false,
      memories: [],
    });

    expect(prompt).toContain("You are ARO");
    expect(prompt).not.toContain("[ACTIVE PERSONALITY PROFILE]");
  });

  it("migrates only old known defaults", () => {
    const custom = "My custom prompt must stay.";
    const result = migrateInstructionDefaults({
      storedVersion: null,
      customSystemPrompts: {
        ...buildDefaultSystemPrompts(),
        chat: "You are ARO, a calm local assistant. Be direct, helpful, and concise.",
        code: custom,
      },
      customPersonalities: [
        {
          id: "custom",
          name: "Custom",
          description: "Custom",
          prompt: "Do not overwrite me.",
          icon: "bot",
          avatarColor: "blue",
          temperature: 0.7,
        },
      ],
    });

    expect(result.version).toBe(INSTRUCTION_DEFAULTS_VERSION);
    expect(result.customSystemPrompts.chat).toContain("[ARO IDENTITY]");
    expect(result.customSystemPrompts.code).toBe(custom);
    expect(result.customPersonalities[0].prompt).toBe("Do not overwrite me.");
  });

  it("keeps compiled prompts under the budget", () => {
    const prompt = compileSystemPrompt({
      mode: "think",
      personality: {
        id: "long",
        name: "Long",
        description: "Long",
        prompt: "x".repeat(20_000),
        icon: "brain",
        avatarColor: "blue",
        temperature: 0.7,
      },
      customSystemPromptsEnabled: false,
      customSystemPrompts: {},
      customInstructionsEnabled: true,
      memories,
    });

    expect(prompt.length).toBeLessThanOrEqual(INSTRUCTION_PROMPT_CHAR_LIMIT);
    expect(estimateInstructionTokens(prompt)).toBeGreaterThan(0);
  });

  it("selects relevant memories before filler memories", () => {
    const selected = selectInstructionMemories(memories, "Svelte project");

    expect(selected[0].id).toBe("m1");
    expect(selected.some((memory) => memory.category === "preference")).toBe(true);
  });

  it("injects rich OS and environment context when provided", () => {
    const promptWithObject = compileSystemPrompt({
      mode: "chat",
      customSystemPromptsEnabled: false,
      customSystemPrompts: {},
      customInstructionsEnabled: false,
      language: "fr",
      osEnvironment: {
        date: "Samedi 19 septembre 2026",
        time: "05:40:00",
        timezone: "Europe/Paris",
        platform: "Windows 11",
        arch: "x86_64",
        battery: { level: 98, charging: true, label: "98% (En charge)" },
        volume: { level: 40, muted: false },
        network: { online: true, ssid: "ARO-5G", signal: "95%", internalIp: "192.168.1.100" },
        display: { width: 2560, height: 1440, scaleFactor: 2 },
        clipboardSnippet: "const foo = 'bar';",
      },
    });

    expect(promptWithObject).toContain("[ENVIRONNEMENT ET SYSTÈME UTILISATEUR]");
    expect(promptWithObject).toContain("Samedi 19 septembre 2026, 05:40:00 (Fuseau: Europe/Paris)");
    expect(promptWithObject).toContain("Windows 11 (x86_64)");
    expect(promptWithObject).toContain("Batterie: 98% (En charge)");
    expect(promptWithObject).toContain("Volume audio système: 40% (actif)");
    expect(promptWithObject).toContain("Wi-Fi: \"ARO-5G\" (95%)");
    expect(promptWithObject).toContain("IP: 192.168.1.100");
    expect(promptWithObject).toContain("2560x1440 @ 2x DPI");
    expect(promptWithObject).toContain("Extrait presse-papiers: \"const foo = 'bar';\"");

    const promptWithString = compileSystemPrompt({
      mode: "chat",
      customSystemPromptsEnabled: false,
      customSystemPrompts: {},
      customInstructionsEnabled: false,
      osEnvironment: "[USER WORKSTATION & ENVIRONMENT CONTEXT]\n- OS: macOS Sonoma\n- Battery: 85%",
    });

    expect(promptWithString).toContain("[USER WORKSTATION & ENVIRONMENT CONTEXT]");
    expect(promptWithString).toContain("- OS: macOS Sonoma");
  });
});
