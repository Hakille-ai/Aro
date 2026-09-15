import { describe, expect, it } from "vitest";
import type { Conversation, Folder, Project } from "../../lib/types";
import { computeProjectStats, filterConversationsForProject, getUnassignedConversations } from "./model";

describe("Projects Model", () => {
  const project1: Project = {
    id: "proj-1",
    name: "Project One",
    description: "First test project",
    color: "#3b82f6",
    icon: "folder-git2",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  const folder1: Folder = {
    id: "fold-1",
    projectId: "proj-1",
    name: "Subfolder 1",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  const conv1: Conversation = {
    id: "c1",
    title: "Project Chat",
    mode: "chat",
    projectId: "proj-1",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  const conv2: Conversation = {
    id: "c2",
    title: "Folder Chat",
    mode: "chat",
    folderId: "fold-1",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  const conv3: Conversation = {
    id: "c3",
    title: "Unassigned Chat",
    mode: "chat",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  it("computes stats correctly including project conversations and folder conversations", () => {
    const stats = computeProjectStats([project1], [folder1], [conv1, conv2, conv3]);
    expect(stats).toHaveLength(1);
    expect(stats[0].conversationCount).toBe(2);
    expect(stats[0].folderCount).toBe(1);
  });

  it("filters conversations for a specific project", () => {
    const filtered = filterConversationsForProject([conv1, conv2, conv3], "proj-1", [folder1]);
    expect(filtered).toHaveLength(2);
    expect(filtered.map((c) => c.id)).toEqual(["c1", "c2"]);
  });

  it("returns unassigned conversations", () => {
    const unassigned = getUnassignedConversations([conv1, conv2, conv3], [folder1]);
    expect(unassigned).toHaveLength(1);
    expect(unassigned[0].id).toBe("c3");
  });
});
