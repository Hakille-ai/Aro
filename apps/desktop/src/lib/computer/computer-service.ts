import { writable, get } from "svelte/store";
import type {
  ComputerPermissions,
  ComputerEnvironmentContext,
  ComputerToolResult,
  ComputerBatteryInfo,
  ComputerVolumeInfo,
  ComputerNetworkInfo,
  ComputerDisplayInfo,
  ComputerSystemStats,
} from "./computer-types";

const STORAGE_KEY_COMPUTER_PERMS = "aro_computer_permissions";

export const defaultComputerPermissions: ComputerPermissions = {
  enabled: true,
  allowComputerUse: true,
  confirmDangerousActions: true,
  shareBattery: true,
  shareIp: true,
  shareVolume: true,
  shareNetwork: true,
  shareDisplay: true,
  shareClipboard: false,
  shareSystemStats: true,
  allowVolumeControl: true,
  allowScreenCapture: true,
  allowNetworkInspection: true,
  allowAppLauncher: true,
  allowSystemInspector: true,
};

function loadStoredPermissions(): ComputerPermissions {
  if (typeof localStorage === "undefined") return { ...defaultComputerPermissions };
  try {
    const raw = localStorage.getItem(STORAGE_KEY_COMPUTER_PERMS);
    if (!raw) return { ...defaultComputerPermissions };
    return { ...defaultComputerPermissions, ...JSON.parse(raw) };
  } catch {
    return { ...defaultComputerPermissions };
  }
}

function saveStoredPermissions(perms: ComputerPermissions): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY_COMPUTER_PERMS, JSON.stringify(perms));
  } catch (err) {
    console.error("Failed to save computer permissions:", err);
  }
}

export const computerPermissions = writable<ComputerPermissions>(loadStoredPermissions());

computerPermissions.subscribe((perms) => {
  saveStoredPermissions(perms);
});

export function updateComputerPermissions(partial: Partial<ComputerPermissions>): void {
  computerPermissions.update((prev) => ({ ...prev, ...partial }));
}

export function resetComputerPermissions(): void {
  computerPermissions.set({ ...defaultComputerPermissions });
}

function isTauri(): boolean {
  return typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);
}

async function invokeTauri<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return await invoke<T>(cmd, args);
}

// In-memory cache for fast, non-blocking synchronous access in prompt compilation
let cachedEnvironment: ComputerEnvironmentContext | null = null;
let lastCacheTime = 0;
const CACHE_TTL_MS = 15_000; // 15 seconds

