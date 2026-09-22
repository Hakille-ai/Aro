import { describe, expect, it } from "vitest";
import type { Conversation, Folder, Project } from "../../lib/types";
import {
  computeProjectStats,
  filterConversationsForProject,
  getUnassignedConversations,
  getProjectOrganizationId,
  getFolderOrganizationId,
  filterProjectsForScope,
  filterFoldersForScope,
  getConversationOrganizationId,
  filterConversationsForScope,
} from "./model";

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

  describe("Workspace scoping and isolation", () => {
    const personalProject: Project = {
      id: "p-pers",
      name: "Personal Project",
      color: "#3b82f6",
      icon: "folder",
      createdAt: "2026-07-23T00:00:00Z",
      updatedAt: "2026-07-23T00:00:00Z",
      organizationId: null,
    };

    const orgProject: Project = {
      id: "p-org",
      name: "Org Project",
      color: "#10b981",
      icon: "folder",
      createdAt: "2026-07-23T00:00:00Z",
      updatedAt: "2026-07-23T00:00:00Z",
      organizationId: "org-alpha",
    };

    const otherOrgProject: Project = {
      id: "p-other-org",
      name: "Other Org Project",
      color: "#ef4444",
      icon: "folder",
      createdAt: "2026-07-23T00:00:00Z",
      updatedAt: "2026-07-23T00:00:00Z",
      organizationId: "org-beta",
    };

    const legacyProject: Project = {
      id: "p-legacy",
      name: "Legacy Project",
      color: "#8b5cf6",
      icon: "folder",
      createdAt: "2026-07-23T00:00:00Z",
      updatedAt: "2026-07-23T00:00:00Z",
    };

    const legacyFolder: Folder = {
      id: "f-legacy",
      projectId: "p-legacy",
      name: "Legacy Folder",
      createdAt: "2026-07-23T00:00:00Z",
      updatedAt: "2026-07-23T00:00:00Z",
    };

    const orgConvInLegacy: Conversation = {
      id: "c-legacy-org",
      title: "Conv In Legacy",
      mode: "chat",
      projectId: "p-legacy",
      organizationId: "org-alpha",
      createdAt: "2026-07-23T00:00:00Z",
      updatedAt: "2026-07-23T00:00:00Z",
    };

    const personalFolder: Folder = {
      id: "f-pers",
      name: "Personal Folder",
      createdAt: "2026-07-23T00:00:00Z",
      updatedAt: "2026-07-23T00:00:00Z",
      organizationId: null,
    };

    const orgFolder: Folder = {
      id: "f-org",
      name: "Org Folder",
      createdAt: "2026-07-23T00:00:00Z",
      updatedAt: "2026-07-23T00:00:00Z",
      organizationId: "org-alpha",
    };

    it("gets explicit organizationId for projects and folders", () => {
      expect(getProjectOrganizationId(orgProject)).toBe("org-alpha");
      expect(getProjectOrganizationId(personalProject)).toBeNull();
      expect(getFolderOrganizationId(orgFolder)).toBe("org-alpha");
      expect(getFolderOrganizationId(personalFolder)).toBeNull();
    });

    it("infers organizationId for legacy project without explicit tag from contained conversations", () => {
      // Legacy project with an org-alpha conversation is inferred as org-alpha
      expect(getProjectOrganizationId(legacyProject, [orgConvInLegacy])).toBe("org-alpha");
      // Legacy project with no org conversation defaults to null (personal)
      expect(getProjectOrganizationId(legacyProject, [])).toBeNull();
    });

    it("infers organizationId for folder from parent project or conversation", () => {
      const childFolderOfOrgProj: Folder = {
        id: "f-child",
        projectId: "p-org",
        name: "Child of Org",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      expect(getFolderOrganizationId(childFolderOfOrgProj, [orgProject])).toBe("org-alpha");

      const standaloneFolderWithOrgConv: Folder = {
        id: "f-conv-org",
        name: "Folder with Org Conv",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      const convInFolder: Conversation = {
        id: "c-in-folder",
        title: "Conv",
        mode: "chat",
        folderId: "f-conv-org",
        organizationId: "org-alpha",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      expect(getFolderOrganizationId(standaloneFolderWithOrgConv, [], [convInFolder])).toBe("org-alpha");
    });

    it("filters projects strictly for personal space", () => {
      const allProjects = [personalProject, orgProject, otherOrgProject, legacyProject];
      // legacyProject with no org conversation belongs to personal
      const personalFiltered = filterProjectsForScope(allProjects, "personal", null, []);
      expect(personalFiltered.map((p) => p.id)).toEqual(["p-pers", "p-legacy"]);
    });

    it("filters projects strictly for organization space", () => {
      const allProjects = [personalProject, orgProject, otherOrgProject, legacyProject];
      // In org-alpha, with legacyProject containing org-alpha conversation
      const alphaFiltered = filterProjectsForScope(allProjects, "organization", "org-alpha", [orgConvInLegacy]);
      expect(alphaFiltered.map((p) => p.id)).toEqual(["p-org", "p-legacy"]);

      // In org-beta, only otherOrgProject is displayed
      const betaFiltered = filterProjectsForScope(allProjects, "organization", "org-beta", [orgConvInLegacy]);
      expect(betaFiltered.map((p) => p.id)).toEqual(["p-other-org"]);
    });

    it("filters folders strictly for personal vs organization scope", () => {
      const allFolders = [personalFolder, orgFolder, legacyFolder];
      const personalFiltered = filterFoldersForScope(allFolders, "personal", null, [personalProject], []);
      expect(personalFiltered.map((f) => f.id)).toEqual(["f-pers", "f-legacy"]);

      const orgFiltered = filterFoldersForScope(allFolders, "organization", "org-alpha", [personalProject], []);
      expect(orgFiltered.map((f) => f.id)).toEqual(["f-org"]);
    });

    it("infers organizationId for conversations from explicit tag, project, or folder", () => {
      const explicitOrgConv: Conversation = {
        id: "c-exp",
        title: "Explicit",
        mode: "chat",
        organizationId: "org-alpha",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      expect(getConversationOrganizationId(explicitOrgConv, [orgProject], [orgFolder])).toBe("org-alpha");

      const convInOrgProjWithoutOrgTag: Conversation = {
        id: "c-in-proj",
        title: "In Proj",
        mode: "chat",
        projectId: "p-org",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      expect(getConversationOrganizationId(convInOrgProjWithoutOrgTag, [orgProject], [orgFolder])).toBe("org-alpha");

      const convInOrgFolderWithoutOrgTag: Conversation = {
        id: "c-in-folder",
        title: "In Folder",
        mode: "chat",
        folderId: "f-org",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      expect(getConversationOrganizationId(convInOrgFolderWithoutOrgTag, [orgProject], [orgFolder])).toBe("org-alpha");

      const unassignedPersonalConv: Conversation = {
        id: "c-personal",
        title: "Unassigned Personal",
        mode: "chat",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      expect(getConversationOrganizationId(unassignedPersonalConv, [orgProject], [orgFolder])).toBeNull();
    });

    it("filters conversations strictly for personal vs organization scope", () => {
      const convExplicitOrg: Conversation = {
        id: "c1",
        title: "Explicit Org",
        mode: "chat",
        organizationId: "org-alpha",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      const convInOrgProj: Conversation = {
        id: "c2",
        title: "In Org Proj",
        mode: "chat",
        projectId: "p-org",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      const convInOrgFolder: Conversation = {
        id: "c3",
        title: "In Org Folder",
        mode: "chat",
        folderId: "f-org",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      const convInPersProj: Conversation = {
        id: "c4",
        title: "In Personal Proj",
        mode: "chat",
        projectId: "p-pers",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };
      const convPersonal: Conversation = {
        id: "c5",
        title: "Unassigned Personal",
        mode: "chat",
        createdAt: "2026-07-23T00:00:00Z",
        updatedAt: "2026-07-23T00:00:00Z",
      };

      const allConvs = [convExplicitOrg, convInOrgProj, convInOrgFolder, convInPersProj, convPersonal];
      const projs = [personalProject, orgProject];
      const flds = [personalFolder, orgFolder];

      const personalFiltered = filterConversationsForScope(allConvs, "personal", null, projs, flds);
      expect(personalFiltered.map((c) => c.id)).toEqual(["c4", "c5"]);

      const orgFiltered = filterConversationsForScope(allConvs, "organization", "org-alpha", projs, flds);
      expect(orgFiltered.map((c) => c.id)).toEqual(["c1", "c2", "c3"]);

      const otherOrgFiltered = filterConversationsForScope(allConvs, "organization", "org-beta", projs, flds);
      expect(otherOrgFiltered).toHaveLength(0);
    });
  });
});
