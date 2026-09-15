import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import RightPanel from "./RightPanel.svelte";
import * as transport from "../../lib/api/transport";

// Mock transport API
vi.mock("../../lib/api/transport", async () => {
  const actual = await vi.importActual<typeof transport>("../../lib/api/transport");
  return {
    ...actual,
    listPlans: vi.fn().mockResolvedValue([]),
    createPlan: vi.fn().mockResolvedValue({ id: "plan-1" }),
    updatePlan: vi.fn().mockResolvedValue({ id: "plan-1" }),
    deletePlan: vi.fn().mockResolvedValue(undefined),
    writeWorkspaceFile: vi.fn().mockResolvedValue(undefined),
    applyWorkspaceDiff: vi.fn().mockResolvedValue(undefined),
  };
});

function makeTestArtifacts() {
  return [
    {
      id: "art-1",
      title: "App.svelte",
      filePath: "src/App.svelte",
      kind: "code",
      content: "<main>Hello</main>",
      additions: 10,
      deletions: 2,
      status: "pending",
      createdAt: "2026-09-04T12:00:00Z",
    },
    {
      id: "art-2",
      title: "Button.svelte",
      filePath: "src/components/Button.svelte",
      kind: "code",
      content: "<button>Click</button>",
      additions: 5,
      deletions: 0,
      status: "applied",
      createdAt: "2026-09-04T12:01:00Z",
    },
    {
      id: "art-3",
      title: "styles.css",
      filePath: "src/styles.css",
      kind: "diff",
      diffText: "--- a/src/styles.css\n+++ b/src/styles.css\n@@ -1 +1 @@\n-color: red;\n+color: blue;\n",
      additions: 1,
      deletions: 1,
      status: "rejected",
      createdAt: "2026-09-04T12:02:00Z",
    },
    {
      id: "art-4",
      title: "unspecified-status.ts",
      filePath: "src/unspecified-status.ts",
      kind: "code",
      content: "export const x = 1;",
      additions: 1,
      deletions: 0,
      // status undefined -> should default to pending
      createdAt: "2026-09-04T12:03:00Z",
    },
  ];
}

function makeRightPanelProps(overrides: Record<string, any> = {}) {
  return {
    activeConversationId: "conv-test-1",
    language: "fr" as const,
    width: 600,
    onClose: vi.fn(),
    artifacts: makeTestArtifacts(),
    contextSources: [],
    visibleSources: [],
    attachedFiles: [],
    agentInboxTotals: { running: 0, queued: 0, waiting: 0, done: 0 },
    agentInboxLanes: [],
    agentRunsBusy: false,
    agentActionBusy: null,
    selectedAgentRunView: null,
    personalities: [],
    onToggleAgentLane: vi.fn(),
    onSetAgentLaneAction: vi.fn(),
    onUpdateAgentLanePriority: vi.fn(),
    onOpenAgentRun: vi.fn(),
    onPreviewFile: vi.fn(),
    onStartAgentChat: vi.fn(),
    onStartCreateAgent: vi.fn(),
    onStartEditAgent: vi.fn(),
    onDeleteAgent: vi.fn(),
    ...overrides,
  } as any;
}