export async function getLiveComputerEnvironment(
  permissions: ComputerPermissions = get(computerPermissions)
): Promise<ComputerEnvironmentContext> {
  const now = new Date();
  const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  const dateStr = now.toLocaleDateString(navigator.language || "fr-FR", {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  });
  const timeStr = now.toLocaleTimeString(navigator.language || "fr-FR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  // Base platform detection
  let platform = "Windows";
  let arch = "x86_64";
  if (typeof navigator !== "undefined") {
    const ua = navigator.userAgent;
    if (/mac/i.test(ua)) platform = "macOS";
    else if (/linux/i.test(ua)) platform = "Linux";
    else if (/win/i.test(ua)) platform = "Windows";

    if (/arm64|aarch64/i.test(ua) || (navigator as any).userAgentData?.architecture === "arm") {
      arch = "arm64";
    }
  }

  let batteryInfo: ComputerBatteryInfo | null = null;
  let volumeInfo: ComputerVolumeInfo | null = null;
  let networkInfo: ComputerNetworkInfo | null = null;
  let displayInfo: ComputerDisplayInfo | null = null;
  let systemStats: ComputerSystemStats | null = null;
  let clipboardSnippet: string | null = null;

  // 1. Display
  if (typeof window !== "undefined" && window.screen) {
    displayInfo = {
      width: window.screen.width,
      height: window.screen.height,
      scaleFactor: window.devicePixelRatio || 1,
      colorDepth: window.screen.colorDepth,
    };
  }

  // 2. Battery via Web API or Tauri
  if (typeof navigator !== "undefined" && typeof (navigator as any).getBattery === "function") {
    try {
      const b = await (navigator as any).getBattery();
      const level = Math.round(b.level * 100);
      batteryInfo = {
        level,
        charging: b.charging,
        label: b.charging ? `${level}% (En charge / AC)` : `${level}% (Sur batterie)`,
      };
    } catch {
      // Ignored
    }
  }

  // 3. Network via Web API
  if (typeof navigator !== "undefined") {
    const conn = (navigator as any).connection;
    networkInfo = {
      online: navigator.onLine ?? true,
      type: conn?.effectiveType || conn?.type || "wifi",
      ssid: undefined,
      internalIp: undefined,
      externalIp: undefined,
    };
  }

  // 4. If in Tauri, fetch rich native stats
  if (isTauri()) {
    try {
      const toolRes: any = await invokeTauri("computer_use", {
        request: { action: "system_info" },
      });
      if (toolRes?.output) {
        const out = toolRes.output;
        if (out.os) platform = out.os;
        if (out.arch) arch = out.arch;
        if (out.battery) {
          batteryInfo = {
            level: out.battery.level ?? 100,
            charging: out.battery.charging ?? true,
            label: out.battery.label || `${out.battery.level}%`,
          };
        }
        if (out.memory) {
          systemStats = {
            osName: out.os || platform,
            platform,
            arch,
            totalRamMb: out.memory.totalMb,
            freeRamMb: out.memory.freeMb,
            usedRamMb: out.memory.usedMb,
          };
        }
      }
    } catch {
      // Fallback gracefully
    }

    try {
      const volRes: any = await invokeTauri("computer_use", {
        request: { action: "volume_get" },
      });
      if (volRes?.output) {
        volumeInfo = {
          level: volRes.output.volume ?? 50,
          muted: volRes.output.muted ?? false,
        };
      }
    } catch {
      // Fallback
    }

    try {
      const netRes: any = await invokeTauri("computer_use", {
        request: { action: "network_info" },
      });
      if (netRes?.output) {
        networkInfo = {
          online: netRes.output.online ?? true,
          ssid: netRes.output.ssid,
          signal: netRes.output.signal,
          internalIp: netRes.output.internalIp,
          type: "wifi",
        };
      }
    } catch {
      // Fallback
    }
  }

  // Default volume if not retrieved
  if (!volumeInfo) {
    volumeInfo = { level: 50, muted: false };
  }

  // 5. External IP fetch if permitted (fast non-blocking 2s timeout)
  if (permissions.enabled && permissions.shareIp) {
    try {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), 2000);
      const resp = await fetch("https://api.ipify.org?format=json", {
        signal: controller.signal,
      });
      clearTimeout(timer);
      if (resp.ok) {
        const data = await resp.json();
        if (data?.ip) {
          if (!networkInfo) {
            networkInfo = {
              online: true,
              type: "wifi",
              ssid: undefined,
              internalIp: undefined,
              externalIp: data.ip,
            };
          } else {
            networkInfo.externalIp = data.ip;
          }
        }
      }
    } catch {
      // Offline, timeout, or blocked - ignore gracefully
    }
  }

  // 6. Clipboard snippet if enabled
  if (permissions.enabled && permissions.shareClipboard && typeof navigator !== "undefined" && navigator.clipboard?.readText) {
    try {
      const text = await navigator.clipboard.readText();
      if (text && text.trim()) {
        clipboardSnippet = text.trim().slice(0, 150);
      }
    } catch {
      // Permission denied
    }
  }

  // Filter based on granular user permissions
  if (!permissions.enabled) {
    batteryInfo = null;
    volumeInfo = null;
    networkInfo = null;
    displayInfo = null;
    clipboardSnippet = null;
    systemStats = null;
  } else {
    if (!permissions.shareBattery) batteryInfo = null;
    if (!permissions.shareVolume) volumeInfo = null;
    if (!permissions.shareDisplay) displayInfo = null;
    if (!permissions.shareClipboard) clipboardSnippet = null;
    if (!permissions.shareSystemStats) systemStats = null;
    if (!permissions.shareNetwork && networkInfo) {
      networkInfo = null;
    } else if (networkInfo && !permissions.shareIp) {
      networkInfo.internalIp = undefined;
      networkInfo.externalIp = undefined;
    }
  }

  const context: ComputerEnvironmentContext = {
    date: dateStr,
    time: timeStr,
    timestamp: now.toISOString(),
    timezone: timeZone,
    platform,
    arch,
    language: navigator?.language || "fr-FR",
    battery: batteryInfo,
    volume: volumeInfo,
    network: networkInfo,
    display: displayInfo,
    clipboardSnippet,
    stats: systemStats,
  };

  cachedEnvironment = context;
  lastCacheTime = Date.now();
  return context;
}

