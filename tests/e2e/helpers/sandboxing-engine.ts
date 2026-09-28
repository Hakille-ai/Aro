export interface SandboxExecutionOptions {
  timeoutMs?: number;
  maxOutputBytes?: number;
  env?: Record<string, string>;
  cwd?: string;
}

export interface SandboxExecutionResult {
  exitCode: number;
  stdout: string;
  stderr: string;
  timedOut: boolean;
  truncated: boolean;
  durationMs: number;
}

const SENSITIVE_KEY_PATTERNS = [
  /KEY$/i,
  /SECRET/i,
  /TOKEN/i,
  /PASSWORD/i,
  /PASSWD/i,
  /CREDENTIAL/i,
  /AUTH/i,
  /PRIVATE/i,
  /API[-_]?KEY/i,
  /DATABASE_URL/i,
  /DB_PASS/i,
  /SESSION/i,
  /SIGNING/i,
  /CERT/i,
];

const PRESERVED_SAFE_KEYS = new Set([
  "PATH",
  "LANG",
  "LC_ALL",
  "HOME",
  "USER",
  "SHELL",
  "TERM",
  "PWD",
  "TMP",
  "TEMP",
  "NODE_ENV",
  "SYSTEMROOT",
  "WINDIR",
  "COMSPEC",
  "PATHEXT",
]);

/**
 * Strips sensitive environment variables from an env object.
 */
export function scrubbedEnv(env: Record<string, string>): Record<string, string> {
  const result: Record<string, string> = {};

  for (const [key, value] of Object.entries(env)) {
    const upperKey = key.toUpperCase();
    if (PRESERVED_SAFE_KEYS.has(upperKey)) {
      result[key] = value;
      continue;
    }

    const isSensitive = SENSITIVE_KEY_PATTERNS.some((pattern) => pattern.test(key));
    if (!isSensitive) {
      result[key] = value;
    }
  }

  return result;
}

/**
 * Sandboxed command execution simulator respecting timeout, output buffer, and scrubbed env.
 */
export class SandboxedExecutionEngine {
  private defaultTimeoutMs: number;
  private defaultMaxOutputBytes: number;

  constructor(defaultTimeoutMs = 5000, defaultMaxOutputBytes = 1024 * 1024) {
    this.defaultTimeoutMs = defaultTimeoutMs;
    this.defaultMaxOutputBytes = defaultMaxOutputBytes;
  }

  public prepareEnvironment(customEnv?: Record<string, string>): Record<string, string> {
    const raw = customEnv || process.env as Record<string, string>;
    return scrubbedEnv(raw);
  }

  public async executeCommand(
    command: string,
    args: string[] = [],
    options?: SandboxExecutionOptions
  ): Promise<SandboxExecutionResult> {
    const timeoutMs = options?.timeoutMs ?? this.defaultTimeoutMs;
    const maxOutputBytes = options?.maxOutputBytes ?? this.defaultMaxOutputBytes;
    const env = this.prepareEnvironment(options?.env);

    if (timeoutMs <= 0) {
      throw new Error("Invalid timeout: timeoutMs must be greater than 0");
    }

    if (!command || command.trim().length === 0) {
      return {
        exitCode: 1,
        stdout: "",
        stderr: "Error: Empty command specified",
        timedOut: false,
        truncated: false,
        durationMs: 0,
      };
    }

    // Simulation of simulated commands or testing mocks
    const startTime = Date.now();

    // Check simulated mock execution
    if (command.includes("sleep") || command.includes("hang")) {
      const sleepTime = parseInt(args[0] || "10", 10) * 1000;
      if (sleepTime > timeoutMs) {
        return {
          exitCode: 124,
          stdout: "",
          stderr: `Execution timed out after ${timeoutMs}ms`,
          timedOut: true,
          truncated: false,
          durationMs: timeoutMs,
        };
      }
    }

    if (command === "echo_secret") {
      // Should not print secret because env is scrubbed
      const secretVal = env["MY_SECRET_TOKEN"] || env["OPENAI_API_KEY"] || "<scrubbed>";
      return {
        exitCode: 0,
        stdout: secretVal,
        stderr: "",
        timedOut: false,
        truncated: false,
        durationMs: Date.now() - startTime,
      };
    }

    if (command === "flood_output") {
      const massiveOutput = "A".repeat(maxOutputBytes + 5000);
      const truncatedOutput = massiveOutput.slice(0, maxOutputBytes);
      return {
        exitCode: 0,
        stdout: truncatedOutput,
        stderr: "[WARN: Output truncated to max buffer size]",
        timedOut: false,
        truncated: true,
        durationMs: Date.now() - startTime,
      };
    }

    if (command === "fail_with_code") {
      const code = parseInt(args[0] || "42", 10);
      return {
        exitCode: code,
        stdout: "",
        stderr: `Process exited with error code ${code}`,
        timedOut: false,
        truncated: false,
        durationMs: Date.now() - startTime,
      };
    }

    return {
      exitCode: 0,
      stdout: `Executed: ${command} ${args.join(" ")}`,
      stderr: "",
      timedOut: false,
      truncated: false,
      durationMs: Date.now() - startTime,
    };
  }
}
