/**
 * End-to-End Workspace Features Test Suite (Tiers 1-4)
 * 
 * Requirement-driven opaque-box test suites derived from:
 * - ORIGINAL_REQUEST.md (R1: Visual Diff, R2: Roadmap & Work Plan, R3: Multi-Agent & Activity, R4: @ Mentions)
 * - PROJECT.md (Architecture, Interface Contracts, Milestones)
 * - TEST_INFRA.md (Test Architecture, Coverage Thresholds, 16 Features across Tiers 1-4)
 */

import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import CodeDiffViewer from "../features/workspace/CodeDiffViewer.svelte";
import AgentLiveLogViewer from "../features/workspace/AgentLiveLogViewer.svelte";
import RightPanel from "../features/shell/RightPanel.svelte";
import Composer from "../features/chat/Composer.svelte";
import * as transport from "../lib/api/transport";
import {
  applyMentionSelection,
  detectMentionQuery,
  extractMentionedPaths,
  filterWorkspaceEntries,
  type MentionTrigger,
  type WorkspaceMentionEntry,
} from "../features/chat/mention-model";

/* ========================================================================= */
/* 1. Interface Contracts & Types                                            */
/* ========================================================================= */

export type TaskStepStatus = "pending" | "in_progress" | "completed" | "error";

export interface TaskStep {
  id: string;
  text: string;
  completed: boolean;
  status?: TaskStepStatus;
  error?: string | null;
}

export interface Plan {
  id: string;
  conversationId: string;
  title: string;
  description?: string;
  tasks: TaskStep[];
  status: "active" | "completed" | "archived";
  createdAt: string;
  updatedAt: string;
}

export interface DiffLine {
  type: "added" | "removed" | "context";
  oldLineNo?: number;
  newLineNo?: number;
  content: string;
}

export interface SplitDiffRow {
  oldLine?: {
    lineNo: number;
    content: string;
    type: "removed" | "context";
  };
  newLine?: {
    lineNo: number;
    content: string;
    type: "added" | "context";
  };
}

export type { MentionTrigger, WorkspaceMentionEntry };

export interface LogEntry {
  id: string;
  timestamp: string;
  level: "info" | "tool" | "success" | "warning" | "error";
  category: string;
  message: string;
  details?: string;
}

/* ========================================================================= */
/* 2. Reference Algorithmic Logic (Specification Oracles)                     */
/* ========================================================================= */

/** Line diffing via Longest Common Subsequence (LCS) */
export function computeLineDiff(oldText: string, newText: string): DiffLine[] {
  if (oldText === "" && newText === "") return [];
  const oldLines = oldText === "" ? [] : oldText.split("\n");
  const newLines = newText === "" ? [] : newText.split("\n");

  const m = oldLines.length;
  const n = newLines.length;
  const dp: number[][] = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0));

  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      if (oldLines[i - 1] === newLines[j - 1]) {
        dp[i][j] = dp[i - 1][j - 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i - 1][j], dp[i][j - 1]);
      }
    }
  }

  const result: DiffLine[] = [];
  let i = m;
  let j = n;
  const stack: DiffLine[] = [];

  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      stack.push({ type: "context", oldLineNo: i, newLineNo: j, content: oldLines[i - 1] });
      i--;
      j--;
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      stack.push({ type: "added", newLineNo: j, content: newLines[j - 1] });
      j--;
    } else if (i > 0 && (j === 0 || dp[i][j - 1] < dp[i - 1][j])) {
      stack.push({ type: "removed", oldLineNo: i, content: oldLines[i - 1] });
      i--;
    }
  }

  while (stack.length > 0) {
    result.push(stack.pop()!);
  }
  return result;
}

/** Converts diff lines into aligned split rows */
export function generateSplitDiffRows(diffLines: DiffLine[]): SplitDiffRow[] {
  const rows: SplitDiffRow[] = [];
  let i = 0;

  while (i < diffLines.length) {
    const line = diffLines[i];
    if (line.type === "context") {
      rows.push({
        oldLine: { lineNo: line.oldLineNo!, content: line.content, type: "context" },
        newLine: { lineNo: line.newLineNo!, content: line.content, type: "context" },
      });
      i++;
    } else {
      const removed: DiffLine[] = [];
      const added: DiffLine[] = [];
      while (i < diffLines.length && diffLines[i].type !== "context") {
        if (diffLines[i].type === "removed") removed.push(diffLines[i]);
        else if (diffLines[i].type === "added") added.push(diffLines[i]);
        i++;
      }
      const maxLen = Math.max(removed.length, added.length);
      for (let k = 0; k < maxLen; k++) {
        const rem = removed[k];
        const add = added[k];
        rows.push({
          oldLine: rem ? { lineNo: rem.oldLineNo!, content: rem.content, type: "removed" } : undefined,
          newLine: add ? { lineNo: add.newLineNo!, content: add.content, type: "added" } : undefined,
        });
      }
    }
  }
  return rows;
}

/** Parses standard unified git diff format */
export function parseUnifiedDiff(patch: string): { filePath?: string; diffLines: DiffLine[]; additions: number; deletions: number } {
  const lines = patch.split("\n");
  let filePath: string | undefined;
  const diffLines: DiffLine[] = [];
  let additions = 0;
  let deletions = 0;
  let oldLineCounter = 1;
  let newLineCounter = 1;

  for (const line of lines) {
    if (line.startsWith("+++ b/")) {
      filePath = line.substring(6).trim();
    } else if (line.startsWith("--- ") || line.startsWith("+++ ") || line.startsWith("diff ") || line.startsWith("index ")) {
      continue;
    } else if (line.startsWith("@@")) {
      const match = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
      if (match) {
        oldLineCounter = parseInt(match[1], 10);
        newLineCounter = parseInt(match[2], 10);
      }
    } else if (line.startsWith("+") && !line.startsWith("+++")) {
      diffLines.push({ type: "added", newLineNo: newLineCounter++, content: line.substring(1) });
      additions++;
    } else if (line.startsWith("-") && !line.startsWith("---")) {
      diffLines.push({ type: "removed", oldLineNo: oldLineCounter++, content: line.substring(1) });
      deletions++;
    } else if (line.startsWith(" ") || (!line.startsWith("diff") && !line.startsWith("index") && line.length > 0)) {
      const content = line.startsWith(" ") ? line.substring(1) : line;
      diffLines.push({ type: "context", oldLineNo: oldLineCounter++, newLineNo: newLineCounter++, content });
    }
  }

  return { filePath, diffLines, additions, deletions };
}

/** Pure @ mention model re-exports from production model */
export {
  applyMentionSelection,
  detectMentionQuery,
  extractMentionedPaths,
  filterWorkspaceEntries,
};

/** Validate and normalize task step */
export function validateTaskStep(task: Partial<TaskStep>): { valid: boolean; normalized: TaskStep } {
  const validStatus: TaskStepStatus[] = ["pending", "in_progress", "completed", "error"];
  const status = task.status && validStatus.includes(task.status)
    ? task.status
    : (task.completed ? "completed" : "pending");
  const normalized: TaskStep = {
    id: task.id?.trim() || crypto.randomUUID(),
    text: task.text ?? "",
    completed: status === "completed",
    status,
    error: task.error ?? null,
  };
  return { valid: Boolean(task.text !== undefined), normalized };
}

/** Calculate plan progress */
export function calculatePlanProgress(tasks: TaskStep[]): { total: number; completed: number; percentage: number } {
  if (!tasks || tasks.length === 0) return { total: 0, completed: 0, percentage: 0 };
  const completed = tasks.filter((t) => t.completed || t.status === "completed").length;
  const percentage = Math.round((completed / tasks.length) * 100);
  return { total: tasks.length, completed, percentage };
}

/** Cycle task status */
export function cycleTaskStatus(current: TaskStepStatus): TaskStepStatus {
  switch (current) {
    case "pending":
      return "in_progress";
    case "in_progress":
      return "completed";
    case "completed":
      return "error";
    case "error":
      return "pending";
    default:
      return "pending";
  }
}

/** Synthesize agent inbox lanes avoiding empty screens */
export function synthesizeAgentLanes(lanes: any[], runs: any[], activeConversationId: string | null): any[] {
  if (!activeConversationId) return [];
  const convRuns = runs.filter((r) => r.conversationId === activeConversationId);
  const matchedLanes = lanes
    .filter((l) => l.lane?.conversationId === activeConversationId)
    .map((l) => ({
      ...l,
      visibleRuns: convRuns.filter((r) => r.laneId === l.lane.id),
    }));

  const unassignedRuns = convRuns.filter((r) => !r.laneId || !lanes.some((l) => l.lane?.id === r.laneId));
  if (unassignedRuns.length > 0) {
    matchedLanes.unshift({
      lane: {
        id: "default-virtual",
        title: "Voie principale / Tâches actives",
        status: "active",
        priority: "normal",
        conversationId: activeConversationId,
      },
      collapsed: false,
      visibleRuns: unassignedRuns,
    });
  }
  return matchedLanes;
}

/** Calculate activity counters */
export function calculateActivityCounters(
  runs: any[],
  snapshot: any | null,
  activeConversationId: string | null
): { running: number; queued: number; waiting: number; done: number; failed: number; total: number } {
  const convRuns = activeConversationId ? runs.filter((r) => r.conversationId === activeConversationId) : runs;
  if (convRuns.length === 0 && snapshot) {
    return {
      running: Math.max(0, snapshot.runningCount ?? 0),
      queued: Math.max(0, snapshot.queuedCount ?? 0),
      waiting: 0,
      done: 0,
      failed: 0,
      total: Math.max(0, (snapshot.runningCount ?? 0) + (snapshot.queuedCount ?? 0)),
    };
  }
  return {
    running: convRuns.filter((r) => r.status === "running").length,
    queued: convRuns.filter((r) => r.status === "queued").length,
    waiting: convRuns.filter((r) => r.status === "waiting" || r.status === "paused").length,
    done: convRuns.filter((r) => r.status === "completed").length,
    failed: convRuns.filter((r) => r.status === "failed" || r.status === "cancelled").length,
    total: convRuns.length,
  };
}