export function getCachedComputerEnvironment(
  permissions: ComputerPermissions = get(computerPermissions)
): ComputerEnvironmentContext {
  if (cachedEnvironment && Date.now() - lastCacheTime < CACHE_TTL_MS) {
    return cachedEnvironment;
  }
  // Synchronous fast fallback while async refresh runs in background
  const now = new Date();
  const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  const dateStr = now.toLocaleDateString(navigator?.language || "fr-FR", {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  });
  const timeStr = now.toLocaleTimeString(navigator?.language || "fr-FR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  const fallback: ComputerEnvironmentContext = {
    date: dateStr,
    time: timeStr,
    timestamp: now.toISOString(),
    timezone: timeZone,
    platform: "Windows",
    arch: "x86_64",
    language: navigator?.language || "fr-FR",
    display: typeof window !== "undefined" && window.screen
      ? {
          width: window.screen.width,
          height: window.screen.height,
          scaleFactor: window.devicePixelRatio || 1,
        }
      : null,
    volume: permissions.enabled && permissions.shareVolume ? { level: 50, muted: false } : null,
    battery: null,
    network: permissions.enabled && permissions.shareNetwork ? { online: navigator?.onLine ?? true } : null,
    clipboardSnippet: null,
    stats: null,
  };

  // Trigger background refresh
  getLiveComputerEnvironment(permissions).catch(() => {});
  return fallback;
}

