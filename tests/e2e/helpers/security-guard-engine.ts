import {
  type PermissionPresetMode,
  type AgentPermissionProfile,
} from "@aro/contracts";

export class ToolAuthorizationError extends Error {
  public toolName: string;
  public reason: string;
  public preset: PermissionPresetMode;

  constructor(toolName: string, reason: string, preset: PermissionPresetMode) {
    super(`Tool Authorization Denied [${toolName}] under preset '${preset}': ${reason}`);
    this.name = "ToolAuthorizationError";
    this.toolName = toolName;
    this.reason = reason;
    this.preset = preset;
  }
}

export type ToolCategory = "read" | "write" | "shell" | "network" | "other";

export function classifyTool(toolName: string): ToolCategory {
  const lower = toolName.trim().toLowerCase();

  // Check shell first because some shell tools might contain words like "run" or "exec"
  if (
    lower.includes("shell") ||
    lower.includes("bash") ||
    lower.includes("terminal") ||
    lower.includes("cmd") ||
    lower.includes("exec") ||
    lower.includes("run_command") ||
    lower.includes("core.shell.execute") ||
    lower.includes("core.code.execute")
  ) {
    return "shell";
  }

  // Network tools
  if (
    lower.includes("web") ||
    lower.includes("http") ||
    lower.includes("download") ||
    lower.includes("curl") ||
    lower.includes("browser") ||
    lower.includes("external.fetch")
  ) {
    return "network";
  }

  // File write / mutation tools
  if (
    lower.includes("write") ||
    lower.includes("edit") ||
    lower.includes("replace") ||
    lower.includes("patch") ||
    lower.includes("create") ||
    lower.includes("delete") ||
    lower.includes("remove") ||
    lower.includes("save") ||
    lower.includes("workspace.write") ||
    lower.includes("workspace.delete") ||
    lower.includes("workspace.replace_in_files") ||
    lower.includes("artifact.create") ||
    lower.includes("artifact.update")
  ) {
    return "write";
  }

  // File read / inspection tools
  if (
    lower.includes("read") ||
    lower.includes("view") ||
    lower.includes("search") ||
    lower.includes("list") ||
    lower.includes("inspect") ||
    lower.includes("workspace.read") ||
    lower.includes("workspace.list_dir") ||
    lower.includes("workspace.git_diff") ||
    lower.includes("artifact.list")
  ) {
    return "read";
  }

  return "other";
}

export class ToolAuthorizationGuard {
  public preset: PermissionPresetMode;
  public profile?: AgentPermissionProfile | null;
  public allowedTools?: Set<string>;
  public deniedTools?: Set<string>;

  constructor(options?: {
    preset?: PermissionPresetMode;
    profile?: AgentPermissionProfile | null;
    allowedTools?: string[];
    deniedTools?: string[];
  }) {
    this.preset = options?.preset ?? "standard";
    this.profile = options?.profile ?? null;
    if (options?.allowedTools) {
      this.allowedTools = new Set(options.allowedTools.map((t) => t.toLowerCase()));
    }
    if (options?.deniedTools) {
      this.deniedTools = new Set(options.deniedTools.map((t) => t.toLowerCase()));
    }
  }

  public checkPermission(toolName: string): { allowed: boolean; reason?: string } {
    if (!toolName || typeof toolName !== "string" || toolName.trim().length === 0) {
      throw new ToolAuthorizationError(toolName || "<empty>", "Tool name cannot be empty", this.preset);
    }

    const normalizedTool = toolName.trim().toLowerCase();

    // 1. Explicit denied tools list takes highest priority
    if (this.deniedTools && this.deniedTools.has(normalizedTool)) {
      throw new ToolAuthorizationError(toolName, `Tool explicitly blacklisted in denied_tools`, this.preset);
    }

    // 2. Explicit allowed tools list (if set) acts as a strict whitelist
    if (this.allowedTools && !this.allowedTools.has(normalizedTool)) {
      throw new ToolAuthorizationError(toolName, `Tool not found in explicit allowed_tools whitelist`, this.preset);
    }

    const category = classifyTool(normalizedTool);

    // 3. Preset checks
    switch (this.preset) {
      case "sandbox": {
        if (category === "shell") {
          throw new ToolAuthorizationError(toolName, "Command execution is forbidden in Sandbox mode", this.preset);
        }
        if (category === "write") {
          throw new ToolAuthorizationError(toolName, "File write and mutation operations are forbidden in Sandbox mode", this.preset);
        }
        if (category === "network") {
          throw new ToolAuthorizationError(toolName, "External network access is forbidden in Sandbox mode", this.preset);
        }
        if (category === "read" && !normalizedTool.includes("context")) {
          throw new ToolAuthorizationError(toolName, "Local filesystem access is forbidden in Sandbox mode", this.preset);
        }
        return { allowed: true };
      }

      case "read-only": {
        if (category === "shell") {
          throw new ToolAuthorizationError(toolName, "Command execution is forbidden in Read-Only mode", this.preset);
        }
        if (category === "write") {
          throw new ToolAuthorizationError(toolName, "File write and mutation operations are forbidden in Read-Only mode", this.preset);
        }
        if (category === "network") {
          throw new ToolAuthorizationError(toolName, "External network access is forbidden in Read-Only mode", this.preset);
        }
        if (category === "read" || category === "other") {
          return { allowed: true };
        }
        return { allowed: true };
      }

      case "developer": {
        // Full access to read, write, shell (auto-approved), and network
        return { allowed: true };
      }

      case "custom": {
        if (!this.profile) {
          throw new ToolAuthorizationError(toolName, "Custom preset requires an active AgentPermissionProfile", this.preset);
        }
        if (category === "read" && !this.profile.allowRead) {
          throw new ToolAuthorizationError(toolName, `File reads prohibited by profile "${this.profile.name}"`, this.preset);
        }
        if (category === "write" && !this.profile.allowWrite) {
          throw new ToolAuthorizationError(toolName, `File writes prohibited by profile "${this.profile.name}"`, this.preset);
        }
        if (category === "shell" && !this.profile.allowShell) {
          throw new ToolAuthorizationError(toolName, `Shell execution prohibited by profile "${this.profile.name}"`, this.preset);
        }
        if (category === "network" && !this.profile.allowNetwork) {
          throw new ToolAuthorizationError(toolName, `Network access prohibited by profile "${this.profile.name}"`, this.preset);
        }
        return { allowed: true };
      }

      case "standard":
      default: {
        // Standard preset: read allowed, write allowed, shell requires user approval, network allowed for docs
        if (category === "shell") {
          return { allowed: true, reason: "Requires user confirmation before execution" };
        }
        return { allowed: true };
      }
    }
  }
}