describe("Adversarial Component Suite: RightPanel.svelte Outputs & Filtering", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("navigates to outputs tab and displays all initial artifacts", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    expect(screen.getByText("App.svelte")).toBeInTheDocument();
    expect(screen.getByText("Button.svelte")).toBeInTheDocument();
    expect(screen.getByText("styles.css")).toBeInTheDocument();
    expect(screen.getByText("unspecified-status.ts")).toBeInTheDocument();
  });

  it("filters artifacts by status: En attente (pending)", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const pendingBtn = screen.getByRole("button", { name: /^En attente$/i });
    await fireEvent.click(pendingBtn);

    // App.svelte (pending) and unspecified-status.ts (default pending) should be visible
    expect(screen.getByText("App.svelte")).toBeInTheDocument();
    expect(screen.getByText("unspecified-status.ts")).toBeInTheDocument();

    // Button.svelte (applied) and styles.css (rejected) should NOT be visible
    expect(screen.queryByText("Button.svelte")).not.toBeInTheDocument();
    expect(screen.queryByText("styles.css")).not.toBeInTheDocument();
  });

  it("filters artifacts by status: Appliqués (applied)", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const appliedBtn = screen.getByRole("button", { name: /^Appliqués$/i });
    await fireEvent.click(appliedBtn);

    expect(screen.getByText("Button.svelte")).toBeInTheDocument();
    expect(screen.queryByText("App.svelte")).not.toBeInTheDocument();
    expect(screen.queryByText("styles.css")).not.toBeInTheDocument();
    expect(screen.queryByText("unspecified-status.ts")).not.toBeInTheDocument();
  });

  it("filters artifacts by status: Rejetés (rejected)", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const rejectedBtn = screen.getByRole("button", { name: /^Rejetés$/i });
    await fireEvent.click(rejectedBtn);

    expect(screen.getByText("styles.css")).toBeInTheDocument();
    expect(screen.queryByText("App.svelte")).not.toBeInTheDocument();
    expect(screen.queryByText("Button.svelte")).not.toBeInTheDocument();
    expect(screen.queryByText("unspecified-status.ts")).not.toBeInTheDocument();
  });

  it("restores all artifacts when clicking Tous", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const rejectedBtn = screen.getByRole("button", { name: /^Rejetés$/i });
    await fireEvent.click(rejectedBtn);
    expect(screen.queryByText("App.svelte")).not.toBeInTheDocument();

    const allBtn = screen.getByRole("button", { name: /^Tous$/i });
    await fireEvent.click(allBtn);
    expect(screen.getByText("App.svelte")).toBeInTheDocument();
    expect(screen.getByText("Button.svelte")).toBeInTheDocument();
    expect(screen.getByText("styles.css")).toBeInTheDocument();
  });

  it("filters artifacts via search input by title or path", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const searchInput = screen.getByPlaceholderText(/Filtrer les fichiers/i);
    await fireEvent.input(searchInput, { target: { value: "components" } });

    expect(screen.getByText("Button.svelte")).toBeInTheDocument();
    expect(screen.queryByText("App.svelte")).not.toBeInTheDocument();
    expect(screen.queryByText("styles.css")).not.toBeInTheDocument();

    // Clear search with clear button
    const clearBtn = screen.getByRole("button", { name: "" }); // icon-only clear button
    await fireEvent.click(clearBtn);

    expect(screen.getByText("App.svelte")).toBeInTheDocument();
    expect(screen.getByText("Button.svelte")).toBeInTheDocument();
  });

  it("renders empty search state when query matches nothing", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const searchInput = screen.getByPlaceholderText(/Filtrer les fichiers/i);
    await fireEvent.input(searchInput, { target: { value: "nonexistent-query-xyz" } });

    expect(screen.getByText(/Aucun résultat/i)).toBeInTheDocument();
    expect(screen.queryByText("App.svelte")).not.toBeInTheDocument();
  });

  it("combines search query with status filter correctly", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    // Filter to pending
    const pendingBtn = screen.getByRole("button", { name: /^En attente$/i });
    await fireEvent.click(pendingBtn);

    // Search for App
    const searchInput = screen.getByPlaceholderText(/Filtrer les fichiers/i);
    await fireEvent.input(searchInput, { target: { value: "App" } });

    expect(screen.getByText("App.svelte")).toBeInTheDocument();
    expect(screen.queryByText("unspecified-status.ts")).not.toBeInTheDocument();
  });

  it("opens detail view on Prévisualiser and navigates back with Retour aux artefacts", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const previewBtns = screen.getAllByRole("button", { name: /Prévisualiser/i });
    await fireEvent.click(previewBtns[0]); // Preview App.svelte

    // Detail top bar should show "Retour aux artefacts"
    const backBtn = screen.getByRole("button", { name: /Retour aux artefacts/i });
    expect(backBtn).toBeInTheDocument();

    // Click back button to return to list view
    await fireEvent.click(backBtn);
    expect(screen.queryByText(/Retour aux artefacts/i)).not.toBeInTheDocument();
    expect(screen.getByText("Button.svelte")).toBeInTheDocument();
  });

  it("rejects an artifact and updates its status visually", async () => {
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const rejectBtns = screen.getAllByRole("button", { name: /^Rejeter$/i });
    await fireEvent.click(rejectBtns[0]); // Reject first artifact (App.svelte)

    // Filter to rejected
    const rejectedBtn = screen.getByRole("button", { name: /^Rejetés$/i });
    await fireEvent.click(rejectedBtn);

    expect(screen.getByText("App.svelte")).toBeInTheDocument();
  });

  it("EMPIRICAL TEST: verifies applyWorkspaceDiff vs writeWorkspaceFile behavior", async () => {
    // We test applying art-3 which has diffText
    render(RightPanel, makeRightPanelProps());
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    // Find apply buttons
    const applyBtns = screen.getAllByRole("button", { name: /^Appliquer$/i });
    // art-1 has apply button (pending). art-3 has status rejected, so its apply button is also visible.
    // Let's click apply on art-3 (styles.css with diffText)
    await fireEvent.click(applyBtns[1]); // styles.css

    expect(transport.applyWorkspaceDiff).toHaveBeenCalledWith(
      "src/styles.css",
      expect.stringContaining("--- a/src/styles.css"),
      "conv-test-1"
    );
  });

  it("EMPIRICAL TEST: behavior when artifact is created by extractArtifactsFromMessages without diffText property", async () => {
    // Artifact created by extractArtifactsFromMessages:
    // Notice it has kind: "diff", content: "...diff patch...", diffText: undefined!
    const extractedArtifact = {
      id: "art-extracted-diff",
      title: "Patch.diff",
      filePath: "src/Patch.ts",
      kind: "diff",
      content: "--- a/src/Patch.ts\n+++ b/src/Patch.ts\n@@ -1 +1 @@\n-a\n+b\n",
      additions: 1,
      deletions: 1,
      status: "pending",
      createdAt: "2026-09-04T12:00:00Z",
      // NOTE: diffText is NOT provided by extractArtifactsFromMessages!
    };

    render(RightPanel, makeRightPanelProps({ artifacts: [extractedArtifact] }));
    const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
    await fireEvent.click(outputsBtn);

    const applyBtn = screen.getByRole("button", { name: /^Appliquer$/i });
    await fireEvent.click(applyBtn);

    // Observe what RightPanel called:
    // RightPanel checked `if (artifact.diffText)` which was undefined!
    // So it called `writeWorkspaceFile` with the raw diff patch instead of `applyWorkspaceDiff`!
    const writeCalls = vi.mocked(transport.writeWorkspaceFile).mock.calls;
    const diffCalls = vi.mocked(transport.applyWorkspaceDiff).mock.calls;

    // This proves empirically whether applyWorkspaceDiff was called or writeWorkspaceFile was called:
    expect(diffCalls.length).toBe(0); // applyWorkspaceDiff was NOT called!
    expect(writeCalls.length).toBe(1); // writeWorkspaceFile was called instead!
    expect(writeCalls[0][0]).toBe("src/Patch.ts");
    expect(writeCalls[0][1]).toContain("--- a/src/Patch.ts"); // raw patch written to file!
  });
});
