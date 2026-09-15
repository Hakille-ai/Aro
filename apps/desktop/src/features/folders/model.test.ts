import { describe, expect, it } from "vitest";
import type { Conversation, Folder } from "../../lib/types";
import { buildFolderTree, countConversationsInFolder } from "./model";

describe("Folders Model", () => {
  const folder1: Folder = {
    id: "f1",
    projectId: "p1",
    name: "Docs",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  const folder2: Folder = {
    id: "f2",
    projectId: null,
    name: "General Notes",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  const conv1: Conversation = {
    id: "c1",
    title: "Doc Chat 1",
    mode: "chat",
    folderId: "f1",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  const conv2: Conversation = {
    id: "c2",
    title: "Doc Chat 2",
    mode: "chat",
    folderId: "f1",
    createdAt: "2026-07-23T00:00:00Z",
    updatedAt: "2026-07-23T00:00:00Z",
  };

  it("builds folder tree and associates conversations", () => {
    const tree = buildFolderTree([folder1, folder2], [conv1, conv2], "p1");
    expect(tree).toHaveLength(1);
    expect(tree[0].folder.name).toBe("Docs");
    expect(tree[0].conversations).toHaveLength(2);
  });

  it("counts conversations in folder correctly", () => {
    expect(countConversationsInFolder("f1", [conv1, conv2])).toBe(2);
    expect(countConversationsInFolder("f2", [conv1, conv2])).toBe(0);
  });
});
