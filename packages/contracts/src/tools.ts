export interface ToolParameterDescriptor {
  name: string;
  type: "string" | "number" | "boolean" | "object" | "array";
  description: string;
  required: boolean;
}

export type ToolCategoryName = "workspace" | "artifact" | "core" | "system";

export interface ToolContractDescriptor {
  name: string;
  category: ToolCategoryName;
  description: string;
  parameters: ToolParameterDescriptor[];
  requiredPermissions: string[];
}

export type ToolDescriptor = ToolContractDescriptor;

export const DEFAULT_WORKSPACE_TOOLS: ToolContractDescriptor[] = [
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
];
