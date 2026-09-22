// Types for Computer & OS Integration in ARO

export interface ComputerPermissions {
  /** Master switch to enable/disable OS & hardware integration */
  enabled: boolean;
  /** Allow AI agents to control the workstation (apps, volume, screen) */
  allowComputerUse: boolean;
  /** Require explicit user confirmation before sensitive or mutating system actions */
  confirmDangerousActions: boolean;

  // Environment context sharing permissions (injected in system prompts)
  shareBattery: boolean;
  shareIp: boolean;
  shareVolume: boolean;
  shareNetwork: boolean;
  shareDisplay: boolean;
  shareClipboard: boolean;
  shareSystemStats: boolean;

  // Granular agent tool permissions
  allowVolumeControl: boolean;
  allowScreenCapture: boolean;
  allowNetworkInspection: boolean;
  allowAppLauncher: boolean;
  allowSystemInspector: boolean;
}

export interface ComputerBatteryInfo {
  level: number; // 0 to 100
  charging: boolean;
  label: string;
}

export interface ComputerVolumeInfo {
  level: number; // 0 to 100
  muted: boolean;
}

export interface ComputerNetworkInfo {
  online: boolean;
  type?: string;
  ssid?: string;
  signal?: string;
  internalIp?: string;
  externalIp?: string;
}

export interface ComputerDisplayInfo {
  width: number;
  height: number;
  scaleFactor: number;
  colorDepth?: number;
}

export interface ComputerSystemStats {
  osName: string;
  platform: string;
  arch: string;
  cpuName?: string;
  cpuCores?: number;
  totalRamMb?: number;
  freeRamMb?: number;
  usedRamMb?: number;
  uptimeSeconds?: number;
}

export interface ComputerEnvironmentContext {
  date: string;
  time: string;
  timestamp: string;
  timezone: string;
  platform: string;
  arch: string;
  language: string;
  battery?: ComputerBatteryInfo | null;
  volume?: ComputerVolumeInfo | null;
  network?: ComputerNetworkInfo | null;
  display?: ComputerDisplayInfo | null;
  clipboardSnippet?: string | null;
  stats?: ComputerSystemStats | null;
}

export interface ComputerToolResult {
  success: boolean;
  action: string;
  output?: any;
  message?: string;
  error?: string;
  filePath?: string;
  artifacts?: any[];
}
