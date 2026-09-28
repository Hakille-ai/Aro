import * as path from "node:path";

export class WorkspaceConfinementError extends Error {
  public code: "MISSING_ROOT" | "PATH_ESCAPE" | "INVALID_INPUT" | "NULL_BYTE";

  constructor(message: string, code: "MISSING_ROOT" | "PATH_ESCAPE" | "INVALID_INPUT" | "NULL_BYTE") {
    super(`Workspace Confinement Violation: ${message}`);
    this.name = "WorkspaceConfinementError";
    this.code = code;
  }
}

export function normalizePathSeparators(p: string): string {
  return p.replace(/\\/g, "/");
}

/**
 * Resolves an input path against a workspace root and enforces strict containment.
 * Fails closed if root_path is empty or missing.
 * Fails closed if input_path escapes workspace root via traversal, drive hopping, or null bytes.
 */
export function resolveWorkspacePath(rootPath: string, inputPath: string): string {
  if (!rootPath || typeof rootPath !== "string" || rootPath.trim().length === 0) {
    throw new WorkspaceConfinementError("Missing or empty workspace root path", "MISSING_ROOT");
  }

  if (inputPath === null || inputPath === undefined || typeof inputPath !== "string") {
    throw new WorkspaceConfinementError("Invalid input path", "INVALID_INPUT");
  }

  // Check for null bytes
  if (inputPath.includes("\0") || rootPath.includes("\0")) {
    throw new WorkspaceConfinementError("Null byte detected in path", "NULL_BYTE");
  }

  // Normalize root path to absolute
  const resolvedRoot = path.resolve(rootPath);
  const normalizedRoot = normalizePathSeparators(resolvedRoot).toLowerCase().replace(/\/+$/, "");

  let candidate = inputPath;
  try {
    const decoded = decodeURIComponent(inputPath);
    if (decoded !== inputPath) {
      candidate = decoded;
    }
  } catch {
    // Malformed URI encoding
  }

  // If candidate is empty string, return normalizedRoot
  if (candidate.trim().length === 0) {
    return resolvedRoot;
  }

  // Resolve input path relative to rootPath
  const resolvedTarget = path.isAbsolute(candidate)
    ? path.resolve(candidate)
    : path.resolve(resolvedRoot, candidate);

  const normalizedTarget = normalizePathSeparators(resolvedTarget).toLowerCase();

  // Strict confinement check: target must start with normalizedRoot
  // Note: must ensure it is within directory, not just sharing prefix (e.g. /app vs /app-secret)
  const isInside =
    normalizedTarget === normalizedRoot ||
    normalizedTarget.startsWith(normalizedRoot + "/");

  if (!isInside) {
    throw new WorkspaceConfinementError(
      `Path '${inputPath}' escapes workspace root '${rootPath}'`,
      "PATH_ESCAPE"
    );
  }

  return resolvedTarget;
}
