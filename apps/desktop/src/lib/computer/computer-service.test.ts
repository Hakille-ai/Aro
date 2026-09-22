import { describe, it, expect, beforeEach } from "vitest";
import {
  defaultComputerPermissions,
  updateComputerPermissions,
  resetComputerPermissions,
  formatEnvironmentContextForPrompt,
  executeComputerTool,
  computerPermissions,
} from "./computer-service";
import type { ComputerEnvironmentContext, ComputerPermissions } from "./computer-types";
import { get } from "svelte/store";

describe("computer-service", () => {
  beforeEach(() => {
    resetComputerPermissions();
  });

  it("initializes with Apple-grade default permissions", () => {
    const perms = get(computerPermissions);
    expect(perms.enabled).toBe(true);
    expect(perms.allowComputerUse).toBe(true);
    expect(perms.shareBattery).toBe(true);
    expect(perms.shareIp).toBe(true);
    expect(perms.shareVolume).toBe(true);
    expect(perms.shareNetwork).toBe(true);
    expect(perms.shareDisplay).toBe(true);
    expect(perms.shareClipboard).toBe(false); // privacy-first default
    expect(perms.allowVolumeControl).toBe(true);
    expect(perms.allowScreenCapture).toBe(true);
    expect(perms.allowNetworkInspection).toBe(true);
    expect(perms.allowAppLauncher).toBe(true);
  });

  it("updates permissions reactively", () => {
    updateComputerPermissions({ shareBattery: false, shareClipboard: true });
    const perms = get(computerPermissions);
    expect(perms.shareBattery).toBe(false);
    expect(perms.shareClipboard).toBe(true);
  });

  it("formats environment context correctly in French and English", () => {
    const perms: ComputerPermissions = {
      ...defaultComputerPermissions,
      shareClipboard: true,
    };

    const mockInfo: ComputerEnvironmentContext = {
      date: "Samedi 19 septembre 2026",
      time: "05:30:00",
      timestamp: "2026-09-19T05:30:00.000Z",
      timezone: "Europe/Paris",
      platform: "Windows 11",
      arch: "x86_64",
      language: "fr-FR",
      battery: { level: 98, charging: true, label: "98% (En charge / AC)" },
      volume: { level: 35, muted: false },
      network: {
        online: true,
        type: "wifi",
        ssid: "Pixel 9 amd",
        signal: "96%",
        internalIp: "192.168.136.242",
      },
      display: { width: 1920, height: 1080, scaleFactor: 1.25 },
      clipboardSnippet: "https://example.com/api/test",
      stats: {
        osName: "Microsoft Windows 11",
        platform: "Windows",
        arch: "x86_64",
        cpuCores: 16,
        totalRamMb: 32768,
        usedRamMb: 20480,
      },
    };

    const frPrompt = formatEnvironmentContextForPrompt(perms, mockInfo, "fr");
    expect(frPrompt).toContain("[ENVIRONNEMENT ET SYSTÈME UTILISATEUR]");
    expect(frPrompt).toContain("Samedi 19 septembre 2026");
    expect(frPrompt).toContain("Windows 11 (x86_64)");
    expect(frPrompt).toContain("1920x1080 @ 1.25x DPI");
    expect(frPrompt).toContain("Batterie: 98% (En charge / AC)");
    expect(frPrompt).toContain("Volume audio système: 35% (actif)");
    expect(frPrompt).toContain("Wi-Fi: \"Pixel 9 amd\" (96%)");
    expect(frPrompt).toContain("IP: 192.168.136.242");
    expect(frPrompt).toContain("RAM: 20.0 GB / 32.0 GB");
    expect(frPrompt).toContain("Extrait presse-papiers: \"https://example.com/api/test\"");
    expect(frPrompt).toContain("Contrôle du volume sonore");
    expect(frPrompt).toContain("Capture d'écran");

    const enPrompt = formatEnvironmentContextForPrompt(perms, mockInfo, "en");
    expect(enPrompt).toContain("[USER WORKSTATION & ENVIRONMENT CONTEXT]");
    expect(enPrompt).toContain("System Audio Volume: 35% (unmuted)");
    expect(enPrompt).toContain("Battery: 98% (En charge / AC)");
  });

  it("omits disabled fields when user revokes permissions", () => {
    const perms: ComputerPermissions = {
      ...defaultComputerPermissions,
      shareBattery: false,
      shareIp: false,
      shareVolume: false,
      shareClipboard: false,
    };

    const mockInfo: ComputerEnvironmentContext = {
      date: "Saturday, September 19, 2026",
      time: "05:30:00",
      timestamp: "2026-09-19T05:30:00.000Z",
      timezone: "Europe/London",
      platform: "macOS",
      arch: "arm64",
      language: "en-US",
      battery: { level: 50, charging: false, label: "50% (On battery)" },
      volume: { level: 80, muted: false },
      network: {
        online: true,
        type: "wifi",
        ssid: "Office-Network",
        internalIp: "10.0.0.45",
      },
      clipboardSnippet: "Secret password",
    };

    const prompt = formatEnvironmentContextForPrompt(perms, mockInfo, "en");
    expect(prompt).not.toContain("Battery:");
    expect(prompt).not.toContain("System Audio Volume:");
    expect(prompt).not.toContain("10.0.0.45"); // IP omitted!
    expect(prompt).not.toContain("Secret password"); // Clipboard omitted!
    expect(prompt).toContain("Office-Network"); // General network still present
  });

  it("returns empty string when master switch is disabled", () => {
    const perms: ComputerPermissions = {
      ...defaultComputerPermissions,
      enabled: false,
    };

    const mockInfo: ComputerEnvironmentContext = {
      date: "2026-09-19",
      time: "05:30:00",
      timestamp: "2026-09-19T05:30:00.000Z",
      timezone: "UTC",
      platform: "Linux",
      arch: "x86_64",
      language: "en",
    };

    const prompt = formatEnvironmentContextForPrompt(perms, mockInfo, "fr");
    expect(prompt).toBe("");
  });

  it("blocks tools when permissions are disabled", async () => {
    const disabledPerms: ComputerPermissions = {
      ...defaultComputerPermissions,
      allowVolumeControl: false,
      allowScreenCapture: false,
    };

    const volRes = await executeComputerTool("volume_set", { level: 40 }, disabledPerms);
    expect(volRes.success).toBe(false);
    expect(volRes.error).toContain("Volume control tool is disabled");

    const screenRes = await executeComputerTool("screenshot", {}, disabledPerms);
    expect(screenRes.success).toBe(false);
    expect(screenRes.error).toContain("Screen capture tool is disabled");
  });

  it("executes permitted tools successfully in sandbox/browser mode", async () => {
    const perms: ComputerPermissions = { ...defaultComputerPermissions };

    const volRes = await executeComputerTool("volume_set", { level: 65 }, perms);
    expect(volRes.success).toBe(true);
    expect(volRes.output?.volume).toBe(65);

    const netRes = await executeComputerTool("network_info", {}, perms);
    expect(netRes.success).toBe(true);
    expect(netRes.output?.online).toBeDefined();
  });
});
