import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent, screen, waitFor } from "@testing-library/svelte";
import PluginDetailView from "../features/settings/pages/PluginDetailView.svelte";
import PluginsSettings from "../features/settings/pages/PluginsSettings.svelte";
import * as api from "../lib/api";

describe("PluginDetailView Component", () => {
  const mockInstalledPlugin: api.InstalledPlugin = {
    id: "filesystem-tools",
    name: "Filesystem & Workspace",
    version: "1.0.0",
    description: "Standard filesystem operations, tree inspection, and search via Model Context Protocol.",
    author: "ARO Ecosystem",
    rootPath: "C:\\Users\\ARO\\plugins\\filesystem-tools",
    dataPath: "C:\\Users\\ARO\\data\\filesystem-tools",
    enabled: true,
    isSystem: false,
    status: "active",
    installedAt: "2026-09-12T10:00:00Z",
    source: "marketplace",
    keywords: ["fs", "workspace", "files"],
    license: "MIT",
    mcpServers: [
      {
        name: "local-fs",
        transportType: "stdio",
        command: "npx -y @agentplugins/local-fs",
        status: "ready",
      },
    ],
    skills: [
      {
        id: "workspace-explorer",
        name: "Workspace Explorer",
        description: "Explores directories and lists active project trees.",
        tags: ["fs", "search"],
        hasScripts: true,
      },
      {
        id: "file-finder",
        name: "File Finder",
        description: "Finds files matching patterns with smart excludes.",
        tags: ["finder", "glob"],
        hasScripts: true,
      },
    ],
  };

  const mockMarketplacePlugin: api.MarketplacePlugin = {
    id: "filesystem-tools",
    name: "Filesystem & Workspace",
    version: "1.0.0",
    description: "Standard filesystem operations, tree inspection, and search via Model Context Protocol.",
    author: "ARO Ecosystem",
    category: "Filesystem",
    icon: "📁",
    repository: "https://github.com/agentplugins/filesystem-tools",
    keywords: ["fs", "workspace", "files"],
    mcpServersCount: 1,
    skillsCount: 2,
    installed: true,
    sampleSkills: ["workspace-explorer", "file-finder"],
    sampleMcpServers: ["local-fs"],
  };

  beforeEach(() => {
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
  });

  it("renders plugin details hero, badges, and technical specs", () => {
    const onBack = vi.fn();
    const onInstall = vi.fn();
    const onToggle = vi.fn();
    const onUninstall = vi.fn();
    const onRefresh = vi.fn();

    const { container } = render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: true,
        installedPlugins: [mockInstalledPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack,
        onInstall,
        onToggle,
        onUninstall,
        onRefresh,
      },
    });

    expect(screen.getAllByText("Filesystem & Workspace").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("v1.0.0")).toBeInTheDocument();
    expect(screen.getByText("Spec v1.0.0")).toBeInTheDocument();
    expect(screen.getByText("À propos de ce Plugin")).toBeInTheDocument();
    expect(screen.getByText("Spécifications Techniques")).toBeInTheDocument();
    expect(container.querySelector(".plugin-detail-root.dark")).toBeTruthy();
  });

  it("switches tabs between Overview, Skills, MCP Servers, Permissions, and Manifest", async () => {
    render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: false,
        installedPlugins: [mockInstalledPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack: vi.fn(),
        onInstall: vi.fn(),
        onToggle: vi.fn(),
        onUninstall: vi.fn(),
        onRefresh: vi.fn(),
      },
    });

    // 1. Skills tab
    const skillsTabBtn = screen.getByRole("button", { name: /Skills \(2\)/i });
    await fireEvent.click(skillsTabBtn);
    expect(screen.getByText("Workspace Explorer")).toBeInTheDocument();
    expect(screen.getByText("File Finder")).toBeInTheDocument();

    // 2. MCP Servers tab
    const mcpTabBtn = screen.getByRole("button", { name: /Serveurs MCP \(1\)/i });
    await fireEvent.click(mcpTabBtn);
    expect(screen.getByText("local-fs")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Tester le Serveur MCP/i })).toBeInTheDocument();

    // 3. Permissions tab
    const permTabBtn = screen.getByRole("button", { name: /Permissions & Sécurité/i });
    await fireEvent.click(permTabBtn);
    expect(screen.getByText("Sandbox & Isolation des Processus")).toBeInTheDocument();
    expect(screen.getByText("Accès au Système de Fichiers")).toBeInTheDocument();

    // 4. Manifest tab
    const manifestTabBtn = screen.getByRole("button", { name: /Manifeste \(plugin\.json\)/i });
    await fireEvent.click(manifestTabBtn);
    expect(screen.getByText("plugin.json")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Copier le Manifeste/i })).toBeInTheDocument();
  });

  it("executes MCP server test and displays live tool results", async () => {
    vi.spyOn(api, "testPluginMcpServer").mockResolvedValueOnce([
      { name: "read_file", description: "Read file contents from disk" },
      { name: "write_file", description: "Write file content securely" },
    ]);

    render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: true,
        installedPlugins: [mockInstalledPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack: vi.fn(),
        onInstall: vi.fn(),
        onToggle: vi.fn(),
        onUninstall: vi.fn(),
        onRefresh: vi.fn(),
      },
    });

    const mcpTabBtn = screen.getByRole("button", { name: /Serveurs MCP \(1\)/i });
    await fireEvent.click(mcpTabBtn);

    const testBtn = screen.getByRole("button", { name: /Tester le Serveur MCP/i });
    await fireEvent.click(testBtn);

    await waitFor(() => {
      expect(screen.getByText(/Connexion réussie !/i)).toBeInTheDocument();
      expect(screen.getByText(/2 outil\(s\) MCP disponible\(s\)/i)).toBeInTheDocument();
      expect(screen.getByText("read_file")).toBeInTheDocument();
      expect(screen.getByText("write_file")).toBeInTheDocument();
    });
  });

  it("copies manifest to clipboard when copy button is clicked", async () => {
    render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: true,
        installedPlugins: [mockInstalledPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack: vi.fn(),
        onInstall: vi.fn(),
        onToggle: vi.fn(),
        onUninstall: vi.fn(),
        onRefresh: vi.fn(),
      },
    });

    const manifestTabBtn = screen.getByRole("button", { name: /Manifeste \(plugin\.json\)/i });
    await fireEvent.click(manifestTabBtn);

    const copyBtn = screen.getByRole("button", { name: /Copier le Manifeste/i });
    await fireEvent.click(copyBtn);

    expect(navigator.clipboard.writeText).toHaveBeenCalled();
  });

  it("navigates back when clicking the back button", async () => {
    const onBack = vi.fn();
    render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: true,
        installedPlugins: [mockInstalledPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack,
        onInstall: vi.fn(),
        onToggle: vi.fn(),
        onUninstall: vi.fn(),
        onRefresh: vi.fn(),
      },
    });

    const backBtn = screen.getByTitle("Revenir à la liste des plugins");
    await fireEvent.click(backBtn);
    expect(onBack).toHaveBeenCalledTimes(1);
  });

  it("navigates by category breadcrumb button when onSelectCategory is provided", async () => {
    const onSelectCategory = vi.fn();
    render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: true,
        installedPlugins: [mockInstalledPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack: vi.fn(),
        onSelectCategory,
        onInstall: vi.fn(),
        onToggle: vi.fn(),
        onUninstall: vi.fn(),
        onRefresh: vi.fn(),
      },
    });

    const catBtn = screen.getByTitle(/Filtrer le catalogue par Filesystem/i);
    await fireEvent.click(catBtn);
    expect(onSelectCategory).toHaveBeenCalledWith("Filesystem");
  });

  it("supports skill execution via Enter keypress in input", async () => {
    const spy = vi.spyOn(api, "invokePluginSkill").mockResolvedValueOnce({
      output: "Executed skill successfully",
    });

    render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: true,
        installedPlugins: [mockInstalledPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack: vi.fn(),
        onInstall: vi.fn(),
        onToggle: vi.fn(),
        onUninstall: vi.fn(),
        onRefresh: vi.fn(),
      },
    });

    const skillsTabBtn = screen.getByRole("button", { name: /Skills \(2\)/i });
    await fireEvent.click(skillsTabBtn);

    const inputs = screen.getAllByPlaceholderText(/Paramètre de test/i);
    await fireEvent.input(inputs[0], { target: { value: "test query" } });
    await fireEvent.keyDown(inputs[0], { key: "Enter" });

    expect(spy).toHaveBeenCalledWith("filesystem-tools", "workspace-explorer", {
      prompt: "test query",
    });
  });

  it("protects system plugins from uninstallation and displays protected badge", () => {
    const systemPlugin: api.InstalledPlugin = {
      ...mockInstalledPlugin,
      isSystem: true,
    };

    render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: true,
        installedPlugins: [systemPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack: vi.fn(),
        onInstall: vi.fn(),
        onToggle: vi.fn(),
        onUninstall: vi.fn(),
        onRefresh: vi.fn(),
      },
    });

    expect(screen.getByText("Système protégé")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Désinstaller/i })).toBeNull();
  });

  it("displays error status pill and banner when status is error", () => {
    const errorPlugin: api.InstalledPlugin = {
      ...mockInstalledPlugin,
      status: "error",
      statusMessage: "MCP server process crashed on startup",
    };

    render(PluginDetailView, {
      props: {
        pluginId: "filesystem-tools",
        isDark: true,
        installedPlugins: [errorPlugin],
        marketplacePlugins: [mockMarketplacePlugin],
        actionLoading: {},
        onBack: vi.fn(),
        onInstall: vi.fn(),
        onToggle: vi.fn(),
        onUninstall: vi.fn(),
        onRefresh: vi.fn(),
      },
    });

    expect(screen.getByText(/Erreur système/i)).toBeInTheDocument();
    expect(screen.getByText("MCP server process crashed on startup")).toBeInTheDocument();
  });
});

