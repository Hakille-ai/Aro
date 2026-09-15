<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fade, fly } from "svelte/transition";

  // Spotlight variables & checks
  let isSpotlightMode = false;
  let isTauriEnv = false;
  if (typeof window !== "undefined") {
    const params = new URLSearchParams(window.location.search);
    isSpotlightMode = params.get("mode") === "spotlight";
    isTauriEnv = Boolean((window as any).__TAURI_INTERNALS__);
  }

  let spotlightInput = "";
  let spotlightInputEl: HTMLTextAreaElement;
  let spotlightMessages: ChatMessage[] = [];
  let spotlightSending = false;
  let spotlightConversationId: string | null = null;
  let spotlightPersonalityMenuOpen = false;

  async function submitSpotlightMessage() {
    const content = spotlightInput.trim();
    const filesToSend = attachedFiles;
    if ((!content && filesToSend.length === 0) || spotlightSending) return;
    if (!ensureCloudWriteAllowed("envoyer un message")) return;
    
    spotlightSending = true;
    spotlightInput = "";
    attachedFiles = [];
    
    const attachmentRefs = buildAttachmentRefs(filesToSend);
    const payloadContent = content || (attachmentRefs.length > 0 ? "Analyse les fichiers joints." : "");
    // Même injection de contexte que le composer principal (@mentions +
    // contenu des pièces jointes), sinon le modèle ne les voit jamais.
    let sendContent = payloadContent;
    try {
      const { contextBlock } = await buildMentionContext(payloadContent);
      if (contextBlock) sendContent = contextBlock;
    } catch (err) {
      console.warn("Spotlight mention context injection failed", err);
    }
    try {
      const attachmentContext = await buildLocalAttachmentContext(filesToSend);
      if (attachmentContext) sendContent = `${sendContent}\n\nContexte des pièces jointes :\n${attachmentContext}`;
    } catch (err) {
      console.warn("Spotlight attachment context injection failed", err);
    }

    // Add user message immediately
    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      conversationId: spotlightConversationId || "",
      role: "user",
      content: payloadContent,
      attachments: attachmentRefs,
      createdAt: new Date().toISOString(),
      tokenEstimate: null,
    };
    
    const assistantMsgId = crypto.randomUUID();
    const generatingMsg: ChatMessage = {
      id: assistantMsgId,
      conversationId: spotlightConversationId || "",
      role: "assistant",
      content: "",
      createdAt: new Date().toISOString(),
      tokenEstimate: null,
      isGenerating: true,
    };
    
    spotlightMessages = [...spotlightMessages, userMsg, generatingMsg];
    await tick();
    const resultsAreaBefore = document.querySelector(".spotlight-results");
    if (resultsAreaBefore) {
      resultsAreaBefore.scrollTop = resultsAreaBefore.scrollHeight;
    }
    
    try {
      const personalityId = selectedPersonalityId;
      const activePers = allPersonalities.find(p => p.id === personalityId) || allPersonalities[0];
      const systemPrompt = compileSystemPrompt(activePers, "chat", payloadContent);

      const response = await sendMessageStream({
        conversationId: spotlightConversationId,
        content: sendContent,
        mode: "chat",
        systemPrompt: systemPrompt,
        modelId: settings?.model.activeModelRef?.modelId ?? null,
        provider: settings?.model.activeModelRef?.providerId ?? null,
        attachments: attachmentRefs,
        webAccess,
        searchSettings: settings?.search ?? null,
      }, assistantMsgId);

      spotlightConversationId = response.conversation.id;

      // Met à jour les deux bulles temporaires (l'id de conversation réel
      // n'est connu qu'après réponse, notamment pour un nouveau fil).
      spotlightMessages = spotlightMessages.map(msg => {
        if (msg.id === assistantMsgId) {
          return {
            ...response.assistantMessage,
            content: response.assistantMessage.content,
            steps: msg.steps
          };
        }
        if (msg.id === userMsg.id) {
          return {
            ...response.userMessage,
            content: payloadContent,
          };
        }
        return msg;
      });

      if (response.agentRunId) {
        await refreshAgentRuns(response.agentRunId);
      }

      if (settings?.speakResponses) {
        await speak(response.assistantMessage.content);
      }
    } catch (err) {
      console.error("Spotlight message send failed", err);
      const alreadyRunning = (err as { alreadyRunning?: boolean })?.alreadyRunning;
      const friendly = alreadyRunning
        ? (currentLanguage === "fr"
            ? "Une réponse est déjà en cours. Réessayez quand elle est terminée."
            : "A response is already being generated. Try again when it finishes.")
        : normalizeError(err);
      // Erreur visible DANS l'overlay (errorMessage global y est invisible).
      spotlightMessages = spotlightMessages.map(msg =>
        msg.id === assistantMsgId
          ? { ...msg, content: friendly, isGenerating: false }
          : msg,
      );
      errorMessage = friendly;
    } finally {
      spotlightSending = false;
      await updateSpotlightWindowSize();
    }
  }

  async function handleSpotlightKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      await submitSpotlightMessage();
    } else if (e.key === "Escape") {
      e.preventDefault();
      hideSpotlightWindow();
    }
  }

  async function hideSpotlightWindow() {
    if (typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__)) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      getCurrentWindow().hide();
    }
  }

  function isTauriWindow() {
    return typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);
  }

  async function minimizeMainWindow(event: MouseEvent | undefined = undefined) {
    if (event) event.stopPropagation();
    if (!isTauriWindow()) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().minimize();
    } catch (err) {
      console.error("Minimize failed", err);
    }
  }

  async function toggleMaximizeMainWindow(event: MouseEvent | undefined = undefined) {
    if (event) event.stopPropagation();
    if (!isTauriWindow()) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      if (await win.isMaximized()) {
        await win.unmaximize();
      } else {
        await win.maximize();
      }
    } catch (err) {
      console.error("Maximize toggle failed", err);
    }
  }

  async function closeMainWindow(event: MouseEvent | undefined = undefined) {
    if (event) event.stopPropagation();
    if (!isTauriWindow()) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    } catch (err) {
      console.error("Close failed", err);
    }
  }

  async function toggleSpotlightRecording() {
    if (assistantSpeaking) {
      stopSpeaking();
      return;
    }
    if (!recording && !ensureCloudWriteAllowed("dicter un message")) return;
    if (!ensureVoiceRecordingReady()) {
      return;
    }
    voiceInputMode = "dictation";
    disarmHandsFreeVoice();
    if (recording) {
      voiceAutoListen = false;
      await stopRecording();
    } else {
      errorMessage = "";
      voiceAutoListen = false;
      await startRecording();
    }
  }

  async function startResizingWindow(e: MouseEvent) {
    if (!Boolean(window.__TAURI_INTERNALS__)) return;
    e.preventDefault();
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const appWindow = getCurrentWindow() as any;
      await appWindow.startResizing("bottom-right");
    } catch (err) {
      console.error("Failed to start resizing window", err);
    }
  }

  async function updateSpotlightWindowSize() {
    if (!isSpotlightMode || !Boolean(window.__TAURI_INTERNALS__)) return;
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const { LogicalSize } = await import("@tauri-apps/api/dpi");
      const appWindow = getCurrentWindow();
      
      // Wait for a tick so the DOM has updated
      await new Promise(resolve => setTimeout(resolve, 30));
      
      const container = document.querySelector(".spotlight-container");
      if (container) {
        let height = container.getBoundingClientRect().height;
        
        // If model menu is open, make sure window is tall enough to show it
        const modelMenu = container.querySelector(".model-menu");
        if (modelMenu) {
          const menuRect = modelMenu.getBoundingClientRect();
          const containerRect = container.getBoundingClientRect();
          const menuBottomRelative = menuRect.bottom - containerRect.top;
          if (menuBottomRelative > height) {
            height = menuBottomRelative;
          }
        }
        
        // If personality menu is open, make sure window is tall enough to show it
        const personalityMenu = container.querySelector(".personality-menu");
        if (personalityMenu) {
          const menuRect = personalityMenu.getBoundingClientRect();
          const containerRect = container.getBoundingClientRect();
          const menuBottomRelative = menuRect.bottom - containerRect.top;
          if (menuBottomRelative > height) {
            height = menuBottomRelative;
          }
        }
        
        // Vertical margins for the shell shadow: 8px top + 24px bottom = 32px
        const targetHeight = Math.ceil(height) + 32;
        
        // Preserve current width so manual resizing is not lost
        const currentSize = await appWindow.innerSize();
        const factor = await appWindow.scaleFactor();
        const currentWidth = Math.round(currentSize.width / factor);
        
        await appWindow.setSize(new LogicalSize(currentWidth > 0 ? currentWidth : 680, targetHeight));
      }
    } catch (err) {
      console.error("Failed to resize spotlight", err);
    }
  }

  async function expandToMainWindow() {
    if (spotlightConversationId) {
      const { emit } = await import("@tauri-apps/api/event");
      await emit("open-conversation", spotlightConversationId);
    }
    hideSpotlightWindow();
    
    if (Boolean(window.__TAURI_INTERNALS__)) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("show_main_window");
    }
  }
  import Bot from "@lucide/svelte/icons/bot";
  import Check from "@lucide/svelte/icons/check";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Circle from "@lucide/svelte/icons/circle";
  import CloudUpload from "@lucide/svelte/icons/cloud-upload";
  import Edit2 from "@lucide/svelte/icons/edit-2";
  import FileText from "@lucide/svelte/icons/file-text";
  import Mic from "@lucide/svelte/icons/mic";
  import MoreVertical from "@lucide/svelte/icons/more-vertical";
  import MoreHorizontal from "@lucide/svelte/icons/more-horizontal";
  import Minus from "@lucide/svelte/icons/minus";
  import Plus from "@lucide/svelte/icons/plus";
  import RefreshCw from "@lucide/svelte/icons/refresh-cw";
  import Send from "@lucide/svelte/icons/send";
  import Settings from "@lucide/svelte/icons/settings";
  import Trash2 from "@lucide/svelte/icons/trash-2";
  import User from "@lucide/svelte/icons/user";
  import X from "@lucide/svelte/icons/x";
  import Pause from "@lucide/svelte/icons/pause";
  import Play from "@lucide/svelte/icons/play";
  import Square from "@lucide/svelte/icons/square";
  import Search from "@lucide/svelte/icons/search";
  import ThumbsUp from "@lucide/svelte/icons/thumbs-up";
  import ArrowLeft from "@lucide/svelte/icons/arrow-left";
  import Cpu from "@lucide/svelte/icons/cpu";
  import Volume2 from "@lucide/svelte/icons/volume-2";
  import FolderOpen from "@lucide/svelte/icons/folder-open";
  import Activity from "@lucide/svelte/icons/activity";
  import ThumbsDown from "@lucide/svelte/icons/thumbs-down";
  import Copy from "@lucide/svelte/icons/copy";
  import MessageSquare from "@lucide/svelte/icons/message-square";
  import Sliders from "@lucide/svelte/icons/sliders";
  import LineChart from "@lucide/svelte/icons/line-chart";
  import Building2 from "@lucide/svelte/icons/building-2";
  import Users from "@lucide/svelte/icons/users";
  import Shield from "@lucide/svelte/icons/shield";
  import Mail from "@lucide/svelte/icons/mail";
  import Key from "@lucide/svelte/icons/key";
  import Lock from "@lucide/svelte/icons/lock";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import Maximize2 from "@lucide/svelte/icons/maximize-2";
  import Briefcase from "@lucide/svelte/icons/briefcase";
  import Brain from "@lucide/svelte/icons/brain";
  import Database from "@lucide/svelte/icons/database";
  import ChevronsUpDown from "@lucide/svelte/icons/chevrons-up-down";
  import Terminal from "@lucide/svelte/icons/terminal";
  import Info from "@lucide/svelte/icons/info";
  import Puzzle from "@lucide/svelte/icons/puzzle";
  import Network from "@lucide/svelte/icons/network";
  import Server from "@lucide/svelte/icons/server";
  import Link from "@lucide/svelte/icons/link";
  import Globe2 from "@lucide/svelte/icons/globe-2";
  import Zap from "@lucide/svelte/icons/zap";
  import Clock from "@lucide/svelte/icons/clock";
  import LogOut from "@lucide/svelte/icons/log-out";
  import Keyboard from "@lucide/svelte/icons/keyboard";
  import { renderMarkdown } from "./lib/markdown";
  import {
    acceptCloudInvitation,
    bootstrap,
    checkVoiceModelStatus,
    checkRuntime,
    createCloudApiKey,
    createCloudCollectionItem,
    createCloudOrganization,
    createConversation,
    deleteCloudCollectionItem,
    deleteConversation,
    deleteEmptyConversations,
    getCloudSession,
    getCloudCollection,
    getAgentRun,
    deleteCloudState,
    deleteMemoryItem,
    clearModelProviderApiKey,
    inviteCloudMember,
    deleteModelProvider,
    listCloudApiKeys,
    listCloudOrganizations,
    listCloudMembers,
    listAllCloudInvitations,
    listAgentRuns,
    listMemoryItems,
    listPermissionProfiles,
    memoryIndexReindex,
    memoryIndexStatus,
    loginCloud,
    requestPasswordReset,
    confirmPasswordReset,
    sendDesktopNotification,
    modelOptionKey,
    logoutCloud,
    registerCloud,
    removeCloudMember,
    pauseAgentRun,
    refreshModelCatalog,
    resumeAgentLane,
    resumeAgentRun,
    setCloudState,
    setAgentLanePriority,
    selectModelRef,
    setModelProviderApiKey,
    switchCloudOrganization,
    testModelProvider,
    updateCloudCollectionItem,
    updateCloudMemberRole,
    updateCloudOrganization,
    updateCloudUserProfile,
    updateConversationTitle,
    exportConversation,
    updatePreferences,
    upsertPermissionProfile,
    listModels,
    listMessages,
    getAgentOrchestratorSnapshot,
    listAgentLanes,
    resetMemory,
    pauseAgentLane,
    cancelAgentRun,
    sendMessage,
    sendMessageStream,
    sendArenaStream,
    startAgentRun,
    synthesizeSpeech,
    detectWakeWord,
    transcribeAudio,
    updateSettings,
    uploadCloudFile,
    downloadCloudFile,
    upsertModelProvider,
    updateMessage,
    upsertMemoryItem,
    regenerateMessage,
    regenerateMessageStream,
    revokeCloudApiKey,
    revokeCloudInvitation,
    createFolder,
    createProject,
    deleteFolder,
    deleteProject,
    exportProject,
    importConversations,
    listFolders,
    listProjects,
    moveConversation,
    searchConversationContent,
    updateFolder,
    updateProject,
    listInstalledPlugins,
    pluginSkillsToUserSkills,
    pluginMcpServersToMcpServers,
    togglePlugin,
    listEpisodes,
  } from "./lib/api";
  import type {
    AgentLaneView,
    AgentOrchestratorSnapshot,
    AgentRun,
    AgentStep,
    AgentRunPriority,
    AgentRunView,
    AppSettings,
    AttachmentRef,
    AssistantMode,
    ChatMessage,
    CloudSessionView,
    Conversation,
    FileObject,
    Folder,
    LongTermMemoryItem,
    EpisodeItem,
    MemoryConfigurationSettings,
    MemoryIndexStatus,
    ModelOption,
    ModelProviderConnection,
    ModelProviderKind,
    Project,
    ModelRef,
    Organization,
    OrganizationInvitation,
    OrganizationMember,
    PermissionCommandApproval,
    PermissionPresetMode,
    PermissionProfile,
    RuntimeStatus,
    SyncStatus,
    VoiceCapabilityStatus,
    VoiceModelsStatus,
    VoiceProfile,
    WebAccessMode,
  } from "./lib/types";
  import type { ArenaSlotState, ArenaStreamChunk } from "./lib/types";
  import CustomSelect from "./lib/CustomSelect.svelte";
  import SplashScreen from "./features/shell/SplashScreen.svelte";
  import CreateOrganizationModal from "./features/organizations/CreateOrganizationModal.svelte";
  import AddProviderModal from "./features/models/AddProviderModal.svelte";
  import PreviewOverlays from "./features/files/PreviewOverlays.svelte";
  import WorkspacePopover from "./features/organizations/WorkspacePopover.svelte";
  import ImageLightboxModal from "./features/workspace/ImageLightboxModal.svelte";
  import {
    createInitialOrganizationInfo,
    createInitialUserProfile,
    currentOrganizationRole as currentOrganizationRoleForSession,
    filterOrganizationMembers,
    getInitials,
    invitationStatusLabel as organizationInvitationStatusLabel,
    isCurrentOrgMember,
    mapApiKeyRecord,
    mapOrganizationMember,
    mergeOrganizationInvitations,
    teamPayload,
  } from "./features/organizations/model";
  import type {
    ApiKeyRecord,
    OrganizationInfo,
    OrgMember,
    OrgTeam,
    UserProfile,
  } from "./features/organizations/model";
  import CloudAuthPage from "./features/auth/CloudAuthPage.svelte";
  import NotificationToastStack from "./features/notifications/NotificationToastStack.svelte";
  import type { ToastNotification } from "./features/notifications/model";
  import CommandPalette from "./features/command-palette/CommandPalette.svelte";
  import QuickOpenModal from "./features/workspace/QuickOpenModal.svelte";
  import {
    DEFAULT_SHORTCUTS,
    buildCommandPaletteResults,
    formatShortcut as formatShortcutModel,
    hasModifiers,
    loadShortcuts as loadShortcutModel,
    matchShortcutEvent,
    shortcutFromKeyboardEvent,
    updateShortcut as updateShortcutModel,
  } from "./features/command-palette/model";
  import type { ShortcutKey, Shortcuts } from "./features/command-palette/model";
  import RenameConversationModal from "./features/conversations/RenameConversationModal.svelte";
  import CreateProjectModal from "./features/projects/CreateProjectModal.svelte";
  import CreateFolderModal from "./features/folders/CreateFolderModal.svelte";
  import MoveToProjectFolderModal from "./features/conversations/MoveToProjectFolderModal.svelte";
  import InviteMemberModal from "./features/organizations/InviteMemberModal.svelte";
  import CreateTeamModal from "./features/organizations/CreateTeamModal.svelte";
  import PersonalityModal from "./features/personalities/PersonalityModal.svelte";
  import VoiceProfileModal from "./features/voice/VoiceProfileModal.svelte";
  import SpotlightPage from "./features/spotlight/SpotlightPage.svelte";
  import MainSidebar from "./features/shell/MainSidebar.svelte";
  import ModelsSettings from "./features/settings/pages/ModelsSettings.svelte";
  import SearchSettings from "./features/settings/pages/SearchSettings.svelte";
  import VoiceSettings from "./features/settings/pages/VoiceSettings.svelte";
  import PathsSettings from "./features/settings/pages/PathsSettings.svelte";
  import SystemSettings from "./features/settings/pages/SystemSettings.svelte";
  import PermissionsSettings from "./features/settings/pages/PermissionsSettings.svelte";
  import MemorySettings from "./features/settings/pages/MemorySettings.svelte";
  import PreferencesSettings from "./features/settings/pages/PreferencesSettings.svelte";
  import ShortcutsSettings from "./features/settings/pages/ShortcutsSettings.svelte";
  import SystemPromptSettings from "./features/settings/pages/SystemPromptSettings.svelte";
  import SkillsSettings from "./features/settings/pages/SkillsSettings.svelte";
  import ContextPanel from "./features/chat/ContextPanel.svelte";
  import InstructionsSettings from "./features/settings/pages/InstructionsSettings.svelte";
  import AgentsSettings from "./features/settings/pages/AgentsSettings.svelte";
  import AgentModal from "./features/chat/AgentModal.svelte";
  import MonitoringSettings from "./features/settings/pages/MonitoringSettings.svelte";
  import ConversationTopbar from "./features/chat/ConversationTopbar.svelte";
  import ProfileSettings from "./features/settings/pages/ProfileSettings.svelte";
  import PluginsSettings from "./features/settings/pages/PluginsSettings.svelte";
  import Composer from "./features/chat/Composer.svelte";
  import OrganizationSettings from "./features/settings/pages/OrganizationSettings.svelte";
  import RightPanel from "./features/shell/RightPanel.svelte";
  import ConfirmModal from "./features/shell/ConfirmModal.svelte";
  import { requestConfirm } from "./lib/confirm";
  import HooksSettings from "./features/settings/pages/HooksSettings.svelte";
  import { extractArtifactsFromMessages, mergeArtifacts } from "./lib/artifacts";
  import { applyWorkspaceDiff, getWorkspaceTree, readWorkspaceFile } from "./lib/api";
  import {
    dedupeMentionedPaths,
    extractMentionedPaths,
    flattenWorkspaceTreeToMentions,
    resolveMentionedEntries,
    type WorkspaceMentionEntry,
  } from "./features/chat/mention-model";
  import ConversationView from "./features/chat/ConversationView.svelte";
  import McpSettings from "./features/settings/pages/McpSettings.svelte";
  import SchedulerSettings from "./features/settings/pages/SchedulerSettings.svelte";
  import ArenaView from "./features/arena/ArenaView.svelte";
  import SkillEditorModal from "./features/skills/SkillEditorModal.svelte";
  import SettingsSidebar from "./features/settings/SettingsSidebar.svelte";
  import type { SettingsTab } from "./features/settings/types";
  import {
    createInitialSkillGroups,
    createInitialSkills,
    skillGroupPayload,
    skillPayload,
  } from "./features/skills/model";
  import type { UserSkill, UserSkillGroup } from "./features/skills/model";
  import {
    createInitialPlugins,
    getPluginsByCategory,
    isSensitiveIntegrationField,
    mergePluginPresets,
    pluginPayload,
    pluginSafeForLocalStorage,
  } from "./features/plugins/model";
  import type { UserPlugin } from "./features/plugins/model";
  import {
    DEFAULT_HOOK_FORM,
    DEFAULT_MCP_FORM,
    DEFAULT_SCHEDULER_FORM,
  } from "./features/integrations/model";
  import type {
    AroHook,
    McpServer,
    McpTransport,
    ScheduledTask,
    SchedulerType,
  } from "./features/integrations/model";
  import {
    buildActivityWeeks,
    createInitialMonitoringState,
    createPerformanceRecord,
    incrementActivityLog,
    populateMockActivity,
    prependPerformanceRecord,
    summarizePerformance,
  } from "./features/monitoring/model";
  import type { PerformanceRecord } from "./features/monitoring/model";
  import {
    INSTRUCTION_DEFAULTS_VERSION,
    buildDefaultSystemPrompts,
    compileSystemPrompt as compileAroSystemPrompt,
    composeInstructionPrompt,
    defaultInstructionFormatting,
    defaultInstructionIdentity,
    defaultInstructionRules,
    defaultPersonalitiesForLanguage,
    estimateInstructionTokens,
    instructionPresets,
    migrateInstructionDefaults,
    normalizeInstructionMode,
  } from "./lib/instructions";
  import type { InstructionMemory, InstructionPersonality } from "./lib/instructions";
  import {
    appendBoundedAudioChunk,
    buildVoiceAudioConstraints,
    createVoiceCaptureSession,
    encodeWav as encodeVoiceWav,
    EMPTY_AUDIO_CHUNK_BUFFER,
    isWhisperHallucination as isWhisperHallucinationText,
    samplesForDuration,
  } from "./lib/voice";
  import type { VoiceCaptureChunkMessage, VoiceCaptureSession } from "./lib/voice";

  type AttachedFile = {
    id: string;
    name: string;
    size: number;
    type: string;
    file: File;
    mode: "local-reference" | "cloud-object";
    uploadStatus: "local" | "uploading" | "uploaded" | "failed";
    fileId?: string | null;
    note?: string;
    error?: string;
  };

  type VoiceInputMode = "push-to-talk" | "dictation" | "hands-free";
  type VoiceSessionState = "capturing" | "transcribing" | "submitting" | "complete" | "cancelled";
  type ContextSectionKey = "outputs" | "agentInbox" | "sources";
  type AgentInboxLane = AgentLaneView & {
    collapsed: boolean;
    visibleRuns: AgentRun[];
    totalRuns: number;
    activeRunCount: number;
  };
  type VoiceSession = {
    id: string;
    conversationId: string | null;
    mode: VoiceInputMode;
    autoSubmit: boolean;
    startedAt: number;
    state: VoiceSessionState;
    transcript: string;
  };

  const DEFAULT_ATTACHMENT_MIME = "application/octet-stream";

  let conversations: Conversation[] = [];
  let projects: Project[] = [];
  let folders: Folder[] = [];
  let showCreateProjectModal = false;
  let projectToEdit: Project | null = null;
  let showCreateFolderModal = false;
  let folderToEdit: Folder | null = null;
  let folderDefaultProjectId: string | null = null;
  let showMoveModal = false;
  let conversationToMove: Conversation | null = null;
  // Destination choisie sur la page d'accueil pour la prochaine conversation.
  // Consommée à la création (création atomique), réinitialisée ensuite.
  let pendingProjectId: string | null = null;
  let pendingFolderId: string | null = null;
  let activeConversation: Conversation | null = null;
  let messages: ChatMessage[] = [];
  let settings: AppSettings | null = null;
  let settingsDraft: AppSettings | null = null;
  let modelOptions: ModelOption[] = [];
  let modelProviders: ModelProviderConnection[] = [];
  let modelProviderBusy: Record<string, boolean> = {};
  let modelProviderStatus: Record<string, RuntimeStatus> = {};
  let modelProviderKeyDrafts: Record<string, string> = {};
  let modelSearchQuery = "";
  let showAddProviderSheet = false;
  let addProviderKind: ModelProviderKind = "openai";
  let addProviderName = "OpenAI";
  let addProviderEndpoint = "https://api.openai.com/v1";
  let addProviderApiKey = "";
  let runtime: RuntimeStatus | null = null;
  let voiceModelStatus: VoiceModelsStatus | null = null;
  let cloudSession: CloudSessionView | null = null;
  let cloudAuthenticated = false;
  let cloudSyncStatus: SyncStatus = { health: "offline-read-only", pendingEvents: 0, lastSyncedAt: null };
  let apiBaseUrl = "";
  let cloudOrganizations: Organization[] = [];
  let cloudOrganizationBusy = false;
  let newOrganizationName = "";
  let showCloudAuthPanel = false;
  let showCreateOrgModal = false;
  let cloudAuthMode: "login" | "register" | "invitation" | "forgot-password" | "reset-password" = "login";
  let cloudInvitationToken = "";
  let cloudAuthEmail = "";
  let cloudAuthPassword = "";
  let cloudAuthName = "";
  let cloudAuthOrgName = "ARO Workspace";
  let cloudAuthError = "";
  let cloudAuthSuccessMessage = "";
  let cloudAuthDevTokenUrl = "";
  let cloudAuthBusy = false;
  let showPassword = false;
  let showQuickOpenModal = false;
  let lightboxImageUrl = "";
  let lightboxImageTitle = "";
  let toastNotifications: ToastNotification[] = [];

  if (typeof window !== "undefined") {
    (window as any).__openImageLightbox = (target: any, title: string = "") => {
      if (typeof target === "string") {
        lightboxImageUrl = target;
        lightboxImageTitle = title;
      } else if (target) {
        const img = target.tagName === "IMG" ? (target as HTMLImageElement) : target.querySelector("img");
        if (img?.src) {
          lightboxImageUrl = img.src;
          const rawAlt = img.getAttribute("data-alt");
          let altText = img.alt || "";
          if (rawAlt) {
            try { altText = decodeURIComponent(rawAlt); } catch {}
          }
          lightboxImageTitle = altText || "";
        }
      }
    };
    (window as any).__downloadImage = async (target: any) => {
      let url = "";
      if (typeof target === "string") {
        url = target;
      } else if (target) {
        const card = target.closest(".chat-image-card, .gallery-card-item");
        const img = card ? card.querySelector("img") : (target.tagName === "IMG" ? (target as HTMLImageElement) : null);
        if (img?.src) url = img.src;
      }
      if (!url) return;

      try {
        const response = await fetch(url);
        const blob = await response.blob();
        const blobUrl = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = blobUrl;
        a.download = url.split("/").pop() || "image-aro.png";
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(blobUrl);
      } catch {
        window.open(url, "_blank");
      }
    };

    (window as any).__handleImageError = (imgEl: any, text: string = "") => {
      if (!imgEl || !imgEl.parentElement) return;
      const parent = imgEl.parentElement;
      const rawAlt = imgEl.getAttribute("data-alt");
      let altText = text || imgEl.alt || "";
      if (rawAlt) {
        try { altText = decodeURIComponent(rawAlt); } catch {}
      }
      const fallback = document.createElement("div");
      fallback.className = "image-fallback-card";
      fallback.innerHTML = `
        <div class="fallback-icon">🖼️</div>
        <div class="fallback-info">
          <span class="fallback-title">Image non disponible</span>
          <span class="fallback-desc">${altText || "Source distante introuvable"}</span>
        </div>
      `;
      parent.replaceChild(fallback, imgEl);
    };

    (window as any).__setGalleryLayout = (btnEl: any, mode: string = "horizontal") => {
      const container = btnEl.closest(".gallery-block-container");
      if (!container) return;
      const grid = container.querySelector(".futuristic-gallery-grid");
      const buttons = container.querySelectorAll(".layout-btn");
      buttons.forEach((b: any) => b.classList.remove("active"));
      btnEl.classList.add("active");

      if (grid) {
        if (mode === "horizontal") {
          grid.classList.remove("vertical");
          grid.classList.add("horizontal");
        } else {
          grid.classList.remove("horizontal");
          grid.classList.add("vertical");
        }
      }
    };
  }

  function addNotificationToast(toast: Omit<ToastNotification, "id" | "createdAt">) {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const newToast: ToastNotification = {
      id,
      ...toast,
      createdAt: Date.now(),
    };
    toastNotifications = [newToast, ...toastNotifications].slice(0, 5);
    setTimeout(() => {
      dismissNotificationToast(id);
    }, 8000);
  }

  function dismissNotificationToast(id: string) {
    toastNotifications = toastNotifications.filter((t) => t.id !== id);
  }

  function handleForgotPassword() {
    cloudAuthMode = "forgot-password";
    cloudAuthError = "";
    cloudAuthSuccessMessage = "";
    cloudAuthDevTokenUrl = "";
  }
  let collectionsSyncBusy = false;
  let collectionsSyncError = "";
  let agentRuns: AgentRun[] = [];
  let agentLanes: AgentLaneView[] = [];
  let orchestratorSnapshot: AgentOrchestratorSnapshot | null = null;
  let selectedAgentRunView: AgentRunView | null = null;
  let agentRunsBusy = false;
  let agentActionBusy: string | null = null;
  let stepsByMessageId: Record<string, AgentStep[]> = {};
  let runToMessageMap: Record<string, string> = {};
  let expandedMessageSteps: Record<string, boolean> = {};
  let expandedStepDetails: Record<string, boolean> = {};

  function toggleMessageSteps(msgId: string) {
    expandedMessageSteps[msgId] = !expandedMessageSteps[msgId];
    expandedMessageSteps = expandedMessageSteps;
  }

  function toggleStepDetails(msgId: string, seq: number) {
    const key = `${msgId}-${seq}`;
    expandedStepDetails[key] = !expandedStepDetails[key];
    expandedStepDetails = expandedStepDetails;
  }

  let cloudStorageBridgeInstalled = false;
  let hydratingCloudState = false;
  let cloudWriteLocked = false;
  let cloudStatusShortLabel = "";
  $: cloudWriteLocked = cloudAuthenticated && cloudSyncStatus.health !== "online";
  $: cloudStatusShortLabel = !cloudAuthenticated
    ? ""
    : cloudSyncStatus.health === "session-expired"
      ? (currentLanguage === "fr" ? "Session expiree" : "Session expired")
      : cloudSyncStatus.health === "offline-read-only"
        ? (currentLanguage === "fr" ? "Lecture seule" : "Read-only")
        : cloudSyncStatus.health === "sync-pending"
          ? (currentLanguage === "fr" ? "Sync en attente" : "Sync pending")
          : "";
  let activeMode: AssistantMode = "chat";
  let attachedFiles: AttachedFile[] = [];
  let input = "";
  let webAccess: WebAccessMode = "auto";

  // M4 (F4.3) — workspace file index for @ mentions + context injection.
  let workspaceMentionEntries: WorkspaceMentionEntry[] = [];

  async function loadWorkspaceMentions(conversationId: string | null = null) {
    try {
      const tree = await getWorkspaceTree(conversationId ?? activeConversation?.id ?? undefined);
      workspaceMentionEntries = flattenWorkspaceTreeToMentions(tree.entries ?? []);
    } catch (err) {
      console.warn("Workspace mention index refresh failed", err);
    }
  }

  const MAX_MENTION_CONTEXT_FILES = 5;
  const MAX_MENTION_FILE_CHARS = 12_000;
  const MAX_ATTACHMENT_CONTEXT_FILES = 5;
  const MAX_ATTACHMENT_FILE_CHARS = 12_000;
  const MAX_ATTACHMENT_READ_BYTES = 2_000_000;
  const TEXT_ATTACHMENT_MIMES = ["application/json", "application/x-yaml"];
  const TEXT_ATTACHMENT_EXTENSIONS = [
    "ts", "tsx", "js", "jsx", "svelte", "rs", "py", "json", "toml",
    "yaml", "yml", "css", "scss", "html", "sql", "sh", "md", "txt",
    "csv", "xml", "c", "cpp", "h", "hpp", "java", "go", "env",
  ];

  function isTextReadableAttachment(file: AttachedFile): boolean {
    if (!file || file.size <= 0 || file.size > MAX_ATTACHMENT_READ_BYTES) return false;
    const mime = (file.type || "").toLowerCase();
    if (mime.startsWith("text/") || TEXT_ATTACHMENT_MIMES.includes(mime)) return true;
    const ext = file.name.split(".").pop()?.toLowerCase() ?? "";
    return TEXT_ATTACHMENT_EXTENSIONS.includes(ext);
  }

  // Les pièces jointes n'étaient que des métadonnées : le modèle ne voyait
  // jamais leur contenu. On injecte ici le texte des fichiers lisibles
  // (locaux via File.text(), cloud via téléchargement), tronqué et plafonné.
  async function buildLocalAttachmentContext(files: AttachedFile[]): Promise<string> {
    const parts: string[] = [];
    for (const file of files.filter(isTextReadableAttachment).slice(0, MAX_ATTACHMENT_CONTEXT_FILES)) {
      try {
        let text = "";
        if (file.mode === "cloud-object" && file.fileId) {
          const blob = await downloadCloudFile(file.fileId);
          text = await blob.text();
        } else if (file.file) {
          text = await file.file.text();
        }
        if (!text.trim()) continue;
        const body =
          text.length > MAX_ATTACHMENT_FILE_CHARS
            ? `${text.slice(0, MAX_ATTACHMENT_FILE_CHARS)}\n…[tronqué]`
            : text;
        parts.push(`--- Pièce jointe : ${file.name} ---\n${body}`);
      } catch (err) {
        console.warn(`Attachment context read failed for ${file.name}`, err);
      }
    }
    return parts.join("\n\n");
  }

  async function buildMentionContext(content: string): Promise<{ contextBlock: string; referenced: WorkspaceMentionEntry[] }> {
    const wanted = dedupeMentionedPaths(extractMentionedPaths(content));
    if (wanted.length === 0) return { contextBlock: "", referenced: [] };
    const referenced = resolveMentionedEntries(content, workspaceMentionEntries).filter((e) => !e.isDir);
    if (referenced.length === 0) return { contextBlock: "", referenced: [] };
    const conversationId = activeConversation?.id ?? undefined;
    const settled = await Promise.all(
      referenced.slice(0, MAX_MENTION_CONTEXT_FILES).map(async (entry) => {
        try {
          const raw = await readWorkspaceFile(entry.relativePath, conversationId);
          const truncated = raw.length > MAX_MENTION_FILE_CHARS;
          const body = truncated ? `${raw.slice(0, MAX_MENTION_FILE_CHARS)}\n…[tronqué]` : raw;
          return `--- Fichier : ${entry.relativePath} ---\n${body}`;
        } catch (err) {
          console.warn(`Mention context read failed for ${entry.relativePath}`, err);
          return null;
        }
      }),
    );
    const parts = settled.filter((p): p is string => p !== null);
    if (parts.length === 0) return { contextBlock: "", referenced: [] };
    return {
      contextBlock: `Contexte du projet (références @) :\n${parts.join("\n\n")}\n\n--- Message ---\n${content}`,
      referenced,
    };
  }

  function toggleWebAccess() {
    webAccess = webAccess === "off" ? "auto" : webAccess === "auto" ? "on" : "off";
  }

  function webAccessTitle(mode = webAccess) {
    if (mode === "on") {
      return currentLanguage === "fr"
        ? "Web force: l'assistant peut chercher et lire des pages."
        : "Web on: assistant may search and read pages.";
    }
    if (mode === "off") {
      return currentLanguage === "fr"
        ? "Web coupe: aucune recherche ni lecture web."
        : "Web off: no search or page reading.";
    }
    return currentLanguage === "fr"
      ? "Web auto: utilise le web seulement si utile."
      : "Web auto: uses the web only when useful.";
  }

  function webAccessAriaLabel(mode = webAccess) {
    return currentLanguage === "fr"
      ? `Acces web ${mode}`
      : `Web access ${mode}`;
  }

  let searchQuery = "";
  $: filteredConversations = conversations.filter(c => 
    c.title.toLowerCase().includes(searchQuery.toLowerCase())
  );
  let loading = true;
  let showSplash = !isSpotlightMode;
  let splashProgress = 0;
  let sending = false;
  let anyConversationSending = false;
  let sendingByConversation: Record<string, boolean> = {};
  let inFlightMessagesByConversation: Record<string, ChatMessage[]> = {};
  let changingModel = false;
  let modelMenuOpen = false;
  let recording = false;
  let recordingStarting = false;
  let voiceInputMode: VoiceInputMode = "dictation";
  let voiceHandsFreeArmed = false;
  let voiceAutoListen = false;
  let voiceSessionConversationId: string | null = null;
  let activeVoiceSession: VoiceSession | null = null;
  let lastVoiceSession: VoiceSession | null = null;
  let assistantSpeaking = false;
  let voiceVolume = 0;
  let hasSpoken = false;
  let lastSpeakingTime = 0;
  let currentSpeakingAudio: HTMLAudioElement | null = null;
  let currentSpeakingAudioUrl: string | null = null;
  let showSettings = false;
  let activeSettingsTab: SettingsTab = "profile";
  let pendingSettingsTab: SettingsTab | null = null;
  let settingsSidebarWidth = 220;
  let settingsSidebarOpen = true;

  // Customizable Keyboard Shortcuts
  let shortcuts: Shortcuts = { ...DEFAULT_SHORTCUTS };

  let recordingShortcutFor: ShortcutKey | null = null;

  function loadShortcuts() {
    shortcuts = loadShortcutModel(typeof localStorage === "undefined" ? undefined : localStorage);
  }

  function updateShortcut(key: ShortcutKey, value: string) {
    shortcuts = updateShortcutModel(
      shortcuts,
      key,
      value,
      typeof localStorage === "undefined" ? undefined : localStorage,
    );
  }

  // Command Palette (Spotlight Mode)
  let showCommandPalette = false;
  let commandPaletteSearch = "";
  let commandPaletteSelectedIndex = 0;
  let commandPaletteInput: HTMLInputElement;

  function toggleCommandPalette() {
    showCommandPalette = !showCommandPalette;
    if (showCommandPalette) {
      commandPaletteSearch = "";
      commandPaletteSelectedIndex = 0;
      tick().then(() => {
        if (commandPaletteInput) commandPaletteInput.focus();
      });
    }
  }
  let lastShowSettings = false;

  const NEW_CONVERSATION_SEND_KEY = "__new_conversation__";

  function conversationSendKey(conversationId: string | null | undefined = undefined): string {
    return conversationId || NEW_CONVERSATION_SEND_KEY;
  }

  function setConversationSending(conversationId: string | null | undefined, value: boolean) {
    const key = conversationSendKey(conversationId);
    if (value) {
      sendingByConversation = { ...sendingByConversation, [key]: true };
      return;
    }
    const { [key]: _removed, ...rest } = sendingByConversation;
    sendingByConversation = rest;
  }

  function isConversationSending(conversationId: string | null | undefined = undefined, _trigger: any = undefined): boolean {
    return Boolean(sendingByConversation[conversationSendKey(conversationId)]);
  }

  function isStillViewingConversation(originalConversationId: string | null | undefined = undefined): boolean {
    return originalConversationId
      ? activeConversation?.id === originalConversationId
      : activeConversation === null;
  }

  function conversationTitleFromContent(content: string): string {
    const normalized = content.split(/\s+/).join(" ").trim();
    if (!normalized) return currentLanguage === "fr" ? "Nouvelle conversation" : "New conversation";
    return normalized.split(" ").slice(0, 8).join(" ");
  }

  function promoteConversation(conversation: Conversation) {
    conversations = [
      conversation,
      ...conversations.filter((item) => item.id !== conversation.id),
    ];
  }

  function selectPendingDestination(projectId: string | null, folderId: string | null) {
    pendingProjectId = projectId;
    pendingFolderId = folderId;
  }

  $: pendingDestinationLabel = (() => {
    if (!pendingProjectId && !pendingFolderId) return null;
    const folder = folders.find((f) => f.id === pendingFolderId);
    if (folder) {
      const parent = projects.find((p) => p.id === folder.projectId);
      return parent ? `${parent.name} / ${folder.name}` : folder.name;
    }
    return projects.find((p) => p.id === pendingProjectId)?.name ?? null;
  })();

  async function createAndActivateConversation(content: string, mode: AssistantMode): Promise<Conversation> {
    const projectId = pendingProjectId;
    const folderId = pendingFolderId;
    const conversation = await createConversation(conversationTitleFromContent(content), mode, projectId, folderId);
    // Destination consommée : on repart sur "Sans classement" pour le prochain chat.
    pendingProjectId = null;
    pendingFolderId = null;
    activeConversation = conversation;
    activeMode = conversation.mode;
    promoteConversation(conversation);
    messages = [];
    return conversation;
  }

  function createVoiceSession(): VoiceSession {
    const session: VoiceSession = {
      id: crypto.randomUUID(),
      conversationId: activeConversation?.id ?? null,
      mode: voiceInputMode,
      autoSubmit: voiceAutoListen || voiceInputMode === "hands-free",
      startedAt: Date.now(),
      state: "capturing",
      transcript: "",
    };
    activeVoiceSession = session;
    voiceSessionConversationId = session.conversationId;
    return session;
  }

  function updateVoiceSession(patch: Partial<VoiceSession>) {
    if (!activeVoiceSession) return;
    activeVoiceSession = { ...activeVoiceSession, ...patch };
  }

  function clearVoiceSession(state: VoiceSessionState) {
    if (activeVoiceSession) {
      lastVoiceSession = { ...activeVoiceSession, state };
    }
    activeVoiceSession = null;
    voiceSessionConversationId = null;
  }

  function setInFlightMessages(conversationId: string | null | undefined, nextMessages: ChatMessage[]) {
    if (!conversationId) return;
    inFlightMessagesByConversation = {
      ...inFlightMessagesByConversation,
      [conversationId]: nextMessages,
    };
  }

  function clearInFlightMessages(conversationId: string | null | undefined) {
    if (!conversationId) return;
    const { [conversationId]: _removed, ...rest } = inFlightMessagesByConversation;
    inFlightMessagesByConversation = rest;
  }

  function applyMessageChunk(
    messageList: ChatMessage[],
    messageId: string,
    content: string,
  ): { messages: ChatMessage[]; changed: boolean } {
    let changed = false;
    const nextMessages = messageList.map(msg => {
      if (msg.id === "temp-generating" || msg.id === messageId) {
        changed = true;
        return {
          ...msg,
          id: messageId,
          content: msg.content + content,
          isGenerating: false,
        };
      }
      return msg;
    });
    return { messages: nextMessages, changed };
  }

  $: sending = isConversationSending(activeConversation?.id ?? null, sendingByConversation);
  $: anyConversationSending = Object.values(sendingByConversation).some(Boolean);

  function openSettings(tab: SettingsTab = "profile") {
    pendingSettingsTab = tab;
    activeSettingsTab = tab;
    showSettings = true;
  }

  function leaveSettingsView() {
    showSettings = false;
    pendingSettingsTab = null;
    lastShowSettings = false;
  }

  // MCP Servers State
  let mcpServers: McpServer[] = [];
  let selectedMcpServerId: string | null = null;
  let showMcpModal = false;
  let mcpModalMode: "add" | "edit" = "add";
  let editingMcpServerId: string | null = null;
  let mcpFormName = DEFAULT_MCP_FORM.name;
  let mcpFormType: McpTransport = DEFAULT_MCP_FORM.type;
  let mcpFormCommand = DEFAULT_MCP_FORM.command;
  let mcpFormArgs = DEFAULT_MCP_FORM.args;
  let mcpFormUrl = DEFAULT_MCP_FORM.url;
  let mcpFormEnv: { key: string; value: string }[] = [...DEFAULT_MCP_FORM.env];

  // Hooks State
  let hooks: AroHook[] = [];
  let showHooksModal = false;
  let hooksModalMode: "add" | "edit" = "add";
  let editingHookId: string | null = null;

  let hookFormName = DEFAULT_HOOK_FORM.name;
  let hookFormUrl = DEFAULT_HOOK_FORM.url;
  let hookFormSecret = DEFAULT_HOOK_FORM.secret;
  let hookFormEvents: string[] = [...DEFAULT_HOOK_FORM.events];

  // Scheduler State
  let scheduledTasks: ScheduledTask[] = [];
  let showSchedulerModal = false;
  let schedulerModalMode: "add" | "edit" = "add";
  let editingTaskId: string | null = null;

  let schedulerFormName = DEFAULT_SCHEDULER_FORM.name;
  let schedulerFormPrompt = DEFAULT_SCHEDULER_FORM.prompt;
  let schedulerFormType: SchedulerType = DEFAULT_SCHEDULER_FORM.type;
  let schedulerFormDuration = DEFAULT_SCHEDULER_FORM.duration;
  let schedulerFormCron = DEFAULT_SCHEDULER_FORM.cron;

  // IA Permissions State
  let activePermissionPreset: PermissionPresetMode = (typeof localStorage !== "undefined" && localStorage.getItem("aro-composer-permission-preset") as PermissionPresetMode) || "standard";

  let permissionProfiles: PermissionProfile[] = [];
  let activePermissionProfileId = "";
  let permissionsLoading = false;
  let permissionsSaving = false;
  let permissionsStatus = "";
  let permissionsError = "";
  let permReadFile = activePermissionPreset === "sandbox" ? false : true;
  let permWriteFile = activePermissionPreset === "standard" || activePermissionPreset === "developer";
  let permExecuteCommands = activePermissionPreset === "developer";
  let permCommandApprovalMode: PermissionCommandApproval = activePermissionPreset === "developer" ? "never" : "always";
  let permNetworkAccess = activePermissionPreset !== "read-only" && activePermissionPreset !== "sandbox";
  let permRedactSecrets = true;
  let permAllowedDomains = ["github.com", "google.com", "npmjs.com", "crates.io"];
  let permAllowedPaths = ["C:/Users/Stagiaire/Documents/ARO"];
  
  let newDomainInput = "";
  let newPathInput = "";

  let showNetworkAlert = false;

  $: activePermissionLabel = (() => {
    if (activePermissionPreset === "standard") return "Standard";
    if (activePermissionPreset === "read-only") return currentLanguage === "fr" ? "Lecture seule" : "Read-only";
    if (activePermissionPreset === "developer") return currentLanguage === "fr" ? "Autonome" : "Developer";
    if (activePermissionPreset === "sandbox") return currentLanguage === "fr" ? "Isolé" : "Sandbox";
    const profile = permissionProfiles.find((p) => p.id === activePermissionProfileId);
    return profile?.name || (currentLanguage === "fr" ? "Personnalisé" : "Custom");
  })();

  async function handleSelectPermissionPreset(preset: "standard" | "read-only" | "developer" | "sandbox") {
    activePermissionPreset = preset;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-composer-permission-preset", preset);
    }
    if (preset === "standard") {
      permReadFile = true;
      permWriteFile = true;
      permExecuteCommands = false;
      permNetworkAccess = true;
      permCommandApprovalMode = "always";
    } else if (preset === "read-only") {
      permReadFile = true;
      permWriteFile = false;
      permExecuteCommands = false;
      permNetworkAccess = false;
      permCommandApprovalMode = "always";
    } else if (preset === "developer") {
      permReadFile = true;
      permWriteFile = true;
      permExecuteCommands = true;
      permNetworkAccess = true;
      permCommandApprovalMode = "never";
    } else if (preset === "sandbox") {
      permReadFile = false;
      permWriteFile = false;
      permExecuteCommands = false;
      permNetworkAccess = false;
      permCommandApprovalMode = "always";
    }
    await ensurePresetPermissionProfile(preset);
  }

  function handleSelectPermissionProfile(profileId: string) {
    selectPermissionProfile(profileId);
    activePermissionPreset = "custom";
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-composer-permission-preset", "custom");
    }
  }

  async function ensurePresetPermissionProfile(preset: "standard" | "read-only" | "developer" | "sandbox"): Promise<string | null> {
    const presetNames: Record<string, string> = {
      "standard": currentLanguage === "fr" ? "Profil Standard" : "Standard Profile",
      "read-only": currentLanguage === "fr" ? "Profil Lecture seule" : "Read-Only Profile",
      "developer": currentLanguage === "fr" ? "Profil Autonome (Dev)" : "Autonomous Profile (Dev)",
      "sandbox": currentLanguage === "fr" ? "Profil Isolé (Sandbox)" : "Sandbox Profile",
    };
    const targetName = presetNames[preset];
    const matched = permissionProfiles.find((p) => p.name === targetName);
    if (matched) {
      activePermissionProfileId = matched.id;
      return matched.id;
    }
    const draft: PermissionProfile = {
      id: globalThis.crypto?.randomUUID?.() ?? `permission-${Date.now()}`,
      name: targetName,
      trustedRoots: uniqueNonEmpty(permAllowedPaths),
      allowedDomains: uniqueNonEmpty(permAllowedDomains.map((d) => d.toLowerCase())),
      allowRead: permReadFile,
      allowWrite: permWriteFile,
      allowShell: permExecuteCommands,
      allowNetwork: permNetworkAccess,
      commandApproval: permCommandApprovalMode,
      redactSecrets: permRedactSecrets,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    };
    try {
      const saved = await upsertPermissionProfile(draft);
      permissionProfiles = [saved, ...permissionProfiles.filter((p) => p.id !== saved.id)];
      activePermissionProfileId = saved.id;
      return saved.id;
    } catch (e) {
      console.warn("Could not sync preset permission profile:", e);
      return null;
    }
  }

  $: {
    if (!permNetworkAccess && typeof window !== "undefined" && !localStorage.getItem("networkAlertDismissed")) {
      const hasNetworkError = messages.some((msg) => 
        msg.steps && msg.steps.some((step) => 
          step.status === "failed" && 
          step.error && 
          step.error.toLowerCase().includes("network access is disabled")
        )
      );
      if (hasNetworkError) {
        showNetworkAlert = true;
      }
    }
  }

  function uniqueNonEmpty(values: string[]) {
    return Array.from(new Set(values.map((value) => value.trim()).filter(Boolean)));
  }

  function makePermissionProfileDraft(): PermissionProfile {
    const now = new Date().toISOString();
    return {
      id: globalThis.crypto?.randomUUID?.() ?? `permission-${Date.now()}`,
      name: currentLanguage === "fr" ? "Profil ARO Desktop" : "ARO Desktop profile",
      trustedRoots: uniqueNonEmpty(permAllowedPaths),
      allowedDomains: uniqueNonEmpty(permAllowedDomains.map((domain) => domain.toLowerCase())),
      allowRead: permReadFile,
      allowWrite: permWriteFile,
      allowShell: permExecuteCommands,
      allowNetwork: permNetworkAccess,
      commandApproval: permCommandApprovalMode,
      redactSecrets: permRedactSecrets,
      createdAt: now,
      updatedAt: now,
    };
  }

  function applyPermissionProfile(profile: PermissionProfile) {
    activePermissionProfileId = profile.id;
    permReadFile = profile.allowRead;
    permWriteFile = profile.allowWrite;
    permExecuteCommands = profile.allowShell;
    permCommandApprovalMode = profile.commandApproval;
    permNetworkAccess = profile.allowNetwork;
    permRedactSecrets = profile.redactSecrets;
    permAllowedDomains = [...profile.allowedDomains];
    permAllowedPaths = [...profile.trustedRoots];
  }

  function selectedPermissionProfile() {
    return permissionProfiles.find((profile) => profile.id === activePermissionProfileId) ?? null;
  }

  function buildPermissionProfilePayload(): PermissionProfile {
    const now = new Date().toISOString();
    const existing = selectedPermissionProfile();
    return {
      ...(existing ?? makePermissionProfileDraft()),
      name: existing?.name || (currentLanguage === "fr" ? "Profil ARO Desktop" : "ARO Desktop profile"),
      trustedRoots: uniqueNonEmpty(permAllowedPaths),
      allowedDomains: uniqueNonEmpty(permAllowedDomains.map((domain) => domain.toLowerCase())),
      allowRead: permReadFile,
      allowWrite: permWriteFile,
      allowShell: permExecuteCommands,
      allowNetwork: permNetworkAccess,
      commandApproval: permCommandApprovalMode,
      redactSecrets: permRedactSecrets,
      updatedAt: now,
    };
  }

  async function refreshPermissionProfiles() {
    permissionsLoading = true;
    permissionsError = "";
    try {
      permissionProfiles = await listPermissionProfiles();
      const selected = selectedPermissionProfile() ?? permissionProfiles[0];
      if (selected) {
        applyPermissionProfile(selected);
      }
    } catch (error) {
      permissionsError = normalizeError(error);
    } finally {
      permissionsLoading = false;
    }
  }

  async function savePermissionProfile() {
    if (!ensureCloudWriteAllowed("modifier les permissions")) return;
    permissionsSaving = true;
    permissionsError = "";
    try {
      const saved = await upsertPermissionProfile(buildPermissionProfilePayload());
      permissionProfiles = [
        saved,
        ...permissionProfiles.filter((profile) => profile.id !== saved.id),
      ];
      applyPermissionProfile(saved);
      permissionsStatus = currentLanguage === "fr" ? "Autorisations synchronisées." : "Permissions synced.";
      setTimeout(() => {
        permissionsStatus = "";
      }, 2200);
    } catch (error) {
      permissionsError = normalizeError(error);
    } finally {
      permissionsSaving = false;
    }
  }

  function selectPermissionProfile(profileId: string) {
    const profile = permissionProfiles.find((item) => item.id === profileId);
    if (profile) applyPermissionProfile(profile);
  }

  async function addAllowedDomain() {
    if (!ensureCloudWriteAllowed("modifier les domaines autorises")) return;
    const domain = newDomainInput.trim().toLowerCase();
    if (domain && !permAllowedDomains.includes(domain)) {
      permAllowedDomains = [...permAllowedDomains, domain];
      newDomainInput = "";
      await savePermissionProfile();
    }
  }

  async function removeAllowedDomain(domain: string) {
    if (!ensureCloudWriteAllowed("modifier les domaines autorises")) return;
    permAllowedDomains = permAllowedDomains.filter(d => d !== domain);
    await savePermissionProfile();
  }

  async function addAllowedPath() {
    if (!ensureCloudWriteAllowed("modifier les chemins autorises")) return;
    const path = newPathInput.trim();
    if (path && !permAllowedPaths.includes(path)) {
      permAllowedPaths = [...permAllowedPaths, path];
      newPathInput = "";
      await savePermissionProfile();
    }
  }

  async function removeAllowedPath(path: string) {
    if (!ensureCloudWriteAllowed("modifier les chemins autorises")) return;
    permAllowedPaths = permAllowedPaths.filter(p => p !== path);
    await savePermissionProfile();
  }

  // Real-time system monitoring state
  const monitoringDefaults = createInitialMonitoringState();
  let monCpu = monitoringDefaults.cpu; // cpu percentage
  let monRam = monitoringDefaults.ram; // ram usage in GB
  let monRamMax = monitoringDefaults.ramMax; // max ram in GB
  let monGpu = monitoringDefaults.gpu; // gpu percentage
  let monGpuVram = monitoringDefaults.gpuVram; // vram in GB
  let monGpuVramMax = monitoringDefaults.gpuVramMax; // max vram in GB
  
  // Real-time sparkline history (last 20 values)
  let cpuHistory = monitoringDefaults.cpuHistory;
  let ramHistory = monitoringDefaults.ramHistory;
  let gpuHistory = monitoringDefaults.gpuHistory;
  
  // Model performance metrics
  let lastResponseTime = 0.0; // in seconds
  let avgResponseTime = 0.0;
  let lastTokenCount = 0;
  let lastTokenSpeed = 0.0; // tokens per second
  let totalTokensGenerated = 0;
  
  // Performance history log
  let performanceHistory: PerformanceRecord[] = [];
  
  // User activity tracker (GitHub-style calendar)
  let userActivityLog: Record<string, number> = {};

  function incrementActivity(dateStr: string) {
    userActivityLog = incrementActivityLog(userActivityLog, dateStr);
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-user-activity", JSON.stringify(userActivityLog));
    }
  }

  $: activityWeeks = buildActivityWeeks(userActivityLog, currentLanguage);

  // Profile & Org state
  let userProfile: UserProfile = createInitialUserProfile();
  let activeOrg: OrganizationInfo = createInitialOrganizationInfo();

  let currentOrganizationRole: "admin" | "member" = "member";
  $: currentOrganizationRole = currentOrganizationRoleForSession(cloudSession);

  let orgMembers: OrgMember[] = [];
  let orgTeams: OrgTeam[] = [];
  let apiKeys: ApiKeyRecord[] = [];

  // Helper variables for forms & filtering
  let memberSearchQuery = "";
  let showInviteModal = false;
  let showCreateTeamModal = false;

  // Form states for modals
  let inviteFormName = "";
  let inviteFormEmail = "";
  let inviteFormRole: "admin" | "manager" | "member" | "guest" = "member";

  let teamFormName = "";
  let teamFormDesc = "";
  let teamFormMembers: string[] = [];
  
  let apiKeyNameDraft = "";

  // Custom Personalities & Memory types & states
  interface Personality {
    id: string;
    cloudId?: string;
    name: string;
    description: string;
    prompt: string;
    icon: string;
    avatarColor: string;
    temperature: number;
    voiceId?: string | null;
    isDefault?: boolean;
  }

  let customInstructionsEnabled = true;
  let selectedPersonalityId = "default";
  let customPersonalities: Personality[] = [];
  let conversationPersonalities: Record<string, string> = {};
  let customAgentsList: any[] = [];
  let conversationCustomAgents: Record<string, string> = {};
  let showAgentModal = false;
  let editingAgentObj: any = null;
  
  let customSystemPromptsEnabled = false;
  let customSystemPrompts: Record<string, string> = buildDefaultSystemPrompts();

  const defaultAgiIdentity: Record<string, string> = { ...defaultInstructionIdentity };
  const defaultAgiRules: Record<string, string> = { ...defaultInstructionRules };
  const defaultAgiFormatting: Record<string, string> = { ...defaultInstructionFormatting };

  let agiIdentity = { ...defaultAgiIdentity };
  let agiRules = { ...defaultAgiRules };
  let agiFormatting = { ...defaultAgiFormatting };
  
  let selectedPromptMode = "chat";

  $: activeColor = 
    selectedPromptMode === 'chat' ? '#0071e3' :
    selectedPromptMode === 'think' ? '#bf5af2' :
    selectedPromptMode === 'code' ? '#30d158' :
    selectedPromptMode === 'summarize' ? '#ff9f0a' : '#64d2ff';

  $: activeColorLight = 
    selectedPromptMode === 'chat' ? 'rgba(0, 113, 227, 0.08)' :
    selectedPromptMode === 'think' ? 'rgba(191, 90, 242, 0.08)' :
    selectedPromptMode === 'code' ? 'rgba(48, 209, 88, 0.08)' :
    selectedPromptMode === 'summarize' ? 'rgba(255, 159, 10, 0.08)' : 'rgba(100, 210, 255, 0.08)';

  // ─── Arena Mode ───────────────────────────────────────────────────────────
  let arenaMode = false;
  let arenaModelA = "";
  let arenaModelB = "";
  let arenaSending = false;
  let modelAMenuOpen = false;
  let modelBMenuOpen = false;
  type ArenaTurn = { userContent: string; slotA: ArenaSlotState; slotB: ArenaSlotState };
  let arenaHistory: ArenaTurn[] = [];

  function defaultSlot(modelId: string, messageId: string | null = null): ArenaSlotState {
    return { modelId, content: "", done: false, ttft: null, tokensPerSec: 0, chunkCount: 0, startedAt: performance.now(), firstChunkAt: null, vote: null, messageId, error: null };
  }

  function findArenaSlot(chunk: ArenaStreamChunk): { turn: ArenaTurn; slot: "slotA" | "slotB" } | null {
    if (!arenaHistory.length) return null;
    const key = chunk.slot === "a" ? "slotA" : "slotB";
    // Route par messageId (robuste aux chunks tardifs), repli : dernier tour.
    if (chunk.messageId) {
      for (let i = arenaHistory.length - 1; i >= 0; i--) {
        const turn = arenaHistory[i];
        if (turn[key].messageId === chunk.messageId) return { turn, slot: key };
      }
    }
    const turn = arenaHistory[arenaHistory.length - 1];
    return { turn, slot: key };
  }

  function handleArenaChunk(chunk: ArenaStreamChunk) {
    const found = findArenaSlot(chunk);
    if (!found) return;
    const { turn, slot } = found;
    if (chunk.done) {
      const s = turn[slot];
      const elapsed = (performance.now() - s.startedAt) / 1000;
      turn[slot] = { ...s, done: true, tokensPerSec: elapsed > 0 ? Math.round(s.chunkCount / elapsed) : 0 };
    } else {
      const s = turn[slot];
      const now = performance.now();
      const ttft = s.firstChunkAt === null ? (now - s.startedAt) : s.ttft;
      turn[slot] = { ...s, content: s.content + chunk.content, chunkCount: s.chunkCount + 1, firstChunkAt: s.firstChunkAt ?? now, ttft };
    }
    arenaHistory = [...arenaHistory];

    // Scroll arena history to bottom
    setTimeout(() => {
      const historyEl = document.querySelector('.arena-history');
      if (historyEl) {
        historyEl.scrollTop = historyEl.scrollHeight;
      }
    }, 0);
  }

  async function submitArena() {
    const content = input.trim();
    if (!content || arenaSending || !arenaModelA || !arenaModelB) return;
    if (!ensureCloudWriteAllowed("envoyer un message en Arena")) return;
    const modelA = modelOptions.find((model) => model.id === arenaModelA);
    const modelB = modelOptions.find((model) => model.id === arenaModelB);
    if (!modelA || !modelB) return;
    if (arenaModelA === arenaModelB) {
      addNotificationToast({
        type: "info",
        title: currentLanguage === "fr" ? "Mêmes modèles" : "Same models",
        body: currentLanguage === "fr"
          ? "Choisissez deux modèles différents pour les comparer."
          : "Pick two different models to compare them.",
      });
      return;
    }
    arenaSending = true;
    input = "";
    await resizeComposer();
    const tempIdA = crypto.randomUUID();
    const tempIdB = crypto.randomUUID();
    const turn: ArenaTurn = {
      userContent: content,
      slotA: defaultSlot(arenaModelA, tempIdA),
      slotB: defaultSlot(arenaModelB, tempIdB),
    };
    arenaHistory = [...arenaHistory, turn];
    try {
      await sendArenaStream({
        content,
        mode: activeMode,
        systemPrompt: compileSystemPrompt(allPersonalities.find(p => p.id === selectedPersonalityId) || allPersonalities[0], activeMode, content),
        modelA,
        modelB,
        tempMessageIdA: tempIdA,
        tempMessageIdB: tempIdB,
      });
    } catch (error) {
      // Plus de spinner infini : les deux slots portent l'erreur.
      const message = normalizeError(error);
      arenaHistory = arenaHistory.map((t) =>
        t === turn
          ? {
              ...t,
              slotA: { ...t.slotA, done: true, error: message },
              slotB: { ...t.slotB, done: true, error: message },
            }
          : t,
      );
      addNotificationToast({
        type: "error",
        title: currentLanguage === "fr" ? "Échec Arena" : "Arena failed",
        body: message,
      });
    } finally {
      arenaSending = false;
    }
  }

  function castArenaVote(turnIdx: number, winner: 'a' | 'b' | 'tie') {
    arenaHistory = arenaHistory.map((t, i) => {
      if (i !== turnIdx) return t;
      return {
        ...t,
        slotA: { ...t.slotA, vote: winner === 'a' ? 'winner' : winner === 'tie' ? 'tie' : 'loser' },
        slotB: { ...t.slotB, vote: winner === 'b' ? 'winner' : winner === 'tie' ? 'tie' : 'loser' },
      };
    });
  }
  
  function resetSelectedPromptToDefault() {
    if (!ensureCloudWriteAllowed("modifier les directives systeme")) return;
    agiIdentity[selectedPromptMode] = defaultAgiIdentity[selectedPromptMode];
    agiRules[selectedPromptMode] = defaultAgiRules[selectedPromptMode];
    agiFormatting[selectedPromptMode] = defaultAgiFormatting[selectedPromptMode];
    saveAgiPrompts();
  }

  function applyAgiPreset(presetId: string) {
    if (!ensureCloudWriteAllowed("appliquer un preset de directives")) return;
    const preset = instructionPresets[presetId];
    if (preset) {
      agiIdentity[selectedPromptMode] = preset.identity;
      agiRules[selectedPromptMode] = preset.rules;
      agiFormatting[selectedPromptMode] = preset.formatting;
      
      // Update model temperature directly if settingsDraft exists!
      if (settingsDraft && settingsDraft.model) {
        settingsDraft.model.temperature = preset.temp;
        autosaveSettings();
      }
      saveAgiPrompts();
    }
  }

  // Form states for custom personalities
  let showPersonalityModal = false;
  let showTopbarPersonalityDropdown = false;
  let editingPersonalityId: string | null = null;
  let personalityFormName = "";
  let personalityFormDesc = "";
  let personalityFormPrompt = "";
  let personalityFormIcon = "bot";
  let personalityFormColor = "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)";
  let personalityFormTemperature = 0.7;
  let personalityFormVoiceId = "default";

  // Voice profiles states & variables
  let selectedVoiceId = "default-fr";
  let customVoices: VoiceProfile[] = [];

  // Form states for custom voices
  let showVoiceModal = false;
  let editingVoiceId: string | null = null;
  let voiceFormName = "";
  let voiceFormDesc = "";
  let voiceFormPath = "";
  let voiceFormSpeakerId: number | null = null;
  let voiceFormLanguage = "fr";
  let voiceFormColor = "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)";

  $: defaultVoices = [
    {
      id: "default-fr",
      name: currentLanguage === "fr" ? "Français - UPMC" : "French - UPMC",
      description: currentLanguage === "fr" ? "Voix française standard par défaut" : "Default standard French voice",
      path: "vendor/voice/models/piper/fr_FR-upmc-medium/fr_FR-upmc-medium.onnx",
      speakerId: null,
      language: "fr",
      avatarColor: "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)",
      isDefault: true
    },
    {
      id: "default-en",
      name: currentLanguage === "fr" ? "Anglais - Ryan" : "English - Ryan",
      description: currentLanguage === "fr" ? "Voix masculine américaine" : "American male voice",
      path: "vendor/voice/models/piper/en_US-ryan-medium/en_US-ryan-medium.onnx",
      speakerId: null,
      language: "en",
      avatarColor: "linear-gradient(135deg, #34A853 0%, #1A73E8 100%)",
      isDefault: true
    }
  ];

  $: allVoices = [...defaultVoices, ...customVoices];

  $: defaultPersonalities = defaultPersonalitiesForLanguage(currentLanguage) as Personality[];

  $: allPersonalities = [
    ...defaultPersonalities,
    ...customPersonalities
  ];
  $: promptPreviewPersonality = allPersonalities.find(p => p.id === selectedPersonalityId) || allPersonalities[0] || null;
  $: activePersonality = allPersonalities.find(p => p.id === selectedPersonalityId) || allPersonalities[0];
  $: compiledPromptPreview = promptPreviewPersonality
    ? compileSystemPrompt(promptPreviewPersonality, selectedPromptMode)
    : "";
  $: visibleInstructionPresets = [
    {
      ...instructionPresets.socrates,
      name: currentLanguage === "fr" ? "Raisonnement" : instructionPresets.socrates.name,
      sub: currentLanguage === "fr" ? "Clarifier les hypothèses" : instructionPresets.socrates.sub,
    },
    {
      ...instructionPresets.codex,
      name: currentLanguage === "fr" ? "Code" : instructionPresets.codex.name,
      sub: currentLanguage === "fr" ? "Ingénieur senior" : instructionPresets.codex.sub,
    },
    {
      ...instructionPresets.nova,
      name: currentLanguage === "fr" ? "Créatif" : instructionPresets.nova.name,
      sub: currentLanguage === "fr" ? "Explorer les options" : instructionPresets.nova.sub,
    },
    {
      ...instructionPresets.quiet,
      name: currentLanguage === "fr" ? "Bref" : instructionPresets.quiet.name,
      sub: currentLanguage === "fr" ? "Réponse minimale" : instructionPresets.quiet.sub,
    },
  ];

  $: {
    if (customSystemPromptsEnabled) {
      let changed = false;
      for (const m of ["chat", "think", "code", "summarize", "quiet"]) {
        const nextPrompt = composeInstructionPrompt(
          m,
          agiIdentity[m],
          agiRules[m],
          agiFormatting[m],
        );
        if (customSystemPrompts[m] !== nextPrompt) {
          customSystemPrompts[m] = nextPrompt;
          changed = true;
        }
      }
      if (changed) {
        customSystemPrompts = { ...customSystemPrompts };
      }
    }
  }

  let memoriesList: LongTermMemoryItem[] = [];
  let memoryIndex: MemoryIndexStatus | null = null;
  let memoryIndexBusy = false;

  let memorySearchQuery = "";
  const memoryEntryLimit = 500;
  let selectedMemoryFilter: "all" | "personal" | "technical" | "system" | "preference" = "all";
  let showAddMemoryInline = false;
  let newMemoryText = "";
  let newMemoryCategory: "personal" | "technical" | "system" | "preference" = "personal";

  let editingMemoryId: string | null = null;
  let editingMemoryText = "";
  let episodesList: EpisodeItem[] = [];
  let newMemorySalience = 0.7;
  let newMemoryPinned = false;

  // Skills structures
  let userSkillGroups: UserSkillGroup[] = createInitialSkillGroups();
  let userSkills: UserSkill[] = createInitialSkills();

  let skillSearchQuery = "";
  let showCreateSkillModal = false;
  let showCreateGroupModal = false;
  let editingSkillId: string | null = null;
  let selectedCategoryFilter = "Tous";
  let selectedGroupIdFilter = "all";

  // Skill Form States
  let skillFormName = "";
  let skillFormDesc = "";
  let skillFormIcon = "🧩";
  let skillFormCategory = "Général";
  let skillFormGroupId = "all";
  let skillFormTriggers = "";
  let skillFormType: "system_prompt" | "python" | "api" = "system_prompt";
  let skillFormContent = "";

  // Group Form States
  let groupFormName = "";
  let groupFormDesc = "";

  function saveSkillsToLocalStorage() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-user-skills", JSON.stringify(userSkills));
    }
    userSkills = [...userSkills];
  }

  function saveGroupsToLocalStorage() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-user-skill-groups", JSON.stringify(userSkillGroups));
    }
    userSkillGroups = [...userSkillGroups];
  }

  async function handleCreateOrUpdateSkill() {
    if (!ensureCloudWriteAllowed("modifier les skills")) return;
    const name = skillFormName.trim();
    const desc = skillFormDesc.trim();
    const content = skillFormContent.trim();
    if (!name || !content) return;

    if (editingSkillId) {
      const existingSkill = userSkills.find(s => s.id === editingSkillId);
      if (!existingSkill) return;
      let updatedSkill: UserSkill = {
        ...existingSkill,
        name,
        description: desc,
        icon: skillFormIcon,
        category: skillFormCategory,
        groupId: skillFormGroupId,
        triggers: skillFormTriggers,
        type: skillFormType,
        content,
      };
      userSkills = userSkills.map(s => s.id === editingSkillId ? updatedSkill : s);
      try {
        const saved = await upsertCloudItem("skills", updatedSkill, skillPayload(updatedSkill));
        userSkills = userSkills.map(s => s.id === saved.id ? saved : s);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    } else {
      let newSkill: UserSkill = {
        id: crypto.randomUUID(),
        name,
        description: desc,
        icon: skillFormIcon,
        category: skillFormCategory,
        groupId: skillFormGroupId,
        triggers: skillFormTriggers,
        type: skillFormType,
        content,
        enabled: true,
        createdAt: new Date().toISOString(),
      };
      try {
        newSkill = await upsertCloudItem("skills", newSkill, skillPayload(newSkill));
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
      userSkills = [newSkill, ...userSkills];
    }
    
    saveSkillsToLocalStorage();
    closeSkillForm();
  }

  async function deleteSkill(id: string) {
    if (!ensureCloudWriteAllowed("supprimer un skill")) return;
    if (!await confirmDanger("Supprimer ce skill ?", "Delete this skill?")) return;
    if (id.startsWith("plugin:")) {
      addNotificationToast({
        type: "info",
        title: "Skill de plugin",
        body: "Ce skill est fourni par un plugin. Désinstallez ou désactivez le plugin depuis l'onglet Plugins.",
      });
      return;
    }
    const skill = userSkills.find(s => s.id === id);
    userSkills = userSkills.filter(s => s.id !== id);
    if (skill) {
      try {
        await deleteCloudItem("skills", skill);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveSkillsToLocalStorage();
  }

  async function toggleSkill(id: string) {
    if (id.startsWith("plugin:")) {
      const parts = id.split(":");
      const pluginId = parts[1];
      const target = pluginSkills.find(s => s.id === id);
      const nextEnabled = target ? !target.enabled : false;
      try {
        await togglePlugin(pluginId, nextEnabled);
        await loadInstalledPluginsData();
      } catch (err) {
        console.warn("Failed to toggle plugin skill:", err);
      }
      return;
    }
    if (!ensureCloudWriteAllowed("modifier un skill")) return;
    const existingSkill = userSkills.find(s => s.id === id);
    if (!existingSkill) return;
    let updatedSkill: UserSkill = { ...existingSkill, enabled: !existingSkill.enabled };
    userSkills = userSkills.map(s => s.id === id ? updatedSkill : s);
    try {
      const saved = await upsertCloudItem("skills", updatedSkill, skillPayload(updatedSkill));
      userSkills = userSkills.map(s => s.id === saved.id ? saved : s);
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
    saveSkillsToLocalStorage();
  }

  function openSkillForm(skill: UserSkill | null = null) {
    if (skill) {
      editingSkillId = skill.id;
      skillFormName = skill.name;
      skillFormDesc = skill.description;
      skillFormIcon = skill.icon || "🧩";
      skillFormCategory = skill.category || "Général";
      skillFormGroupId = skill.groupId || "all";
      skillFormTriggers = skill.triggers || "";
      skillFormType = skill.type || "system_prompt";
      skillFormContent = skill.content;
    } else {
      editingSkillId = null;
      skillFormName = "";
      skillFormDesc = "";
      skillFormIcon = "🧩";
      skillFormCategory = "Général";
      skillFormGroupId = selectedGroupIdFilter === "all" ? "all" : selectedGroupIdFilter;
      skillFormTriggers = "";
      skillFormType = "system_prompt";
      skillFormContent = "";
    }
    showCreateSkillModal = true;
  }

  function closeSkillForm() {
    showCreateSkillModal = false;
    editingSkillId = null;
  }

  async function handleCreateGroup() {
    if (!ensureCloudWriteAllowed("modifier les groupes de skills")) return;
    const name = groupFormName.trim();
    const desc = groupFormDesc.trim();
    if (!name) return;

    let newGroup: UserSkillGroup = {
      id: "grp-" + crypto.randomUUID(),
      name,
      description: desc,
      createdAt: new Date().toISOString(),
    };
    try {
      newGroup = await upsertCloudItem("skill-groups", newGroup, skillGroupPayload(newGroup));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }

    userSkillGroups = [...userSkillGroups, newGroup];
    saveGroupsToLocalStorage();

    // Reset form & close
    groupFormName = "";
    groupFormDesc = "";
    showCreateGroupModal = false;
  }

  async function deleteGroup(id: string) {
    if (!ensureCloudWriteAllowed("supprimer un groupe de skills")) return;
    if (id === "all") return;
    if (!await confirmDanger("Supprimer ce groupe ?", "Delete this group?")) return;
    const group = userSkillGroups.find(g => g.id === id);
    
    // Remove group
    userSkillGroups = userSkillGroups.filter(g => g.id !== id);
    if (group) {
      try {
        await deleteCloudItem("skill-groups", group);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveGroupsToLocalStorage();

    // Re-associate skills belonging to this group to "all" (unassociated)
    userSkills = userSkills.map(s => s.groupId === id ? { ...s, groupId: "all" } : s);
    saveSkillsToLocalStorage();

    if (selectedGroupIdFilter === id) {
      selectedGroupIdFilter = "all";
    }
  }

  // Agent Plugins dynamic integration for Skills & MCP
  let installedAgentPlugins: any[] = [];
  let pluginSkills: UserSkill[] = [];
  let pluginMcpServers: McpServer[] = [];

  async function loadInstalledPluginsData() {
    try {
      const plugins = await listInstalledPlugins();
      installedAgentPlugins = plugins;
      pluginSkills = pluginSkillsToUserSkills(plugins);
      pluginMcpServers = pluginMcpServersToMcpServers(plugins);
    } catch (err) {
      console.warn("Failed to load installed plugins data in App:", err);
    }
  }

  $: if (activeSettingsTab === "skills" || activeSettingsTab === "mcp" || activeSettingsTab === "plugins") {
    loadInstalledPluginsData();
  }

  $: allSkills = [...userSkills, ...pluginSkills];

  $: filteredSkills = allSkills.filter(s => {
    const query = (skillSearchQuery || "").toLowerCase().trim();
    const matchSearch = !query ||
                        (s.name || "").toLowerCase().includes(query) || 
                        (s.description || "").toLowerCase().includes(query) ||
                        (s.pluginName || "").toLowerCase().includes(query) ||
                        ((s.tags || []).some(t => t.toLowerCase().includes(query)));
    const matchCat = selectedCategoryFilter === "Tous" || 
                     (selectedCategoryFilter === "Plugins" && s.isPlugin) ||
                     s.category === selectedCategoryFilter;
    const matchGroup = selectedGroupIdFilter === "all" || s.groupId === selectedGroupIdFilter || s.isPlugin;
    return matchSearch && matchCat && matchGroup;
  });

  $: uniqueCategories = [
    "Tous",
    ...(allSkills.some(s => s.isPlugin) ? ["Plugins"] : []),
    ...Array.from(new Set(allSkills.map(s => s.category || "Général"))).filter(c => c !== "Tous" && c !== "Plugins")
  ];

  $: allMcpServers = [...mcpServers, ...pluginMcpServers];

  // Plugins Structures
  let userPlugins: UserPlugin[] = createInitialPlugins();

  let showConfigurePluginModal = false;
  let showCustomPluginModal = false;
  let editingPluginId: string | null = null;
  let pluginFormFields: { [key: string]: string } = {};
  let configureModalStep: "consent" | "credentials" = "consent";

  let pluginSearchQuery = "";


  function mcpSafeForLocalStorage(server: McpServer): McpServer {
    // Environment variables can contain credentials under arbitrary names. Keep
    // no MCP environment values in the renderer's persistent storage.
    return { ...server, env: {} };
  }

  function hookSafeForLocalStorage(hook: AroHook): AroHook {
    return { ...hook, secret: undefined };
  }

  function savePluginsToLocalStorage() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(
        "aro-user-plugins",
        JSON.stringify(userPlugins.map(pluginSafeForLocalStorage)),
      );
    }
    userPlugins = [...userPlugins];
    void syncPluginsToCloud();
  }

  async function syncPluginsToCloud() {
    if (!canWriteCloudCollections()) return;
    try {
      const savedPlugins = await Promise.all(
        userPlugins.map((plugin) => {
          const safePlugin = pluginSafeForLocalStorage(plugin);
          return upsertCloudItem("plugins", safePlugin, pluginPayload(safePlugin, safePlugin.fields));
        }),
      );
      const cloudIdsById = new Map(savedPlugins.map((plugin) => [plugin.id, plugin.cloudId]));
      userPlugins = userPlugins.map((plugin) => ({
        ...plugin,
        cloudId: cloudIdsById.get(plugin.id) ?? plugin.cloudId,
        fields: pluginSafeForLocalStorage(plugin).fields,
      }));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  function togglePluginEnabled(id: string) {
    if (!ensureCloudWriteAllowed("modifier un plugin")) return;
    userPlugins = userPlugins.map(p => p.id === id ? { ...p, enabled: !p.enabled } : p);
    savePluginsToLocalStorage();
  }

  let oauthPollingInterval: any = null;
  let oauthConnecting = false;
  let oauthError = "";
  let oauthStatusMessage = "";

  function openConfigurePlugin(plugin: UserPlugin) {
    editingPluginId = plugin.id;
    pluginFormFields = { ...plugin.fields };
    configureModalStep = "consent";
    showConfigurePluginModal = true;
    oauthError = "";
    oauthConnecting = false;
    if (oauthPollingInterval) {
      clearInterval(oauthPollingInterval);
      oauthPollingInterval = null;
    }
  }

  async function handleOauthConnect(plugin: UserPlugin) {
    oauthConnecting = true;
    oauthError = "";
    oauthStatusMessage = "Initialisation de la connexion sécurisée...";
    
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      let providerId = plugin.id.replace("plg-", "");
      if (providerId === "google") {
        providerId = "google-workspace";
      }
      const startResult: any = await invoke("integration_connect_start", {
        providerId,
        scopes: []
      });
      
      const authUrl = startResult.authorizationUrl;
      const oauthState = startResult.state;
      
      oauthStatusMessage = "Ouverture du navigateur sécurisé...";
      await invoke("open_url", { url: authUrl });
      
      oauthStatusMessage = "En attente de confirmation dans votre navigateur...";
      
      const pollStart = Date.now();
      const pollTimeout = 5 * 60 * 1000;
      
      if (oauthPollingInterval) {
        clearInterval(oauthPollingInterval);
      }
      
      oauthPollingInterval = setInterval(async () => {
        if (Date.now() - pollStart > pollTimeout) {
          clearInterval(oauthPollingInterval);
          oauthPollingInterval = null;
          oauthConnecting = false;
          oauthError = "La connexion a expiré (délai de 5 minutes dépassé).";
          return;
        }
        
        try {
          const statusResult: any = await invoke("integration_connect_status", {
            oauthState
          });
          
          if (statusResult.status === "completed") {
            clearInterval(oauthPollingInterval);
            oauthPollingInterval = null;
            oauthStatusMessage = "Connexion réussie ! Configuration en cours...";
            
            userPlugins = userPlugins.map(p => {
              if (p.id === plugin.id) {
                return {
                  ...p,
                  status: "connected",
                  enabled: true,
                  profileName: "Compte Connecté",
                  profileDetails: "Accès activé via redirection OAuth"
                };
              }
              return p;
            });
            savePluginsToLocalStorage();
            
            setTimeout(() => {
              oauthConnecting = false;
              showConfigurePluginModal = false;
            }, 1500);
            
          } else if (statusResult.status === "failed") {
            clearInterval(oauthPollingInterval);
            oauthPollingInterval = null;
            oauthConnecting = false;
            oauthError = statusResult.error || "La connexion a échoué.";
          } else if (statusResult.status === "expired") {
            clearInterval(oauthPollingInterval);
            oauthPollingInterval = null;
            oauthConnecting = false;
            oauthError = "La session d'autorisation a expiré.";
          }
        } catch (pollErr) {
          console.error("Error polling oauth status:", pollErr);
        }
      }, 1500);
      
    } catch (err: any) {
      console.error("Failed to start OAuth connection:", err);
      oauthConnecting = false;
      oauthError = typeof err === "string" ? err : (err.message || "Une erreur est survenue lors de l'initialisation d'OAuth.");
    }
  }

  function toggleSubService(pluginId: string, serviceId: string) {
    if (!ensureCloudWriteAllowed("modifier un plugin")) return;
    userPlugins = userPlugins.map(p => {
      if (p.id === pluginId && p.subServices) {
        return {
          ...p,
          subServices: p.subServices.map(s => s.id === serviceId ? { ...s, enabled: !s.enabled } : s)
        };
      }
      return p;
    });
    savePluginsToLocalStorage();
  }

  async function savePluginConfiguration() {
    if (!ensureCloudWriteAllowed("configurer un plugin")) return;
    if (!editingPluginId) return;
    
    const plugin = userPlugins.find(p => p.id === editingPluginId);
    if (!plugin) return;

    const sensitiveEntries = Object.entries(pluginFormFields).filter(([key, value]) =>
      isSensitiveIntegrationField(key) && value.trim().length > 0,
    );
    if (sensitiveEntries.length > 0) {
      if (!Boolean(window.__TAURI_INTERNALS__)) {
        oauthError = "La configuration d'identifiants nécessite l'application desktop sécurisée.";
        return;
      }
      const publicConfig = Object.fromEntries(
        Object.entries(pluginFormFields).filter(([key]) => !isSensitiveIntegrationField(key)),
      );
      const secretPayload = Object.fromEntries(sensitiveEntries);
      let providerId = plugin.id.replace("plg-", "");
      if (providerId === "google") providerId = "google-workspace";
      oauthConnecting = true;
      oauthError = "";
      oauthStatusMessage = "Stockage sécurisé des identifiants...";
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const connection: any = await invoke("integration_api_key_connect", {
          providerId,
          secret: JSON.stringify(secretPayload),
          publicConfig,
          account: { displayName: plugin.name },
        });
        userPlugins = userPlugins.map((candidate) => candidate.id === plugin.id ? {
          ...candidate,
          fields: pluginSafeForLocalStorage(candidate).fields,
          status: "connected",
          enabled: true,
          profileName: connection?.account?.displayName ?? plugin.name,
          profileDetails: "Identifiants conservés dans le coffre backend",
        } : candidate);
        pluginFormFields = Object.fromEntries(
          Object.entries(pluginFormFields).map(([key, value]) => [
            key,
            isSensitiveIntegrationField(key) ? "" : value,
          ]),
        );
        savePluginsToLocalStorage();
        showConfigurePluginModal = false;
      } catch (error) {
        oauthError = normalizeError(error);
      } finally {
        oauthConnecting = false;
      }
      return;
    }

    if (editingPluginId !== "plg-apple") {
      oauthError = "Aucun identifiant sécurisé n'a été fourni. La configuration n'a pas été enregistrée.";
      return;
    }

    if (typeof window === "undefined" || !Boolean(window.__TAURI_INTERNALS__)) {
      oauthError = "L'intégration Apple nécessite l'application desktop sur macOS.";
      return;
    }

    oauthConnecting = true;
    oauthError = "";
    oauthStatusMessage = "Connexion aux données Apple locales...";
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const appleData = await invoke("get_apple_data") as any;
      if (appleData?.platform !== "macos") {
        throw new Error("L'intégration Apple locale est disponible uniquement sur macOS.");
      }

      const notesCount = Array.isArray(appleData.notes) ? appleData.notes.length : 0;
      const remindersCount = Array.isArray(appleData.reminders) ? appleData.reminders.length : 0;
      const calendarsCount = Array.isArray(appleData.calendars) ? appleData.calendars.length : 0;
      userPlugins = userPlugins.map((candidate) => candidate.id === plugin.id ? {
        ...candidate,
        fields: pluginSafeForLocalStorage(candidate).fields,
        status: "connected",
        enabled: true,
        profileName: "iCloud local",
        profileAvatar: "",
        profileDetails: `${notesCount} Notes, ${remindersCount} Rappels, ${calendarsCount} Agendas`,
      } : candidate);
      savePluginsToLocalStorage();
      showConfigurePluginModal = false;
      editingPluginId = null;
    } catch (error) {
      oauthError = normalizeError(error);
    } finally {
      oauthConnecting = false;
    }
  }

  async function deletePlugin(id: string) {
    if (!ensureCloudWriteAllowed("supprimer un plugin")) return;
    if (!await confirmDanger("Supprimer ce plugin ?", "Delete this plugin?")) return;
    // Only allow deleting custom plugins (not presets)
    if (id.startsWith("plg-github") || id.startsWith("plg-google") || id.startsWith("plg-microsoft") || id.startsWith("plg-apple") || id.startsWith("plg-meta") || id.startsWith("plg-slack") || id.startsWith("plg-spotify") || id.startsWith("plg-canva") || id.startsWith("plg-figma") || id.startsWith("plg-jira") || id.startsWith("plg-vercel") || id.startsWith("plg-linear") || id.startsWith("plg-gitlab") || id.startsWith("plg-discord") || id.startsWith("plg-trello") || id.startsWith("plg-zoom") || id.startsWith("plg-dropbox") || id.startsWith("plg-aws") || id.startsWith("plg-gcp") || id.startsWith("plg-kubernetes") || id.startsWith("plg-huggingface") || id.startsWith("plg-clickup") || id.startsWith("plg-salesforce") || id.startsWith("plg-hubspot") || id.startsWith("plg-asana") || id.startsWith("plg-twilio") || id.startsWith("plg-shopify") || id.startsWith("plg-stripe") || id.startsWith("plg-mailchimp") || id.startsWith("plg-intercom") || id.startsWith("plg-sentry") || id.startsWith("plg-datadog") || id.startsWith("plg-plaid") || id.startsWith("plg-paypal") || id.startsWith("plg-fitbit") || id.startsWith("plg-withings") || id.startsWith("plg-supabase") || id.startsWith("plg-coinbase") || id.startsWith("plg-strava") || id.startsWith("plg-openai") || id.startsWith("plg-anthropic") || id.startsWith("plg-sendgrid") || id.startsWith("plg-bitbucket") || id.startsWith("plg-pinterest") || id.startsWith("plg-reddit") || id.startsWith("plg-ghost") || id.startsWith("plg-twitch") || id.startsWith("plg-calendly") || id.startsWith("plg-digitalocean") || id.startsWith("plg-airtable") || id.startsWith("plg-monday") || id.startsWith("plg-heroku") || id.startsWith("plg-netlify") || id.startsWith("plg-pipedrive") || id.startsWith("plg-buffer") || id.startsWith("plg-miro") || id.startsWith("plg-gitbook") || id.startsWith("plg-mailgun") || id.startsWith("plg-plausible") || id.startsWith("plg-chrome") || id.startsWith("plg-notion")) return;
    const plugin = userPlugins.find(p => p.id === id);
    userPlugins = userPlugins.filter(p => p.id !== id);
    if (plugin) {
      try {
        await deleteCloudItem("plugins", plugin);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    savePluginsToLocalStorage();
  }

  let selectedPluginCategory = "Tous";


  $: pluginCategories = ["Tous", ...Array.from(new Set(userPlugins.map(p => p.category || "Autre").filter(Boolean)))];

  $: filteredPlugins = userPlugins.filter(p => {
    const matchesSearch = p.name.toLowerCase().includes(pluginSearchQuery.toLowerCase()) || 
                          p.description.toLowerCase().includes(pluginSearchQuery.toLowerCase());
    const matchesCategory = selectedPluginCategory === "Tous" || p.category === selectedPluginCategory;
    return matchesSearch && matchesCategory;
  });

  $: displayedCategories = selectedPluginCategory === "Tous" 
    ? getPluginsByCategory(filteredPlugins)
    : { [selectedPluginCategory]: filteredPlugins };

  $: editingPlugin = userPlugins.find(p => p.id === editingPluginId);

  function saveMcpServersToLocalStorage() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(
        "aro-mcp-servers",
        JSON.stringify(mcpServers.map(mcpSafeForLocalStorage)),
      );
    }
    mcpServers = [...mcpServers];
    void syncMcpServersToCloud();
  }

  async function syncMcpServersToCloud() {
    if (!canWriteCloudCollections()) return;
    try {
      const savedServers = await Promise.all(
        mcpServers.map((server) => {
          const safeServer = mcpSafeForLocalStorage(server);
          return upsertCloudItem("mcp", safeServer, mcpPayload(safeServer));
        }),
      );
      const cloudIdsById = new Map(savedServers.map((server) => [server.id, server.cloudId]));
      mcpServers = mcpServers.map((server) => ({
        ...server,
        cloudId: cloudIdsById.get(server.id) ?? server.cloudId,
        env: {},
      }));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  function saveHooksToLocalStorage() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(
        "aro-hooks",
        JSON.stringify(hooks.map(hookSafeForLocalStorage)),
      );
    }
    hooks = [...hooks];
    void syncHooksToCloud();
  }

  async function syncHooksToCloud() {
    if (!canWriteCloudCollections()) return;
    try {
      const savedHooks = await Promise.all(
        hooks.map((hook) => {
          const safeHook = hookSafeForLocalStorage(hook);
          return upsertCloudItem("hooks", safeHook, hookPayload(safeHook));
        }),
      );
      const cloudIdsById = new Map(savedHooks.map((hook) => [hook.id, hook.cloudId]));
      hooks = hooks.map((hook) => ({
        ...hook,
        cloudId: cloudIdsById.get(hook.id) ?? hook.cloudId,
        secret: undefined,
      }));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  function loadHooksFromLocalStorage() {
    if (typeof localStorage !== "undefined") {
      const saved = localStorage.getItem("aro-hooks");
      if (saved) {
        try {
          hooks = (JSON.parse(saved) as AroHook[]).map(hookSafeForLocalStorage);
          localStorage.setItem("aro-hooks", JSON.stringify(hooks));
        } catch (e) {
          console.error("Failed to parse hooks:", e);
        }
      } else {
        hooks = [];
      }
    }
  }

  async function toggleHook(id: string) {
    if (!ensureCloudWriteAllowed("modifier un hook")) return;
    hooks = hooks.map(h => h.id === id ? { ...h, enabled: !h.enabled } : h);
    saveHooksToLocalStorage();
  }

  async function deleteHook(id: string) {
    if (!ensureCloudWriteAllowed("supprimer un hook")) return;
    if (!await confirmDanger("Supprimer ce hook ?", "Delete this hook?")) return;
    const hook = hooks.find(h => h.id === id);
    hooks = hooks.filter(h => h.id !== id);
    if (hook) {
      try {
        await deleteCloudItem("hooks", hook);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveHooksToLocalStorage();
  }

  function openAddHookModal() {
    hooksModalMode = "add";
    hookFormName = DEFAULT_HOOK_FORM.name;
    hookFormUrl = DEFAULT_HOOK_FORM.url;
    hookFormSecret = DEFAULT_HOOK_FORM.secret;
    hookFormEvents = [...DEFAULT_HOOK_FORM.events];
    showHooksModal = true;
  }

  function openEditHookModal(hook: AroHook) {
    hooksModalMode = "edit";
    editingHookId = hook.id;
    hookFormName = hook.name;
    hookFormUrl = hook.url;
    hookFormSecret = hook.secret || "";
    hookFormEvents = [...hook.events];
    showHooksModal = true;
  }

  function toggleHookFormEvent(event: string) {
    if (hookFormEvents.includes(event)) {
      hookFormEvents = hookFormEvents.filter(e => e !== event);
    } else {
      hookFormEvents = [...hookFormEvents, event];
    }
  }

  function handleSaveHook() {
    if (!ensureCloudWriteAllowed("modifier les hooks")) return;
    if (!hookFormName.trim() || !hookFormUrl.trim()) return;

    if (hooksModalMode === "add") {
      const newHook: AroHook = {
        id: "hk-" + Math.random().toString(36).substring(2, 9),
        name: hookFormName.trim(),
        url: hookFormUrl.trim(),
        secret: hookFormSecret.trim() || undefined,
        events: hookFormEvents,
        enabled: true,
        createdAt: new Date().toISOString(),
        status: "idle"
      };
      hooks = [...hooks, newHook];
    } else if (hooksModalMode === "edit" && editingHookId) {
      hooks = hooks.map(h => h.id === editingHookId ? {
        ...h,
        name: hookFormName.trim(),
        url: hookFormUrl.trim(),
        secret: hookFormSecret.trim() || undefined,
        events: hookFormEvents
      } : h);
    }

    saveHooksToLocalStorage();
    showHooksModal = false;
  }

  function testHook(id: string) {
    if (!ensureCloudWriteAllowed("tester un hook")) return;
    hooks = hooks.map(h => h.id === id ? { ...h, status: "testing" } : h);
    
    setTimeout(() => {
      hooks = hooks.map(h => {
        if (h.id === id) {
          const isSuccess = h.url.startsWith("http");
          return {
            ...h,
            status: isSuccess ? "success" : "error",
            lastTriggered: new Date().toISOString()
          };
        }
        return h;
      });
      saveHooksToLocalStorage();
    }, 1500);
  }

  function saveTasksToLocalStorage() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-scheduled-tasks", JSON.stringify(scheduledTasks));
    }
    scheduledTasks = [...scheduledTasks];
    void syncTasksToCloud();
  }

  async function syncTasksToCloud() {
    if (!canWriteCloudCollections()) return;
    try {
      const savedTasks = await Promise.all(
        scheduledTasks.map((task) => upsertCloudItem("scheduled-tasks", task, scheduledTaskPayload(task))),
      );
      const cloudIdsById = new Map(savedTasks.map((task) => [task.id, task.cloudId]));
      scheduledTasks = scheduledTasks.map((task) => ({
        ...task,
        cloudId: cloudIdsById.get(task.id) ?? task.cloudId,
      }));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  function loadTasksFromLocalStorage() {
    if (typeof localStorage !== "undefined") {
      const saved = localStorage.getItem("aro-scheduled-tasks");
      if (saved) {
        try {
          scheduledTasks = JSON.parse(saved);
        } catch (e) {
          console.error("Failed to parse scheduled tasks:", e);
        }
      } else {
        scheduledTasks = [];
        if (false) scheduledTasks = [
          {
            id: "sc-1",
            name: "Rapport de matinée",
            prompt: "Fais un résumé des tâches de la journée et du statut des dépôts de l'équipe.",
            type: "cron",
            cronExpression: "0 9 * * 1-5",
            enabled: true,
            createdAt: new Date(Date.now() - 3 * 24 * 3600000).toISOString(),
            lastRun: new Date(Date.now() - 25 * 3600000).toISOString(),
            status: "completed"
          },
          {
            id: "sc-2",
            name: "Vérification des serveurs",
            prompt: "Vérifie l'état des serveurs MCP connectés et signale les anomalies.",
            type: "timer",
            durationMinutes: 60,
            enabled: false,
            createdAt: new Date(Date.now() - 8 * 24 * 3600000).toISOString(),
            status: "idle"
          }
        ];
        saveTasksToLocalStorage();
      }
    }
  }

  async function toggleTask(id: string) {
    if (!ensureCloudWriteAllowed("modifier une tache planifiee")) return;
    scheduledTasks = scheduledTasks.map(t => t.id === id ? { ...t, enabled: !t.enabled } : t);
    saveTasksToLocalStorage();
  }

  async function deleteTask(id: string) {
    if (!ensureCloudWriteAllowed("supprimer une tache planifiee")) return;
    if (!await confirmDanger("Supprimer cette tâche planifiée ?", "Delete this scheduled task?")) return;
    const task = scheduledTasks.find(t => t.id === id);
    scheduledTasks = scheduledTasks.filter(t => t.id !== id);
    if (task) {
      try {
        await deleteCloudItem("scheduled-tasks", task);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveTasksToLocalStorage();
  }

  function openAddSchedulerModal() {
    schedulerModalMode = "add";
    schedulerFormName = DEFAULT_SCHEDULER_FORM.name;
    schedulerFormPrompt = DEFAULT_SCHEDULER_FORM.prompt;
    schedulerFormType = DEFAULT_SCHEDULER_FORM.type;
    schedulerFormDuration = DEFAULT_SCHEDULER_FORM.duration;
    schedulerFormCron = DEFAULT_SCHEDULER_FORM.cron;
    showSchedulerModal = true;
  }

  function openEditSchedulerModal(task: ScheduledTask) {
    schedulerModalMode = "edit";
    editingTaskId = task.id;
    schedulerFormName = task.name;
    schedulerFormPrompt = task.prompt;
    schedulerFormType = task.type;
    schedulerFormDuration = task.durationMinutes || 15;
    schedulerFormCron = task.cronExpression || "*/30 * * * *";
    showSchedulerModal = true;
  }

  function handleSaveTask() {
    if (!ensureCloudWriteAllowed("modifier les taches planifiees")) return;
    if (!schedulerFormName.trim() || !schedulerFormPrompt.trim()) return;

    if (schedulerModalMode === "add") {
      const newTask: ScheduledTask = {
        id: "sc-" + Math.random().toString(36).substring(2, 9),
        name: schedulerFormName.trim(),
        prompt: schedulerFormPrompt.trim(),
        type: schedulerFormType,
        durationMinutes: schedulerFormType === "timer" ? schedulerFormDuration : undefined,
        cronExpression: schedulerFormType === "cron" ? schedulerFormCron.trim() : undefined,
        enabled: true,
        createdAt: new Date().toISOString(),
        status: "idle"
      };
      scheduledTasks = [...scheduledTasks, newTask];
    } else if (schedulerModalMode === "edit" && editingTaskId) {
      scheduledTasks = scheduledTasks.map(t => t.id === editingTaskId ? {
        ...t,
        name: schedulerFormName.trim(),
        prompt: schedulerFormPrompt.trim(),
        type: schedulerFormType,
        durationMinutes: schedulerFormType === "timer" ? schedulerFormDuration : undefined,
        cronExpression: schedulerFormType === "cron" ? schedulerFormCron.trim() : undefined
      } : t);
    }

    saveTasksToLocalStorage();
    showSchedulerModal = false;
  }

  function triggerTaskImmediately(id: string) {
    if (!ensureCloudWriteAllowed("lancer une tache planifiee")) return;
    scheduledTasks = scheduledTasks.map(t => t.id === id ? { ...t, status: "running" } : t);
    
    setTimeout(() => {
      scheduledTasks = scheduledTasks.map(t => {
        if (t.id === id) {
          return {
            ...t,
            status: "completed",
            lastRun: new Date().toISOString()
          };
        }
        return t;
      });
      saveTasksToLocalStorage();
    }, 2000);
  }

  function addMcpEnvVar() {
    mcpFormEnv = [...mcpFormEnv, { key: "", value: "" }];
  }

  function removeMcpEnvVar(index: number) {
    mcpFormEnv = mcpFormEnv.filter((_, i) => i !== index);
  }

  function openAddMcpModal() {
    mcpModalMode = "add";
    mcpFormName = DEFAULT_MCP_FORM.name;
    mcpFormType = DEFAULT_MCP_FORM.type;
    mcpFormCommand = DEFAULT_MCP_FORM.command;
    mcpFormArgs = DEFAULT_MCP_FORM.args;
    mcpFormUrl = DEFAULT_MCP_FORM.url;
    mcpFormEnv = [...DEFAULT_MCP_FORM.env];
    showMcpModal = true;
  }

  function openEditMcpModal(server: McpServer) {
    if (server.isPlugin) {
      addNotificationToast({
        type: "info",
        title: "Serveur MCP géré par plugin",
        body: `Ce serveur MCP provient du plugin '${server.pluginName || server.pluginId}'. Sa configuration est définie dans son manifest mcp.json.`,
      });
      return;
    }
    mcpModalMode = "edit";
    editingMcpServerId = server.id;
    mcpFormName = server.name;
    mcpFormType = server.type;
    mcpFormCommand = server.command || "";
    mcpFormArgs = (server.args || []).join(" ");
    mcpFormUrl = server.url || "";
    mcpFormEnv = Object.entries(server.env || {}).map(([key, value]) => ({ key, value }));
    showMcpModal = true;
  }

  function notifyFormError(body: string) {
    addNotificationToast({
      type: "error",
      title: currentLanguage === "fr" ? "Formulaire incomplet" : "Incomplete form",
      body,
    });
  }

  async function confirmDanger(titleFr: string, titleEn: string, bodyFr = "", bodyEn = ""): Promise<boolean> {
    return requestConfirm({
      title: currentLanguage === "fr" ? titleFr : titleEn,
      body: currentLanguage === "fr" ? bodyFr : bodyEn,
      confirmLabel: currentLanguage === "fr" ? "Supprimer" : "Delete",
      cancelLabel: currentLanguage === "fr" ? "Annuler" : "Cancel",
    });
  }

  function handleSaveMcpServer() {
    if (!ensureCloudWriteAllowed("modifier les serveurs MCP")) return;
    if (!mcpFormName.trim()) {
      notifyFormError("Veuillez saisir un nom pour le serveur MCP.");
      return;
    }
    if (mcpFormType === "stdio" && !mcpFormCommand.trim()) {
      notifyFormError("Veuillez saisir une commande d'exécution.");
      return;
    }
    if (mcpFormType === "sse" && !mcpFormUrl.trim()) {
      notifyFormError("Veuillez saisir l'URL du serveur SSE.");
      return;
    }

    const envMap: Record<string, string> = {};
    for (const item of mcpFormEnv) {
      if (item.key.trim()) {
        envMap[item.key.trim()] = item.value;
      }
    }

    const parsedArgs = mcpFormArgs.trim() ? mcpFormArgs.split(/\s+/) : [];

    if (mcpModalMode === "add") {
      const newServer: McpServer = {
        id: "mcp-" + Math.random().toString(36).substring(2, 9),
        name: mcpFormName.trim(),
        type: mcpFormType,
        command: mcpFormType === "stdio" ? mcpFormCommand.trim() : undefined,
        args: mcpFormType === "stdio" ? parsedArgs : undefined,
        env: mcpFormType === "stdio" ? envMap : undefined,
        url: mcpFormType === "sse" ? mcpFormUrl.trim() : undefined,
        enabled: true,
        status: "connecting",
        tools: [],
        resources: []
      };

      mcpServers = [...mcpServers, newServer];
      verifyMcpServerConnection(newServer.id);
    } else if (mcpModalMode === "edit" && editingMcpServerId) {
      mcpServers = mcpServers.map(s => s.id === editingMcpServerId ? {
        ...s,
        name: mcpFormName.trim(),
        type: mcpFormType,
        command: mcpFormType === "stdio" ? mcpFormCommand.trim() : undefined,
        args: mcpFormType === "stdio" ? parsedArgs : undefined,
        env: mcpFormType === "stdio" ? envMap : undefined,
        url: mcpFormType === "sse" ? mcpFormUrl.trim() : undefined,
        status: "connecting"
      } : s);
      verifyMcpServerConnection(editingMcpServerId);
    }

    saveMcpServersToLocalStorage();
    showMcpModal = false;
  }

  async function deleteMcpServer(id: string) {
    if (!ensureCloudWriteAllowed("supprimer un serveur MCP")) return;
    if (id.startsWith("plugin:")) {
      addNotificationToast({
        type: "info",
        title: "Serveur MCP de plugin",
        body: "Ce serveur MCP est fourni par un plugin. Vous pouvez le désactiver ou désinstaller le plugin depuis l'onglet Plugins.",
      });
      return;
    }
    if (await requestConfirm({
      title: "Supprimer ce serveur MCP ?",
      body: "Le serveur sera retiré de la liste. Cette action est immédiate.",
      confirmLabel: currentLanguage === "fr" ? "Supprimer" : "Delete",
      cancelLabel: currentLanguage === "fr" ? "Annuler" : "Cancel",
    })) {
      const server = mcpServers.find(s => s.id === id);
      mcpServers = mcpServers.filter(s => s.id !== id);
      if (selectedMcpServerId === id) {
        selectedMcpServerId = null;
      }
      if (server) {
        try {
          await deleteCloudItem("mcp", server);
        } catch (error) {
          collectionsSyncError = normalizeError(error);
        }
      }
      saveMcpServersToLocalStorage();
    }
  }

  function toggleMcpServer(id: string) {
    if (id.startsWith("plugin:")) {
      const parts = id.split(":");
      const pluginId = parts[1];
      const target = pluginMcpServers.find(s => s.id === id);
      const nextEnabled = target ? !target.enabled : false;
      void (async () => {
        try {
          await togglePlugin(pluginId, nextEnabled);
          await loadInstalledPluginsData();
        } catch (err) {
          console.warn("Failed to toggle plugin MCP server:", err);
        }
      })();
      return;
    }
    if (!ensureCloudWriteAllowed("modifier un serveur MCP")) return;
    mcpServers = mcpServers.map(s => {
      if (s.id === id) {
        const nextEnabled = !s.enabled;
        return {
          ...s,
          enabled: nextEnabled,
          status: nextEnabled ? "connecting" : "disconnected"
        };
      }
      return s;
    });
    saveMcpServersToLocalStorage();
    const updated = mcpServers.find(s => s.id === id);
    if (updated && updated.enabled) {
      verifyMcpServerConnection(id);
    }
  }

  function verifyMcpServerConnection(id: string) {
    if (!mcpServers.some((server) => server.id === id)) return;
    mcpServers = mcpServers.map((server) => server.id === id ? {
      ...server,
      status: "error",
      tools: [],
      resources: [],
      error: "L'exécution MCP sécurisée n'est pas encore activée sur le worker backend.",
    } : server);
    saveMcpServersToLocalStorage();
  }

  function addPresetMcpServer(preset: { name: string; type: "stdio" | "sse"; command?: string; args?: string[]; url?: string }) {
    const newServer: McpServer = {
      id: "mcp-" + Math.random().toString(36).substring(2, 9),
      name: preset.name,
      type: preset.type,
      command: preset.command,
      args: preset.args,
      url: preset.url,
      enabled: true,
      status: "connecting",
      tools: [],
      resources: []
    };

    mcpServers = [...mcpServers, newServer];
    saveMcpServersToLocalStorage();
    verifyMcpServerConnection(newServer.id);
  }

  function loadInstructionsAndMemory() {
    if (typeof localStorage !== "undefined") {
      const savedSysEnabled = localStorage.getItem("aro-custom-system-prompts-enabled");
      if (savedSysEnabled !== null) {
        customSystemPromptsEnabled = savedSysEnabled === "true";
      }

      const savedSysPrompts = localStorage.getItem("aro-custom-system-prompts");
      if (savedSysPrompts !== null) {
        try {
          customSystemPrompts = JSON.parse(savedSysPrompts);
        } catch (e) {
          console.error("Failed to parse custom system prompts", e);
        }
      }

      const savedAgiId = localStorage.getItem("aro-agi-identity");
      if (savedAgiId) {
        try { agiIdentity = JSON.parse(savedAgiId); } catch(e) {}
      }
      const savedAgiRules = localStorage.getItem("aro-agi-rules");
      if (savedAgiRules) {
        try { agiRules = JSON.parse(savedAgiRules); } catch(e) {}
      }
      const savedAgiFormatting = localStorage.getItem("aro-agi-formatting");
      if (savedAgiFormatting) {
        try { agiFormatting = JSON.parse(savedAgiFormatting); } catch(e) {}
      }

      const savedEnabled = localStorage.getItem("aro-custom-instructions-enabled");
      if (savedEnabled !== null) {
        customInstructionsEnabled = savedEnabled === "true";
      }

      const savedSelectedPers = localStorage.getItem("aro-selected-personality-id");
      if (savedSelectedPers !== null) {
        selectedPersonalityId = savedSelectedPers;
      }

      const savedCustomPers = localStorage.getItem("aro-custom-personalities");
      if (savedCustomPers !== null) {
        try {
          customPersonalities = JSON.parse(savedCustomPers);
        } catch (e) {
          console.error("Failed to parse custom personalities", e);
        }
      }

      const savedInstructionVersion = localStorage.getItem("aro-instruction-defaults-version");
      const migratedInstructions = migrateInstructionDefaults({
        storedVersion: savedInstructionVersion,
        customSystemPrompts,
        customPersonalities: customPersonalities as InstructionPersonality[],
      });
      customSystemPrompts = migratedInstructions.customSystemPrompts;
      customPersonalities = migratedInstructions.customPersonalities as Personality[];
      if (migratedInstructions.changed || savedInstructionVersion !== INSTRUCTION_DEFAULTS_VERSION) {
        localStorage.setItem("aro-instruction-defaults-version", migratedInstructions.version);
        localStorage.setItem("aro-custom-system-prompts", JSON.stringify(customSystemPrompts));
        localStorage.setItem("aro-custom-personalities", JSON.stringify(customPersonalities));
      }

      const savedConvPers = localStorage.getItem("aro-conversation-personalities");
      if (savedConvPers !== null) {
        try {
          conversationPersonalities = JSON.parse(savedConvPers);
        } catch (e) {
          console.error("Failed to parse conversation personalities", e);
        }
      }

      const savedConvCustomAgents = localStorage.getItem("aro-conversation-custom-agents");
      if (savedConvCustomAgents !== null) {
        try {
          conversationCustomAgents = JSON.parse(savedConvCustomAgents);
        } catch (e) {
          console.error("Failed to parse conversation custom agents", e);
        }
      }

      const savedCustomAgentsList = localStorage.getItem("aro-custom-agents-definitions");
      if (savedCustomAgentsList !== null) {
        try {
          customAgentsList = JSON.parse(savedCustomAgentsList);
        } catch (e) {
          console.error("Failed to parse custom agents list", e);
        }
      }

      const savedCustomVoices = localStorage.getItem("aro-custom-voices");
      if (savedCustomVoices !== null) {
        try {
          customVoices = JSON.parse(savedCustomVoices);
        } catch (e) {
          console.error("Failed to parse custom voices", e);
        }
      }

      const savedSelectedVoice = localStorage.getItem("aro-selected-voice-id");
      if (savedSelectedVoice !== null) {
        selectedVoiceId = savedSelectedVoice;
      }

      const savedWakeWord = localStorage.getItem("aro-wakeword-enabled");
      if (!cloudAuthenticated && savedWakeWord !== null) {
        wakeWordEnabled = savedWakeWord === "true";
      }

      const savedSkills = localStorage.getItem("aro-user-skills");
      if (savedSkills !== null) {
        try {
          userSkills = JSON.parse(savedSkills);
        } catch (e) {
          console.error("Failed to parse custom skills", e);
        }
      }

      const savedGroups = localStorage.getItem("aro-user-skill-groups");
      if (savedGroups !== null) {
        try {
          userSkillGroups = JSON.parse(savedGroups);
        } catch (e) {
          console.error("Failed to parse custom skill groups", e);
        }
      }

      const savedPlugins = localStorage.getItem("aro-user-plugins");
      if (savedPlugins !== null) {
        try {
          const parsed = (JSON.parse(savedPlugins) as UserPlugin[]).map(pluginSafeForLocalStorage);
          // One-time migration for legacy renderer storage. Existing credential
          // values are deliberately discarded rather than kept in a WebView.
          localStorage.setItem("aro-user-plugins", JSON.stringify(parsed));
          let merged = [...parsed];
          
          // Delete old plg-gdocs if present
          merged = merged.filter(p => p.id !== "plg-gdocs");
          
          // Merge presets
          for (const defaultPlugin of userPlugins) {
            const existingIndex = merged.findIndex(p => p.id === defaultPlugin.id);
            if (existingIndex === -1) {
              merged.push(defaultPlugin);
            } else {
              const existing = merged[existingIndex];
              // Keep enabled state and fields
              existing.configFields = defaultPlugin.configFields;
              if (defaultPlugin.subServices && !existing.subServices) {
                existing.subServices = defaultPlugin.subServices;
              }
              existing.description = defaultPlugin.description;
              existing.name = defaultPlugin.name;
              existing.icon = defaultPlugin.icon;
            }
          }
          userPlugins = merged;
        } catch (e) {
          console.error("Failed to parse user plugins", e);
        }
      }
      
      const savedMcpServers = localStorage.getItem("aro-mcp-servers");
      if (savedMcpServers !== null) {
        try {
          mcpServers = (JSON.parse(savedMcpServers) as McpServer[]).map(mcpSafeForLocalStorage);
          localStorage.setItem("aro-mcp-servers", JSON.stringify(mcpServers));
        } catch (e) {
          console.error("Failed to parse MCP servers", e);
        }
      } else {
        mcpServers = [];
        if (false) mcpServers = [
          {
            id: "mcp-filesystem",
            name: "Filesystem Local",
            type: "stdio",
            command: "npx",
            args: ["-y", "@modelcontextprotocol/server-filesystem", "C:/Users/Stagiaire/Documents"],
            env: {},
            enabled: true,
            status: "connected",
            tools: [
              { name: "read_file", description: "Lit le contenu complet d'un fichier spécifié.", inputSchema: { type: "object", properties: { path: { type: "string" } } } },
              { name: "write_file", description: "Crée ou écrase un fichier avec du contenu texte.", inputSchema: { type: "object", properties: { path: { type: "string" }, content: { type: "string" } } } },
              { name: "list_directory", description: "Liste le contenu d'un dossier spécifié en local.", inputSchema: { type: "object", properties: { path: { type: "string" } } } }
            ],
            resources: [
              { uri: "file:///C:/Users/Stagiaire/Documents", name: "Workspace Root" }
            ]
          },
          {
            id: "mcp-git",
            name: "Git Repository Controller",
            type: "stdio",
            command: "npx",
            args: ["-y", "@modelcontextprotocol/server-git"],
            env: {},
            enabled: false,
            status: "disconnected",
            tools: [
              { name: "git_status", description: "Renvoie l'état courant de l'arbre de travail Git." },
              { name: "git_log", description: "Affiche l'historique des commits du dépôt local." },
              { name: "git_diff", description: "Affiche les modifications non validées dans l'arbre." }
            ],
            resources: []
          }
        ];
        saveMcpServersToLocalStorage();
      }
    }
  }

  function saveCustomVoices() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-custom-voices", JSON.stringify(customVoices));
    }
    customVoices = [...customVoices];
    void syncVoicesToCloud();
  }

  function saveSelectedVoice() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-selected-voice-id", selectedVoiceId);
    }
  }

  function updateSettingsWithActiveVoice() {
    const allV = [...defaultVoices, ...customVoices];
    const activeVoice = allV.find(v => v.id === selectedVoiceId);
    if (activeVoice && settingsDraft) {
      settingsDraft.voice.piperVoicePath = activeVoice.path;
      autosaveSettings();
    }
  }

  function saveCustomPersonalities() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-custom-personalities", JSON.stringify(customPersonalities));
    }
    customPersonalities = [...customPersonalities];
    void syncPersonalitiesToCloud();
  }

  function saveSelectedPersonality() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-selected-personality-id", selectedPersonalityId);
    }
  }

  function saveConversationPersonalities() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-conversation-personalities", JSON.stringify(conversationPersonalities));
    }
    conversationPersonalities = { ...conversationPersonalities };
  }

  function saveInstructions() {
    if (!ensureCloudWriteAllowed("modifier les instructions")) return;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-custom-instructions-enabled", String(customInstructionsEnabled));
      localStorage.setItem("aro-selected-personality-id", selectedPersonalityId);
    }
  }

  function saveCustomSystemPrompts() {
    if (!ensureCloudWriteAllowed("modifier les directives systeme")) return;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-custom-system-prompts-enabled", String(customSystemPromptsEnabled));
      localStorage.setItem("aro-custom-system-prompts", JSON.stringify(customSystemPrompts));
      localStorage.setItem("aro-instruction-defaults-version", INSTRUCTION_DEFAULTS_VERSION);
    }
    void syncSystemPromptsToCloud();
  }

  function saveAgiPrompts() {
    if (!ensureCloudWriteAllowed("modifier les instructions ARO")) return;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-agi-identity", JSON.stringify(agiIdentity));
      localStorage.setItem("aro-agi-rules", JSON.stringify(agiRules));
      localStorage.setItem("aro-agi-formatting", JSON.stringify(agiFormatting));
      localStorage.setItem("aro-instruction-defaults-version", INSTRUCTION_DEFAULTS_VERSION);
    }
    void syncSystemPromptsToCloud();
  }

  async function syncPersonalitiesToCloud() {
    if (!canWriteCloudCollections()) return;
    try {
      const savedPersonalities = await Promise.all(
        customPersonalities.map((personality) =>
          upsertCloudItem("personalities", personality, personalityPayload(personality)),
        ),
      );
      const cloudIdsById = new Map(savedPersonalities.map((personality) => [personality.id, personality.cloudId]));
      customPersonalities = customPersonalities.map((personality) => ({
        ...personality,
        cloudId: cloudIdsById.get(personality.id) ?? personality.cloudId,
      }));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  async function syncVoicesToCloud() {
    if (!canWriteCloudCollections()) return;
    try {
      const savedVoices = await Promise.all(
        customVoices.map((voice) => upsertCloudItem("voice-profiles", voice, voicePayload(voice))),
      );
      const cloudIdsById = new Map(savedVoices.map((voice) => [voice.id, voice.cloudId]));
      customVoices = customVoices.map((voice) => ({
        ...voice,
        cloudId: cloudIdsById.get(voice.id) ?? voice.cloudId,
      }));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  async function syncSystemPromptsToCloud() {
    if (!canWriteCloudCollections()) return;
    try {
      await Promise.all(
        ["chat", "think", "code", "summarize", "quiet"].map((mode) =>
          createCloudCollectionItem("system-prompts", systemPromptPayload(mode)),
        ),
      );
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  function compileSystemPrompt(activePers: Personality | null, mode: string, userInput = ""): string {
    const basePrompt = compileAroSystemPrompt({
      mode: normalizeInstructionMode(mode),
      personality: activePers as InstructionPersonality | null,
      customSystemPromptsEnabled,
      customSystemPrompts,
      customInstructionsEnabled,
      memories: memoriesList as InstructionMemory[],
      userInput,
      language: currentLanguage,
      model: settings?.model.activeModelRef ?? null,
      runtimeDetail: runtime?.detail ?? null,
      skills: userSkills,
    });

    let permDirective = "";
    if (activePermissionPreset === "read-only") {
      permDirective = currentLanguage === "fr"
        ? "\n\n[POLITIQUE D'AUTONOMIE ET PERMISSIONS : LECTURE SEULE]\nL'utilisateur a configuré vos permissions en mode LECTURE SEULE STRICTE. Vous avez l'interdiction formelle de modifier, créer ou supprimer des fichiers, d'exécuter des commandes système ou d'appeler des outils d'écriture. Répondez uniquement par l'analyse, l'explication et la consultation."
        : "\n\n[AUTONOMY & PERMISSION POLICY: STRICT READ-ONLY]\nThe user has set your permissions to STRICT READ-ONLY. You are strictly forbidden from modifying, creating, or deleting files, running terminal commands, or invoking mutating tools. Operate solely by analyzing, reviewing, and answering.";
    } else if (activePermissionPreset === "developer") {
      permDirective = currentLanguage === "fr"
        ? "\n\n[POLITIQUE D'AUTONOMIE ET PERMISSIONS : AUTONOME / DÉVELOPPEUR]\nL'utilisateur vous a accordé les pleines permissions autonomes. Vous êtes autorisé à lire, écrire, corriger les fichiers du projet, installer des paquets et exécuter les commandes de terminal nécessaires de façon autonome."
        : "\n\n[AUTONOMY & PERMISSION POLICY: FULL AUTONOMY / DEVELOPER]\nThe user has granted you full autonomous permissions. You are authorized to read, write, and modify project files, install dependencies, and execute terminal commands autonomously to accomplish the requested task.";
    } else if (activePermissionPreset === "sandbox") {
      permDirective = currentLanguage === "fr"
        ? "\n\n[POLITIQUE D'AUTONOMIE ET PERMISSIONS : ISOLÉ (SANDBOX)]\nMode Sandbox actif. Vous n'avez aucun accès aux fichiers locaux, aucun accès au shell, et aucun accès réseau extérieur. Travaillez exclusivement avec le contexte textuel fourni dans la conversation."
        : "\n\n[AUTONOMY & PERMISSION POLICY: SANDBOX / ISOLATED]\nSandbox mode active. You have NO access to local files, NO shell execution, and NO external network access. Operate strictly within the provided conversational context.";
    } else if (activePermissionPreset === "standard") {
      permDirective = currentLanguage === "fr"
        ? "\n\n[POLITIQUE D'AUTONOMIE ET PERMISSIONS : STANDARD]\nMode standard équilibré. La lecture et l'écriture de fichiers du projet sont autorisées. L'exécution de commandes système ou d'actions destructives nécessite l'accord de l'utilisateur."
        : "\n\n[AUTONOMY & PERMISSION POLICY: STANDARD]\nBalanced standard mode. Reading and editing project files are permitted. System command execution and destructive actions require explicit user confirmation.";
    }

    return basePrompt + permDirective;
  }

  function saveMemories() {
    memoriesList = [...memoriesList];
  }

  async function refreshMemoryIndexStatus() {
    try {
      memoryIndex = await memoryIndexStatus();
    } catch (error) {
      memoryIndex = {
        mode: "auto",
        state: "degraded",
        qdrantUrl: "",
        collectionName: null,
        embeddingProvider: "unknown",
        embeddingModel: "unknown",
        dimension: null,
        indexedCount: null,
        message: normalizeError(error),
      };
    }
    void refreshEpisodes();
  }

  async function refreshEpisodes() {
    try {
      episodesList = await listEpisodes(activeConversation?.id, 50);
    } catch (error) {
      console.warn("Failed to load episodes:", error);
    }
  }

  async function reindexMemories() {
    memoryIndexBusy = true;
    collectionsSyncError = "";
    try {
      const report = await memoryIndexReindex();
      memoryIndex = report.status;
    } catch (error) {
      collectionsSyncError = normalizeError(error);
      await refreshMemoryIndexStatus();
    } finally {
      memoryIndexBusy = false;
    }
  }

  async function addMemory() {
    const content = newMemoryText.trim();
    if (!content) return;
    const id = crypto.randomUUID();
    const now = new Date().toISOString();
    let memory: LongTermMemoryItem = {
      id,
      clientId: id,
      content,
      category: newMemoryCategory,
      scope: "user",
      status: "approved",
      sourceConversationId: activeConversation?.id ?? null,
      sourceMessageIds: [],
      pinned: newMemoryPinned,
      salience: newMemorySalience,
      recallCount: 0,
      lastUsedAt: null,
      createdAt: now,
      updatedAt: now,
    };
    try {
      memory = await upsertMemoryItem(memory);
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
    memoriesList = [memory, ...memoriesList];
    saveMemories();
    await refreshMemoryIndexStatus();
    newMemoryText = "";
    showAddMemoryInline = false;
  }

  async function updateMemory(memory: LongTermMemoryItem) {
    const content = editingMemoryText.trim();
    if (!content) return;
    const updated = { ...memory, content, updatedAt: new Date().toISOString() };
    try {
      const saved = await upsertMemoryItem(updated);
      memoriesList = memoriesList.map((item) => (item.id === memory.id ? saved : item));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
      memoriesList = memoriesList.map((item) => (item.id === memory.id ? updated : item));
    }
    saveMemories();
    await refreshMemoryIndexStatus();
    editingMemoryId = null;
  }

  async function deleteMemory(memory: LongTermMemoryItem) {
    if (!await confirmDanger("Supprimer cette mémoire ?", "Delete this memory?")) return;
    memoriesList = memoriesList.filter((item) => item.id !== memory.id);
    try {
      await deleteMemoryItem(memory.id);
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
    saveMemories();
    await refreshMemoryIndexStatus();
  }

  async function togglePinMemory(memory: LongTermMemoryItem) {
    const updated = { ...memory, pinned: !memory.pinned, updatedAt: new Date().toISOString() };
    try {
      const saved = await upsertMemoryItem(updated);
      memoriesList = memoriesList.map((item) => (item.id === memory.id ? saved : item));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
      memoriesList = memoriesList.map((item) => (item.id === memory.id ? updated : item));
    }
    saveMemories();
    await refreshMemoryIndexStatus();
  }



  // CRUD handlers for custom personalities
  function savePersonality() {
    if (!ensureCloudWriteAllowed("modifier les personnalites")) return;
    if (!personalityFormName.trim() || !personalityFormPrompt.trim()) return;

    if (editingPersonalityId) {
      customPersonalities = customPersonalities.map(p => {
        if (p.id === editingPersonalityId) {
          return {
            ...p,
            name: personalityFormName.trim(),
            description: personalityFormDesc.trim(),
            prompt: personalityFormPrompt.trim(),
            icon: personalityFormIcon,
            avatarColor: personalityFormColor,
            temperature: personalityFormTemperature,
            voiceId: personalityFormVoiceId
          };
        }
        return p;
      });
    } else {
      const newPers: Personality = {
        id: Math.random().toString(36).substring(2, 11),
        name: personalityFormName.trim(),
        description: personalityFormDesc.trim(),
        prompt: personalityFormPrompt.trim(),
        icon: personalityFormIcon,
        avatarColor: personalityFormColor,
        temperature: personalityFormTemperature,
        voiceId: personalityFormVoiceId
      };
      customPersonalities = [...customPersonalities, newPers];
    }
    
    saveCustomPersonalities();
    showPersonalityModal = false;
    resetPersonalityForm();
  }

  function resetPersonalityForm() {
    editingPersonalityId = null;
    personalityFormName = "";
    personalityFormDesc = "";
    personalityFormPrompt = "";
    personalityFormIcon = "bot";
    personalityFormColor = "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)";
    personalityFormTemperature = 0.7;
    personalityFormVoiceId = "default";
  }

  // Voice CRUD handlers
  function saveVoice() {
    if (!ensureCloudWriteAllowed("modifier les voix")) return;
    if (!voiceFormName.trim() || !voiceFormPath.trim()) return;

    const speakerVal = voiceFormSpeakerId !== null && voiceFormSpeakerId !== undefined && (voiceFormSpeakerId as any) !== "" ? Number(voiceFormSpeakerId) : null;

    if (editingVoiceId) {
      customVoices = customVoices.map(v => {
        if (v.id === editingVoiceId) {
          return {
            ...v,
            name: voiceFormName.trim(),
            description: voiceFormDesc.trim(),
            path: voiceFormPath.trim(),
            speakerId: speakerVal,
            language: voiceFormLanguage,
            avatarColor: voiceFormColor
          };
        }
        return v;
      });
    } else {
      const newVoice: VoiceProfile = {
        id: Math.random().toString(36).substring(2, 11),
        name: voiceFormName.trim(),
        description: voiceFormDesc.trim(),
        path: voiceFormPath.trim(),
        speakerId: speakerVal,
        language: voiceFormLanguage,
        avatarColor: voiceFormColor
      };
      customVoices = [...customVoices, newVoice];
    }
    
    saveCustomVoices();
    showVoiceModal = false;
    resetVoiceForm();

    updateSettingsWithActiveVoice();
  }

  function resetVoiceForm() {
    editingVoiceId = null;
    voiceFormName = "";
    voiceFormDesc = "";
    voiceFormPath = "";
    voiceFormSpeakerId = null;
    voiceFormLanguage = "fr";
    voiceFormColor = "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)";
  }

  function openCreateVoice() {
    if (!ensureCloudWriteAllowed("creer une voix")) return;
    resetVoiceForm();
    showVoiceModal = true;
  }

  function openEditVoice(voice: VoiceProfile) {
    if (!ensureCloudWriteAllowed("modifier une voix")) return;
    editingVoiceId = voice.id;
    voiceFormName = voice.name;
    voiceFormDesc = voice.description;
    voiceFormPath = voice.path;
    voiceFormSpeakerId = voice.speakerId !== undefined ? voice.speakerId : null;
    voiceFormLanguage = voice.language;
    voiceFormColor = voice.avatarColor;
    showVoiceModal = true;
  }

  async function deleteVoice(id: string) {
    if (!ensureCloudWriteAllowed("supprimer une voix")) return;
    if (!await confirmDanger("Supprimer cette voix ?", "Delete this voice?")) return;
    const voice = customVoices.find(v => v.id === id);
    customVoices = customVoices.filter(v => v.id !== id);
    if (voice) {
      try {
        await deleteCloudItem("voice-profiles", voice);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveCustomVoices();
    
    if (selectedVoiceId === id) {
      selectedVoiceId = "default-fr";
      saveSelectedVoice();
      updateSettingsWithActiveVoice();
    }
  }

  function selectVoice(id: string) {
    if (!ensureCloudWriteAllowed("changer la voix active")) return;
    selectedVoiceId = id;
    saveSelectedVoice();
    updateSettingsWithActiveVoice();
  }

  function openCreatePersonality() {
    if (!ensureCloudWriteAllowed("creer une personnalite")) return;
    resetPersonalityForm();
    showPersonalityModal = true;
  }

  function openEditPersonality(pers: Personality) {
    if (!ensureCloudWriteAllowed("modifier une personnalite")) return;
    editingPersonalityId = pers.id;
    personalityFormName = pers.name;
    personalityFormDesc = pers.description;
    personalityFormPrompt = pers.prompt;
    personalityFormIcon = pers.icon;
    personalityFormColor = pers.avatarColor;
    personalityFormTemperature = pers.temperature;
    personalityFormVoiceId = pers.voiceId || "default";
    showPersonalityModal = true;
  }

  async function deletePersonality(id: string) {
    if (!ensureCloudWriteAllowed("supprimer une personnalite")) return;
    if (!await confirmDanger("Supprimer cette personnalité ?", "Delete this personality?")) return;
    const personality = customPersonalities.find(p => p.id === id);
    customPersonalities = customPersonalities.filter(p => p.id !== id);
    if (personality) {
      try {
        await deleteCloudItem("personalities", personality);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveCustomPersonalities();
    
    if (selectedPersonalityId === id) {
      selectedPersonalityId = "default";
      saveSelectedPersonality();
    }
    
    for (const key in conversationPersonalities) {
      if (conversationPersonalities[key] === id) {
        delete conversationPersonalities[key];
      }
    }
    saveConversationPersonalities();
  }

  function openCreateAgentModal() {
    editingAgentObj = null;
    showAgentModal = true;
  }

  function openEditAgentModal(agent: any) {
    editingAgentObj = { ...agent };
    showAgentModal = true;
  }

  async function saveCustomAgentFromPanel(agent: any) {
    if (!agent.name.trim()) return;
    if (!ensureCloudWriteAllowed("enregistrer un agent")) return;
    
    if (agent.id) {
      const matched = customAgentsList.find(a => a.id === agent.id);
      if (matched) {
        const updated = {
          ...matched,
          name: agent.name.trim(),
          description: agent.description.trim(),
          systemPrompt: agent.systemPrompt || agent.prompt.trim(),
          modelProviderId: agent.modelProviderId || "",
          modelId: agent.modelId || "",
          permissionProfileId: agent.permissionProfileId || "",
          commandApproval: agent.commandApproval || "",
          icon: agent.icon || "bot",
          enabledTools: Array.isArray(agent.enabledTools) ? agent.enabledTools : ["web.search", "web.fetch", "agent.delegate"]
        };
        try {
          await updateCloudCollectionItem("agent-definitions", updated.cloudId || updated.id, {
            name: updated.name,
            description: updated.description,
            systemPrompt: updated.systemPrompt,
            modelProviderId: updated.modelProviderId,
            modelId: updated.modelId,
            permissionProfileId: updated.permissionProfileId,
            commandApproval: updated.commandApproval,
            icon: updated.icon,
            enabledTools: updated.enabledTools
          });
          customAgentsList = customAgentsList.map(a => a.id === agent.id ? updated : a);
        } catch (e) {
          console.error("Failed to update cloud agent:", e);
        }
      }
    } else {
      const newAgent = {
        id: Math.random().toString(36).substring(2, 11),
        name: agent.name.trim(),
        description: agent.description.trim(),
        systemPrompt: agent.systemPrompt || agent.prompt.trim(),
        modelProviderId: agent.modelProviderId || "",
        modelId: agent.modelId || "",
        permissionProfileId: agent.permissionProfileId || "",
        commandApproval: agent.commandApproval || "",
        icon: agent.icon || "bot",
        enabledTools: Array.isArray(agent.enabledTools) ? agent.enabledTools : ["web.search", "web.fetch", "agent.delegate"]
      };
      try {
        const created = await createCloudCollectionItem("agent-definitions", {
          name: newAgent.name,
          description: newAgent.description,
          systemPrompt: newAgent.systemPrompt,
          modelProviderId: newAgent.modelProviderId,
          modelId: newAgent.modelId,
          permissionProfileId: newAgent.permissionProfileId,
          commandApproval: newAgent.commandApproval,
          icon: newAgent.icon,
          enabledTools: newAgent.enabledTools
        });
        const mapped = {
          ...newAgent,
          cloudId: String((created as any)?.id ?? "")
        };
        customAgentsList = [...customAgentsList, mapped];
      } catch (e) {
        console.error("Failed to create cloud agent:", e);
        customAgentsList = [...customAgentsList, newAgent];
      }
    }
    localStorage.setItem("aro-custom-agents-definitions", JSON.stringify(customAgentsList));
  }

  async function deleteCustomAgentFromPanel(id: string) {
    if (!ensureCloudWriteAllowed("supprimer un agent")) return;
    const agent = customAgentsList.find(a => a.id === id);
    if (agent) {
      try {
        if (agent.cloudId) {
          await deleteCloudCollectionItem("agent-definitions", agent.cloudId);
        }
      } catch (e) {
        console.error("Failed to delete cloud agent:", e);
      }
      customAgentsList = customAgentsList.filter(a => a.id !== id);
      localStorage.setItem("aro-custom-agents-definitions", JSON.stringify(customAgentsList));
    }
  }

  async function startAgentChatFromPanel(agentId: string) {
    try {
      const matchedAgent = customAgentsList.find(a => a.id === agentId);
      if (!matchedAgent) return;
      const title = matchedAgent.name;
      const conversation = await createAndActivateConversation(title, activeMode);
      conversationCustomAgents = { ...conversationCustomAgents, [conversation.id]: agentId };
      localStorage.setItem("aro-conversation-custom-agents", JSON.stringify(conversationCustomAgents));
      showRightPanel = false;
    } catch (error) {
      errorMessage = normalizeError(error);
    }
  }

  function selectPersonality(id: string) {
    if (!ensureCloudWriteAllowed("changer la personnalite active")) return;
    selectedPersonalityId = id;
    saveSelectedPersonality();
  }

  function selectConversationPersonality(conversationId: string, personalityId: string) {
    if (!ensureCloudWriteAllowed("changer la personnalite de la conversation")) return;
    conversationPersonalities = { ...conversationPersonalities, [conversationId]: personalityId };
    saveConversationPersonalities();
  }

  function selectConversationCustomAgent(conversationId: string, agentId: string | null) {
    if (!ensureCloudWriteAllowed("changer l'agent de la conversation")) return;
    if (agentId) {
      conversationCustomAgents = { ...conversationCustomAgents, [conversationId]: agentId };
      if (conversationPersonalities[conversationId]) {
        delete conversationPersonalities[conversationId];
        conversationPersonalities = { ...conversationPersonalities };
        saveConversationPersonalities();
      }
    } else {
      if (conversationCustomAgents[conversationId]) {
        delete conversationCustomAgents[conversationId];
        conversationCustomAgents = { ...conversationCustomAgents };
      }
    }
    localStorage.setItem("aro-conversation-custom-agents", JSON.stringify(conversationCustomAgents));
  }


  function getCurrentTabLabel(tab: SettingsTab, labels: Record<string, string>, language: "fr" | "en") {
    if (!labels) return "";
    switch (tab) {
      case "profile": return labels.profileTab;
      case "organization": return labels.orgTab;
      case "general":
      case "models": return labels.modelTab;
      case "system-prompt": return language === "fr" ? "Instructions ARO" : "ARO Instructions";
      case "instructions": return labels.instructionsTab;
      case "memory": return labels.memoryTab;
      case "voice": return labels.voiceTab;
      case "preferences": return labels.preferencesTab;
      case "skills": return "Skills";
      case "plugins": return "Plugins";
      case "mcp": return "Model Context Protocol (MCP)";
      case "hooks": return "Hooks & Webhooks";
      case "scheduler": return "Planificateur";
      case "permissions": return labels.permissionsTab;
      case "search": return language === "fr" ? "Moteur de recherche" : "Search Engine";
      case "paths": return labels.pathsTab;
      case "monitoring": return labels.monitoringTab;
      case "system": return labels.maintenanceTab;
      default: return "";
    }
  }

  // Load profile and org data in onMount
  function loadProfileAndOrgData() {
    if (typeof localStorage !== "undefined") {
      const savedProfile = localStorage.getItem("aro-user-profile");
      if (savedProfile) {
        try { userProfile = JSON.parse(savedProfile); } catch (e) { console.error(e); }
      }
      
      const savedOrg = localStorage.getItem("aro-active-org");
      if (savedOrg) {
        try { activeOrg = JSON.parse(savedOrg); } catch (e) { console.error(e); }
      }

      const savedMembers = localStorage.getItem("aro-org-members");
      if (savedMembers) {
        try { orgMembers = JSON.parse(savedMembers); } catch (e) { console.error(e); }
      }

      const savedTeams = localStorage.getItem("aro-org-teams");
      if (savedTeams) {
        try { orgTeams = JSON.parse(savedTeams); } catch (e) { console.error(e); }
      }

      const savedApiKeys = localStorage.getItem("aro-api-keys");
      if (savedApiKeys) {
        try { apiKeys = JSON.parse(savedApiKeys); } catch (e) { console.error(e); }
      }
    }
    loadInstructionsAndMemory();
  }

  // Save helpers
  let profileSaveTimer: number | null = null;
  let orgSaveTimer: number | null = null;

  function saveUserProfile() {
    if (!ensureCloudWriteAllowed("modifier le profil")) return;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-user-profile", JSON.stringify(userProfile));
    }
    userProfile = { ...userProfile };
    if (profileSaveTimer) window.clearTimeout(profileSaveTimer);
    profileSaveTimer = window.setTimeout(async () => {
      if (!canWriteCloudCollections()) return;
      try {
        const updated = await updateCloudUserProfile({
          name: userProfile.name,
          roleTitle: userProfile.roleTitle,
          avatarColor: userProfile.avatarColor,
        });
        userProfile = {
          name: updated.name,
          email: updated.email,
          roleTitle: updated.roleTitle ?? "",
          avatarColor: updated.avatarColor ?? userProfile.avatarColor,
        };
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }, 500);
  }

  function saveOrgInfo() {
    if (!ensureCloudWriteAllowed("modifier l'organisation")) return;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-active-org", JSON.stringify(activeOrg));
    }
    activeOrg = { ...activeOrg };
    if (orgSaveTimer) window.clearTimeout(orgSaveTimer);
    orgSaveTimer = window.setTimeout(async () => {
      const organizationId = cloudSession?.activeOrganization.id;
      if (!canWriteCloudCollections() || !organizationId) return;
      try {
        const updated = await updateCloudOrganization(organizationId, {
          name: activeOrg.name,
          domain: activeOrg.domain,
          description: activeOrg.description,
        });
        activeOrg = {
          name: updated.name,
          domain: updated.domain ?? "",
          description: updated.description ?? "",
        };
        if (cloudSession) {
          cloudSession = { ...cloudSession, activeOrganization: updated };
        }
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }, 500);
  }

  function saveMembers() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-org-members", JSON.stringify(orgMembers));
    }
    orgMembers = [...orgMembers];
  }

  function saveTeams() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-org-teams", JSON.stringify(orgTeams));
    }
    orgTeams = [...orgTeams];
    void syncTeamsToCloud();
  }

  function saveApiKeys() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(
        "aro-api-keys",
        JSON.stringify(apiKeys.map(({ oneTimeSecret, ...key }) => key)),
      );
    }
    apiKeys = [...apiKeys];
  }

  async function syncTeamsToCloud() {
    if (!canWriteCloudCollections()) return;
    try {
      const savedTeams = await Promise.all(
        orgTeams.map((team) => upsertCloudItem("teams", team, teamPayload(team))),
      );
      const cloudIdsById = new Map(savedTeams.map((team) => [team.id, team.cloudId]));
      orgTeams = orgTeams.map((team) => ({
        ...team,
        cloudId: cloudIdsById.get(team.id) ?? team.cloudId,
      }));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  // API Key Management
  async function generateApiKey(name: string) {
    if (!ensureCloudWriteAllowed("creer une cle API")) return;
    if (!name.trim()) return;
    try {
      if (canWriteCloudCollections()) {
        const response = await createCloudApiKey(name.trim());
        apiKeys = [
          { ...mapApiKeyRecord(response.key), oneTimeSecret: response.secret, visible: true },
          ...apiKeys,
        ];
      } else {
        const secret = `aro_live_${crypto.randomUUID().replaceAll("-", "")}`;
        apiKeys = [
          {
            id: crypto.randomUUID(),
            name: name.trim(),
            prefix: secret.substring(0, 14),
            oneTimeSecret: secret,
            createdAt: new Date().toISOString().split("T")[0],
            visible: true,
          },
          ...apiKeys,
        ];
      }
      saveApiKeys();
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  async function revokeApiKey(id: string) {
    if (!ensureCloudWriteAllowed("revoquer une cle API")) return;
    if (!await confirmDanger(
      "Révoquer cette clé API ?",
      "Revoke this API key?",
      "Les intégrations qui l'utilisent cesseront de fonctionner.",
      "Integrations using it will stop working.",
    )) return;
    apiKeys = apiKeys.filter(k => k.id !== id);
    if (canWriteCloudCollections()) {
      try {
        await revokeCloudApiKey(id);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveApiKeys();
  }

  function toggleKeyVisibility(id: string) {
    apiKeys = apiKeys.map(k => k.id === id ? { ...k, visible: !k.visible } : k);
    saveApiKeys();
  }

  // Member Management
  async function inviteMember(name: string, email: string, role: "admin" | "manager" | "member" | "guest") {
    if (!ensureCloudWriteAllowed("inviter un membre")) return;
    if (!name.trim() || !email.trim()) return;
    try {
      let newMember: OrgMember;
      if (canWriteCloudCollections()) {
        const receipt = await inviteCloudMember({
          name: name.trim(),
          email: email.trim(),
          role,
        });
        newMember = receipt.member
          ? {
              ...mapOrganizationMember(receipt.member),
              invitationId: receipt.invitationId,
              invitationStatus: "pending",
              invitationExpiresAt: receipt.expiresAt,
              invitationDeliveryStatus: "pending",
            }
          : {
              id: receipt.invitationId,
              invitationId: receipt.invitationId,
              invitationStatus: "pending",
              invitationExpiresAt: receipt.expiresAt,
              invitationDeliveryStatus: "pending",
              name: receipt.name,
              email: receipt.email,
              role: receipt.role,
              status: "invited",
            };
      } else {
        newMember = {
          id: crypto.randomUUID(),
          name: name.trim(),
          email: email.trim(),
          role,
          status: "invited",
        };
      }
      orgMembers = [...orgMembers, newMember];
      saveMembers();
      showInviteModal = false;
      inviteFormName = "";
      inviteFormEmail = "";
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  async function removeMember(id: string) {
    if (!ensureCloudWriteAllowed("supprimer un membre")) return;
    if (!await confirmDanger(
      "Retirer ce membre ?",
      "Remove this member?",
      "Il perdra l'accès à l'organisation.",
      "They will lose access to the organization.",
    )) return;
    const member = orgMembers.find(m => m.id === id);
    if (!member || id === "1" || member.userId === cloudSession?.user.id) return;
    if (member.invitationId) {
      try {
        if (canWriteCloudCollections()) {
          await revokeCloudInvitation(member.invitationId);
        }
        orgMembers = orgMembers.filter(m => m.id !== id);
        orgTeams = orgTeams.map(t => ({
          ...t,
          memberIds: t.memberIds.filter(mId => mId !== id),
        }));
        saveMembers();
        saveTeams();
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
      return;
    }
    orgMembers = orgMembers.filter(m => m.id !== id);
    orgTeams = orgTeams.map(t => ({
      ...t,
      memberIds: t.memberIds.filter(mId => mId !== id)
    }));
    if (canWriteCloudCollections()) {
      try {
        await removeCloudMember(member.cloudId ?? member.id);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveMembers();
    saveTeams();
  }

  async function updateMemberRole(id: string, role: "admin" | "manager" | "member" | "guest") {
    if (!ensureCloudWriteAllowed("modifier un role membre")) return;
    const member = orgMembers.find(m => m.id === id);
    if (!member || id === "1" || member.userId === cloudSession?.user.id) return;
    if (member.invitationId) {
      collectionsSyncError = "Le rôle d’une invitation en attente ne peut pas encore être modifié.";
      return;
    }
    try {
      if (canWriteCloudCollections()) {
        const updated = mapOrganizationMember(await updateCloudMemberRole(member.cloudId ?? member.id, role));
        orgMembers = orgMembers.map(m => m.id === id ? updated : m);
      } else {
        orgMembers = orgMembers.map(m => m.id === id ? { ...m, role } : m);
      }
      saveMembers();
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  // Team Management
  async function createTeam(name: string, description: string, memberIds: string[]) {
    if (!ensureCloudWriteAllowed("creer une equipe")) return;
    if (!name.trim()) return;
    let newTeam: OrgTeam = {
      id: crypto.randomUUID(),
      name: name.trim(),
      description: description.trim(),
      memberIds,
    };
    try {
      newTeam = await upsertCloudItem("teams", newTeam, teamPayload(newTeam));
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
    orgTeams = [...orgTeams, newTeam];
    saveTeams();
    showCreateTeamModal = false;
    teamFormName = "";
    teamFormDesc = "";
    teamFormMembers = [];
  }

  async function deleteTeam(id: string) {
    if (!ensureCloudWriteAllowed("supprimer une equipe")) return;
    if (!await confirmDanger("Supprimer cette équipe ?", "Delete this team?")) return;
    const team = orgTeams.find(t => t.id === id);
    orgTeams = orgTeams.filter(t => t.id !== id);
    if (team) {
      try {
        await deleteCloudItem("teams", team);
      } catch (error) {
        collectionsSyncError = normalizeError(error);
      }
    }
    saveTeams();
  }

  let filteredMembers: OrgMember[] = [];
  $: filteredMembers = filterOrganizationMembers(orgMembers, memberSearchQuery);

  function estimateTokens(text: string): number {
    if (!text) return 0;
    return Math.ceil(text.length / 4);
  }
  
  function addPerformanceRecord(conversationTitle: string, timeSec: number, tokens: number) {
    const newRecord = createPerformanceRecord(conversationTitle, timeSec, tokens, currentLanguage);
    
    lastResponseTime = newRecord.responseTime;
    lastTokenCount = tokens;
    lastTokenSpeed = newRecord.speed;
    
    performanceHistory = prependPerformanceRecord(performanceHistory, newRecord);
    
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("aro-perf-history", JSON.stringify(performanceHistory));
    }
    
    const summary = summarizePerformance(performanceHistory);
    avgResponseTime = summary.averageResponseTime;
    totalTokensGenerated = summary.totalTokensGenerated;

    // Track user prompt activity
    const todayStr = new Date().toISOString().split('T')[0];
    incrementActivity(todayStr);
  }

  let currentTheme: "light" | "dark" = (typeof localStorage !== "undefined" && localStorage.getItem("aro-theme") as "light" | "dark") || "light";
  let currentLanguage: "fr" | "en" = (typeof localStorage !== "undefined" && localStorage.getItem("aro-language") as "fr" | "en") || "fr";
  let themeDraft: "light" | "dark" = currentTheme;
  let languageDraft: "fr" | "en" = currentLanguage;

  function applyTheme(theme: "light" | "dark") {
    if (typeof document !== "undefined" && document.body) {
      if (theme === "dark") {
        document.body.classList.add("dark-theme");
      } else {
        document.body.classList.remove("dark-theme");
      }
    }
  }
  $: applyTheme(currentTheme);

  const translations = {
    fr: {
      localAssistant: "assistant local",
      hideSidebar: "Masquer la barre latérale",
      showSidebar: "Afficher la barre latérale",
      newConversation: "Nouvelle conversation",
      searchPlaceholder: "Rechercher...",
      clearSearch: "Effacer la recherche",
      conversations: "Conversations",
      noConversations: "Aucune conversation",
      conversationOptions: "Options de la conversation",
      rename: "Renommer",
      delete: "Supprimer",
      settings: "Paramètres",
      cleanEmptyConversations: "Supprimer les conversations vides (sans aucun message)",
      back: "Retour",
      backToChat: "Retour au chat",
      groupAccount: "Compte & Équipe",
      groupApp: "Configuration de l'IA",
      groupExtensions: "Extensions & APIs",
      groupPreferences: "Préférences & Audio",
      groupSystem: "Système & Métriques",
      modelTab: "Modèle IA",
      voiceTab: "Voix & Audio",
      pathsTab: "Chemins Système",
      maintenanceTab: "Maintenance",
      permissionsTab: "Autorisations de l'IA",
      preferencesTab: "Préférences",
      monitoringTab: "Monitoring",
      instructionsTab: "Profils ARO",
      instructionsTitle: "Profils ARO",
      instructionsDesc: "Créez et configurez les profils de réponse utilisés par les instructions ARO.",
      enableCustomPrompt: "Activer les profils personnalisés",
      systemPromptLabel: "Consignes du profil actif",
      charLimit: "limite de caractères",
      resetToDefault: "Réinitialiser",
      previewBehavior: "Aperçu du comportement",
      previewUserAsk: "Exemple de prompt utilisateur",
      previewBotAns: "Aperçu de la réponse d'ARO",
      presetSaved: "Profil configuré avec succès !",
      
      memoryTab: "Mémoire",
      memoryTitle: "Mémoire à long terme",
      memoryDesc: "Consultez et gérez les faits qu'ARO retient au fil de vos échanges pour contextualiser ses réponses.",
      memorySearchPlaceholder: "Rechercher un fait mémorisé...",
      memoryLimitLabel: "Utilisation de la mémoire",
      addMemoryBtn: "Ajouter un fait",
      newMemoryPlaceholder: "ex. L'utilisateur développe une application en Svelte...",
      newMemoryCategoryLabel: "Catégorie",
      saveBtn: "Enregistrer",
      cancelBtn: "Annuler",
      noMemoriesFound: "Aucun souvenir ne correspond à votre recherche.",
      
      catPersonal: "Personnel",
      catTechnical: "Technique",
      catSystem: "Système",
      catPreference: "Préférence",
      
      // Model tab
      modelConfigTitle: "Configuration du Modèle",
      modelConfigDesc: "Définir le fournisseur de modèle IA et ses comportements généraux.",
      providerTitle: "Fournisseur (Provider)",
      providerDesc: "Sélectionnez le backend d'inférence de l'assistant.",
      modelIdTitle: "Identifiant du modèle",
      modelIdDesc: "Nom du modèle utilisé pour les requêtes (ex: gemma2:latest).",
      ollamaEndpointTitle: "Endpoint Ollama",
      ollamaEndpointDesc: "Adresse du serveur Ollama local ou distant.",
      llamaCppEndpointTitle: "Endpoint llama.cpp",
      llamaCppEndpointDesc: "Adresse de l'instance d'inférence llama.cpp.",
      temperatureTitle: "Température",
      temperatureDesc: "Contrôle la créativité et l'aléatoire des réponses.",
      maxTokensTitle: "Tokens maximum",
      maxTokensDesc: "Limite de tokens générés dans les réponses.",
      retainHistoryTitle: "Conserver l'historique",
      retainHistoryDesc: "Garder les conversations précédentes au rechargement.",
      
      // Voice tab
      voiceTabTitle: "Voix & Audio",
      voiceTabDesc: "Configurer l'activation de la parole et de la synthèse vocale.",
      speakResponsesTitle: "Parler les réponses",
      speakResponsesDesc: "Lire automatiquement à voix haute les messages de l'assistant.",
      wakeWordTitle: "Mot d'éveil 'ARO'",
      wakeWordDesc: "Déclencher l'écoute automatique en disant 'ARO'.",
      voiceActivationTitle: "Activation vocale",
      voiceActivationDesc: "Permettre d'interagir par la voix.",
      sttEngineTitle: "Reconnaissance vocale (STT)",
      sttEngineDesc: "Moteur de conversion de la parole en texte.",
      disabled: "Désactivé",
      ttsEngineTitle: "Synthèse vocale (TTS)",
      ttsEngineDesc: "Moteur de génération de voix artificielle.",
      
      // Paths tab
      pathsTabTitle: "Chemins Système",
      pathsTabDesc: "Configurer les chemins d'accès locaux pour Whisper et Piper.",
      whisperBinaryTitle: "Binaire Whisper",
      whisperBinaryDesc: "Chemin vers l'exécutable whisper-cli ou whisper-cpp.",
      whisperModelTitle: "Modèle Whisper",
      whisperModelDesc: "Chemin vers le fichier de modèle Whisper (format .bin).",
      piperBinaryTitle: "Binaire Piper",
      piperBinaryDesc: "Chemin vers l'exécutable de synthèse vocale piper.",
      piperVoiceTitle: "Voix Piper",
      piperVoiceDesc: "Chemin vers le fichier de voix Piper (format .onnx).",
      
      // Maintenance tab
      maintenanceTabTitle: "Maintenance & Système",
      maintenanceTabDesc: "Vérifiez l'état de l'application et gérez sa mémoire locale.",
      runtimeStatusTitle: "Statut du runtime",
      runtimeStatusDesc: "Informations détaillées renvoyées par le serveur de runtime ARO.",
      noRuntimeInfo: "Aucune information de runtime disponible.",
      checkStatusTitle: "Vérifier l'état",
      checkStatusDesc: "Forcer une vérification du runtime et recharger les modèles.",
      checkBtn: "Vérifier",
      resetMemoryTitle: "Réinitialiser le stockage local",
      resetMemoryDesc: "ATTENTION: Supprime l'historique local et les souvenirs locaux. Les souvenirs déjà synchronisés dans le cloud se gèrent depuis l'onglet Mémoire.",
      resetBtn: "Réinitialiser",
      
      // Preferences tab
      preferencesTabTitle: "Préférences d'affichage et de langue",
      preferencesTabDesc: "Personnaliser le thème de couleur et la langue de l'interface utilisateur.",
      themeTitle: "Thème d'affichage",
      themeDesc: "Basculez entre le thème clair par défaut et le thème sombre.",
      themeLight: "Clair",
      themeDark: "Sombre",
      languageTitle: "Langue de l'interface",
      languageDesc: "Choisissez la langue d'affichage des menus et options.",
      languageFr: "Français",
      languageEn: "English",
      
      // Monitoring tab
      monitoringTabTitle: "Supervision du Système & Performances",
      monitoringTabDesc: "Visualisez l'utilisation des ressources système et les mesures de performance des modèles en temps réel.",
      ramUsage: "Utilisation RAM",
      cpuUsage: "Utilisation CPU",
      gpuUsage: "Utilisation GPU",
      responseTime: "Temps de réponse",
      tokensTitle: "Débit de Tokens",
      perfHistory: "Historique des Performances",
      avgResponseTime: "Temps de réponse moyen",
      inferenceSpeed: "Vitesse d'inférence",
      lastInference: "Dernière inférence",
      totalTokens: "Total de tokens",
      noPerfData: "Aucune donnée de performance enregistrée pour le moment. Envoyez des messages pour générer des statistiques.",
      secAbbr: "s",
      tokensPerSec: "tokens/s",
      activityTitle: "Activité d'utilisation",
      activitySubtitle: "requêtes envoyées au cours des 12 derniers mois",
      profileTab: "Profil",
      profileTabTitle: "Informations du Profil",
      profileTabDesc: "Gérez vos informations personnelles, votre avatar et vos clés d'API de développement.",
      profileName: "Nom complet",
      profileEmail: "Adresse e-mail",
      profileRole: "Rôle / Titre",
      avatarColorLabel: "Couleur de l'avatar",
      apiKeysTitle: "Clés d'API Personnelles",
      apiKeysDesc: "Clés d'API pour intégrer l'assistant ARO dans vos outils de développement.",
      keyNameLabel: "Nom de la clé",
      keyNamePlaceholder: "ex: Clé Production, SDK Local...",
      generateKeyBtn: "Générer une clé",
      revokeBtn: "Révoquer",
      orgTab: "Organisation",
      orgTabTitle: "Gestion de l'Organisation",
      orgTabDesc: "Configurez votre entreprise, gérez les membres de l'équipe et attribuez les privilèges d'accès.",
      orgName: "Nom de l'organisation",
      orgDomain: "Domaine de l'organisation",
      orgDesc: "Description de l'entreprise",
      currentRoleLabel: "Rôle courant",
      adminBadge: "Administrateur",
      memberBadge: "Membre",
      managerBadge: "Gestionnaire",
      guestBadge: "Invité",
      roleAdminLabel: "Admin 👑",
      roleManagerLabel: "Manager 👤",
      roleMemberLabel: "Membre 👤",
      roleGuestLabel: "Invité 👤",
      roleAdminDesc: "Accès complet",
      roleMemberDesc: "Accès en lecture seule",
      membersTitle: "Membres de l'organisation",
      inviteMemberBtn: "Inviter un collaborateur",
      searchMembersPlaceholder: "Rechercher un membre par nom ou e-mail...",
      teamsTitle: "Équipes",
      createTeamBtn: "Créer une équipe",
      noTeamsYet: "Aucune équipe créée pour le moment.",
      teamNameLabel: "Nom de l'équipe",
      teamDescLabel: "Description",
      teamMembersLabel: "Membres de l'équipe",
      deleteTeamConfirm: "Supprimer l'équipe",
      inviteModalTitle: "Inviter un nouveau membre",
      inviteNameLabel: "Nom complet",
      inviteEmailLabel: "Adresse e-mail",
      inviteRoleLabel: "Rôle d'accès",
      inviteSendBtn: "Envoyer l'invitation",
      adminOnlyWarning: "Action réservée aux administrateurs. Basculez en mode Administrateur pour modifier.",
      adminOnlyLocked: "Verrouillé (Mode Admin requis)",
      
      // Common / Modal
      cancel: "Annuler",
      save: "Enregistrer",
      renameConversationModalTitle: "Renommer la conversation",
      newTitleLabel: "Nouveau titre",
      enterTitlePlaceholder: "Entrez le titre...",
      deleteConversationDropdown: "Supprimer la conversation",
      renameConversationDropdown: "Renommer la conversation",
      showContext: "Afficher le contexte",
      outputs: "Sorties",
      noArtifactsYet: "Aucun artefact pour l'instant",
      sources: "Sources",
      noSourcesYet: "Aucune source pour l'instant",
      loadingText: "Chargement",
      recordBtnTitle: "Enregistrer",
      copyBtn: "Copier",
      copiedBtn: "Copié !",
      goodAnswerTitle: "Bonne réponse",
      badAnswerTitle: "Mauvaise réponse",
      addFilesTitle: "Ajouter des fichiers",
      askAroPlaceholder: "Demandez n'importe quoi à ARO",
      chooseModelTitle: "Choisir le modèle",
      sendBtnTitle: "Envoyer",
      missingText: "manquant",
      youLabel: "Vous",
      
      // Date relative
      justNow: "Inst.",
      yesterday: "Hier",
      todayAt: "Aujourd'hui à",
      yesterdayAt: "Hier à",
      at: "à",
      weeksAbbr: "sem",
      monthsAbbr: "mois",
      daysAbbr: "j",
      hoursAbbr: "h",
      minutesAbbr: "m",
      
      // Voice feedback
      voiceConfigWarning: "La voix locale n'est pas encore configurée. Activez Whisper dans les paramètres, puis renseignez le binaire et le modèle.",
      noTranscriptionError: "Aucune transcription retournée par le runtime voix local.",
      listeningHint: "À l'écoute...",
      transcribingHint: "Transcription...",
      transcribingLocallyHint: "Transcription locale...",
      voiceReadyLabel: "Voix prête",
      voiceReadyHint: "Appuyez sur le micro pour dicter.",
      voiceNeedsSetupLabel: "Voix à configurer",
      voiceNeedsSetupHint: "Ouvrez Voix & Audio pour activer la reconnaissance vocale.",
      voiceListeningLabel: "Écoute en cours",
      voiceManualDictationHint: "La dictée remplira le champ sans envoyer automatiquement.",
      voiceAutoSubmitHint: "La phrase sera envoyée après la transcription.",
      voiceWakeLabel: "Mot d'éveil actif",
      voiceWakeHint: "ARO attend le mot d'éveil sans envoyer de message.",
      voiceSpeakingLabel: "Lecture vocale",
      voiceSpeakingHint: "La réponse est lue à voix haute.",
      voiceHandsFreeLabel: "Mains libres",
      voicePushToTalkHint: "Appuyez de nouveau pour arreter et transcrire.",
      voiceHandsFreeIdleHint: "Choisissez Libre pour armer le micro; rien ne demarre au lancement.",
      voiceHandsFreeActiveHint: "ARO ecoute explicitement et enverra apres silence.",
      stopSpeakingBtn: "Arrêter la lecture"
    },
    en: {
      localAssistant: "local assistant",
      hideSidebar: "Hide sidebar",
      showSidebar: "Show sidebar",
      newConversation: "New conversation",
      searchPlaceholder: "Search...",
      clearSearch: "Clear search",
      conversations: "Conversations",
      noConversations: "No conversations",
      conversationOptions: "Conversation options",
      rename: "Rename",
      delete: "Delete",
      settings: "Settings",
      cleanEmptyConversations: "Delete empty conversations (no messages)",
      back: "Back",
      backToChat: "Back to chat",
      groupAccount: "Account & Team",
      groupApp: "AI Configuration",
      groupExtensions: "Extensions & APIs",
      groupPreferences: "Preferences & Audio",
      groupSystem: "System & Diagnostics",
      modelTab: "AI Model",
      voiceTab: "Voice & Audio",
      pathsTab: "System Paths",
      maintenanceTab: "Maintenance",
      permissionsTab: "AI Permissions",
      preferencesTab: "Preferences",
      monitoringTab: "Monitoring",
      instructionsTab: "ARO Profiles",
      instructionsTitle: "ARO Profiles",
      instructionsDesc: "Create and configure the response profiles used by ARO instructions.",
      enableCustomPrompt: "Enable custom profiles",
      systemPromptLabel: "Active profile instructions",
      charLimit: "character limit",
      resetToDefault: "Reset",
      previewBehavior: "Behavior Preview",
      previewUserAsk: "Example user prompt",
      previewBotAns: "ARO's response preview",
      presetSaved: "Profile loaded successfully!",
      
      memoryTab: "Memory",
      memoryTitle: "Long-term Memory",
      memoryDesc: "View and manage the facts ARO remembers across conversations to contextualize its answers.",
      memorySearchPlaceholder: "Search memorized facts...",
      memoryLimitLabel: "Memory usage",
      addMemoryBtn: "Add a fact",
      newMemoryPlaceholder: "e.g. User is building a Svelte application...",
      newMemoryCategoryLabel: "Category",
      saveBtn: "Save",
      cancelBtn: "Cancel",
      noMemoriesFound: "No memories match your search.",
      
      catPersonal: "Personal",
      catTechnical: "Technical",
      catSystem: "System",
      catPreference: "Preference",
      
      // Model tab
      modelConfigTitle: "Model Configuration",
      modelConfigDesc: "Define the AI model provider and general behaviors.",
      providerTitle: "Provider",
      providerDesc: "Select the assistant's inference backend.",
      modelIdTitle: "Model ID",
      modelIdDesc: "Name of the model used for requests (e.g. gemma2:latest).",
      ollamaEndpointTitle: "Ollama Endpoint",
      ollamaEndpointDesc: "Address of the local or remote Ollama server.",
      llamaCppEndpointTitle: "llama.cpp Endpoint",
      llamaCppEndpointDesc: "Address of the llama.cpp inference instance.",
      temperatureTitle: "Temperature",
      temperatureDesc: "Controls the creativity and randomness of responses.",
      maxTokensTitle: "Maximum Tokens",
      maxTokensDesc: "Limit of generated tokens in responses.",
      retainHistoryTitle: "Retain History",
      retainHistoryDesc: "Keep previous conversations upon reloading.",
      
      // Voice tab
      voiceTabTitle: "Voice & Audio",
      voiceTabDesc: "Configure voice activation and text-to-speech synthesis.",
      speakResponsesTitle: "Speak Responses",
      speakResponsesDesc: "Automatically read assistant messages out loud.",
      wakeWordTitle: "Wake Word 'ARO'",
      wakeWordDesc: "Trigger automatic listening by saying 'ARO'.",
      voiceActivationTitle: "Voice Activation",
      voiceActivationDesc: "Enable voice interaction.",
      sttEngineTitle: "Speech Recognition (STT)",
      sttEngineDesc: "Engine for speech-to-text conversion.",
      disabled: "Disabled",
      ttsEngineTitle: "Speech Synthesis (TTS)",
      ttsEngineDesc: "Artificial voice generation engine.",
      
      // Paths tab
      pathsTabTitle: "System Paths",
      pathsTabDesc: "Configure local directory paths for Whisper and Piper.",
      whisperBinaryTitle: "Whisper Binary",
      whisperBinaryDesc: "Path to whisper-cli or whisper-cpp executable.",
      whisperModelTitle: "Whisper Model",
      whisperModelDesc: "Path to Whisper model file (.bin format).",
      piperBinaryTitle: "Piper Binary",
      piperBinaryDesc: "Path to piper text-to-speech executable.",
      piperVoiceTitle: "Piper Voice",
      piperVoiceDesc: "Path to Piper voice file (.onnx format).",
      
      // Maintenance tab
      maintenanceTabTitle: "Maintenance & System",
      maintenanceTabDesc: "Check app status and manage local memory.",
      runtimeStatusTitle: "Runtime Status",
      runtimeStatusDesc: "Detailed information returned by ARO runtime server.",
      noRuntimeInfo: "No runtime information available.",
      checkStatusTitle: "Check Status",
      checkStatusDesc: "Force a runtime check and reload models.",
      checkBtn: "Check",
      resetMemoryTitle: "Reset Local Storage",
      resetMemoryDesc: "WARNING: Deletes local history and local memories. Cloud-synced memories are managed from the Memory tab.",
      resetBtn: "Reset",
      
      // Preferences tab
      preferencesTabTitle: "Display and Language Preferences",
      preferencesTabDesc: "Customize color theme and user interface language.",
      themeTitle: "Display Theme",
      themeDesc: "Switch between the default light theme and the dark theme.",
      themeLight: "Light",
      themeDark: "Dark",
      languageTitle: "Interface Language",
      languageDesc: "Choose UI language for menus and settings.",
      languageFr: "Français",
      languageEn: "English",
      
      // Monitoring tab
      monitoringTabTitle: "System Supervision & Performance",
      monitoringTabDesc: "View real-time system resource utilization and model performance metrics.",
      ramUsage: "RAM Usage",
      cpuUsage: "CPU Usage",
      gpuUsage: "GPU Usage",
      responseTime: "Response Time",
      tokensTitle: "Token Throughput",
      perfHistory: "Performance History",
      avgResponseTime: "Avg Response Time",
      inferenceSpeed: "Inference Speed",
      lastInference: "Last inference",
      totalTokens: "Total tokens",
      noPerfData: "No performance data recorded yet. Send messages to generate statistics.",
      secAbbr: "s",
      tokensPerSec: "tokens/s",
      activityTitle: "Usage Activity",
      activitySubtitle: "prompts sent in the last 12 months",
      profileTab: "Profile",
      profileTabTitle: "Profile Information",
      profileTabDesc: "Manage your personal details, custom avatar, and developer API keys.",
      profileName: "Full Name",
      profileEmail: "Email Address",
      profileRole: "Role / Title",
      avatarColorLabel: "Avatar Color",
      apiKeysTitle: "Personal API Keys",
      apiKeysDesc: "API keys to integrate the ARO assistant into your development workflows.",
      keyNameLabel: "Key Name",
      keyNamePlaceholder: "e.g., Production key, Local SDK...",
      generateKeyBtn: "Generate Key",
      revokeBtn: "Revoke",
      orgTab: "Organization",
      orgTabTitle: "Organization Management",
      orgTabDesc: "Configure your enterprise, manage team members, and assign access privileges.",
      orgName: "Organization Name",
      orgDomain: "Organization Domain",
      orgDesc: "Company Description",
      currentRoleLabel: "Current role",
      adminBadge: "Administrator",
      memberBadge: "Member",
      managerBadge: "Manager",
      guestBadge: "Guest",
      roleAdminLabel: "Admin 👑",
      roleManagerLabel: "Manager 👤",
      roleMemberLabel: "Member 👤",
      roleGuestLabel: "Guest 👤",
      roleAdminDesc: "Full access",
      roleMemberDesc: "Read-only access",
      membersTitle: "Organization Members",
      inviteMemberBtn: "Invite Collaborator",
      searchMembersPlaceholder: "Search members by name or email...",
      teamsTitle: "Teams",
      createTeamBtn: "Create Team",
      noTeamsYet: "No teams created yet.",
      teamNameLabel: "Team Name",
      teamDescLabel: "Description",
      teamMembersLabel: "Team Members",
      deleteTeamConfirm: "Delete Team",
      inviteModalTitle: "Invite New Member",
      inviteNameLabel: "Full Name",
      inviteEmailLabel: "Email Address",
      inviteRoleLabel: "Access Role",
      inviteSendBtn: "Send Invitation",
      adminOnlyWarning: "Administrator only action. Switch to Admin mode to modify.",
      adminOnlyLocked: "Locked (Admin mode required)",
      
      // Common / Modal
      cancel: "Cancel",
      save: "Save",
      renameConversationModalTitle: "Rename conversation",
      newTitleLabel: "New title",
      enterTitlePlaceholder: "Enter title...",
      deleteConversationDropdown: "Delete conversation",
      renameConversationDropdown: "Rename conversation",
      showContext: "Show context",
      outputs: "Outputs",
      noArtifactsYet: "No artifacts yet",
      sources: "Sources",
      noSourcesYet: "No sources yet",
      loadingText: "Loading",
      recordBtnTitle: "Record",
      copyBtn: "Copy",
      copiedBtn: "Copied!",
      goodAnswerTitle: "Good answer",
      badAnswerTitle: "Bad answer",
      addFilesTitle: "Add files",
      askAroPlaceholder: "Ask ARO anything",
      chooseModelTitle: "Choose model",
      sendBtnTitle: "Send",
      missingText: "missing",
      youLabel: "You",
      
      // Date relative
      justNow: "Now",
      yesterday: "Yesterday",
      todayAt: "Today at",
      yesterdayAt: "Yesterday at",
      at: "at",
      weeksAbbr: "w",
      monthsAbbr: "mo",
      daysAbbr: "d",
      hoursAbbr: "h",
      minutesAbbr: "m",
      
      // Voice feedback
      voiceConfigWarning: "Local voice is not yet configured. Enable Whisper in Settings, then specify the binary and model.",
      noTranscriptionError: "No transcription returned by the local voice runtime.",
      listeningHint: "Listening...",
      transcribingHint: "Transcribing...",
      transcribingLocallyHint: "Transcribing locally...",
      voiceReadyLabel: "Voice ready",
      voiceReadyHint: "Press the mic to dictate.",
      voiceNeedsSetupLabel: "Voice setup needed",
      voiceNeedsSetupHint: "Open Voice & Audio to enable speech recognition.",
      voiceListeningLabel: "Listening",
      voiceManualDictationHint: "Dictation will fill the field without sending automatically.",
      voiceAutoSubmitHint: "The phrase will be sent after transcription.",
      voiceWakeLabel: "Wake word active",
      voiceWakeHint: "ARO is waiting for the wake word without sending a message.",
      voiceSpeakingLabel: "Speaking",
      voiceSpeakingHint: "The response is being read out loud.",
      voiceHandsFreeLabel: "Hands-free",
      voicePushToTalkHint: "Press again to stop and transcribe.",
      voiceHandsFreeIdleHint: "Choose Hands-free to arm the mic; nothing starts on launch.",
      voiceHandsFreeActiveHint: "ARO is explicitly listening and will send after silence.",
      stopSpeakingBtn: "Stop speaking"
    }
  };

  let t = translations[currentLanguage];
  $: t = translations[currentLanguage];
  $: currentSettingsTabLabel = getCurrentTabLabel(activeSettingsTab, t, currentLanguage);

  $: sttOptions = [
    { value: "disabled", label: t.disabled },
    { value: "whisper-cpp", label: "Whisper.cpp" }
  ];

  $: ttsOptions = [
    { value: "disabled", label: t.disabled },
    { value: "piper", label: "Piper" }
  ];

  $: themeOptions = [
    { value: "light", label: t.themeLight },
    { value: "dark", label: t.themeDark }
  ];

  $: languageOptions = [
    { value: "fr", label: t.languageFr },
    { value: "en", label: t.languageEn }
  ];

  $: if (showSettings && !lastShowSettings) {
    if (settings) {
      settingsDraft = cloneSettings(settings);
    }
    themeDraft = currentTheme;
    languageDraft = currentLanguage;
    wakeWordDraftEnabled = wakeWordEnabled;
    activeSettingsTab = pendingSettingsTab ?? "profile";
    pendingSettingsTab = null;
    lastShowSettings = true;
  } else if (!showSettings && lastShowSettings) {
    pendingSettingsTab = null;
    lastShowSettings = false;
  }
  let windowWidth = 1024;
  let settingsMobileView: "sidebar" | "content" = "content";

  $: if (activeSettingsTab) {
    if (windowWidth < 768) {
      settingsMobileView = "content";
    }
  }

  let sidebarOpen = true;
  let sidebarWidth = 280;
  let isResizing = false;
  let showContextPanel = false;
  let showRightPanel = false;
  let rightPanelWidth = 500;
  let isResizingRightPanel = false;
  let collapsedContextSections: Record<ContextSectionKey, boolean> = {
    outputs: false,
    agentInbox: false,
    sources: true,
  };
  let collapsedAgentLaneIds: Record<string, boolean> = {};
  let agentTimelineCollapsed = false;
  let showConversationMenu = false;
  let activeSidebarMenuId: string | null = null;
  let renamingConversationObject: Conversation | null = null;
  let renameInputValue = "";
  let messageFeedback: Record<string, 'good' | 'bad' | null> = {};
  let copiedMessageId: string | null = null;
  let errorMessage = "";
  let recordingHint = "";
  let messagesEnd: HTMLDivElement;
  let conversationContainer: HTMLDivElement;
  let editingMessageId: string | null = null;
  let editingMessageText = "";
  let composerInput: HTMLTextAreaElement;
  let fileInput: HTMLInputElement;
  let recordingStream: MediaStream | null = null;
  let audioContext: AudioContext | null = null;
  let audioSource: MediaStreamAudioSourceNode | null = null;
  let audioProcessor: ScriptProcessorNode | null = null;
  let silentGain: GainNode | null = null;
  let voiceCaptureSession: VoiceCaptureSession | null = null;
  let pcmChunks: Float32Array[] = [];
  let recordingSampleRate = 16000;
  const voiceAudioConstraints = buildVoiceAudioConstraints();
  const maxRecordingSeconds = 60;
  const liveTranscriptionSeconds = 12;
  const wakeWordWindowSeconds = 2.5;

  // Wake word states
  let wakeWordEnabled = false;
  let wakeWordDraftEnabled = false;
  let wakeWordStream: MediaStream | null = null;
  let wakeWordAudioContext: AudioContext | null = null;
  let wakeWordAudioSource: MediaStreamAudioSourceNode | null = null;
  let wakeWordAudioProcessor: ScriptProcessorNode | null = null;
  let wakeWordSilentGain: GainNode | null = null;
  let wakeWordCaptureSession: VoiceCaptureSession | null = null;
  let wakeWordPcmChunks: Float32Array[] = [];
  let wakeWordSampleRate = 16000;
  let wakeWordTimer: any = null;
  let isCheckingWakeWord = false;

  // Un seul contexte partagé : un AudioContext par détection fuyait
  // (rafale de wakes = N contextes) et restait muet sous AutoplayPolicy.
  let chimeContext: AudioContext | null = null;

  function playWakeWordChime(): Promise<void> {
    return new Promise((resolve) => {
      try {
        if (!chimeContext || chimeContext.state === "closed") {
          chimeContext = new (window.AudioContext || (window as any).webkitAudioContext)();
        }
        const ctx = chimeContext;
        if (ctx.state === "suspended") {
          void ctx.resume().catch(() => {});
        }
        const playTone = (freq: number, startTime: number, duration: number) => {
          const osc = ctx.createOscillator();
          const gainNode = ctx.createGain();
          osc.type = "sine";
          osc.frequency.setValueAtTime(freq, startTime);
          gainNode.gain.setValueAtTime(0, startTime);
          gainNode.gain.linearRampToValueAtTime(0.2, startTime + 0.05);
          gainNode.gain.exponentialRampToValueAtTime(0.0001, startTime + duration);
          osc.connect(gainNode);
          gainNode.connect(ctx.destination);
          osc.start(startTime);
          osc.stop(startTime + duration);
          osc.onended = () => {
            try {
              osc.disconnect();
              gainNode.disconnect();
            } catch { /* ignore */ }
          };
        };
        const now = ctx.currentTime;
        playTone(659.25, now, 0.4); // E5
        playTone(1046.50, now + 0.15, 0.5); // C6
        setTimeout(resolve, 800);
      } catch (err) {
        console.error("Failed to play chime via Web Audio", err);
        resolve();
      }
    });
  }

  function boundAudioChunksForDuration(
    chunks: Float32Array[],
    sampleRate: number,
    seconds: number,
  ): Float32Array[] {
    const maxSamples = samplesForDuration(sampleRate, seconds);
    let bounded = { ...EMPTY_AUDIO_CHUNK_BUFFER };
    for (const chunk of chunks) {
      bounded = appendBoundedAudioChunk(bounded, chunk, maxSamples);
    }
    return [...bounded.chunks];
  }

  function firstVoiceIssue(status: { issues?: { message: string }[] } | null | undefined): string {
    return status?.issues?.[0]?.message ?? "";
  }

  function voiceRuntimeName(status: VoiceCapabilityStatus | null | undefined): string {
    if (!status || status.runtime === "disabled") return t.disabled;
    return status.runtime === "whisper-cpp"
      ? "Whisper.cpp"
      : status.runtime === "piper"
        ? "Piper"
        : status.runtime;
  }

  function disarmHandsFreeVoice() {
    voiceHandsFreeArmed = false;
    voiceAutoListen = false;
    voiceAutoRestartFailures = 0;
    stopWakeWordListening();
  }

  function voiceSetupMessage(): string {
    return (
      firstVoiceIssue(voiceSpeechToTextStatus) ||
      firstVoiceIssue(voiceRuntimeStatus) ||
      t.voiceConfigWarning
    );
  }

  function ensureVoiceRecordingReady(): boolean {
    if (!voiceConfigured || !voiceSpeechToTextReady) {
      errorMessage = voiceSetupMessage();
      openSettings("voice");
      return false;
    }
    return true;
  }

  function handleWakeWordAudioChunk(inputBuffer: Float32Array, sampleRate = wakeWordSampleRate) {
    wakeWordSampleRate = sampleRate;
    const bounded = appendBoundedAudioChunk(
      { ...EMPTY_AUDIO_CHUNK_BUFFER, chunks: wakeWordPcmChunks },
      inputBuffer,
      samplesForDuration(wakeWordSampleRate, wakeWordWindowSeconds),
    );
    wakeWordPcmChunks = [...bounded.chunks];
  }

  function startLegacyWakeWordCapture(stream: MediaStream) {
    wakeWordAudioContext = new AudioContext();
    wakeWordSampleRate = wakeWordAudioContext.sampleRate;
    wakeWordAudioSource = wakeWordAudioContext.createMediaStreamSource(stream);
    wakeWordAudioProcessor = wakeWordAudioContext.createScriptProcessor(4096, 1, 1);

    const silentGainNode = wakeWordAudioContext.createGain();
    silentGainNode.gain.value = 0;

    wakeWordAudioProcessor.onaudioprocess = (event) => {
      handleWakeWordAudioChunk(event.inputBuffer.getChannelData(0), wakeWordAudioContext?.sampleRate ?? wakeWordSampleRate);
    };

    wakeWordAudioSource.connect(wakeWordAudioProcessor);
    wakeWordAudioProcessor.connect(silentGainNode);
    silentGainNode.connect(wakeWordAudioContext.destination);
    wakeWordSilentGain = silentGainNode;
  }

  async function startWakeWordListening() {
    if (!voiceCanRecord || wakeWordStream || isCheckingWakeWord) return;
    wakeWordPcmChunks = [];
    try {
      const stream = await navigator.mediaDevices.getUserMedia(voiceAudioConstraints);
      wakeWordStream = stream;
      try {
        wakeWordCaptureSession = await createVoiceCaptureSession(stream, {
          closeContextOnStop: false,
          onChunk: (chunk: VoiceCaptureChunkMessage) => handleWakeWordAudioChunk(chunk.samples, chunk.sampleRate),
          onError: (err) => {
            console.error("Wake word capture failed", err);
            disarmHandsFreeVoice();
          },
          stopTracksOnStop: false,
        });
        wakeWordAudioContext = wakeWordCaptureSession.context;
        wakeWordSampleRate = wakeWordCaptureSession.sampleRate;
      } catch (workletError) {
        console.warn("AudioWorklet wake-word capture unavailable, using legacy capture", workletError);
        startLegacyWakeWordCapture(stream);
      }
      
      wakeWordTimer = setInterval(checkWakeWord, 1500);
    } catch (err) {
      console.error("Failed to start wake word listening", err);
      errorMessage = microphoneErrorMessage(err);
      // Même fuite potentielle qu'en enregistrement : le flux peut être
      // ouvert alors que la session de capture a échoué.
      try {
        wakeWordStream?.getTracks().forEach((track) => track.stop());
      } catch { /* ignore */ }
      try {
        await wakeWordAudioContext?.close();
      } catch { /* ignore */ }
      wakeWordStream = null;
      wakeWordAudioContext = null;
      disarmHandsFreeVoice();
    }
  }

  // Erreurs getUserMedia traduites en messages actionnables (au lieu du
  // brut "Permission denied" / "Could not start audio source").
  function microphoneErrorMessage(err: unknown): string {
    const name = err instanceof DOMException ? err.name : "";
    if (name === "NotAllowedError" || name === "SecurityError") {
      return currentLanguage === "fr"
        ? "Micro refusé : autorisez le microphone pour ARO dans le navigateur/système, puis réessayez."
        : "Microphone denied: allow microphone access for ARO, then retry.";
    }
    if (name === "NotFoundError" || name === "OverconstrainedError") {
      return currentLanguage === "fr"
        ? "Aucun microphone détecté : branchez un micro puis réessayez."
        : "No microphone detected: plug one in, then retry.";
    }
    if (name === "NotReadableError" || name === "AbortError") {
      return currentLanguage === "fr"
        ? "Micro occupé par une autre application : fermez-la puis réessayez."
        : "Microphone is busy in another app: close it, then retry.";
    }
    return normalizeError(err);
  }

  function stopWakeWordListening() {
    if (wakeWordTimer) {
      clearInterval(wakeWordTimer);
      wakeWordTimer = null;
    }
    try {
      void wakeWordCaptureSession?.stop();
      wakeWordAudioProcessor?.disconnect();
      wakeWordAudioSource?.disconnect();
      wakeWordSilentGain?.disconnect();
      wakeWordStream?.getTracks().forEach((track) => track.stop());
      // close() concurrent au disconnect() de stop() : rejet possible,
      // absorbé ici au lieu de fuir en promesse non gérée.
      void wakeWordAudioContext?.close()?.catch(() => {});
    } catch (err) {
      console.error("Error stopping wake word listening", err);
    } finally {
      wakeWordAudioProcessor = null;
      wakeWordCaptureSession = null;
      wakeWordAudioSource = null;
      wakeWordStream = null;
      wakeWordAudioContext = null;
      wakeWordPcmChunks = [];
      wakeWordSilentGain = null;
    }
  }

  async function checkWakeWord() {
    if (!voiceCanRecord || isCheckingWakeWord || wakeWordPcmChunks.length < 5) return;
    isCheckingWakeWord = true;
    try {
      const chunksCopy = [...wakeWordPcmChunks];
      const wavBytes = encodeVoiceWav(chunksCopy, wakeWordSampleRate);
      const result = await detectWakeWord(Array.from(wavBytes), "audio/wav", {
        language: currentLanguage,
        allowEmbeddedAro: false,
      });
      if (result.detected) {
        stopWakeWordListening();
        await playWakeWordChime();
        voiceHandsFreeArmed = true;
        voiceAutoListen = true;
        await startRecording();
      }
    } catch (err) {
      console.error("Wake word check failed", err);
    } finally {
      isCheckingWakeWord = false;
    }
  }

  $: voiceConfigured = Boolean(
    settings?.voice.enabled && settings.voice.speechToText !== "disabled",
  );
  $: voiceRuntimeStatus = voiceModelStatus?.voiceStatus ?? runtime?.voiceStatus ?? null;
  $: voiceSpeechToTextStatus = voiceRuntimeStatus?.speechToText ?? null;
  $: voiceTextToSpeechStatus = voiceRuntimeStatus?.textToSpeech ?? null;
  $: voiceWakeWordStatus = voiceRuntimeStatus?.wakeWord ?? null;
  $: voiceSpeechToTextReady = voiceRuntimeStatus
    ? Boolean(voiceSpeechToTextStatus?.ready)
    : voiceConfigured;
  $: voiceTextToSpeechReady = voiceRuntimeStatus
    ? Boolean(voiceTextToSpeechStatus?.ready)
    : Boolean(settings?.voice.enabled && settings.voice.textToSpeech !== "disabled");
  $: voiceCanRecord = voiceConfigured && voiceSpeechToTextReady;
  $: modelRuntimeReady = Boolean(runtime?.modelReady ?? currentModel?.ready ?? false);
  $: voiceWakeModelReady = Boolean(
    wakeWordEnabled && (voiceWakeWordStatus ? voiceWakeWordStatus.ready : voiceCanRecord),
  );
  $: voiceModeOptions = [
    {
      id: "push-to-talk" as const,
      label: currentLanguage === "fr" ? "Appui" : "Push",
      title: currentLanguage === "fr" ? "Appui manuel: transcription uniquement a l'arret." : "Manual push-to-talk: transcribes only when stopped.",
    },
    {
      id: "dictation" as const,
      label: currentLanguage === "fr" ? "Dictee" : "Dictation",
      title: currentLanguage === "fr" ? "Dictee: apercu live dans le champ, sans envoi automatique." : "Dictation: live field preview, no automatic send.",
    },
    {
      id: "hands-free" as const,
      label: currentLanguage === "fr" ? "Libre" : "Hands-free",
      title: wakeWordEnabled
        ? (currentLanguage === "fr" ? "Mains libres: attend le mot d'eveil ARO avant d'ecouter." : "Hands-free: waits for the ARO wake word before listening.")
        : (currentLanguage === "fr" ? "Mains libres: ecoute explicite et envoi apres silence." : "Hands-free: explicit listening and sends after silence."),
    },
  ];
  $: modelReadinessLabel = modelRuntimeReady
    ? (currentLanguage === "fr" ? "Modèle prêt" : "Model ready")
    : (currentLanguage === "fr" ? "Modele a configurer" : "Model setup needed");
  $: speechReadinessLabel = voiceSpeechToTextReady
    ? `${voiceRuntimeName(voiceSpeechToTextStatus)} ${currentLanguage === "fr" ? "prêt" : "ready"}`
    : (currentLanguage === "fr" ? "STT a configurer" : "STT setup needed");
  $: ttsReadinessLabel = voiceTextToSpeechReady
    ? `${voiceRuntimeName(voiceTextToSpeechStatus)} ${currentLanguage === "fr" ? "prêt" : "ready"}`
    : (currentLanguage === "fr" ? "TTS a configurer" : "TTS setup needed");
  $: wakeWordReadinessLabel = !wakeWordEnabled
    ? (currentLanguage === "fr" ? "Mot d'eveil coupe" : "Wake word off")
    : voiceWakeModelReady
      ? (voiceHandsFreeArmed
        ? (currentLanguage === "fr" ? "Mot d'eveil en ecoute" : "Wake word listening")
        : (currentLanguage === "fr" ? "Mot d'éveil prêt" : "Wake word ready"))
      : (firstVoiceIssue(voiceWakeWordStatus) ||
        (currentLanguage === "fr" ? "Mot d'eveil a configurer" : "Wake word setup needed"));
  $: voiceReadinessIssue = !voiceConfigured
    ? t.voiceNeedsSetupHint
    : !voiceSpeechToTextReady
      ? voiceSetupMessage()
      : "";
  $: if (!voiceCanRecord) {
    disarmHandsFreeVoice();
  }
  $: voiceStateLabel = assistantSpeaking
    ? t.voiceSpeakingLabel
    : recording
      ? t.voiceListeningLabel
      : !voiceConfigured || !voiceSpeechToTextReady
        ? t.voiceNeedsSetupLabel
        : voiceInputMode === "hands-free"
          ? t.voiceHandsFreeLabel
        : wakeWordEnabled && voiceHandsFreeArmed
          ? t.voiceWakeLabel
          : t.voiceReadyLabel;
  $: voiceStateHint = assistantSpeaking
    ? t.voiceSpeakingHint
    : recording
      ? (voiceInputMode === "push-to-talk" ? t.voicePushToTalkHint : ((voiceAutoListen || voiceInputMode === "hands-free") ? t.voiceAutoSubmitHint : t.voiceManualDictationHint))
      : voiceReadinessIssue
        ? voiceReadinessIssue
        : voiceInputMode === "hands-free"
          ? (voiceHandsFreeArmed
            ? (wakeWordEnabled ? t.voiceWakeHint : t.voiceHandsFreeActiveHint)
            : t.voiceHandsFreeIdleHint)
          : voiceInputMode === "push-to-talk"
            ? t.voicePushToTalkHint
            : t.voiceReadyHint;
  $: activeModelKey = settings?.model.activeModelRef
    ? modelOptionKey(settings.model.activeModelRef)
    : "";
  $: currentModel = modelOptions.find((model) => model.id === activeModelKey) ?? null;
  $: currentModelLabel = currentModel?.label ?? settings?.model.activeModelRef?.label ?? settings?.model.modelId ?? "Model";
  $: localModelOptions = modelOptions.filter((model) => model.local);
  $: connectedModelOptions = modelOptions.filter((model) => !model.local && model.ready);
  $: unavailableModelOptions = modelOptions.filter((model) => !model.local && !model.ready);
  $: filteredModelOptions = modelOptions.filter((model) => {
    const query = modelSearchQuery.trim().toLowerCase();
    if (!query) return true;
    return `${model.label} ${model.modelId} ${model.family ?? ""} ${model.providerKind}`.toLowerCase().includes(query);
  });

  $: if (typeof document !== "undefined") {
    if (isSpotlightMode) {
      document.body.style.background = "transparent";
      document.documentElement.style.background = "transparent";
    }
  }

  $: if (isSpotlightMode) {
    const _rec = recording;
    const _msgs = spotlightMessages;
    const _sending = spotlightSending;
    const _menu = modelMenuOpen;
    const _persMenu = spotlightPersonalityMenuOpen;
    const _files = attachedFiles;
    updateSpotlightWindowSize();
  }

  $: {
    if (voiceHandsFreeArmed && voiceInputMode === "hands-free" && voiceCanRecord && !wakeWordEnabled && !recording && !recordingStarting && !activeVoiceSession && !assistantSpeaking && !sending && !loading) {
      voiceAutoListen = true;
      startRecording();
    }
  }

  $: {
    if (voiceHandsFreeArmed && voiceInputMode === "hands-free" && wakeWordEnabled && voiceCanRecord && !recording && !recordingStarting && !activeVoiceSession && !assistantSpeaking && !sending && !loading) {
      startWakeWordListening();
    } else {
      stopWakeWordListening();
    }
  }

  $: {
    if (activeConversation && !recording && !recordingStarting && !activeVoiceSession) {
      hasSpoken = false;
      pcmChunks = [];
    }
  }

  function startResizing(event: MouseEvent) {
    event.preventDefault();
    isResizing = true;
    document.body.style.cursor = "col-resize";
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", stopResizing);
  }

  function handleMouseMove(event: MouseEvent) {
    if (!isResizing) return;
    const newWidth = Math.max(200, Math.min(event.clientX, 480));
    sidebarWidth = newWidth;
  }

  function stopResizing() {
    isResizing = false;
    document.body.style.cursor = "";
    window.removeEventListener("mousemove", handleMouseMove);
    window.removeEventListener("mouseup", stopResizing);
  }

  function startResizingRightPanel(event: MouseEvent) {
    event.preventDefault();
    isResizingRightPanel = true;
    document.body.style.cursor = "col-resize";
    window.addEventListener("mousemove", handleMouseMoveRightPanel);
    window.addEventListener("mouseup", stopResizingRightPanel);
  }

  function handleMouseMoveRightPanel(event: MouseEvent) {
    if (!isResizingRightPanel) return;
    const newWidth = Math.max(320, Math.min(window.innerWidth - event.clientX, Math.min(650, window.innerWidth * 0.5)));
    rightPanelWidth = newWidth;
  }

  function stopResizingRightPanel() {
    isResizingRightPanel = false;
    document.body.style.cursor = "";
    window.removeEventListener("mousemove", handleMouseMoveRightPanel);
    window.removeEventListener("mouseup", stopResizingRightPanel);
  }

  onMount(() => {
    const unlisteners: (() => void)[] = [];
    loadShortcuts();
    void loadWorkspaceMentions(activeConversation?.id ?? null);
    if (typeof window !== "undefined") {
      const fragment = new URLSearchParams(window.location.hash.replace(/^#/, ""));
      const invitationToken = fragment.get("token")?.trim() ?? "";
      if (invitationToken.length >= 32) {
        cloudInvitationToken = invitationToken;
        cloudAuthMode = "invitation";
        showCloudAuthPanel = true;
        window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}`);
      }
    }

    const onAroPrompt = (e: any) => {
      const text = e.detail?.text;
      if (text) {
        void sendUserPrompt(text);
      }
    };
    window.addEventListener("aro:send-prompt", onAroPrompt);
    unlisteners.push(() => window.removeEventListener("aro:send-prompt", onAroPrompt));

    const onWorkspaceTreeRefresh = async (_e: any) => {
      try {
        projects = await listProjects();
        folders = await listFolders();
      } catch {
        /* ignored */
      }
      void loadWorkspaceMentions(activeConversation?.id ?? null);
    };
    window.addEventListener("aro:workspace-tree-refresh", onWorkspaceTreeRefresh);
    unlisteners.push(() => window.removeEventListener("aro:workspace-tree-refresh", onWorkspaceTreeRefresh));

    const onPreviewDiff = (e: any) => {
      showRightPanel = true;
      if (e.detail) {
        window.dispatchEvent(new CustomEvent("aro:select-artifact", { detail: e.detail }));
      }
    };
    window.addEventListener("aro:preview-diff", onPreviewDiff);
    unlisteners.push(() => window.removeEventListener("aro:preview-diff", onPreviewDiff));

    const onApplyDiff = async (e: any) => {
      const detail = e.detail;
      if (!detail?.filePath || !detail?.diff) return;
      try {
        await applyWorkspaceDiff(detail.filePath, detail.diff, activeConversation?.id ?? undefined);
        window.dispatchEvent(new CustomEvent("aro:workspace-tree-refresh", { detail: { path: detail.filePath } }));
      } catch (err) {
        console.error("Failed to apply diff from event:", err);
      }
    };
    window.addEventListener("aro:apply-diff", onApplyDiff);
    unlisteners.push(() => window.removeEventListener("aro:apply-diff", onApplyDiff));

    const onAroRequestAiPlan = (e: any) => {
      const prompt = e.detail?.prompt;
      if (prompt) {
        void handleRequestAiPlan(prompt);
      }
    };
    window.addEventListener("aro:request-ai-plan", onAroRequestAiPlan);
    unlisteners.push(() => window.removeEventListener("aro:request-ai-plan", onAroRequestAiPlan));

    // "Examiner avec l'Agent" (explorateur / viewer) : insère @chemin dans
    // le composer et rend le focus pour enchaîner directement.
    const onExamineWorkspaceFile = (e: any) => {
      const path = e.detail?.path;
      if (!path || typeof path !== "string") return;
      const mention = `@${path} `;
      if (!input.includes(mention)) {
        input = `${input}${input.endsWith(" ") || input.length === 0 ? "" : " "}${mention}`;
      }
      resizeComposer();
      requestAnimationFrame(() => {
        try {
          composerInput?.focus();
        } catch {
          /* focus indisponible : on ignore */
        }
      });
    };
    window.addEventListener("aro:examine-workspace-file", onExamineWorkspaceFile);
    unlisteners.push(() => window.removeEventListener("aro:examine-workspace-file", onExamineWorkspaceFile));


    const webUnsubscribe = (e: any) => {
      const payload = e.detail;
      const { conversationId, messageId, content, done } = payload;
      if (done) return;

      if (isSpotlightMode) {
        spotlightMessages = spotlightMessages.map(msg => {
          if (msg.id === "spotlight-temp-generating" || msg.id === messageId) {
            return {
              ...msg,
              id: messageId,
              content: msg.content + content,
              isGenerating: false,
            };
          }
          return msg;
        });
        tick().then(() => {
          const resultsArea = document.querySelector(".spotlight-results");
          if (resultsArea) {
            resultsArea.scrollTop = resultsArea.scrollHeight;
          }
          updateSpotlightWindowSize();
        });
      } else {
        const applyChunk = (items: ChatMessage[]) => items.map(msg => {
          if (msg.id === "temp-generating" || msg.id === messageId) {
            return {
              ...msg,
              id: messageId,
              content: msg.content + content,
              isGenerating: false,
            };
          }
          return msg;
        });
        const inFlightMessages = inFlightMessagesByConversation[conversationId];
        if (inFlightMessages) {
          setInFlightMessages(conversationId, applyChunk(inFlightMessages));
        }
        const visibleHasChunkTarget = messages.some((msg) => msg.id === "temp-generating" || msg.id === messageId);
        if (activeConversation?.id === conversationId || (!activeConversation && visibleHasChunkTarget)) {
          messages = applyChunk(messages);
          scrollToBottom(false);
        }
      }
    };
    window.addEventListener("aro-chat-stream-chunk", webUnsubscribe);
    unlisteners.push(() => window.removeEventListener("aro-chat-stream-chunk", webUnsubscribe));

    if (Boolean(window.__TAURI_INTERNALS__)) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen("chat-stream-chunk", (event) => {
          const payload = event.payload as {
            conversationId: string;
            messageId: string;
            content: string;
            done: boolean;
          };
          const { conversationId, messageId, content, done } = payload;
          if (done) return;

          if (isSpotlightMode) {
            spotlightMessages = spotlightMessages.map(msg => {
              if (msg.id === "spotlight-temp-generating" || msg.id === messageId) {
                return {
                  ...msg,
                  id: messageId,
                  content: msg.content + content,
                  isGenerating: false,
                };
              }
              return msg;
            });
            tick().then(() => {
              const resultsArea = document.querySelector(".spotlight-results");
              if (resultsArea) {
                resultsArea.scrollTop = resultsArea.scrollHeight;
              }
              updateSpotlightWindowSize();
            });
          } else {
            const applyChunk = (items: ChatMessage[]) => items.map(msg => {
              if (msg.id === "temp-generating" || msg.id === messageId) {
                return {
                  ...msg,
                  id: messageId,
                  content: msg.content + content,
                  isGenerating: false,
                };
              }
              return msg;
            });
            const inFlightMessages = inFlightMessagesByConversation[conversationId];
            if (inFlightMessages) {
              setInFlightMessages(conversationId, applyChunk(inFlightMessages));
            }
            const visibleHasChunkTarget = messages.some((msg) => msg.id === "temp-generating" || msg.id === messageId);
            if (activeConversation?.id === conversationId || (!activeConversation && visibleHasChunkTarget)) {
              messages = applyChunk(messages);
              scrollToBottom(false);
            }
          }
        }).then(unlisten => unlisteners.push(unlisten));
      });

      // Arena stream listener
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen("arena-stream-chunk", (event) => {
          handleArenaChunk(event.payload as ArenaStreamChunk);
        }).then(unlisten => unlisteners.push(unlisten));
      });

      // Agent step update listener
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen("agent-step-update", (event) => {
          const payload = event.payload as {
            conversationId: string;
            step: AgentStep;
          };
          const { conversationId, step } = payload;
          const runId = step.runId;

          // Stream live steps directly to selectedAgentRunView if inspected
          if (selectedAgentRunView && selectedAgentRunView.run?.id === runId) {
            const currentRunSteps = [...(selectedAgentRunView.steps || [])];
            const sIdx = currentRunSteps.findIndex((s) => s.sequence === step.sequence);
            if (sIdx >= 0) {
              currentRunSteps[sIdx] = step;
            } else {
              currentRunSteps.push(step);
            }
            currentRunSteps.sort((a, b) => a.sequence - b.sequence);
            selectedAgentRunView = {
              ...selectedAgentRunView,
              steps: currentRunSteps,
            };
          }

          let messageId = runToMessageMap[runId];

          if (!messageId) {
            const activeMsg = messages.find(m => m.isGenerating && m.conversationId === conversationId)
              || spotlightMessages.find(m => m.isGenerating && m.conversationId === conversationId);
            if (activeMsg) {
              messageId = activeMsg.id;
              runToMessageMap[runId] = messageId;
            }
          }

          if (messageId) {
            const currentSteps = stepsByMessageId[messageId] || [];
            const idx = currentSteps.findIndex(s => s.sequence === step.sequence);
            if (idx >= 0) {
              currentSteps[idx] = step;
            } else {
              currentSteps.push(step);
            }
            currentSteps.sort((a, b) => a.sequence - b.sequence);
            stepsByMessageId[messageId] = currentSteps;
            stepsByMessageId = stepsByMessageId;

            if (expandedMessageSteps[messageId] === undefined) {
              expandedMessageSteps[messageId] = true;
              expandedMessageSteps = expandedMessageSteps;
            }

            if (step.kind === "final" || step.status === "failed") {
              setTimeout(() => {
                expandedMessageSteps[messageId] = false;
                expandedMessageSteps = expandedMessageSteps;
              }, 1200);
            }

            const applySteps = (items: ChatMessage[]) => items.map(msg => {
              if (msg.id === messageId) {
                return {
                  ...msg,
                  steps: currentSteps,
                };
              }
              return msg;
            });

            const inFlightMessages = inFlightMessagesByConversation[conversationId];
            if (inFlightMessages) {
              setInFlightMessages(conversationId, applySteps(inFlightMessages));
            }
            if (activeConversation?.id === conversationId) {
              messages = applySteps(messages);
            }
            if (isSpotlightMode) {
              spotlightMessages = applySteps(spotlightMessages);
            }
          }
        }).then(unlisten => unlisteners.push(unlisten));
      });
    }

    if (isSpotlightMode && Boolean(window.__TAURI_INTERNALS__)) {
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
        const appWindow = getCurrentWindow();
        appWindow.listen("tauri://blur", () => {
          appWindow.hide();
        }).then(unlisten => unlisteners.push(unlisten));
        appWindow.listen("tauri://focus", () => {
          setTimeout(() => {
            spotlightInputEl?.focus();
          }, 50);
        }).then(unlisten => unlisteners.push(unlisten));
      });
    } else if (!isSpotlightMode && Boolean(window.__TAURI_INTERNALS__)) {
      import("@tauri-apps/api/event").then(({ listen }) => {
        listen("open-conversation", async (event) => {
          const convId = event.payload as string | null;
          if (convId) {
            await loadBootstrap();
            const conv = conversations.find(c => c.id === convId);
            if (conv) {
              await openConversation(conv);
            }
          }
        }).then(unlisten => unlisteners.push(unlisten));
      });
    }

    (window as any).__copyCodeBlock = async (btn: HTMLButtonElement) => {
      const container = btn.closest('.code-block-container');
      if (!container) return;
      const code = decodeURIComponent(container.getAttribute('data-code') || '');
      try {
        await navigator.clipboard.writeText(code);
        btn.innerText = currentLanguage === "fr" ? "Copié !" : "Copied!";
        btn.classList.add('copied');
        setTimeout(() => {
          btn.innerText = currentLanguage === "fr" ? "Copier" : "Copy";
          btn.classList.remove('copied');
        }, 2000);
      } catch (err) {
        console.error("Failed to copy code block", err);
      }
    };

    (window as any).__previewCodeBlock = (btn: HTMLButtonElement) => {
      const container = btn.closest('.code-block-container');
      if (!container) return;
      const code = decodeURIComponent(container.getAttribute('data-code') || '');
      codePreviewHtml = code;
      isCodePreviewOpen = true;
      if (isSpotlightMode) {
        setTimeout(updateSpotlightWindowSize, 50);
      }
    };

    (window as any).__sendChatOption = (target: string | HTMLElement) => {
      let optionText = "";
      if (typeof target === "string") {
        optionText = target;
      } else if (target) {
        const raw = target.getAttribute?.("data-option");
        if (raw) {
          try { optionText = decodeURIComponent(raw); } catch { optionText = raw; }
        }
        if (!optionText && target.textContent) {
          optionText = target.textContent.trim();
        }
      }
      if (optionText) {
        void sendUserPrompt(optionText);
      }
    };

    (window as any).__sendChatInput = (inputId: string) => {
      const el = document.getElementById(inputId) as HTMLInputElement;
      if (el && el.value.trim()) {
        const val = el.value.trim();
        el.value = "";
        void sendUserPrompt(val);
      }
    };

    (window as any).__useToolPrompt = (target: string | HTMLElement) => {
      let toolName = "";
      if (typeof target === "string") {
        toolName = target;
      } else if (target) {
        const raw = target.getAttribute?.("data-tool");
        if (raw) {
          try { toolName = decodeURIComponent(raw); } catch { toolName = raw; }
        }
      }
      if (toolName) void sendUserPrompt(`Exécute l'outil ${toolName}`);
    };

    const handleGlobalOptionClick = (e: MouseEvent) => {
      if ((window as any).__aroChatInteractionsInstalled) return;
      const chip = (e.target as HTMLElement)?.closest?.(".form-option-chip") as HTMLElement | null;
      if (chip) {
        e.preventDefault();
        e.stopPropagation();
        let optionText = "";
        const raw = chip.getAttribute("data-option");
        if (raw) {
          try { optionText = decodeURIComponent(raw); } catch { optionText = raw; }
        }
        if (!optionText && chip.textContent) {
          optionText = chip.textContent.trim();
        }
        if (optionText) {
          chip.style.opacity = "0.6";
          chip.style.transform = "scale(0.96)";
          setTimeout(() => {
            chip.style.opacity = "";
            chip.style.transform = "";
          }, 300);
          void sendUserPrompt(optionText);
        }
        return;
      }

      const submitBtn = (e.target as HTMLElement)?.closest?.(".form-submit-btn") as HTMLElement | null;
      if (submitBtn) {
        e.preventDefault();
        e.stopPropagation();
        const row = submitBtn.closest(".form-input-row");
        const inputEl = row?.querySelector(".form-text-input") as HTMLInputElement | null;
        if (inputEl && inputEl.value.trim()) {
          const val = inputEl.value.trim();
          inputEl.value = "";
          void sendUserPrompt(val);
        }
        return;
      }

      const toolBtn = (e.target as HTMLElement)?.closest?.(".tool-card-act-btn") as HTMLElement | null;
      if (toolBtn) {
        e.preventDefault();
        e.stopPropagation();
        const raw = toolBtn.getAttribute("data-tool");
        let toolName = "";
        if (raw) {
          try { toolName = decodeURIComponent(raw); } catch { toolName = raw; }
        }
        if (toolName) void sendUserPrompt(`Exécute l'outil ${toolName}`);
        return;
      }
    };

    const handleGlobalOptionKeydown = (e: KeyboardEvent) => {
      if ((window as any).__aroChatInteractionsInstalled) return;
      if (e.key === "Enter") {
        const target = e.target as HTMLElement;
        if (target && target.classList.contains("form-text-input")) {
          e.preventDefault();
          const inputEl = target as HTMLInputElement;
          if (inputEl.value.trim()) {
            const val = inputEl.value.trim();
            inputEl.value = "";
            void sendUserPrompt(val);
          }
        }
      }
    };

    document.addEventListener("click", handleGlobalOptionClick, true);
    document.addEventListener("keydown", handleGlobalOptionKeydown, true);
    unlisteners.push(() => {
      document.removeEventListener("click", handleGlobalOptionClick, true);
      document.removeEventListener("keydown", handleGlobalOptionKeydown, true);
    });
    
    // Load profile, organization, performance history and user activity from localStorage
    loadProfileAndOrgData();
    void refreshPermissionProfiles();
    void refreshMemoryIndexStatus();
    
    if (typeof localStorage !== "undefined") {
      const savedActivity = localStorage.getItem("aro-user-activity");
      if (savedActivity) {
        try {
          userActivityLog = JSON.parse(savedActivity);
        } catch (e) {
          console.error("Failed to load user activity", e);
        }
      } else {
        const isCloudUser = Boolean(localStorage.getItem("aro_web_access_token"));
        userActivityLog = isCloudUser ? {} : populateMockActivity();
        localStorage.setItem("aro-user-activity", JSON.stringify(userActivityLog));
      }

      const saved = localStorage.getItem("aro-perf-history");
      if (saved) {
        try {
          performanceHistory = JSON.parse(saved);
          if (performanceHistory.length > 0) {
            const summary = summarizePerformance(performanceHistory);
            avgResponseTime = summary.averageResponseTime;
            totalTokensGenerated = summary.totalTokensGenerated;
            
            const lastRec = performanceHistory[0];
            lastResponseTime = lastRec.responseTime;
            lastTokenCount = lastRec.tokens;
            lastTokenSpeed = lastRec.speed;
          }
        } catch (e) {
          console.error("Failed to load performance history", e);
        }
      }
    }
    
    // System stats monitoring interval
    const timer = setInterval(() => {
      let cpuTarget = 0;
      let ramTarget = 0;
      let gpuTarget = 0;
      let vramTarget = 0;
      
      if (anyConversationSending) {
        cpuTarget = 45 + Math.random() * 30;
        gpuTarget = 30 + Math.random() * 35;
        vramTarget = 4.5 + Math.random() * 1.2;
      } else if (assistantSpeaking) {
        cpuTarget = 15 + Math.random() * 15;
        gpuTarget = 8 + Math.random() * 10;
        vramTarget = 2.8 + Math.random() * 0.3;
      } else {
        cpuTarget = 1.5 + Math.random() * 5.0;
        gpuTarget = 0.5 + Math.random() * 3.0;
        vramTarget = 2.1 + Math.random() * 0.15;
      }

      if (typeof window !== "undefined" && (window as any).performance?.memory) {
        const mem = (window as any).performance.memory;
        ramTarget = Number((mem.usedJSHeapSize / 1024 / 1024 / 1024).toFixed(2));
        monRamMax = Number((mem.jsHeapSizeLimit / 1024 / 1024 / 1024).toFixed(1));
      } else if (anyConversationSending) {
        ramTarget = 6.2 + Math.random() * 1.5;
      } else if (assistantSpeaking) {
        ramTarget = 4.6 + Math.random() * 0.4;
      } else {
        ramTarget = 4.1 + Math.random() * 0.2;
      }
      
      monCpu = Math.round(monCpu + (cpuTarget - monCpu) * 0.3);
      monRam = Number((monRam + (ramTarget - monRam) * 0.2).toFixed(2));
      monGpu = Math.round(monGpu + (gpuTarget - monGpu) * 0.3);
      monGpuVram = Number((monGpuVram + (vramTarget - monGpuVram) * 0.2).toFixed(2));
      
      cpuHistory = [...cpuHistory.slice(1), monCpu];
      ramHistory = [...ramHistory.slice(1), monRam];
      gpuHistory = [...gpuHistory.slice(1), monGpu];
    }, 1000);

    loadHooksFromLocalStorage();
    loadTasksFromLocalStorage();
    loadBootstrap();

    return () => {
      clearInterval(timer);
      for (const unlisten of unlisteners) {
        try { unlisten(); } catch (e) { console.error(e); }
      }
      // Nettoyage voix au démontage : micro, contextes audio, minuteurs et
      // boucle d'onde ne doivent pas survivre (HMR, navigation).
      try {
        if (animationFrameId) cancelAnimationFrame(animationFrameId);
      } catch { /* ignore */ }
      try {
        if (wakeWordTimer) clearInterval(wakeWordTimer);
      } catch { /* ignore */ }
      stopSpeaking();
      void stopRecordingCapture().catch(() => {});
      try {
        stopWakeWordListening();
      } catch { /* ignore */ }
      try {
        void chimeContext?.close()?.catch(() => {});
      } catch { /* ignore */ }
      chimeContext = null;
      if (previewFileObject?.url) {
        try {
          URL.revokeObjectURL(previewFileObject.url);
        } catch { /* ignore */ }
      }
    };
  });

  const formatShortcut = (shortcut: string) => formatShortcutModel(
    shortcut,
    typeof navigator !== "undefined" && navigator.userAgent.includes("Mac"),
  );

  function handleShortcutRecord(e: KeyboardEvent) {
    if (!recordingShortcutFor) return;
    e.preventDefault();
    e.stopPropagation();

    const finalShortcut = shortcutFromKeyboardEvent(e);
    if (finalShortcut) {
      updateShortcut(recordingShortcutFor, finalShortcut);
      recordingShortcutFor = null;
    }
  }

  function handleGlobalKeydown(e: KeyboardEvent) {
    if (recordingShortcutFor) {
      handleShortcutRecord(e);
      return;
    }

    const activeEl = document.activeElement;
    const isInputActive = activeEl && (
      activeEl.tagName === "INPUT" || 
      activeEl.tagName === "TEXTAREA" || 
      activeEl.getAttribute("contenteditable") === "true"
    );

    if (matchShortcutEvent(e, shortcuts.spotlight)) {
      e.preventDefault();
      toggleCommandPalette();
    } else if (matchShortcutEvent(e, shortcuts.voice)) {
      if (isInputActive && !hasModifiers(e)) return;
      e.preventDefault();
      toggleVoiceRecording();
    } else if (matchShortcutEvent(e, shortcuts.settings)) {
      if (isInputActive && !hasModifiers(e)) return;
      e.preventDefault();
      openSettings("general");
    } else if (matchShortcutEvent(e, shortcuts.newChat)) {
      if (isInputActive && !hasModifiers(e)) return;
      e.preventDefault();
      startConversation();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "p") {
      e.preventDefault();
      showQuickOpenModal = !showQuickOpenModal;
    } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "m") {
      e.preventDefault();
      if (activeConversation) {
        handleOpenMoveModal(activeConversation);
      }
    }
  }

  async function toggleVoiceRecording() {
    if (recording) {
      await stopRecording();
    } else {
      await startRecording();
    }
  }

  function executeCommandPaletteItem(item: any) {
    if (item.action) {
      item.action();
    }
  }

  function handleCommandPaletteKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (commandPaletteFilteredResults.length === 0) {
        commandPaletteSelectedIndex = 0;
        return;
      }
      commandPaletteSelectedIndex = (commandPaletteSelectedIndex + 1) % commandPaletteFilteredResults.length;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (commandPaletteFilteredResults.length === 0) {
        commandPaletteSelectedIndex = 0;
        return;
      }
      commandPaletteSelectedIndex = (commandPaletteSelectedIndex - 1 + commandPaletteFilteredResults.length) % commandPaletteFilteredResults.length;
    } else if (e.key === "Enter") {
      e.preventDefault();
      const selected = commandPaletteFilteredResults[commandPaletteSelectedIndex];
      if (selected) {
        selected.action();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      toggleCommandPalette();
    }
  }

  $: activeProject = activeConversation?.projectId ? projects.find((p) => p.id === activeConversation?.projectId) ?? null : null;

  $: commandPaletteFilteredResults = buildCommandPaletteResults({
    language: currentLanguage,
    theme: currentTheme,
    shortcuts,
    conversations,
    projects,
    folders,
    actions: {
      close: toggleCommandPalette,
      startConversation,
      openSettings: () => openSettings("general"),
      applyTheme: (theme) => {
        currentTheme = theme;
        applyTheme(theme);
      },
      toggleVoiceRecording,
      clearMemory,
      selectMode: (mode) => {
        activeMode = mode;
        if (activeConversation) activeConversation.mode = mode;
      },
      openConversation: (conversation) => openConversation(conversation as Conversation),
      createProject: handleOpenCreateProjectModal,
      createFolder: () => handleOpenCreateFolderModal(null),
      exportActiveMarkdown: () => {
        if (activeConversation) handleExportConversation(activeConversation, "markdown");
        else notifyNoActiveConversation();
      },
      exportActiveJson: () => {
        if (activeConversation) handleExportConversation(activeConversation, "json");
        else notifyNoActiveConversation();
      },
      openProject: (projectId: string) => openMostRecentConversation({ projectId }),
      openFolder: (folderId: string) => openMostRecentConversation({ folderId }),
    },
  }, commandPaletteSearch);

  // L'index de sélection ne doit jamais dépasser la liste filtrée
  // (filtre qui rétrécit, liste vide → NaN sinon).
  $: if (commandPaletteSelectedIndex >= commandPaletteFilteredResults.length) {
    commandPaletteSelectedIndex = 0;
  }

  function notifyNoActiveConversation() {
    addNotificationToast({
      type: "info",
      title: currentLanguage === "fr" ? "Aucune conversation ouverte" : "No open conversation",
      body: currentLanguage === "fr"
        ? "Ouvrez d'abord une conversation pour utiliser cette commande."
        : "Open a conversation first to use this command.",
    });
  }

  // Palette : ouvrir la conversation la plus récente d'un projet/dossier.
  // Sans conversation : toast d'info au lieu d'une fermeture silencieuse.
  function openMostRecentConversation(scope: { projectId?: string; folderId?: string }) {
    const match = conversations.find((c) => {
      if (scope.folderId) return c.folderId === scope.folderId;
      if (scope.projectId) return c.projectId === scope.projectId && !c.folderId;
      return false;
    });
    if (match) {
      void openConversation(match);
      return;
    }
    addNotificationToast({
      type: "info",
      title: currentLanguage === "fr" ? "Rien à ouvrir" : "Nothing to open",
      body: currentLanguage === "fr"
        ? "Aucune conversation dans cet espace pour l'instant."
        : "No conversations in this space yet.",
    });
  }

  async function loadBootstrap() {
    loading = true;
    errorMessage = "";
    
    let progressInterval: number | undefined;
    if (showSplash) {
      splashProgress = 0;
      progressInterval = window.setInterval(() => {
        if (splashProgress < 90) {
          splashProgress += Math.floor(Math.random() * 12) + 6;
          if (splashProgress > 90) splashProgress = 90;
        }
      }, 70);
    }

    try {
      const payload = await bootstrap();
      settings = payload.settings;
      settingsDraft = cloneSettings(payload.settings);
      updateSettingsWithActiveVoice();
      modelOptions = ensureCurrentModelOption(payload.models, payload.settings);
      modelProviders = payload.modelProviders ?? payload.settings.model.providers ?? [];
      conversations = payload.conversations;
      try {
        projects = await listProjects();
        folders = await listFolders();
      } catch { /* ignored */ }
      await refreshAgentRuns();
      runtime = payload.runtime;
      voiceModelStatus = payload.voiceModelStatus ?? null;
      if (voiceModelStatus && runtime) {
        runtime = {
          ...runtime,
          voiceReady: voiceModelStatus.voiceStatus.ready,
          voiceStatus: voiceModelStatus.voiceStatus,
        };
      }
      cloudSession = await getCloudSession();
      cloudAuthenticated = Boolean(payload.cloudAuthenticated || cloudSession);
      cloudSyncStatus = payload.syncStatus ?? { health: "offline-read-only", pendingEvents: 0, lastSyncedAt: null };
      apiBaseUrl = payload.apiBaseUrl ?? "";
      if (payload.preferences) {
        currentTheme = payload.preferences.theme === "dark" ? "dark" : "light";
        currentLanguage = payload.preferences.language === "en" ? "en" : "fr";
        wakeWordEnabled = payload.preferences.wakeWordEnabled;
        themeDraft = currentTheme;
        languageDraft = currentLanguage;
        wakeWordDraftEnabled = wakeWordEnabled;
      }
      hydrateCloudState(payload.clientState ?? {});
      installCloudStorageBridge();
      if (cloudAuthenticated) {
        try {
          cloudOrganizations = await listCloudOrganizations();
        } catch (error) {
          cloudOrganizations = cloudSession?.activeOrganization ? [cloudSession.activeOrganization] : [];
          collectionsSyncError = normalizeError(error);
        }
      } else {
        cloudOrganizations = [];
      }
      if (payload.currentUser) {
        userProfile = {
          name: payload.currentUser.name,
          email: payload.currentUser.email,
          roleTitle: payload.currentUser.roleTitle ?? "",
          avatarColor: payload.currentUser.avatarColor ?? "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)",
        };
      }
      if (payload.activeOrganization) {
        activeOrg = {
          name: payload.activeOrganization.name,
          domain: payload.activeOrganization.domain ?? "",
          description: payload.activeOrganization.description ?? "",
        };
      }
      if (payload.memberships?.length && payload.currentUser) {
        orgMembers = payload.memberships.map((membership) => {
          const existing = orgMembers.find((m) => m.userId === membership.userId);
          return {
            id: membership.id,
            cloudId: membership.id,
            userId: membership.userId,
            name: membership.userId === payload.currentUser?.id
              ? payload.currentUser.name
              : (existing?.name ?? membership.userId),
            email: membership.userId === payload.currentUser?.id
              ? payload.currentUser.email
              : (existing?.email ?? ""),
            role: (membership.role === "owner" || membership.role === "admin") ? "admin" : "member",
            status: membership.status === "active" ? "active" : "invited",
          };
        });
      }
      if (cloudAuthenticated) {
        await loadCloudCollections();
        try {
          apiKeys = (await listCloudApiKeys()).map(mapApiKeyRecord);
        } catch (error) {
          collectionsSyncError = normalizeError(error);
        }
        try {
          const members = await listCloudMembers();
          const canManageInvitations = cloudSession?.memberships.some((membership) =>
            membership.userId === cloudSession?.user.id
            && membership.organizationId === cloudSession?.activeOrganization.id
            && membership.status === "active"
            && (membership.role === "owner" || membership.role === "admin"),
          ) ?? false;
          let invitations: OrganizationInvitation[] = [];
          if (canManageInvitations) {
            try {
              invitations = await listAllCloudInvitations(false);
            } catch (error) {
              collectionsSyncError = normalizeError(error);
            }
          }
          orgMembers = mergeOrganizationInvitations(members, invitations);
          saveMembers();
        } catch (error) {
          collectionsSyncError = normalizeError(error);
        }
      }
      if (!cloudAuthenticated && !isSpotlightMode) {
        showCloudAuthPanel = true;
      }
      activeConversation = conversations[0] ?? null;
      activeMode = activeConversation?.mode ?? "chat";
      messages = activeConversation ? await listMessages(activeConversation.id) : [];
      populateLoadedSteps(messages);
      void loadWorkspaceMentions(activeConversation?.id ?? null);
      await scrollToBottom(true);
      setTimeout(() => scrollToBottom(true), 50);
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      loading = false;
      if (progressInterval) {
        clearInterval(progressInterval);
      }
      if (showSplash) {
        splashProgress = 100;
        setTimeout(() => {
          showSplash = false;
        }, 500);
      }
    }
  }

  function toggleCloudAuthMode() {
    cloudAuthMode = cloudAuthMode === "login" ? "register" : "login";
    cloudAuthError = "";
    cloudAuthSuccessMessage = "";
    cloudAuthDevTokenUrl = "";
    showPassword = false;
  }

  async function submitCloudAuth() {
    if (cloudAuthBusy) return;
    cloudAuthError = "";
    cloudAuthSuccessMessage = "";
    cloudAuthDevTokenUrl = "";
    cloudAuthBusy = true;
    showPassword = false;
    try {
      if (cloudAuthMode === "forgot-password") {
        const res = await requestPasswordReset(cloudAuthEmail.trim());
        cloudAuthSuccessMessage = res.message;
        cloudAuthDevTokenUrl = res.devTokenUrl || "";
        addNotificationToast({
          type: "info",
          title: "E-mail de réinitialisation",
          body: res.message,
        });
        cloudAuthBusy = false;
        return;
      }
      if (cloudAuthMode === "invitation") {
        cloudSession = await acceptCloudInvitation(
          cloudInvitationToken,
          cloudAuthEmail.trim(),
          cloudAuthPassword,
        );
        cloudInvitationToken = "";
      } else if (cloudAuthMode === "login") {
        cloudSession = await loginCloud({
          email: cloudAuthEmail.trim(),
          password: cloudAuthPassword,
        });
      } else {
        cloudSession = await registerCloud({
          email: cloudAuthEmail.trim(),
          password: cloudAuthPassword,
          name: cloudAuthName.trim() || cloudAuthEmail.trim(),
          organizationName: cloudAuthOrgName.trim() || "ARO Workspace",
        });
      }
      cloudAuthenticated = true;
      showCloudAuthPanel = false;
      leaveSettingsView();
      cloudAuthPassword = "";
      clearOrganizationScopedState();
      await loadBootstrap();
    } catch (error) {
      cloudAuthError = normalizeError(error);
    } finally {
      cloudAuthBusy = false;
    }
  }

  async function disconnectCloud() {
    cloudAuthError = "";
    let logoutError = "";
    try {
      await logoutCloud();
    } catch (error) {
      logoutError = normalizeError(error);
    }
    cloudSession = null;
    cloudAuthenticated = false;
    cloudOrganizations = [];
    cloudSyncStatus = { health: "offline-read-only", pendingEvents: 0, lastSyncedAt: null };
    showCloudAuthPanel = false;
    clearOrganizationScopedState();
    await loadBootstrap();
    cloudAuthError = logoutError;
  }

  async function switchOrganizationFromUi(organizationId: string) {
    if (!organizationId || organizationId === cloudSession?.activeOrganization.id || cloudOrganizationBusy) return;
    cloudOrganizationBusy = true;
    collectionsSyncError = "";
    try {
      cloudSession = await switchCloudOrganization(organizationId);
      clearOrganizationScopedState();
      await loadBootstrap();
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    } finally {
      cloudOrganizationBusy = false;
    }
  }

  async function createOrganizationFromUi() {
    const name = newOrganizationName.trim();
    if (!name || cloudOrganizationBusy) return;
    cloudOrganizationBusy = true;
    collectionsSyncError = "";
    try {
      cloudSession = await createCloudOrganization({ name });
      newOrganizationName = "";
      clearOrganizationScopedState();
      await loadBootstrap();
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    } finally {
      cloudOrganizationBusy = false;
    }
  }

  function hydrateCloudState(clientState: Record<string, unknown>) {
    if (typeof localStorage === "undefined") return;
    hydratingCloudState = true;
    try {
      const cloudKeys = new Set(Object.keys(clientState).filter((key) => key.startsWith("aro-")));
      if (cloudAuthenticated) {
        for (const key of cloudOwnedStateKeys()) {
          if (!cloudKeys.has(key)) {
            localStorage.removeItem(key);
          }
        }
      }
      for (const [key, value] of Object.entries(clientState)) {
        if (!key.startsWith("aro-")) continue;
        if (isSensitiveCloudStateKey(key)) {
          hydrateSensitiveCloudState(key, value);
          localStorage.removeItem(key);
          continue;
        }
        if (typeof value === "string") {
          localStorage.setItem(key, value);
        } else {
          localStorage.setItem(key, JSON.stringify(value));
        }
      }
      loadProfileAndOrgData();
      void refreshPermissionProfiles();
      loadHooksFromLocalStorage();
      loadTasksFromLocalStorage();
      purgeAroLocalStorage();
    } finally {
      hydratingCloudState = false;
    }
  }

  function cloudOwnedStateKeys(): string[] {
    return [
      "aro-user-profile",
      "aro-active-org",
      "aro-org-members",
      "aro-org-teams",
      "aro-api-keys",
      "aro-perm-read-file",
      "aro-perm-write-file",
      "aro-perm-execute-commands",
      "aro-perm-command-approval",
      "aro-perm-network-access",
      "aro-perm-allowed-domains",
      "aro-perm-allowed-paths",
      "aro-custom-system-prompts-enabled",
      "aro-custom-system-prompts",
      "aro-instruction-defaults-version",
      "aro-agi-identity",
      "aro-agi-rules",
      "aro-agi-formatting",
      "aro-custom-instructions-enabled",
      "aro-selected-personality-id",
      "aro-custom-personalities",
      "aro-conversation-personalities",
      "aro-custom-voices",
      "aro-selected-voice-id",
      "aro-wakeword-enabled",
      "aro-user-skills",
      "aro-user-skill-groups",
      "aro-user-plugins",
      "aro-mcp-servers",
      "aro-hooks",
      "aro-scheduled-tasks",
      "aro-theme",
      "aro-language",
      "aro-user-activity",
      "aro-perf-history",
    ];
  }

  function shouldRetainLocalStorageKey(key: string): boolean {
    return !key.startsWith("aro-");
  }

  function purgeAroLocalStorage() {
    if (typeof localStorage === "undefined") return;
    for (let index = localStorage.length - 1; index >= 0; index -= 1) {
      const key = localStorage.key(index);
      if (key && !shouldRetainLocalStorageKey(key)) {
        localStorage.removeItem(key);
      }
    }
  }

  function hydrateSensitiveCloudState(key: string, value: unknown) {
    const parsed = typeof value === "string" ? parseStorageValue(value) : value;
    if (key === "aro-user-plugins") {
      userPlugins = cloudArray<UserPlugin>(parsed, userPlugins).map(pluginSafeForLocalStorage);
    } else if (key === "aro-mcp-servers") {
      mcpServers = cloudArray<McpServer>(parsed, mcpServers).map(mcpSafeForLocalStorage);
    } else if (key === "aro-hooks") {
      hooks = cloudArray<AroHook>(parsed, hooks).map(hookSafeForLocalStorage);
    } else if (key === "aro-api-keys") {
      apiKeys = cloudArray<ApiKeyRecord>(parsed, []);
    }
  }

  function cloudArray<T>(value: unknown, fallback: T[]): T[] {
    return Array.isArray(value) ? value as T[] : fallback;
  }

  function isSensitiveCloudStateKey(key: string): boolean {
    return key === "aro-api-keys"
      || key === "aro-user-plugins"
      || key === "aro-mcp-servers"
      || key === "aro-hooks"
      || key.includes("token")
      || key.includes("secret")
      || key.includes("key");
  }

  function installCloudStorageBridge() {
    if (cloudStorageBridgeInstalled || typeof localStorage === "undefined" || typeof Storage === "undefined") return;
    const prototype = Storage.prototype;
    const originalSetItem = prototype.setItem;
    const originalRemoveItem = prototype.removeItem;
    prototype.setItem = function(key: string, value: string) {
      if (hydratingCloudState || shouldRetainLocalStorageKey(key)) {
        originalSetItem.call(this, key, value);
      } else {
        originalRemoveItem.call(this, key);
      }
      if (!hydratingCloudState && cloudAuthenticated && key.startsWith("aro-")) {
        void setCloudState(key, parseStorageValue(value)).catch((error) => {
          console.warn("Cloud state sync failed", key, error);
        });
      }
    };
    prototype.removeItem = function(key: string) {
      originalRemoveItem.call(this, key);
      if (!hydratingCloudState && cloudAuthenticated && key.startsWith("aro-")) {
        void deleteCloudState(key).catch((error) => {
          console.warn("Cloud state delete failed", key, error);
        });
      }
    };
    cloudStorageBridgeInstalled = true;
  }

  function parseStorageValue(value: string): unknown {
    try {
      return JSON.parse(value);
    } catch {
      return value;
    }
  }

  type CloudRow = Record<string, any>;

  function canWriteCloudCollections() {
    return cloudAuthenticated && cloudSyncStatus.health === "online";
  }

  function cloudWriteBlockedMessage(action = "modifier cet espace") {
    if (cloudSyncStatus.health === "session-expired") {
      return currentLanguage === "fr"
        ? `Session expiree: reconnectez-vous pour ${action}.`
        : `Session expired: sign in again to ${action}.`;
    }
    if (cloudSyncStatus.health === "offline-read-only") {
      return currentLanguage === "fr"
        ? `Mode lecture seule hors ligne: impossible de ${action}.`
        : `Offline read-only mode: cannot ${action}.`;
    }
    if (cloudSyncStatus.health === "sync-pending") {
      return currentLanguage === "fr"
        ? `Synchronisation en attente: terminez la sync avant de ${action}.`
        : `Sync pending: finish syncing before you ${action}.`;
    }
    return currentLanguage === "fr"
      ? `ARO Cloud n'est pas prêt pour ${action}.`
      : `ARO Cloud is not ready to ${action}.`;
  }

  function ensureCloudWriteAllowed(action = "modifier cet espace"): boolean {
    if (!cloudAuthenticated || canWriteCloudCollections()) return true;
    collectionsSyncError = cloudWriteBlockedMessage(action);
    if (cloudSyncStatus.health === "session-expired") {
      showCloudAuthPanel = true;
    }
    return false;
  }

  function cloudWriteDisabledTitle(action = "modifier cet espace") {
    return cloudWriteLocked ? cloudWriteBlockedMessage(action) : undefined;
  }

  function clearOrganizationScopedState() {
    activeConversation = null;
    messages = [];
    conversations = [];
    memoriesList = [];
    userSkills = [];
    userSkillGroups = [
      {
        id: "all",
        name: "Tous les Skills",
        description: "Toutes vos competences configurees",
        createdAt: new Date().toISOString(),
      },
    ];
    mcpServers = [];
    hooks = [];
    scheduledTasks = [];
    orgTeams = [];
    orgMembers = [];
    apiKeys = [];
    permissionProfiles = [];
    activePermissionProfileId = "";
    permissionsStatus = "";
    permissionsError = "";
    customPersonalities = [];
    customVoices = [];
  }

  function asCloudRows(value: unknown): CloudRow[] {
    return Array.isArray(value) ? value.filter((item): item is CloudRow => item && typeof item === "object") : [];
  }

  function rowValue<T>(row: CloudRow, camel: string, snake: string, fallback: T): T {
    return (row[camel] ?? row[snake] ?? fallback) as T;
  }

  function rowClientId(row: CloudRow): string {
    return String(row.clientId ?? row.client_id ?? row.config?.clientId ?? row.id ?? crypto.randomUUID());
  }

  function rowDate(row: CloudRow, camel: string, snake: string): string {
    const raw = rowValue<string | null>(row, camel, snake, null);
    return raw ? String(raw) : new Date().toISOString();
  }

  function mapSkillGroupRow(row: CloudRow): UserSkillGroup {
    return {
      id: rowClientId(row),
      cloudId: String(row.id),
      name: String(row.name ?? ""),
      description: String(row.description ?? ""),
      createdAt: rowDate(row, "createdAt", "created_at"),
    };
  }


  function mapSkillRow(row: CloudRow): UserSkill {
    const triggers = Array.isArray(row.triggers) ? row.triggers.join(", ") : String(row.triggers ?? "");
    return {
      id: rowClientId(row),
      cloudId: String(row.id),
      name: String(row.name ?? ""),
      description: String(row.description ?? ""),
      icon: String(row.icon ?? "🧩"),
      category: String(row.category ?? "Général"),
      groupId: String(row.clientGroupId ?? row.client_group_id ?? row.groupId ?? row.group_id ?? "all"),
      triggers,
      type: (row.kind ?? row.type ?? "system_prompt") as UserSkill["type"],
      content: String(row.content ?? ""),
      enabled: Boolean(row.enabled ?? true),
      createdAt: rowDate(row, "createdAt", "created_at"),
    };
  }


  function mapPluginRow(row: CloudRow, presets: UserPlugin[]): UserPlugin {
    const config = row.config && typeof row.config === "object" ? row.config : {};
    const clientId = rowClientId(row);
    const preset = presets.find((plugin) => plugin.id === clientId || plugin.name === row.name);
    return {
      ...(preset ?? {
        id: clientId,
        name: String(row.name ?? "Plugin"),
        description: String(row.description ?? ""),
        icon: "🔌",
        category: String(row.category ?? "Autre"),
        status: "setup" as const,
        authType: "none" as const,
        fields: {},
        configFields: [],
        enabled: true,
        createdAt: rowDate(row, "createdAt", "created_at"),
      }),
      id: clientId,
      cloudId: String(row.id),
      name: String(row.name ?? preset?.name ?? "Plugin"),
      description: String(row.description ?? preset?.description ?? ""),
      category: String(row.category ?? preset?.category ?? "Autre"),
      status: (row.status ?? preset?.status ?? "setup") as UserPlugin["status"],
      authType: (row.authType ?? row.auth_type ?? preset?.authType ?? "none") as UserPlugin["authType"],
      enabled: Boolean(row.enabled ?? preset?.enabled ?? true),
      fields: { ...(preset?.fields ?? {}), ...(config.fields ?? {}) },
      configFields: config.configFields ?? preset?.configFields ?? [],
      profileName: config.profileName ?? preset?.profileName,
      profileAvatar: config.profileAvatar ?? preset?.profileAvatar,
      profileDetails: config.profileDetails ?? preset?.profileDetails,
      subServices: config.subServices ?? preset?.subServices,
      createdAt: rowDate(row, "createdAt", "created_at"),
    };
  }


  function mapMcpRow(row: CloudRow): McpServer {
    return {
      id: rowClientId(row),
      cloudId: String(row.id),
      name: String(row.name ?? ""),
      type: (row.transport ?? row.type ?? "stdio") as McpServer["type"],
      command: row.command ?? undefined,
      args: Array.isArray(row.args) ? row.args : [],
      env: {},
      url: row.url ?? undefined,
      enabled: Boolean(row.enabled ?? true),
      status: (row.status ?? "disconnected") as McpServer["status"],
      tools: Array.isArray(row.tools) ? row.tools : [],
      resources: Array.isArray(row.resources) ? row.resources : [],
    };
  }

  function mcpPayload(server: McpServer) {
    return {
      clientId: server.id,
      name: server.name,
      transport: server.type,
      command: server.command,
      args: server.args ?? [],
      env: server.env ?? {},
      url: server.url,
      enabled: server.enabled,
      status: server.status,
      tools: server.tools ?? [],
      resources: server.resources ?? [],
    };
  }

  function mapHookRow(row: CloudRow): AroHook {
    return {
      id: rowClientId(row),
      cloudId: String(row.id),
      name: String(row.name ?? ""),
      url: String(row.url ?? ""),
      events: Array.isArray(row.events) ? row.events : [],
      enabled: Boolean(row.enabled ?? true),
      createdAt: rowDate(row, "createdAt", "created_at"),
      lastTriggered: rowValue<string | undefined>(row, "lastTriggeredAt", "last_triggered_at", undefined),
      status: (row.status ?? "idle") as AroHook["status"],
    };
  }

  function hookPayload(hook: AroHook) {
    return {
      clientId: hook.id,
      name: hook.name,
      url: hook.url,
      secret: hook.secret,
      events: hook.events,
      enabled: hook.enabled,
      status: hook.status ?? "idle",
      lastTriggered: hook.lastTriggered,
    };
  }

  function mapScheduledTaskRow(row: CloudRow): ScheduledTask {
    return {
      id: rowClientId(row),
      cloudId: String(row.id),
      name: String(row.name ?? ""),
      prompt: String(row.prompt ?? ""),
      type: (row.scheduleType ?? row.schedule_type ?? row.type ?? "timer") as ScheduledTask["type"],
      durationMinutes: row.durationMinutes ?? row.duration_minutes ?? undefined,
      cronExpression: row.cronExpression ?? row.cron_expression ?? undefined,
      enabled: Boolean(row.enabled ?? true),
      createdAt: rowDate(row, "createdAt", "created_at"),
      lastRun: row.lastRunAt ?? row.last_run_at ?? undefined,
      status: (row.status ?? "idle") as ScheduledTask["status"],
    };
  }

  function scheduledTaskPayload(task: ScheduledTask) {
    return {
      clientId: task.id,
      name: task.name,
      prompt: task.prompt,
      scheduleType: task.type,
      durationMinutes: task.durationMinutes,
      cronExpression: task.cronExpression,
      enabled: task.enabled,
      status: task.status,
      lastRun: task.lastRun,
    };
  }

  function mapTeamRow(row: CloudRow): OrgTeam {
    return {
      id: rowClientId(row),
      cloudId: String(row.id),
      name: String(row.name ?? ""),
      description: String(row.description ?? ""),
      memberIds: Array.isArray(row.memberIds)
        ? row.memberIds
        : Array.isArray(row.member_ids)
          ? row.member_ids
          : [],
    };
  }

  function mapPersonalityRow(row: CloudRow): Personality {
    return {
      id: rowClientId(row),
      cloudId: String(row.id),
      name: String(row.name ?? ""),
      description: String(row.description ?? ""),
      prompt: String(row.prompt ?? ""),
      icon: String(row.icon ?? "bot"),
      avatarColor: String(row.avatarColor ?? row.avatar_color ?? "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)"),
      temperature: Number(row.temperature ?? 0.7),
      voiceId: row.voiceId ?? row.voice_id ?? null,
      isDefault: Boolean(row.isDefault ?? row.is_default ?? false),
    };
  }

  function personalityPayload(personality: Personality) {
    return {
      clientId: personality.id,
      name: personality.name,
      description: personality.description,
      prompt: personality.prompt,
      icon: personality.icon,
      avatarColor: personality.avatarColor,
      temperature: personality.temperature,
      voiceId: personality.voiceId,
      isDefault: personality.isDefault ?? false,
    };
  }

  function mapVoiceRow(row: CloudRow): VoiceProfile {
    return {
      id: rowClientId(row),
      cloudId: String(row.id),
      name: String(row.name ?? ""),
      description: String(row.description ?? ""),
      path: String(row.path ?? ""),
      speakerId: row.speakerId ?? row.speaker_id ?? null,
      language: String(row.language ?? "fr"),
      avatarColor: String(row.avatarColor ?? row.avatar_color ?? "linear-gradient(135deg, #3B8BDB 0%, #0071e3 100%)"),
      isDefault: Boolean(row.isDefault ?? row.is_default ?? false),
    };
  }

  function voicePayload(voice: VoiceProfile) {
    return {
      clientId: voice.id,
      name: voice.name,
      description: voice.description,
      path: voice.path,
      speakerId: voice.speakerId,
      language: voice.language,
      avatarColor: voice.avatarColor,
      isDefault: voice.isDefault ?? false,
    };
  }

  function systemPromptPayload(mode: string) {
    return {
      clientId: mode,
      mode,
      identity: agiIdentity[mode] ?? "",
      rules: agiRules[mode] ?? "",
      formatting: agiFormatting[mode] ?? "",
      compiledPrompt: customSystemPrompts[mode] ?? "",
      enabled: customSystemPromptsEnabled,
    };
  }

  async function upsertCloudItem<T extends { id: string; cloudId?: string }>(
    collection: string,
    item: T,
    payload: unknown,
  ): Promise<T> {
    if (!canWriteCloudCollections()) {
      if (cloudAuthenticated) {
        throw new Error(cloudWriteBlockedMessage(`synchroniser ${collection}`));
      }
      return item;
    }
    const saved = item.cloudId
      ? await updateCloudCollectionItem<CloudRow>(collection, item.cloudId, payload)
      : await createCloudCollectionItem<CloudRow>(collection, payload);
    return { ...item, cloudId: String(saved.id ?? item.cloudId ?? item.id) };
  }

  async function deleteCloudItem(collection: string, item: { cloudId?: string }) {
    if (!canWriteCloudCollections()) {
      if (cloudAuthenticated) {
        throw new Error(cloudWriteBlockedMessage(`supprimer dans ${collection}`));
      }
      return;
    }
    if (item.cloudId) {
      await deleteCloudCollectionItem(collection, item.cloudId);
    }
  }

  async function refreshAgentRuns(focusRunId: string | null = null) {
    agentRunsBusy = true;
    try {
      const [runs, lanes, snapshot] = await Promise.all([
        listAgentRuns(),
        listAgentLanes(),
        getAgentOrchestratorSnapshot(),
      ]);
      
      // Check for newly completed agent runs to notify user
      for (const run of runs) {
        const prev = agentRuns.find((r) => r.id === run.id);
        if (prev && prev.status !== "completed" && run.status === "completed") {
          const title = `✓ Agent ARO Terminé`;
          const body = run.goal || `L'agent a terminé d'exécuter la tâche demandée.`;
          sendDesktopNotification(title, body, run.conversationId ?? undefined);
          addNotificationToast({
            type: "agent-completed",
            title,
            body,
            conversationId: run.conversationId ?? undefined,
          });
        }
      }

      agentRuns = runs;
      agentLanes = lanes;
      orchestratorSnapshot = snapshot;
      if (focusRunId) {
        selectedAgentRunView = await getAgentRun(focusRunId);
      } else if (selectedAgentRunView) {
        const currentSelectedId = selectedAgentRunView.run.id;
        const stillVisible = runs.some((r) => r.id === currentSelectedId);
        selectedAgentRunView = stillVisible ? await getAgentRun(currentSelectedId) : null;
      }
    } catch (error) {
      console.error("Failed to load agent runs", error);
    } finally {
      agentRunsBusy = false;
    }
  }

  async function openAgentRun(runId: string) {
    agentActionBusy = runId;
    try {
      selectedAgentRunView = await getAgentRun(runId);
      showContextPanel = true;
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      agentActionBusy = null;
    }
  }

  async function setAgentRunAction(runId: string, action: "pause" | "resume" | "cancel") {
    agentActionBusy = runId;
    try {
      const updated = action === "pause"
        ? await pauseAgentRun(runId)
        : action === "resume"
          ? await resumeAgentRun(runId)
          : await cancelAgentRun(runId);
      agentRuns = agentRuns.map((run) => run.id === updated.id ? updated : run);
      if (selectedAgentRunView?.run.id === updated.id) {
        selectedAgentRunView = await getAgentRun(updated.id);
      }
      await refreshAgentRuns(updated.id);
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      agentActionBusy = null;
    }
  }

  async function setAgentLaneAction(laneId: string, action: "pause" | "resume") {
    agentActionBusy = laneId;
    try {
      const updated = action === "pause"
        ? await pauseAgentLane(laneId)
        : await resumeAgentLane(laneId);
      agentLanes = agentLanes.map((view) => view.lane.id === laneId ? updated : view);
      await refreshAgentRuns(selectedAgentRunView?.run.id ?? null);
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      agentActionBusy = null;
    }
  }

  async function updateAgentLanePriority(laneId: string, priority: AgentRunPriority) {
    agentActionBusy = laneId;
    try {
      const updated = await setAgentLanePriority(laneId, priority);
      agentLanes = agentLanes.map((view) => view.lane.id === laneId ? updated : view);
      await refreshAgentRuns(selectedAgentRunView?.run.id ?? null);
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      agentActionBusy = null;
    }
  }

  $: chatArtifacts = extractArtifactsFromMessages(messages, activeConversation?.id ?? null);
  $: agentArtifacts = selectedAgentRunView?.artifacts ?? [];
  $: contextArtifacts = mergeArtifacts(agentArtifacts, chatArtifacts);
  $: contextSources = selectedAgentRunView?.contextPack?.sources ?? [];
  $: memoryContextSources = contextSources.filter((source) => source.kind === "memory");
  $: visibleContextSources = contextSources.filter((source) => source.kind !== "memory");
  $: agentInboxTotals = summarizeAgentInbox(agentRuns, orchestratorSnapshot);
  $: agentInboxLanes = buildAgentInboxLanes(agentLanes, agentRuns);

  function toggleContextSection(section: ContextSectionKey) {
    collapsedContextSections = {
      ...collapsedContextSections,
      [section]: !collapsedContextSections[section],
    };
  }

  function toggleAgentLane(laneId: string) {
    collapsedAgentLaneIds = {
      ...collapsedAgentLaneIds,
      [laneId]: !(collapsedAgentLaneIds[laneId] ?? true),
    };
  }

  function dedupeAgentRuns(runs: AgentRun[]): AgentRun[] {
    const byId = new Map<string, AgentRun>();
    for (const run of runs) {
      const existing = byId.get(run.id);
      if (!existing || existing.updatedAt < run.updatedAt) {
        byId.set(run.id, run);
      }
    }
    return Array.from(byId.values()).sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
  }

  function isActiveAgentRun(run: AgentRun): boolean {
    return run.status === "running" || run.status === "queued" || run.status === "waiting" || run.status === "paused";
  }

  function buildAgentInboxLanes(lanes: AgentLaneView[], runs: AgentRun[]): AgentInboxLane[] {
    const activeId = activeConversation?.id;
    if (!activeId) return [];

    const convRuns = dedupeAgentRuns(runs.filter((r) => r.conversationId === activeId));
    const knownLaneIds = new Set(lanes.map((l) => l.lane?.id).filter(Boolean));

    const runsByLane = new Map<string, AgentRun[]>();
    for (const run of convRuns) {
      if (run.laneId && knownLaneIds.has(run.laneId)) {
        const list = runsByLane.get(run.laneId) ?? [];
        list.push(run);
        runsByLane.set(run.laneId, list);
      }
    }

    const matchedLanes: AgentInboxLane[] = lanes
      .filter((laneView) => laneView.lane.conversationId === activeId)
      .map((laneView) => {
        const laneRunsAll = dedupeAgentRuns([
          ...laneView.latestRuns.filter((r) => r.conversationId === activeId),
          ...(runsByLane.get(laneView.lane.id) ?? []),
        ]);

        const activeRuns = laneRunsAll.filter(isActiveAgentRun);
        const inactiveRuns = laneRunsAll.filter((r) => !isActiveAgentRun(r));

        const laneRuns = [
          ...activeRuns,
          ...(inactiveRuns.length > 0 ? [inactiveRuns[0]] : []),
        ];

        const activeRunCount = activeRuns.length;
        const collapsed = collapsedAgentLaneIds[laneView.lane.id] ?? activeRunCount === 0;
        const visibleRuns = collapsed ? laneRuns.slice(0, 1) : laneRuns;

        return {
          ...laneView,
          latestRuns: laneRuns,
          collapsed,
          visibleRuns,
          totalRuns: laneRuns.length,
          activeRunCount,
        };
      });

    // Virtual default lane fallback for unassigned runs
    const unassignedRuns = convRuns.filter((r) => !r.laneId || !knownLaneIds.has(r.laneId));
    if (unassignedRuns.length > 0) {
      const activeRunCount = unassignedRuns.filter(isActiveAgentRun).length;
      const collapsed = collapsedAgentLaneIds["default-virtual"] ?? false;
      matchedLanes.unshift({
        lane: {
          id: "default-virtual",
          conversationId: activeId,
          title: currentLanguage === "fr" ? "Voie principale / Tâches actives" : "Main Lane / Active Tasks",
          status: "active",
          priority: "normal",
          maxConcurrentRuns: 2,
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
        },

        queuedCount: unassignedRuns.filter((r) => r.status === "queued").length,
        runningCount: unassignedRuns.filter((r) => r.status === "running").length,
        waitingCount: unassignedRuns.filter((r) => r.status === "waiting" || r.status === "paused").length,
        latestRuns: unassignedRuns,
        collapsed,
        visibleRuns: unassignedRuns,
        totalRuns: unassignedRuns.length,
        activeRunCount,
      });
    }

    return matchedLanes.sort((left, right) => {
      const leftActive = left.activeRunCount + (left.lane.status === "active" ? 1 : 0);
      const rightActive = right.activeRunCount + (right.lane.status === "active" ? 1 : 0);
      return rightActive - leftActive || right.lane.updatedAt.localeCompare(left.lane.updatedAt);
    });
  }

  function summarizeAgentInbox(runs: AgentRun[], snapshot: AgentOrchestratorSnapshot | null) {
    const activeId = activeConversation?.id;
    if (!activeId) {
      return { running: 0, queued: 0, waiting: 0, done: 0, failed: 0, total: 0 };
    }
    const convRuns = runs.filter((run) => run.conversationId === activeId);
    const uniqueRuns = Array.from(new Map(convRuns.map((r) => [r.id, r])).values());

    if (uniqueRuns.length === 0 && snapshot) {
      const running = Math.max(0, snapshot.runningCount ?? 0);
      const queued = Math.max(0, snapshot.queuedCount ?? 0);
      return {
        running,
        queued,
        waiting: 0,
        done: 0,
        failed: 0,
        total: Math.max(0, running + queued),
      };
    }

    return {
      running: Math.max(0, uniqueRuns.filter((r) => r.status === "running").length),
      queued: Math.max(0, uniqueRuns.filter((r) => r.status === "queued").length),
      waiting: Math.max(0, uniqueRuns.filter((r) => r.status === "waiting" || r.status === "paused").length),
      done: Math.max(0, uniqueRuns.filter((r) => r.status === "completed").length),
      failed: Math.max(0, uniqueRuns.filter((r) => r.status === "failed" || r.status === "cancelled").length),
      total: uniqueRuns.length,
    };
  }


  function agentStatusLabel(status: AgentRun["status"] | AgentLaneView["lane"]["status"]): string {
    const labels: Record<string, string> = currentLanguage === "fr"
      ? {
          active: "Actif",
          paused: "En pause",
          queued: "En file",
          running: "En cours",
          waiting: "En attente",
          completed: "Termine",
          failed: "Echec",
          cancelled: "Annule",
        }
      : {
          active: "Active",
          paused: "Paused",
          queued: "Queued",
          running: "Running",
          waiting: "Waiting",
          completed: "Done",
          failed: "Failed",
          cancelled: "Cancelled",
        };
    return labels[status] ?? status;
  }

  function agentPriorityLabel(priority: AgentRunPriority): string {
    if (currentLanguage !== "fr") return priority;
    return {
      low: "basse",
      normal: "normale",
      high: "haute",
      critical: "critique",
    }[priority];
  }

  function agentStatusTone(status: string): string {
    if (status === "running" || status === "active") return "active";
    if (status === "queued" || status === "waiting") return "waiting";
    if (status === "paused") return "paused";
    if (status === "failed" || status === "cancelled") return "failed";
    return "done";
  }

  function agentLaneSummary(lane: AgentInboxLane): string {
    const parts = [
      `${lane.runningCount} ${currentLanguage === "fr" ? "en cours" : "running"}`,
      `${lane.queuedCount} ${currentLanguage === "fr" ? "en file" : "queued"}`,
    ];
    if (lane.waitingCount > 0) {
      parts.push(`${lane.waitingCount} ${currentLanguage === "fr" ? "en attente" : "waiting"}`);
    }
    if (lane.totalRuns > 0) {
      parts.push(`${lane.totalRuns} ${currentLanguage === "fr" ? "runs" : "runs"}`);
    }
    return parts.join(" · ");
  }

  function compactAgentGoal(goal: string, maxChars = 58): string {
    const normalized = goal.split(/\s+/).join(" ").trim();
    if (normalized.length <= maxChars) return normalized;
    return `${normalized.slice(0, Math.max(0, maxChars - 1)).trim()}...`;
  }

  function agentStepLabel(kind: string): string {
    const labels: Record<string, string> = currentLanguage === "fr"
      ? {
          "run-started": "Demarrage",
          "context-built": "Contexte",
          model: "Modele",
          tool: "Outil",
          checkpoint: "Sauvegarde",
          final: "Reponse",
          error: "Erreur",
        }
      : {
          "run-started": "Started",
          "context-built": "Context",
          model: "Model",
          tool: "Tool",
          checkpoint: "Checkpoint",
          final: "Answer",
          error: "Error",
        };
    return labels[kind] ?? kind;
  }

  async function startAgentFromComposer() {
    const goal = input.trim() || activeConversation?.title || "Continue current conversation";
    if (!goal || isConversationSending(activeConversation?.id ?? null)) return;
    agentActionBusy = "new";
    try {
      let targetConversation = activeConversation;
      if (!targetConversation) {
        targetConversation = await createAndActivateConversation(goal, activeMode);
      }
      let systemPrompt = null;
      let modelId = settings?.model.activeModelRef?.modelId ?? null;
      let provider = settings?.model.activeModelRef?.providerId ?? null;
      let customPermissionId = null;

      if (targetConversation) {
        const customAgentId = conversationCustomAgents[targetConversation.id];
        if (customAgentId) {
          const matchedAgent = customAgentsList.find(a => a.id === customAgentId);
          if (matchedAgent) {
            systemPrompt = matchedAgent.systemPrompt;
            if (matchedAgent.modelId) modelId = matchedAgent.modelId;
            if (matchedAgent.modelProviderId) provider = matchedAgent.modelProviderId;
            
            const baseProfileId = matchedAgent.permissionProfileId || activePermissionProfileId;
            if (baseProfileId) {
              const baseProfile = permissionProfiles.find(p => p.id === baseProfileId);
              if (baseProfile) {
                const overrideMode = matchedAgent.commandApproval;
                if (overrideMode && baseProfile.commandApproval !== overrideMode) {
                  const overrideName = `${baseProfile.name} (${overrideMode === 'always' ? 'Always ask' : overrideMode === 'safe-auto' ? 'Safe auto' : 'Never ask'})`;
                  const existingOverride = permissionProfiles.find(p => 
                    p.name === overrideName && 
                    p.commandApproval === overrideMode &&
                    p.allowRead === baseProfile.allowRead &&
                    p.allowWrite === baseProfile.allowWrite &&
                    p.allowShell === baseProfile.allowShell &&
                    p.allowNetwork === baseProfile.allowNetwork
                  );
                  if (existingOverride) {
                    customPermissionId = existingOverride.id;
                  } else {
                    try {
                      const draft = {
                        ...baseProfile,
                        name: overrideName,
                        commandApproval: overrideMode,
                        updatedAt: new Date().toISOString()
                      };
                      delete (draft as any).id;
                      const saved = await upsertPermissionProfile(draft);
                      permissionProfiles = [saved, ...permissionProfiles];
                      customPermissionId = saved.id;
                    } catch (e) {
                      console.error("Failed to create overridden permission profile:", e);
                      customPermissionId = baseProfileId;
                    }
                  }
                } else {
                  customPermissionId = baseProfileId;
                }
              } else {
                customPermissionId = baseProfileId;
              }
            }
          }
        }
      }

      const view = await startAgentRun({
        conversationId: targetConversation.id,
        goal,
        mode: activeMode,
        systemPrompt,
        modelId,
        provider,
        autonomyProfileId: customPermissionId || activePermissionProfileId || null,
        maxSteps: null,
      });
      selectedAgentRunView = view;
      agentRuns = [view.run, ...agentRuns.filter((run) => run.id !== view.run.id)];
      showContextPanel = true;
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      agentActionBusy = null;
    }
  }

  async function loadCloudCollections() {
    if (!cloudAuthenticated) return;
    collectionsSyncBusy = true;
    collectionsSyncError = "";
    const pluginPresets = userPlugins.map((plugin) => ({ ...plugin }));
    userPlugins = pluginPresets;
    try {
      const [
        memoryRows,
        groupRows,
        skillRows,
        pluginRows,
        mcpRows,
        hookRows,
        taskRows,
        personalityRows,
        voiceRows,
        systemPromptRows,
        teamRows,
        agentDefinitionRows,
      ] = await Promise.all([
        listMemoryItems(),
        getCloudCollection<unknown>("skill-groups"),
        getCloudCollection<unknown>("skills"),
        getCloudCollection<unknown>("plugins"),
        getCloudCollection<unknown>("mcp"),
        getCloudCollection<unknown>("hooks"),
        getCloudCollection<unknown>("scheduled-tasks"),
        getCloudCollection<unknown>("personalities"),
        getCloudCollection<unknown>("voice-profiles"),
        getCloudCollection<unknown>("system-prompts"),
        getCloudCollection<unknown>("teams"),
        getCloudCollection<unknown>("agent-definitions"),
      ]);
      memoriesList = memoryRows;
      await refreshMemoryIndexStatus();
      const loadedGroups = asCloudRows(groupRows).map(mapSkillGroupRow);
      userSkillGroups = [
        userSkillGroups.find((group) => group.id === "all") ?? {
          id: "all",
          name: "Tous les Skills",
          description: "Toutes vos compétences configurées",
          createdAt: new Date().toISOString(),
        },
        ...loadedGroups.filter((group) => group.id !== "all"),
      ];
      userSkills = asCloudRows(skillRows).map(mapSkillRow);
      const loadedPlugins = asCloudRows(pluginRows).map((row) => mapPluginRow(row, pluginPresets));
      userPlugins = mergePluginPresets(pluginPresets, loadedPlugins);
      mcpServers = asCloudRows(mcpRows).map(mapMcpRow);
      hooks = asCloudRows(hookRows).map(mapHookRow);
      scheduledTasks = asCloudRows(taskRows).map(mapScheduledTaskRow);
      orgTeams = asCloudRows(teamRows).map(mapTeamRow);
      customPersonalities = asCloudRows(personalityRows).map(mapPersonalityRow);
      customVoices = asCloudRows(voiceRows).map(mapVoiceRow);
      customAgentsList = asCloudRows(agentDefinitionRows).map(row => ({
        id: rowClientId(row),
        cloudId: String(row.id),
        name: String(row.name ?? ""),
        description: String(row.description ?? ""),
        systemPrompt: String(row.systemPrompt ?? row.system_prompt ?? ""),
        modelProviderId: String(row.modelProviderId ?? row.model_provider_id ?? ""),
        modelId: String(row.modelId ?? row.model_id ?? ""),
        icon: String(row.icon ?? "bot"),
        permissionProfileId: String(row.permissionProfileId ?? row.permission_profile_id ?? ""),
        commandApproval: String(row.commandApproval ?? row.command_approval ?? ""),
        enabledTools: Array.isArray(row.enabledTools ?? row.enabled_tools) ? (row.enabledTools ?? row.enabled_tools) : [],
      }));
      localStorage.setItem("aro-custom-agents-definitions", JSON.stringify(customAgentsList));
      for (const row of asCloudRows(systemPromptRows)) {
        const mode = String(row.mode ?? "");
        if (!mode) continue;
        agiIdentity[mode] = String(row.identity ?? agiIdentity[mode] ?? "");
        agiRules[mode] = String(row.rules ?? agiRules[mode] ?? "");
        agiFormatting[mode] = String(row.formatting ?? agiFormatting[mode] ?? "");
        customSystemPrompts[mode] = String(row.compiledPrompt ?? row.compiled_prompt ?? customSystemPrompts[mode] ?? "");
        customSystemPromptsEnabled = Boolean(row.enabled ?? customSystemPromptsEnabled);
      }
    } catch (error) {
      collectionsSyncError = normalizeError(error);
      cloudSyncStatus = { ...cloudSyncStatus, health: "offline-read-only" };
    } finally {
      collectionsSyncBusy = false;
    }
  }


  function populateLoadedSteps(loadedMessages: ChatMessage[]) {
    // Reset client maps
    stepsByMessageId = {};
    runToMessageMap = {};
    for (const msg of loadedMessages) {
      if (msg.role === "assistant" && msg.steps && msg.steps.length > 0) {
        stepsByMessageId[msg.id] = msg.steps;
        if (msg.agentRunId) {
          runToMessageMap[msg.agentRunId] = msg.id;
        }
      }
    }
    stepsByMessageId = stepsByMessageId;
    runToMessageMap = runToMessageMap;
  }

  async function openConversation(conversation: Conversation) {
    stopSpeaking();
    pendingProjectId = null;
    pendingFolderId = null;
    activeConversation = conversation;
    activeMode = conversation.mode ?? "chat";
    errorMessage = "";
    showConversationMenu = false;
    void loadWorkspaceMentions(conversation.id);
    try {
      messages = inFlightMessagesByConversation[conversation.id] ?? await listMessages(conversation.id);
      populateLoadedSteps(messages);
    } catch (error) {
      errorMessage = normalizeError(error);
      messages = [];
    }
    await scrollToBottom(true);
    setTimeout(() => scrollToBottom(true), 50);
  }

  async function startConversation() {
    if (!ensureCloudWriteAllowed("demarrer une nouvelle conversation")) return;
    stopSpeaking();
    errorMessage = "";
    showConversationMenu = false;
    activeConversation = null;
    messages = [];
    pendingProjectId = null;
    pendingFolderId = null;
  }

  async function handleCleanEmptyConversations() {
    const confirmed = await requestConfirm({
      title: currentLanguage === "fr" ? "Supprimer les conversations vides ?" : "Delete empty conversations?",
      body:
        currentLanguage === "fr"
          ? "Toutes les conversations sans aucun message seront supprimées. Celles avec du contenu sont conservées."
          : "All conversations without any messages will be deleted. Ones with content are kept.",
      confirmLabel: currentLanguage === "fr" ? "Nettoyer" : "Clean up",
      cancelLabel: currentLanguage === "fr" ? "Annuler" : "Cancel",
    });
    if (!confirmed) return;
    try {
      const deletedIds = await deleteEmptyConversations();
      conversations = conversations.filter((c) => !deletedIds.includes(c.id));
      if (activeConversation && deletedIds.includes(activeConversation.id)) {
        activeConversation = null;
        messages = [];
      }
      addNotificationToast({
        type: "success",
        title: currentLanguage === "fr" ? "Nettoyage terminé" : "Cleanup done",
        body:
          currentLanguage === "fr"
            ? `${deletedIds.length} conversation(s) vide(s) supprimée(s).`
            : `${deletedIds.length} empty conversation(s) deleted.`,
      });
    } catch (error) {
      errorMessage = normalizeError(error);
    }
  }

  async function removeConversation(conversation: Conversation, event: MouseEvent) {
    event.stopPropagation();
    if (!ensureCloudWriteAllowed("supprimer une conversation")) return;
    await deleteConversation(conversation.id);
    conversations = conversations.filter((item) => item.id !== conversation.id);
    if (activeConversation?.id === conversation.id) {
      activeConversation = conversations[0] ?? null;
      messages = activeConversation ? await listMessages(activeConversation.id) : [];
      populateLoadedSteps(messages);
    }
  }

  function handleOpenCreateProjectModal() {
    projectToEdit = null;
    showCreateProjectModal = true;
  }

  function handleEditProject(proj: Project) {
    projectToEdit = proj;
    showCreateProjectModal = true;
  }

  async function handleSaveProject(data: { name: string; description?: string; instructions?: string; rootPath?: string; color: string; icon: string }) {
    if (!ensureCloudWriteAllowed("enregistrer un projet")) return;
    if (projectToEdit) {
      const updated = await updateProject(projectToEdit.id, data);
      projects = projects.map((p) => (p.id === updated.id ? updated : p));
    } else {
      const created = await createProject(data.name, data.description, data.instructions, data.rootPath, data.color, data.icon);
      projects = [created, ...projects];
    }
  }

  async function handleDeleteProject(proj: Project) {
    if (!ensureCloudWriteAllowed("supprimer un projet")) return;
    if (await requestConfirm({
      title: currentLanguage === "fr" ? `Supprimer le projet "${proj.name}" ?` : `Delete project "${proj.name}"?`,
      body: currentLanguage === "fr"
        ? "Ses dossiers seront détachés et ses conversations redeviendront indépendantes."
        : "Its folders will be detached and its conversations will become independent.",
      confirmLabel: currentLanguage === "fr" ? "Supprimer" : "Delete",
      cancelLabel: currentLanguage === "fr" ? "Annuler" : "Cancel",
    })) {
      await deleteProject(proj.id);
      projects = projects.filter((p) => p.id !== proj.id);
      folders = folders.map((f) => (f.projectId === proj.id ? { ...f, projectId: null } : f));
      conversations = conversations.map((c) => (c.projectId === proj.id ? { ...c, projectId: null } : c));
    }
  }

  function handleOpenCreateFolderModal(projectId: string | null = null) {
    folderToEdit = null;
    folderDefaultProjectId = projectId;
    showCreateFolderModal = true;
  }

  function handleEditFolder(folder: Folder) {
    folderToEdit = folder;
    showCreateFolderModal = true;
  }

  async function handleSaveFolder(data: { name: string; projectId?: string | null; rootPath?: string; color?: string }) {
    if (!ensureCloudWriteAllowed("enregistrer un dossier")) return;
    if (folderToEdit) {
      const updated = await updateFolder(folderToEdit.id, data);
      folders = folders.map((f) => (f.id === updated.id ? updated : f));
    } else {
      const created = await createFolder(data.name, data.projectId, data.rootPath, data.color);
      folders = [created, ...folders];
    }
  }

  async function handleDeleteFolder(fold: Folder) {
    if (!ensureCloudWriteAllowed("supprimer un dossier")) return;
    if (await requestConfirm({
      title: currentLanguage === "fr" ? `Supprimer le dossier "${fold.name}" ?` : `Delete folder "${fold.name}"?`,
      body: currentLanguage === "fr"
        ? "Ses conversations redeviendront indépendantes."
        : "Its conversations will become independent.",
      confirmLabel: currentLanguage === "fr" ? "Supprimer" : "Delete",
      cancelLabel: currentLanguage === "fr" ? "Annuler" : "Cancel",
    })) {
      await deleteFolder(fold.id);
      folders = folders.filter((f) => f.id !== fold.id);
      conversations = conversations.map((c) => (c.folderId === fold.id ? { ...c, folderId: null } : c));
    }
  }

  function handleOpenMoveModal(conversation: Conversation) {
    conversationToMove = conversation;
    showMoveModal = true;
  }

  async function handleMoveConversation(conversationId: string, projectId: string | null, folderId: string | null) {
    if (!ensureCloudWriteAllowed("déplacer une conversation")) return;
    const updated = await moveConversation(conversationId, projectId, folderId);
    conversations = conversations.map((c) => (c.id === updated.id ? updated : c));
    if (activeConversation?.id === updated.id) {
      activeConversation = updated;
    }
  }

  async function handleDropConversationToFolder(conversationId: string, folderId: string) {
    const targetFolder = folders.find((f) => f.id === folderId);
    const projectId = targetFolder?.projectId ?? null;
    await handleMoveConversation(conversationId, projectId, folderId);
  }

  async function handleDropConversationToProject(conversationId: string, projectId: string) {
    await handleMoveConversation(conversationId, projectId, null);
  }

  async function handleExportConversation(conversation: Conversation, format: 'markdown' | 'json') {
    try {
      const blob = await exportConversation(conversation.id, format);
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      const sanitizedTitle = conversation.title.replace(/[^a-zA-Z0-9_-]/g, "_");
      a.download = `${sanitizedTitle}_${conversation.id.slice(0, 8)}.${format === "json" ? "json" : "md"}`;
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      a.remove();
    } catch (error) {
      errorMessage = normalizeError(error);
    }
  }

  async function handleExportProject(project: Project) {
    try {
      const blob = await exportProject(project.id);
      const url = window.URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      const sanitizedTitle = project.name.replace(/[^a-zA-Z0-9_-]/g, "_");
      a.download = `project_${sanitizedTitle}_${project.id.slice(0, 8)}.json`;
      document.body.appendChild(a);
      a.click();
      window.URL.revokeObjectURL(url);
      a.remove();
    } catch (error) {
      errorMessage = normalizeError(error);
    }
  }

  function renameConversation(conversation: Conversation) {
    if (!ensureCloudWriteAllowed("renommer une conversation")) return;
    renamingConversationObject = conversation;
    renameInputValue = conversation.title;
  }

  async function saveRename() {
    if (!renamingConversationObject) return;
    if (!ensureCloudWriteAllowed("renommer une conversation")) return;
    const conversation = renamingConversationObject;
    const newTitle = renameInputValue.trim();
    if (!newTitle || newTitle === conversation.title) {
      renamingConversationObject = null;
      return;
    }
    try {
      const updated = await updateConversationTitle(conversation.id, newTitle);
      conversations = conversations.map((item) =>
        item.id === conversation.id ? { ...item, title: updated.title, updatedAt: updated.updatedAt } : item
      );
      if (activeConversation?.id === conversation.id) {
        activeConversation = { ...activeConversation, title: updated.title, updatedAt: updated.updatedAt };
      }
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      renamingConversationObject = null;
    }
  }

  function startEdit(message: ChatMessage) {
    editingMessageId = message.id;
    editingMessageText = message.content;
  }

  function cancelEdit() {
    editingMessageId = null;
    editingMessageText = "";
  }

  async function saveEdit(message: ChatMessage) {
    if (!ensureCloudWriteAllowed("modifier un message")) return;
    const convId = message.conversationId;
    if (isConversationSending(convId)) return;
    const newContent = editingMessageText.trim();
    if (!newContent) return;

    if (newContent === message.content) {
      cancelEdit();
      return;
    }

    const messageIndex = messages.findIndex((m) => m.id === message.id);
    if (messageIndex === -1) return;

    cancelEdit();
    stopSpeaking();

    const updatedUserMsg = { ...message, content: newContent };
    
    // Add user message and assistant placeholder generating state
    const assistantMsgId = crypto.randomUUID();
    const tempGeneratingMsg: ChatMessage = {
      id: assistantMsgId,
      conversationId: message.conversationId,
      role: "assistant",
      content: "",
      createdAt: new Date().toISOString(),
      tokenEstimate: null,
      isGenerating: true,
    };
    
    messages = [...messages.slice(0, messageIndex), updatedUserMsg, tempGeneratingMsg];
    setInFlightMessages(convId, messages);
    await scrollToBottom(true);

    setConversationSending(convId, true);
    errorMessage = "";

    try {
      await updateMessage(message.id, newContent);
      const startTime = performance.now();
      const personalityId = conversationPersonalities[convId] || selectedPersonalityId;
      const activePers = allPersonalities.find(p => p.id === personalityId) || allPersonalities[0];
      const modeAtEdit = activeConversation?.mode || "chat";
      const conversationTitleAtEdit = activeConversation?.title || "Conversation";
      let systemPrompt = compileSystemPrompt(activePers, modeAtEdit, newContent);
      const customAgentId = conversationCustomAgents[convId];
      if (customAgentId) {
        const matchedAgent = customAgentsList.find(a => a.id === customAgentId);
        if (matchedAgent) {
          systemPrompt = matchedAgent.systemPrompt;
        }
      }

      const assistantMsg = await regenerateMessageStream(message.conversationId, systemPrompt, assistantMsgId);
      const endTime = performance.now();
      const timeSec = (endTime - startTime) / 1000;
      const stillViewingTarget = isStillViewingConversation(convId);

      // Update placeholders with final metadata
      if (stillViewingTarget) {
        messages = messages.map(msg => {
          if (msg.id === assistantMsgId) {
            return {
              ...assistantMsg,
              content: assistantMsg.content
            };
          }
          return msg;
        });
      }
      
      const tokenCount = assistantMsg.tokenEstimate || estimateTokens(assistantMsg.content);
      addPerformanceRecord(conversationTitleAtEdit, timeSec, tokenCount);
      
      if (stillViewingTarget) {
        await speak(assistantMsg.content);
      }
    } catch (error) {
      if (isStillViewingConversation(convId)) {
        errorMessage = normalizeError(error);
        // Remove temp generating message on error
        messages = messages.filter(msg => msg.id !== assistantMsgId);
      }
    } finally {
      setConversationSending(convId, false);
      clearInFlightMessages(convId);
      void refreshAgentRuns();
    }
  }

  let isAiPlanGenerating = false;
  async function handleRequestAiPlan(prompt: string) {
    if (isAiPlanGenerating) return;
    isAiPlanGenerating = true;
    try {
      await submitMessage(prompt);
    } finally {
      isAiPlanGenerating = false;
    }
  }

  async function submitMessage(forcedContent: string | undefined = undefined) {

    if (arenaMode) {
      await submitArena();
      return;
    }
    const isForced = typeof forcedContent === "string";
    const content = (isForced ? forcedContent : input).trim();
    const filesToSend = isForced ? [] : attachedFiles;
    const initialConversationId = activeConversation?.id ?? null;
    let targetConversationId = initialConversationId;
    if ((!content && filesToSend.length === 0) || isConversationSending(initialConversationId)) return;
    if (!ensureCloudWriteAllowed("envoyer un message")) return;
    const attachmentRefs = buildAttachmentRefs(filesToSend);
    const payloadContent = content || (attachmentRefs.length > 0 ? "Analyse les fichiers joints." : "");
    // F4.3 — inject referenced workspace files as context (visible bubble keeps @tokens).
    let sendContent = payloadContent;
    if (!isForced && payloadContent) {
      try {
        const { contextBlock } = await buildMentionContext(payloadContent);
        if (contextBlock) sendContent = contextBlock;
      } catch (err) {
        console.warn("Mention context injection failed", err);
      }
      try {
        const attachmentContext = await buildLocalAttachmentContext(filesToSend);
        if (attachmentContext) sendContent = `${sendContent}\n\nContexte des pièces jointes :\n${attachmentContext}`;
      } catch (err) {
        console.warn("Attachment context injection failed", err);
      }
    }
    const modeAtSend = activeMode;
    let conversationTitleAtSend = activeConversation?.title || conversationTitleFromContent(payloadContent);
    const tempUserMsgId = crypto.randomUUID();
    const assistantMsgId = crypto.randomUUID();
    let createdConversationForSend = false;

    setConversationSending(initialConversationId, true);
    errorMessage = "";
    input = "";
    attachedFiles = [];
    await resizeComposer();

    const startTime = performance.now();
    try {
      if (!targetConversationId) {
        const conversation = await createAndActivateConversation(payloadContent, modeAtSend);
        targetConversationId = conversation.id;
        conversationTitleAtSend = conversation.title;
        createdConversationForSend = true;
        setConversationSending(targetConversationId, true);
        setConversationSending(initialConversationId, false);
      }

      const personalityId = targetConversationId 
        ? (conversationPersonalities[targetConversationId] || selectedPersonalityId) 
        : selectedPersonalityId;
      const activePers = allPersonalities.find(p => p.id === personalityId) || allPersonalities[0];
      let systemPrompt = compileSystemPrompt(activePers, modeAtSend, sendContent);
      let modelId = settings?.model.activeModelRef?.modelId ?? null;
      let provider = settings?.model.activeModelRef?.providerId ?? null;

      if (targetConversationId) {
        const customAgentId = conversationCustomAgents[targetConversationId];
        if (customAgentId) {
          const matchedAgent = customAgentsList.find(a => a.id === customAgentId);
          if (matchedAgent) {
            systemPrompt = matchedAgent.systemPrompt;
            if (matchedAgent.modelId) {
              modelId = matchedAgent.modelId;
            }
            if (matchedAgent.modelProviderId) {
              provider = matchedAgent.modelProviderId;
            }
          }
        }
      }

      const tempUserMsg: ChatMessage = {
        id: tempUserMsgId,
        conversationId: targetConversationId ?? "",
        role: "user",
        content: payloadContent,
        attachments: attachmentRefs,
        createdAt: new Date().toISOString(),
        tokenEstimate: null,
      };

      const tempGeneratingMsg: ChatMessage = {
        id: assistantMsgId,
        conversationId: targetConversationId ?? "",
        role: "assistant",
        content: "",
        createdAt: new Date().toISOString(),
        tokenEstimate: null,
        isGenerating: true,
      };

      messages = [...messages, tempUserMsg, tempGeneratingMsg];
      setInFlightMessages(targetConversationId, messages);
      await scrollToBottom(true);

      const response = await sendMessageStream({
        conversationId: targetConversationId,
        content: sendContent,
        mode: modeAtSend,
        systemPrompt: systemPrompt,
        modelId: modelId,
        provider: provider,
        attachments: attachmentRefs,
        webAccess,
        searchSettings: settings?.search ?? null,
      }, assistantMsgId);
      const endTime = performance.now();
      const timeSec = (endTime - startTime) / 1000;
      const stillViewingTarget = isStillViewingConversation(targetConversationId);
      
      if (stillViewingTarget) {
        activeConversation = response.conversation;
      }
      
      if (createdConversationForSend && response.conversation.id) {
        conversationPersonalities[response.conversation.id] = personalityId;
        saveConversationPersonalities();
      }

      promoteConversation(response.conversation);

      // Update placeholders with final metadata only if this conversation is still on screen.
      if (stillViewingTarget) {
        messages = messages.map(msg => {
          if (msg.id === assistantMsgId) {
            return {
              ...response.assistantMessage,
              content: response.assistantMessage.content,
              steps: msg.steps
            };
          }
          if (msg.id === tempUserMsgId) {
            return response.userMessage;
          }
          return msg;
        });
      }

      const tokenCount = response.assistantMessage.tokenEstimate || estimateTokens(response.assistantMessage.content);
      addPerformanceRecord(response.conversation.title || conversationTitleAtSend, timeSec, tokenCount);

      if (response.agentRunId) {
        await refreshAgentRuns(response.agentRunId);
      }
      
      if (stillViewingTarget) {
        await speak(response.assistantMessage.content);
      }
    } catch (error) {
      if (isStillViewingConversation(targetConversationId)) {
        if (!isForced) {
          input = content;
          attachedFiles = filesToSend;
          await resizeComposer();
        }
        // 409 : une génération tourne déjà côté serveur (rien n'a été
        // enregistré) : info douce + saisie restaurée, pas d'erreur.
        if ((error as { alreadyRunning?: boolean })?.alreadyRunning) {
          addNotificationToast({
            type: "info",
            title: currentLanguage === "fr" ? "Génération déjà en cours" : "Generation already running",
            body: currentLanguage === "fr"
              ? "Une réponse est déjà en cours pour cette conversation. Réessayez quand elle est terminée."
              : "A response is already being generated for this conversation. Try again when it finishes.",
          });
        } else {
          errorMessage = normalizeError(error);
        }
        // Remove temp generating message on error
        messages = messages.filter(msg => msg.id !== assistantMsgId && msg.id !== tempUserMsgId);
      } else {
        // L'utilisateur a changé de conversation pendant l'envoi : l'erreur
        // ne doit pas disparaître silencieusement (les messages optimistes
        // sont nettoyés dans finally).
        addNotificationToast({
          type: "error",
          title: currentLanguage === "fr" ? "Échec de l'envoi" : "Send failed",
          body: normalizeError(error),
        });
      }
    } finally {
      setConversationSending(targetConversationId, false);
      if (targetConversationId !== initialConversationId) {
        setConversationSending(initialConversationId, false);
      }
      clearInFlightMessages(targetConversationId);
    }
  }

  async function sendUserPrompt(promptText: string) {
    if (!promptText || !promptText.trim()) return;
    const cleanText = promptText.trim();
    const convId = activeConversation?.id ?? null;
    if (isConversationSending(convId)) {
      console.warn("Conversation is already sending, skipping duplicate prompt:", cleanText);
      return;
    }
    input = "";
    // Note : on ne vide PAS attachedFiles ici — submitMessage les consomme
    // (filesToSend) puis les réinitialise après envoi. Les vider ici ferait
    // perdre silencieusement les pièces jointes déjà sélectionnées.
    await resizeComposer();
    await submitMessage(cleanText);
  }

  // Ensure window global functions are wired immediately
  if (typeof window !== "undefined") {
    (window as any).__aroSendPrompt = sendUserPrompt;
    (window as any).__sendChatOptionHandler = sendUserPrompt;
    (window as any).__sendChatOption = (target: string | HTMLElement) => {
      let optionText = "";
      if (typeof target === "string") {
        optionText = target;
      } else if (target) {
        const raw = target.getAttribute?.("data-option");
        if (raw) {
          try { optionText = decodeURIComponent(raw); } catch { optionText = raw; }
        }
        if (!optionText && target.textContent) {
          optionText = target.textContent.trim();
        }
      }
      if (optionText) {
        void sendUserPrompt(optionText);
      }
    };
  }

  function legacyUnusedIsWhisperHallucination(text: string): boolean {
    const clean = text.trim().toLowerCase();
    if (!clean) return true;
    
    // Ambient sound markers like [Musique], (silence), *Générique*
    if (/^\[.*\]$/.test(clean) || /^\(.*\)$/.test(clean) || /^\*.*\*$/.test(clean)) {
      return true;
    }
    
    const commonHallucinations = [
      "musique",
      "music",
      "générique",
      "generique",
      "silence",
      "blank",
      "bruit",
      "noise",
      "thank you",
      "thank you.",
      "thank you very much.",
      "merci",
      "merci.",
      "merci beaucoup",
      "merci beaucoup.",
      "sous-titres",
      "sous-titres par",
      "laughter",
      "rires",
      "sourire",
      "générique de fin",
      "fin de la musique"
    ];
    
    if (commonHallucinations.includes(clean)) return true;
    if (/^[.,\/#!$%\^&\*;:{}=\-_`~()?"'\s]+$/.test(clean)) return true;
    
    return false;
  }

  // Live Voice features states & helpers
  let lastLiveTranscribeTime = 0;
  let isLiveTranscribing = false;
  let wavePhase = 0;
  let animationFrameId: number;

  function animateWave() {
    if (recording) {
      wavePhase += 0.08;
      wavePhase = wavePhase; // Trigger Svelte reactivity
      animationFrameId = requestAnimationFrame(animateWave);
    }
  }

  function getWavePath(phaseOffset: number, vol: number, height = 60) {
    const width = 200;
    const centerY = height / 2;
    // Scale volume to a peak pixel amplitude (max 50% of height)
    const amplitude = Math.max(1.5, vol * (height * 0.5));
    
    let points = [];
    const steps = 40;
    for (let i = 0; i <= steps; i++) {
      const x = (i / steps) * width;
      const envelope = Math.sin((i / steps) * Math.PI); // Taper amplitude at the boundaries
      const angle = (i / steps) * Math.PI * 3 + wavePhase + phaseOffset;
      const y = centerY + Math.sin(angle) * amplitude * envelope;
      points.push(`${x.toFixed(1)},${y.toFixed(1)}`);
    }
    return `M ${points.join(" L ")}`;
  }

  async function runLiveTranscription() {
    if (voiceInputMode === "push-to-talk") return;
    if (isLiveTranscribing || pcmChunks.length < 5) return;
    if (cloudWriteLocked) return;
    const sessionId = activeVoiceSession?.id ?? null;
    const targetConversationId = activeVoiceSession?.conversationId ?? voiceSessionConversationId;
    if (!isSpotlightMode && !isStillViewingConversation(targetConversationId)) return;
    isLiveTranscribing = true;
    try {
      const chunksCopy = boundAudioChunksForDuration(
        pcmChunks,
        recordingSampleRate,
        liveTranscriptionSeconds,
      );
      const wavBytes = encodeVoiceWav(chunksCopy, recordingSampleRate);
      const result = await transcribeAudio(Array.from(wavBytes), "audio/wav");
      if (sessionId && activeVoiceSession?.id !== sessionId) return;
      if (!isSpotlightMode && !isStillViewingConversation(targetConversationId)) return;
      if (result.text && result.text.trim()) {
        const text = result.text.trim();
        if (!isWhisperHallucinationText(text)) {
          updateVoiceSession({ transcript: text });
          if (isSpotlightMode) {
            spotlightInput = text;
          } else {
            input = text;
            await resizeComposer();
            if (messages.length === 0) {
              recordingHint = text;
            }
          }
        }
      }
    } catch (e) {
      console.error("Live transcription error:", e);
    } finally {
      isLiveTranscribing = false;
    }
  }

  function handleRecordingAudioChunk(
    inputBuffer: Float32Array,
    sampleRate = recordingSampleRate,
    chunkRms: number | undefined = undefined,
  ) {
    recordingSampleRate = sampleRate;
    const bounded = appendBoundedAudioChunk(
      { ...EMPTY_AUDIO_CHUNK_BUFFER, chunks: pcmChunks },
      inputBuffer,
      samplesForDuration(recordingSampleRate, maxRecordingSeconds),
    );
    pcmChunks = [...bounded.chunks];

    let rms = chunkRms;
    if (rms === undefined) {
      let sum = 0;
      for (let i = 0; i < inputBuffer.length; i++) {
        sum += inputBuffer[i] * inputBuffer[i];
      }
      rms = Math.sqrt(sum / inputBuffer.length);
    }
    voiceVolume = voiceVolume * 0.3 + rms * 0.7;

    const threshold = 0.02;
    if (rms > threshold) {
      lastSpeakingTime = Date.now();
      hasSpoken = true;
    } else if (hasSpoken) {
      const silenceDuration = Date.now() - lastSpeakingTime;
      if (silenceDuration > 1500) {
        hasSpoken = false;
        triggerAutoSubmit();
      }
    }

    if (
      hasSpoken &&
      voiceInputMode !== "push-to-talk" &&
      !isLiveTranscribing &&
      Date.now() - lastLiveTranscribeTime > 2000
    ) {
      lastLiveTranscribeTime = Date.now();
      runLiveTranscription();
    }
  }

  function startLegacyRecordingCapture(stream: MediaStream) {
    audioContext = new AudioContext();
    recordingSampleRate = audioContext.sampleRate;
    audioSource = audioContext.createMediaStreamSource(stream);
    audioProcessor = audioContext.createScriptProcessor(4096, 1, 1);
    silentGain = audioContext.createGain();
    silentGain.gain.value = 0;

    audioProcessor.onaudioprocess = (event) => {
      handleRecordingAudioChunk(event.inputBuffer.getChannelData(0), audioContext?.sampleRate ?? recordingSampleRate);
    };

    audioSource.connect(audioProcessor);
    audioProcessor.connect(silentGain);
    silentGain.connect(audioContext.destination);
  }

  async function stopRecordingCapture() {
    try {
      await voiceCaptureSession?.stop();
      audioProcessor?.disconnect();
      audioSource?.disconnect();
      silentGain?.disconnect();
      recordingStream?.getTracks().forEach((track) => track.stop());
      if (audioContext && audioContext.state !== "closed") {
        await audioContext.close();
      }
    } finally {
      voiceCaptureSession = null;
      recordingStream = null;
      audioContext = null;
      audioSource = null;
      audioProcessor = null;
      silentGain = null;
    }
  }

  async function startRecording() {
    if (!voiceCanRecord || recordingStarting || recording || assistantSpeaking || sending || loading) return;
    errorMessage = "";
    recordingStarting = true;
    try {
      stopSpeaking();
      createVoiceSession();
      const stream = await navigator.mediaDevices.getUserMedia(voiceAudioConstraints);
      recordingStream = stream;
      pcmChunks = [];
      hasSpoken = false;
      lastSpeakingTime = Date.now();
      lastLiveTranscribeTime = Date.now(); // reset live transcription timer
      voiceVolume = 0;

      try {
        voiceCaptureSession = await createVoiceCaptureSession(stream, {
          closeContextOnStop: false,
          onChunk: (chunk: VoiceCaptureChunkMessage) =>
            handleRecordingAudioChunk(chunk.samples, chunk.sampleRate, chunk.rms),
          onError: (err) => {
            console.error("Voice capture failed", err);
            errorMessage = normalizeError(err);
            void stopRecording();
          },
          stopTracksOnStop: false,
        });
        audioContext = voiceCaptureSession.context;
        recordingSampleRate = voiceCaptureSession.sampleRate;
      } catch (workletError) {
        console.warn("AudioWorklet voice capture unavailable, using legacy capture", workletError);
        startLegacyRecordingCapture(stream);
      }

      recording = true;
      recordingHint = t.listeningHint;
      
      // Start wave animation loop
      requestAnimationFrame(animateWave);
    } catch (error) {
      errorMessage = microphoneErrorMessage(error);
      voiceAutoListen = false;
      voiceHandsFreeArmed = false;
      recording = false;
      recordingHint = "";
      voiceVolume = 0;
      // Le micro et l'AudioContext peuvent être déjà ouverts à ce stade
      // (getUserMedia OK puis capture en échec) : on les libère, sinon ils
      // fuient à chaque tentative ratée.
      try {
        await stopRecordingCapture();
      } catch {
        /* libération best-effort */
      }
      clearVoiceSession("cancelled");
    } finally {
      recordingStarting = false;
    }
  }

  async function sendVoiceMessageToAnchoredConversation(
    text: string,
    targetConversationId: string | null,
  ) {
    if (!ensureCloudWriteAllowed("envoyer un message vocal")) return;
    if (isConversationSending(targetConversationId)) return;
    updateVoiceSession({ state: "submitting", transcript: text });

    const initialConversationId = targetConversationId;
    let resolvedConversationId = targetConversationId;
    const assistantMsgId = crypto.randomUUID();
    const tempUserMsgId = crypto.randomUUID();
    let modeAtSend = activeMode;
    let personalityId = selectedPersonalityId;
    let createdConversationForSend = false;

    setConversationSending(initialConversationId, true);
    try {
      if (!resolvedConversationId) {
        const conversation = await createAndActivateConversation(text, modeAtSend);
        resolvedConversationId = conversation.id;
        createdConversationForSend = true;
        voiceSessionConversationId = conversation.id;
        updateVoiceSession({ conversationId: conversation.id });
        setConversationSending(resolvedConversationId, true);
        setConversationSending(initialConversationId, false);
      }

      const targetConversation = resolvedConversationId
        ? conversations.find((conversation) => conversation.id === resolvedConversationId) ?? activeConversation
        : null;
      modeAtSend = targetConversation?.mode ?? activeMode;
      personalityId = resolvedConversationId
        ? (conversationPersonalities[resolvedConversationId] || selectedPersonalityId)
        : selectedPersonalityId;
      const activePers = allPersonalities.find((personality) => personality.id === personalityId) || allPersonalities[0];
      const tempUserMsg: ChatMessage = {
        id: tempUserMsgId,
        conversationId: resolvedConversationId ?? "",
        role: "user",
        content: text,
        createdAt: new Date().toISOString(),
        tokenEstimate: null,
      };
      const tempGeneratingMsg: ChatMessage = {
        id: assistantMsgId,
        conversationId: resolvedConversationId ?? "",
        role: "assistant",
        content: "",
        createdAt: new Date().toISOString(),
        tokenEstimate: null,
        isGenerating: true,
      };

      const visibleTarget = isStillViewingConversation(resolvedConversationId);
      const baseMessages = visibleTarget
        ? messages
        : resolvedConversationId
          ? await listMessages(resolvedConversationId)
          : [];
      const optimisticMessages = [...baseMessages, tempUserMsg, tempGeneratingMsg];
      if (resolvedConversationId) {
        setInFlightMessages(resolvedConversationId, optimisticMessages);
      }
      if (visibleTarget) {
        messages = optimisticMessages;
        if (input.trim() === text) {
          input = "";
          await resizeComposer();
        }
        await scrollToBottom(true);
      }

      const response = await sendMessageStream({
        conversationId: resolvedConversationId,
        content: text,
        mode: modeAtSend,
        systemPrompt: compileSystemPrompt(activePers, modeAtSend, text),
        modelId: settings?.model.activeModelRef?.modelId ?? null,
        provider: settings?.model.activeModelRef?.providerId ?? null,
        webAccess,
        searchSettings: settings?.search ?? null,
      }, assistantMsgId);

      if (createdConversationForSend && response.conversation.id) {
        conversationPersonalities[response.conversation.id] = personalityId;
        saveConversationPersonalities();
      }
      promoteConversation(response.conversation);
      if (isStillViewingConversation(resolvedConversationId)) {
        activeConversation = response.conversation;
        messages = await listMessages(response.conversation.id);
        await scrollToBottom(false);
        await speak(response.assistantMessage.content);
      }
    } catch (error) {
      if (isStillViewingConversation(resolvedConversationId)) {
        errorMessage = (error as { alreadyRunning?: boolean })?.alreadyRunning
          ? (currentLanguage === "fr"
              ? "Une réponse est déjà en cours. Réessayez quand elle est terminée."
              : "A response is already being generated. Try again when it finishes.")
          : normalizeError(error);
        messages = messages.filter((message) => message.id !== assistantMsgId && message.id !== tempUserMsgId);
      }
    } finally {
      setConversationSending(resolvedConversationId, false);
      if (resolvedConversationId !== initialConversationId) {
        setConversationSending(initialConversationId, false);
      }
      clearInFlightMessages(resolvedConversationId);
    }
  }

  async function handleVoiceTranscript(
    text: string,
    targetConversationId: string | null,
    shouldAutoSubmit: boolean,
  ) {
    updateVoiceSession({ transcript: text });
    if (isSpotlightMode) {
      spotlightInput = text;
      if (shouldAutoSubmit) {
        await submitSpotlightMessage();
      } else {
        await updateSpotlightWindowSize();
      }
      return;
    }

    if (!shouldAutoSubmit) {
      if (isStillViewingConversation(targetConversationId)) {
        input = text;
        await resizeComposer();
      }
      return;
    }

    if (isStillViewingConversation(targetConversationId)) {
      input = text;
      await resizeComposer();
    }

    await sendVoiceMessageToAnchoredConversation(text, targetConversationId);
  }

  // Garde-fou boucle infinie : après 3 redémarrages auto consécutifs en
  // échec (binaire manquant, micro coupé…), on rend la main au lieu de
  // ré-enregistrer indéfiniment. Réinitialisé à chaque transcript valide.
  let voiceAutoRestartFailures = 0;
  const MAX_VOICE_AUTO_RESTARTS = 3;

  async function restartRecordingAfterFailure() {
    voiceAutoRestartFailures += 1;
    if (voiceAutoRestartFailures > MAX_VOICE_AUTO_RESTARTS) {
      voiceAutoRestartFailures = 0;
      voiceAutoListen = false;
      voiceHandsFreeArmed = false;
      recordingHint = "";
      errorMessage =
        currentLanguage === "fr"
          ? "Capture vocale impossible après plusieurs essais. Vérifiez le micro et les binaires voix dans Paramètres > Voix."
          : "Voice capture keeps failing. Check the microphone and voice binaries in Settings > Voice.";
      return;
    }
    await startRecording();
  }

  async function triggerAutoSubmit() {
    if (!recording) return;
    const shouldAutoSubmit = activeVoiceSession?.autoSubmit ?? (voiceAutoListen || voiceInputMode === "hands-free");
    const targetConversationId = activeVoiceSession?.conversationId ?? voiceSessionConversationId;
    updateVoiceSession({ state: "transcribing" });
    recording = false;
    recordingHint = t.transcribingHint;
    try {
      if (animationFrameId) cancelAnimationFrame(animationFrameId);
      const chunksSnapshot = [...pcmChunks];
      const sampleRateSnapshot = recordingSampleRate;
      await stopRecordingCapture();

      const wavBytes = encodeVoiceWav(chunksSnapshot, sampleRateSnapshot);
      const result = await transcribeAudio(Array.from(wavBytes), "audio/wav");

      if (result.text && result.text.trim()) {
        const text = result.text.trim();
        if (isWhisperHallucinationText(text)) {
          if (voiceConfigured && shouldAutoSubmit && !wakeWordEnabled) {
            await restartRecordingAfterFailure();
          }
        } else {
          voiceAutoRestartFailures = 0;
          await handleVoiceTranscript(text, targetConversationId, shouldAutoSubmit);
        }
      } else {
        if (voiceConfigured && shouldAutoSubmit && !wakeWordEnabled) {
          await restartRecordingAfterFailure();
        }
      }
    } catch (error) {
      console.error("Auto submit transcription failed", error);
      if (voiceConfigured && shouldAutoSubmit && !wakeWordEnabled) {
        await restartRecordingAfterFailure();
      }
    } finally {
      voiceAutoListen = shouldAutoSubmit && voiceInputMode === "hands-free" && !wakeWordEnabled;
      recordingHint = "";
      pcmChunks = [];
      voiceVolume = 0;
      clearVoiceSession("complete");
    }
  }

  async function setVoiceInputMode(mode: VoiceInputMode) {
    if (mode !== "hands-free") {
      if (voiceInputMode === "hands-free") {
        disarmHandsFreeVoice();
      }
      voiceInputMode = mode;
      voiceAutoListen = false;
      if (recording) {
        await stopRecording();
      }
      return;
    }

    voiceInputMode = "hands-free";
    if (!ensureVoiceRecordingReady()) {
      voiceInputMode = "dictation";
      disarmHandsFreeVoice();
      return;
    }
    voiceHandsFreeArmed = true;
    voiceAutoListen = !wakeWordEnabled;
    voiceAutoRestartFailures = 0;
  }

  async function toggleRecording() {
    if (assistantSpeaking) {
      stopSpeaking();
      return;
    }
    // Départ manuel : on repart de zéro sur le compteur d'échecs auto.
    voiceAutoRestartFailures = 0;
    if (!recording && !ensureCloudWriteAllowed("dicter un message")) return;
    if (!ensureVoiceRecordingReady()) {
      return;
    }

    if (voiceInputMode === "hands-free") {
      if (recording || voiceHandsFreeArmed || wakeWordStream) {
        disarmHandsFreeVoice();
        if (recording) {
          await stopRecording();
        }
        return;
      }
      voiceHandsFreeArmed = true;
      voiceAutoListen = !wakeWordEnabled;
      if (!wakeWordEnabled) {
        await startRecording();
      }
      return;
    }

    if (recording) {
      voiceAutoListen = false;
      voiceHandsFreeArmed = false;
      await stopRecording();
      return;
    }

    errorMessage = "";
    voiceAutoListen = false;
    voiceHandsFreeArmed = false;
    await startRecording();
  }

  async function stopRecording() {
    const targetConversationId = activeVoiceSession?.conversationId ?? voiceSessionConversationId;
    updateVoiceSession({ state: "transcribing" });
    recording = false;
    recordingHint = t.transcribingLocallyHint;
    try {
      if (animationFrameId) cancelAnimationFrame(animationFrameId);
      const chunksSnapshot = [...pcmChunks];
      const sampleRateSnapshot = recordingSampleRate;
      await stopRecordingCapture();

      const wavBytes = encodeVoiceWav(chunksSnapshot, sampleRateSnapshot);
      const result = await transcribeAudio(Array.from(wavBytes), "audio/wav");
      if (result.text && result.text.trim()) {
        const text = result.text.trim();
        if (!isWhisperHallucinationText(text)) {
          updateVoiceSession({ transcript: text });
          if (ensureCloudWriteAllowed("dicter un message") && isStillViewingConversation(targetConversationId)) {
            input = text;
            await resizeComposer();
          }
        } else {
        }
      } else {
        errorMessage = t.noTranscriptionError;
      }
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      recordingHint = "";
      pcmChunks = [];
      voiceVolume = 0;
      clearVoiceSession("complete");
    }
  }

  async function refreshRuntimeState() {
    const [nextRuntime, nextVoiceModelStatus] = await Promise.all([
      checkRuntime(),
      checkVoiceModelStatus(),
    ]);
    voiceModelStatus = nextVoiceModelStatus;
    runtime = {
      ...nextRuntime,
      voiceReady: nextVoiceModelStatus.voiceStatus.ready,
      voiceStatus: nextVoiceModelStatus.voiceStatus,
    };
  }

  async function refreshRuntime() {
    errorMessage = "";
    try {
      await refreshRuntimeState();
    } catch (error) {
      errorMessage = normalizeError(error);
    }
  }

  async function closeSettings() {
    if (!cloudWriteLocked) {
      await autosaveSettings();
    }
    showSettings = false;
  }

  async function autosaveSettings() {
    if (!settingsDraft) return;
    if (!ensureCloudWriteAllowed("modifier les reglages")) return;
    errorMessage = "";
    try {
      settingsDraft.voice.wakeWord = settingsDraft.voice.wakeWord ?? {
        enabled: false,
        runtime: "disabled",
        modelPath: null,
        threshold: 0.65,
      };
      settingsDraft.voice.wakeWord.enabled = wakeWordDraftEnabled;
      settingsDraft.voice.wakeWord.runtime = wakeWordDraftEnabled ? "local-model" : "disabled";
      settings = await updateSettings(settingsDraft);
      
      // Save preferences
      currentTheme = themeDraft;
      currentLanguage = languageDraft;
      wakeWordEnabled = wakeWordDraftEnabled;
      if (!wakeWordEnabled || !settings.voice.enabled || settings.voice.speechToText === "disabled") {
        disarmHandsFreeVoice();
      }
      if (cloudAuthenticated) {
        await updatePreferences({
          theme: currentTheme,
          language: currentLanguage,
          wakeWordEnabled,
        });
      } else if (typeof localStorage !== "undefined") {
        localStorage.setItem("aro-theme", currentTheme);
        localStorage.setItem("aro-language", currentLanguage);
        localStorage.setItem("aro-wakeword-enabled", String(wakeWordEnabled));
      }
      
      await refreshRuntimeState();
      modelOptions = ensureCurrentModelOption(await listModels(), settings);
      modelProviders = settings.model.providers;
    } catch (error) {
      errorMessage = normalizeError(error);
    }
  }

  async function handleSaveMemorySettings(updatedMemory: MemoryConfigurationSettings) {
    if (!settings) return;
    try {
      const nextSettings = {
        ...settings,
        memory: updatedMemory,
      };
      settings = await updateSettings(nextSettings);
    } catch (error) {
      console.error("Failed to save memory configuration:", error);
      errorMessage = normalizeError(error);
    }
  }

  async function clearMemory() {
    if (!await confirmDanger(
      "Tout effacer ?",
      "Erase everything?",
      "Conversations, messages et mémoires locaux seront définitivement supprimés.",
      "Local conversations, messages and memories will be permanently deleted.",
    )) return;
    stopSpeaking();
    await resetMemory();
    if (cloudAuthenticated) {
      await loadBootstrap();
    } else {
      conversations = [];
      messages = [];
      memoriesList = [];
      activeConversation = null;
    }
  }

  async function changeModel(modelKey: string) {
    const model = modelOptions.find((item) => item.id === modelKey);
    if (!settings || !settingsDraft || !model || changingModel || modelKey === activeModelKey) return;
    if (!ensureCloudWriteAllowed("changer de modèle")) return;
    changingModel = true;
    errorMessage = "";
    try {
      settings = await selectModelRef(model.providerId, model.modelId);
      settingsDraft = cloneSettings(settings);
      await refreshRuntimeState();
      modelOptions = ensureCurrentModelOption(await listModels(), settings);
      modelProviders = settings.model.providers;
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      changingModel = false;
    }
  }

  async function selectModel(modelKey: string) {
    modelMenuOpen = false;
    modelSearchQuery = "";
    await changeModel(modelKey);
  }

  async function refreshModelsState(nextSettings = settings) {
    if (!nextSettings) return;
    settings = nextSettings;
    settingsDraft = cloneSettings(nextSettings);
    modelProviders = nextSettings.model.providers;
    modelOptions = ensureCurrentModelOption(await listModels(), nextSettings);
    await refreshRuntimeState();
  }

  async function testProvider(providerId: string) {
    modelProviderBusy = { ...modelProviderBusy, [providerId]: true };
    errorMessage = "";
    try {
      const draftKey = modelProviderKeyDrafts[providerId]?.trim() ?? "";
      if (draftKey) {
        await saveProviderKey(providerId);
      }
      const status = await testModelProvider(providerId);
      modelProviderStatus = { ...modelProviderStatus, [providerId]: status };
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      modelProviderBusy = { ...modelProviderBusy, [providerId]: false };
    }
  }

  async function refreshProviderCatalog(providerId: string) {
    if (!ensureCloudWriteAllowed("rafraîchir les modèles")) return;
    modelProviderBusy = { ...modelProviderBusy, [providerId]: true };
    errorMessage = "";
    try {
      const draftKey = modelProviderKeyDrafts[providerId]?.trim() ?? "";
      if (draftKey) {
        await saveProviderKey(providerId);
      }
      await refreshModelsState(await refreshModelCatalog(providerId));
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      modelProviderBusy = { ...modelProviderBusy, [providerId]: false };
    }
  }

  async function saveProviderKey(providerId: string) {
    const apiKey = modelProviderKeyDrafts[providerId]?.trim() ?? "";
    if (!apiKey || !ensureCloudWriteAllowed("ajouter une cle provider")) return;
    modelProviderBusy = { ...modelProviderBusy, [providerId]: true };
    errorMessage = "";
    try {
      await refreshModelsState(await setModelProviderApiKey(providerId, apiKey));
      modelProviderKeyDrafts = { ...modelProviderKeyDrafts, [providerId]: "" };
    } catch (error) {
      errorMessage = normalizeError(error);
      modelProviderStatus = {
        ...modelProviderStatus,
        [providerId]: {
          modelProvider: "mock",
          modelId: "unknown",
          modelReady: false,
          voiceReady: false,
          voiceStatus: null,
          endpoint: null,
          detail: errorMessage,
          checkedAt: new Date().toISOString()
        }
      };
    } finally {
      modelProviderBusy = { ...modelProviderBusy, [providerId]: false };
    }
  }

  async function clearProviderKey(providerId: string) {
    if (!ensureCloudWriteAllowed("retirer une cle provider")) return;
    modelProviderBusy = { ...modelProviderBusy, [providerId]: true };
    errorMessage = "";
    try {
      await refreshModelsState(await clearModelProviderApiKey(providerId));
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      modelProviderBusy = { ...modelProviderBusy, [providerId]: false };
    }
  }

  async function removeProvider(providerId: string) {
    if (!ensureCloudWriteAllowed("supprimer un provider")) return;
    modelProviderBusy = { ...modelProviderBusy, [providerId]: true };
    errorMessage = "";
    try {
      await refreshModelsState(await deleteModelProvider(providerId));
    } catch (error) {
      errorMessage = normalizeError(error);
    } finally {
      modelProviderBusy = { ...modelProviderBusy, [providerId]: false };
    }
  }

  const providerOptions = [
    { value: "mock", label: "Mock" },
    { value: "ollama", label: "Ollama" },
    { value: "llama-cpp", label: "llama.cpp" },
    { value: "openai", label: "OpenAI" },
    { value: "anthropic", label: "Anthropic" },
    { value: "google", label: "Google Gemini" },
    { value: "mistral", label: "Mistral" },
    { value: "openai-compatible", label: "OpenAI-compatible" }
  ];

  const addProviderDefaults = {
    "openai": { name: "OpenAI", endpoint: "https://api.openai.com/v1" },
    anthropic: { name: "Anthropic", endpoint: "https://api.anthropic.com/v1" },
    google: { name: "Google Gemini", endpoint: "https://generativelanguage.googleapis.com/v1beta" },
    mistral: { name: "Mistral", endpoint: "https://api.mistral.ai/v1" },
    "openai-compatible": { name: "Custom Provider", endpoint: "https://api.example.com/v1" },
    mock: { name: "Mock", endpoint: "" },
    ollama: { name: "Ollama", endpoint: "http://127.0.0.1:11434" },
    "llama-cpp": { name: "llama.cpp", endpoint: "http://127.0.0.1:8080" },
  } satisfies Record<ModelProviderKind, { name: string; endpoint: string }>;

  function openAddProvider(kind: ModelProviderKind = "openai") {
    const defaults = addProviderDefaults[kind];
    addProviderKind = kind;
    addProviderName = defaults.name;
    addProviderEndpoint = defaults.endpoint;
    addProviderApiKey = "";
    showAddProviderSheet = true;
  }

  $: if (addProviderDefaults?.[addProviderKind] && showAddProviderSheet) {
    const defaults = addProviderDefaults[addProviderKind];
    if (!addProviderName.trim() || providerOptions.some((option) => option.value === addProviderKind && addProviderName === option.label)) {
      addProviderName = defaults.name;
    }
    if (!addProviderEndpoint.trim()) addProviderEndpoint = defaults.endpoint;
  }

  async function createProviderFromSheet() {
    if (!ensureCloudWriteAllowed("ajouter un provider")) return;
    const now = new Date().toISOString();
    const idBase = addProviderKind === "openai-compatible" ? addProviderName : addProviderKind;
    const id = `${String(idBase).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "provider"}-${Date.now().toString(36)}`;
    const isLocal = addProviderKind === "mock" || addProviderKind === "ollama" || addProviderKind === "llama-cpp";
    const modelId = addProviderKind === "anthropic"
      ? "claude-sonnet-5"
      : addProviderKind === "google"
        ? "gemini-3.5-flash"
        : addProviderKind === "mistral"
          ? "mistral-small-4"
          : addProviderKind === "openai"
            ? "gpt-5.4-mini"
            : addProviderKind === "ollama"
              ? "gemma3:1b"
              : addProviderKind === "llama-cpp"
                ? "local-model"
                : "custom-model";
    const provider: ModelProviderConnection = {
      id,
      kind: addProviderKind,
      displayName: addProviderName.trim() || addProviderDefaults[addProviderKind].name,
      enabled: true,
      endpoint: addProviderEndpoint.trim() || null,
      authConfigured: Boolean(addProviderApiKey.trim()) || isLocal,
      models: [{
        providerId: id,
        providerKind: addProviderKind,
        modelId,
        label: modelId,
        family: addProviderName.trim() || addProviderDefaults[addProviderKind].name,
        local: isLocal,
        installed: isLocal || Boolean(addProviderApiKey.trim()),
        ready: isLocal || Boolean(addProviderApiKey.trim()),
      }],
      createdAt: now,
      updatedAt: now,
    };
    try {
      let nextSettings = await upsertModelProvider(provider);
      if (addProviderApiKey.trim()) {
        nextSettings = await setModelProviderApiKey(provider.id, addProviderApiKey.trim());
      }
      await refreshModelsState(nextSettings);
      showAddProviderSheet = false;
    } catch (error) {
      errorMessage = normalizeError(error);
    }
  }

  function openFilePicker() {
    fileInput?.click();
  }

  async function handleFiles(event: Event) {
    const target = event.currentTarget as HTMLInputElement;
    const files = Array.from(target.files ?? []);
    if (files.length === 0) return;
    const parsedFiles = files.map((file) => createAttachment(file));
    attachedFiles = [...attachedFiles, ...parsedFiles];
    target.value = "";
  }

  function createAttachment(file: File): AttachedFile {
    const id = createLocalId(file.name);
    return {
      id,
      name: file.name,
      size: file.size,
      type: file.type || DEFAULT_ATTACHMENT_MIME,
      file,
      mode: "local-reference",
      uploadStatus: "local",
      note: "Local reference",
    };
  }

  function removeAttachment(id: string) {
    attachedFiles = attachedFiles.filter((file) => file.id !== id);
  }

  async function uploadAttachmentToCloud(id: string) {
    const attachment = attachedFiles.find((file) => file.id === id);
    if (!attachment || attachment.uploadStatus === "uploading") return;
    if (!cloudAuthenticated) {
      attachedFiles = attachedFiles.map((file) =>
        file.id === id ? { ...file, uploadStatus: "failed", error: "Cloud login required" } : file,
      );
      return;
    }
    attachedFiles = attachedFiles.map((file) =>
      file.id === id ? { ...file, uploadStatus: "uploading", error: undefined } : file,
    );
    try {
      const uploaded: FileObject = await uploadCloudFile(attachment.file);
      attachedFiles = attachedFiles.map((file) =>
        file.id === id
          ? {
              ...file,
              mode: "cloud-object",
              uploadStatus: "uploaded",
              fileId: uploaded.id,
              type: uploaded.mimeType || file.type,
              note: "Cloud object",
              error: undefined,
            }
          : file,
      );
    } catch (error) {
      attachedFiles = attachedFiles.map((file) =>
        file.id === id
          ? {
              ...file,
              uploadStatus: "failed",
              error: normalizeError(error),
              note: "Upload failed",
            }
          : file,
      );
    }
  }

  function buildAttachmentRefs(files: AttachedFile[]): AttachmentRef[] {
    return files.map((file) => ({
      fileId: file.mode === "cloud-object" ? file.fileId ?? null : null,
      mode: file.mode,
      displayName: file.name,
      sizeBytes: file.size,
      mimeType: file.type || DEFAULT_ATTACHMENT_MIME,
    }));
  }

  function createLocalId(prefix: string): string {
    const randomValue =
      globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    return `${prefix}-${randomValue}`;
  }

  function releaseSpeakingAudioUrl(url = currentSpeakingAudioUrl) {
    if (!url) return;
    try {
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error("Failed to release speech audio URL", e);
    }
    if (currentSpeakingAudioUrl === url) {
      currentSpeakingAudioUrl = null;
    }
  }

  function stopSpeaking() {
    if (currentSpeakingAudio) {
      try {
        currentSpeakingAudio.pause();
        currentSpeakingAudio.removeAttribute("src");
        currentSpeakingAudio.load();
      } catch (e) {
        console.error("Failed to pause audio", e);
      }
      currentSpeakingAudio = null;
    }
    releaseSpeakingAudioUrl();
    assistantSpeaking = false;
  }

  async function speak(text: string) {
    if (!settings?.speakResponses || activeMode === "quiet") return;
    if (!voiceTextToSpeechReady) return;
    try {
      stopSpeaking();

      let voicePath: string | null = null;
      let speakerId: number | null = null;

      const conversationId = activeConversation?.id;
      const persId = conversationId ? conversationPersonalities[conversationId] : selectedPersonalityId;
      const pers = [...defaultPersonalities, ...customPersonalities].find(p => p.id === persId);

      if (pers && pers.voiceId && pers.voiceId !== "default") {
        const voice = [...defaultVoices, ...customVoices].find(v => v.id === pers.voiceId);
        if (voice) {
          voicePath = voice.path;
          speakerId = voice.speakerId || null;
        }
      } else {
        const voice = [...defaultVoices, ...customVoices].find(v => v.id === selectedVoiceId);
        if (voice) {
          voicePath = voice.path;
          speakerId = voice.speakerId || null;
        }
      }

      const result = await synthesizeSpeech(text, voicePath, speakerId);
      if (!result.audioBytes.length) return;
      const audioBlob = new Blob([new Uint8Array(result.audioBytes)], { type: result.mimeType });
      const audioUrl = URL.createObjectURL(audioBlob);
      const audio = new Audio(audioUrl);
      
      currentSpeakingAudio = audio;
      currentSpeakingAudioUrl = audioUrl;
      assistantSpeaking = true;
      voiceVolume = 0;

      audio.onended = () => {
        if (currentSpeakingAudio === audio) {
          assistantSpeaking = false;
          currentSpeakingAudio = null;
        }
        releaseSpeakingAudioUrl(audioUrl);
      };

      audio.onerror = () => {
        if (currentSpeakingAudio === audio) {
          assistantSpeaking = false;
          currentSpeakingAudio = null;
        }
        releaseSpeakingAudioUrl(audioUrl);
      };

      await audio.play();
    } catch (error) {
      console.error("Synthesize speech failed", error);
      assistantSpeaking = false;
      currentSpeakingAudio = null;
      releaseSpeakingAudioUrl();
    }
  }

  async function scrollToBottom(instant = false) {
    await tick();
    if (conversationContainer) {
      if (instant) {
        conversationContainer.style.scrollBehavior = "auto";
        conversationContainer.scrollTop = conversationContainer.scrollHeight;
        requestAnimationFrame(() => {
          if (conversationContainer) {
            conversationContainer.style.scrollBehavior = "smooth";
          }
        });
      } else {
        conversationContainer.style.scrollBehavior = "smooth";
        conversationContainer.scrollTop = conversationContainer.scrollHeight;
      }
    }
  }

  async function resizeComposer() {
    await tick();
    if (!composerInput) return;
    composerInput.style.height = "24px";
    composerInput.style.height = `${Math.min(composerInput.scrollHeight, 168)}px`;
  }

  function cloneSettings(value: AppSettings): AppSettings {
    return JSON.parse(JSON.stringify(value));
  }

  function normalizeError(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function formatRelativeTime(value: string): string {
    const date = new Date(value);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffSecs = Math.floor(diffMs / 1000);
    const diffMins = Math.floor(diffSecs / 60);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);

    if (diffSecs < 60) {
      return t.justNow;
    }
    if (diffMins < 60) {
      return `${diffMins}${t.minutesAbbr}`;
    }
    if (diffHours < 24) {
      return `${diffHours}${t.hoursAbbr}`;
    }
    if (diffDays === 1) {
      return t.yesterday;
    }
    if (diffDays < 7) {
      return `${diffDays}${t.daysAbbr}`;
    }
    const diffWeeks = Math.floor(diffDays / 7);
    if (diffWeeks < 4) {
      return `${diffWeeks}${t.weeksAbbr}`;
    }
    const diffMonths = Math.floor(diffDays / 30);
    return currentLanguage === "fr" ? `${diffMonths}mois` : `${diffMonths}mo`;
  }

  function formatTime(value: string): string {
    const date = new Date(value);
    const now = new Date();
    
    const isToday = date.toDateString() === now.toDateString();
    
    const yesterday = new Date(now);
    yesterday.setDate(now.getDate() - 1);
    const isYesterday = date.toDateString() === yesterday.toDateString();
    
    const locale = currentLanguage === "fr" ? "fr-FR" : "en-US";
    const timeStr = new Intl.DateTimeFormat(locale, {
      hour: "2-digit",
      minute: "2-digit",
    }).format(date);
    
    if (isToday) {
      return `${t.todayAt} ${timeStr}`;
    } else if (isYesterday) {
      return `${t.yesterdayAt} ${timeStr}`;
    }
    
    const diffTime = Math.abs(now.getTime() - date.getTime());
    const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));
    if (diffDays < 7) {
      const dayName = new Intl.DateTimeFormat(locale, { weekday: 'long' }).format(date);
      const capitalizedDay = dayName.charAt(0).toUpperCase() + dayName.slice(1);
      return `${capitalizedDay} ${t.at} ${timeStr}`;
    }
    
    const dateStr = new Intl.DateTimeFormat(locale, {
      day: "numeric",
      month: "long",
    }).format(date);
    return `${dateStr} ${t.at} ${timeStr}`;
  }

  async function copyMessageText(messageId: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedMessageId = messageId;
      setTimeout(() => {
        if (copiedMessageId === messageId) {
          copiedMessageId = null;
        }
      }, 2000);
    } catch (err) {
      console.error("Failed to copy message text", err);
    }
  }

  async function rememberMessage(message: ChatMessage) {
    const content = message.content.trim();
    if (!content) return;
    const id = crypto.randomUUID();
    const now = new Date().toISOString();
    let memory: LongTermMemoryItem = {
      id,
      clientId: id,
      content,
      category: message.role === "assistant" ? "technical" : "personal",
      scope: "user",
      status: "approved",
      sourceConversationId: message.conversationId || activeConversation?.id || null,
      sourceMessageIds: [message.id],
      pinned: false,
      salience: 0.65,
      lastUsedAt: null,
      createdAt: now,
      updatedAt: now,
    };
    try {
      memory = await upsertMemoryItem(memory);
      memoriesList = [memory, ...memoriesList.filter((item) => item.id !== memory.id)];
      saveMemories();
      await refreshMemoryIndexStatus();
    } catch (error) {
      collectionsSyncError = normalizeError(error);
    }
  }

  function handleFeedback(messageId: string, rating: 'good' | 'bad') {
    if (messageFeedback[messageId] === rating) {
      messageFeedback[messageId] = null;
    } else {
      messageFeedback[messageId] = rating;
    }
  }

  function formatFileSize(size: number): string {
    const units = ["B", "KB", "MB", "GB"];
    let value = size;
    let unitIndex = 0;
    while (value >= 1024 && unitIndex < units.length - 1) {
      value /= 1024;
      unitIndex += 1;
    }
    const precision = unitIndex === 0 || value >= 10 ? 0 : 1;
    return `${value.toFixed(precision)} ${units[unitIndex]}`;
  }

  function legacyUnusedEncodeWav(chunks: Float32Array[], sampleRate: number): Uint8Array {
    const samples = mergeAudioChunks(chunks);
    const bytesPerSample = 2;
    const headerBytes = 44;
    const buffer = new ArrayBuffer(headerBytes + samples.length * bytesPerSample);
    const view = new DataView(buffer);

    writeAscii(view, 0, "RIFF");
    view.setUint32(4, 36 + samples.length * bytesPerSample, true);
    writeAscii(view, 8, "WAVE");
    writeAscii(view, 12, "fmt ");
    view.setUint32(16, 16, true);
    view.setUint16(20, 1, true);
    view.setUint16(22, 1, true);
    view.setUint32(24, sampleRate, true);
    view.setUint32(28, sampleRate * bytesPerSample, true);
    view.setUint16(32, bytesPerSample, true);
    view.setUint16(34, 8 * bytesPerSample, true);
    writeAscii(view, 36, "data");
    view.setUint32(40, samples.length * bytesPerSample, true);

    let offset = headerBytes;
    for (const sample of samples) {
      const clamped = Math.max(-1, Math.min(1, sample));
      view.setInt16(offset, clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff, true);
      offset += bytesPerSample;
    }

    return new Uint8Array(buffer);
  }

  function mergeAudioChunks(chunks: Float32Array[]): Float32Array {
    const totalLength = chunks.reduce((total, chunk) => total + chunk.length, 0);
    const merged = new Float32Array(totalLength);
    let offset = 0;
    for (const chunk of chunks) {
      merged.set(chunk, offset);
      offset += chunk.length;
    }
    return merged;
  }

  function writeAscii(view: DataView, offset: number, value: string) {
    for (let index = 0; index < value.length; index += 1) {
      view.setUint8(offset + index, value.charCodeAt(index));
    }
  }

  function ensureCurrentModelOption(models: ModelOption[], currentSettings: AppSettings): ModelOption[] {
    const normalized = models.map((model) => ({
      ...model,
      id: model.id ?? modelOptionKey(model),
      provider: model.provider ?? model.providerKind,
    }));
    const active = currentSettings.model.activeModelRef;
    const activeKey = modelOptionKey(active);
    if (normalized.some((model) => model.id === activeKey)) return normalized;
    return [
      {
        ...active,
        id: activeKey,
        label: active.label || active.modelId,
        provider: active.providerKind,
        installed: false,
        ready: false,
      },
      ...normalized,
    ];
  }

  function conversationInitial(conversation: Conversation): string {
    return conversation.title.trim().charAt(0).toUpperCase() || "A";
  }

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  // Attachment preview state
  let previewFileObject: { name: string; mimeType: string; url: string; content?: string } | null = null;
  let previewLoading = false;
  let codePreviewHtml = "";
  let isCodePreviewOpen = false;

  function isMimeTypeText(mime: string): boolean {
    const clean = mime.toLowerCase();
    return clean.startsWith("text/") || 
           clean.includes("json") || 
           clean.includes("xml") || 
           clean.includes("javascript");
  }

  function isExtensionText(filename: string): boolean {
    const ext = filename.split(".").pop()?.toLowerCase();
    if (!ext) return false;
    const textExts = ["txt", "md", "csv", "py", "js", "ts", "rs", "css", "html", "json", "xml", "yml", "yaml", "ini", "conf", "sh", "bat", "sql"];
    return textExts.includes(ext);
  }

  async function previewAttachment(fileObj: AttachedFile) {
    previewLoading = true;
    try {
      const blob = fileObj.file;
      const url = URL.createObjectURL(blob);
      let content: string | undefined = undefined;
      
      const isText = isMimeTypeText(fileObj.type) || isExtensionText(fileObj.name);
      if (isText) {
        content = await blob.text();
      }
      
      previewFileObject = {
        name: fileObj.name,
        mimeType: fileObj.type,
        url,
        content
      };
    } catch (e) {
      errorMessage = "Erreur lors du chargement de l'aperçu: " + normalizeError(e);
    } finally {
      previewLoading = false;
    }
  }

  async function previewMessageAttachment(att: AttachmentRef) {
    if (!att.fileId) {
      errorMessage = "Ce fichier n'est pas disponible en aperçu (pas d'ID cloud).";
      return;
    }
    previewLoading = true;
    try {
      const blob = await downloadCloudFile(att.fileId);
      const url = URL.createObjectURL(blob);
      let content: string | undefined = undefined;
      
      const isText = isMimeTypeText(att.mimeType) || isExtensionText(att.displayName);
      if (isText) {
        content = await blob.text();
      }
      
      previewFileObject = {
        name: att.displayName,
        mimeType: att.mimeType,
        url,
        content
      };
    } catch (e) {
      errorMessage = "Impossible de charger le fichier du cloud: " + normalizeError(e);
    } finally {
      previewLoading = false;
    }
  }

  function closePreview() {
    if (previewFileObject) {
      URL.revokeObjectURL(previewFileObject.url);
      previewFileObject = null;
    }
  }
</script>

<svelte:window
  bind:innerWidth={windowWidth}
  on:keydown={handleGlobalKeydown}
  on:click={(e) => {
    if (showConversationMenu) {
      const target = e.target;
      if (target instanceof HTMLElement && !target.closest('.conversation-menu-container')) {
        showConversationMenu = false;
      }
    }
    if (activeSidebarMenuId) {
      const target = e.target;
      if (target instanceof HTMLElement && !target.closest('.sidebar-menu-container')) {
        activeSidebarMenuId = null;
      }
    }
    if (showCloudAuthPanel) {
      const target = e.target;
      if (target instanceof HTMLElement && !target.closest('.cloud-status-button') && !target.closest('.workspace-popover')) {
        showCloudAuthPanel = false;
      }
    }
  }}
/>

{#if showSplash && !isSpotlightMode}
  <SplashScreen dark={currentTheme === "dark"} progress={splashProgress} />
{/if}

{#if showCloudAuthPanel && cloudAuthenticated && !isSpotlightMode}
  <WorkspacePopover
    language={currentLanguage}
    organizations={cloudOrganizations}
    session={cloudSession}
    busy={cloudOrganizationBusy}
    error={collectionsSyncError}
    {sidebarOpen}
    {sidebarWidth}
    onClose={() => (showCloudAuthPanel = false)}
    onCreate={() => { showCreateOrgModal = true; showCloudAuthPanel = false; }}
    onSwitch={switchOrganizationFromUi}
  />
{/if}

{#if showCreateOrgModal && !isSpotlightMode}
  <CreateOrganizationModal
    language={currentLanguage}
    bind:name={newOrganizationName}
    busy={cloudOrganizationBusy}
    error={collectionsSyncError}
    onClose={() => (showCreateOrgModal = false)}
    onCreate={createOrganizationFromUi}
  />
{/if}

{#if showAddProviderSheet && !isSpotlightMode}
  <AddProviderModal
    language={currentLanguage}
    bind:kind={addProviderKind}
    bind:name={addProviderName}
    bind:endpoint={addProviderEndpoint}
    bind:apiKey={addProviderApiKey}
    writeLocked={cloudWriteLocked}
    options={providerOptions}
    defaults={addProviderDefaults}
    onClose={() => (showAddProviderSheet = false)}
    onSubmit={createProviderFromSheet}
  />
{/if}

{#if !isSpotlightMode && isTauriEnv}
  <div class="custom-titlebar" data-tauri-drag-region>
    <div class="titlebar-left" data-tauri-drag-region>
      <img src="/logo.png" alt="ARO" class="titlebar-logo" />
      <span class="titlebar-title">ARO</span>
    </div>
    <div class="titlebar-right" data-tauri-drag-region="false">
      {#if isTauriEnv}
        <button class="titlebar-win-btn" type="button" aria-label="Minimize" data-tauri-drag-region="false" on:mousedown={(e) => e.stopPropagation()} on:click={minimizeMainWindow}>
          <Minus size={14} />
        </button>
        <button class="titlebar-win-btn" type="button" aria-label="Maximize" data-tauri-drag-region="false" on:mousedown={(e) => e.stopPropagation()} on:click={toggleMaximizeMainWindow}>
          <Square size={12} />
        </button>
        <button class="titlebar-win-btn close" type="button" aria-label="Close" data-tauri-drag-region="false" on:mousedown={(e) => e.stopPropagation()} on:click={closeMainWindow}>
          <X size={15} />
        </button>
      {/if}
    </div>
  </div>
{/if}

{#if isSpotlightMode}
  <SpotlightPage
    {spotlightMessages}
    {currentTheme}
    {recording}
    bind:spotlightInputEl
    bind:spotlightInput
    {cloudWriteLocked}
    {recordingHint}
    {currentLanguage}
    {attachedFiles}
    {cloudAuthenticated}
    {t}
    {assistantSpeaking}
    {voiceVolume}
    bind:spotlightPersonalityMenuOpen
    bind:modelMenuOpen
    {activePersonality}
    {allPersonalities}
    {selectedPersonalityId}
    {modelRuntimeReady}
    {runtime}
    {modelReadinessLabel}
    {settings}
    {changingModel}
    {currentModelLabel}
    bind:modelSearchQuery
    {filteredModelOptions}
    {activeModelKey}
    {spotlightSending}
    {hideSpotlightWindow}
    {submitSpotlightMessage}
    {handleSpotlightKeydown}
    {cloudWriteDisabledTitle}
    {previewAttachment}
    {formatFileSize}
    {uploadAttachmentToCloud}
    {removeAttachment}
    {openFilePicker}
    {toggleSpotlightRecording}
    {stopSpeaking}
    {selectPersonality}
    {selectModel}
    {openSettings}
    {getWavePath}
    {expandToMainWindow}
    {startResizingWindow}
  />
{:else if !cloudAuthenticated}
  <CloudAuthPage
    language={currentLanguage}
    bind:mode={cloudAuthMode}
    invitationToken={cloudInvitationToken}
    bind:email={cloudAuthEmail}
    bind:password={cloudAuthPassword}
    bind:name={cloudAuthName}
    bind:organizationName={cloudAuthOrgName}
    bind:showPassword
    busy={cloudAuthBusy}
    error={cloudAuthError}
    successMessage={cloudAuthSuccessMessage}
    devTokenUrl={cloudAuthDevTokenUrl}
    onSubmit={submitCloudAuth}
    onToggleMode={toggleCloudAuthMode}
    onForgotPassword={handleForgotPassword}
  />
{:else}
<main
  class="shell"
  class:sidebar-collapsed={!sidebarOpen}
  class:settings-active={showSettings && Boolean(settingsDraft)}
  class:resizing={isResizing}
  style={showSettings && settingsDraft ? "grid-template-columns: 1fr;" : (!sidebarOpen ? "grid-template-columns: 66px 1fr;" : `grid-template-columns: ${sidebarWidth}px auto 1fr;`)}
>
  <div class="aurora-bg" aria-hidden="true">
    <div class="aurora-blob aurora-1"></div>
    <div class="aurora-blob aurora-2"></div>
    <div class="aurora-blob aurora-3"></div>
  </div>
  {#if !showSettings || !settingsDraft}
    <MainSidebar
      {sidebarOpen}
      onToggleSidebar={() => (sidebarOpen = !sidebarOpen)}
      labels={t}
      bind:searchQuery
      bind:activeSidebarMenuId
      bind:showCloudAuthPanel
      conversations={filteredConversations}
      {projects}
      {folders}
      {activeConversation}
      {sendingByConversation}
      {cloudWriteLocked}
      {cloudAuthenticated}
      {cloudSyncStatus}
      {cloudSession}
      {cloudStatusShortLabel}
      {runtime}
      {isConversationSending}
      {formatRelativeTime}
      {cloudWriteDisabledTitle}
      onStartConversation={startConversation}
      onOpenConversation={openConversation}
      onRenameConversation={renameConversation}
      onRemoveConversation={removeConversation}
      onOpenSettings={openSettings}
      onOpenCreateProjectModal={handleOpenCreateProjectModal}
      onOpenCreateFolderModal={handleOpenCreateFolderModal}
      onEditProject={handleEditProject}
      onDeleteProject={handleDeleteProject}
      onEditFolder={handleEditFolder}
      onDeleteFolder={handleDeleteFolder}
      onOpenMoveModal={handleOpenMoveModal}
      onDropConversationToFolder={handleDropConversationToFolder}
      onDropConversationToProject={handleDropConversationToProject}
      onMoveConversation={handleMoveConversation}
      onExportProject={handleExportProject}
      onCleanEmptyConversations={handleCleanEmptyConversations}
    />
  {/if}

  {#if sidebarOpen && !showSettings}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="sidebar-resizer"
      class:resizing={isResizing}
      role="separator"
      aria-label="Resize sidebar"
      on:mousedown={startResizing}
    ></div>
  {/if}

  <section class="workspace">
    <div class="workspace-content">
    {#if showSettings && settingsDraft}
      <div
        class="settings-page"
        class:mobile-sidebar={windowWidth < 768 && settingsMobileView === "sidebar"}
        class:mobile-content={windowWidth < 768 && settingsMobileView === "content"}
      >
        <SettingsSidebar
          bind:activeTab={activeSettingsTab}
          bind:open={settingsSidebarOpen}
          bind:width={settingsSidebarWidth}
          language={currentLanguage}
          labels={t}
          onClose={closeSettings}
        />

        <!-- Content Area -->
        <main class="settings-content">
          <div class="settings-content-header">
            {#if windowWidth < 768}
              <button
                class="settings-content-back-btn"
                type="button"
                on:click={() => (settingsMobileView = "sidebar")}
              >
                <ArrowLeft size={14} />
                <span>{t.back}</span>
              </button>
              <span class="settings-breadcrumb-title-mobile">
                {currentSettingsTabLabel}
              </span>
            {:else}
              <span class="settings-breadcrumb-title">
                {t.settings} &rsaquo; {currentSettingsTabLabel}
              </span>
            {/if}
          </div>
          {#if cloudWriteLocked}
            <div class="settings-readonly-banner" role="status">
              <Lock size={14} />
              <span>{cloudWriteDisabledTitle("modifier les reglages")}</span>
            </div>
          {/if}
          <div class="settings-content-scroll">
{#if activeSettingsTab === "models"}
              <ModelsSettings
                bind:settingsDraft
                language={currentLanguage}
                labels={t}
                {currentModelLabel}
                {currentModel}
                {modelProviders}
                {modelProviderBusy}
                {modelProviderStatus}
                bind:modelProviderKeyDrafts
                bind:modelSearchQuery
                {filteredModelOptions}
                {activeModelKey}
                writeLocked={cloudWriteLocked}
                onOpenAddProvider={openAddProvider}
                onTestProvider={testProvider}
                onRefreshProviderCatalog={refreshProviderCatalog}
                onSaveProviderKey={saveProviderKey}
                onClearProviderKey={clearProviderKey}
                onRemoveProvider={removeProvider}
                onSelectModel={selectModel}
                onAutosave={autosaveSettings}
              />
            {/if}

{#if activeSettingsTab === "voice"}
              <VoiceSettings
                bind:settingsDraft
                language={currentLanguage}
                theme={currentTheme}
                labels={t}
                {runtime}
                {modelRuntimeReady}
                {modelReadinessLabel}
                {voiceSpeechToTextReady}
                {voiceSpeechToTextStatus}
                {speechReadinessLabel}
                {voiceWakeModelReady}
                {wakeWordEnabled}
                {wakeWordReadinessLabel}
                {voiceTextToSpeechReady}
                {voiceTextToSpeechStatus}
                {ttsReadinessLabel}
                {voiceReadinessIssue}
                bind:wakeWordDraftEnabled
                {sttOptions}
                {ttsOptions}
                voices={allVoices}
                {selectedVoiceId}
                writeLocked={cloudWriteLocked}
                {firstVoiceIssue}
                {formatTime}
                writeDisabledTitle={(action) => cloudWriteDisabledTitle(action) ?? null}
                onAutosave={autosaveSettings}
                onOpenCreateVoice={openCreateVoice}
                onOpenEditVoice={openEditVoice}
                onDeleteVoice={deleteVoice}
                onSelectVoice={selectVoice}
              />
            {/if}

{#if activeSettingsTab === "paths"}
              <PathsSettings
                bind:settingsDraft
                language={currentLanguage}
                labels={t}
                writeLocked={cloudWriteLocked}
                onAutosave={autosaveSettings}
              />
            {/if}

{#if activeSettingsTab === "search"}
              <SearchSettings
                bind:settingsDraft
                language={currentLanguage}
                onAutosave={autosaveSettings}
              />
            {/if}

{#if activeSettingsTab === "system"}
              <SystemSettings
                labels={t}
                {runtime}
                writeLocked={cloudWriteLocked}
                writeDisabledTitle={(action) => cloudWriteDisabledTitle(action) ?? null}
                onRefreshRuntime={refreshRuntime}
                onClearMemory={clearMemory}
              />
            {/if}

{#if activeSettingsTab === "system-prompt"}
              <SystemPromptSettings
                {t}
                {currentLanguage}
                {currentTheme}
                bind:activeSettingsTab
                bind:customSystemPromptsEnabled
                {cloudWriteLocked}
                {activeColor}
                {activeColorLight}
                bind:selectedPromptMode
                {promptPreviewPersonality}
                {visibleInstructionPresets}
                bind:agiIdentity
                bind:agiRules
                bind:agiFormatting
                {compiledPromptPreview}
                bind:settingsDraft
                {allPersonalities}
                {selectedPersonalityId}
                {saveCustomSystemPrompts}
                {cloudWriteDisabledTitle}
                {applyAgiPreset}
                {saveAgiPrompts}
                {resetSelectedPromptToDefault}
                {autosaveSettings}
                {compileSystemPrompt}
              />
            {/if}

{#if activeSettingsTab === "instructions"}
              <InstructionsSettings
                {t}
                {currentLanguage}
                {currentTheme}
                {cloudWriteLocked}
                bind:customInstructionsEnabled
                {allPersonalities}
                {selectedPersonalityId}
                {cloudWriteDisabledTitle}
                {openCreatePersonality}
                {saveInstructions}
                {selectPersonality}
                {openEditPersonality}
                {deletePersonality}
              />
            {/if}

{#if activeSettingsTab === "memory"}
              <MemorySettings
                {t}
                {currentLanguage}
                {currentTheme}
                {memoriesList}
                {episodesList}
                {memoryIndex}
                {memoryIndexBusy}
                {memoryEntryLimit}
                bind:memorySearchQuery
                bind:selectedMemoryFilter
                bind:showAddMemoryInline
                bind:newMemoryText
                bind:newMemoryCategory
                bind:newMemorySalience
                bind:newMemoryPinned
                bind:editingMemoryId
                bind:editingMemoryText
                {refreshMemoryIndexStatus}
                {reindexMemories}
                {addMemory}
                {updateMemory}
                {deleteMemory}
                {togglePinMemory}
                onRefreshEpisodes={refreshEpisodes}
                memorySettings={settings?.memory}
                onSaveMemorySettings={handleSaveMemorySettings}
              />
            {/if}

{#if activeSettingsTab === "permissions"}
              <PermissionsSettings
                {t}
                {currentLanguage}
                {permissionProfiles}
                bind:activePermissionProfileId
                {permissionsLoading}
                {permissionsSaving}
                {permissionsStatus}
                {permissionsError}
                bind:permReadFile
                bind:permWriteFile
                bind:permExecuteCommands
                bind:permCommandApprovalMode
                bind:permNetworkAccess
                bind:permRedactSecrets
                {permAllowedDomains}
                {permAllowedPaths}
                bind:newDomainInput
                bind:newPathInput
                {cloudWriteLocked}
                {cloudWriteDisabledTitle}
                {savePermissionProfile}
                {selectPermissionProfile}
                {addAllowedDomain}
                {removeAllowedDomain}
                {addAllowedPath}
                {removeAllowedPath}
              />
            {/if}

{#if activeSettingsTab === "agents"}
              <AgentsSettings
                language={currentLanguage}
                writeLocked={cloudWriteLocked}
                agents={customAgentsList}
                onStartCreateAgent={openCreateAgentModal}
                onStartEditAgent={openEditAgentModal}
                onDeleteAgent={deleteCustomAgentFromPanel}
              />
            {/if}

{#if activeSettingsTab === "preferences"}
              <PreferencesSettings
                labels={t}
                bind:themeDraft
                bind:languageDraft
                {themeOptions}
                {languageOptions}
                onAutosave={autosaveSettings}
              />
            {/if}

{#if activeSettingsTab === "shortcuts"}
              <ShortcutsSettings
                language={currentLanguage}
                {shortcuts}
                bind:recordingShortcutFor
                {formatShortcut}
              />
            {/if}

{#if activeSettingsTab === "skills"}
              <SkillsSettings
                theme={currentTheme}
                bind:skillSearchQuery
                bind:selectedCategoryFilter
                {filteredSkills}
                {uniqueCategories}
                onOpenSkillForm={openSkillForm}
                onToggleSkill={toggleSkill}
                onDeleteSkill={deleteSkill}
              />
            {/if}

{#if activeSettingsTab === "plugins"}
              <PluginsSettings
                {currentTheme}
              />
            {/if}

{#if showCreateSkillModal}
              <SkillEditorModal
                {currentTheme}
                {editingSkillId}
                bind:skillFormName
                bind:skillFormDesc
                bind:skillFormIcon
                bind:skillFormCategory
                bind:skillFormGroupId
                bind:skillFormContent
                {userSkillGroups}
                {closeSkillForm}
                {handleCreateOrUpdateSkill}
              />
            {/if}



            <!-- MCP Server Configuration Modal -->

            <!-- Hooks / Webhooks Configuration Modal -->

            <!-- Scheduler Task Configuration Modal -->

{#if activeSettingsTab === "mcp"}
              <McpSettings
                {currentTheme}
                mcpServers={allMcpServers}
                bind:selectedMcpServerId
                bind:showMcpModal
                {mcpModalMode}
                bind:mcpFormName
                bind:mcpFormType
                bind:mcpFormCommand
                bind:mcpFormArgs
                bind:mcpFormUrl
                bind:mcpFormEnv
                {openAddMcpModal}
                {openEditMcpModal}
                {deleteMcpServer}
                {toggleMcpServer}
                {addPresetMcpServer}
                {addMcpEnvVar}
                {removeMcpEnvVar}
                {handleSaveMcpServer}
              />
            {/if}

{#if activeSettingsTab === "hooks"}
              <HooksSettings
                theme={currentTheme}
                {hooks}
                bind:showHooksModal
                {hooksModalMode}
                bind:hookFormName
                bind:hookFormUrl
                bind:hookFormSecret
                bind:hookFormEvents
                onOpenAddHookModal={openAddHookModal}
                onOpenEditHookModal={openEditHookModal}
                onToggleHookFormEvent={toggleHookFormEvent}
                onSaveHook={handleSaveHook}
                onTestHook={testHook}
                onDeleteHook={deleteHook}
                onToggleHook={toggleHook}
              />
            {/if}

{#if activeSettingsTab === "scheduler"}
              <SchedulerSettings
                theme={currentTheme}
                {scheduledTasks}
                bind:showSchedulerModal
                {schedulerModalMode}
                bind:schedulerFormName
                bind:schedulerFormPrompt
                bind:schedulerFormType
                bind:schedulerFormDuration
                bind:schedulerFormCron
                onOpenAddSchedulerModal={openAddSchedulerModal}
                onOpenEditSchedulerModal={openEditSchedulerModal}
                onSaveTask={handleSaveTask}
                onTriggerTaskImmediately={triggerTaskImmediately}
                onDeleteTask={deleteTask}
                onToggleTask={toggleTask}
              />
            {/if}

{#if activeSettingsTab === "monitoring"}
              <MonitoringSettings
                labels={t}
                language={currentLanguage}
                {monCpu}
                {monRam}
                {monRamMax}
                {monGpu}
                {monGpuVram}
                {monGpuVramMax}
                {cpuHistory}
                {ramHistory}
                {gpuHistory}
                {lastResponseTime}
                {avgResponseTime}
                {lastTokenCount}
                {lastTokenSpeed}
                {totalTokensGenerated}
                {performanceHistory}
                {activityWeeks}
              />
            {/if}

{#if activeSettingsTab === "profile"}
              <ProfileSettings
                labels={t}
                language={currentLanguage}
                bind:userProfile
                {apiKeys}
                bind:apiKeyNameDraft
                {cloudSession}
                writeLocked={cloudWriteLocked}
                writeDisabledTitle={(action) => cloudWriteDisabledTitle(action) ?? null}
                ensureWriteAllowed={ensureCloudWriteAllowed}
                {getInitials}
                onSaveUserProfile={saveUserProfile}
                onGenerateApiKey={generateApiKey}
                onToggleKeyVisibility={toggleKeyVisibility}
                onRevokeApiKey={revokeApiKey}
                onCloseSettings={closeSettings}
                onDisconnectCloud={disconnectCloud}
              />
            {/if}

{#if activeSettingsTab === "organization"}
              <OrganizationSettings
                labels={t}
                language={currentLanguage}
                {currentOrganizationRole}
                bind:activeOrg
                {userProfile}
                {orgMembers}
                {filteredMembers}
                {orgTeams}
                bind:memberSearchQuery
                bind:showInviteModal
                bind:showCreateTeamModal
                bind:teamFormMembers
                writeLocked={cloudWriteLocked}
                writeDisabledTitle={(action) => cloudWriteDisabledTitle(action) ?? null}
                ensureWriteAllowed={ensureCloudWriteAllowed}
                {getInitials}
                isCurrentMember={(member) => isCurrentOrgMember(member, cloudSession?.user.id)}
                invitationStatusLabel={(member) => organizationInvitationStatusLabel(member, currentLanguage)}
                onSaveOrgInfo={saveOrgInfo}
                onUpdateMemberRole={updateMemberRole}
                onRemoveMember={removeMember}
                onDeleteTeam={deleteTeam}
              />
            {/if}
          </div>


        </main>
      </div>
    {:else}
<ConversationTopbar
      {activeConversation}
      {activeProject}
      {conversationPersonalities}
      {selectedPersonalityId}
      personalities={allPersonalities}
      conversationCustomAgents={conversationCustomAgents}
      customAgentsList={customAgentsList}
      theme={currentTheme}
      language={currentLanguage}
      labels={t}
      bind:showTopbarPersonalityDropdown
      bind:showConversationMenu
      bind:showRightPanel
      attachedFileCount={attachedFiles.length}
      {cloudWriteLocked}
      {cloudWriteDisabledTitle}
      {ensureCloudWriteAllowed}
      onSelectConversationPersonality={selectConversationPersonality}
      onSelectConversationCustomAgent={selectConversationCustomAgent}
      onRenameConversation={renameConversation}
      onRemoveConversation={removeConversation}
      onMoveConversation={handleOpenMoveModal}
      onExportConversation={handleExportConversation}
      onOpenMemorySettings={() => openSettings("memory")}
    />

{#if arenaMode}
      <ArenaView
        bind:arenaMode
        bind:arenaModelA
        bind:arenaModelB
        bind:modelAMenuOpen
        bind:modelBMenuOpen
        modelOptions={modelOptions}
        {arenaHistory}
        missingLabel={t.missingText}
        onVote={castArenaVote}
      />
    {:else}
<ConversationView
      bind:conversationContainer
      bind:messagesEnd
      {loading}
      {messages}
      labels={t}
      language={currentLanguage}
      theme={currentTheme}
      personalities={allPersonalities}
      {selectedPersonalityId}
      {cloudWriteLocked}
      {cloudWriteDisabledTitle}
      {recording}
      {assistantSpeaking}
      {voiceVolume}
      {wakeWordEnabled}
      {voiceStateLabel}
      {recordingHint}
      {voiceStateHint}
      runtimeDetail={runtime?.detail}
      {modelRuntimeReady}
      {voiceSpeechToTextReady}
      {voiceWakeModelReady}
      {voiceTextToSpeechReady}
      speakResponses={Boolean(settings?.speakResponses)}
      speechToTextIssue={firstVoiceIssue(voiceSpeechToTextStatus)}
      textToSpeechIssue={firstVoiceIssue(voiceTextToSpeechStatus)}
      {modelReadinessLabel}
      {speechReadinessLabel}
      {wakeWordReadinessLabel}
      textToSpeechReadinessLabel={ttsReadinessLabel}
      {voiceModeOptions}
      {voiceInputMode}
      {voiceHandsFreeArmed}
      {expandedMessageSteps}
      {expandedStepDetails}
      {copiedMessageId}
      {messageFeedback}
      {editingMessageId}
      bind:editingMessageText
      {sending}
      {getWavePath}
      {formatTime}
      {formatFileSize}
      onSendUserPrompt={sendUserPrompt}
      onToggleRecording={toggleRecording}
      onStopSpeaking={stopSpeaking}
      onSetVoiceInputMode={setVoiceInputMode}
      onSelectPersonality={selectPersonality}
      {projects}
      {folders}
      {pendingProjectId}
      {pendingFolderId}
      showDestinationPicker={!activeConversation}
      onSelectDestination={selectPendingDestination}
      onToggleMessageSteps={toggleMessageSteps}
      onToggleStepDetails={toggleStepDetails}
      onPreviewAttachment={previewMessageAttachment}
      onCopyMessage={copyMessageText}
      onRememberMessage={rememberMessage}
      onFeedback={handleFeedback}
      onStartEdit={startEdit}
      onCancelEdit={cancelEdit}
      onSaveEdit={saveEdit}
    />
    {/if}

    {#if showNetworkAlert}
      <div class="network-alert-banner">
        <div class="network-alert-content">
          <span class="network-alert-icon">🌐</span>
          <span class="network-alert-text">
            {currentLanguage === "fr" 
              ? "La recherche Web a échoué car l'accès réseau est désactivé." 
              : "Web search failed because network access is disabled."}
          </span>
        </div>
        <div class="network-alert-actions">
          <button 
            type="button" 
            class="network-alert-btn-enable" 
            on:click={async () => {
              permNetworkAccess = true;
              await savePermissionProfile();
              showNetworkAlert = false;
            }}
          >
            {currentLanguage === "fr" ? "Activer l'accès" : "Enable access"}
          </button>
          <button 
            type="button" 
            class="network-alert-btn-close" 
            on:click={() => {
              localStorage.setItem("networkAlertDismissed", "true");
              showNetworkAlert = false;
            }}
          >
            {currentLanguage === "fr" ? "Fermer" : "Close"}
          </button>
        </div>
      </div>
    {/if}

<Composer
      labels={t}
      language={currentLanguage}
      bind:errorMessage
      bind:input
      bind:fileInput
      bind:composerInput
      {attachedFiles}
      {cloudWriteLocked}
      {cloudAuthenticated}
      messagesCount={messages.length}
      {webAccess}
      {voiceModeOptions}
      {voiceInputMode}
      {voiceHandsFreeArmed}
      {recording}
      {assistantSpeaking}
      {voiceVolume}
      {wakeWordEnabled}
      {recordingHint}
      {voiceStateLabel}
      {voiceStateHint}
      {modelRuntimeReady}
      {voiceSpeechToTextReady}
      {voiceWakeModelReady}
      {modelReadinessLabel}
      {speechReadinessLabel}
      {wakeWordReadinessLabel}
      settingsAvailable={Boolean(settings)}
      {changingModel}
      runtimeDetail={runtime?.detail}
      {currentModelLabel}
      bind:modelMenuOpen
      bind:modelSearchQuery
      modelOptions={filteredModelOptions}
      {activeModelKey}
      bind:arenaMode
      {arenaSending}
      {arenaModelA}
      {arenaModelB}
      {sending}
      {cloudWriteDisabledTitle}
      {webAccessTitle}
      {webAccessAriaLabel}
      {formatFileSize}
      {getWavePath}
      onSubmitMessage={submitMessage}
      onFilesChange={handleFiles}
      onPreviewAttachment={previewAttachment}
      onUploadAttachment={uploadAttachmentToCloud}
      onRemoveAttachment={removeAttachment}
      onResizeComposer={resizeComposer}
      onOpenFilePicker={openFilePicker}
      onToggleWebAccess={toggleWebAccess}
      onSetVoiceInputMode={setVoiceInputMode}
      onToggleRecording={toggleRecording}
      onStopSpeaking={stopSpeaking}
      onSelectModel={selectModel}
      onOpenModelSettings={() => openSettings("models")}
      workspaceMentionEntries={workspaceMentionEntries}
      activePermissionMode={activePermissionPreset}
      {activePermissionLabel}
      {permissionProfiles}
      {activePermissionProfileId}
      onSelectPermissionPreset={handleSelectPermissionPreset}
      onSelectPermissionProfile={handleSelectPermissionProfile}
      onOpenPermissionSettings={() => openSettings("permissions")}
      destinationLabel={pendingDestinationLabel}
      showDestinationBadge={!activeConversation}
      onDestinationClick={() => {
        try {
          window.dispatchEvent(new CustomEvent("aro:open-destination-picker"));
        } catch {
          // ignore
        }
      }}
    />
    {/if}
    </div>

    {#if showRightPanel && (!showSettings || !settingsDraft)}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="right-panel-resizer"
        class:resizing={isResizingRightPanel}
        role="separator"
        aria-label="Resize right panel"
        on:mousedown={startResizingRightPanel}
      ></div>
      <RightPanel
        activeConversationId={activeConversation?.id ?? null}
        artifacts={contextArtifacts}
        contextSources={contextSources}
        visibleSources={visibleContextSources}
        attachedFiles={attachedFiles}
        agentInboxTotals={agentInboxTotals}
        agentInboxLanes={agentInboxLanes}
        agentRunsBusy={agentRunsBusy}
        agentActionBusy={agentActionBusy}
        selectedAgentRunView={selectedAgentRunView}
        personalities={customAgentsList}
        onToggleAgentLane={toggleAgentLane}
        onSetAgentLaneAction={setAgentLaneAction}
        onUpdateAgentLanePriority={updateAgentLanePriority}
        onOpenAgentRun={openAgentRun}
        onStartAgentChat={startAgentChatFromPanel}
        onStartCreateAgent={openCreateAgentModal}
        onStartEditAgent={openEditAgentModal}
        onDeleteAgent={deleteCustomAgentFromPanel}
        bind:width={rightPanelWidth}
        onPreviewFile={previewAttachment}
        language={currentLanguage}
        onRequestAiPlan={handleRequestAiPlan}
        aiPlanBusy={isAiPlanGenerating}

        onClose={() => (showRightPanel = false)}
      />

    {/if}
  </section>
</main>
{/if}
<PreviewOverlays
  file={previewFileObject}
  codeOpen={isCodePreviewOpen}
  codeHtml={codePreviewHtml}
  loading={previewLoading}
  language={currentLanguage}
  theme={currentTheme}
  onCloseFile={closePreview}
  onCloseCode={() => (isCodePreviewOpen = false)}
/>

{#if showCommandPalette}
  <CommandPalette
    language={currentLanguage}
    bind:search={commandPaletteSearch}
    bind:selectedIndex={commandPaletteSelectedIndex}
    results={commandPaletteFilteredResults}
    bind:inputElement={commandPaletteInput}
    onClose={toggleCommandPalette}
    onKeydown={handleCommandPaletteKeydown}
    onExecute={executeCommandPaletteItem}
    {formatShortcut}
  />
{/if}

{#if renamingConversationObject}
  <RenameConversationModal
    title={t.renameConversationModalTitle}
    label={t.newTitleLabel}
    placeholder={t.enterTitlePlaceholder}
    cancelLabel={t.cancel}
    saveLabel={t.save}
    bind:value={renameInputValue}
    writeLocked={cloudWriteLocked}
    disabledTitle={cloudWriteDisabledTitle("renommer une conversation") ?? undefined}
    {focusOnMount}
    onClose={() => (renamingConversationObject = null)}
    onSave={saveRename}
  />
{/if}

{#if showCreateProjectModal}
  <CreateProjectModal
    {projectToEdit}
    writeLocked={cloudWriteLocked}
    disabledTitle={cloudWriteDisabledTitle("enregistrer un projet") ?? undefined}
    onClose={() => {
      showCreateProjectModal = false;
      projectToEdit = null;
    }}
    onSave={handleSaveProject}
  />
{/if}

{#if showCreateFolderModal}
  <CreateFolderModal
    {folderToEdit}
    {projects}
    defaultProjectId={folderDefaultProjectId}
    writeLocked={cloudWriteLocked}
    disabledTitle={cloudWriteDisabledTitle("enregistrer un dossier") ?? undefined}
    onClose={() => {
      showCreateFolderModal = false;
      folderToEdit = null;
      folderDefaultProjectId = null;
    }}
    onSave={handleSaveFolder}
  />
{/if}

{#if showMoveModal && conversationToMove}
  <MoveToProjectFolderModal
    conversation={conversationToMove}
    {projects}
    {folders}
    writeLocked={cloudWriteLocked}
    onClose={() => {
      showMoveModal = false;
      conversationToMove = null;
    }}
    onMove={handleMoveConversation}
  />
{/if}

{#if showInviteModal}
  <InviteMemberModal
    labels={t}
    bind:name={inviteFormName}
    bind:email={inviteFormEmail}
    bind:role={inviteFormRole}
    writeLocked={cloudWriteLocked}
    disabledTitle={cloudWriteDisabledTitle("inviter un membre") ?? undefined}
    onClose={() => (showInviteModal = false)}
    onInvite={inviteMember}
  />
{/if}

{#if showPersonalityModal}
  <PersonalityModal
    editingId={editingPersonalityId}
    language={currentLanguage}
    theme={currentTheme}
    bind:name={personalityFormName}
    bind:description={personalityFormDesc}
    bind:prompt={personalityFormPrompt}
    bind:icon={personalityFormIcon}
    bind:color={personalityFormColor}
    bind:temperature={personalityFormTemperature}
    bind:voiceId={personalityFormVoiceId}
    voices={allVoices}
    cancelLabel={t.cancel}
    writeLocked={cloudWriteLocked}
    writeDisabledTitle={cloudWriteDisabledTitle("modifier une personnalite") ?? null}
    onClose={() => (showPersonalityModal = false)}
    onSubmit={savePersonality}
  />
{/if}

{#if showVoiceModal}
  <VoiceProfileModal
    editingId={editingVoiceId}
    language={currentLanguage}
    theme={currentTheme}
    bind:name={voiceFormName}
    bind:description={voiceFormDesc}
    bind:path={voiceFormPath}
    bind:speakerId={voiceFormSpeakerId}
    bind:voiceLanguage={voiceFormLanguage}
    bind:color={voiceFormColor}
    cancelLabel={t.cancel}
    writeLocked={cloudWriteLocked}
    writeDisabledTitle={cloudWriteDisabledTitle("modifier une voix") ?? null}
    onClose={() => (showVoiceModal = false)}
    onSubmit={saveVoice}
  />
{/if}

{#if showCreateTeamModal}
  <CreateTeamModal
    labels={t}
    bind:name={teamFormName}
    bind:description={teamFormDesc}
    bind:selectedMemberIds={teamFormMembers}
    members={orgMembers}
    writeLocked={cloudWriteLocked}
    disabledTitle={cloudWriteDisabledTitle("creer une equipe") ?? undefined}
    onClose={() => (showCreateTeamModal = false)}
    onCreate={createTeam}
  />
{/if}

{#if showAgentModal}
  <AgentModal
    language={currentLanguage}
    writeLocked={cloudWriteLocked}
    agent={editingAgentObj}
    personalities={allPersonalities}
    modelProviders={modelProviders}
    modelOptions={modelOptions}
    permissionProfiles={permissionProfiles}
    userSkills={userSkills}
    userPlugins={userPlugins}
    onClose={() => { showAgentModal = false; editingAgentObj = null; }}
    onSave={saveCustomAgentFromPanel}
  />
{/if}

{#if showQuickOpenModal}
  <QuickOpenModal
    conversationId={activeConversation?.id}
    language={currentLanguage}
    onClose={() => (showQuickOpenModal = false)}
    onSelectFile={(filePath) => {
      showQuickOpenModal = false;
      showRightPanel = true;
      // Différé : laisse RightPanel se monter (et son listener
      // aro:open-workspace-file s'enregistrer) avant d'émettre.
      setTimeout(() => {
        try {
          window.dispatchEvent(
            new CustomEvent("aro:open-workspace-file", { detail: { path: filePath } })
          );
        } catch {
          addNotificationToast({
            type: "info",
            title: "Fichier sélectionné",
            body: filePath,
          });
        }
      }, 0);
    }}
  />
{/if}

<NotificationToastStack
  notifications={toastNotifications}
  onDismiss={dismissNotificationToast}
  onOpenConversation={(convId) => {
    const target = conversations.find((c) => c.id === convId);
    if (target) openConversation(target);
  }}
/>

{#if lightboxImageUrl}
  <ImageLightboxModal
    imageUrl={lightboxImageUrl}
    title={lightboxImageTitle}
    language={currentLanguage}
    onClose={() => {
      lightboxImageUrl = "";
      lightboxImageTitle = "";
    }}
  />
{/if}

<ConfirmModal language={currentLanguage} />