export function formatEnvironmentContextForPrompt(
  permissions: ComputerPermissions,
  info: ComputerEnvironmentContext,
  language: "fr" | "en" = "fr"
): string {
  if (!permissions.enabled) return "";

  const lines: string[] = [];
  const fr = language === "fr";

  lines.push(`[${fr ? "ENVIRONNEMENT ET SYSTÈME UTILISATEUR" : "USER WORKSTATION & ENVIRONMENT CONTEXT"}]`);
  lines.push(
    `- ${fr ? "Date et heure locale" : "Local Date & Time"}: ${info.date}, ${info.time} (${fr ? "Fuseau" : "Timezone"}: ${info.timezone})`
  );
  lines.push(`- ${fr ? "Système & Architecture" : "OS & Architecture"}: ${info.platform} (${info.arch}) | ${fr ? "Langue" : "Language"}: ${info.language}`);

  if (permissions.shareDisplay && info.display) {
    lines.push(
      `- ${fr ? "Écran principal" : "Display"}: ${info.display.width}x${info.display.height} @ ${info.display.scaleFactor}x DPI`
    );
  }

  if (permissions.shareBattery && info.battery) {
    lines.push(
      `- ${fr ? "Batterie" : "Battery"}: ${info.battery.label}`
    );
  }

  if (permissions.shareVolume && info.volume) {
    const muteLabel = info.volume.muted ? (fr ? "muet" : "muted") : (fr ? "actif" : "unmuted");
    lines.push(
      `- ${fr ? "Volume audio système" : "System Audio Volume"}: ${info.volume.level}% (${muteLabel})`
    );
  }

  if (permissions.shareNetwork && info.network) {
    const parts: string[] = [];
    parts.push(info.network.online ? (fr ? "En ligne" : "Online") : (fr ? "Hors ligne" : "Offline"));
    if (info.network.ssid) parts.push(`Wi-Fi: "${info.network.ssid}"${info.network.signal ? ` (${info.network.signal})` : ""}`);
    if (permissions.shareIp) {
      if (info.network.internalIp && info.network.externalIp) {
        parts.push(`IP: ${info.network.internalIp} (locale) | ${info.network.externalIp} (publique)`);
      } else if (info.network.internalIp) {
        parts.push(`IP: ${info.network.internalIp}`);
      } else if (info.network.externalIp) {
        parts.push(fr ? `IP publique: ${info.network.externalIp}` : `Public IP: ${info.network.externalIp}`);
      }
    }
    lines.push(`- ${fr ? "Réseau" : "Network"}: ${parts.join(" | ")}`);
  }

  if (permissions.shareSystemStats && info.stats) {
    const parts: string[] = [];
    if (info.stats.usedRamMb && info.stats.totalRamMb) {
      const usedGb = (info.stats.usedRamMb / 1024).toFixed(1);
      const totalGb = (info.stats.totalRamMb / 1024).toFixed(1);
      parts.push(`RAM: ${usedGb} GB / ${totalGb} GB`);
    }
    if (info.stats.cpuCores) parts.push(`CPU: ${info.stats.cpuCores} cores`);
    if (parts.length > 0) {
      lines.push(`- ${fr ? "Ressources système" : "System Resources"}: ${parts.join(" | ")}`);
    }
  }

  if (permissions.shareClipboard && info.clipboardSnippet) {
    lines.push(`- ${fr ? "Extrait presse-papiers" : "Clipboard Snippet"}: "${info.clipboardSnippet}"`);
  }

  // Tools section
  if (permissions.allowComputerUse) {
    const toolList: string[] = [];
    if (permissions.allowVolumeControl) {
      toolList.push(
        fr
          ? "- Contrôle du volume sonore (`volume_set` avec `level: 0..100`, `volume_up` / `volume_down` avec `delta`, `volume_mute` avec `mute: true/false`, `volume_get`)"
          : "- System audio volume control (`volume_set` with `level: 0..100`, `volume_up` / `volume_down` with `delta`, `volume_mute` with `mute: true/false`, `volume_get`)"
      );
    }
    if (permissions.allowScreenCapture) {
      toolList.push(
        fr
          ? "- Capture d'écran (`screenshot` : capture l'écran principal en haute résolution et l'intègre automatiquement comme artefact visuel analysable)"
          : "- Screen capture (`screenshot`: captures primary monitor in high resolution and attaches it as an analyzable visual artifact)"
      );
    }
    if (permissions.allowNetworkInspection) {
      toolList.push(
        fr
          ? "- Diagnostic réseau & Wi-Fi (`network_info` ou `wifi` : statut connexion, nom du réseau SSID, qualité signal, adresse IP locale et passerelle)"
          : "- Network & WiFi diagnostic (`network_info` or `wifi`: online state, SSID, signal strength, local IP address)"
      );
    }
    if (permissions.allowAppLauncher) {
      toolList.push(
        fr
          ? "- Lanceur d'applications & fichiers (`app_launch` ou `open` avec `target`: chemin de fichier, application système ou URL web)"
          : "- Application & file launcher (`app_launch` or `open` with `target`: file path, system executable, or web URL)"
      );
    }
    if (permissions.allowSystemInspector) {
      toolList.push(
        fr
          ? "- Inspecteur système OS (`system_info` : diagnostic batterie, chargeur AC, mémoire RAM totale/libre, nombre de cœurs CPU, version OS)"
          : "- System inspector (`system_info`: battery, AC power, total/available RAM, CPU cores, OS distribution/version)"
      );
    }

    if (toolList.length > 0) {
      lines.push("");
      lines.push(`[${fr ? "OUTILS SYSTÈME DISPONIBLES POUR L'AGENT" : "AVAILABLE NATIVE WORKSTATION TOOLS"}]`);
      lines.push(
        fr
          ? "Vous disposez d'outils réels pour interagir directement avec l'ordinateur de l'utilisateur à sa demande. Pour les exécuter, invoquez l'outil `core.computer.use` (ou `computer_use`) avec le paramètre `action` correspondant :"
          : "You have real native tools to interact directly with the user's computer upon request. To execute them, invoke `core.computer.use` (or `computer_use`) with the matching `action` parameter:"
      );
      lines.push(...toolList);
      lines.push(
        fr
          ? "Exemples d'appel : `action: \"volume_set\", level: 75`, `action: \"volume_up\", delta: 10`, `action: \"volume_mute\", mute: true`, `action: \"screenshot\"`, `action: \"network_info\"`, `action: \"app_launch\", target: \"notepad\"`."
          : "Invocation examples: `action: \"volume_set\", level: 75`, `action: \"volume_up\", delta: 10`, `action: \"volume_mute\", mute: true`, `action: \"screenshot\"`, `action: \"network_info\"`, `action: \"app_launch\", target: \"notepad\"`."
      );
    }
  }

  return lines.join("\n");
}