describe("PluginsSettings Navigation to PluginDetailView", () => {
  beforeEach(() => {
    vi.spyOn(api, "listInstalledPlugins").mockResolvedValue([
      {
        id: "filesystem-tools",
        name: "Filesystem & Workspace",
        version: "1.0.0",
        description: "Filesystem tools for ARO",
        author: "ARO Ecosystem",
        rootPath: "/plugins/fs",
        dataPath: "/data/fs",
        enabled: true,
        status: "active",
        installedAt: "2026-09-12T10:00:00Z",
        source: "marketplace",
        keywords: ["fs"],
        mcpServers: [
          {
            name: "local-fs",
            transportType: "stdio",
            status: "ready",
          },
        ],
        skills: [
          {
            id: "fs-explorer",
            name: "FS Explorer",
            description: "Explore FS",
            tags: ["fs"],
            hasScripts: true,
          },
        ],
      },
    ]);

    vi.spyOn(api, "listMarketplacePlugins").mockResolvedValue(api.CURATED_MARKETPLACE);
  });

  it("opens plugin details page when clicking on a plugin card in marketplace", async () => {
    const { container } = render(PluginsSettings, {
      props: {
        currentTheme: "dark",
      },
    });

    await waitFor(() => {
      expect(screen.getByText("Web Search & Fetch")).toBeInTheDocument();
    });

    // Find the Web Search card
    const cardTitle = screen.getByText("Web Search & Fetch");
    const card = cardTitle.closest(".plugin-card");
    expect(card).toBeTruthy();

    // Click the card to open details page
    if (card) {
      await fireEvent.click(card);
    }

    // Details view should now be displayed
    await waitFor(() => {
      expect(container.querySelector(".plugin-detail-root")).toBeTruthy();
      expect(screen.getByRole("button", { name: /Installer le plugin/i })).toBeInTheDocument();
      expect(screen.getByTitle("Revenir à la liste des plugins")).toBeInTheDocument();
    });

    // Click back button to return to marketplace
    const backBtn = screen.getByTitle("Revenir à la liste des plugins");
    await fireEvent.click(backBtn);

    // Grid should be restored
    await waitFor(() => {
      expect(container.querySelector(".plugins-grid")).toBeTruthy();
    });
  });

  it("opens plugin details page when clicking Détails button on an installed plugin", async () => {
    const { container } = render(PluginsSettings, {
      props: {
        currentTheme: "dark",
      },
    });

    await waitFor(() => {
      expect(screen.getByText("Plugins Installés")).toBeInTheDocument();
    });

    // Switch to Installed tab
    const installedTabBtn = screen.getByRole("button", { name: /Plugins Installés/i });
    await fireEvent.click(installedTabBtn);

    await waitFor(() => {
      expect(screen.getByText("Filesystem & Workspace")).toBeInTheDocument();
    });

    // Click "Détails" button
    const detailsBtns = screen.getAllByRole("button", { name: /Détails/i });
    await fireEvent.click(detailsBtns[0]);

    // Detail view should open
    await waitFor(() => {
      expect(container.querySelector(".plugin-detail-root")).toBeTruthy();
      expect(screen.getByText("À propos de ce Plugin")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: /Désinstaller/i })).toBeInTheDocument();
    });
  });

  it("does not navigate to detail view when pressing Enter on inner button due to target checking", async () => {
    const { container } = render(PluginsSettings, {
      props: {
        currentTheme: "dark",
      },
    });

    await waitFor(() => {
      expect(screen.getByText("Web Search & Fetch")).toBeInTheDocument();
    });

    const installBtns = screen.getAllByRole("button", { name: /Installer/i });
    // Pressing Enter directly on the inner install button should not bubble to open details
    await fireEvent.keyDown(installBtns[0], { key: "Enter" });

    // Details view should NOT open
    expect(container.querySelector(".plugin-detail-root")).toBeNull();
  });
});