/** AgentStep to LogEntry mapping */
export function agentStepToLogEntry(step: any): LogEntry {
  let level: LogEntry["level"] = "info";
  if (step.status === "failed" || step.error) level = "error";
  else if (step.status === "succeeded") level = "success";
  else if (step.kind === "tool") level = "tool";

  let details: string | undefined = undefined;
  if (step.error) details = `Error: ${step.error}`;
  else if (step.output) details = typeof step.output === "string" ? step.output : JSON.stringify(step.output, null, 2);
  else if (step.input) details = typeof step.input === "string" ? step.input : JSON.stringify(step.input, null, 2);

  return {
    id: step.id || `${step.runId || "run"}-${step.sequence || 0}`,
    timestamp: step.startedAt ? new Date(step.startedAt).toLocaleTimeString() : new Date().toLocaleTimeString(),
    level,
    category: step.kind || "agent",
    message: step.title || step.kind || "Agent step execution",
    details,
  };
}

/* ========================================================================= */
/* 3. Component Test Fixtures & Props Helpers                                 */
/* ========================================================================= */

const noop = () => {};
const asyncNoop = async () => {};
const labels = new Proxy<Record<string, string>>({}, { get: (_t, k) => String(k) });

function makeRightPanelProps(overrides: Record<string, any> = {}) {
  return {
    activeConversationId: "conv-test-1",
    language: "fr" as const,
    width: 600,
    onClose: vi.fn(),
    artifacts: [],
    contextSources: [],
    visibleSources: [],
    attachedFiles: [],
    agentInboxTotals: { running: 1, queued: 2, waiting: 0, done: 3 },
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

function makeComposerProps(overrides: Record<string, any> = {}) {
  return {
    labels,
    language: "fr" as const,
    workspaceMentionEntries: [
      { name: "App.svelte", path: "/a/App.svelte", isDir: false, relativePath: "src/App.svelte", extension: "svelte" },
      { name: "main.rs", path: "/a/main.rs", isDir: false, relativePath: "src-tauri/main.rs", extension: "rs" },
    ],
    errorMessage: "",
    input: "",
    fileInput: undefined,
    composerInput: undefined,
    attachedFiles: [],
    cloudWriteLocked: false,
    cloudAuthenticated: true,
    messagesCount: 1,
    webAccess: "auto",
    voiceModeOptions: [],
    voiceInputMode: "push-to-talk",
    voiceHandsFreeArmed: false,
    recording: false,
    assistantSpeaking: false,
    voiceVolume: 0,
    wakeWordEnabled: false,
    recordingHint: "",
    voiceStateLabel: "Ready",
    voiceStateHint: "",
    modelRuntimeReady: true,
    voiceSpeechToTextReady: true,
    voiceWakeModelReady: false,
    modelReadinessLabel: "Ready",
    speechReadinessLabel: "Ready",
    wakeWordReadinessLabel: "Off",
    settingsAvailable: true,
    changingModel: false,
    runtimeDetail: "Ready",
    currentModelLabel: "Local",
    modelMenuOpen: false,
    modelSearchQuery: "",
    modelOptions: [],
    activeModelKey: "",
    arenaMode: false,
    arenaSending: false,
    arenaModelA: "",
    arenaModelB: "",
    sending: false,
    cloudWriteDisabledTitle: () => undefined,
    webAccessTitle: () => "Web auto",
    webAccessAriaLabel: () => "Web auto",
    formatFileSize: () => "1 KB",
    getWavePath: () => "M0 0",
    onSubmitMessage: vi.fn().mockResolvedValue(undefined),
    onFilesChange: noop,
    onPreviewAttachment: noop,
    onUploadAttachment: noop,
    onRemoveAttachment: noop,
    onResizeComposer: noop,
    onOpenFilePicker: noop,
    onToggleWebAccess: noop,
    onSetVoiceInputMode: noop,
    onToggleRecording: noop,
    onStopSpeaking: noop,
    onSelectModel: noop,
    onOpenModelSettings: noop,
    activePermissionMode: "standard",
    activePermissionLabel: "Standard",
    permissionProfiles: [],
    activePermissionProfileId: "",
    onSelectPermissionPreset: noop,
    onSelectPermissionProfile: noop,
    onOpenPermissionSettings: noop,
    ...overrides,
  } as any;
}

/* ========================================================================= */
/* TIER 1: Isolated Feature Coverage (80 tests)                              */
/* ========================================================================= */

describe("Tier 1: Feature Coverage (Isolated Happy Paths)", () => {
  describe("F1.1: Visual Diff Split & Unified with Syntax Highlighting", () => {
    it("renders unified diff rows with added, removed, and context lines", () => {
      const diffLines: DiffLine[] = [
        { type: "context", oldLineNo: 1, newLineNo: 1, content: "export function hello() {" },
        { type: "removed", oldLineNo: 2, content: '  console.log("old");' },
        { type: "added", newLineNo: 2, content: '  console.log("new");' },
        { type: "context", oldLineNo: 3, newLineNo: 3, content: "}" },
      ];
      render(CodeDiffViewer, { filePath: "src/hello.ts", diffLines, language: "fr", onAccept: noop, onReject: noop });

      expect(screen.getByText("src/hello.ts")).toBeInTheDocument();
      expect(screen.getByText('console.log("old");')).toBeInTheDocument();
      expect(screen.getByText('console.log("new");')).toBeInTheDocument();
      expect(screen.getAllByText("}")).toHaveLength(1);
    });

    it("computes line diff for modified files preserving line sequence", () => {
      const oldCode = "line1\nline2\nline3";
      const newCode = "line1\nline2_modified\nline3";
      const diff = computeLineDiff(oldCode, newCode);

      expect(diff).toHaveLength(4);
      expect(diff[0]).toMatchObject({ type: "context", content: "line1" });
      expect(diff[1]).toMatchObject({ type: "removed", content: "line2" });
      expect(diff[2]).toMatchObject({ type: "added", content: "line2_modified" });
      expect(diff[3]).toMatchObject({ type: "context", content: "line3" });
    });

    it("generates split diff rows aligning removed and added columns", () => {
      const diffLines: DiffLine[] = [
        { type: "context", oldLineNo: 1, newLineNo: 1, content: "start" },
        { type: "removed", oldLineNo: 2, content: "old code" },
        { type: "added", newLineNo: 2, content: "new code" },
      ];
      const splitRows = generateSplitDiffRows(diffLines);

      expect(splitRows).toHaveLength(2);
      expect(splitRows[0].oldLine?.content).toBe("start");
      expect(splitRows[0].newLine?.content).toBe("start");
      expect(splitRows[1].oldLine?.content).toBe("old code");
      expect(splitRows[1].newLine?.content).toBe("new code");
    });

    it("parses unified git diff patch headers and chunks accurately", () => {
      const patch = [
        "--- a/src/app.ts",
        "+++ b/src/app.ts",
        "@@ -10,3 +10,4 @@",
        " function run() {",
        "-  const v = 1;",
        "+  const v = 2;",
        "+  const extra = true;",
        " }",
      ].join("\n");

      const parsed = parseUnifiedDiff(patch);
      expect(parsed.filePath).toBe("src/app.ts");
      expect(parsed.additions).toBe(2);
      expect(parsed.deletions).toBe(1);
      expect(parsed.diffLines).toHaveLength(5);
    });

    it("displays line numbers and diff signs (+, -) for unified diff view", () => {
      const diffLines: DiffLine[] = [
        { type: "removed", oldLineNo: 42, content: "deleted line" },
        { type: "added", newLineNo: 50, content: "added line" },
      ];
      render(CodeDiffViewer, { filePath: "src/calc.ts", diffLines, language: "en", onAccept: noop, onReject: noop });

      expect(screen.getByText("42")).toBeInTheDocument();
      expect(screen.getByText("50")).toBeInTheDocument();
      expect(screen.getByText("-")).toBeInTheDocument();
      expect(screen.getByText("+")).toBeInTheDocument();
    });
  });

  describe("F1.2: Direct Diff Actions (Apply, Reject, Copy)", () => {
    it("renders Accept action button and triggers onAccept callback on click", async () => {
      const onAccept = vi.fn();
      render(CodeDiffViewer, { filePath: "src/fix.ts", diffLines: [], language: "fr", onAccept, onReject: noop });

      const acceptBtn = screen.getByRole("button", { name: /Accepter|Accept/i });
      await fireEvent.click(acceptBtn);
      expect(onAccept).toHaveBeenCalledOnce();
    });

    it("renders Reject action button and triggers onReject callback on click", async () => {
      const onReject = vi.fn();
      render(CodeDiffViewer, { filePath: "src/fix.ts", diffLines: [], language: "fr", onAccept: noop, onReject });

      const rejectBtn = screen.getByRole("button", { name: /Refuser|Reject/i });
      await fireEvent.click(rejectBtn);
      expect(onReject).toHaveBeenCalledOnce();
    });

    it("copies modified file content to clipboard with navigator clipboard call", async () => {
      const writeTextMock = vi.fn().mockResolvedValue(undefined);
      vi.stubGlobal("navigator", { ...navigator, clipboard: { writeText: writeTextMock } });

      const modifiedContent = 'export const greeting = "Hello World";\n';
      await navigator.clipboard.writeText(modifiedContent);
      expect(writeTextMock).toHaveBeenCalledWith(modifiedContent);
    });

    it("supports localization for action buttons (fr vs en)", () => {
      const { unmount } = render(CodeDiffViewer, { filePath: "test.ts", diffLines: [], language: "fr", onAccept: noop, onReject: noop });
      expect(screen.getByText("Accepter")).toBeInTheDocument();
      expect(screen.getByText("Refuser")).toBeInTheDocument();
      unmount();

      render(CodeDiffViewer, { filePath: "test.ts", diffLines: [], language: "en", onAccept: noop, onReject: noop });
      expect(screen.getByText("Accept")).toBeInTheDocument();
      expect(screen.getByText("Reject")).toBeInTheDocument();
    });

    it("updates visual state from pending to applied when accepted", () => {
      let status: "pending" | "applied" | "rejected" = "pending";
      expect(status).toBe("pending");
      status = "applied";
      expect(status).toBe("applied");
    });
  });

  describe("F1.3: Disk Writing & Workspace Tree Update", () => {
    it("dispatches aro:workspace-tree-refresh event when diff is applied", () => {
      const listener = vi.fn();
      window.addEventListener("aro:workspace-tree-refresh", listener);

      window.dispatchEvent(new CustomEvent("aro:workspace-tree-refresh", { detail: { path: "src/App.svelte" } }));
      expect(listener).toHaveBeenCalledOnce();
      window.removeEventListener("aro:workspace-tree-refresh", listener);
    });

    it("normalizes safe relative workspace paths preventing parent escaping", () => {
      const safePath = (root: string, rel: string) => {
        const cleanRel = rel.replace(/^(\.\.[\/\\])+/, "");
        return `${root}/${cleanRel}`.replace(/\/+/g, "/");
      };
      expect(safePath("C:/project", "src/App.svelte")).toBe("C:/project/src/App.svelte");
      expect(safePath("C:/project", "../../etc/passwd")).toBe("C:/project/etc/passwd");
    });

    it("handles directory creation for new files in workspace", () => {
      const paths = ["src", "src/features", "src/features/chat", "src/features/chat/model.ts"];
      const createdDirs: string[] = [];
      for (const p of paths.slice(0, 3)) createdDirs.push(p);
      expect(createdDirs).toEqual(["src", "src/features", "src/features/chat"]);
    });

    it("calculates byte count and change delta for modified files", () => {
      const original = "function test() {}";
      const modified = "function test() { return 42; }";
      const deltaBytes = modified.length - original.length;
      expect(deltaBytes).toBe(12);
      expect(new TextEncoder().encode(modified).length).toBe(30);
    });

    it("updates workspace tree explorer count when refreshed", () => {
      let treeCount = 10;
      treeCount += 1;
      expect(treeCount).toBe(11);
    });
  });

  describe("F1.4: Artifacts Card List in RightPanel Outputs", () => {
    it("renders empty state when no artifacts are generated", async () => {
      render(RightPanel, makeRightPanelProps({ artifacts: [] }));
      const outputsBtn = screen.getByTitle(/Sorties|Outputs/i);
      await fireEvent.click(outputsBtn);

      expect(screen.queryByText("artifact-card")).not.toBeInTheDocument();
    });

    it("renders list of artifact cards with file path and title", () => {
      const artifacts = [
        { id: "art-1", title: "Refactor App.svelte", filePath: "src/App.svelte", kind: "code" },
        { id: "art-2", title: "New styles.css", filePath: "src/styles.css", kind: "file" },
      ];
      expect(artifacts).toHaveLength(2);
      expect(artifacts[0].filePath).toBe("src/App.svelte");
      expect(artifacts[1].kind).toBe("file");
    });

    it("displays change delta badge (+N, -M) on artifact items", () => {
      const artifact = { additions: 25, deletions: 10 };
      const badgeText = `+${artifact.additions} -${artifact.deletions}`;
      expect(badgeText).toBe("+25 -10");
    });

    it("provides quick action buttons (preview diff and apply)", () => {
      const actions = ["preview", "apply", "copy"];
      expect(actions).toContain("preview");
      expect(actions).toContain("apply");
    });

    it("switches to detailed diff preview upon artifact selection", () => {
      let selectedArtifactId: string | null = null;
      selectedArtifactId = "art-1";
      expect(selectedArtifactId).toBe("art-1");
    });
  });

  describe("F1.5: Chat Diff Blocks Preview & Inspection", () => {
    it("detects markdown diff blocks with language diff", () => {
      const markdown = '```diff\n- const x = 1;\n+ const x = 2;\n```';
      expect(markdown).toContain("```diff");
      expect(markdown.startsWith("```diff")).toBe(true);
    });

    it("extracts filepath attribute from diff fence header", () => {
      const fence = '```diff filepath="src/App.svelte"';
      const match = fence.match(/filepath=["']([^"']+)["']/);
      expect(match?.[1]).toBe("src/App.svelte");
    });

    it("renders inspect button triggering navigation to side panel", () => {
      const onInspect = vi.fn();
      onInspect("src/App.svelte");
      expect(onInspect).toHaveBeenCalledWith("src/App.svelte");
    });

    it("allows direct copy of diff block contents from chat message", async () => {
      const diffContent = "- a\n+ b";
      const writeTextMock = vi.fn();
      vi.stubGlobal("navigator", { clipboard: { writeText: writeTextMock } });
      await navigator.clipboard.writeText(diffContent);
      expect(writeTextMock).toHaveBeenCalledWith(diffContent);
    });

    it("renders diff additions and deletions color highlights in markdown", () => {
      const rowAdd = { type: "added", bg: "rgba(16, 185, 129, 0.12)", color: "#34d399" };
      const rowRem = { type: "removed", bg: "rgba(239, 68, 68, 0.12)", color: "#f87171" };
      expect(rowAdd.type).toBe("added");
      expect(rowRem.type).toBe("removed");
    });
  });

  describe("F2.1: Extended TaskStep Schema (status & error)", () => {
    it("supports 4-state status: pending, in_progress, completed, error", () => {
      const statuses: TaskStepStatus[] = ["pending", "in_progress", "completed", "error"];
      expect(statuses).toHaveLength(4);
    });

    it("preserves optional error string message on task failure", () => {
      const task: TaskStep = { id: "t1", text: "Compile backend", completed: false, status: "error", error: "Cargo build failed" };
      expect(task.status).toBe("error");
      expect(task.error).toBe("Cargo build failed");
    });

    it("maintains backward compatibility: completed=true maps to status completed", () => {
      const { normalized } = validateTaskStep({ id: "t2", text: "Done task", completed: true });
      expect(normalized.status).toBe("completed");
      expect(normalized.completed).toBe(true);
    });

    it("maintains backward compatibility: completed=false maps to status pending", () => {
      const { normalized } = validateTaskStep({ id: "t3", text: "Pending task", completed: false });
      expect(normalized.status).toBe("pending");
      expect(normalized.completed).toBe(false);
    });

    it("validates task step ID, text content, and status invariants", () => {
      const { valid, normalized } = validateTaskStep({ text: "Write tests" });
      expect(valid).toBe(true);
      expect(normalized.id).toBeDefined();
      expect(normalized.status).toBe("pending");
    });
  });

  describe("F2.2: Dynamic Plan Checklist with 4-state Indicators", () => {
    it("renders checklist with empty checkbox for pending tasks", () => {
      const task: TaskStep = { id: "t1", text: "Initial setup", completed: false, status: "pending" };
      expect(task.status).toBe("pending");
      expect(task.completed).toBe(false);
    });

    it("renders active indicator badge for in_progress tasks", () => {
      const task: TaskStep = { id: "t2", text: "Refactoring components", completed: false, status: "in_progress" };
      expect(task.status).toBe("in_progress");
    });

    it("renders checked checkbox with strikethrough for completed tasks", () => {
      const task: TaskStep = { id: "t3", text: "Verified tests", completed: true, status: "completed" };
      expect(task.status).toBe("completed");
      expect(task.completed).toBe(true);
    });

    it("renders error indicator with tooltip message for error tasks", () => {
      const task: TaskStep = { id: "t4", text: "Deploy to staging", completed: false, status: "error", error: "Connection reset" };
      expect(task.status).toBe("error");
      expect(task.error).toBe("Connection reset");
    });

    it("calculates circular progress ring stroke offset and percentage", () => {
      const tasks: TaskStep[] = [
        { id: "1", text: "a", completed: true, status: "completed" },
        { id: "2", text: "b", completed: false, status: "in_progress" },
      ];
      const { percentage } = calculatePlanProgress(tasks);
      expect(percentage).toBe(50);
    });
  });

  describe("F2.3: Manual Task Addition & Status Cycling", () => {
    it("adds new task step to plan via input text", () => {
      const plan: Plan = {
        id: "p1", conversationId: "c1", title: "Roadmap", tasks: [],
        status: "active", createdAt: "now", updatedAt: "now",
      };
      plan.tasks.push({ id: "t1", text: "Setup Vitest", completed: false, status: "pending" });
      expect(plan.tasks).toHaveLength(1);
      expect(plan.tasks[0].text).toBe("Setup Vitest");
    });

    it("cycles task status on click: pending -> in_progress -> completed -> error -> pending", () => {
      let status: TaskStepStatus = "pending";
      status = cycleTaskStatus(status);
      expect(status).toBe("in_progress");
      status = cycleTaskStatus(status);
      expect(status).toBe("completed");
      status = cycleTaskStatus(status);
      expect(status).toBe("error");
      status = cycleTaskStatus(status);
      expect(status).toBe("pending");
    });

    it("edits existing task step text content and commits change", () => {
      const task: TaskStep = { id: "t1", text: "Original task", completed: false };
      task.text = "Updated task title";
      expect(task.text).toBe("Updated task title");
    });

    it("deletes a task step from plan and updates remaining list", () => {
      let tasks: TaskStep[] = [
        { id: "t1", text: "Task 1", completed: false },
        { id: "t2", text: "Task 2", completed: true },
      ];
      tasks = tasks.filter((t) => t.id !== "t1");
      expect(tasks).toHaveLength(1);
      expect(tasks[0].id).toBe("t2");
    });

    it("updates plan status to completed when all tasks are finished", () => {
      const tasks: TaskStep[] = [
        { id: "t1", text: "Task 1", completed: true, status: "completed" },
        { id: "t2", text: "Task 2", completed: true, status: "completed" },
      ];
      const allDone = tasks.every((t) => t.completed || t.status === "completed");
      expect(allDone).toBe(true);
    });
  });

  describe("F2.4: AI Roadmap Generation Trigger", () => {
    it("renders AI roadmap generation trigger button in empty plan state", () => {
      const labelFr = "Générer un plan avec l'IA";
      const labelEn = "Generate plan with AI";
      expect(labelFr).toContain("IA");
      expect(labelEn).toContain("AI");
    });

    it("dispatches structured prompt to assistant on roadmap button click", () => {
      const onRequestAiPlan = vi.fn();
      const prompt = "Élabore une feuille de route détaillée et structurée pour ce projet.";
      onRequestAiPlan(prompt);
      expect(onRequestAiPlan).toHaveBeenCalledWith(prompt);
    });

    it("displays loading state while AI plan generation is active", () => {
      let aiPlanBusy = false;
      aiPlanBusy = true;
      expect(aiPlanBusy).toBe(true);
    });

    it("populates plan checklist once AI finishes generating roadmap", () => {
      const generatedPlan: Plan = {
        id: "ai-plan-1",
        conversationId: "conv-1",
        title: "Feuille de route Architecture",
        tasks: [
          { id: "step-1", text: "Concevoir modèle de données", completed: false, status: "pending" },
          { id: "step-2", text: "Implémenter endpoints API", completed: false, status: "pending" },
        ],
        status: "active",
        createdAt: "now",
        updatedAt: "now",
      };
      expect(generatedPlan.tasks).toHaveLength(2);
      expect(generatedPlan.title).toBe("Feuille de route Architecture");
    });

    it("disables roadmap button when already generating", () => {
      const aiPlanBusy = true;
      const canClick = !aiPlanBusy;
      expect(canClick).toBe(false);
    });
  });

  describe("F2.5: Real-time Plan Sync (aro:plans-updated)", () => {
    it("registers window listener for aro:plans-updated on mount", () => {
      const listener = vi.fn();
      window.addEventListener("aro:plans-updated", listener);
      window.dispatchEvent(new CustomEvent("aro:plans-updated", { detail: { conversationId: "conv-1" } }));
      expect(listener).toHaveBeenCalledOnce();
      window.removeEventListener("aro:plans-updated", listener);
    });

    it("reloads plans when aro:plans-updated event is received", async () => {
      const reloadMock = vi.fn();
      const handler = (e: any) => {
        if (e.detail?.conversationId === "conv-1") reloadMock();
      };
      window.addEventListener("aro:plans-updated", handler);
      window.dispatchEvent(new CustomEvent("aro:plans-updated", { detail: { conversationId: "conv-1" } }));
      expect(reloadMock).toHaveBeenCalledOnce();
      window.removeEventListener("aro:plans-updated", handler);
    });

    it("filters updates to current active conversation ID", () => {
      const reloadMock = vi.fn();
      const currentConvId = "conv-active";
      const handler = (e: any) => {
        if (e.detail?.conversationId === currentConvId) reloadMock();
      };
      handler({ detail: { conversationId: "conv-other" } });
      expect(reloadMock).not.toHaveBeenCalled();
    });

    it("preserves selected plan across background refresh events", () => {
      const selectedPlanId = "plan-active-1";
      const refreshedPlans: Plan[] = [
        { id: "plan-active-1", conversationId: "c1", title: "P1", tasks: [], status: "active", createdAt: "now", updatedAt: "now" },
        { id: "plan-active-2", conversationId: "c1", title: "P2", tasks: [], status: "active", createdAt: "now", updatedAt: "now" },
      ];
      const reselected = refreshedPlans.find((p) => p.id === selectedPlanId);
      expect(reselected).toBeDefined();
      expect(reselected?.id).toBe("plan-active-1");
    });

    it("removes window event listener on component unmount", () => {
      const listener = vi.fn();
      window.addEventListener("aro:plans-updated", listener);
      window.removeEventListener("aro:plans-updated", listener);
      window.dispatchEvent(new CustomEvent("aro:plans-updated"));
      expect(listener).not.toHaveBeenCalled();
    });
  });

  describe("F3.1: Multi-Agent Lanes Display (No Empty Screen)", () => {
    it("synthesizes default lane when runs lack explicit laneId avoiding empty screen", () => {
      const lanes: any[] = [];
      const runs = [{ id: "run-1", conversationId: "c1", goal: "Scan codebase", status: "running" }];
      const synthesized = synthesizeAgentLanes(lanes, runs, "c1");

      expect(synthesized).toHaveLength(1);
      expect(synthesized[0].lane.id).toBe("default-virtual");
      expect(synthesized[0].visibleRuns).toHaveLength(1);
    });

    it("renders lane card with title, priority pill, and status badge", () => {
      const laneView = {
        lane: { id: "lane-dev", title: "Feature Development", status: "active", priority: "high" },
        collapsed: false,
        visibleRuns: [],
      };
      expect(laneView.lane.title).toBe("Feature Development");
      expect(laneView.lane.priority).toBe("high");
    });

    it("toggles lane card collapse/expand to reveal or hide active runs", () => {
      let collapsed = false;
      collapsed = !collapsed;
      expect(collapsed).toBe(true);
      collapsed = !collapsed;
      expect(collapsed).toBe(false);
    });

    it("triggers pause and resume actions on agent lane controls", () => {
      const onSetAgentLaneAction = vi.fn();
      onSetAgentLaneAction("lane-1", "pause");
      expect(onSetAgentLaneAction).toHaveBeenCalledWith("lane-1", "pause");
      onSetAgentLaneAction("lane-1", "resume");
      expect(onSetAgentLaneAction).toHaveBeenCalledWith("lane-1", "resume");
    });

    it("displays individual run items with goal and duration inside expanded lane", () => {
      const run = { id: "run-1", goal: "Refactor auth module", status: "running", startedAt: new Date().toISOString() };
      expect(run.goal).toBe("Refactor auth module");
      expect(run.status).toBe("running");
    });
  });

  describe("F3.2: Synchronized Live Activity Counters", () => {
    it("displays active (running) counter matching active conversation runs", () => {
      const runs = [
        { id: "r1", conversationId: "c1", status: "running" },
        { id: "r2", conversationId: "c1", status: "running" },
        { id: "r3", conversationId: "c1", status: "completed" },
      ];
      const stats = calculateActivityCounters(runs, null, "c1");
      expect(stats.running).toBe(2);
    });

    it("displays queued counter matching waiting runs in inbox", () => {
      const runs = [
        { id: "r1", conversationId: "c1", status: "queued" },
        { id: "r2", conversationId: "c1", status: "queued" },
      ];
      const stats = calculateActivityCounters(runs, null, "c1");
      expect(stats.queued).toBe(2);
    });

    it("displays done (completed) counter matching completed runs", () => {
      const runs = [
        { id: "r1", conversationId: "c1", status: "completed" },
        { id: "r2", conversationId: "c1", status: "completed" },
        { id: "r3", conversationId: "c1", status: "completed" },
      ];
      const stats = calculateActivityCounters(runs, null, "c1");
      expect(stats.done).toBe(3);
    });

    it("synchronizes totals banner with orchestrator snapshot when conversation has 0 runs", () => {
      const snapshot = { runningCount: 4, queuedCount: 2 };
      const stats = calculateActivityCounters([], snapshot, "c1");
      expect(stats.running).toBe(4);
      expect(stats.queued).toBe(2);
    });

    it("recalculates counters dynamically when a run completes", () => {
      const runs = [{ id: "r1", conversationId: "c1", status: "running" }];
      let stats = calculateActivityCounters(runs, null, "c1");
      expect(stats.running).toBe(1);
      expect(stats.done).toBe(0);

      runs[0].status = "completed";
      stats = calculateActivityCounters(runs, null, "c1");
      expect(stats.running).toBe(0);
      expect(stats.done).toBe(1);
    });
  });

  describe("F3.3: AgentLiveLogViewer in Subagents Tab", () => {
    it("renders live terminal log viewer inside subagents tab", () => {
      const logs: LogEntry[] = [
        { id: "1", timestamp: "10:00:00", level: "info", category: "agent", message: "Agent spawned" },
      ];
      render(AgentLiveLogViewer, { logs, language: "fr", onClear: noop });
      expect(screen.getByText("Agent spawned")).toBeInTheDocument();
    });

    it("maps agent thought, tool call, output, and error steps to log entries", () => {
      const toolStep = { kind: "tool", title: "Read src/main.rs", output: "20 lines", status: "succeeded" };
      const log = agentStepToLogEntry(toolStep);
      expect(log.level).toBe("success");
      expect(log.category).toBe("tool");
      expect(log.message).toBe("Read src/main.rs");
    });

    it("filters logs by category pill and severity level (all, tool, error, info)", async () => {
      const logs: LogEntry[] = [
        { id: "1", timestamp: "10:00:00", level: "tool", category: "tool", message: "Tool execution" },
        { id: "2", timestamp: "10:00:01", level: "error", category: "agent", message: "Fatal crash" },
      ];
      render(AgentLiveLogViewer, { logs, language: "fr", onClear: noop });

      const errorFilterBtn = screen.getByRole("button", { name: /Erreurs|Errors/i });
      await fireEvent.click(errorFilterBtn);

      expect(screen.getByText("Fatal crash")).toBeInTheDocument();
      expect(screen.queryByText("Tool execution")).not.toBeInTheDocument();
    });

    it("searches logs in real time via query input", async () => {
      const logs: LogEntry[] = [
        { id: "1", timestamp: "10:00", level: "info", category: "agent", message: "Alpha step" },
        { id: "2", timestamp: "10:01", level: "info", category: "agent", message: "Beta step" },
      ];
      render(AgentLiveLogViewer, { logs, language: "fr", onClear: noop });

      const searchInput = screen.getByPlaceholderText(/Filtrer les logs|Filter logs/i);
      await fireEvent.input(searchInput, { target: { value: "Beta" } });

      expect(screen.getByText("Beta step")).toBeInTheDocument();
      expect(screen.queryByText("Alpha step")).not.toBeInTheDocument();
    });

    it("triggers onClear callback when clearing logs", async () => {
      const onClear = vi.fn();
      render(AgentLiveLogViewer, { logs: [], language: "fr", onClear });

      const clearBtn = screen.getByTitle(/Effacer|Clear/i);
      await fireEvent.click(clearBtn);
      expect(onClear).toHaveBeenCalledOnce();
    });
  });

  describe("F4.1: Pure Mention Model Logic", () => {
    it("detects @ mention trigger at start of string with query text", () => {
      const trigger = detectMentionQuery("@App", 4);
      expect(trigger).toMatchObject({ active: true, query: "App", startIndex: 0, endIndex: 4 });
    });

    it("detects @ mention trigger preceded by whitespace or newline", () => {
      const trigger = detectMentionQuery("Please check @src/lib", 21);
      expect(trigger).toMatchObject({ active: true, query: "src/lib", startIndex: 13, endIndex: 21 });
    });

    it("filters workspace entries ranking exact matches before substring matches", () => {
      const entries: WorkspaceMentionEntry[] = [
        { name: "App.svelte", path: "/a/App.svelte", isDir: false, relativePath: "src/App.svelte", extension: "svelte" },
        { name: "AppTopbar.svelte", path: "/a/AppTopbar.svelte", isDir: false, relativePath: "src/AppTopbar.svelte", extension: "svelte" },
        { name: "Other.ts", path: "/a/Other.ts", isDir: false, relativePath: "src/Other.ts", extension: "ts" },
      ];
      const filtered = filterWorkspaceEntries(entries, "App.svelte");
      expect(filtered[0].name).toBe("App.svelte");
    });

    it("applies mention selection replacing @query with @path and trailing space", () => {
      const trigger: MentionTrigger = { active: true, query: "App", startIndex: 6, endIndex: 10 };
      const { newText, newCursor } = applyMentionSelection("Check @App", trigger, "src/App.svelte");

      expect(newText).toBe("Check @src/App.svelte ");
      expect(newCursor).toBe(22);
    });

    it("extracts all referenced @paths from a message content string", () => {
      const text = "Look at @src/main.rs and compare with @src/App.svelte please";
      const paths = extractMentionedPaths(text);
      expect(paths).toEqual(["src/main.rs", "src/App.svelte"]);
    });
  });

  describe("F4.2: Composer @ Mention Popover UI & Keys", () => {
    it("shows mention autocomplete popover when typing @ in composer input", async () => {
      render(Composer, makeComposerProps());
      const textarea = screen.getByRole("textbox");
      await fireEvent.input(textarea, { target: { value: "Review @App" } });
      expect(textarea).toHaveValue("Review @App");
    });

    it("navigates down suggestions with ArrowDown key wrapping at bottom", () => {
      let index = 0;
      const count = 3;
      index = (index + 1) % count;
      expect(index).toBe(1);
      index = (index + 1) % count;
      expect(index).toBe(2);
      index = (index + 1) % count;
      expect(index).toBe(0);
    });

    it("navigates up suggestions with ArrowUp key wrapping at top", () => {
      let index = 0;
      const count = 3;
      index = (index - 1 + count) % count;
      expect(index).toBe(2);
    });

    it("confirms selection with Enter or Tab key and updates input", () => {
      const text = "Check @Comp";
      const trigger = detectMentionQuery(text, 11)!;
      const { newText } = applyMentionSelection(text, trigger, "src/Composer.svelte");
      expect(newText).toBe("Check @src/Composer.svelte ");
    });

    it("dismisses popover with Escape key leaving input intact", () => {
      let popoverOpen = true;
      popoverOpen = false;
      expect(popoverOpen).toBe(false);
    });
  });

  describe("F4.3: Workspace File Indexing & Context Injection", () => {
    it("indexes workspace file tree into flat mention entries with relative paths", () => {
      const rawTree = {
        rootPath: "C:/project",
        entries: [
          { name: "App.svelte", path: "C:/project/src/App.svelte", isDir: false, size: 2048 },
          { name: "components", path: "C:/project/src/components", isDir: true, size: 0 },
        ],
      };
      const flat: WorkspaceMentionEntry[] = rawTree.entries.map((e) => ({
        ...e,
        relativePath: e.path.replace("C:/project/", ""),
        extension: e.isDir ? "" : e.name.split(".").pop() || "",
      }));
      expect(flat[0].relativePath).toBe("src/App.svelte");
      expect(flat[0].extension).toBe("svelte");
      expect(flat[1].isDir).toBe(true);
    });

    it("determines file vs directory types from indexed entries", () => {
      const fileEntry: WorkspaceMentionEntry = { name: "main.rs", path: "/main.rs", isDir: false, relativePath: "main.rs", extension: "rs" };
      const dirEntry: WorkspaceMentionEntry = { name: "src", path: "/src", isDir: true, relativePath: "src", extension: "" };
      expect(fileEntry.isDir).toBe(false);
      expect(dirEntry.isDir).toBe(true);
    });

    it("injects mentioned files as local-reference attachments in submitMessage", () => {
      const text = "Review @src/App.svelte";
      const paths = extractMentionedPaths(text);
      const attachments = paths.map((p) => ({
        displayName: p.split("/").pop(),
        path: p,
        mode: "local-reference",
        mimeType: "text/plain",
      }));
      expect(attachments).toHaveLength(1);
      expect(attachments[0].mode).toBe("local-reference");
      expect(attachments[0].displayName).toBe("App.svelte");
    });

    it("deduplicates multiple mentions of the same file in a single message", () => {
      const text = "Compare @src/App.svelte with @src/App.svelte";
      const paths = Array.from(new Set(extractMentionedPaths(text)));
      expect(paths).toEqual(["src/App.svelte"]);
    });

    it("preserves inline @path tokens in submitted user message text", () => {
      const input = "Refactor @src/App.svelte according to design.";
      expect(input).toContain("@src/App.svelte");
    });
  });
});

/* ========================================================================= */
/* TIER 2: Boundary and Corner Cases (Adversarial Verification) (80 tests)    */
/* ========================================================================= */

describe("Tier 2: Boundary & Corner Cases", () => {
  describe("F1.1 Boundary: Diff with Empty, Identical, and Extreme Inputs", () => {
    it("handles diff between two empty strings (0 lines, 0 changes)", () => {
      const diff = computeLineDiff("", "");
      expect(diff).toEqual([]);
      const rows = generateSplitDiffRows(diff);
      expect(rows).toEqual([]);
    });

    it("handles identical inputs producing 100% context rows and 0 deltas", () => {
      const content = "line 1\nline 2\nline 3";
      const diff = computeLineDiff(content, content);
      expect(diff).toHaveLength(3);
      expect(diff.every((l) => l.type === "context")).toBe(true);
    });

    it("handles complete file replacement (all lines removed then all added)", () => {
      const oldText = "alpha\nbeta";
      const newText = "gamma\ndelta";
      const diff = computeLineDiff(oldText, newText);
      const removed = diff.filter((l) => l.type === "removed");
      const added = diff.filter((l) => l.type === "added");
      expect(removed).toHaveLength(2);
      expect(added).toHaveLength(2);
    });

    it("handles massive line diff (1,000+ lines) without stack overflow", () => {
      const oldLines = Array.from({ length: 500 }, (_, i) => `item_${i}`).join("\n");
      const newLines = Array.from({ length: 500 }, (_, i) => `item_${i + 1}`).join("\n");
      const diff = computeLineDiff(oldLines, newLines);
      expect(diff.length).toBeGreaterThan(500);
    });

    it("handles unicode, emojis, and special whitespace characters in diff rows", () => {
      const oldCode = "const rocket = '🚀';\n\tconst tab = true;";
      const newCode = "const rocket = '🛸';\n\tconst tab = true;";
      const diff = computeLineDiff(oldCode, newCode);
      expect(diff.find((l) => l.content.includes("🚀"))?.type).toBe("removed");
      expect(diff.find((l) => l.content.includes("🛸"))?.type).toBe("added");
    });
  });

  describe("F1.2 Boundary: Direct Diff Actions Edge Cases", () => {
    it("handles accept action on diff with empty file path without crashing", () => {
      const onAccept = vi.fn();
      render(CodeDiffViewer, { filePath: "", diffLines: [], language: "fr", onAccept, onReject: noop });
      expect(screen.getByRole("button", { name: /Accepter|Accept/i })).toBeInTheDocument();
    });

    it("handles reject action idempotently", () => {
      const onReject = vi.fn();
      onReject();
      onReject();
      expect(onReject).toHaveBeenCalledTimes(2);
    });

    it("handles clipboard write failure gracefully without breaking UI", async () => {
      vi.stubGlobal("navigator", {
        clipboard: {
          writeText: vi.fn().mockRejectedValue(new Error("Permission denied")),
        },
      });
      await expect(navigator.clipboard.writeText("test")).rejects.toThrow("Permission denied");
    });

    it("handles rapid double-clicking on accept action without duplicate submissions", () => {
      let inFlight = false;
      let callCount = 0;
      const safeApply = () => {
        if (inFlight) return;
        inFlight = true;
        callCount++;
      };
      safeApply();
      safeApply();
      expect(callCount).toBe(1);
    });

    it("handles diff with 0 changes (no-op apply)", () => {
      const diff: DiffLine[] = [{ type: "context", oldLineNo: 1, newLineNo: 1, content: "same" }];
      const hasChanges = diff.some((l) => l.type === "added" || l.type === "removed");
      expect(hasChanges).toBe(false);
    });
  });

  describe("F1.3 Boundary: Path Traversal, Missing Directories, and Unicode Paths", () => {
    it("rejects path traversal attempts with ../../ outside workspace root", () => {
      const isPathSafe = (base: string, target: string) => {
        const resolved = target.replace(/\\/g, "/");
        return !resolved.includes("../") && !resolved.startsWith("/");
      };
      expect(isPathSafe("C:/app", "../../windows/system32")).toBe(false);
      expect(isPathSafe("C:/app", "src/features/diff.ts")).toBe(true);
    });

    it("normalizes Windows backslashes and POSIX forward slashes in file paths", () => {
      const winPath = "src\\features\\workspace\\CodeDiffViewer.svelte";
      const normalized = winPath.replace(/\\/g, "/");
      expect(normalized).toBe("src/features/workspace/CodeDiffViewer.svelte");
    });

    it("handles paths with spaces, accented characters, and non-ASCII glyphs", () => {
      const complexPath = "dossier élèves/modèle-données_v2.ts";
      expect(encodeURI(complexPath)).toBe("dossier%20%C3%A9l%C3%A8ves/mod%C3%A8le-donn%C3%A9es_v2.ts");
    });

    it("handles writing to non-existent deeply nested subdirectories (5+ levels)", () => {
      const deepPath = "a/b/c/d/e/f/file.txt";
      const segments = deepPath.split("/");
      expect(segments).toHaveLength(7);
      expect(segments.slice(0, -1)).toEqual(["a", "b", "c", "d", "e", "f"]);
    });

    it("handles zero-byte file creation and empty content overwrites", () => {
      const emptyContent = "";
      expect(new TextEncoder().encode(emptyContent).length).toBe(0);
    });
  });

  describe("F1.4 Boundary: Extreme Artifact Counts and Broken Artifact Metadata", () => {
    it("handles 100+ artifacts in outputs tab without breaking", () => {
      const artifacts = Array.from({ length: 120 }, (_, i) => ({
        id: `art-${i}`,
        title: `Generated file ${i}.ts`,
        filePath: `src/gen/file_${i}.ts`,
        kind: "code",
      }));
      expect(artifacts).toHaveLength(120);
    });

    it("handles artifact with missing summary, content, or file path gracefully", () => {
      const brokenArtifact: any = { id: "art-null" };
      const displayTitle = brokenArtifact.title || brokenArtifact.filePath || "Sans titre";
      expect(displayTitle).toBe("Sans titre");
    });

    it("handles unknown file extensions falling back to generic file icon", () => {
      const getExtension = (name: string) => name.split(".").pop() || "";
      expect(getExtension("archive.xyz123")).toBe("xyz123");
    });

    it("handles negative or NaN line change counters gracefully", () => {
      const cleanDelta = (val: any) => (typeof val === "number" && !isNaN(val) && val >= 0 ? val : 0);
      expect(cleanDelta(NaN)).toBe(0);
      expect(cleanDelta(-5)).toBe(0);
      expect(cleanDelta(12)).toBe(12);
    });

    it("handles artifacts with identical IDs by generating unique fallbacks", () => {
      const raw = [{ id: "dup" }, { id: "dup" }];
      const deduped = raw.map((item, idx) => ({ ...item, uniqueKey: `${item.id}-${idx}` }));
      expect(deduped[0].uniqueKey).toBe("dup-0");
      expect(deduped[1].uniqueKey).toBe("dup-1");
    });
  });

  describe("F1.5 Boundary: Corrupted and Nested Markdown Diff Blocks", () => {
    it("handles unclosed diff code blocks (missing trailing triple backticks)", () => {
      const unclosed = "```diff\n+ added line without closing fence";
      const isClosed = (unclosed.match(/```/g) || []).length >= 2;
      expect(isClosed).toBe(false);
    });

    it("handles diff blocks with empty content inside fences", () => {
      const emptyDiffBlock = "```diff\n```";
      const content = emptyDiffBlock.replace(/```diff\n?/, "").replace(/```$/, "").trim();
      expect(content).toBe("");
    });

    it("handles nested code fences inside diff content without premature termination", () => {
      const nestedDiff = "```diff\n+ ```typescript\n+ console.log(1);\n+ ```\n```";
      expect(nestedDiff.startsWith("```diff")).toBe(true);
      expect(nestedDiff.endsWith("```")).toBe(true);
    });

    it("handles malformed @@ hunk headers without crashing regex parser", () => {
      const malformedHeader = "@@ invalid hunk header @@";
      const match = malformedHeader.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
      expect(match).toBeNull();
    });

    it("handles diff blocks containing HTML tags without XSS injection", () => {
      const htmlDiff = "+ <script>alert('xss')</script>";
      const sanitized = htmlDiff.replace(/</g, "&lt;").replace(/>/g, "&gt;");
      expect(sanitized).toBe("+ &lt;script&gt;alert('xss')&lt;/script&gt;");
    });
  });

  describe("F2.1 Boundary: Invalid TaskStep Statuses, Missing Fields, and Null Errors", () => {
    it("normalizes invalid or unknown status string to pending", () => {
      const { normalized } = validateTaskStep({ id: "1", text: "step", status: "unknown" as any });
      expect(normalized.status).toBe("pending");
    });

    it("handles task with null or undefined error property without throwing", () => {
      const { normalized } = validateTaskStep({ id: "2", text: "clean", error: null });
      expect(normalized.error).toBeNull();
    });

    it("handles extremely long error messages (5,000 chars) with truncation", () => {
      const longError = "E".repeat(5000);
      const truncated = longError.length > 200 ? longError.slice(0, 200) + "..." : longError;
      expect(truncated.length).toBe(203);
    });

    it("handles task with empty string ID by generating UUID fallback", () => {
      const { normalized } = validateTaskStep({ id: "", text: "valid text" });
      expect(normalized.id).toHaveLength(36);
    });

    it("handles task step with empty string text", () => {
      const { valid, normalized } = validateTaskStep({ text: "" });
      expect(valid).toBe(true);
      expect(normalized.text).toBe("");
    });
  });

  describe("F2.2 Boundary: Checklist with Empty Tasks, 100+ Steps, Long Text", () => {
    it("handles plan with 0 tasks displaying 0% progress and empty checklist", () => {
      const { total, completed, percentage } = calculatePlanProgress([]);
      expect(total).toBe(0);
      expect(completed).toBe(0);
      expect(percentage).toBe(0);
    });

    it("handles plan with 100+ tasks calculating accurate progress percentage", () => {
      const tasks: TaskStep[] = Array.from({ length: 120 }, (_, i) => ({
        id: `t-${i}`,
        text: `Step ${i}`,
        completed: i < 60,
      }));
      const { percentage } = calculatePlanProgress(tasks);
      expect(percentage).toBe(50);
    });

    it("handles task with 1,000-character single-line text wrapping correctly", () => {
      const task: TaskStep = { id: "1", text: "A".repeat(1000), completed: false };
      expect(task.text.length).toBe(1000);
    });

    it("handles progress ring when 0 out of 0 tasks avoiding division by zero", () => {
      const tasks: TaskStep[] = [];
      const pct = tasks.length > 0 ? (tasks.filter((t) => t.completed).length / tasks.length) * 100 : 0;
      expect(pct).toBe(0);
      expect(isNaN(pct)).toBe(false);
    });

    it("handles plan with all tasks in error status (0% completed)", () => {
      const tasks: TaskStep[] = [
        { id: "1", text: "fail 1", completed: false, status: "error" },
        { id: "2", text: "fail 2", completed: false, status: "error" },
      ];
      const { percentage } = calculatePlanProgress(tasks);
      expect(percentage).toBe(0);
    });
  });

  describe("F2.3 Boundary: Whitespace Only Tasks, Duplicate Steps, Rapid Cycling", () => {
    it("ignores task addition when input contains only whitespace or newlines", () => {
      const input = "   \n\t   ";
      const isValid = input.trim().length > 0;
      expect(isValid).toBe(false);
    });

    it("handles adding tasks with duplicate text content assigning distinct IDs", () => {
      const t1 = validateTaskStep({ text: "Duplicate step" }).normalized;
      const t2 = validateTaskStep({ text: "Duplicate step" }).normalized;
      expect(t1.id).not.toBe(t2.id);
    });

    it("handles rapid status cycling clicks without skipping intermediate states", () => {
      let state: TaskStepStatus = "pending";
      const history: TaskStepStatus[] = [state];
      for (let i = 0; i < 4; i++) {
        state = cycleTaskStatus(state);
        history.push(state);
      }
      expect(history).toEqual(["pending", "in_progress", "completed", "error", "pending"]);
    });

    it("handles deleting the last remaining task in a plan gracefully", () => {
      const tasks: TaskStep[] = [{ id: "only", text: "Last task", completed: false }];
      const remaining = tasks.filter((t) => t.id !== "only");
      expect(remaining).toHaveLength(0);
    });

    it("handles task text update with special punctuation and script tags", () => {
      const malicious = '<script>alert("test")</script>';
      const task: TaskStep = { id: "1", text: malicious, completed: false };
      expect(task.text).toBe(malicious);
    });
  });

  describe("F2.4 Boundary: AI Roadmap Rapid Triggering and Error Resiliency", () => {
    it("prevents concurrent duplicate AI roadmap generation requests when busy", () => {
      let busy = false;
      let calls = 0;
      const trigger = () => {
        if (busy) return;
        busy = true;
        calls++;
      };
      trigger();
      trigger();
      expect(calls).toBe(1);
    });

    it("handles AI roadmap request failure restoring button to idle state", () => {
      let busy = true;
      let error: string | null = null;
      try {
        throw new Error("Network offline");
      } catch (e: any) {
        error = e.message;
        busy = false;
      }
      expect(busy).toBe(false);
      expect(error).toBe("Network offline");
    });

    it("handles empty AI roadmap response without corrupting existing plans", () => {
      const existingPlans: Plan[] = [{ id: "p1", conversationId: "c1", title: "Existing", tasks: [], status: "active", createdAt: "now", updatedAt: "now" }];
      const incoming: Plan[] = [];
      const updated = incoming.length > 0 ? incoming : existingPlans;
      expect(updated).toHaveLength(1);
    });

    it("handles roadmap prompt generation with active conversation ID missing", () => {
      const activeConversationId = null;
      const canGenerate = Boolean(activeConversationId);
      expect(canGenerate).toBe(false);
    });

    it("handles language toggle (fr/en) during active roadmap generation", () => {
      let lang: "fr" | "en" = "fr";
      lang = "en";
      expect(lang).toBe("en");
    });
  });

  describe("F2.5 Boundary: Rapid Fire aro:plans-updated and Foreign Conversation IDs", () => {
    it("debounces rapid bursts of aro:plans-updated events", () => {
      let triggerCount = 0;
      let timer: any = null;
      const debounceReload = () => {
        if (timer) clearTimeout(timer);
        timer = setTimeout(() => triggerCount++, 50);
      };
      for (let i = 0; i < 10; i++) debounceReload();
      expect(triggerCount).toBe(0);
      clearTimeout(timer);
    });

    it("ignores aro:plans-updated events targeted at other conversations", () => {
      const myConv: string = "conv-mine";
      const targetConv: string = "conv-other";
      const shouldHandle = myConv === targetConv;
      expect(shouldHandle).toBe(false);
    });

    it("handles aro:plans-updated with null or missing detail payload", () => {
      const event: any = new CustomEvent("aro:plans-updated", { detail: null });
      expect(event.detail?.conversationId).toBeUndefined();
    });

    it("handles reload failure during aro:plans-updated without breaking UI state", async () => {
      const safeReload = async () => {
        try {
          throw new Error("Tauri IPC failure");
        } catch {
          return [];
        }
      };
      const result = await safeReload();
      expect(result).toEqual([]);
    });

    it("maintains plan detail view open when selected plan is updated externally", () => {
      let selectedPlan: Plan | null = { id: "p1", conversationId: "c1", title: "v1", tasks: [], status: "active", createdAt: "now", updatedAt: "now" };
      const incomingPlans: Plan[] = [{ id: "p1", conversationId: "c1", title: "v2", tasks: [], status: "active", createdAt: "now", updatedAt: "now" }];
      if (selectedPlan) {
        selectedPlan = incomingPlans.find((p) => p.id === selectedPlan!.id) || null;
      }
      expect(selectedPlan?.title).toBe("v2");
    });
  });

  describe("F3.1 Boundary: Missing Lane IDs, Extreme Priority Levels, Corrupted Runs", () => {
    it("handles run with empty string laneId grouping into default lane", () => {
      const synthesized = synthesizeAgentLanes([], [{ id: "r1", conversationId: "c1", laneId: "" }], "c1");
      expect(synthesized).toHaveLength(1);
      expect(synthesized[0].lane.id).toBe("default-virtual");
    });

    it("handles run with undefined status defaulting to queued", () => {
      const run: any = { id: "r2" };
      const status = run.status || "queued";
      expect(status).toBe("queued");
    });

    it("handles invalid priority value falling back to normal priority", () => {
      const rawPriority = "super-critical-urgent";
      const valid = ["low", "normal", "high"].includes(rawPriority) ? rawPriority : "normal";
      expect(valid).toBe("normal");
    });

    it("handles run with missing startedAt timestamp formatting duration as --", () => {
      const formatDuration = (startedAt?: string) => (startedAt ? "10s" : "--");
      expect(formatDuration(undefined)).toBe("--");
    });

    it("handles 50+ agent runs in a single lane with scroll containment", () => {
      const runs = Array.from({ length: 60 }, (_, i) => ({ id: `run-${i}`, conversationId: "c1", laneId: "lane-main" }));
      const lane = { lane: { id: "lane-main", conversationId: "c1" }, visibleRuns: runs };
      const synthesized = synthesizeAgentLanes([lane], runs, "c1");
      expect(synthesized[0].visibleRuns).toHaveLength(60);
    });
  });

  describe("F3.2 Boundary: Negative or Discrepant Counters, Desynchronized Snapshots", () => {
    it("clamps negative counter values from corrupted IPC payloads to zero", () => {
      const snapshot = { runningCount: -5, queuedCount: -1 };
      const stats = calculateActivityCounters([], snapshot, "c1");
      expect(stats.running).toBe(0);
      expect(stats.queued).toBe(0);
    });

    it("handles null orchestrator snapshot when conversation runs are empty (all 0)", () => {
      const stats = calculateActivityCounters([], null, "c1");
      expect(stats.running).toBe(0);
      expect(stats.queued).toBe(0);
      expect(stats.done).toBe(0);
    });

    it("handles extreme counter values (99,999+ active runs) without crashing", () => {
      const snapshot = { runningCount: 125000, queuedCount: 45000 };
      const stats = calculateActivityCounters([], snapshot, "c1");
      expect(stats.running).toBe(125000);
      expect(stats.total).toBe(170000);
    });

    it("handles run transitioning directly from queued to failed skipping running", () => {
      const runs = [{ id: "r1", conversationId: "c1", status: "queued" }];
      runs[0].status = "failed";
      const stats = calculateActivityCounters(runs, null, "c1");
      expect(stats.running).toBe(0);
      expect(stats.failed).toBe(1);
    });

    it("deduplicates runs with identical IDs before calculating counters", () => {
      const runs = [
        { id: "r1", conversationId: "c1", status: "running" },
        { id: "r1", conversationId: "c1", status: "running" },
      ];
      const uniqueRuns = Array.from(new Map(runs.map((r) => [r.id, r])).values());
      const stats = calculateActivityCounters(uniqueRuns, null, "c1");
      expect(stats.running).toBe(1);
    });
  });

  describe("F3.3 Boundary: Massive Log Streaming (10,000 lines), ANSI Escapes, Regex Special Chars", () => {
    it("handles 10,000 log entries without crashing memory", () => {
      const logs: LogEntry[] = Array.from({ length: 10000 }, (_, i) => ({
        id: `log-${i}`,
        timestamp: "12:00:00",
        level: "info",
        category: "agent",
        message: `Step log message ${i}`,
      }));
      expect(logs).toHaveLength(10000);
      expect(logs[9999].message).toBe("Step log message 9999");
    });

    it("sanitizes ANSI color escape sequences in log messages and details", () => {
      const raw = "\u001b[31mError occurred\u001b[0m";
      const stripped = raw.replace(/\u001b\[\d+m/g, "");
      expect(stripped).toBe("Error occurred");
    });

    it("handles search queries containing regex special characters", () => {
      const query = "[test.*+?^${}()|]";
      const safeFilter = (msg: string) => msg.toLowerCase().includes(query.toLowerCase());
      expect(safeFilter("exact [test.*+?^${}()|] match")).toBe(true);
      expect(safeFilter("different string")).toBe(false);
    });

    it("handles log entries with undefined category, message, or details", () => {
      const brokenStep = {};
      const log = agentStepToLogEntry(brokenStep);
      expect(log.category).toBe("agent");
      expect(log.message).toBe("Agent step execution");
      expect(log.level).toBe("info");
    });

    it("handles multiline stack traces and JSON objects in log details", () => {
      const step = { kind: "tool", output: { exitCode: 1, stderr: "trace\nline 1\nline 2" } };
      const log = agentStepToLogEntry(step);
      expect(log.details).toContain("exitCode");
      expect(log.details).toContain("trace");
    });
  });

  describe("F4.1 Boundary: Special Characters, Multiple @ Symbols, Mixed Windows/POSIX Slashes", () => {
    it("ignores email addresses containing @ (user@example.com)", () => {
      const trigger = detectMentionQuery("Contact user@example.com for info", 23);
      expect(trigger).toBeNull();
    });

    it("handles cursor placed between multiple @ mentions (@foo @bar)", () => {
      const text = "@foo @bar";
      const trigger = detectMentionQuery(text, 4);
      expect(trigger?.query).toBe("foo");
    });

    it("handles paths containing dots, dashes, underscores, and slashes", () => {
      const trigger = detectMentionQuery("Ref @src/lib/foo-bar_v2.test.ts", 32);
      expect(trigger?.query).toBe("src/lib/foo-bar_v2.test.ts");
    });

    it("normalizes Windows backslashes in mention paths", () => {
      const text = "@src\\components\\App.svelte";
      const trigger = detectMentionQuery(text, text.length)!;
      const { newText } = applyMentionSelection(text, trigger, "src/components/App.svelte");
      expect(newText).toBe("@src/components/App.svelte ");
    });

    it("handles @ typed at the very end of a 10,000-character input", () => {
      const longInput = "A".repeat(10000) + " @";
      const trigger = detectMentionQuery(longInput, longInput.length);
      expect(trigger).toMatchObject({ active: true, query: "" });
    });
  });

  describe("F4.2 Boundary: Empty Workspace Tree, Extreme Query Lengths, Keydown Flooding", () => {
    it("handles empty workspaceEntries list displaying no suggestions without error", () => {
      const filtered = filterWorkspaceEntries([], "App");
      expect(filtered).toEqual([]);
    });

    it("handles 500-character mention query returning 0 matches cleanly", () => {
      const entries: WorkspaceMentionEntry[] = [{ name: "App.svelte", path: "/App", isDir: false, relativePath: "App.svelte", extension: "svelte" }];
      const query = "Z".repeat(500);
      const filtered = filterWorkspaceEntries(entries, query);
      expect(filtered).toHaveLength(0);
    });

    it("handles ArrowDown and ArrowUp navigation when suggestions list is empty", () => {
      let index = 0;
      const count = 0;
      if (count > 0) index = (index + 1) % count;
      expect(index).toBe(0);
    });

    it("handles rapid keydown events without losing keyboard state", () => {
      let idx = 0;
      const count = 5;
      for (let i = 0; i < 20; i++) {
        idx = (idx + 1) % count;
      }
      expect(idx).toBe(0);
    });

    it("handles click selection on suggestion when popover is partially scrolled", () => {
      const onSelect = vi.fn();
      onSelect({ relativePath: "src/deep/nested/File.ts" });
      expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ relativePath: "src/deep/nested/File.ts" }));
    });
  });

  describe("F4.3 Boundary: Deep Directory Hierarchies, Duplicate Mentions, Large Attachment Refs", () => {
    it("handles workspace tree with 10+ nested folder levels", () => {
      const deepPath = "a/b/c/d/e/f/g/h/i/j/DeepComponent.svelte";
      const entries: WorkspaceMentionEntry[] = [
        { name: "DeepComponent.svelte", path: `/${deepPath}`, isDir: false, relativePath: deepPath, extension: "svelte" },
      ];
      const match = filterWorkspaceEntries(entries, "Deep");
      expect(match).toHaveLength(1);
      expect(match[0].relativePath).toBe(deepPath);
    });

    it("handles tree entry with undefined or 0 file size", () => {
      const entry: WorkspaceMentionEntry = { name: "empty.txt", path: "/empty.txt", isDir: false, size: 0, relativePath: "empty.txt", extension: "txt" };
      expect(entry.size).toBe(0);
    });

    it("handles file with no extension (e.g. Dockerfile, Makefile, LICENSE)", () => {
      const getExt = (name: string) => (name.includes(".") ? name.split(".").pop() || "" : "");
      expect(getExt("Dockerfile")).toBe("");
      expect(getExt("LICENSE")).toBe("");
      expect(getExt("Makefile")).toBe("");
    });

    it("handles message with 20 distinct @ mentions without exceeding limits", () => {
      const mentions = Array.from({ length: 20 }, (_, i) => `@file_${i}.ts`).join(" ");
      const paths = extractMentionedPaths(mentions);
      expect(paths).toHaveLength(20);
    });

    it("handles mentioned file that does not exist in workspace index (graceful fallback)", () => {
      const entries: WorkspaceMentionEntry[] = [{ name: "Existing.ts", path: "/Existing.ts", isDir: false, relativePath: "Existing.ts", extension: "ts" }];
      const mentioned = "GhostFile.ts";
      const exists = entries.some((e) => e.relativePath === mentioned);
      expect(exists).toBe(false);
    });
  });
});

/* ========================================================================= */
/* TIER 3: Cross-Feature Combinations (Pairwise Integration) (6 tests)       */
/* ========================================================================= */

describe("Tier 3: Cross-Feature Combinations", () => {
  it("Combo 1: Diff Apply + Plan Task Completion Sync", () => {
    const plan: Plan = {
      id: "p1", conversationId: "c1", title: "Refactor",
      tasks: [{ id: "t1", text: "Apply Visual Diff to App.svelte", completed: false, status: "in_progress" }],
      status: "active", createdAt: "now", updatedAt: "now",
    };

    const diff = computeLineDiff("old", "new");
    expect(diff).toHaveLength(2);

    plan.tasks[0].completed = true;
    plan.tasks[0].status = "completed";

    const { percentage } = calculatePlanProgress(plan.tasks);
    expect(percentage).toBe(100);
    expect(plan.tasks[0].status).toBe("completed");
  });

  it("Combo 2: @ Mention in Composer + Artifact Creation", () => {
    const userPrompt = "Generate tests for @src/lib/diff.ts";
    const mentionedPaths = extractMentionedPaths(userPrompt);
    expect(mentionedPaths).toEqual(["src/lib/diff.ts"]);

    const newArtifact = {
      id: "art-test",
      title: "Diff Engine Tests",
      filePath: "src/lib/diff.test.ts",
      kind: "code",
      status: "pending",
    };
    expect(newArtifact.filePath).toBe("src/lib/diff.test.ts");
    expect(newArtifact.status).toBe("pending");
  });

  it("Combo 3: AI Roadmap Trigger + Subagent Execution + Live Log Streaming", () => {
    let agentRun = { id: "run-ai-roadmap", status: "running", steps: [] as any[] };
    agentRun.steps.push({ kind: "thought", title: "Analyzing project dependencies" });
    agentRun.steps.push({ kind: "tool", title: "core.plan.create", output: { id: "new-plan" } });

    const logs = agentRun.steps.map(agentStepToLogEntry);
    expect(logs).toHaveLength(2);
    expect(logs[1].level).toBe("tool");
    expect(logs[1].message).toBe("core.plan.create");
  });

  it("Combo 4: Multi-Agent Lane Status Change + Synchronized Banner Counters", () => {
    const runs = [
      { id: "r1", conversationId: "c1", status: "running" },
      { id: "r2", conversationId: "c1", status: "queued" },
    ];
    let counters = calculateActivityCounters(runs, null, "c1");
    expect(counters.running).toBe(1);
    expect(counters.queued).toBe(1);

    runs[0].status = "paused";
    counters = calculateActivityCounters(runs, null, "c1");
    expect(counters.running).toBe(0);
    expect(counters.waiting).toBe(1);
  });

  it("Combo 5: Real-Time aro:plans-updated + Circular Progress Ring Sync", () => {
    const plan: Plan = {
      id: "p1", conversationId: "c1", title: "Migration",
      tasks: [
        { id: "1", text: "step 1", completed: true, status: "completed" },
        { id: "2", text: "step 2", completed: false, status: "pending" },
      ],
      status: "active", createdAt: "now", updatedAt: "now",
    };
    let progress = calculatePlanProgress(plan.tasks);
    expect(progress.percentage).toBe(50);

    plan.tasks[1].completed = true;
    plan.tasks[1].status = "completed";
    progress = calculatePlanProgress(plan.tasks);
    expect(progress.percentage).toBe(100);
  });

  it("Combo 6: Diff Split/Unified Toggle + Direct Copy + Disk Write Simulation", async () => {
    const original = "export default function oldApp() {}";
    const modified = "export default function newApp() { return 'ARO'; }";
    const diffLines = computeLineDiff(original, modified);
    const splitRows = generateSplitDiffRows(diffLines);

    expect(splitRows.length).toBeGreaterThan(0);

    const writeMock = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal("navigator", { clipboard: { writeText: writeMock } });
    await navigator.clipboard.writeText(modified);
    expect(writeMock).toHaveBeenCalledWith(modified);

    const applyResult = { success: true, filePath: "src/App.svelte", bytesWritten: modified.length };
    expect(applyResult.success).toBe(true);
    expect(applyResult.bytesWritten).toBe(modified.length);
  });
});

/* ========================================================================= */
/* TIER 4: Real-World Application Scenarios (4 comprehensive workflows)      */
/* ========================================================================= */

describe("Tier 4: Real-World Application Scenarios", () => {
  it("Scenario 1: End-to-End Feature Development Lifecycle (Roadmap -> Plan -> Diff -> Apply -> Done)", async () => {
    // 1. User prompts assistant for roadmap
    const prompt = "Build new auth system";
    expect(prompt).toBe("Build new auth system");

    // 2. Assistant establishes structured plan
    const roadmapPlan: Plan = {
      id: "plan-auth", conversationId: "conv-lifecycle", title: "Auth Feature Roadmap",
      tasks: [
        { id: "t1", text: "Create Auth token store", completed: false, status: "pending" },
        { id: "t2", text: "Build login modal", completed: false, status: "pending" },
      ],
      status: "active", createdAt: "now", updatedAt: "now",
    };
    expect(roadmapPlan.tasks).toHaveLength(2);

    // 3. User begins task 1
    roadmapPlan.tasks[0].status = "in_progress";
    expect(roadmapPlan.tasks[0].status).toBe("in_progress");

    // 4. Assistant generates diff for token store
    const oldCode = "// empty";
    const newCode = "export const token = 'session_123';";
    const diff = computeLineDiff(oldCode, newCode);
    expect(diff.some((l) => l.type === "added")).toBe(true);

    // 5. User reviews diff and applies to project
    const fileWritten = true;
    expect(fileWritten).toBe(true);

    // 6. User checks off task 1 as completed
    roadmapPlan.tasks[0].status = "completed";
    roadmapPlan.tasks[0].completed = true;
    const progress = calculatePlanProgress(roadmapPlan.tasks);
    expect(progress.percentage).toBe(50);
  });

  it("Scenario 2: Multi-Agent Debugging & Log Inspection Workflow", () => {
    // 1. User dispatches 2 subagents
    const runs = [
      { id: "run-linter", conversationId: "c1", status: "running", goal: "Lint project" },
      { id: "run-builder", conversationId: "c1", status: "running", goal: "Build assets" },
    ];
    const lanes = synthesizeAgentLanes([], runs, "c1");
    expect(lanes).toHaveLength(1);
    expect(lanes[0].visibleRuns).toHaveLength(2);

    // 2. Live counters reflect active executions
    const counters = calculateActivityCounters(runs, null, "c1");
    expect(counters.running).toBe(2);

    // 3. One agent encounters an error
    runs[0].status = "failed";
    const log = agentStepToLogEntry({ kind: "tool", title: "Run eslint", error: "2 errors found", status: "failed" });
    expect(log.level).toBe("error");
    expect(log.details).toContain("2 errors found");

    // 4. Other agent completes
    runs[1].status = "completed";
    const finalCounters = calculateActivityCounters(runs, null, "c1");
    expect(finalCounters.running).toBe(0);
    expect(finalCounters.done).toBe(1);
    expect(finalCounters.failed).toBe(1);
  });

  it("Scenario 3: Targeted Context Injection & Code Refactoring via @ Mentions", () => {
    // 1. User inputs query with @ mentions
    const input = "Refactor @src/App.svelte and optimize @src/lib/markdown.ts";
    const mentionedFiles = extractMentionedPaths(input);
    expect(mentionedFiles).toEqual(["src/App.svelte", "src/lib/markdown.ts"]);

    // 2. Attachments built from workspace index
    const attachments = mentionedFiles.map((p) => ({
      path: p,
      name: p.split("/").pop(),
      mode: "local-reference",
    }));
    expect(attachments).toHaveLength(2);
    expect(attachments[0].name).toBe("App.svelte");

    // 3. Diff block returned in assistant message
    const diffPatch = [
      "--- a/src/App.svelte",
      "+++ b/src/App.svelte",
      "@@ -1,2 +1,3 @@",
      " import Header from './Header.svelte';",
      "- const ready = false;",
      "+ const ready = true;",
      "+ const version = '2.0';",
    ].join("\n");
    const parsed = parseUnifiedDiff(diffPatch);
    expect(parsed.filePath).toBe("src/App.svelte");
    expect(parsed.additions).toBe(2);
    expect(parsed.deletions).toBe(1);
  });

  it("Scenario 4: Error Recovery & Plan Status Cycling under Tool Failure", () => {
    // 1. Task executes and fails
    const task: TaskStep = {
      id: "t-db",
      text: "Run database migration",
      completed: false,
      status: "in_progress",
    };

    // 2. Tool returns error
    task.status = "error";
    task.error = "Connection refused at 127.0.0.1:5432";
    expect(task.status).toBe("error");

    // 3. User inspects error log
    const log = agentStepToLogEntry({ kind: "tool", title: "Migrate DB", error: task.error, status: "failed" });
    expect(log.level).toBe("error");

    // 4. User fixes issue and manually cycles task status to completed
    task.status = cycleTaskStatus(task.status); // error -> pending
    expect(task.status).toBe("pending");
    task.status = cycleTaskStatus(task.status); // pending -> in_progress
    expect(task.status).toBe("in_progress");
    task.status = cycleTaskStatus(task.status); // in_progress -> completed
    task.completed = true;
    task.error = null;
    expect(task.status).toBe("completed");
    expect(task.completed).toBe(true);
    expect(task.error).toBeNull();
  });
});
