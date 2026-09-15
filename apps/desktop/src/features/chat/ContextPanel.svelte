<script context="module" lang="ts">
  import type { AgentLaneView, AgentRun } from "../../lib/types";

  export type ContextSectionKey = "outputs" | "agentInbox" | "sources";
  export type AgentInboxLane = AgentLaneView & {
    collapsed: boolean;
    visibleRuns: AgentRun[];
    totalRuns: number;
    activeRunCount: number;
  };
  export type AgentInboxTotals = {
    running: number;
    queued: number;
    waiting: number;
    done: number;
  };
  export type ContextPanelLabels = {
    outputs: string;
    noArtifactsYet: string;
    sources: string;
    noSourcesYet: string;
  };
  export type ContextAttachedFile = { name: string; size: number };
</script>

<script lang="ts">
  import Activity from "@lucide/svelte/icons/activity";
  import Brain from "@lucide/svelte/icons/brain";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import FileText from "@lucide/svelte/icons/file-text";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Square from "@lucide/svelte/icons/square";
  import X from "@lucide/svelte/icons/x";
  import type {
    AgentArtifact,
    AgentRunPriority,
    AgentRunView,
    ContextSource,
  } from "../../lib/types";

  export let showContextPanel: boolean;
  export let windowWidth: number;
  export let language: "fr" | "en";
  export let labels: ContextPanelLabels;
  export let collapsedSections: Record<ContextSectionKey, boolean>;
  export let artifacts: AgentArtifact[];
  export let contextSources: ContextSource[];
  export let memorySources: ContextSource[];
  export let visibleSources: ContextSource[];
  export let attachedFiles: ContextAttachedFile[];
  export let agentInboxTotals: AgentInboxTotals;
  export let agentInboxLanes: AgentInboxLane[];
  export let agentRunsBusy: boolean;
  export let agentActionBusy: string | null;
  export let selectedAgentRunView: AgentRunView | null;
  export let agentTimelineCollapsed: boolean;
  export let onToggleSection: (section: ContextSectionKey) => void;
  export let onStartAgent: () => void | Promise<void>;
  export let onToggleAgentLane: (laneId: string) => void;
  export let onSetAgentLaneAction: (laneId: string, action: "pause" | "resume") => void | Promise<void>;
  export let onUpdateAgentLanePriority: (laneId: string, priority: AgentRunPriority) => void | Promise<void>;
  export let onOpenAgentRun: (runId: string) => void | Promise<void>;
  export let onSetAgentRunAction: (runId: string, action: "pause" | "resume" | "cancel") => void | Promise<void>;
  export let agentStatusTone: (status: string) => string;
  export let agentStatusLabel: (status: AgentRun["status"] | AgentInboxLane["lane"]["status"]) => string;
  export let agentPriorityLabel: (priority: AgentRunPriority) => string;
  export let agentLaneSummary: (lane: AgentInboxLane) => string;
  export let compactAgentGoal: (goal: string, maxChars?: number) => string;
  export let agentStepLabel: (kind: string) => string;
  export let formatFileSize: (size: number) => string;
</script>