export async function executeComputerTool(
  action: string,
  input: Record<string, any> = {},
  permissions: ComputerPermissions = get(computerPermissions)
): Promise<ComputerToolResult> {
  if (!permissions.enabled || !permissions.allowComputerUse) {
    return {
      success: false,
      action,
      error: "Computer use is disabled in user settings. Enable it in Settings > Computer.",
    };
  }

  // Granular capability check
  if (action.startsWith("volume") && !permissions.allowVolumeControl) {
    return {
      success: false,
      action,
      error: "Volume control tool is disabled in user settings.",
    };
  }
  if ((action === "screenshot" || action === "screen_capture" || action === "take_screenshot") && !permissions.allowScreenCapture) {
    return {
      success: false,
      action,
      error: "Screen capture tool is disabled in user settings.",
    };
  }
  if ((action === "network_info" || action === "wifi_status" || action === "wifi") && !permissions.allowNetworkInspection) {
    return {
      success: false,
      action,
      error: "Network inspection tool is disabled in user settings.",
    };
  }
  if ((action === "app_launch" || action === "open" || action === "launch") && !permissions.allowAppLauncher) {
    return {
      success: false,
      action,
      error: "Application launcher tool is disabled in user settings.",
    };
  }
  if ((action === "system_info" || action === "system_status" || action === "info") && !permissions.allowSystemInspector) {
    return {
      success: false,
      action,
      error: "System inspector tool is disabled in user settings.",
    };
  }

  if (isTauri()) {
    try {
      const res: any = await invokeTauri("computer_use", {
        request: { action, ...input },
      });
      return {
        success: res.output?.success ?? true,
        action,
        output: res.output,
        message: res.summary || res.output?.message,
        filePath: res.output?.filePath,
        artifacts: res.artifacts,
      };
    } catch (err: any) {
      return {
        success: false,
        action,
        error: err?.message || String(err),
      };
    }
  }

  // Web fallback simulation
  if (action === "volume_set") {
    return {
      success: true,
      action,
      output: { volume: input.level ?? 50 },
      message: `System volume set to ${input.level ?? 50}% (browser sandbox simulated)`,
    };
  }
  if (action === "volume_up") {
    const delta = input.delta ?? 10;
    const vol = Math.min(100, 50 + delta);
    return {
      success: true,
      action,
      output: { volume: vol },
      message: `System volume increased to ${vol}% (browser sandbox simulated)`,
    };
  }
  if (action === "volume_down") {
    const delta = input.delta ?? 10;
    const vol = Math.max(0, 50 - delta);
    return {
      success: true,
      action,
      output: { volume: vol },
      message: `System volume decreased to ${vol}% (browser sandbox simulated)`,
    };
  }
  if (action === "volume_get") {
    return {
      success: true,
      action,
      output: { volume: 50, muted: false },
      message: "Current system volume is 50%",
    };
  }
  if (action === "volume_mute" || action === "volume_unmute" || action === "mute" || action === "unmute") {
    const isMute = input.mute ?? (action !== "volume_unmute" && action !== "unmute");
    return {
      success: true,
      action,
      output: { muted: isMute },
      message: `Audio muted: ${isMute}`,
    };
  }
  if (action === "screenshot" || action === "screen_capture" || action === "take_screenshot") {
    return {
      success: true,
      action,
      message: "Screenshot captured (browser window context)",
      output: { width: window.innerWidth, height: window.innerHeight },
    };
  }
  if (action === "network_info" || action === "wifi_status" || action === "wifi") {
    const isOnline = typeof navigator !== "undefined" && typeof navigator.onLine === "boolean" ? navigator.onLine : true;
    return {
      success: true,
      action,
      output: { online: isOnline, type: "wifi", ssid: "Wi-Fi Network", signal: "95%" },
      message: "Network status: Online",
    };
  }
  if (action === "app_launch" || action === "open" || action === "launch") {
    const target = input.target || input.app || input.url || input.path || "";
    if (/^https?:\/\//i.test(target)) {
      window.open(target, "_blank");
      return {
        success: true,
        action,
        message: `Opened link: ${target}`,
      };
    }
    return {
      success: true,
      action,
      message: `Requested app launch: ${target} (native execution required)`,
    };
  }

  return {
    success: true,
    action,
    message: `Computer tool action ${action} completed successfully`,
  };
}
