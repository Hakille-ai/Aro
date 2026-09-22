<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import FileText from "@lucide/svelte/icons/file-text";
  import Bot from "@lucide/svelte/icons/bot";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import ClipboardList from "@lucide/svelte/icons/clipboard-list";
  import Globe from "@lucide/svelte/icons/globe";
  import BookOpen from "@lucide/svelte/icons/book-open";
  import Search from "@lucide/svelte/icons/search";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import Activity from "@lucide/svelte/icons/activity";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import Brain from "@lucide/svelte/icons/brain";
  import Cpu from "@lucide/svelte/icons/cpu";
  import User from "@lucide/svelte/icons/user";
  import Sliders from "@lucide/svelte/icons/sliders";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import CheckSquare from "@lucide/svelte/icons/check-square";
  import Square from "@lucide/svelte/icons/square";
  import ExternalLink from "@lucide/svelte/icons/external-link";
  import Info from "@lucide/svelte/icons/info";
  import Archive from "@lucide/svelte/icons/archive";
  import Check from "@lucide/svelte/icons/check";
  import ListTodo from "@lucide/svelte/icons/list-todo";
  import FileCode from "@lucide/svelte/icons/file-code";
  import Eye from "@lucide/svelte/icons/eye";
  import Copy from "@lucide/svelte/icons/copy";
  import Circle from "@lucide/svelte/icons/circle";
  import Loader2 from "@lucide/svelte/icons/loader-2";
  import CheckCircle2 from "@lucide/svelte/icons/check-circle-2";
  import AlertCircle from "@lucide/svelte/icons/alert-circle";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import { onMount } from "svelte";
  import { listPlans, createPlan, updatePlan, deletePlan, writeWorkspaceFile, applyWorkspaceDiff, getWorkspaceTree, readWorkspaceFile } from "../../lib/api/transport";
  import type { Plan, TaskStep, WorkspaceTreeEntry } from "../../lib/api/transport";
  import { calculatePlanProgress, cycleTaskStatus, validateTaskStep, formatTaskError } from "../../lib/plan-utils";
  import { agentStepToLogEntry } from "../../lib/agent-utils";
  import { requestConfirm } from "../../lib/confirm";
  import WorkspaceTreeExplorer from "../workspace/WorkspaceTreeExplorer.svelte";
  import WorkspaceFileViewer from "../workspace/WorkspaceFileViewer.svelte";
  import AgentLiveLogViewer from "../workspace/AgentLiveLogViewer.svelte";
  import CodeDiffViewer from "../workspace/CodeDiffViewer.svelte";
  import IntegratedBrowser from "../browser/IntegratedBrowser.svelte";
  import {
    navigateBrowser,
    takeBrowserControl,
    createBrowserTab,
    browserTabs,
    switchBrowserTab,
  } from "../browser/browser-store";
  import { get } from "svelte/store";
  import { computeLineDiff, parseUnifiedDiff, type DiffLine } from "../../lib/diff";


  // Types & props
  export let activeConversationId: string | null = null;
  export let language: "fr" | "en" = "fr";
  export let width: number;
  export let onClose: () => void;

  export let artifacts: any[] = [];
  export let contextSources: any[] = [];
  export let visibleSources: any[] = [];
  export let attachedFiles: any[] = [];
  export let agentInboxTotals: any = { running: 0, queued: 0, waiting: 0, done: 0 };
  export let agentInboxLanes: any[] = [];
  export let agentRunsBusy: any = false;
  export let agentActionBusy: string | null = null;
  export let personalities: any[] = [];
  export let plans: Plan[] = [];
  export let selectedPlan: Plan | null = null;
  export let onRequestAiPlan: ((prompt: string) => void | Promise<void>) | undefined = undefined;
  export let aiPlanBusy: boolean = false;
  export let selectedAgentRunView: any = null;

  let localAiPlanBusy = false;
  $: isAiPlanBusy = aiPlanBusy || localAiPlanBusy;
  $: selectedPlanProgress = calculatePlanProgress(selectedPlan?.tasks);

  let clearedStepCounts = new Map<string, number>();
  let selectedRunLogs: any[] = [];
  $: {
    const runId = selectedAgentRunView?.run?.id;
    const allSteps = selectedAgentRunView?.steps || [];
    const clearedCount = runId ? (clearedStepCounts.get(runId) || 0) : 0;
    selectedRunLogs = allSteps.slice(clearedCount).map(agentStepToLogEntry);
  }

  function handleClearRunLogs() {
    const runId = selectedAgentRunView?.run?.id;
    if (runId) {
      const stepCount = (selectedAgentRunView?.steps || []).length;
      clearedStepCounts = new Map(clearedStepCounts).set(runId, stepCount);
    }
  }


  // Callbacks
  export let onToggleAgentLane: (laneId: string) => void;
  export let onSetAgentLaneAction: (laneId: string, action: "pause" | "resume") => void | Promise<void>;
  export let onUpdateAgentLanePriority: (laneId: string, priority: any) => void | Promise<void>;
  export let onOpenAgentRun: (runId: string) => void;
  export let onPreviewFile: (fileObj: any) => void;
  export let onStartAgentChat: (personalityId: string) => void | Promise<void>;
  export let onStartCreateAgent: () => void;
  export let onStartEditAgent: (agent: any) => void;
  export let onDeleteAgent: (id: string) => void | Promise<void>;

  // Local component states
  let agentsSubTab: "lanes" | "custom" = "lanes";
  let editingAgent: any = null;

  async function handleDeleteAgent(id: string) {
    const confirmed = await requestConfirm({
      title: language === "fr" ? "Supprimer cet agent ?" : "Delete this agent?",
      confirmLabel: language === "fr" ? "Supprimer" : "Delete",
      cancelLabel: language === "fr" ? "Annuler" : "Cancel",
    });
    if (confirmed) {
      onDeleteAgent(id);
    }
  }
  type TabKey = "none" | "outputs" | "agents" | "explorer" | "plan" | "browser" | "sources";
  let activeTab: TabKey = "none";

  // Search and filter for sources tab
  let sourcesSearchQuery = "";
  let sourcesFilter: "all" | "web" | "files" | "memory" = "all";
  let copiedSourceUrl: string | null = null;

  // Search filter for explorer
  let explorerSearchQuery = "";

  // Active workspace file (click a file in the explorer -> preview on the right,
  // always showing the file and the root it comes from).
  interface ActiveWorkspaceFileState {
    relativePath: string;
    rootPath: string;
  }
  const WORKSPACE_PREVIEW_MAX_CHARS = 200_000;
  let activeWorkspaceFile: ActiveWorkspaceFileState | null = null;
  let workspaceFileContent = "";
  let workspaceFileLoading = false;
  let workspaceFileError: string | null = null;
  let workspaceFileTruncated = false;

  async function openWorkspaceFile(entry: WorkspaceTreeEntry, rootPath: string) {
    const relativePath = entry.relativePath || entry.name;
    activeWorkspaceFile = { relativePath, rootPath };
    activeTab = "explorer";
    workspaceFileLoading = true;
    workspaceFileError = null;
    workspaceFileContent = "";
    workspaceFileTruncated = false;
    if (typeof window !== "undefined" && width < 760) {
      width = Math.min(Math.max(window.innerWidth - 350, 600), 850);
    }
    try {
      const text = await readWorkspaceFile(relativePath, activeConversationId || undefined);
      if (text.length > WORKSPACE_PREVIEW_MAX_CHARS) {
        workspaceFileContent = text.slice(0, WORKSPACE_PREVIEW_MAX_CHARS);
        workspaceFileTruncated = true;
      } else {
        workspaceFileContent = text;
      }
    } catch (err) {
      console.error("Failed to read workspace file:", err);
      workspaceFileError =
        language === "fr"
          ? `Impossible d'ouvrir « ${relativePath} » (${err instanceof Error ? err.message : String(err)})`
          : `Could not open "${relativePath}" (${err instanceof Error ? err.message : String(err)})`;
    } finally {
      workspaceFileLoading = false;
    }
  }

  function closeWorkspaceFile() {
    activeWorkspaceFile = null;
    workspaceFileContent = "";
    workspaceFileError = null;
    workspaceFileTruncated = false;
  }

  function examineWorkspaceFile(relativePath: string) {
    try {
      window.dispatchEvent(
        new CustomEvent("aro:examine-workspace-file", { detail: { path: relativePath } })
      );
    } catch {
      // Non-fatal: l'événement est un bonus pour le composer.
    }
    try {
      void navigator.clipboard?.writeText(`@${relativePath}`);
    } catch {
      // Presse-papiers indisponible : on ignore.
    }
  }

  async function openWorkspaceFileByPath(relativePath: string) {
    try {
      const tree = await getWorkspaceTree(activeConversationId || undefined);
      const found = (tree.entries || []).find(
        (e: WorkspaceTreeEntry) => (e.relativePath || e.name) === relativePath && !e.isDir
      );
      // Même si l'entrée n'est plus dans l'arbre, on tente la lecture :
      // le backend renverra une erreur propre affichée dans le viewer.
      await openWorkspaceFile(
        found ?? { name: relativePath, path: relativePath, relativePath, isDir: false, size: 0 },
        tree.rootPath
      );
    } catch (err) {
      console.error("Failed to open workspace file by path:", err);
      activeWorkspaceFile = { relativePath, rootPath: "" };
      workspaceFileLoading = false;
      workspaceFileContent = "";
      workspaceFileTruncated = false;
      workspaceFileError =
        language === "fr"
          ? `Impossible de charger « ${relativePath} » (${err instanceof Error ? err.message : String(err)})`
          : `Could not load "${relativePath}" (${err instanceof Error ? err.message : String(err)})`;
    }
  }

  function handleAroOpenWorkspaceFile(event: Event) {
    const detail = (event as CustomEvent).detail as { path?: string; relativePath?: string } | undefined;
    const p = detail?.path ?? detail?.relativePath;
    if (p) void openWorkspaceFileByPath(p);
  }

  onMount(() => {
    window.addEventListener("aro:open-workspace-file", handleAroOpenWorkspaceFile);
    const handleAgentBrowserStep = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail && detail.url) {
        const tabs = get(browserTabs);
        const existing = tabs.find((t) => t.url === detail.url);
        if (existing) {
          browserTabs.update((all) =>
            all.map((t) =>
              t.id === existing.id
                ? {
                    ...t,
                    isAiControlled: true,
                    aiStatusMessage: detail.stepTitle || (language === "fr" ? "L'IA analyse cette page..." : "AI is analyzing this page..."),
                    extractedContent: detail.content || t.extractedContent,
                  }
                : t
            )
          );
        } else {
          createBrowserTab(
            detail.url,
            detail.title,
            true,
            detail.stepTitle || (language === "fr" ? "L'IA analyse cette page..." : "AI is analyzing this page...")
          );
        }
      }
    };
    window.addEventListener("aro:agent-browser-step", handleAgentBrowserStep);

    const handleOpenBrowser = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail) {
        activeTab = "browser";
        const isTakeControl = Boolean(detail.takeControl);
        if (detail.url) {
          if (detail.newTab) {
            createBrowserTab(detail.url, detail.title, !isTakeControl);
          } else {
            const tabs = get(browserTabs);
            const existing = tabs.find((t) => t.url === detail.url);
            if (existing) {
              switchBrowserTab(existing.id);
            } else {
              navigateBrowser(detail.url, !isTakeControl, detail.aiStatus);
            }
          }
        }
        if (isTakeControl) {
          takeBrowserControl();
        }
        if (typeof window !== "undefined" && width < 700) {
          width = Math.min(Math.max(window.innerWidth - 350, 650), 850);
        }
      }
    };
    window.addEventListener("aro:open-browser", handleOpenBrowser);
    return () => {
      window.removeEventListener("aro:open-workspace-file", handleAroOpenWorkspaceFile);
      window.removeEventListener("aro:open-browser", handleOpenBrowser);
      window.removeEventListener("aro:agent-browser-step", handleAgentBrowserStep);
    };
  });

  // Outputs & Artifacts Tab state
  let selectedArtifact: any | null = null;
  let artifactSearchQuery = "";
  let artifactStatusFilter: "all" | "pending" | "applied" | "rejected" = "all";
  let applyingArtifactId: string | null = null;
  let copiedArtifactId: string | null = null;

  function getArtifactKey(artifact: any, index: number): string {
    return artifact?.id ? `${artifact.id}-${index}` : `artifact-${index}`;
  }

  function getArtifactTitle(artifact: any): string {
    if (!artifact) return language === "fr" ? "Sans titre" : "Untitled";
    return artifact.title || artifact.filePath || (language === "fr" ? "Sans titre" : "Untitled");
  }

  function getArtifactExtension(pathOrArtifact: any): string {
    const p = typeof pathOrArtifact === "string" ? pathOrArtifact : (pathOrArtifact?.filePath || pathOrArtifact?.title || "");
    if (!p) return "";
    const parts = p.split(".");
    return parts.length > 1 ? parts.pop()!.toLowerCase() : "";
  }

  function cleanDelta(val: any): number {
    return typeof val === "number" && !isNaN(val) && val >= 0 ? val : 0;
  }

  function getArtifactDeltas(artifact: any): { additions: number; deletions: number } {
    if (!artifact) return { additions: 0, deletions: 0 };
    if (artifact.additions !== undefined || artifact.deletions !== undefined) {
      return {
        additions: cleanDelta(artifact.additions),
        deletions: cleanDelta(artifact.deletions),
      };
    }
    if (artifact.additionsCount !== undefined || artifact.deletionsCount !== undefined) {
      return {
        additions: cleanDelta(artifact.additionsCount),
        deletions: cleanDelta(artifact.deletionsCount),
      };
    }
    if (artifact.diffLines && Array.isArray(artifact.diffLines) && artifact.diffLines.length > 0) {
      return {
        additions: artifact.diffLines.filter((l: any) => l && l.type === "added").length,
        deletions: artifact.diffLines.filter((l: any) => l && l.type === "removed").length,
      };
    }
    if (artifact.diffText) {
      try {
        const p = parseUnifiedDiff(artifact.diffText);
        return { additions: cleanDelta(p.additions), deletions: cleanDelta(p.deletions) };
      } catch {
        return { additions: 0, deletions: 0 };
      }
    }
    if (artifact.originalContent !== undefined && artifact.modifiedContent !== undefined) {
      try {
        const lines = computeLineDiff(artifact.originalContent, artifact.modifiedContent);
        return {
          additions: lines.filter((l: any) => l && l.type === "added").length,
          deletions: lines.filter((l: any) => l && l.type === "removed").length,
        };
      } catch {
        return { additions: 0, deletions: 0 };
      }
    }
    const content = artifact.modifiedContent ?? artifact.content ?? "";
    const lineCount = content ? content.split("\n").length : 0;
    return { additions: lineCount, deletions: 0 };
  }

  function getArtifactDiffLines(artifact: any): DiffLine[] {
    if (!artifact) return [];
    if (artifact.diffLines && Array.isArray(artifact.diffLines) && artifact.diffLines.length > 0) {
      return artifact.diffLines;
    }
    if (artifact.diffText) {
      try {
        return parseUnifiedDiff(artifact.diffText).diffLines;
      } catch (err) {
        console.error("Failed to parse diffText:", err);
      }
    }
    if (artifact.originalContent !== undefined && artifact.modifiedContent !== undefined) {
      try {
        return computeLineDiff(artifact.originalContent, artifact.modifiedContent);
      } catch (err) {
        console.error("Failed to computeLineDiff:", err);
      }
    }
    const content = artifact.modifiedContent ?? artifact.content ?? "";
    if (content) {
      try {
        return computeLineDiff("", content);
      } catch (err) {
        console.error("Failed to computeLineDiff from content:", err);
      }
    }
    return [];
  }

  $: filteredArtifacts = (artifacts || []).filter((art: any) => {
    if (!art) return false;
    if (artifactStatusFilter !== "all") {
      const st = art.status || "pending";
      if (st !== artifactStatusFilter) return false;
    }
    if (artifactSearchQuery.trim()) {
      const q = artifactSearchQuery.toLowerCase();
      const title = (art.title || "").toLowerCase();
      const path = (art.filePath || "").toLowerCase();
      return title.includes(q) || path.includes(q);
    }
    return true;
  });

  async function handleApplyArtifact(artifact: any) {
    if (!artifact) return;
    const path = artifact.filePath;
    if (!path) {
      console.warn("L'artefact ne possède pas de chemin de fichier (filePath)");
      return;
    }
    applyingArtifactId = artifact.id || path;
    try {
      if (artifact.diffText) {
        await applyWorkspaceDiff(path, artifact.diffText, activeConversationId || undefined);
      } else {
        const content = artifact.modifiedContent !== undefined 
          ? artifact.modifiedContent 
          : (artifact.content !== undefined ? artifact.content : "");
        await writeWorkspaceFile(path, content, activeConversationId || undefined);
      }
      artifact.status = "applied";
      artifacts = [...artifacts];
      if (selectedArtifact && (selectedArtifact.id === artifact.id || selectedArtifact.filePath === artifact.filePath)) {
        selectedArtifact.status = "applied";
        selectedArtifact = { ...selectedArtifact };
      }
      if (typeof window !== "undefined") {
        window.dispatchEvent(new CustomEvent("aro:workspace-tree-refresh", { detail: { path } }));
      }
    } catch (err) {
      console.error("Erreur lors de l'application de l'artefact sur disque :", err);
    } finally {
      applyingArtifactId = null;
    }
  }

  function handleArtifactApplied(artifact: any) {
    if (!artifact) return;
    artifact.status = "applied";
    artifacts = [...artifacts];
    if (selectedArtifact && (selectedArtifact.id === artifact.id || selectedArtifact.filePath === artifact.filePath)) {
      selectedArtifact.status = "applied";
      selectedArtifact = { ...selectedArtifact };
    }
  }

  function handleRejectArtifact(artifact: any) {
    if (!artifact) return;
    artifact.status = "rejected";
    artifacts = [...artifacts];
    if (selectedArtifact && (selectedArtifact.id === artifact.id || selectedArtifact.filePath === artifact.filePath)) {
      selectedArtifact.status = "rejected";
      selectedArtifact = { ...selectedArtifact };
    }
  }

  async function handleCopyArtifact(artifact: any) {
    if (!artifact) return;
    const text = artifact.modifiedContent 
      ?? artifact.content 
      ?? (artifact.diffLines ? artifact.diffLines.filter((l: any) => l && l.type !== "removed").map((l: any) => l.content).join("\n") : "")
      ?? artifact.diffText 
      ?? "";
    try {
      if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        copiedArtifactId = artifact.id || artifact.filePath || "copied";
        setTimeout(() => {
          if (copiedArtifactId === (artifact.id || artifact.filePath || "copied")) {
            copiedArtifactId = null;
          }
        }, 2000);
      }
    } catch (err) {
      console.warn("Échec de copie presse-papier :", err);
    }
  }

  // Plan todo list state
  let showNewPlanForm = false;
  let newPlanTitle = "";
  let newPlanDescription = "";
  let newPlanTasksText = ""; // Newline separated tasks
  let plansLoading = false;
  let newPlanTaskInput = "";
  let plansDebounceTimer: ReturnType<typeof setTimeout> | null = null;

  async function loadPlans() {
    if (!activeConversationId) {
      return;
    }
    plansLoading = true;
    try {
      plans = await listPlans(activeConversationId);
      if (selectedPlan) {
        // Refresh selected plan state
        const refreshed = plans.find(p => p.id === selectedPlan!.id);
        selectedPlan = refreshed || null;
      }
    } catch (e) {
      console.error("Failed to load plans:", e);
    } finally {
      plansLoading = false;
    }
  }

  function handlePlansUpdatedEvent(event: Event) {
    const customEvent = event as CustomEvent<{ conversationId?: string } | null>;
    const targetConv = customEvent.detail?.conversationId;
    if (targetConv && activeConversationId && targetConv !== activeConversationId) {
      return;
    }
    if (plansDebounceTimer) clearTimeout(plansDebounceTimer);
    plansDebounceTimer = setTimeout(() => {
      loadPlans();
    }, 50);
  }

  function notifyPlansUpdated() {
    if (typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent("aro:plans-updated", {
          detail: { conversationId: activeConversationId }
        })
      );
    }
  }

  async function handleTriggerAiPlan() {
    if (isAiPlanBusy) return;
    if (!activeConversationId) return;

    localAiPlanBusy = true;
    const prompt = language === "fr"
      ? "Élabore une feuille de route détaillée et structurée pour ce projet."
      : "Generate a detailed and structured roadmap for this project.";

    try {
      if (typeof window !== "undefined") {
        window.dispatchEvent(
          new CustomEvent("aro:request-ai-plan", {
            detail: { prompt, conversationId: activeConversationId }
          })
        );
      }
      if (onRequestAiPlan) {
        await onRequestAiPlan(prompt);
      }
    } catch (err) {
      console.error("Failed to request AI roadmap plan:", err);
    } finally {
      localAiPlanBusy = false;
    }
  }

  // Load plans on mount and register artifact and plan listeners
  onMount(() => {
    loadPlans();

    const handleSelectArtifact = (e: any) => {
      activeTab = "outputs";
      if (e.detail?.filePath || e.detail?.id) {
        const found = (artifacts || []).find((a: any) => 
          (e.detail.id && a.id === e.detail.id) || (e.detail.filePath && a.filePath === e.detail.filePath)
        );
        if (found) {
          selectedArtifact = found;
        }
      }
    };

    if (typeof window !== "undefined") {
      window.addEventListener("aro:select-artifact", handleSelectArtifact as EventListener);
      window.addEventListener("aro:plans-updated", handlePlansUpdatedEvent as EventListener);
      window.addEventListener("aro:open-workspace-file", handleAroOpenWorkspaceFile as EventListener);
    }

    return () => {
      if (typeof window !== "undefined") {
        window.removeEventListener("aro:select-artifact", handleSelectArtifact as EventListener);
        window.removeEventListener("aro:plans-updated", handlePlansUpdatedEvent as EventListener);
        window.removeEventListener("aro:open-workspace-file", handleAroOpenWorkspaceFile as EventListener);
      }
      if (plansDebounceTimer) clearTimeout(plansDebounceTimer);
    };
  });

  // Reload plans and reset selection when switching conversation
  let prevActiveConversationId: string | null | undefined = undefined;
  $: if (activeConversationId !== prevActiveConversationId) {
    prevActiveConversationId = activeConversationId;
    selectedPlan = null;
    if (activeConversationId) {
      loadPlans();
    } else {
      plans = [];
    }
    closeWorkspaceFile();
  }

  $: allTasks = plans.flatMap(p => p.tasks || []);
  $: completedTasksCount = allTasks.filter(t => t.completed || t.status === "completed").length;
  $: totalTasksCount = allTasks.length;

  async function handleCreatePlan() {
    if (!newPlanTitle.trim() || !activeConversationId) return;
    const taskLines = newPlanTasksText
      .split("\n")
      .map(line => line.trim())
      .filter(line => line.length > 0);
    const initialTasks: Omit<TaskStep, "id">[] = taskLines.map(text => ({
      text,
      completed: false,
      status: "pending"
    }));

    try {
      await createPlan({
        conversationId: activeConversationId,
        title: newPlanTitle.trim(),
        description: newPlanDescription.trim() || undefined,
        tasks: initialTasks as any
      });
      newPlanTitle = "";
      newPlanDescription = "";
      newPlanTasksText = "";
      showNewPlanForm = false;
      notifyPlansUpdated();
      await loadPlans();
    } catch (e) {
      console.error("Failed to create plan:", e);
    }
  }

  async function handleCycleTaskStatus(plan: Plan, taskId: string) {
    const updatedTasks = plan.tasks.map(t => {
      if (t.id !== taskId) return t;
      const currentStatus = t.status || (t.completed ? "completed" : "pending");
      const nextStatus = cycleTaskStatus(currentStatus);
      return {
        ...t,
        status: nextStatus,
        completed: nextStatus === "completed",
        error: nextStatus === "error" ? (t.error || (language === "fr" ? "Erreur d'exécution" : "Execution error")) : null,
      };
    });

    const updatedPlan: Plan = {
      ...plan,
      tasks: updatedTasks
    };

    if (selectedPlan && selectedPlan.id === plan.id) {
      selectedPlan = updatedPlan;
    }
    plans = plans.map(p => p.id === plan.id ? updatedPlan : p);

    try {
      await updatePlan(updatedPlan);
      notifyPlansUpdated();
      await loadPlans();
    } catch (e) {
      console.error("Failed to cycle task status:", e);
      await loadPlans();
    }
  }

  async function handleToggleTask(plan: Plan, taskId: string) {
    await handleCycleTaskStatus(plan, taskId);
  }

  async function handleAddTaskToPlan(plan: Plan) {
    const trimmed = newPlanTaskInput.trim();
    if (!trimmed) return;

    const { normalized } = validateTaskStep({
      id: crypto.randomUUID(),
      text: trimmed,
      completed: false,
      status: "pending",
    });

    const updatedPlan: Plan = {
      ...plan,
      tasks: [...plan.tasks, normalized]
    };
    newPlanTaskInput = "";

    if (selectedPlan && selectedPlan.id === plan.id) {
      selectedPlan = updatedPlan;
    }
    plans = plans.map(p => p.id === plan.id ? updatedPlan : p);

    try {
      await updatePlan(updatedPlan);
      notifyPlansUpdated();
      await loadPlans();
    } catch (e) {
      console.error("Failed to add task:", e);
      await loadPlans();
    }
  }

  async function handleDeleteTask(plan: Plan, taskId: string) {
    const updatedTasks = plan.tasks.filter(t => t.id !== taskId);
    const updatedPlan: Plan = {
      ...plan,
      tasks: updatedTasks
    };
    try {
      if (selectedPlan && selectedPlan.id === plan.id) {
        selectedPlan = updatedPlan;
      }
      plans = plans.map(p => p.id === plan.id ? updatedPlan : p);

      await updatePlan(updatedPlan);
      notifyPlansUpdated();
      await loadPlans();
    } catch (e) {
      console.error("Failed to delete task:", e);
      await loadPlans();
    }
  }

  async function handleUpdateTaskText(plan: Plan, taskId: string, newText: string) {
    const updatedTasks = plan.tasks.map(t =>
      t.id === taskId ? { ...t, text: newText } : t
    );
    const updatedPlan: Plan = {
      ...plan,
      tasks: updatedTasks
    };
    try {
      if (selectedPlan && selectedPlan.id === plan.id) {
        selectedPlan = updatedPlan;
      }
      plans = plans.map(p => p.id === plan.id ? updatedPlan : p);

      await updatePlan(updatedPlan);
      notifyPlansUpdated();
      await loadPlans();
    } catch (e) {
      console.error("Failed to update task text:", e);
      await loadPlans();
    }
  }

  async function handleUpdatePlanStatus(plan: Plan, status: "active" | "completed" | "archived") {
    const updatedPlan: Plan = {
      ...plan,
      status
    };
    try {
      if (selectedPlan && selectedPlan.id === plan.id) {
        selectedPlan = updatedPlan;
      }
      plans = plans.map(p => p.id === plan.id ? updatedPlan : p);

      await updatePlan(updatedPlan);
      notifyPlansUpdated();
      await loadPlans();
    } catch (e) {
      console.error("Failed to update plan status:", e);
      await loadPlans();
    }
  }

  async function handleDeletePlan(id: string) {
    const confirmed = await requestConfirm({
      title: language === "fr" ? "Supprimer ce plan définitivement ?" : "Delete this plan permanently?",
      confirmLabel: language === "fr" ? "Supprimer" : "Delete",
      cancelLabel: language === "fr" ? "Annuler" : "Cancel",
    });
    if (!confirmed) return;
    try {
      if (selectedPlan?.id === id) {
        selectedPlan = null;
      }
      await deletePlan(id);
      notifyPlansUpdated();
      await loadPlans();
    } catch (e) {
      console.error("Failed to delete plan:", e);
    }
  }

  // Browser simulator state
  let browserUrl = "https://google.com";
  let browserSearchQuery = "";
  let browserLoading = false;
  let browserResults: { title: string; url: string; snippet: string }[] = [];

  // Helper for goals
  function compactAgentGoal(goal: string): string {
    if (!goal) return "";
    return goal.length > 55 ? goal.substring(0, 52) + "..." : goal;
  }

  // Toggle tab
  function setTab(tab: TabKey) {
    if (activeTab === tab) {
      activeTab = "none"; // Deselect back to dashboard
    } else {
      activeTab = tab;
    }
  }

  // Web Browser simulator search
  function handleBrowserSearch() {
    if (!browserSearchQuery.trim()) return;
    browserLoading = true;
    browserUrl = `https://google.com/search?q=${encodeURIComponent(browserSearchQuery)}`;
    
    setTimeout(() => {
      browserResults = [
        {
          title: `${browserSearchQuery} - Actualités & Informations`,
          url: `https://news.google.com/search?q=${encodeURIComponent(browserSearchQuery)}`,
          snippet: `Découvrez les derniers articles de presse et analyses détaillées concernant ${browserSearchQuery}. Les tendances montrent une adoption massive.`
        },
        {
          title: `Définition de ${browserSearchQuery} sur Wikipédia`,
          url: `https://fr.wikipedia.org/wiki/${encodeURIComponent(browserSearchQuery)}`,
          snippet: `${browserSearchQuery} fait référence aux dernières technologies disruptives développées localement et exécutées en local-first.`
        },
        {
          title: `Documentation officielle de ${browserSearchQuery}`,
          url: `https://docs.rs/releases/search?q=${encodeURIComponent(browserSearchQuery)}`,
          snippet: `Retrouvez les guides d'utilisation, l'API de référence et les bibliothèques logicielles pour intégrer ${browserSearchQuery} dans vos projets.`
        }
      ];
      browserLoading = false;
      browserSearchQuery = "";
    }, 800);
  }

  function handleGoToUrl() {
    if (!browserUrl.trim()) return;
    browserLoading = true;
    setTimeout(() => {
      browserLoading = false;
      if (!browserUrl.includes("google.com/search")) {
        browserResults = [];
      }
    }, 600);
  }

  function copySourceUrl(url: string) {
    if (!url) return;
    navigator.clipboard?.writeText(url);
    copiedSourceUrl = url;
    setTimeout(() => {
      if (copiedSourceUrl === url) copiedSourceUrl = null;
    }, 2000);
  }

  $: allSourcesCount = contextSources.length + attachedFiles.length;

  $: filteredContextSources = contextSources.filter(s => {
    const term = sourcesSearchQuery.trim().toLowerCase();
    const matchesSearch = !term ||
      Boolean(s.title && s.title.toLowerCase().includes(term)) ||
      Boolean(s.url && s.url.toLowerCase().includes(term)) ||
      Boolean(s.uri && s.uri.toLowerCase().includes(term)) ||
      Boolean(s.excerpt && s.excerpt.toLowerCase().includes(term));
    if (!matchesSearch) return false;

    if (sourcesFilter === "web") {
      return s.kind === "web" || s.kind === "web-search" || (s.url && s.url.startsWith("http"));
    }
    if (sourcesFilter === "memory") {
      return s.kind === "memory";
    }
    if (sourcesFilter === "files") {
      return false;
    }
    return true;
  });

  $: filteredAttachedFiles = attachedFiles.filter(f => {
    const term = sourcesSearchQuery.trim().toLowerCase();
    const matchesSearch = !term || Boolean(f.name && f.name.toLowerCase().includes(term));
    if (!matchesSearch) return false;
    if (sourcesFilter === "web" || sourcesFilter === "memory") return false;
    return true;
  });

  // Reference unused exports to silence compiler warnings
  $: {
    visibleSources;
    agentRunsBusy;
    onUpdateAgentLanePriority;
  }