<div class="context-card">
  {#if windowWidth < 768}
    <div class="context-mobile-header">
      <h3>{language === "fr" ? "Sorties & Agents" : "Outputs & Agents"}</h3>
      <button
        class="context-mobile-close-btn"
        type="button"
        on:click={() => (showContextPanel = false)}
      >
        <X size={16} />
      </button>
    </div>
  {/if}
  <div class="context-section">
    <button
      class="context-section-header context-section-toggle"
      type="button"
      aria-expanded={!collapsedSections.outputs}
      on:click={() => onToggleSection("outputs")}
    >
      <div class="context-section-icon outputs-icon">
        <FileText size={14} />
      </div>
      <span class="context-section-title">{labels.outputs}</span>
      {#if artifacts.length > 0}
        <span class="context-section-count">{artifacts.length}</span>
      {/if}
      <ChevronDown
        size={14}
        style={`transition: transform 160ms ease; transform: ${collapsedSections.outputs ? "rotate(-90deg)" : "rotate(0deg)"};`}
      />
    </button>
    {#if !collapsedSections.outputs}
      <div class="context-section-body context-section-scroll compact">
        {#if artifacts.length}
          <div class="context-files-list">
            {#each artifacts as artifact}
              <div class="context-file-item">
                <div class="context-file-icon">
                  <FileText size={13} />
                </div>
                <span class="context-file-name" title={artifact.title}>{artifact.title}</span>
                <span class="context-file-size">{artifact.kind}</span>
              </div>
            {/each}
          </div>
        {:else}
          <div class="context-empty-state">
            <span class="context-empty-text">{labels.noArtifactsYet}</span>
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="context-section agent-inbox-section" class:collapsed={collapsedSections.agentInbox}>
    <div class="context-section-header agent-inbox-header">
      <button
        class="context-section-title-button"
        type="button"
        aria-expanded={!collapsedSections.agentInbox}
        on:click={() => onToggleSection("agentInbox")}
      >
        <div class="context-section-icon sources-icon">
          <Activity size={14} />
        </div>
        <span class="context-section-title">Agent Inbox</span>
        {#if agentInboxTotals.running + agentInboxTotals.queued + agentInboxTotals.waiting > 0}
          <span class="context-section-count">
            {agentInboxTotals.running + agentInboxTotals.queued + agentInboxTotals.waiting}
          </span>
        {/if}
        <ChevronDown
          size={14}
          style={`transition: transform 160ms ease; transform: ${collapsedSections.agentInbox ? "rotate(-90deg)" : "rotate(0deg)"};`}
        />
      </button>
      <button
        class="agent-mini-action"
        type="button"
        title="Nouvel agent"
        aria-label="Nouvel agent"
        disabled={agentActionBusy === "new"}
        on:click={onStartAgent}
      >
        <Plus size={12} />
      </button>
    </div>
    {#if !collapsedSections.agentInbox}
      <div class="context-section-body agent-inbox-body">
        <div class="agent-orchestrator-summary">
          <span><strong>{agentInboxTotals.running}</strong> actifs</span>
          <span><strong>{agentInboxTotals.queued}</strong> en file</span>
          <span><strong>{agentInboxTotals.waiting}</strong> attente</span>
          <span><strong>{agentInboxTotals.done}</strong> finis</span>
        </div>
        {#if agentRunsBusy}
          <div class="context-empty-state">
            <span class="context-empty-text">Chargement...</span>
          </div>
        {:else if agentInboxLanes.length === 0}
          <div class="context-empty-state">
            <span class="context-empty-text">Aucun agent en cours</span>
          </div>
        {:else}
          <div class="agent-lane-list">
            {#each agentInboxLanes as laneView}
              <div
                class:selected={laneView.visibleRuns.some((run) => selectedAgentRunView?.run.id === run.id)}
                class="agent-lane-card"
              >
                <div class="agent-lane-head">
                  <button
                    class="agent-lane-toggle"
                    type="button"
                    aria-expanded={!laneView.collapsed}
                    on:click={() => onToggleAgentLane(laneView.lane.id)}
                  >
                    <ChevronDown
                      size={13}
                      style={`transition: transform 160ms ease; transform: ${laneView.collapsed ? "rotate(-90deg)" : "rotate(0deg)"};`}
                    />
                    <span class="agent-lane-title" title={laneView.lane.title}>{laneView.lane.title}</span>
                  </button>
                  <div class="agent-lane-legacy-meta" aria-hidden="true">
                    <span class="agent-lane-meta">
                      {laneView.runningCount} running · {laneView.queuedCount} queued · {laneView.waitingCount} waiting
                    </span>
                  </div>
                  <div class="agent-actions">
                    {#if laneView.lane.status === "active"}
                      <button
                        type="button"
                        title="Pause"
                        aria-label="Pause"
                        disabled={agentActionBusy === laneView.lane.id}
                        on:click={() => onSetAgentLaneAction(laneView.lane.id, "pause")}
                      >
                        <Pause size={12} />
                      </button>
                    {:else}
                      <button
                        type="button"
                        title="Reprendre"
                        aria-label="Reprendre"
                        disabled={agentActionBusy === laneView.lane.id}
                        on:click={() => onSetAgentLaneAction(laneView.lane.id, "resume")}
                      >
                        <Play size={12} />
                      </button>
                    {/if}
                  </div>
                </div>
                <div class="agent-lane-meta-row">
                  <span class={`agent-status-pill ${agentStatusTone(laneView.lane.status)}`}>
                    {agentStatusLabel(laneView.lane.status)}
                  </span>
                  <span class="agent-lane-meta" title={agentLaneSummary(laneView)}>{agentLaneSummary(laneView)}</span>
                </div>
                <div class="agent-lane-controls">
                  <select
                    aria-label="Priorite lane"
                    value={laneView.lane.priority}
                    disabled={agentActionBusy === laneView.lane.id}
                    on:change={(event) => onUpdateAgentLanePriority(laneView.lane.id, event.currentTarget.value as AgentRunPriority)}
                  >
                    <option value="low">basse</option>
                    <option value="normal">normale</option>
                    <option value="high">haute</option>
                    <option value="critical">critique</option>
                  </select>
                </div>
                {#if laneView.visibleRuns.length > 0}
                  <div class="context-files-list agent-lane-runs">
                    {#each laneView.visibleRuns as run}
                      <button
                        class:selected={selectedAgentRunView?.run.id === run.id}
                        class="context-file-item agent-run-row"
                        type="button"
                        on:click={() => onOpenAgentRun(run.id)}
                      >
                        <div class="context-file-icon">
                          <Activity size={13} />
                        </div>
                        <span class="context-file-name" title={run.goal}>{compactAgentGoal(run.goal)}</span>
                        <span class={`agent-status-pill ${agentStatusTone(run.status)}`}>
                          {agentStatusLabel(run.status)}
                        </span>
                      </button>
                    {/each}
                    {#if laneView.collapsed && laneView.totalRuns > laneView.visibleRuns.length}
                      <button class="agent-lane-more" type="button" on:click={() => onToggleAgentLane(laneView.lane.id)}>
                        Voir {laneView.totalRuns - laneView.visibleRuns.length} autres
                      </button>
                    {/if}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if selectedAgentRunView}
          <div class="agent-timeline">
            <div class="agent-timeline-head">
              <button
                class="agent-timeline-toggle"
                type="button"
                aria-expanded={!agentTimelineCollapsed}
                on:click={() => (agentTimelineCollapsed = !agentTimelineCollapsed)}
              >
                <ChevronDown
                  size={13}
                  style={`transition: transform 160ms ease; transform: ${agentTimelineCollapsed ? "rotate(-90deg)" : "rotate(0deg)"};`}
                />
                <span>{compactAgentGoal(selectedAgentRunView.run.goal, 42)}</span>
              </button>
              <div class="agent-actions">
                {#if selectedAgentRunView.run.status === "running"}
                  <button
                    type="button"
                    title="Pause"
                    aria-label="Pause"
                    disabled={agentActionBusy === selectedAgentRunView.run.id}
                    on:click={() => onSetAgentRunAction(selectedAgentRunView!.run.id, "pause")}
                  >
                    <Pause size={12} />
                  </button>
                {:else if selectedAgentRunView.run.status === "paused" || selectedAgentRunView.run.status === "waiting"}
                  <button
                    type="button"
                    title="Reprendre"
                    aria-label="Reprendre"
                    disabled={agentActionBusy === selectedAgentRunView.run.id}
                    on:click={() => onSetAgentRunAction(selectedAgentRunView!.run.id, "resume")}
                  >
                    <Play size={12} />
                  </button>
                {/if}
                {#if selectedAgentRunView.run.status !== "completed" && selectedAgentRunView.run.status !== "cancelled"}
                  <button
                    type="button"
                    title="Stop"
                    aria-label="Stop"
                    disabled={agentActionBusy === selectedAgentRunView.run.id}
                    on:click={() => onSetAgentRunAction(selectedAgentRunView!.run.id, "cancel")}
                  >
                    <Square size={12} />
                  </button>
                {/if}
              </div>
            </div>
            {#if !agentTimelineCollapsed}
              <div class="agent-timeline-status">
                <span class={`agent-status-pill ${agentStatusTone(selectedAgentRunView.run.status)}`}>
                  {agentStatusLabel(selectedAgentRunView.run.status)}
                </span>
                <span>{agentPriorityLabel(selectedAgentRunView.run.priority)}</span>
              </div>
              {#if selectedAgentRunView.run.checkpointSummary}
                <p class="agent-checkpoint">{selectedAgentRunView.run.checkpointSummary}</p>
              {/if}
              <div class="agent-step-list">
                {#each selectedAgentRunView.steps as step}
                  <div class="agent-step-item">
                    <span>{step.sequence}. {agentStepLabel(step.kind)}</span>
                    <span title={step.title}>{compactAgentGoal(step.title, 28)}</span>
                    <span class={`agent-status-pill ${agentStatusTone(step.status)}`}>{step.status}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="context-section">
    <button
      class="context-section-header context-section-toggle"
      type="button"
      aria-expanded={!collapsedSections.sources}
      on:click={() => onToggleSection("sources")}
    >
      <div class="context-section-icon sources-icon">
        <FolderOpen size={14} />
      </div>
      <span class="context-section-title">{labels.sources}</span>
      {#if (contextSources.length || attachedFiles.length) > 0}
        <span class="context-section-count">{contextSources.length || attachedFiles.length}</span>
      {/if}
      <ChevronDown
        size={14}
        style={`transition: transform 160ms ease; transform: ${collapsedSections.sources ? "rotate(-90deg)" : "rotate(0deg)"};`}
      />
    </button>
    {#if !collapsedSections.sources}
      <div class="context-section-body context-section-scroll">
        {#if memorySources.length}
          <div class="context-source-group memory-source-group" style="background: rgba(191,90,242,0.05); border: 1px solid rgba(191,90,242,0.18); border-radius: 10px; padding: 10px; margin-bottom: 12px;">
            <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px;">
              <span class="context-source-label" style="display: inline-flex; align-items: center; gap: 6px; color: #bf5af2; font-weight: 700; font-size: 11px; text-transform: uppercase; letter-spacing: 0.5px;">
                <Brain size={13} />
                {language === "fr" ? "Mémoire Cognitive Active" : "Active Cognitive Memory"}
              </span>
              <span style="font-size: 10px; font-weight: 700; padding: 1px 6px; border-radius: 8px; background: rgba(191,90,242,0.15); color: #bf5af2;">
                {memorySources.length}
              </span>
            </div>
            <div class="context-files-list" style="display: flex; flex-direction: column; gap: 6px;">
              {#each memorySources as source}
                <div class="context-file-item" style="display: flex; flex-direction: column; align-items: stretch; gap: 3px; padding: 8px; border-radius: 8px; background: rgba(255,255,255,0.7); border: 1px solid rgba(191,90,242,0.12); cursor: default;">
                  <div style="display: flex; align-items: center; justify-content: space-between; gap: 6px;">
                    <span class="context-file-name" style="font-weight: 600; font-size: 12px; color: #1d1d1f; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title={source.title}>{source.title}</span>
                    {#if source.score > 0}
                      <span style="font-size: 10px; font-weight: 700; color: #bf5af2; background: rgba(191,90,242,0.12); padding: 1px 5px; border-radius: 4px; flex-shrink: 0;">
                        {Math.round(source.score <= 1 ? source.score * 100 : source.score)}% match
                      </span>
                    {/if}
                  </div>
                  {#if source.excerpt}
                    <span style="font-size: 11px; color: #6e6e73; line-height: 1.35; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical;">
                      {source.excerpt}
                    </span>
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        {/if}
        {#if visibleSources.length}
          <div class="context-source-group">
            <span class="context-source-label">Sources</span>
            <div class="context-files-list">
              {#each visibleSources as source}
                <div class="context-file-item">
                  <div class="context-file-icon">
                    <FileText size={13} />
                  </div>
                  <span class="context-file-name" title={source.excerpt}>{source.title}</span>
                  <span class="context-file-size">{source.kind}</span>
                </div>
              {/each}
            </div>
          </div>
        {:else if contextSources.length === 0 && attachedFiles.length === 0}
          <div class="context-empty-state">
            <span class="context-empty-text">{labels.noSourcesYet}</span>
          </div>
        {:else if contextSources.length === 0}
          <div class="context-files-list">
            {#each attachedFiles as file}
              <div class="context-file-item">
                <div class="context-file-icon">
                  <FileText size={13} />
                </div>
                <span class="context-file-name" title={file.name}>{file.name}</span>
                <span class="context-file-size">{formatFileSize(file.size)}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>
