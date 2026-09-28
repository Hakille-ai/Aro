export interface ToolParameterDescriptor {
  name: string;
  type: "string" | "number" | "boolean" | "object" | "array";
  description: string;
  required: boolean;
}

export interface ToolDescriptor {
  name: string;
  category: "workspace" | "artifact" | "core" | "system";
  description: string;
  parameters: ToolParameterDescriptor[];
  requiredPermissions: string[];
}

export class ToolRegistryEngine {
  private tools = new Map<string, ToolDescriptor>();

  constructor() {
    this.registerDefaultCatalogue();
  }

  public registerTool(tool: ToolDescriptor, allowOverride = false): void {
    if (!tool.name || typeof tool.name !== "string" || tool.name.trim().length === 0) {
      throw new Error("Tool name cannot be empty");
    }

    if (!/^[a-zA-Z0-9_.-]+$/.test(tool.name)) {
      throw new Error(`Invalid tool name format: '${tool.name}'`);
    }

    if (!tool.description) {
      throw new Error(`Tool '${tool.name}' requires a description`);
    }

    const key = tool.name.toLowerCase();
    if (this.tools.has(key) && !allowOverride) {
      throw new Error(`Tool '${tool.name}' is already registered in ToolRegistry`);
    }

    this.tools.set(key, tool);
  }

  public getTool(name: string): ToolDescriptor | undefined {
    return this.tools.get(name.toLowerCase());
  }

  public hasTool(name: string): boolean {
    return this.tools.has(name.toLowerCase());
  }

  public listTools(category?: string): ToolDescriptor[] {
    const all = Array.from(this.tools.values());
    if (!category) return all;
    return all.filter((t) => t.category === category);
  }

  public validateParameters(toolName: string, params: Record<string, unknown>): { valid: boolean; errors: string[] } {
    const tool = this.getTool(toolName);
    if (!tool) {
      return { valid: false, errors: [`Tool '${toolName}' not found in registry`] };
    }

    const errors: string[] = [];
    for (const p of tool.parameters) {
      if (p.required && (params[p.name] === undefined || params[p.name] === null)) {
        errors.push(`Missing required parameter: '${p.name}'`);
      }
    }

    return { valid: errors.length === 0, errors };
  }

  private registerDefaultCatalogue(): void {
    const defaultTools: ToolDescriptor[] = [
      {
        name: "workspace.read",
        category: "workspace",
        description: "Read the contents of a file within the workspace",
        parameters: [{ name: "path", type: "string", description: "Relative file path", required: true }],
        requiredPermissions: ["file:read"],
      },
      {
        name: "workspace.write",
        category: "workspace",
        description: "Write content to a file within the workspace",
        parameters: [
          { name: "path", type: "string", description: "Relative file path", required: true },
          { name: "content", type: "string", description: "File content", required: true },
        ],
        requiredPermissions: ["file:write"],
      },
      {
        name: "workspace.delete",
        category: "workspace",
        description: "Delete a file or directory within the workspace",
        parameters: [{ name: "path", type: "string", description: "Relative file path", required: true }],
        requiredPermissions: ["file:write"],
      },
      {
        name: "workspace.replace_in_files",
        category: "workspace",
        description: "Find and replace text patterns across files in the workspace",
        parameters: [
          { name: "search", type: "string", description: "Search pattern", required: true },
          { name: "replace", type: "string", description: "Replacement text", required: true },
        ],
        requiredPermissions: ["file:write"],
      },
      {
        name: "workspace.git_diff",
        category: "workspace",
        description: "Compute git diff patch between current state and repository HEAD",
        parameters: [{ name: "path", type: "string", description: "Optional path filter", required: false }],
        requiredPermissions: ["file:read"],
      },
      {
        name: "workspace.list_dir",
        category: "workspace",
        description: "List directory contents in the workspace",
        parameters: [{ name: "path", type: "string", description: "Relative directory path", required: false }],
        requiredPermissions: ["file:read"],
      },
      {
        name: "artifact.create",
        category: "artifact",
        description: "Create an immutable output artifact reference",
        parameters: [
          { name: "id", type: "string", description: "Artifact identifier", required: true },
          { name: "title", type: "string", description: "Artifact title", required: true },
          { name: "kind", type: "string", description: "Artifact kind (markdown, code, diff, etc.)", required: true },
        ],
        requiredPermissions: ["artifact:create"],
      },
      {
        name: "artifact.update",
        category: "artifact",
        description: "Update an existing artifact metadata or URI",
        parameters: [{ name: "id", type: "string", description: "Artifact identifier", required: true }],
        requiredPermissions: ["artifact:write"],
      },
      {
        name: "artifact.list",
        category: "artifact",
        description: "List artifacts produced during the agent run",
        parameters: [],
        requiredPermissions: ["artifact:read"],
      },
      {
        name: "core.code.execute",
        category: "core",
        description: "Execute code in an isolated sandbox",
        parameters: [
          { name: "language", type: "string", description: "Programming language", required: true },
          { name: "code", type: "string", description: "Source code to execute", required: true },
        ],
        requiredPermissions: ["code:execute"],
      },
      {
        name: "core.shell.execute",
        category: "core",
        description: "Execute a shell command with scrubbed environment",
        parameters: [
          { name: "command", type: "string", description: "Command string", required: true },
          { name: "args", type: "array", description: "Command arguments", required: false },
        ],
        requiredPermissions: ["shell:execute"],
      },
      {
        name: "external.fetch",
        category: "core",
        description: "Fetch external HTTP resources or documentation",
        parameters: [{ name: "url", type: "string", description: "Target URL", required: true }],
        requiredPermissions: ["network:fetch"],
      },
    ];

    for (const t of defaultTools) {
      this.registerTool(t, true);
    }
  }
}