</script>

<aside class="right-panel" style="width: {width}px;">


  <!-- Segmented Tab Bar -->
  <div class="segmented-tab-bar">
    <button class="tab-btn" class:active={activeTab === "outputs"} on:click={() => setTab("outputs")} title={language === "fr" ? "Sorties" : "Outputs"}>
      <FileText size={15} />
    </button>
    <button class="tab-btn" class:active={activeTab === "agents"} on:click={() => setTab("agents")} title={language === "fr" ? "Sous-agents" : "Subagents"}>
      <Bot size={15} />
    </button>
    <button class="tab-btn" class:active={activeTab === "explorer"} on:click={() => setTab("explorer")} title={language === "fr" ? "Fichiers" : "Explorer"}>
      <FolderOpen size={15} />
    </button>
    <button class="tab-btn" class:active={activeTab === "plan"} on:click={() => setTab("plan")} title={language === "fr" ? "Plan" : "Plan"}>
      <ClipboardList size={15} />
    </button>
    <button class="tab-btn" class:active={activeTab === "browser"} on:click={() => setTab("browser")} title={language === "fr" ? "Navigateur" : "Browser"}>
      <Globe size={15} />
    </button>
    <button class="tab-btn" class:active={activeTab === "sources"} on:click={() => setTab("sources")} title={language === "fr" ? "Sources" : "Sources"}>
      <BookOpen size={15} />
    </button>
    
    <div style="flex-grow: 1;"></div>
    
    <button class="tab-btn close-panel-btn" on:click={onClose} title={language === "fr" ? "Fermer le panneau" : "Close panel"}>
      <X size={15} />
    </button>
  </div>

  <div class="right-panel-body">
    <!-- DASHBOARD HOME VIEW -->
    {#if activeTab === "none"}
      <div class="dashboard-home animate-fade-in">
        <div class="dashboard-hero">
          <h3>{language === "fr" ? "Espace de travail" : "AI Workspace"}</h3>
          <p>{language === "fr" ? "Gérez vos livrables, visualisez les sous-agents et parcourez votre code." : "Manage deliverables, monitor subagents, and browse your workspace."}</p>
        </div>

        <div class="dashboard-grid" class:two-cols={width >= 500}>
          <!-- Outputs Card -->
          <button type="button" class="dashboard-card outputs-card ripple" on:click={() => activeTab = "outputs"}>
            <div class="card-icon outputs-color">
              <FileText size={18} />
            </div>
            <div class="card-info">
              <span class="card-title">{language === "fr" ? "Sorties & Artefacts" : "Outputs & Artifacts"}</span>
              <span class="card-desc">
                {#if artifacts.length === 0}
                  {language === "fr" ? "Aucun fichier généré" : "No files generated yet"}
                {:else}
                  {artifacts.length} {language === "fr" ? "artefact(s) disponible(s)" : "artifact(s) available"}
                {/if}
              </span>
            </div>
            <span class="card-arrow"><ChevronRight size={14} /></span>
          </button>

          <!-- Subagents Card -->
          <button type="button" class="dashboard-card agents-card ripple" on:click={() => activeTab = "agents"}>
            <div class="card-icon agents-color">
              <Bot size={18} />
            </div>
            <div class="card-info">
              <span class="card-title">{language === "fr" ? "Sous-Agents" : "Sub-Agents"}</span>
              <span class="card-desc">
                {#if agentInboxTotals.running + agentInboxTotals.queued > 0}
                  {agentInboxTotals.running} {language === "fr" ? "actifs" : "active"}, {agentInboxTotals.queued} {language === "fr" ? "en file" : "queued"}
                {:else}
                  {language === "fr" ? "Aucun agent en cours" : "No active subagents"}
                {/if}
              </span>
            </div>
            <span class="card-arrow"><ChevronRight size={14} /></span>
          </button>

          <!-- Explorer Card -->
          <button type="button" class="dashboard-card explorer-card ripple" on:click={() => activeTab = "explorer"}>
            <div class="card-icon explorer-color">
              <FolderOpen size={18} />
            </div>
            <div class="card-info">
              <span class="card-title">{language === "fr" ? "Fichiers du Workspace" : "Workspace Files"}</span>
              <span class="card-desc">
                {language === "fr" ? "Parcourez l'arborescence du code" : "Browse project file tree"}
              </span>
            </div>
            <span class="card-arrow"><ChevronRight size={14} /></span>
          </button>

          <!-- Plan Card -->
          <button type="button" class="dashboard-card plan-card ripple" on:click={() => activeTab = "plan"}>
            <div class="card-icon plan-color">
              <ClipboardList size={18} />
            </div>
            <div class="card-info">
              <span class="card-title">{language === "fr" ? "Plan de Travail" : "Workspace Plan"}</span>
              <span class="card-desc">
                {#if totalTasksCount > 0}
                  {completedTasksCount}/{totalTasksCount} {language === "fr" ? "tâches complétées" : "tasks completed"}
                {:else}
                  {language === "fr" ? "Suivez la feuille de route du projet" : "Follow the project roadmap"}
                {/if}
              </span>
            </div>
            <span class="card-arrow"><ChevronRight size={14} /></span>
          </button>

          <!-- Browser Card -->
          <button type="button" class="dashboard-card browser-card ripple" on:click={() => activeTab = "browser"}>
            <div class="card-icon browser-color">
              <Globe size={18} />
            </div>
            <div class="card-info">
              <span class="card-title">{language === "fr" ? "Navigateur Web" : "Web Browser"}</span>
              <span class="card-desc">{language === "fr" ? "Recherches et aperçus de sites" : "Web searches & page previews"}</span>
            </div>
            <span class="card-arrow"><ChevronRight size={14} /></span>
          </button>

          <!-- Sources Card -->
          <button type="button" class="dashboard-card sources-card ripple" on:click={() => activeTab = "sources"}>
            <div class="card-icon sources-color">
              <BookOpen size={18} />
            </div>
            <div class="card-info">
              <span class="card-title">{language === "fr" ? "Sources & Références" : "Sources & References"}</span>
              <span class="card-desc">
                {allSourcesCount} {language === "fr" ? (allSourcesCount > 1 ? "sources actives" : "source active") : (allSourcesCount > 1 ? "active sources" : "active source")}
              </span>
            </div>
            <span class="card-arrow"><ChevronRight size={14} /></span>
          </button>
        </div>
      </div>

    <!-- OUTPUTS TAB -->
    {:else if activeTab === "outputs"}
      <div class="tab-content-view animate-fade-in outputs-tab-container">
        {#if selectedArtifact}
          {@const deltas = getArtifactDeltas(selectedArtifact)}
          <!-- DETAIL DIFF PREVIEW VIEW -->
          <div class="artifact-detail-view animate-fade-in">
            <!-- Top Navigation & Back Button -->
            <div class="artifact-detail-top-bar">
              <button type="button" class="back-btn ripple" on:click={() => (selectedArtifact = null)}>
                <ArrowLeft size={15} />
                <span>{language === "fr" ? "Retour aux artefacts" : "Back to artifacts"}</span>
              </button>

              <div class="artifact-detail-badges">
                {#if selectedArtifact.status === "applied"}
                  <span class="artifact-status-badge applied">
                    <Check size={11} />
                    <span>{language === "fr" ? "Appliqué" : "Applied"}</span>
                  </span>
                {:else if selectedArtifact.status === "rejected"}
                  <span class="artifact-status-badge rejected">
                    <X size={11} />
                    <span>{language === "fr" ? "Rejeté" : "Rejected"}</span>
                  </span>
                {:else}
                  <span class="artifact-status-badge pending">
                    <span>{language === "fr" ? "En attente" : "Pending"}</span>
                  </span>
                {/if}
              </div>
            </div>

            <!-- Metadata Summary Card -->
            <div class="artifact-detail-summary-card">
              <div class="artifact-summary-header">
                <div class="artifact-icon">
                  <FileCode size={16} />
                </div>
                <div class="artifact-meta">
                  <span class="artifact-title-text" title={getArtifactTitle(selectedArtifact)}>
                    {getArtifactTitle(selectedArtifact)}
                  </span>
                  <span class="artifact-path-label" title={selectedArtifact.filePath || ""}>
                    {selectedArtifact.filePath || (language === "fr" ? "Fichier sans chemin" : "Unpathed file")}
                  </span>
                </div>

                <!-- Delta counters -->
                <div class="artifact-deltas">
                  {#if deltas.additions > 0}
                    <span class="delta-pill add">+{deltas.additions}</span>
                  {/if}
                  {#if deltas.deletions > 0}
                    <span class="delta-pill del">-{deltas.deletions}</span>
                  {/if}
                  {#if deltas.additions === 0 && deltas.deletions === 0}
                    <span class="delta-pill neutral">0</span>
                  {/if}
                </div>
              </div>

              <!-- Action buttons in detail card -->
              <div class="artifact-detail-actions">
                <button
                  type="button"
                  class="action-btn copy-btn"
                  on:click={() => handleCopyArtifact(selectedArtifact)}
                >
                  <Copy size={13} />
                  <span>
                    {copiedArtifactId === (selectedArtifact.id || selectedArtifact.filePath)
                      ? (language === "fr" ? "Copié !" : "Copied!")
                      : (language === "fr" ? "Copier le code" : "Copy code")}
                  </span>
                </button>

                <button
                  type="button"
                  class="action-btn reject-btn"
                  disabled={selectedArtifact.status === "rejected"}
                  on:click={() => handleRejectArtifact(selectedArtifact)}
                >
                  <X size={13} />
                  <span>{language === "fr" ? "Rejeter" : "Reject"}</span>
                </button>
              </div>
            </div>

            <!-- Embedded CodeDiffViewer -->
            <div class="artifact-diff-viewer-wrapper">
              <CodeDiffViewer
                filePath={selectedArtifact.filePath || selectedArtifact.title || "artifact"}
                diffLines={getArtifactDiffLines(selectedArtifact)}
                {language}
                modifiedContent={selectedArtifact.modifiedContent ?? selectedArtifact.content}
                diffPatch={selectedArtifact.diffText}
                conversationId={activeConversationId || undefined}
                status={selectedArtifact.status || "pending"}
                actionVariant="apply"
                onAccept={() => handleArtifactApplied(selectedArtifact)}
                onReject={() => handleRejectArtifact(selectedArtifact)}
              />
            </div>
          </div>

        {:else}
          <!-- LIST VIEW -->
          {#if !artifacts || artifacts.length === 0}
            <!-- Empty state -->
            <div class="tab-empty-state">
              <span class="empty-icon-wrap"><FileText size={32} /></span>
              <p class="empty-title">{language === "fr" ? "Aucun fichier généré" : "No deliverables yet"}</p>
              <p class="empty-desc">
                {language === "fr"
                  ? "Les fichiers modifiés ou générés par l'assistant et les sous-agents apparaîtront ici pour revue et application."
                  : "Files modified or generated by the assistant and subagents will appear here for review and application."}
              </p>
            </div>
          {:else}
            <!-- List View with Toolbar & Cards -->
            <div class="outputs-list-view">
              <!-- Toolbar: Heading, Counts, Search & Status Filters -->
              <div class="outputs-toolbar">
                <div class="outputs-toolbar-title-row">
                  <span class="outputs-heading">
                    {language === "fr" ? "Livrables du projet" : "Project Deliverables"}
                  </span>
                  <span class="outputs-count-badge">
                    {artifacts.length} {language === "fr" ? "artefact(s)" : "artifact(s)"}
                  </span>
                </div>

                <div class="outputs-search-filter-row">
                  <div class="outputs-search-box">
                    <Search size={13} />
                    <input
                      type="text"
                      bind:value={artifactSearchQuery}
                      placeholder={language === "fr" ? "Filtrer les fichiers..." : "Filter files..."}
                    />
                    {#if artifactSearchQuery}
                      <button type="button" class="clear-search-btn" on:click={() => (artifactSearchQuery = "")}>
                        <X size={11} />
                      </button>
                    {/if}
                  </div>

                  <!-- Status Filter Pills -->
                  <div class="status-filter-pills">
                    <button
                      type="button"
                      class="filter-pill"
                      class:active={artifactStatusFilter === "all"}
                      on:click={() => (artifactStatusFilter = "all")}
                    >
                      {language === "fr" ? "Tous" : "All"}
                    </button>
                    <button
                      type="button"
                      class="filter-pill"
                      class:active={artifactStatusFilter === "pending"}
                      on:click={() => (artifactStatusFilter = "pending")}
                    >
                      {language === "fr" ? "En attente" : "Pending"}
                    </button>
                    <button
                      type="button"
                      class="filter-pill"
                      class:active={artifactStatusFilter === "applied"}
                      on:click={() => (artifactStatusFilter = "applied")}
                    >
                      {language === "fr" ? "Appliqués" : "Applied"}
                    </button>
                    <button
                      type="button"
                      class="filter-pill"
                      class:active={artifactStatusFilter === "rejected"}
                      on:click={() => (artifactStatusFilter = "rejected")}
                    >
                      {language === "fr" ? "Rejetés" : "Rejected"}
                    </button>
                  </div>
                </div>
              </div>

              <!-- Cards List -->
              {#if filteredArtifacts.length === 0}
                <div class="tab-empty-state">
                  <span class="empty-icon-wrap"><Search size={24} /></span>
                  <p class="empty-title">{language === "fr" ? "Aucun résultat" : "No results"}</p>
                  <p class="empty-desc">
                    {language === "fr"
                      ? "Aucun artefact ne correspond aux critères de recherche actuels."
                      : "No artifacts match the current search filters."}
                  </p>
                </div>
              {:else}
                <div class="artifacts-list" class:two-cols={width >= 560}>
                  {#each filteredArtifacts as artifact, idx (getArtifactKey(artifact, idx))}
                    {@const deltas = getArtifactDeltas(artifact)}
                    {@const ext = getArtifactExtension(artifact)}
                    <div
                      class="artifact-item"
                      class:applied={artifact.status === "applied"}
                      class:rejected={artifact.status === "rejected"}
                    >
                      <!-- Card Head -->
                      <div class="artifact-head">
                        <div class="artifact-icon" title={ext.toUpperCase() || "FILE"}>
                          <FileCode size={15} />
                        </div>
                        <div class="artifact-meta">
                          <button
                            type="button"
                            class="artifact-title-text clickable-title"
                            title={getArtifactTitle(artifact)}
                            on:click={() => (selectedArtifact = artifact)}
                            style="cursor: pointer; border: none; background: transparent; padding: 0; text-align: left;"
                          >
                            {getArtifactTitle(artifact)}
                          </button>
                          <div class="artifact-subline">
                            {#if ext}
                              <span class="artifact-ext-badge">.{ext}</span>
                            {/if}
                            <!-- Status badge -->
                            {#if artifact.status === "applied"}
                              <span class="artifact-status-badge applied">
                                <Check size={10} />
                                <span>{language === "fr" ? "Appliqué" : "Applied"}</span>
                              </span>
                            {:else if artifact.status === "rejected"}
                              <span class="artifact-status-badge rejected">
                                <X size={10} />
                                <span>{language === "fr" ? "Rejeté" : "Rejected"}</span>
                              </span>
                            {:else}
                              <span class="artifact-status-badge pending">
                                <span>{language === "fr" ? "En attente" : "Pending"}</span>
                              </span>
                            {/if}

                            <!-- Delta pills -->
                            <div class="artifact-deltas">
                              {#if deltas.additions > 0}
                                <span class="delta-pill add">+{deltas.additions}</span>
                              {/if}
                              {#if deltas.deletions > 0}
                                <span class="delta-pill del">-{deltas.deletions}</span>
                              {/if}
                              {#if deltas.additions === 0 && deltas.deletions === 0}
                                <span class="delta-pill neutral">0</span>
                              {/if}
                            </div>
                          </div>
                        </div>
                      </div>

                      <!-- Card Actions -->
                      <div class="artifact-actions">
                        <span class="artifact-path-label" title={artifact.filePath || ""}>
                          {artifact.filePath || (language === "fr" ? "Sans chemin" : "No path")}
                        </span>

                        <div class="artifact-card-btns">
                          <button
                            type="button"
                            class="artifact-action-btn preview"
                            on:click={() => (selectedArtifact = artifact)}
                            title={language === "fr" ? "Prévisualiser le Diff" : "Preview Diff"}
                          >
                            <Eye size={12} />
                            <span>{language === "fr" ? "Prévisualiser" : "Preview"}</span>
                          </button>

                          {#if artifact.status !== "applied"}
                            <button
                              type="button"
                              class="artifact-action-btn apply"
                              disabled={applyingArtifactId === (artifact.id || artifact.filePath)}
                              on:click={() => handleApplyArtifact(artifact)}
                              title={language === "fr" ? "Appliquer au projet sur le disque" : "Apply to disk"}
                            >
                              <Check size={12} />
                              <span>{language === "fr" ? "Appliquer" : "Apply"}</span>
                            </button>
                          {/if}

                          {#if artifact.status !== "rejected"}
                            <button
                              type="button"
                              class="artifact-action-btn reject"
                              on:click={() => handleRejectArtifact(artifact)}
                              title={language === "fr" ? "Rejeter les modifications" : "Reject modifications"}
                            >
                              <X size={12} />
                              <span>{language === "fr" ? "Rejeter" : "Reject"}</span>
                            </button>
                          {/if}
                        </div>
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        {/if}
      </div>

    <!-- SUBAGENTS TAB -->
    {:else if activeTab === "agents"}
      <div class="tab-content-view animate-fade-in">
        <!-- Sub-tabs: lanes vs custom -->
        <div class="agents-sub-tabs">
          <button type="button" class="sub-tab-btn" class:active={agentsSubTab === "lanes"} on:click={() => agentsSubTab = "lanes"}>
            <Activity size={12} />
            <span>{language === "fr" ? "Activité" : "Activity"}</span>
          </button>
          <button type="button" class="sub-tab-btn" class:active={agentsSubTab === "custom"} on:click={() => { agentsSubTab = "custom"; editingAgent = null; }}>
            <Bot size={12} />
            <span>{language === "fr" ? "Mes Agents" : "My Agents"}</span>
          </button>
        </div>

        {#if agentsSubTab === "lanes"}
          {#if selectedAgentRunView}
            <!-- DEDICATED AGENT RUN LIVE INSPECTION VIEW -->
            <div class="agent-run-detail-view animate-fade-in">
              <div class="run-detail-top-bar">
                <button type="button" class="back-btn ripple" on:click={() => (selectedAgentRunView = null)}>
                  <ArrowLeft size={15} />
                  <span>{language === "fr" ? "Retour aux voies" : "Back to lanes"}</span>
                </button>
                <div class="run-detail-badges">
                  <span class="lane-status-badge {selectedAgentRunView.run.status}">
                    {selectedAgentRunView.run.status}
                  </span>
                  {#if selectedAgentRunView.run.priority}
                    <span class="lane-priority">{selectedAgentRunView.run.priority}</span>
                  {/if}
                </div>
              </div>

              <div class="run-detail-summary-card">
                <div class="run-goal-heading">
                  <Bot size={16} />
                  <span class="run-goal-text">{selectedAgentRunView.run.goal}</span>
                </div>
                {#if selectedAgentRunView.run.checkpointSummary}
                  <p class="run-checkpoint-text">{selectedAgentRunView.run.checkpointSummary}</p>
                {/if}
                {#if selectedAgentRunView.run.startedAt}
                  <div class="run-meta-time">
                    <span>{language === "fr" ? "Démarré :" : "Started:"} {new Date(selectedAgentRunView.run.startedAt).toLocaleTimeString()}</span>
                  </div>
                {/if}
              </div>

              <!-- Live Terminal Viewer -->
              <div class="agent-log-viewer-container">
                <AgentLiveLogViewer
                  logs={selectedRunLogs}
                  {language}
                  onClear={handleClearRunLogs}
                />
              </div>
            </div>
          {:else}
            <!-- Totals banner -->
            <div class="agents-inbox-stats">
              <div class="stat-badge running">
                <span class="stat-count">{agentInboxTotals.running ?? 0}</span>
                <span class="stat-label">{language === "fr" ? "actifs" : "active"}</span>
              </div>
              <div class="stat-badge queued">
                <span class="stat-count">{agentInboxTotals.queued ?? 0}</span>
                <span class="stat-label">{language === "fr" ? "en file" : "queued"}</span>
              </div>
              <div class="stat-badge done">
                <span class="stat-count">{agentInboxTotals.done ?? 0}</span>
                <span class="stat-label">{language === "fr" ? "finis" : "done"}</span>
              </div>
              {#if (agentInboxTotals.failed ?? 0) > 0}
                <div class="stat-badge failed">
                  <span class="stat-count">{agentInboxTotals.failed}</span>
                  <span class="stat-label">{language === "fr" ? "échecs" : "failed"}</span>
                </div>
              {/if}
            </div>

            {#if agentInboxLanes.length === 0}
              <div class="tab-empty-state">
                <span class="empty-icon-wrap"><Bot size={32} /></span>
                <p class="empty-title">{language === "fr" ? "Aucun sous-agent" : "No Sub-agents"}</p>
                <p class="empty-desc">{language === "fr" ? "Les sous-agents démarrés en arrière-plan apparaîtront ici." : "Sub-agents running tasks in the background will appear here."}</p>
              </div>
            {:else}
              <div class="agent-lanes-scroll" class:two-cols={width >= 500}>
                {#each agentInboxLanes as laneView}
                  <div class="agent-lane-card" class:selected={laneView.visibleRuns.some((run: any) => selectedAgentRunView?.run.id === run.id)}>
                    <div class="agent-lane-head">
                      <button type="button" class="agent-lane-toggle-btn" on:click={() => onToggleAgentLane(laneView.lane.id)}>
                        <ChevronRight size={14} style={`transition: transform 160ms ease; transform: ${laneView.collapsed ? "rotate(0deg)" : "rotate(90deg)"};`} />
                        <span class="agent-lane-title" title={laneView.lane.title}>{laneView.lane.title}</span>
                      </button>
                      <div class="lane-status-controls">
                        {#if laneView.lane.id !== "default-virtual"}
                          {#if laneView.lane.status === "active"}
                            <button class="lane-action-btn" disabled={agentActionBusy === laneView.lane.id} on:click={() => onSetAgentLaneAction(laneView.lane.id, "pause")} title="Pause">
                              <Pause size={12} />
                            </button>
                          {:else if laneView.lane.status === "paused"}
                            <button class="lane-action-btn play" disabled={agentActionBusy === laneView.lane.id} on:click={() => onSetAgentLaneAction(laneView.lane.id, "resume")} title="Resume">
                              <Play size={12} />
                            </button>
                          {/if}
                        {/if}
                      </div>
                    </div>

                    {#if !laneView.collapsed}
                      <div class="lane-body animate-slide-down">
                        <div class="lane-meta-row">
                          <span class="lane-status-badge">{laneView.lane.status}</span>
                          <span class="lane-priority">{language === "fr" ? "Priorité:" : "Priority:"} {laneView.lane.priority || "normal"}</span>
                        </div>

                        {#if laneView.visibleRuns.length > 0}
                          <div class="lane-runs-list">
                            {#each laneView.visibleRuns as run}
                              <button type="button" class="lane-run-row" class:active={selectedAgentRunView?.run.id === run.id} on:click={() => onOpenAgentRun(run.id)}>
                                <span class="run-activity-wrap"><Activity size={12} /></span>
                                <span class="run-goal" title={run.goal}>{compactAgentGoal(run.goal)}</span>
                                <span class="run-duration">{run.startedAt ? (language === "fr" ? "en cours" : "running") : "--"}</span>
                              </button>
                            {/each}
                          </div>
                        {/if}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          {/if}

        {:else}
          <!-- CUSTOM AGENTS MANAGEMENT -->
          <!-- Agents List -->
          <div class="agents-list-container">
            <div class="list-header-row">
              <span class="list-title">{language === "fr" ? "Agents Autonomes" : "Autonomous Agents"}</span>
              <button type="button" class="create-agent-btn" on:click={onStartCreateAgent}>
                <Plus size={12} />
                <span>{language === "fr" ? "Créer" : "Create"}</span>
              </button>
            </div>

            {#if personalities.length === 0}
              <div class="tab-empty-state">
                <span class="empty-icon-wrap"><Bot size={32} /></span>
                <p class="empty-title">{language === "fr" ? "Aucun agent autonome" : "No Autonomous Agents"}</p>
                <p class="empty-desc">
                  {language === "fr" 
                    ? "Créez vos propres agents autonomes configurés avec des instructions système spécifiques." 
                    : "Create your own autonomous agents configured with specific system instructions."}
                </p>
              </div>
            {:else}
              <div class="agents-cards-grid">
                {#each personalities as pers}
                  <div class="agent-pers-card">
                    <div class="card-top">
                      <div class="agent-avatar" style="background: {pers.avatarColor || 'linear-gradient(135deg, #af52de 0%, #7d26cd 100%)'}">
                        {#if pers.icon === "bot"}<Bot size={16} />
                        {:else if pers.icon === "code"}<Terminal size={16} />
                        {:else if pers.icon === "check"}<Check size={16} />
                        {:else if pers.icon === "edit-2"}<Edit2 size={16} />
                        {:else if pers.icon === "brain"}<Brain size={16} />
                        {:else if pers.icon === "cpu"}<Cpu size={16} />
                        {:else if pers.icon === "user"}<User size={16} />
                        {:else if pers.icon === "activity"}<Activity size={16} />
                        {:else if pers.icon === "sliders"}<Sliders size={16} />
                        {:else if pers.icon === "building-2"}<Building2 size={16} />
                        {:else}<Search size={16} />{/if}
                      </div>
                      <div class="agent-info">
                        <span class="agent-name">{pers.name}</span>
                        <span class="agent-desc">{pers.description || (language === "fr" ? "Aucune description" : "No description")}</span>
                      </div>
                    </div>

                    {#if pers.systemPrompt}
                      <div class="prompt-preview-box">
                        <span class="prompt-preview-title">{language === "fr" ? "Instructions :" : "Instructions:"}</span>
                        <p class="prompt-preview-text">{pers.systemPrompt}</p>
                      </div>
                    {/if}

                    <div class="agent-card-actions">
                      <button type="button" class="action-btn chat-btn" on:click={() => onStartAgentChat(pers.id)}>
                        <Play size={12} />
                        <span>{language === "fr" ? "Discuter" : "Chat"}</span>
                      </button>

                      <button type="button" class="action-btn edit-btn" on:click={() => onStartEditAgent(pers)}>
                        {language === "fr" ? "Modifier" : "Edit"}
                      </button>
                      <button type="button" class="action-btn delete-btn" on:click={() => handleDeleteAgent(pers.id)} title={language === "fr" ? "Supprimer" : "Delete"}>
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>

    <!-- WORKSPACE EXPLORER TAB -->
    {:else if activeTab === "explorer"}
      <div class="tab-content-view animate-fade-in explorer-tab-layout" class:file-open={Boolean(activeWorkspaceFile)}>
        <div class="explorer-tree-pane" class:active-file-open={Boolean(activeWorkspaceFile)}>
          <WorkspaceTreeExplorer
            {activeConversationId}
            {language}
            activeFilePath={activeWorkspaceFile?.relativePath ?? null}
            onOpenFile={openWorkspaceFile}
            onSelectFileToPrompt={examineWorkspaceFile}
          />
        </div>

        {#if activeWorkspaceFile}
          <div class="explorer-file-pane">
            <WorkspaceFileViewer
              file={activeWorkspaceFile}
              content={workspaceFileContent}
              loading={workspaceFileLoading}
              error={workspaceFileError}
              truncated={workspaceFileTruncated}
              {language}
              onClose={closeWorkspaceFile}
              onExamine={examineWorkspaceFile}
            />
          </div>
        {/if}

      </div>

    <!-- WORKSPACE PLAN TAB -->
    {:else if activeTab === "plan"}
      <div class="tab-content-view animate-fade-in plan-container">
        {#if selectedPlan}
          <!-- Plan detail view -->
          <div class="plan-detail">
            <button type="button" class="back-btn" on:click={() => selectedPlan = null}>
              <ArrowLeft size={16} />
              <span>{language === "fr" ? "Retour aux plans" : "Back to plans"}</span>
            </button>

            <div class="plan-header-card">
              <h3 class="plan-title">{selectedPlan.title}</h3>
              {#if selectedPlan.description}
                <p class="plan-desc">{selectedPlan.description}</p>
              {/if}
              
              <!-- Circular progress ring or visual indicator -->
              <div class="plan-progress-circle-row">
                <div class="progress-ring-container">
                  <svg width="48" height="48" viewBox="0 0 48 48">
                    <!-- Background circle -->
                    <circle cx="24" cy="24" r="20" class="ring-bg" />
                    <!-- Foreground progress arc -->
                    <circle cx="24" cy="24" r="20" class="ring-fg" 
                            style="stroke-dasharray: {2 * Math.PI * 20}; 
                                   stroke-dashoffset: {2 * Math.PI * 20 * (1 - (selectedPlanProgress.percentage / 100))}" />
                  </svg>
                  <span class="progress-pct">
                    {selectedPlanProgress.percentage}%
                  </span>
                </div>
                <div class="progress-text-details">
                  <span class="progress-label">{language === "fr" ? "Progression du plan" : "Plan completion"}</span>
                  <span class="progress-counts">
                    {selectedPlanProgress.completed} / {selectedPlanProgress.total} {language === "fr" ? "étapes" : "steps"}
                  </span>
                </div>
              </div>
            </div>

            <!-- Steps list title & Add task row -->
            <div class="add-task-row">
              <input type="text" bind:value={newPlanTaskInput} placeholder={language === "fr" ? "Ajouter une étape au plan..." : "Add task step..."} on:keydown={e => e.key === "Enter" && selectedPlan && handleAddTaskToPlan(selectedPlan)} />
              <button type="button" on:click={() => selectedPlan && handleAddTaskToPlan(selectedPlan)}>
                <Plus size={14} />
              </button>
            </div>

            <!-- Tasks list -->
            <div class="plan-tasks-list">
              {#each selectedPlan.tasks as task (task.id)}
                {@const taskStatus = task.status || (task.completed ? "completed" : "pending")}
                <div 
                  class="task-row" 
                  class:completed={taskStatus === "completed"}
                  class:in-progress={taskStatus === "in_progress"}
                  class:has-error={taskStatus === "error"}
                >
                  <button 
                    type="button" 
                    class="task-checkbox task-status-btn status-{taskStatus}" 
                    title={taskStatus === "error" && task.error 
                      ? formatTaskError(task.error) 
                      : (language === "fr" ? `Statut: ${taskStatus} (cliquer pour changer)` : `Status: ${taskStatus} (click to cycle)`)}
                    on:click={() => selectedPlan && handleCycleTaskStatus(selectedPlan, task.id)}
                  >
                    {#if taskStatus === "completed"}
                      <span class="check-icon-wrap status-icon-wrap completed"><CheckCircle2 size={16} /></span>
                    {:else if taskStatus === "in_progress"}
                      <span class="status-icon-wrap in-progress"><Loader2 size={16} class="animate-spin" /></span>
                    {:else if taskStatus === "error"}
                      <span class="status-icon-wrap error"><AlertCircle size={16} /></span>
                    {:else}
                      <span class="status-icon-wrap pending"><Circle size={16} /></span>
                    {/if}
                  </button>

                  <div class="task-content-col">
                    <input 
                      type="text" 
                      class="task-text-input" 
                      value={task.text} 
                      on:change={e => selectedPlan && handleUpdateTaskText(selectedPlan, task.id, e.currentTarget.value)} 
                    />
                    {#if taskStatus === "error" && task.error}
                      <span class="task-error-hint" title={task.error}>
                        {formatTaskError(task.error, 120)}
                      </span>
                    {/if}
                  </div>

                  <button 
                    type="button" 
                    class="task-delete-btn" 
                    title={language === "fr" ? "Supprimer la tâche" : "Delete task"}
                    on:click={() => selectedPlan && handleDeleteTask(selectedPlan, task.id)}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              {/each}
            </div>

            <!-- Detail Actions -->
            <div class="plan-actions-footer">
              {#if selectedPlan.status !== "completed" && selectedPlan.tasks.length > 0 && selectedPlan.tasks.every(t => t.completed || t.status === "completed")}
                <button type="button" class="footer-btn complete-btn" on:click={() => selectedPlan && handleUpdatePlanStatus(selectedPlan, "completed")}>
                  <Check size={14} />
                  <span>{language === "fr" ? "Marquer comme complété" : "Mark as completed"}</span>
                </button>
              {/if}
              {#if selectedPlan.status === "active"}
                <button type="button" class="footer-btn archive-btn" on:click={() => selectedPlan && handleUpdatePlanStatus(selectedPlan, "archived")}>
                  <Archive size={14} />
                  <span>{language === "fr" ? "Archiver" : "Archive"}</span>
                </button>
              {:else if selectedPlan.status === "archived" || selectedPlan.status === "completed"}
                <button type="button" class="footer-btn archive-btn" on:click={() => selectedPlan && handleUpdatePlanStatus(selectedPlan, "active")}>
                  <Archive size={14} />
                  <span>{language === "fr" ? "Désarchiver" : "Activate"}</span>
                </button>
              {/if}
              <button type="button" class="footer-btn delete-btn" on:click={() => selectedPlan && handleDeletePlan(selectedPlan.id)}>
                <Trash2 size={14} />
                <span>{language === "fr" ? "Supprimer" : "Delete"}</span>
              </button>
            </div>
          </div>
        {:else if showNewPlanForm}
          <!-- New Plan Form -->
          <div class="new-plan-form animate-fade-in">
            <div class="form-header">
              <h3>{language === "fr" ? "Créer un nouveau plan" : "Create new plan"}</h3>
              <button type="button" class="close-form-btn" on:click={() => showNewPlanForm = false}>
                <X size={16} />
              </button>
            </div>

            <div class="form-group">
              <label for="new-plan-title">{language === "fr" ? "Titre" : "Title"}</label>
              <input type="text" id="new-plan-title" bind:value={newPlanTitle} placeholder={language === "fr" ? "ex: Déployer l'authentification" : "e.g., Deploy authentication"} />
            </div>

            <div class="form-group">
              <label for="new-plan-desc">{language === "fr" ? "Description (Optionnelle)" : "Description (Optional)"}</label>
              <textarea id="new-plan-desc" bind:value={newPlanDescription} placeholder={language === "fr" ? "Décrivez brièvement les objectifs..." : "Briefly describe the objectives..."} rows="2"></textarea>
            </div>

            <div class="form-group">
              <label for="new-plan-tasks">{language === "fr" ? "Étapes initiales (Une par ligne)" : "Initial Steps (One per line)"}</label>
              <textarea id="new-plan-tasks" bind:value={newPlanTasksText} placeholder={language === "fr" ? "Étape 1\nÉtape 2\nÉtape 3" : "Step 1\nStep 2\nStep 3"} rows="3"></textarea>
            </div>

            <button type="button" class="submit-plan-btn" on:click={handleCreatePlan} disabled={!newPlanTitle.trim()}>
              <Plus size={14} />
              <span>{language === "fr" ? "Créer le Plan" : "Create Plan"}</span>
            </button>
          </div>
        {:else}
          <!-- Plans Dashboard -->
          <div class="plans-dashboard">
            <div class="dashboard-header">
              <h3>{language === "fr" ? "Feuille de Route" : "Project Roadmap"}</h3>
              <div class="header-actions">
                <button 
                  type="button" 
                  class="ai-roadmap-header-btn" 
                  disabled={isAiPlanBusy || !activeConversationId}
                  title={language === "fr" ? "Générer un plan avec l'IA" : "Generate plan with AI"}
                  on:click={handleTriggerAiPlan}
                >
                  {#if isAiPlanBusy}
                    <Loader2 size={13} class="animate-spin" />
                  {:else}
                    <Sparkles size={13} />
                  {/if}
                  <span>{language === "fr" ? "IA Roadmap" : "AI Roadmap"}</span>
                </button>
                <button type="button" class="new-plan-btn" on:click={() => showNewPlanForm = true}>
                  <Plus size={14} />
                  <span>{language === "fr" ? "Nouveau Plan" : "New Plan"}</span>
                </button>
              </div>
            </div>

            {#if plansLoading}
              <div class="plans-loading">
                <div class="spinner"></div>
                <span>{language === "fr" ? "Chargement des plans..." : "Loading plans..."}</span>
              </div>
            {:else if plans.length === 0}
              <div class="tab-empty-state">
                <span class="empty-icon-wrap"><ListTodo size={32} /></span>
                <p class="empty-title">{language === "fr" ? "Aucun plan de projet" : "No Project Plans"}</p>
                <p class="empty-desc">{language === "fr" ? "Créez ou demandez à l'assistant d'établir un plan d'action." : "Create or ask the assistant to establish an action plan."}</p>

                <div class="empty-actions-row">
                  <button 
                    type="button" 
                    class="ai-roadmap-btn" 
                    disabled={isAiPlanBusy || !activeConversationId}
                    on:click={handleTriggerAiPlan}
                  >
                    {#if isAiPlanBusy}
                      <Loader2 size={15} class="animate-spin" />
                      <span>{language === "fr" ? "Génération en cours..." : "Generating roadmap..."}</span>
                    {:else}
                      <Sparkles size={15} />
                      <span>{language === "fr" ? "Générer un plan avec l'IA" : "Generate plan with AI"}</span>
                    {/if}
                  </button>

                  <button type="button" class="create-manual-plan-btn" on:click={() => showNewPlanForm = true}>
                    <Plus size={14} />
                    <span>{language === "fr" ? "Nouveau Plan Manuel" : "New Manual Plan"}</span>
                  </button>
                </div>
              </div>
            {:else}
              <div class="plans-grid">
                {#each plans as plan}
                  {@const prog = calculatePlanProgress(plan.tasks)}
                  <button type="button" class="plan-card-item" class:archived={plan.status === 'archived'} class:completed={plan.status === 'completed'} on:click={() => selectedPlan = plan}>
                    <div class="card-top">
                      <div class="card-header-row">
                        <span class="card-status-badge" class:status-archived={plan.status === 'archived'} class:status-completed={plan.status === 'completed'}>
                          {plan.status}
                        </span>
                      </div>
                      <h4 class="card-title">{plan.title}</h4>
                      {#if plan.description}
                        <p class="card-desc">{plan.description}</p>
                      {/if}
                    </div>

                    <div class="card-progress">
                      <div class="progress-text-row">
                        <span>{prog.percentage}% {language === "fr" ? "complété" : "completed"}</span>
                        <span>{prog.completed}/{prog.total} {language === "fr" ? "étapes" : "steps"}</span>
                      </div>
                      <div class="progress-bar-bg">
                        <div class="progress-bar-fill" style="width: {prog.percentage}%"></div>
                      </div>
                    </div>
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}

      </div>

    <!-- INTEGRATED IN-APP BROWSER TAB -->
    {:else if activeTab === "browser"}
      <div class="tab-content-view animate-fade-in" style="padding: 0; display: flex; flex-direction: column; height: 100%; overflow: hidden;">
        <IntegratedBrowser {language} />
      </div>

    <!-- SOURCES & REFERENCES TAB -->
    {:else if activeTab === "sources"}
      <div class="tab-content-view animate-fade-in sources-container">
        <!-- Sources Top Toolbar -->
        <div class="sources-toolbar">
          <div class="sources-toolbar-header">
            <div class="sources-title-group">
              <span class="sources-title-icon"><BookOpen size={16} /></span>
              <h3 class="sources-title">{language === "fr" ? "Sources & Références" : "Sources & References"}</h3>
              <span class="sources-count-badge">{allSourcesCount}</span>
            </div>
          </div>

          <!-- Search Input -->
          <div class="sources-search-box">
            <Search size={14} class="sources-search-icon" />
            <input
              type="text"
              bind:value={sourcesSearchQuery}
              placeholder={language === "fr" ? "Filtrer les sources..." : "Filter sources..."}
            />
            {#if sourcesSearchQuery}
              <button class="sources-clear-btn" type="button" on:click={() => sourcesSearchQuery = ""}>
                <X size={12} />
              </button>
            {/if}
          </div>

          <!-- Filter Pills -->
          <div class="sources-filter-row">
            <button
              type="button"
              class="sources-filter-pill"
              class:active={sourcesFilter === "all"}
              on:click={() => sourcesFilter = "all"}
            >
              {language === "fr" ? "Toutes" : "All"} ({allSourcesCount})
            </button>
            <button
              type="button"
              class="sources-filter-pill"
              class:active={sourcesFilter === "web"}
              on:click={() => sourcesFilter = "web"}
            >
              <Globe size={12} />
              <span>Web ({contextSources.filter(s => s.kind === "web" || s.kind === "web-search" || (s.url && s.url.startsWith("http"))).length})</span>
            </button>
            {#if attachedFiles.length > 0}
              <button
                type="button"
                class="sources-filter-pill"
                class:active={sourcesFilter === "files"}
                on:click={() => sourcesFilter = "files"}
              >
                <FileText size={12} />
                <span>{language === "fr" ? "Fichiers joints" : "Attached files"} ({attachedFiles.length})</span>
              </button>
            {/if}
            {#if contextSources.some(s => s.kind === "memory")}
              <button
                type="button"
                class="sources-filter-pill"
                class:active={sourcesFilter === "memory"}
                on:click={() => sourcesFilter = "memory"}
              >
                <Brain size={12} />
                <span>{language === "fr" ? "Mémoire" : "Memory"}</span>
              </button>
            {/if}
          </div>
        </div>

        <!-- Scrollable Sources List -->
        <div class="sources-scroll-body">
          {#if filteredAttachedFiles.length === 0 && filteredContextSources.length === 0}
            <div class="sources-empty-view">
              <span class="sources-empty-icon"><BookOpen size={36} /></span>
              <p class="sources-empty-title">
                {language === "fr" ? "Aucune source disponible" : "No sources available"}
              </p>
              <p class="sources-empty-desc">
                {#if sourcesSearchQuery}
                  {language === "fr" ? "Aucune source ne correspond à votre recherche." : "No sources match your search."}
                {:else}
                  {language === "fr" ? "Les pages web consultées par l'agent, les documents joints et les mémoires de contexte apparaîtront ici." : "Web pages browsed by the agent, attached files, and context memories will appear here."}
                {/if}
              </p>
            </div>
          {:else}
            <!-- Attached Files Section -->
            {#if filteredAttachedFiles.length > 0}
              <div class="sources-section">
                <span class="sources-section-title">
                  <FileText size={12} />
                  <span>{language === "fr" ? "Fichiers joints" : "Attached files"} ({filteredAttachedFiles.length})</span>
                </span>
                <div class="sources-cards-list" class:two-cols={width >= 500}>
                  {#each filteredAttachedFiles as file}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div class="source-item-card file-card" on:click={() => onPreviewFile(file)}>
                      <div class="source-card-main">
                        <span class="source-type-icon file-icon"><FileText size={15} /></span>
                        <div class="source-text-block">
                          <span class="source-main-title" title={file.name}>{file.name}</span>
                          {#if file.size}
                            <span class="source-meta-sub">{Math.round(file.size / 1024)} KB</span>
                          {/if}
                        </div>
                        <button type="button" class="source-card-btn" title={language === "fr" ? "Prévisualiser" : "Preview"}>
                          <Eye size={13} />
                        </button>
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}

            <!-- Web & Context Sources Section -->
            {#if filteredContextSources.length > 0}
              <div class="sources-section">
                <span class="sources-section-title">
                  <Globe size={12} />
                  <span>{language === "fr" ? "Pages & Sources de contexte" : "Context & Web Sources"} ({filteredContextSources.length})</span>
                </span>
                <div class="sources-cards-list" class:two-cols={width >= 500}>
                  {#each filteredContextSources as source}
                    <div class="source-item-card web-card">
                      <div class="source-card-main">
                        <span class="source-type-icon web-icon">
                          {#if source.kind === "memory"}
                            <Brain size={15} />
                          {:else if source.kind === "workspace-search" || source.kind === "workspace-list"}
                            <FolderOpen size={15} />
                          {:else}
                            <Globe size={15} />
                          {/if}
                        </span>
                        <div class="source-text-block">
                          <span class="source-main-title" title={source.title || source.url || source.uri || "Source"}>
                            {source.title || source.url || source.uri || (language === "fr" ? "Source" : "Source")}
                          </span>
                          {#if source.url || source.uri}
                            <div class="source-link-row">
                              <a
                                href={source.url || source.uri}
                                target="_blank"
                                rel="noopener noreferrer"
                                class="source-url-link"
                                title={source.url || source.uri}
                              >
                                <span>{source.url || source.uri}</span>
                                <ExternalLink size={10} />
                              </a>
                            </div>
                          {/if}
                        </div>
                        {#if source.url || source.uri}
                          <div class="source-actions-group">
                            <button
                              type="button"
                              class="source-card-btn"
                              title={copiedSourceUrl === (source.url || source.uri) ? (language === "fr" ? "Copié !" : "Copied!") : (language === "fr" ? "Copier le lien" : "Copy link")}
                              on:click|stopPropagation={() => copySourceUrl(source.url || source.uri)}
                            >
                              {#if copiedSourceUrl === (source.url || source.uri)}
                                <Check size={13} style="color: #34c759;" />
                              {:else}
                                <Copy size={13} />
                              {/if}
                            </button>
                            <button
                              type="button"
                              class="source-card-btn"
                              title={language === "fr" ? "Ouvrir dans un nouvel onglet" : "Open in new tab"}
                              on:click|stopPropagation={() => window.open(source.url || source.uri, '_blank')}
                            >
                              <ExternalLink size={13} />
                            </button>
                          </div>
                        {/if}
                      </div>

                      {#if source.excerpt}
                        <div class="source-card-snippet">
                          <p>{source.excerpt}</p>
                        </div>
                      {/if}

                      {#if source.kind || source.score != null}
                        <div class="source-card-meta-row">
                          {#if source.kind}
                            <span class="source-chip kind-chip">{source.kind}</span>
                          {/if}
                          {#if source.score != null}
                            <span class="source-chip score-chip">
                              {language === "fr" ? "Pertinence" : "Relevance"} {Math.round(source.score * 100)}%
                            </span>
                          {/if}
                        </div>
                      {/if}
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          {/if}
        </div>
      </div>
    {/if}
  </div>
</aside>

<style>
  /* Frosted glass background adapting to dark theme classes globally */
  .right-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: rgba(250, 250, 252, 0.88);
    backdrop-filter: blur(20px);
    border-left: 1px solid rgba(0, 0, 0, 0.07);
    position: relative;
    z-index: 10;
    transition: width 0.1s ease-out;
  }

  :global(body.dark-theme) .right-panel {
    background: rgba(22, 22, 26, 0.9);
    border-left: 1px solid rgba(255, 255, 255, 0.08);
  }





  .segmented-tab-bar {
    display: flex;
    gap: 2px;
    background: rgba(0, 0, 0, 0.03);
    border-radius: 8px;
    margin: 14px 16px 8px 16px;
    padding: 3px;
    border: 0.5px solid rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .segmented-tab-bar {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.04);
  }

  .tab-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 1;
    height: 28px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #86868b;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .tab-btn:hover {
    color: #1d1d1f;
    background: rgba(0, 0, 0, 0.02);
  }

  :global(body.dark-theme) .tab-btn:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.03);
  }

  .tab-btn.active {
    background: #ffffff;
    color: #007aff;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08), 0 1px 1px rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .tab-btn.active {
    background: rgba(255, 255, 255, 0.12);
    color: #30d158; /* Dynamic green accent in dark mode matches subagent icons */
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  }

  .right-panel-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  /* Customized premium scrollbar */
  .right-panel-body::-webkit-scrollbar,
  .agent-lanes-scroll::-webkit-scrollbar,
  .explorer-sections::-webkit-scrollbar,
  .plan-tasks-list::-webkit-scrollbar,
  .browser-screen::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }

  .right-panel-body::-webkit-scrollbar-track,
  .agent-lanes-scroll::-webkit-scrollbar-track,
  .explorer-sections::-webkit-scrollbar-track,
  .plan-tasks-list::-webkit-scrollbar-track,
  .browser-screen::-webkit-scrollbar-track {
    background: transparent;
  }

  .right-panel-body::-webkit-scrollbar-thumb,
  .agent-lanes-scroll::-webkit-scrollbar-thumb,
  .explorer-sections::-webkit-scrollbar-thumb,
  .plan-tasks-list::-webkit-scrollbar-thumb,
  .browser-screen::-webkit-scrollbar-thumb {
    background: rgba(0, 0, 0, 0.15);
    border-radius: 3px;
  }

  .right-panel-body::-webkit-scrollbar-thumb:hover,
  .agent-lanes-scroll::-webkit-scrollbar-thumb:hover,
  .explorer-sections::-webkit-scrollbar-thumb:hover,
  .plan-tasks-list::-webkit-scrollbar-thumb:hover,
  .browser-screen::-webkit-scrollbar-thumb:hover {
    background: rgba(0, 0, 0, 0.25);
  }

  :global(body.dark-theme) .right-panel-body::-webkit-scrollbar-thumb,
  :global(body.dark-theme) .agent-lanes-scroll::-webkit-scrollbar-thumb,
  :global(body.dark-theme) .explorer-sections::-webkit-scrollbar-thumb,
  :global(body.dark-theme) .plan-tasks-list::-webkit-scrollbar-thumb,
  :global(body.dark-theme) .browser-screen::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.15);
  }

  :global(body.dark-theme) .right-panel-body::-webkit-scrollbar-thumb:hover,
  :global(body.dark-theme) .agent-lanes-scroll::-webkit-scrollbar-thumb:hover,
  :global(body.dark-theme) .explorer-sections::-webkit-scrollbar-thumb:hover,
  :global(body.dark-theme) .plan-tasks-list::-webkit-scrollbar-thumb:hover,
  :global(body.dark-theme) .browser-screen::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.25);
  }

  /* Empty state */
  .tab-empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 40px 20px;
    flex: 1;
  }

  .empty-icon-wrap {
    opacity: 0.35;
    margin-bottom: 12px;
    color: #86868b;
    display: inline-flex;
  }

  :global(body.dark-theme) .empty-icon-wrap {
    color: #a1a1a6;
  }

  .empty-title {
    font-size: 14px;
    font-weight: 600;
    color: #1d1d1f;
    margin: 0 0 4px 0;
  }

  :global(body.dark-theme) .empty-title {
    color: #ffffff;
  }

  .empty-desc {
    font-size: 11px;
    color: #86868b;
    line-height: 1.4;
    margin: 0;
  }

  :global(body.dark-theme) .empty-desc {
    color: #a1a1a6;
  }

  /* Dashboard Home Layout */
  .dashboard-home {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .dashboard-hero {
    background: linear-gradient(135deg, #1d1d1f 0%, #007aff 100%);
    border: 1px solid rgba(0, 122, 255, 0.12);
    border-radius: 14px;
    padding: 18px;
    color: #ffffff;
    box-shadow: 0 4px 16px rgba(0, 122, 255, 0.12);
  }

  :global(body.dark-theme) .dashboard-hero {
    background: linear-gradient(135deg, #0e0e10 0%, #1a365d 100%);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .dashboard-hero h3 {
    margin: 0 0 4px 0;
    font-size: 16px;
    font-weight: 700;
    color: #ffffff;
  }

  .dashboard-hero p {
    margin: 0;
    font-size: 11.5px;
    color: rgba(255, 255, 255, 0.85);
    line-height: 1.45;
  }

  .dashboard-grid {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* Responsive 2-column grid layout for wide panels */
  .dashboard-grid.two-cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .dashboard-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    border: 1px solid rgba(0, 0, 0, 0.05);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.7);
    backdrop-filter: blur(8px);
    text-align: left;
    cursor: pointer;
    transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
    width: 100%;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.015);
  }

  :global(body.dark-theme) .dashboard-card {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.06);
    backdrop-filter: blur(10px);
  }

  /* Colorful glow hover animations */
  .dashboard-card.outputs-card:hover {
    border-color: rgba(0, 122, 255, 0.3);
    box-shadow: 0 4px 16px rgba(0, 122, 255, 0.08);
    transform: translateY(-2px) scale(1.01);
  }

  .dashboard-card.agents-card:hover {
    border-color: rgba(52, 199, 89, 0.3);
    box-shadow: 0 4px 16px rgba(52, 199, 89, 0.08);
    transform: translateY(-2px) scale(1.01);
  }

  .dashboard-card.explorer-card:hover {
    border-color: rgba(255, 149, 0, 0.3);
    box-shadow: 0 4px 16px rgba(255, 149, 0, 0.08);
    transform: translateY(-2px) scale(1.01);
  }

  .dashboard-card.plan-card:hover {
    border-color: rgba(175, 82, 222, 0.3);
    box-shadow: 0 4px 16px rgba(175, 82, 222, 0.08);
    transform: translateY(-2px) scale(1.01);
  }

  .dashboard-card.browser-card:hover {
    border-color: rgba(90, 200, 250, 0.3);
    box-shadow: 0 4px 16px rgba(90, 200, 250, 0.08);
    transform: translateY(-2px) scale(1.01);
  }

  .dashboard-card.sources-card:hover {
    border-color: rgba(48, 176, 199, 0.3);
    box-shadow: 0 4px 16px rgba(48, 176, 199, 0.08);
    transform: translateY(-2px) scale(1.01);
  }

  /* Card click shrink effect */
  .dashboard-card:active {
    transform: translateY(-1px) scale(0.99);
  }

  .card-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: 9px;
    flex-shrink: 0;
  }

  /* Dashboard card category colors */
  .outputs-color { background: rgba(0, 122, 255, 0.08); color: #007aff; }
  .agents-color { background: rgba(52, 199, 89, 0.08); color: #34c759; }
  .explorer-color { background: rgba(255, 149, 0, 0.08); color: #ff9500; }
  .plan-color { background: rgba(175, 82, 222, 0.08); color: #af52de; }
  .browser-color { background: rgba(90, 200, 250, 0.08); color: #5ac8fa; }
  .sources-color { background: rgba(48, 176, 199, 0.08); color: #30b0c7; }

  .card-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .card-title {
    font-size: 13px;
    font-weight: 600;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .card-title {
    color: #ffffff;
  }

  .card-desc {
    font-size: 11px;
    color: #86868b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(body.dark-theme) .card-desc {
    color: #a1a1a6;
  }

  .card-arrow {
    color: #c7c7cc;
    transition: transform 0.2s ease;
    display: inline-flex;
  }

  .dashboard-card:hover .card-arrow {
    transform: translateX(3px);
    color: #86868b;
  }

  :global(body.dark-theme) .card-arrow {
    color: #515154;
  }

  :global(body.dark-theme) .dashboard-card:hover .card-arrow {
    color: #a1a1a6;
  }

  /* Tab View contents wrapper */
  .tab-content-view {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  /* Workspace Explorer Side-by-Side Layout */
  .explorer-tab-layout {
    display: flex;
    flex-direction: row;
    gap: 12px;
    height: 100%;
    min-height: 0;
    width: 100%;
    overflow: hidden;
  }

  .explorer-tree-pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    width: 100%;
    flex: 1;
    overflow: hidden;
    transition: width 0.25s cubic-bezier(0.16, 1, 0.3, 1), flex 0.25s ease;
  }

  .explorer-tree-pane.active-file-open {
    width: 290px;
    min-width: 250px;
    max-width: 340px;
    flex: 0 0 290px;
    border-right: 1px solid rgba(0, 0, 0, 0.08);
    padding-right: 10px;
  }

  :global(body.dark-theme) .explorer-tree-pane.active-file-open {
    border-right-color: rgba(255, 255, 255, 0.08);
  }

  .explorer-file-pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 320px;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    animation: fadeInFilePane 0.25s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes fadeInFilePane {
    from {
      opacity: 0;
      transform: translateX(10px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  /* OUTPUTS LIST */
  :global(.artifacts-list) {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  :global(.artifacts-list.two-cols) {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  :global(.artifact-item) {
    border: 1px solid rgba(0, 0, 0, 0.06);
    border-radius: 10px;
    padding: 12px;
    background: #ffffff;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  :global(body.dark-theme) :global(.artifact-item) {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.06);
  }

  :global(.artifact-head) {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  :global(.artifact-icon) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: rgba(0, 122, 255, 0.08);
    color: #007aff;
  }

  :global(.artifact-meta) {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  :global(.artifact-title-text) {
    font-size: 12.5px;
    font-weight: 600;
    color: #1d1d1f;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(body.dark-theme) :global(.artifact-title-text) {
    color: #ffffff;
  }

  :global(.artifact-kind) {
    font-size: 10px;
    color: #86868b;
  }

  :global(body.dark-theme) :global(.artifact-kind) {
    color: #a1a1a6;
  }

  :global(.artifact-actions) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-top: 1px solid rgba(0, 0, 0, 0.04);
    padding-top: 8px;
  }

  :global(body.dark-theme) :global(.artifact-actions) {
    border-top-color: rgba(255, 255, 255, 0.04);
  }

  :global(.artifact-path-label) {
    font-size: 11px;
    color: #86868b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 180px;
  }

  :global(body.dark-theme) :global(.artifact-path-label) {
    color: #a1a1a6;
  }

  :global(.artifact-action-btn) {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding: 0 8px;
    border: none;
    border-radius: 5px;
    background: rgba(0, 122, 255, 0.08);
    color: #007aff;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  :global(.artifact-action-btn:hover) {
    background: #007aff;
    color: #ffffff;
  }

  /* OUTPUTS TAB ADVANCED STYLES */
  .outputs-tab-container {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  .outputs-list-view {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .outputs-toolbar {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 2px;
  }

  .outputs-toolbar-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .outputs-heading {
    font-size: 13px;
    font-weight: 600;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .outputs-heading {
    color: #ffffff;
  }

  .outputs-count-badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 12px;
    background: rgba(0, 122, 255, 0.08);
    color: #007aff;
    font-weight: 500;
  }

  :global(body.dark-theme) .outputs-count-badge {
    background: rgba(0, 122, 255, 0.2);
    color: #5ac8fa;
  }

  .outputs-search-filter-row {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .outputs-search-box {
    position: relative;
    display: flex;
    align-items: center;
    background: rgba(0, 0, 0, 0.04);
    border: 1px solid rgba(0, 0, 0, 0.06);
    border-radius: 8px;
    padding: 0 10px;
    height: 32px;
    color: #86868b;
  }

  :global(body.dark-theme) .outputs-search-box {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.08);
    color: #a1a1a6;
  }

  .outputs-search-box input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: 12px;
    outline: none;
    margin-left: 6px;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .outputs-search-box input {
    color: #ffffff;
  }

  .clear-search-btn {
    border: none;
    background: transparent;
    cursor: pointer;
    color: #86868b;
    padding: 2px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .status-filter-pills {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding-bottom: 2px;
  }

  .filter-pill {
    border: 1px solid rgba(0, 0, 0, 0.06);
    background: rgba(0, 0, 0, 0.02);
    border-radius: 6px;
    font-size: 11px;
    padding: 3px 8px;
    cursor: pointer;
    color: #86868b;
    transition: all 0.15s ease;
    white-space: nowrap;
  }

  :global(body.dark-theme) .filter-pill {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.08);
    color: #a1a1a6;
  }

  .filter-pill:hover {
    color: #1d1d1f;
    background: rgba(0, 0, 0, 0.05);
  }

  :global(body.dark-theme) .filter-pill:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.08);
  }

  .filter-pill.active {
    background: #007aff;
    color: #ffffff;
    border-color: #007aff;
    font-weight: 500;
  }

  :global(body.dark-theme) .filter-pill.active {
    background: #007aff;
    color: #ffffff;
    border-color: #007aff;
  }

  .artifact-subline {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    flex-wrap: wrap;
  }

  .artifact-ext-badge {
    font-size: 9.5px;
    text-transform: uppercase;
    font-weight: 600;
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.05);
    color: #86868b;
  }

  :global(body.dark-theme) .artifact-ext-badge {
    background: rgba(255, 255, 255, 0.08);
    color: #a1a1a6;
  }

  .artifact-status-badge {
    font-size: 10px;
    font-weight: 500;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    padding: 1px 6px;
    border-radius: 4px;
  }

  .artifact-status-badge.applied {
    background: rgba(52, 199, 89, 0.12);
    color: #34c759;
  }

  .artifact-status-badge.rejected {
    background: rgba(255, 59, 48, 0.12);
    color: #ff3b30;
  }

  .artifact-status-badge.pending {
    background: rgba(255, 149, 0, 0.12);
    color: #ff9500;
  }

  .artifact-deltas {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 10px;
    font-weight: 600;
    margin-left: 2px;
  }

  .delta-pill.add {
    color: #34c759;
  }

  .delta-pill.del {
    color: #ff3b30;
  }

  .delta-pill.neutral {
    color: #86868b;
  }

  .artifact-card-btns {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .artifact-action-btn.preview {
    background: rgba(0, 122, 255, 0.08);
    color: #007aff;
  }

  .artifact-action-btn.preview:hover {
    background: #007aff;
    color: #ffffff;
  }

  .artifact-action-btn.apply {
    background: rgba(52, 199, 89, 0.1);
    color: #34c759;
  }

  .artifact-action-btn.apply:hover {
    background: #34c759;
    color: #ffffff;
  }

  .artifact-action-btn.reject {
    background: rgba(255, 59, 48, 0.08);
    color: #ff3b30;
  }

  .artifact-action-btn.reject:hover {
    background: #ff3b30;
    color: #ffffff;
  }

  /* Master-Detail View Styles */
  .artifact-detail-view {
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-height: 0;
  }

  .artifact-detail-top-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 2px;
  }

  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: none;
    background: transparent;
    font-size: 12px;
    font-weight: 500;
    color: #007aff;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 6px;
    transition: background 0.15s ease;
  }

  .back-btn:hover {
    background: rgba(0, 122, 255, 0.08);
  }

  :global(body.dark-theme) .back-btn {
    color: #5ac8fa;
  }

  :global(body.dark-theme) .back-btn:hover {
    background: rgba(90, 200, 250, 0.12);
  }

  .artifact-detail-badges {
    display: inline-flex;
    align-items: center;
  }

  .artifact-detail-summary-card {
    border: 1px solid rgba(0, 0, 0, 0.06);
    border-radius: 10px;
    padding: 12px;
    background: #ffffff;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  :global(body.dark-theme) .artifact-detail-summary-card {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.06);
  }

  .artifact-summary-header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .artifact-detail-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: flex-end;
    border-top: 1px solid rgba(0, 0, 0, 0.04);
    padding-top: 8px;
  }

  :global(body.dark-theme) .artifact-detail-actions {
    border-top-color: rgba(255, 255, 255, 0.04);
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 10px;
    border: none;
    border-radius: 6px;
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .action-btn.copy-btn {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  .action-btn.copy-btn:hover {
    background: rgba(0, 0, 0, 0.1);
  }

  :global(body.dark-theme) .action-btn.copy-btn {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  :global(body.dark-theme) .action-btn.copy-btn:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .action-btn.reject-btn {
    background: rgba(255, 59, 48, 0.1);
    color: #ff3b30;
  }

  .action-btn.reject-btn:hover:not(:disabled) {
    background: #ff3b30;
    color: #ffffff;
  }

  .action-btn.reject-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .action-btn.apply-btn {
    background: #007aff;
    color: #ffffff;
  }

  .action-btn.apply-btn:hover:not(:disabled) {
    background: #0062cc;
  }

  .action-btn.apply-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .artifact-diff-viewer-wrapper {
    flex: 1;
    min-height: 380px;
    display: flex;
    flex-direction: column;
    border-radius: 8px;
    overflow: hidden;
  }

  /* SUBAGENTS LIST */
  .agents-inbox-stats {
    display: flex;
    gap: 10px;
    margin-bottom: 16px;
  }

  .stat-badge {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 10px;
    border-radius: 12px;
    border: 1px solid rgba(0, 0, 0, 0.04);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.02);
    transition: all 0.2s ease;
  }

  :global(body.dark-theme) .stat-badge {
    border-color: rgba(255, 255, 255, 0.06);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }

  .stat-badge.running { 
    background: rgba(52, 199, 89, 0.04); 
    color: #34c759; 
    border-color: rgba(52, 199, 89, 0.1);
  }
  .stat-badge.queued { 
    background: rgba(255, 149, 0, 0.04); 
    color: #ff9500; 
    border-color: rgba(255, 149, 0, 0.1);
  }
  .stat-badge.done { 
    background: rgba(142, 142, 147, 0.04); 
    color: #8e8e93; 
    border-color: rgba(142, 142, 147, 0.1);
  }

  :global(body.dark-theme) .stat-badge.running { 
    background: rgba(48, 209, 88, 0.08); 
    color: #30d158; 
    border-color: rgba(48, 209, 88, 0.15);
  }
  :global(body.dark-theme) .stat-badge.queued { 
    background: rgba(255, 159, 10, 0.08); 
    color: #ff9f0a; 
    border-color: rgba(255, 159, 10, 0.15);
  }
  :global(body.dark-theme) .stat-badge.done { 
    background: rgba(152, 152, 157, 0.08); 
    color: #98989d; 
    border-color: rgba(152, 152, 157, 0.15);
  }

  .stat-badge:hover {
    transform: translateY(-1px);
    box-shadow: 0 4px 10px rgba(0, 0, 0, 0.04);
  }

  .stat-count {
    font-size: 18px;
    font-weight: 700;
    line-height: 1;
    margin-bottom: 4px;
  }

  .stat-label {
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.8px;
    opacity: 0.8;
  }

  .agent-lanes-scroll {
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
    flex: 1;
    padding-bottom: 20px;
  }

  .agent-lanes-scroll.two-cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    align-content: start;
  }

  .agent-lane-card {
    border: 1px solid rgba(0, 0, 0, 0.05);
    border-radius: 14px;
    background: #ffffff;
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
    transition: all 0.22s cubic-bezier(0.16, 1, 0.3, 1);
  }

  :global(body.dark-theme) .agent-lane-card {
    background: rgba(30, 30, 35, 0.45);
    border-color: rgba(255, 255, 255, 0.05);
  }

  .agent-lane-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 16px rgba(0, 0, 0, 0.04);
    border-color: rgba(0, 122, 255, 0.15);
  }

  :global(body.dark-theme) .agent-lane-card:hover {
    border-color: rgba(255, 255, 255, 0.1);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2);
  }

  .agent-lane-card.selected {
    border-color: rgba(52, 199, 89, 0.4);
    box-shadow: 0 0 0 2px rgba(52, 199, 89, 0.15);
  }

  :global(body.dark-theme) .agent-lane-card.selected {
    border-color: rgba(48, 209, 88, 0.4);
    box-shadow: 0 0 0 2px rgba(48, 209, 88, 0.15);
  }

  .agent-lane-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 24px;
  }

  .agent-lane-toggle-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    border: none;
    background: transparent;
    cursor: pointer;
    padding: 0;
    color: #1d1d1f;
    min-width: 0;
    text-align: left;
    flex: 1;
  }

  :global(body.dark-theme) .agent-lane-toggle-btn {
    color: #ffffff;
  }

  .agent-lane-title {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .lane-status-controls {
    display: flex;
    gap: 6px;
    margin-left: 8px;
  }

  .lane-action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    border: none;
    background: rgba(255, 59, 48, 0.08);
    color: #ff3b30;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .lane-action-btn:hover {
    background: #ff3b30;
    color: #ffffff;
    transform: scale(1.05);
  }

  :global(body.dark-theme) .lane-action-btn {
    background: rgba(255, 69, 58, 0.15);
    color: #ff453a;
  }

  :global(body.dark-theme) .lane-action-btn:hover {
    background: #ff453a;
    color: #ffffff;
  }

  .lane-action-btn.play {
    background: rgba(52, 199, 89, 0.08);
    color: #34c759;
  }

  .lane-action-btn.play:hover {
    background: #34c759;
    color: #ffffff;
  }

  :global(body.dark-theme) .lane-action-btn.play {
    background: rgba(48, 209, 88, 0.15);
    color: #30d158;
  }

  :global(body.dark-theme) .lane-action-btn.play:hover {
    background: #30d158;
    color: #ffffff;
  }

  .lane-body {
    margin-top: 10px;
    padding-top: 10px;
    border-top: 1px solid rgba(0, 0, 0, 0.04);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  :global(body.dark-theme) .lane-body {
    border-top-color: rgba(255, 255, 255, 0.05);
  }

  .lane-meta-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 10px;
    color: #86868b;
    padding: 0 2px;
  }

  :global(body.dark-theme) .lane-meta-row {
    color: #a1a1a6;
  }

  .lane-status-badge {
    text-transform: uppercase;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.5px;
    color: #34c759;
    background: rgba(52, 199, 89, 0.08);
    padding: 2px 6px;
    border-radius: 4px;
  }

  :global(body.dark-theme) .lane-status-badge {
    color: #30d158;
    background: rgba(48, 209, 88, 0.08);
  }

  .lane-priority {
    font-weight: 600;
    background: rgba(0, 0, 0, 0.03);
    padding: 2px 6px;
    border-radius: 4px;
  }

  :global(body.dark-theme) .lane-priority {
    background: rgba(255, 255, 255, 0.04);
  }

  .lane-runs-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: rgba(0, 0, 0, 0.015);
    border-radius: 8px;
    padding: 6px;
    margin-top: 4px;
  }

  :global(body.dark-theme) .lane-runs-list {
    background: rgba(255, 255, 255, 0.02);
  }

  .lane-run-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: 6px;
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
    min-width: 0;
    transition: all 0.15s ease;
    width: 100%;
  }

  .lane-run-row:hover {
    background: rgba(0, 0, 0, 0.03);
    transform: translateX(1px);
  }

  :global(body.dark-theme) .lane-run-row:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .lane-run-row.active {
    background: rgba(52, 199, 89, 0.06);
  }

  :global(body.dark-theme) .lane-run-row.active {
    background: rgba(48, 209, 88, 0.1);
  }

  .run-activity-wrap {
    color: #34c759;
    flex-shrink: 0;
    display: inline-flex;
  }

  :global(body.dark-theme) .run-activity-wrap {
    color: #30d158;
  }

  .run-goal {
    font-size: 11.5px;
    color: #1d1d1f;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  :global(body.dark-theme) .run-goal {
    color: #ffffff;
  }

  /* AGENTS SUB-TABS NAVIGATION */
  .agents-sub-tabs {
    display: flex;
    background: #f5f5f7;
    border-radius: 10px;
    padding: 2px;
    gap: 2px;
    margin-bottom: 16px;
  }

  :global(body.dark-theme) .agents-sub-tabs {
    background: rgba(30, 30, 35, 0.5);
  }

  .sub-tab-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: none;
    background: transparent;
    color: #86868b;
    padding: 6px;
    font-size: 11px;
    font-weight: 600;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .sub-tab-btn {
    color: #a1a1a6;
  }

  .sub-tab-btn:hover {
    color: #1d1d1f;
  }

  :global(body.dark-theme) .sub-tab-btn:hover {
    color: #ffffff;
  }

  .sub-tab-btn.active {
    background: #ffffff;
    color: #af52de;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .sub-tab-btn.active {
    background: rgba(255, 255, 255, 0.08);
    color: #c783eb;
    box-shadow: none;
  }

  /* CUSTOM AGENTS LIST VIEW */
  .agents-list-container {
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
    flex: 1;
    padding-bottom: 20px;
  }

  .list-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }

  .list-title {
    font-size: 13px;
    font-weight: 700;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .list-title {
    color: #ffffff;
  }

  .create-agent-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: #af52de;
    color: #ffffff;
    border: none;
    border-radius: 8px;
    padding: 5px 10px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .create-agent-btn:hover {
    background: #9b3ec7;
    transform: translateY(-1px);
  }

  .agents-cards-grid {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .agent-pers-card {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.04);
    border-radius: 12px;
    padding: 12px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.01);
    transition: all 0.2s ease;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  :global(body.dark-theme) .agent-pers-card {
    background: rgba(30, 30, 35, 0.4);
    border-color: rgba(255, 255, 255, 0.04);
  }

  .agent-pers-card:hover {
    border-color: rgba(175, 82, 222, 0.15);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.03);
  }

  .card-top {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .agent-avatar {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: #ffffff;
    flex-shrink: 0;
  }

  .agent-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .agent-name {
    font-size: 12.5px;
    font-weight: 600;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .agent-name {
    color: #ffffff;
  }

  .agent-desc {
    font-size: 10.5px;
    color: #86868b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .prompt-preview-box {
    background: #f5f5f7;
    border-radius: 6px;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  :global(body.dark-theme) .prompt-preview-box {
    background: rgba(255, 255, 255, 0.02);
  }

  .prompt-preview-title {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    color: #86868b;
    letter-spacing: 0.5px;
  }

  .prompt-preview-text {
    font-size: 10.5px;
    color: #515154;
    margin: 0;
    line-height: 1.35;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    white-space: pre-wrap;
  }

  :global(body.dark-theme) .prompt-preview-text {
    color: #a1a1a6;
  }

  .agent-card-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .action-btn {
    font-size: 10.5px;
    font-weight: 600;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.12s ease;
    border: 1px solid transparent;
  }

  .action-btn.chat-btn {
    background: rgba(175, 82, 222, 0.08);
    color: #af52de;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 10px;
  }

  .action-btn.chat-btn:hover {
    background: #af52de;
    color: #ffffff;
  }

  .action-btn.edit-btn {
    background: transparent;
    border-color: rgba(0, 0, 0, 0.08);
    color: #86868b;
    padding: 4px 8px;
  }

  :global(body.dark-theme) .action-btn.edit-btn {
    border-color: rgba(255, 255, 255, 0.08);
    color: #a1a1a6;
  }

  .action-btn.edit-btn:hover {
    background: rgba(0, 0, 0, 0.03);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .action-btn.edit-btn:hover {
    background: rgba(255, 255, 255, 0.04);
    color: #ffffff;
  }

  .action-btn.delete-btn {
    background: transparent;
    color: #c7c7cc;
    padding: 4px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .action-btn.delete-btn:hover {
    color: #ff3b30;
    background: rgba(255, 59, 48, 0.05);
  }

  /* WORKSPACE EXPLORER LIST */
  .explorer-sections {
    display: flex;
    flex-direction: column;
    gap: 16px;
    overflow-y: auto;
    flex: 1;
  }

  .explorer-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .group-title {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #86868b;
  }

  :global(body.dark-theme) .group-title {
    color: #a1a1a6;
  }

  .explorer-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .explorer-list.two-cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .explorer-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 8px;
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background 0.12s ease;
    width: 100%;
  }

  .explorer-item:hover {
    background: rgba(0, 0, 0, 0.03);
  }

  :global(body.dark-theme) .explorer-item:hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .file-icon-wrap {
    color: #86868b;
    display: inline-flex;
  }

  :global(body.dark-theme) .file-icon-wrap {
    color: #a1a1a6;
  }

  .file-name {
    font-size: 12px;
    color: #1d1d1f;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  :global(body.dark-theme) .file-name {
    color: #ffffff;
  }

  .explorer-item-web {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    background: rgba(0, 0, 0, 0.015);
    border-radius: 8px;
    border: 1px solid rgba(0, 0, 0, 0.03);
  }

  :global(body.dark-theme) .explorer-item-web {
    background: rgba(30, 30, 35, 0.4);
    border-color: rgba(255, 255, 255, 0.04);
  }

  .web-meta {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .web-icon-wrap {
    color: #5ac8fa;
    display: inline-flex;
  }

  .web-title {
    font-size: 12px;
    font-weight: 600;
    color: #1d1d1f;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(body.dark-theme) .web-title {
    color: #ffffff;
  }

  .web-url {
    font-size: 10px;
    color: #86868b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding-left: 20px;
  }

  /* WORKSPACE PLAN */
  .plan-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .plans-dashboard, .plan-detail, .new-plan-form {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .dashboard-header, .form-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
    padding-bottom: 12px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.05);
  }

  :global(body.dark-theme) .dashboard-header,
  :global(body.dark-theme) .form-header {
    border-bottom-color: rgba(255, 255, 255, 0.06);
  }

  .dashboard-header h3, .form-header h3 {
    font-size: 14px;
    font-weight: 700;
    color: #1d1d1f;
    margin: 0;
  }

  :global(body.dark-theme) .dashboard-header h3,
  :global(body.dark-theme) .form-header h3 {
    color: #ffffff;
  }

  .new-plan-btn, .submit-plan-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: #007aff;
    color: #ffffff;
    border: none;
    border-radius: 99px;
    padding: 6px 12px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .new-plan-btn:hover, .submit-plan-btn:hover {
    background: #0062cc;
    transform: translateY(-1px);
  }

  .submit-plan-btn:disabled {
    background: #a1a1a6;
    cursor: not-allowed;
    transform: none;
  }

  .close-form-btn {
    background: transparent;
    border: none;
    color: #86868b;
    cursor: pointer;
    padding: 4px;
    border-radius: 50%;
    display: inline-flex;
    transition: background 0.12s;
  }

  .close-form-btn:hover {
    background: rgba(0, 0, 0, 0.05);
  }

  :global(body.dark-theme) .close-form-btn:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  /* Form Elements */
  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-bottom: 14px;
  }

  .form-group label {
    font-size: 11px;
    font-weight: 600;
    color: #86868b;
  }

  .form-group input, .form-group textarea {
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 8px;
    padding: 8px 10px;
    font-size: 12px;
    outline: none;
    background: #ffffff;
    color: #1d1d1f;
    transition: border-color 0.15s ease;
  }

  :global(body.dark-theme) .form-group input,
  :global(body.dark-theme) .form-group textarea {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .form-group input:focus, .form-group textarea:focus {
    border-color: #007aff;
  }

  .submit-plan-btn {
    margin-top: 8px;
    justify-content: center;
    padding: 8px;
    border-radius: 8px;
    font-size: 12px;
  }

  /* Plans grid dashboard */
  .plans-grid {
    display: flex;
    flex-direction: column;
    gap: 10px;
    overflow-y: auto;
    flex: 1;
    padding-bottom: 20px;
  }

  .plan-card-item {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.04);
    border-radius: 12px;
    padding: 14px;
    text-align: left;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 12px;
  }

  :global(body.dark-theme) .plan-card-item {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.05);
  }

  .plan-card-item:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
    border-color: rgba(0, 122, 255, 0.2);
  }

  .card-header-row {
    display: flex;
    align-items: center;
    margin-bottom: 6px;
  }

  .card-status-badge {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(0, 122, 255, 0.08);
    color: #007aff;
  }

  .card-status-badge.status-completed {
    background: rgba(52, 199, 89, 0.08);
    color: #34c759;
  }

  .card-status-badge.status-archived {
    background: rgba(142, 142, 147, 0.08);
    color: #8e8e93;
  }

  .card-title {
    font-size: 13px;
    font-weight: 600;
    color: #1d1d1f;
    margin: 0 0 4px 0;
  }

  :global(body.dark-theme) .card-title {
    color: #ffffff;
  }

  .card-desc {
    font-size: 11px;
    color: #86868b;
    margin: 0;
    line-height: 1.35;
  }

  .card-progress {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .progress-text-row {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    font-weight: 600;
    color: #86868b;
  }

  /* Plan Detail View */
  .back-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    color: #007aff;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    margin-bottom: 12px;
    padding: 4px 0;
  }

  .back-btn:hover {
    text-decoration: underline;
  }

  .plan-header-card {
    background: rgba(175, 82, 222, 0.04);
    border: 1px solid rgba(175, 82, 222, 0.08);
    border-radius: 12px;
    padding: 14px;
    margin-bottom: 16px;
  }

  .plan-title {
    font-size: 15px;
    font-weight: 700;
    color: #1d1d1f;
    margin: 0 0 6px 0;
  }

  :global(body.dark-theme) .plan-title {
    color: #ffffff;
  }

  .plan-desc {
    font-size: 12px;
    color: #86868b;
    margin: 0 0 12px 0;
    line-height: 1.4;
  }

  .plan-progress-circle-row {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .progress-ring-container {
    position: relative;
    width: 48px;
    height: 48px;
    display: inline-flex;
  }

  .progress-ring-container svg {
    transform: rotate(-90deg);
  }

  .ring-bg {
    fill: none;
    stroke: rgba(0, 0, 0, 0.04);
    stroke-width: 4;
  }

  :global(body.dark-theme) .ring-bg {
    stroke: rgba(255, 255, 255, 0.06);
  }

  .ring-fg {
    fill: none;
    stroke: #af52de;
    stroke-width: 4;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.35s ease;
  }

  .progress-pct {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    font-size: 10px;
    font-weight: 700;
    color: #af52de;
  }

  .progress-text-details {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .progress-label {
    font-size: 11px;
    font-weight: 600;
    color: #86868b;
  }

  .progress-counts {
    font-size: 13px;
    font-weight: 700;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .progress-counts {
    color: #ffffff;
  }

  .plan-tasks-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    flex: 1;
    margin-bottom: 16px;
    padding-right: 4px;
  }

  /* Add Task Input Row Styling */
  .add-task-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
    background: #f5f5f7;
    border: 1px solid rgba(0, 0, 0, 0.04);
    border-radius: 8px;
    padding: 2px 4px 2px 10px;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .add-task-row {
    background: rgba(30, 30, 35, 0.5);
    border-color: rgba(255, 255, 255, 0.05);
  }

  .add-task-row:focus-within {
    border-color: rgba(175, 82, 222, 0.5);
    box-shadow: 0 0 0 2px rgba(175, 82, 222, 0.15);
    background: #ffffff;
  }

  :global(body.dark-theme) .add-task-row:focus-within {
    background: rgba(20, 20, 25, 0.8);
  }

  .add-task-row input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: 12px;
    color: #1d1d1f;
    outline: none;
    padding: 8px 0;
  }

  :global(body.dark-theme) .add-task-row input {
    color: #ffffff;
  }

  .add-task-row button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: #af52de;
    color: #ffffff;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .add-task-row button:hover {
    background: #9b3ec7;
    transform: scale(1.05);
  }

  .add-task-row button:active {
    transform: scale(0.95);
  }

  .task-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border: 1px solid rgba(0, 0, 0, 0.04);
    border-radius: 8px;
    background: #ffffff;
    transition: all 0.2s ease;
  }

  :global(body.dark-theme) .task-row {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.05);
  }

  .task-row.completed {
    opacity: 0.65;
    background: rgba(0, 0, 0, 0.01);
  }

  :global(body.dark-theme) .task-row.completed {
    background: rgba(255, 255, 255, 0.01);
  }

  .task-text-input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: 12px;
    color: #1d1d1f;
    outline: none;
    padding: 2px 0;
    min-width: 0;
  }

  :global(body.dark-theme) .task-text-input {
    color: #ffffff;
  }

  .task-row.completed .task-text-input {
    text-decoration: line-through;
    color: #86868b;
  }

  .task-checkbox {
    border: none;
    background: transparent;
    cursor: pointer;
    padding: 0;
    color: #86868b;
    display: inline-flex;
    transition: transform 0.1s ease;
  }

  .task-checkbox:active {
    transform: scale(0.9);
  }

  .check-icon-wrap {
    color: #af52de;
    display: inline-flex;
  }

  .task-delete-btn {
    border: none;
    background: transparent;
    color: #c7c7cc;
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: inline-flex;
    transition: all 0.12s ease;
    opacity: 0;
  }

  .task-row:hover .task-delete-btn {
    opacity: 1;
  }

  .task-delete-btn:hover {
    color: #ff3b30;
    background: rgba(255, 59, 48, 0.05);
  }

  :global(body.dark-theme) .task-delete-btn:hover {
    background: rgba(255, 69, 58, 0.15);
    color: #ff453a;
  }

  .plan-actions-footer {
    display: flex;
    gap: 8px;
    padding-top: 12px;
    border-top: 1px solid rgba(0, 0, 0, 0.05);
    margin-top: auto;
  }

  :global(body.dark-theme) .plan-actions-footer {
    border-top-color: rgba(255, 255, 255, 0.06);
  }

  .footer-btn {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    background: rgba(0, 0, 0, 0.03);
    border: none;
    border-radius: 8px;
    padding: 8px 10px;
    font-size: 11px;
    font-weight: 600;
    color: #515154;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  :global(body.dark-theme) .footer-btn {
    background: rgba(255, 255, 255, 0.04);
    color: #a1a1a6;
  }

  .footer-btn:hover {
    background: rgba(0, 0, 0, 0.06);
  }

  :global(body.dark-theme) .footer-btn:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .footer-btn.complete-btn {
    background: rgba(52, 199, 89, 0.08);
    color: #34c759;
  }

  .footer-btn.complete-btn:hover {
    background: #34c759;
    color: #ffffff;
  }

  .footer-btn.delete-btn:hover {
    background: rgba(255, 59, 48, 0.08);
    color: #ff3b30;
  }

  .plans-loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    gap: 8px;
    color: #86868b;
    font-size: 12px;
  }

  /* WEB BROWSER SIMULATOR */
  .browser-address-bar {
    margin-bottom: 8px;
  }

  .browser-input-row {
    display: flex;
    align-items: center;
    gap: 6px;
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 6px;
    padding: 0 8px;
    background: rgba(0, 0, 0, 0.02);
  }

  :global(body.dark-theme) .browser-input-row {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .browser-icon-wrap {
    color: #86868b;
    display: inline-flex;
  }

  .browser-input-row input {
    flex: 1;
    height: 28px;
    border: none;
    background: transparent;
    font-size: 11px;
    outline: none;
    color: #515154;
  }

  :global(body.dark-theme) .browser-input-row input {
    color: #ffffff;
  }

  .browser-search-box {
    display: flex;
    gap: 6px;
    margin-bottom: 12px;
  }

  .browser-search-box input {
    flex: 1;
    height: 30px;
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 6px;
    padding: 0 10px;
    font-size: 11.5px;
    outline: none;
  }

  :global(body.dark-theme) .browser-search-box input {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .browser-search-box button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border: none;
    border-radius: 6px;
    background: #007aff;
    color: #ffffff;
    cursor: pointer;
  }

  .browser-screen {
    flex: 1;
    background: rgba(0, 0, 0, 0.015);
    border: 1px dashed rgba(0, 0, 0, 0.08);
    border-radius: 8px;
    padding: 12px;
    min-height: 220px;
    overflow-y: auto;
    position: relative;
    display: flex;
    flex-direction: column;
  }

  :global(body.dark-theme) .browser-screen {
    background: rgba(0, 0, 0, 0.15);
    border-color: rgba(255, 255, 255, 0.06);
  }

  .browser-loading-overlay {
    position: absolute;
    inset: 0;
    background: rgba(255, 255, 255, 0.85);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    color: #86868b;
  }

  :global(body.dark-theme) .browser-loading-overlay {
    background: rgba(30, 30, 35, 0.9);
    color: #a1a1a6;
  }

  .spinner {
    width: 18px;
    height: 18px;
    border: 2px solid rgba(0, 122, 255, 0.15);
    border-top-color: #007aff;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    margin-bottom: 8px;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .browser-search-results {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .browser-search-results.two-cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .results-kicker {
    font-size: 10px;
    font-weight: 700;
    color: #86868b;
    text-transform: uppercase;
    grid-column: span 2;
  }

  .search-result-card {
    background: #ffffff;
    border: 1px solid rgba(0, 0, 0, 0.04);
    border-radius: 6px;
    padding: 10px;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
  }

  :global(body.dark-theme) .search-result-card {
    background: rgba(30, 30, 35, 0.6);
    border-color: rgba(255, 255, 255, 0.06);
  }

  .result-title {
    font-size: 12px;
    font-weight: 600;
    color: #007aff;
    text-decoration: none;
    display: block;
    margin-bottom: 2px;
  }

  .result-title:hover {
    text-decoration: underline;
  }

  .result-url {
    font-size: 9.5px;
    color: #34c759;
    display: block;
    margin-bottom: 4px;
  }

  .result-snippet {
    font-size: 11px;
    color: #515154;
    line-height: 1.35;
    margin: 0;
  }

  :global(body.dark-theme) .result-snippet {
    color: #a1a1a6;
  }

  .browser-home {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 30px 10px;
    flex: 1;
  }

  .browser-home-icon-wrap {
    color: #5ac8fa;
    opacity: 0.6;
    margin-bottom: 8px;
    display: inline-flex;
  }

  .browser-home-title {
    font-size: 13px;
    font-weight: 600;
    color: #1d1d1f;
    margin: 0 0 4px 0;
  }

  :global(body.dark-theme) .browser-home-title {
    color: #ffffff;
  }

  .browser-home-desc {
    font-size: 11px;
    color: #86868b;
    line-height: 1.35;
    margin: 0 0 14px 0;
  }

  .browser-home-tip {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(90, 200, 250, 0.06);
    border: 0.5px solid rgba(90, 200, 250, 0.12);
    border-radius: 6px;
    padding: 8px;
    text-align: left;
    font-size: 10px;
    color: #2b84ab;
  }

  /* Micro animations */
  .animate-fade-in {
    animation: fadeIn 0.16s ease-out forwards;
  }

  .animate-slide-down {
    animation: slideDown 0.18s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: scale(0.98); }
    to { opacity: 1; transform: scale(1); }
  }

  @keyframes slideDown {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  /* Agent run live inspection styles */
  .agent-run-detail-view {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
    overflow-y: auto;
  }
  .run-detail-top-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-bottom: 8px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  }
  :global(body.dark-theme) .run-detail-top-bar {
    border-bottom-color: rgba(255, 255, 255, 0.08);
  }
  .run-detail-badges {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .run-detail-summary-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.02);
    border: 1px solid rgba(0, 0, 0, 0.06);
  }
  :global(body.dark-theme) .run-detail-summary-card {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.08);
  }
  .run-goal-heading {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
    font-size: 0.88rem;
    color: #1d1d1f;
  }
  :global(body.dark-theme) .run-goal-heading {
    color: #f1f5f9;
  }
  .run-checkpoint-text {
    font-size: 0.76rem;
    color: #64748b;
    margin: 0;
  }
  :global(body.dark-theme) .run-checkpoint-text {
    color: #94a3b8;
  }
  .run-meta-time {
    font-size: 0.72rem;
    color: #86868b;
  }
  .agent-log-viewer-container {
    flex: 1;
    min-height: 280px;
    display: flex;
    flex-direction: column;
  }
  .run-duration {
    font-size: 0.68rem;
    color: #86868b;
    margin-left: auto;
  }
  .stat-badge.failed {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  /* Plan tab roadmap and 4-state indicator styles */
  .header-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .ai-roadmap-header-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    background: rgba(99, 102, 241, 0.1);
    color: #6366f1;
    border: 1px solid rgba(99, 102, 241, 0.25);
    border-radius: 6px;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .ai-roadmap-header-btn:hover:not(:disabled) {
    background: rgba(99, 102, 241, 0.18);
    transform: translateY(-1px);
  }
  .ai-roadmap-header-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .empty-actions-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    margin-top: 14px;
    flex-wrap: wrap;
  }
  .ai-roadmap-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    background: linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%);
    color: #ffffff;
    border: none;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(99, 102, 241, 0.3);
    transition: all 0.15s ease;
  }
  .ai-roadmap-btn:hover:not(:disabled) {
    box-shadow: 0 4px 12px rgba(99, 102, 241, 0.4);
    transform: translateY(-1px);
  }
  .ai-roadmap-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .create-manual-plan-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    background: rgba(0, 0, 0, 0.04);
    color: #1d1d1f;
    border: 1px solid rgba(0, 0, 0, 0.08);
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  :global(body.dark-theme) .create-manual-plan-btn {
    background: rgba(255, 255, 255, 0.06);
    color: #f1f5f9;
    border-color: rgba(255, 255, 255, 0.1);
  }
  .create-manual-plan-btn:hover {
    background: rgba(0, 0, 0, 0.08);
  }
  :global(body.dark-theme) .create-manual-plan-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  .task-row.in-progress {
    border-color: rgba(59, 130, 246, 0.3);
    background: rgba(59, 130, 246, 0.03);
  }
  :global(body.dark-theme) .task-row.in-progress {
    border-color: rgba(59, 130, 246, 0.3);
    background: rgba(59, 130, 246, 0.06);
  }
  .task-row.has-error {
    border-color: rgba(239, 68, 68, 0.35);
    background: rgba(239, 68, 68, 0.03);
  }
  :global(body.dark-theme) .task-row.has-error {
    border-color: rgba(239, 68, 68, 0.4);
    background: rgba(239, 68, 68, 0.06);
  }
  .task-content-col {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .task-error-hint {
    font-size: 10px;
    color: #ef4444;
    margin-top: 1px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .status-icon-wrap {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .status-icon-wrap.pending {
    color: #86868b;
  }
  .status-icon-wrap.in-progress {
    color: #3b82f6;
  }
  .status-icon-wrap.completed {
    color: #10b981;
  }
  .status-icon-wrap.error {
    color: #ef4444;
  }
  :global(.animate-spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
  /* ============================================================
     SOURCES & REFERENCES TAB STYLES
     ============================================================ */
  .sources-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    gap: 12px;
  }

  .sources-toolbar {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-bottom: 12px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.06);
    flex-shrink: 0;
  }

  :global(body.dark-theme) .sources-toolbar {
    border-bottom-color: rgba(255, 255, 255, 0.08);
  }

  .sources-toolbar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .sources-title-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .sources-title-icon {
    color: #30b0c7;
    display: inline-flex;
    align-items: center;
  }

  .sources-title {
    margin: 0;
    font-size: 13.5px;
    font-weight: 700;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .sources-title {
    color: #ffffff;
  }

  .sources-count-badge {
    font-size: 10.5px;
    font-weight: 600;
    padding: 1px 7px;
    border-radius: 999px;
    background: rgba(48, 176, 199, 0.12);
    color: #30b0c7;
  }

  :global(body.dark-theme) .sources-count-badge {
    background: rgba(48, 176, 199, 0.2);
    color: #4ed8ee;
  }

  .sources-search-box {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
  }

  .sources-search-box input {
    width: 100%;
    padding: 7px 28px 7px 28px;
    border-radius: 8px;
    border: 1px solid rgba(0, 0, 0, 0.08);
    background: rgba(0, 0, 0, 0.02);
    font-size: 12px;
    outline: none;
    transition: all 0.15s ease;
    box-sizing: border-box;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .sources-search-box input {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .sources-search-box input:focus {
    border-color: #30b0c7;
    background: #ffffff;
    box-shadow: 0 0 0 2px rgba(48, 176, 199, 0.18);
  }

  :global(body.dark-theme) .sources-search-box input:focus {
    background: rgba(255, 255, 255, 0.08);
    border-color: #4ed8ee;
  }

  .sources-search-box :global(.sources-search-icon) {
    position: absolute;
    left: 8px;
    color: #86868b;
    pointer-events: none;
  }

  .sources-clear-btn {
    position: absolute;
    right: 8px;
    background: none;
    border: none;
    padding: 0;
    color: #86868b;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .sources-clear-btn:hover {
    color: #1d1d1f;
  }

  :global(body.dark-theme) .sources-clear-btn:hover {
    color: #ffffff;
  }

  .sources-filter-row {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow-x: auto;
    padding-bottom: 2px;
  }

  .sources-filter-pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border-radius: 999px;
    border: 1px solid rgba(0, 0, 0, 0.06);
    background: rgba(0, 0, 0, 0.02);
    font-size: 11px;
    font-weight: 500;
    color: #86868b;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .sources-filter-pill {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.08);
    color: #a1a1a6;
  }

  .sources-filter-pill:hover {
    color: #1d1d1f;
    background: rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .sources-filter-pill:hover {
    color: #ffffff;
    background: rgba(255, 255, 255, 0.08);
  }

  .sources-filter-pill.active {
    background: #30b0c7;
    border-color: #30b0c7;
    color: #ffffff;
    font-weight: 600;
  }

  :global(body.dark-theme) .sources-filter-pill.active {
    background: #30b0c7;
    border-color: #30b0c7;
    color: #ffffff;
  }

  .sources-scroll-body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    overflow-y: auto;
    flex: 1;
    padding-right: 2px;
  }

  .sources-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sources-section-title {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: #86868b;
  }

  :global(body.dark-theme) .sources-section-title {
    color: #a1a1a6;
  }

  .sources-cards-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sources-cards-list.two-cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .source-item-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.7);
    border: 1px solid rgba(0, 0, 0, 0.06);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.02);
    transition: all 0.15s ease;
  }

  :global(body.dark-theme) .source-item-card {
    background: rgba(30, 30, 35, 0.5);
    border-color: rgba(255, 255, 255, 0.06);
    box-shadow: none;
  }

  .source-item-card:hover {
    border-color: rgba(48, 176, 199, 0.35);
    background: rgba(255, 255, 255, 0.95);
    transform: translateY(-1px);
    box-shadow: 0 3px 10px rgba(0, 0, 0, 0.04);
  }

  :global(body.dark-theme) .source-item-card:hover {
    background: rgba(38, 38, 45, 0.7);
    border-color: rgba(78, 216, 238, 0.35);
    box-shadow: 0 3px 10px rgba(0, 0, 0, 0.2);
  }

  .source-item-card.file-card {
    cursor: pointer;
  }

  .source-card-main {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .source-type-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 7px;
    flex-shrink: 0;
  }

  .source-type-icon.file-icon {
    background: rgba(0, 122, 255, 0.08);
    color: #007aff;
  }

  .source-type-icon.web-icon {
    background: rgba(48, 176, 199, 0.1);
    color: #30b0c7;
  }

  .source-text-block {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .source-main-title {
    font-size: 12.5px;
    font-weight: 600;
    color: #1d1d1f;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(body.dark-theme) .source-main-title {
    color: #ffffff;
  }

  .source-meta-sub {
    font-size: 10.5px;
    color: #86868b;
  }

  .source-link-row {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .source-url-link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10.5px;
    color: #0071e3;
    text-decoration: none;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(body.dark-theme) .source-url-link {
    color: #2997ff;
  }

  .source-url-link:hover {
    text-decoration: underline;
  }

  .source-url-link span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source-actions-group {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .source-card-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: #86868b;
    cursor: pointer;
    transition: all 0.12s ease;
  }

  .source-card-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  :global(body.dark-theme) .source-card-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .source-card-snippet {
    padding: 6px 8px;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.02);
    border-left: 2px solid #30b0c7;
    margin-top: 2px;
  }

  :global(body.dark-theme) .source-card-snippet {
    background: rgba(255, 255, 255, 0.03);
    border-left-color: #4ed8ee;
  }

  .source-card-snippet p {
    margin: 0;
    font-size: 11px;
    line-height: 1.45;
    color: #6e6e73;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  :global(body.dark-theme) .source-card-snippet p {
    color: #a1a1a6;
  }

  .source-card-meta-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
  }

  .source-chip {
    font-size: 9.5px;
    font-weight: 500;
    padding: 1px 6px;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .source-chip.kind-chip {
    background: rgba(0, 0, 0, 0.04);
    color: #86868b;
  }

  :global(body.dark-theme) .source-chip.kind-chip {
    background: rgba(255, 255, 255, 0.06);
    color: #a1a1a6;
  }

  .source-chip.score-chip {
    background: rgba(52, 199, 89, 0.1);
    color: #34c759;
  }

  :global(body.dark-theme) .source-chip.score-chip {
    background: rgba(52, 199, 89, 0.16);
    color: #30d158;
  }

  .sources-empty-view {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 48px 24px;
    text-align: center;
    gap: 10px;
    flex: 1;
  }

  .sources-empty-icon {
    color: #c7c7cc;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 4px;
  }

  :global(body.dark-theme) .sources-empty-icon {
    color: #48484a;
  }

  .sources-empty-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #1d1d1f;
  }

  :global(body.dark-theme) .sources-empty-title {
    color: #ffffff;
  }

  .sources-empty-desc {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.5;
    color: #86868b;
    max-width: 320px;
  }

  :global(body.dark-theme) .sources-empty-desc {
    color: #a1a1a6;
  }
</style>

