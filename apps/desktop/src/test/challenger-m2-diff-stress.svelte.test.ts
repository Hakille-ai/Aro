import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import CodeDiffViewer from "../features/workspace/CodeDiffViewer.svelte";
import RightPanel from "../features/shell/RightPanel.svelte";
import * as transport from "../lib/api/transport";
import { computeLineDiff, generateSplitDiffRows, parseUnifiedDiff, type DiffLine } from "../lib/diff";

function makeRightPanelProps(overrides: Record<string, any> = {}) {
  return {
    activeConversationId: "conv-challenger-1",
    language: "fr" as const,
    width: 600,
    onClose: vi.fn(),
    artifacts: [],
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

describe("Adversarial Stress Suite: CodeDiffViewer & RightPanel", () => {
  let originalClipboard: any;

  beforeEach(() => {
    originalClipboard = navigator.clipboard;
  });

  afterEach(() => {
    vi.restoreAllMocks();
    Object.defineProperty(navigator, "clipboard", {
      value: originalClipboard,
      writable: true,
      configurable: true,
    });
  });

  /* ========================================================================= */
  /* 1. Asymmetric Diff & Heavy Load Stress                                    */
  /* ========================================================================= */
  describe("1. Split vs Unified Under Asymmetric & Heavy Diff Loads", () => {
    it("handles heavy asymmetric deletions (50 removed, 0 added) in split & unified modes", async () => {
      const diffLines: DiffLine[] = Array.from({ length: 50 }, (_, i) => ({
        type: "removed" as const,
        oldLineNo: i + 1,
        content: `const deprecated_var_${i} = "to be purged";`,
      }));

      const { container } = render(CodeDiffViewer, {
        filePath: "src/cleanup.ts",
        diffLines,
        language: "fr",
        viewMode: "unified",
      });

      // Unified check
      const unifiedRows = container.querySelectorAll(".unified-view .diff-row");
      expect(unifiedRows.length).toBe(50);
      expect(container.querySelectorAll(".diff-row.removed").length).toBe(50);

      // Switch to split mode
      const splitBtn = screen.getByRole("button", { name: "Scindé" });
      await fireEvent.click(splitBtn);

      const splitRows = container.querySelectorAll(".split-view .split-row");
      expect(splitRows.length).toBe(50);

      // Verify old cells have content and removed class, while new cells are empty
      const oldRemovedCells = container.querySelectorAll(".split-cell.old-cell.removed");
      expect(oldRemovedCells.length).toBe(50);

      const newEmptyCells = container.querySelectorAll(".split-cell.new-cell.empty");
      expect(newEmptyCells.length).toBe(50);

      // Switch back to unified
      const unifiedBtn = screen.getByRole("button", { name: "Unifié" });
      await fireEvent.click(unifiedBtn);
      expect(container.querySelectorAll(".unified-view .diff-row").length).toBe(50);
    });

    it("handles heavy asymmetric additions (0 removed, 40 added) with empty old cells in split mode", async () => {
      const diffLines: DiffLine[] = Array.from({ length: 40 }, (_, i) => ({
        type: "added" as const,
        newLineNo: i + 1,
        content: `export const newFeature_${i} = true;`,
      }));

      const { container } = render(CodeDiffViewer, {
        filePath: "src/features.ts",
        diffLines,
        language: "fr",
        viewMode: "split",
      });

      const splitRows = container.querySelectorAll(".split-view .split-row");
      expect(splitRows.length).toBe(40);

      const oldEmptyCells = container.querySelectorAll(".split-cell.old-cell.empty");
      expect(oldEmptyCells.length).toBe(40);

      const newAddedCells = container.querySelectorAll(".split-cell.new-cell.added");
      expect(newAddedCells.length).toBe(40);
    });

    it("correctly calculates split row alignment across multi-block asymmetric diffs", async () => {
      // Block 1: 10 removed, 2 added -> max(10, 2) = 10 rows
      // Block 2: 5 context -> 5 rows
      // Block 3: 1 removed, 12 added -> max(1, 12) = 12 rows
      // Total split rows: 10 + 5 + 12 = 27 rows
      // Total unified rows: 10 + 2 + 5 + 1 + 12 = 30 rows
      const diffLines: DiffLine[] = [
        ...Array.from({ length: 10 }, (_, i) => ({
          type: "removed" as const,
          oldLineNo: i + 1,
          content: `del_1_${i}`,
        })),
        ...Array.from({ length: 2 }, (_, i) => ({
          type: "added" as const,
          newLineNo: i + 1,
          content: `add_1_${i}`,
        })),
        ...Array.from({ length: 5 }, (_, i) => ({
          type: "context" as const,
          oldLineNo: 11 + i,
          newLineNo: 3 + i,
          content: `ctx_${i}`,
        })),
        {
          type: "removed" as const,
          oldLineNo: 16,
          content: "del_2_0",
        },
        ...Array.from({ length: 12 }, (_, i) => ({
          type: "added" as const,
          newLineNo: 8 + i,
          content: `add_2_${i}`,
        })),
      ];

      const splitGenerated = generateSplitDiffRows(diffLines);
      expect(splitGenerated.length).toBe(27);

      const { container } = render(CodeDiffViewer, {
        filePath: "src/complex.ts",
        diffLines,
        language: "fr",
        viewMode: "unified",
      });

      expect(container.querySelectorAll(".unified-view .diff-row").length).toBe(30);

      const splitBtn = screen.getByRole("button", { name: "Scindé" });
      await fireEvent.click(splitBtn);

      expect(container.querySelectorAll(".split-view .split-row").length).toBe(27);
    });

    it("survives rapid toggling between Split and Unified under a 1,000-line diff stress load", async () => {
      const diffLines: DiffLine[] = Array.from({ length: 1000 }, (_, i) => {
        if (i % 3 === 0) return { type: "context" as const, oldLineNo: i + 1, newLineNo: i + 1, content: `ctx_${i}` };
        if (i % 3 === 1) return { type: "removed" as const, oldLineNo: i + 1, content: `rem_${i}` };
        return { type: "added" as const, newLineNo: i + 1, content: `add_${i}` };
      });

      const { container } = render(CodeDiffViewer, {
        filePath: "src/gigantic.ts",
        diffLines,
        language: "fr",
        viewMode: "unified",
      });

      const splitBtn = screen.getByRole("button", { name: "Scindé" });
      const unifiedBtn = screen.getByRole("button", { name: "Unifié" });

      // Rapidly toggle 6 times
      for (let cycle = 0; cycle < 3; cycle++) {
        await fireEvent.click(splitBtn);
        expect(container.querySelector(".split-view")).toBeInTheDocument();
        await fireEvent.click(unifiedBtn);
        expect(container.querySelector(".unified-view")).toBeInTheDocument();
      }

      expect(container.querySelectorAll(".diff-row").length).toBe(1000);
    }, 15000);

    it("handles adversarial inputs: script tags, HTML injection, emojis and unicode characters safely", () => {
      const xssLines: DiffLine[] = [
        { type: "removed", oldLineNo: 1, content: '<script>alert("pwned")</script>' },
        { type: "added", newLineNo: 1, content: '<img src="x" onerror="steal()"/>' },
        { type: "context", oldLineNo: 2, newLineNo: 2, content: 'const emoji = "🚀🔥🤖 — 日本語 & special chars <> & \\" \'";' },
      ];

      const { container } = render(CodeDiffViewer, {
        filePath: "src/security.ts",
        diffLines: xssLines,
        language: "fr",
        viewMode: "unified",
      });

      // Verify raw text is present in DOM safely without executing scripts
      expect(container.innerHTML).not.toContain('<script>alert("pwned")');
      expect(screen.getByText(/const emoji/)).toBeInTheDocument();
    });

    it("resiliently handles sparse arrays and null/undefined DiffLine entries without crashing", () => {
      const corruptLines: any[] = [
        null,
        undefined,
        { type: "added", newLineNo: 1, content: "valid added line" },
        { type: "removed", oldLineNo: 1, content: "valid removed line" },
        null,
      ];

      expect(() => {
        render(CodeDiffViewer, {
          filePath: "src/sparse.ts",
          diffLines: corruptLines as any,
          language: "fr",
        });
      }).not.toThrow();

      expect(screen.getByText("valid added line")).toBeInTheDocument();
      expect(screen.getByText("valid removed line")).toBeInTheDocument();
    });
  });

  /* ========================================================================= */
  /* 2. State Transitions, Concurrency & Clipboard Failure Resilience           */
  /* ========================================================================= */
  describe("2. Clipboard Failure Fallback & Apply/Reject State Transitions", () => {
    it("gracefully catches clipboard writeText rejection without throwing or showing false 'Copié !'", async () => {
      const writeTextMock = vi.fn().mockRejectedValue(new Error("DOMException: Document is not focused"));
      Object.defineProperty(navigator, "clipboard", {
        value: { writeText: writeTextMock },
        writable: true,
        configurable: true,
      });

      render(CodeDiffViewer, {
        filePath: "src/clipboard.ts",
        diffLines: [{ type: "added", newLineNo: 1, content: "const safe = true;" }],
        language: "fr",
      });

      const copyBtn = screen.getByRole("button", { name: /Copier/i });
      await fireEvent.click(copyBtn);

      await waitFor(() => {
        expect(writeTextMock).toHaveBeenCalledWith("const safe = true;");
      });

      // Crucial: Must NOT flip to "Copié !" on rejection
      expect(screen.queryByText("Copié !")).not.toBeInTheDocument();
      expect(screen.getByText(/Copier/i)).toBeInTheDocument();
    });

    it("survives completely missing navigator.clipboard without error", async () => {
      Object.defineProperty(navigator, "clipboard", {
        value: undefined,
        writable: true,
        configurable: true,
      });

      render(CodeDiffViewer, {
        filePath: "src/noclipboard.ts",
        diffLines: [{ type: "added", newLineNo: 1, content: "const a = 1;" }],
        language: "fr",
      });

      const copyBtn = screen.getByRole("button", { name: /Copier/i });
      await expect(fireEvent.click(copyBtn)).resolves.not.toThrow();
      expect(screen.queryByText("Copié !")).not.toBeInTheDocument();
    });

    it("copies only modified lines (context + added, omitting removed) on clipboard success", async () => {
      const writeTextMock = vi.fn().mockResolvedValue(undefined);
      Object.defineProperty(navigator, "clipboard", {
        value: { writeText: writeTextMock },
        writable: true,
        configurable: true,
      });

      render(CodeDiffViewer, {
        filePath: "src/sample.ts",
        diffLines: [
          { type: "context", oldLineNo: 1, newLineNo: 1, content: "import { a } from './a';" },
          { type: "removed", oldLineNo: 2, content: "const oldVal = 10;" },
          { type: "added", newLineNo: 2, content: "const newVal = 20;" },
          { type: "context", oldLineNo: 3, newLineNo: 3, content: "export default newVal;" },
        ],
        language: "fr",
      });

      const copyBtn = screen.getByRole("button", { name: /Copier/i });
      await fireEvent.click(copyBtn);

      await waitFor(() => {
        expect(writeTextMock).toHaveBeenCalledTimes(1);
      });

      const copiedContent = writeTextMock.mock.calls[0][0];
      expect(copiedContent).toBe("import { a } from './a';\nconst newVal = 20;\nexport default newVal;");
      expect(copiedContent).not.toContain("const oldVal = 10;");

      expect(screen.getByText("Copié !")).toBeInTheDocument();
    });

    it("guards against double-click race conditions and debounces concurrent Apply calls", async () => {
      let resolveWrite: () => void;
      const writePromise = new Promise<void>((resolve) => {
        resolveWrite = resolve;
      });

      const writeSpy = vi.spyOn(transport, "writeWorkspaceFile").mockImplementation(() => writePromise);
      const onAcceptMock = vi.fn().mockResolvedValue(undefined);

      render(CodeDiffViewer, {
        filePath: "src/race.ts",
        diffLines: [{ type: "added", newLineNo: 1, content: "race content" }],
        actionVariant: "apply",
        language: "fr",
        onAccept: onAcceptMock,
      });

      const applyBtn = screen.getByRole("button", { name: /Appliquer au projet/i });

      // Click 4 times in rapid succession
      await fireEvent.click(applyBtn);
      await fireEvent.click(applyBtn);
      await fireEvent.click(applyBtn);
      await fireEvent.click(applyBtn);

      expect(onAcceptMock).toHaveBeenCalledTimes(1);
      expect(writeSpy).toHaveBeenCalledTimes(1);
      expect(applyBtn).toBeDisabled();

      // Resolve the write
      resolveWrite!();
      await waitFor(() => {
        expect(screen.getByText("Appliqué")).toBeInTheDocument();
      });

    });

    it("displays error banner when disk write fails and resets isApplying for retry", async () => {
      vi.spyOn(transport, "writeWorkspaceFile").mockRejectedValue(new Error("Permission denied: Read-only disk"));

      render(CodeDiffViewer, {
        filePath: "src/protected.ts",
        diffLines: [{ type: "added", newLineNo: 1, content: "some update" }],
        actionVariant: "apply",
        language: "fr",
      });

      const applyBtn = screen.getByRole("button", { name: /Appliquer au projet/i });
      await fireEvent.click(applyBtn);

      await waitFor(() => {
        expect(screen.getByText(/Permission denied: Read-only disk/i)).toBeInTheDocument();
      });

      // Verify that after error, button is not permanently stuck in Application...
      expect(screen.queryByText("Application...")).not.toBeInTheDocument();
    });

    it("enforces immutable disabled state when initial status is already applied or rejected", () => {
      const { unmount } = render(CodeDiffViewer, {
        filePath: "src/immutable.ts",
        diffLines: [],
        status: "applied",
        language: "fr",
      });

      const appliedBtn = screen.getByRole("button", { name: /Appliqué/i });
      expect(appliedBtn).toBeDisabled();

      unmount();

      render(CodeDiffViewer, {
        filePath: "src/immutable.ts",
        diffLines: [],
        status: "rejected",
        language: "fr",
      });

      const rejectedBtn = screen.getByRole("button", { name: /Refusé/i });
      expect(rejectedBtn).toBeDisabled();
    });
  });

  /* ========================================================================= */
  /* 3. Empty / Malformed Inputs & Edge Cases                                 */
  /* ========================================================================= */
  describe("3. Missing filePath and Empty Diff Edge Cases", () => {
    it("handles missing/empty filePath with localized fallback and skips file writing", async () => {
      const writeSpy = vi.spyOn(transport, "writeWorkspaceFile").mockResolvedValue(undefined);
      const onAcceptMock = vi.fn().mockResolvedValue(undefined);

      render(CodeDiffViewer, {
        filePath: "",
        diffLines: [{ type: "added", newLineNo: 1, content: "code" }],
        language: "fr",
        onAccept: onAcceptMock,
      });

      expect(screen.getByText("Fichier sans titre")).toBeInTheDocument();

      const acceptBtn = screen.getByRole("button", { name: /Accepter/i });
      await fireEvent.click(acceptBtn);

      await waitFor(() => {
        expect(onAcceptMock).toHaveBeenCalledTimes(1);
      });
      // Should not write to disk if filePath is empty
      expect(writeSpy).not.toHaveBeenCalled();
    });

    it("renders empty diff without crash or delta pills when diffLines is empty", () => {
      const { container } = render(CodeDiffViewer, {
        filePath: "src/empty.ts",
        diffLines: [],
        language: "en",
      });

      expect(screen.getByText("src/empty.ts")).toBeInTheDocument();
      expect(container.querySelectorAll(".delta-pill").length).toBe(0);
      expect(container.querySelectorAll(".diff-row").length).toBe(0);
    });
  });

  /* ========================================================================= */
  /* 4. Custom Event Dispatching & RightPanel Integration                     */
  /* ========================================================================= */
  describe("4. Custom Event Dispatching & RightPanel Master-Detail Workflow", () => {
    it("dispatches 'aro:workspace-tree-refresh' with exact path when Apply succeeds in CodeDiffViewer", async () => {
      vi.spyOn(transport, "writeWorkspaceFile").mockResolvedValue(undefined);

      let dispatchedEvent: CustomEvent | null = null;
      const listener = (e: Event) => {
        dispatchedEvent = e as CustomEvent;
      };
      window.addEventListener("aro:workspace-tree-refresh", listener);

      try {
        render(CodeDiffViewer, {
          filePath: "src/components/MyWidget.svelte",
          diffLines: [{ type: "added", newLineNo: 1, content: "<p>widget</p>" }],
          language: "fr",
        });

        const acceptBtn = screen.getByRole("button", { name: /Accepter/i });
        await fireEvent.click(acceptBtn);

        await waitFor(() => {
          expect(dispatchedEvent).not.null;
        });

        expect(dispatchedEvent!.detail).toEqual({
          path: "src/components/MyWidget.svelte",
          source: "CodeDiffViewer",
        });
      } finally {
        window.removeEventListener("aro:workspace-tree-refresh", listener);
      }
    });

    it("switches RightPanel tab and opens detailed diff when 'aro:select-artifact' is fired", async () => {
      const artifacts = [
        {
          id: "art-1",
          title: "App.svelte",
          filePath: "src/App.svelte",
          diffLines: [{ type: "added", newLineNo: 1, content: "<h1>ARO</h1>" }],
          status: "pending",
        },
      ];

      render(RightPanel, makeRightPanelProps({ artifacts }));

      // Dispatch external event to select artifact
      window.dispatchEvent(
        new CustomEvent("aro:select-artifact", {
          detail: { filePath: "src/App.svelte" },
        })
      );

      // Should automatically navigate to outputs tab and show detailed diff
      await waitFor(() => {
        expect(screen.getByText("Retour aux artefacts")).toBeInTheDocument();
      });

      expect(screen.getByText("<h1>ARO</h1>")).toBeInTheDocument();

      // Click "Retour aux artefacts" back button to return to list view
      const backBtn = screen.getByRole("button", { name: /Retour aux artefacts/i });
      await fireEvent.click(backBtn);

      await waitFor(() => {
        expect(screen.getByText(/Livrables du projet/i)).toBeInTheDocument();
      });
    });

    it("filters RightPanel artifacts list by search query and status filter pills", async () => {
      const artifacts = [
        { id: "art-1", title: "Header.svelte", filePath: "src/Header.svelte", status: "pending", additions: 5, deletions: 2 },
        { id: "art-2", title: "Footer.svelte", filePath: "src/Footer.svelte", status: "applied", additions: 10, deletions: 0 },
        { id: "art-3", title: "Sidebar.svelte", filePath: "src/Sidebar.svelte", status: "rejected", additions: 1, deletions: 8 },
      ];

      render(RightPanel, makeRightPanelProps({ artifacts }));

      // Open outputs tab
      const outputsTabBtn = screen.getByTitle(/Sorties/i);
      await fireEvent.click(outputsTabBtn);

      expect(screen.getByText("Header.svelte")).toBeInTheDocument();
      expect(screen.getByText("Footer.svelte")).toBeInTheDocument();
      expect(screen.getByText("Sidebar.svelte")).toBeInTheDocument();

      // Filter by 'applied'
      const appliedFilterBtn = screen.getByRole("button", { name: /Appliqués/i });
      await fireEvent.click(appliedFilterBtn);

      expect(screen.queryByText("Header.svelte")).not.toBeInTheDocument();
      expect(screen.getByText("Footer.svelte")).toBeInTheDocument();
      expect(screen.queryByText("Sidebar.svelte")).not.toBeInTheDocument();

      // Reset to all
      const allFilterBtn = screen.getByRole("button", { name: /Tous/i });
      await fireEvent.click(allFilterBtn);

      // Search for 'side'
      const searchInput = screen.getByPlaceholderText(/Filtrer les fichiers/i);
      await fireEvent.input(searchInput, { target: { value: "side" } });

      expect(screen.queryByText("Header.svelte")).not.toBeInTheDocument();
      expect(screen.queryByText("Footer.svelte")).not.toBeInTheDocument();
      expect(screen.getByText("Sidebar.svelte")).toBeInTheDocument();
    });

    it("applies artifact in RightPanel list and dispatches 'aro:workspace-tree-refresh'", async () => {
      const applyDiffSpy = vi.spyOn(transport, "applyWorkspaceDiff").mockResolvedValue(undefined);

      let refreshed = false;
      const listener = (e: Event) => {
        const ce = e as CustomEvent;
        if (ce.detail?.path === "src/patched.ts") refreshed = true;
      };
      window.addEventListener("aro:workspace-tree-refresh", listener);

      try {
        const artifacts = [
          {
            id: "art-patch",
            title: "patched.ts",
            filePath: "src/patched.ts",
            diffText: "@@ -1,1 +1,2 @@\n-old\n+new\n+added",
            status: "pending",
          },
        ];

        render(RightPanel, makeRightPanelProps({ artifacts }));

        // Open outputs tab
        const outputsTabBtn = screen.getByTitle(/Sorties/i);
        await fireEvent.click(outputsTabBtn);

        const applyBtn = screen.getByRole("button", { name: /Appliquer/i });
        await fireEvent.click(applyBtn);

        await waitFor(() => {
          expect(applyDiffSpy).toHaveBeenCalledWith("src/patched.ts", "@@ -1,1 +1,2 @@\n-old\n+new\n+added", "conv-challenger-1");
          expect(refreshed).toBe(true);
        });
      } finally {
        window.removeEventListener("aro:workspace-tree-refresh", listener);
      }
    });

    it("ADVERSARIAL: rapid continuous toggling (10 cycles = 20 switches) under 1,000-line diff stress load verifies DOM visibility without churn", async () => {
      const diffLines: DiffLine[] = Array.from({ length: 1000 }, (_, i) => {
        if (i % 3 === 0) return { type: "context" as const, oldLineNo: i + 1, newLineNo: i + 1, content: `ctx_${i}` };
        if (i % 3 === 1) return { type: "removed" as const, oldLineNo: i + 1, content: `rem_${i}` };
        return { type: "added" as const, newLineNo: i + 1, content: `add_${i}` };
      });

      const startTime = Date.now();
      const { container } = render(CodeDiffViewer, {
        filePath: "src/gigantic-adversarial.ts",
        diffLines,
        language: "fr",
        viewMode: "unified",
      });

      const splitBtn = screen.getByRole("button", { name: "Scindé" });
      const unifiedBtn = screen.getByRole("button", { name: "Unifié" });

      const unifiedEl = container.querySelector(".unified-view") as HTMLElement;
      expect(unifiedEl).toBeInTheDocument();
      expect(unifiedEl.style.display).toBe("block");
      // Initially, split-view is NOT yet mounted into DOM (lazy mount)
      expect(container.querySelector(".split-view")).toBeNull();

      // Perform 10 full toggle cycles (20 toggle clicks)
      for (let cycle = 0; cycle < 10; cycle++) {
        await fireEvent.click(splitBtn);
        const splitEl = container.querySelector(".split-view") as HTMLElement;
        expect(splitEl).not.toBeNull();
        expect(splitEl.style.display).toBe("block");
        expect(unifiedEl.style.display).toBe("none");

        await fireEvent.click(unifiedBtn);
        expect(unifiedEl.style.display).toBe("block");
        expect(splitEl.style.display).toBe("none");
      }

      const elapsed = Date.now() - startTime;
      // 10 cycles (20 toggles) of 1,000 lines should complete comfortably under 5 seconds with CSS toggling
      expect(elapsed).toBeLessThan(8000);
      expect(container.querySelectorAll(".diff-row").length).toBe(1000);
    }, 15000);

    it("ADVERSARIAL: clicking Apply in embedded CodeDiffViewer in RightPanel executes EXACTLY single disk write and single tree-refresh event", async () => {
      const applyDiffSpy = vi.spyOn(transport, "applyWorkspaceDiff").mockResolvedValue(undefined);
      const writeSpy = vi.spyOn(transport, "writeWorkspaceFile").mockResolvedValue(undefined);

      let refreshEvents: CustomEvent[] = [];
      const listener = (e: Event) => {
        refreshEvents.push(e as CustomEvent);
      };
      window.addEventListener("aro:workspace-tree-refresh", listener);

      try {
        const artifacts = [
          {
            id: "art-embed-diff",
            title: "feature-patch.diff",
            filePath: "src/feature.ts",
            diffText: "@@ -1,1 +1,2 @@\n-oldLine\n+newLine\n+extraLine",
            status: "pending",
          },
        ];

        render(RightPanel, makeRightPanelProps({ artifacts }));

        // 1. Go to outputs tab
        const outputsTabBtn = screen.getByTitle(/Sorties/i);
        await fireEvent.click(outputsTabBtn);

        // 2. Open detail view for this artifact
        const previewBtn = screen.getByRole("button", { name: /Prévisualiser/i });
        await fireEvent.click(previewBtn);

        // Verify detail view is opened
        expect(screen.getByRole("button", { name: /Retour aux artefacts/i })).toBeInTheDocument();

        // 3. In the detail view, find the Apply button inside the embedded CodeDiffViewer
        const applyBtn = screen.getByRole("button", { name: /Appliquer au projet/i });
        expect(applyBtn).toBeInTheDocument();

        // 4. Click Apply (and rapidly click again to test debounce idempotency)
        await fireEvent.click(applyBtn);
        await fireEvent.click(applyBtn);

        // 5. Wait for applied state to reflect
        await waitFor(() => {
          expect(screen.getByRole("button", { name: /Appliqué/i })).toBeInTheDocument();
        });

        // 6. Verify EXACTLY ONE disk write occurred via applyWorkspaceDiff
        expect(applyDiffSpy).toHaveBeenCalledTimes(1);
        expect(applyDiffSpy).toHaveBeenCalledWith(
          "src/feature.ts",
          "@@ -1,1 +1,2 @@\n-oldLine\n+newLine\n+extraLine",
          "conv-challenger-1"
        );
        expect(writeSpy).toHaveBeenCalledTimes(0);

        // 7. Verify EXACTLY ONE aro:workspace-tree-refresh event was fired
        expect(refreshEvents.length).toBe(1);
        expect(refreshEvents[0].detail).toEqual({
          path: "src/feature.ts",
          source: "CodeDiffViewer",
        });

        // 8. Navigate back to list view and confirm card reflects applied status
        const backBtn = screen.getByRole("button", { name: /Retour aux artefacts/i });
        await fireEvent.click(backBtn);

        await waitFor(() => {
          expect(screen.getByText(/Livrables du projet/i)).toBeInTheDocument();
        });
        const appliedBadge = screen.getByText("Appliqué");
        expect(appliedBadge).toBeInTheDocument();
      } finally {
        window.removeEventListener("aro:workspace-tree-refresh", listener);
      }
    });

    it("ADVERSARIAL: clicking Apply in embedded CodeDiffViewer for modified content executes EXACTLY single writeWorkspaceFile and single tree-refresh", async () => {
      const applyDiffSpy = vi.spyOn(transport, "applyWorkspaceDiff").mockResolvedValue(undefined);
      const writeSpy = vi.spyOn(transport, "writeWorkspaceFile").mockResolvedValue(undefined);

      let refreshEvents: CustomEvent[] = [];
      const listener = (e: Event) => {
        refreshEvents.push(e as CustomEvent);
      };
      window.addEventListener("aro:workspace-tree-refresh", listener);

      try {
        const artifacts = [
          {
            id: "art-embed-content",
            title: "Widget.svelte",
            filePath: "src/Widget.svelte",
            kind: "code",
            content: "<div>Updated Widget</div>",
            diffLines: [
              { type: "removed" as const, oldLineNo: 1, content: "<div>Old Widget</div>" },
              { type: "added" as const, newLineNo: 1, content: "<div>Updated Widget</div>" },
            ],
            status: "pending",
          },
        ];

        render(RightPanel, makeRightPanelProps({ artifacts }));

        // Open outputs tab
        const outputsTabBtn = screen.getByTitle(/Sorties/i);
        await fireEvent.click(outputsTabBtn);

        // Click on title to open detail view
        const titleBtn = screen.getByRole("button", { name: /Widget\.svelte/i });
        await fireEvent.click(titleBtn);

        expect(screen.getByRole("button", { name: /Retour aux artefacts/i })).toBeInTheDocument();

        // Apply inside embedded CodeDiffViewer
        const applyBtn = screen.getByRole("button", { name: /Appliquer au projet/i });
        await fireEvent.click(applyBtn);

        await waitFor(() => {
          expect(screen.getByRole("button", { name: /Appliqué/i })).toBeInTheDocument();
        });

        // Verify EXACTLY ONE write call
        expect(writeSpy).toHaveBeenCalledTimes(1);
        expect(writeSpy).toHaveBeenCalledWith(
          "src/Widget.svelte",
          "<div>Updated Widget</div>",
          "conv-challenger-1"
        );
        expect(applyDiffSpy).toHaveBeenCalledTimes(0);

        // Verify EXACTLY ONE tree-refresh event
        expect(refreshEvents.length).toBe(1);
        expect(refreshEvents[0].detail).toEqual({
          path: "src/Widget.svelte",
          source: "CodeDiffViewer",
        });
      } finally {
        window.removeEventListener("aro:workspace-tree-refresh", listener);
      }
    });
  });
});
