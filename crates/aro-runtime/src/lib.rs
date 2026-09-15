mod providers;
mod security;
pub mod context_manager;

pub use context_manager::*;

use aro_agent::{AgentRuntime, EnvironmentSnapshot};
use aro_core::{
    compute_initial_salience, compute_memory_utility, compute_recency_decay, is_memory_tool,
    normalize_tool_id, AgentAction, AgentActionType, AgentContextItem, AgentLane, AgentLaneStatus,
    AgentLaneView, AgentOrchestratorSnapshot, AgentRun, AgentRunPriority, AgentRunStartRequest,
    AgentRunStatus, AgentRunView, AgentStep, AgentStepKind, AgentStepStatus, AroError, AroResult,
    AssistantMode, ChatMessage, ContextPack, ContextSource, Conversation, EpisodeSummary, Folder,
    LongTermMemory, MemoryCategory, MessageRole, ModelResponseFormat, Project, RuntimeStatus,
    ScoredMemory, SendMessageRequest, SendMessageResponse, ToolExecutionRequest, ToolExecutionResult,
    ToolExecutionStatus, WebAccessMode, WebFetchRequest, TOOL_CORE_AGENT_DELEGATE,
    TOOL_CORE_AGENT_SPAWN, TOOL_CORE_AGENT_STATUS, TOOL_CORE_CONNECTOR_CALL,
    TOOL_CORE_CONNECTOR_LIST, TOOL_CORE_CONTEXT_SEARCH, TOOL_CORE_MCP_CALL, TOOL_CORE_MEMORY_DELETE,
    TOOL_CORE_MEMORY_FORGET, TOOL_CORE_MEMORY_LIST, TOOL_CORE_MEMORY_RECALL, TOOL_CORE_MEMORY_SAVE,
    TOOL_CORE_MEMORY_SEARCH, TOOL_CORE_MEMORY_UPDATE, TOOL_CORE_SKILL_INVOKE, TOOL_CORE_SKILL_LIST,
    TOOL_CORE_WEB_PAGE_READ,
};
use aro_memory::SqliteMemoryStore;
use aro_tools::{extract_urls, result_context_items, ToolExecutor, WebAccessPolicy};
use aro_vector::{
    memories_to_context_sources, MemoryIndexStatus, MemoryReindexReport, MemoryVectorScope,
    MemoryVectorService,
};
use chrono::Utc;
use serde_json::{json, Value};

// No step limit: agent tool loops run until the model returns a final/pause
// action (or a validation/provider error). `max_steps` in requests is
// accepted for backward compatibility but no longer bounds execution.

pub use providers::{
    list_models_for_connection, list_ollama_models, LocalModelProvider, ModelProvider, ModelRouter,
};
pub use security::{ensure_loopback_url, ensure_remote_https_url};

#[derive(Clone)]
pub struct AssistantEngine {
    store: SqliteMemoryStore,
    agent: AgentRuntime,
    vector: MemoryVectorService,
    tools: ToolExecutor,
    plugins: std::sync::Arc<aro_plugins::PluginManager>,
}

impl AssistantEngine {
    pub fn new(store: SqliteMemoryStore) -> Self {
        let plugins_dir = directories::ProjectDirs::from("local", "aro", "ARO")
            .map(|dirs| dirs.data_local_dir().join("plugins"))
            .unwrap_or_else(|| std::env::temp_dir().join("aro_plugins"));
        let skill_registry = std::sync::Arc::new(aro_skills::SkillRegistry::new());
        let plugins =
            std::sync::Arc::new(aro_plugins::PluginManager::new(plugins_dir, skill_registry));

        Self {
            store,
            agent: AgentRuntime::new(),
            vector: MemoryVectorService::from_env(),
            tools: ToolExecutor::default(),
            plugins,
        }
    }

    pub fn with_plugin_manager(
        store: SqliteMemoryStore,
        plugins: std::sync::Arc<aro_plugins::PluginManager>,
    ) -> Self {
        Self {
            store,
            agent: AgentRuntime::new(),
            vector: MemoryVectorService::from_env(),
            tools: ToolExecutor::default(),
            plugins,
        }
    }

    pub fn with_vector_service(
        store: SqliteMemoryStore,
        vector: MemoryVectorService,
    ) -> Self {
        let plugins_dir = directories::ProjectDirs::from("local", "aro", "ARO")
            .map(|dirs| dirs.data_local_dir().join("plugins"))
            .unwrap_or_else(|| std::env::temp_dir().join("aro_plugins"));
        let skill_registry = std::sync::Arc::new(aro_skills::SkillRegistry::new());
        let plugins =
            std::sync::Arc::new(aro_plugins::PluginManager::new(plugins_dir, skill_registry));

        Self {
            store,
            agent: AgentRuntime::new(),
            vector,
            tools: ToolExecutor::default(),
            plugins,
        }
    }

    pub fn plugins(&self) -> std::sync::Arc<aro_plugins::PluginManager> {
        self.plugins.clone()
    }

    pub fn tools(&self) -> &ToolExecutor {
        &self.tools
    }

    pub async fn send_message(
        &self,
        request: SendMessageRequest,
        provider: &dyn ModelProvider,
    ) -> AroResult<SendMessageResponse> {
        let trimmed = request.content.trim();
        // Parité avec l'API : un message sans texte mais avec pièces
        // jointes est accepté (ex. envoi photo seule).
        if trimmed.is_empty() && request.attachments.is_empty() {
            return Err(AroError::Configuration(
                "message content cannot be empty".to_string(),
            ));
        }

        let mut conversation = match request.conversation_id {
            Some(id) => self.store.get_conversation(id)?.unwrap_or_else(|| {
                Conversation::with_id(id, make_title(trimmed), request.mode.clone())
            }),
            None => Conversation::new(make_title(trimmed), request.mode.clone()),
        };

        conversation.mode = request.mode.clone();
        conversation.updated_at = Utc::now();
        self.store.upsert_conversation(&conversation)?;
        let lane = self
            .store
            .ensure_agent_lane(Some(conversation.id), conversation.title.clone())?;

        let run_request = AgentRunStartRequest {
            run_id: None,
            lane_id: Some(lane.id),
            conversation_id: Some(conversation.id),
            goal: trimmed.to_string(),
            mode: request.mode.clone(),
            system_prompt: request.system_prompt.clone(),
            model_id: request.model_id.clone(),
            provider: request.provider.clone(),
            autonomy_profile_id: None,
            priority: Some(lane.priority.clone()),
            max_steps: None,
        };
        let mut agent_run = self.agent.start_run(&run_request, None);
        agent_run.status = AgentRunStatus::Running;
        self.store.upsert_agent_run(&agent_run)?;
        self.store
            .add_agent_step(&self.run_started_step(&agent_run, run_request.max_steps))?;

        let mut user_message =
            ChatMessage::new(conversation.id, MessageRole::User, trimmed.to_string());
        user_message.attachments = request.attachments.iter().map(Into::into).collect();
        self.store.add_message(&user_message)?;

        // Continuous Consolidation Daemon Trigger with configurable interval
        let compaction_interval = request
            .memory_settings
            .as_ref()
            .map(|s| s.compaction_interval)
            .unwrap_or(COMPACTION_TURN_INTERVAL);
        let compactor = ContinuousCompactor::new_with_interval(self.store.clone(), compaction_interval);
        if let Ok(Some(compaction)) = compactor.consolidate_if_needed(conversation.id) {
            for mem in &compaction.extracted_memories {
                let _ = self
                    .vector
                    .index_memory(mem, &MemoryVectorScope::local())
                    .await;
            }
        }

        let base_system_prompt = request
            .system_prompt
            .clone()
            .unwrap_or_else(|| request.mode.system_instruction());
        let semantic_candidates = self.store.list_pinned_memories().unwrap_or_default();
        let stored_episodes = self.store.list_episodes(conversation.id).unwrap_or_default();
        let episodic_summaries: Vec<EpisodeSummary> =
            stored_episodes.iter().map(EpisodeSummary::from).collect();
        let full_history = self.store.list_messages(conversation.id)?;

        let window_mgr = if let Some(mem_cfg) = request.memory_settings.as_ref() {
            ContextWindowManager::new(mem_cfg.to_context_budget())
        } else {
            ContextWindowManager::default()
        };
        let assembled = window_mgr.assemble_context(
            &base_system_prompt,
            &semantic_candidates,
            &episodic_summaries,
            &full_history,
        )?;
        let composite_system_prompt = window_mgr.render_system_prompt(&assembled);

        let top_k = request
            .memory_settings
            .as_ref()
            .map(|s| s.top_k)
            .unwrap_or(8);
        let memory_sources = self.memory_context_sources(trimmed, top_k).await?;
        let mut context_sources = memory_sources.clone();
        let prefetch = self
            .prefetch_url_context(
                &agent_run,
                trimmed,
                request.web_access.clone(),
                request.search_settings.as_ref(),
                2,
                &mut None,
            )
            .await?;
        context_sources.extend(prefetch.sources);
        let mut next_sequence = prefetch.next_sequence;
        let context_pack = self.agent.build_context_pack(
            &agent_run,
            &assembled.working_messages,
            &context_sources,
            &[],
            EnvironmentSnapshot::desktop_local(request.provider.clone(), request.model_id.clone()),
        );
        self.touch_recalled_memories(&memory_sources)?;
        self.store.add_agent_step(&self.agent.context_step_at(
            &agent_run,
            next_sequence,
            &context_pack,
        ))?;
        next_sequence += 1;
        let outcome = match self
            .run_tool_loop(
                &mut agent_run,
                assembled.working_messages,
                context_sources,
                context_pack,
                composite_system_prompt,
                trimmed.to_string(),
                provider,
                request.web_access,
                request.search_settings.as_ref(),
                run_request.max_steps,
                next_sequence,
                None,
                &mut None,
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(err) => {
                agent_run.status = AgentRunStatus::Failed;
                agent_run.updated_at = Utc::now();
                agent_run.last_error = Some(err.to_string());
                let _ = self.store.upsert_agent_run(&agent_run);
                return Err(err);
            }
        };
        next_sequence = outcome.next_sequence;
        let mut assistant_message =
            ChatMessage::new(conversation.id, MessageRole::Assistant, outcome.content);
        assistant_message.token_estimate = outcome.token_estimate;
        self.store.add_message(&assistant_message)?;
        let final_history = self.store.list_messages(conversation.id)?;
        let checkpoint = self
            .agent
            .checkpoint_step(&agent_run, next_sequence, &final_history);
        next_sequence += 1;
        if let Some(summary) = checkpoint
            .output
            .get("summary")
            .and_then(|value| value.as_str())
        {
            agent_run.checkpoint_summary = Some(summary.to_string());
        }
        self.store.add_agent_step(&checkpoint)?;
        agent_run.status = outcome.status;
        agent_run.updated_at = Utc::now();
        agent_run.last_error = outcome.last_error;
        if matches!(agent_run.status, AgentRunStatus::Completed) {
            self.store.add_agent_step(&self.agent.final_step(
                &agent_run,
                next_sequence,
                &assistant_message.content,
            ))?;
            agent_run.completed_at = Some(agent_run.updated_at);
        }
        agent_run.heartbeat_at = Some(agent_run.updated_at);
        self.store.upsert_agent_run(&agent_run)?;

        conversation.updated_at = Utc::now();
        self.store.upsert_conversation(&conversation)?;

        Ok(SendMessageResponse {
            conversation,
            user_message,
            assistant_message,
            agent_run_id: Some(agent_run.id),
        })
    }

    pub fn conversations(&self) -> AroResult<Vec<Conversation>> {
        self.store.list_conversations()
    }

    pub fn messages(&self, conversation_id: uuid::Uuid) -> AroResult<Vec<ChatMessage>> {
        let mut messages = self.store.list_messages(conversation_id)?;
        for msg in &mut messages {
            if let Some(run_id) = msg.agent_run_id {
                if let Ok(steps) = self.store.list_agent_steps(run_id) {
                    msg.steps = Some(steps);
                }
            }
        }
        Ok(messages)
    }

    pub fn create_conversation(
        &self,
        title: impl Into<String>,
        mode: AssistantMode,
        project_id: Option<uuid::Uuid>,
        folder_id: Option<uuid::Uuid>,
    ) -> AroResult<Conversation> {
        self.store
            .create_conversation(title, mode, project_id, folder_id)
    }

    pub fn get_conversation(&self, id: uuid::Uuid) -> AroResult<Option<Conversation>> {
        let convs = self.store.list_conversations()?;
        Ok(convs.into_iter().find(|c| c.id == id))
    }

    pub fn resolve_effective_root_path(
        &self,
        conversation_id: uuid::Uuid,
    ) -> AroResult<Option<String>> {
        let convs = self.store.list_conversations()?;
        let conv = match convs.into_iter().find(|c| c.id == conversation_id) {
            Some(c) => c,
            None => return Ok(None),
        };
        if let Some(rp) = &conv.root_path {
            if !rp.trim().is_empty() {
                return Ok(Some(rp.clone()));
            }
        }
        let mut folder_parent_project: Option<uuid::Uuid> = None;
        if let Some(folder_id) = conv.folder_id {
            if let Ok(folders) = self.store.list_folders() {
                if let Some(f) = folders.into_iter().find(|folder| folder.id == folder_id) {
                    if let Some(rp) = &f.root_path {
                        if !rp.trim().is_empty() {
                            return Ok(Some(rp.clone()));
                        }
                    }
                    folder_parent_project = f.project_id;
                }
            }
        }
        // Héritage dossier vide -> projet parent du dossier.
        if let Some(folder_project_id) = folder_parent_project {
            if let Ok(projects) = self.store.list_projects() {
                if let Some(p) = projects
                    .into_iter()
                    .find(|proj| proj.id == folder_project_id)
                {
                    if let Some(rp) = &p.root_path {
                        if !rp.trim().is_empty() {
                            return Ok(Some(rp.clone()));
                        }
                    }
                }
            }
        }
        if let Some(project_id) = conv.project_id {
            if let Ok(projects) = self.store.list_projects() {
                if let Some(p) = projects.into_iter().find(|proj| proj.id == project_id) {
                    if let Some(rp) = &p.root_path {
                        if !rp.trim().is_empty() {
                            return Ok(Some(rp.clone()));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn upsert_conversation(&self, conversation: &Conversation) -> AroResult<()> {
        self.store.upsert_conversation(conversation)
    }

    pub fn delete_conversation(&self, conversation_id: uuid::Uuid) -> AroResult<()> {
        self.store.delete_conversation(conversation_id)
    }

    pub fn delete_empty_conversations(&self) -> AroResult<Vec<uuid::Uuid>> {
        self.store.delete_empty_conversations()
    }

    pub fn projects(&self) -> AroResult<Vec<Project>> {
        self.store.list_projects()
    }

    pub fn save_project(&self, project: &Project) -> AroResult<()> {
        self.store.save_project(project)
    }

    pub fn delete_project(&self, id: uuid::Uuid) -> AroResult<()> {
        self.store.delete_project(id)
    }

    pub fn deduplicate_projects(&self) -> AroResult<Vec<uuid::Uuid>> {
        self.store.deduplicate_projects()
    }

    pub fn deduplicate_folders(&self) -> AroResult<Vec<uuid::Uuid>> {
        self.store.deduplicate_folders()
    }

    pub fn folders(&self) -> AroResult<Vec<Folder>> {
        self.store.list_folders()
    }

    pub fn save_folder(&self, folder: &Folder) -> AroResult<()> {
        self.store.save_folder(folder)
    }

    pub fn delete_folder(&self, id: uuid::Uuid) -> AroResult<()> {
        self.store.delete_folder(id)
    }

    pub fn move_conversation(
        &self,
        conversation_id: uuid::Uuid,
        project_id: Option<uuid::Uuid>,
        folder_id: Option<uuid::Uuid>,
    ) -> AroResult<Conversation> {
        self.store
            .move_conversation(conversation_id, project_id, folder_id)
    }

    pub fn set_conversation_root_path(
        &self,
        conversation_id: uuid::Uuid,
        root_path: Option<String>,
    ) -> AroResult<Conversation> {
        self.store
            .set_conversation_root_path(conversation_id, root_path)
    }

    pub fn search_messages(&self, query: &str, limit: usize) -> AroResult<Vec<ChatMessage>> {
        self.store.search_messages(query, limit)
    }

    pub fn update_conversation_title(
        &self,
        conversation_id: uuid::Uuid,
        title: String,
    ) -> AroResult<Conversation> {
        let mut conversation = self
            .store
            .get_conversation(conversation_id)?
            .ok_or_else(|| AroError::Memory("Conversation not found".to_string()))?;
        conversation.title = title;
        conversation.updated_at = chrono::Utc::now();
        self.store.upsert_conversation(&conversation)?;
        Ok(conversation)
    }

    pub fn upsert_message(&self, message: &ChatMessage) -> AroResult<()> {
        self.store.upsert_message(message)
    }

    pub fn update_message(&self, message_id: uuid::Uuid, content: String) -> AroResult<()> {
        let message = self
            .store
            .get_message(message_id)?
            .ok_or_else(|| AroError::Memory("Message not found".to_string()))?;
        self.store.update_message(message_id, content)?;
        self.store
            .delete_messages_after(message.conversation_id, message.created_at)?;
        Ok(())
    }

    pub async fn start_agent_run(&self, request: AgentRunStartRequest) -> AroResult<AgentRunView> {
        let trimmed = request.goal.trim();
        if trimmed.is_empty() {
            return Err(AroError::Configuration(
                "agent goal cannot be empty".to_string(),
            ));
        }
        let conversation_id = match request.conversation_id {
            Some(id) => Some(id),
            None => {
                let conversation = self.store.create_conversation(
                    make_title(trimmed),
                    request.mode.clone(),
                    None,
                    None,
                )?;
                Some(conversation.id)
            }
        };
        let request = AgentRunStartRequest {
            run_id: request.run_id,
            lane_id: request.lane_id,
            conversation_id,
            goal: trimmed.to_string(),
            ..request
        };
        let max_steps = request.max_steps;
        let environment =
            EnvironmentSnapshot::desktop_local(request.provider.clone(), request.model_id.clone());
        let lane = match request.lane_id {
            Some(lane_id) => self
                .store
                .get_agent_lane(lane_id)?
                .ok_or_else(|| AroError::Memory("agent lane not found".to_string()))?,
            None => {
                let title = conversation_id
                    .and_then(|id| self.store.get_conversation(id).ok().flatten())
                    .map(|conversation| conversation.title)
                    .unwrap_or_else(|| make_title(trimmed));
                self.store.ensure_agent_lane(conversation_id, title)?
            }
        };
        let autonomy_profile_id = request.autonomy_profile_id;
        let mut run = self.agent.start_run(
            &AgentRunStartRequest {
                lane_id: Some(lane.id),
                priority: request
                    .priority
                    .clone()
                    .or_else(|| Some(lane.priority.clone())),
                ..request
            },
            autonomy_profile_id,
        );
        self.agent.schedule_run(
            &mut run,
            &lane,
            self.store.count_running_agent_runs()?,
            self.store.count_running_agent_runs_for_lane(lane.id)?,
        );
        self.store.upsert_agent_run(&run)?;
        self.store
            .add_agent_step(&self.run_started_step(&run, max_steps))?;
        let history = conversation_id
            .map(|id| self.store.list_messages(id))
            .transpose()?
            .unwrap_or_default();
        let memory_sources = self.memory_context_sources(trimmed, 8).await?;
        let context_pack =
            self.agent
                .build_context_pack(&run, &history, &memory_sources, &[], environment);
        self.touch_recalled_memories(&memory_sources)?;
        self.store
            .add_agent_step(&self.agent.context_step(&run, &context_pack))?;
        Ok(AgentRunView {
            run: self
                .store
                .get_agent_run(run.id)?
                .ok_or_else(|| AroError::Memory("agent run not found".to_string()))?,
            steps: self.store.list_agent_steps(run.id)?,
            artifacts: self.store.list_agent_artifacts(run.id)?,
            context_pack: Some(context_pack),
        })
    }

    pub fn agent_runs(&self) -> AroResult<Vec<AgentRun>> {
        self.store.list_agent_runs()
    }

    pub fn agent_orchestrator_snapshot(&self) -> AroResult<AgentOrchestratorSnapshot> {
        let lanes = self.store.list_agent_lane_views()?;
        Ok(AgentOrchestratorSnapshot {
            max_global_running: self.agent.max_global_running(),
            running_count: lanes.iter().map(|lane| lane.running_count).sum(),
            queued_count: lanes.iter().map(|lane| lane.queued_count).sum(),
            lanes,
        })
    }

    pub fn agent_lane_views(&self) -> AroResult<Vec<AgentLaneView>> {
        self.store.list_agent_lane_views()
    }

    pub fn update_agent_lane_status(
        &self,
        lane_id: uuid::Uuid,
        status: AgentLaneStatus,
    ) -> AroResult<AgentLane> {
        let lane = self
            .store
            .update_agent_lane_status(lane_id, status.clone())?
            .ok_or_else(|| AroError::Memory("agent lane not found".to_string()))?;
        match status {
            AgentLaneStatus::Paused => {
                for run in self.store.list_agent_runs_for_lane(lane_id, 100)? {
                    if matches!(run.status, AgentRunStatus::Running) {
                        let _ = self.store.update_agent_run_status(
                            run.id,
                            AgentRunStatus::Paused,
                            Some("Paused with lane.".to_string()),
                        )?;
                    }
                }
            }
            AgentLaneStatus::Active => {
                self.promote_lane_queue(&lane)?;
            }
        }
        self.store
            .get_agent_lane(lane_id)?
            .ok_or_else(|| AroError::Memory("agent lane not found".to_string()))
    }

    pub fn update_agent_lane_priority(
        &self,
        lane_id: uuid::Uuid,
        priority: AgentRunPriority,
    ) -> AroResult<AgentLane> {
        let lane = self
            .store
            .update_agent_lane_priority(lane_id, priority)?
            .ok_or_else(|| AroError::Memory("agent lane not found".to_string()))?;
        self.promote_lane_queue(&lane)?;
        Ok(lane)
    }

    pub fn agent_run_view(&self, run_id: uuid::Uuid) -> AroResult<AgentRunView> {
        let run = self
            .store
            .get_agent_run(run_id)?
            .ok_or_else(|| AroError::Memory("agent run not found".to_string()))?;
        let steps = self.store.list_agent_steps(run_id)?;
        let artifacts = self.store.list_agent_artifacts(run_id)?;
        let context_pack = steps
            .iter()
            .rev()
            .find(|step| matches!(step.kind, AgentStepKind::ContextBuilt))
            .and_then(|step| serde_json::from_value::<ContextPack>(step.output.clone()).ok());
        Ok(AgentRunView {
            run,
            steps,
            artifacts,
            context_pack,
        })
    }

    pub fn update_agent_run_status(
        &self,
        run_id: uuid::Uuid,
        status: AgentRunStatus,
    ) -> AroResult<AgentRun> {
        self.store
            .update_agent_run_status(run_id, status, None)?
            .ok_or_else(|| AroError::Memory("agent run not found".to_string()))
    }

    pub fn search_agent_context(
        &self,
        query: &str,
        limit: usize,
    ) -> AroResult<Vec<AgentContextItem>> {
        self.store.search_agent_context(query, limit)
    }

    pub fn memories(&self) -> AroResult<Vec<LongTermMemory>> {
        self.store.list_memories()
    }

    pub fn list_episodes(
        &self,
        conversation_id: Option<uuid::Uuid>,
        limit: Option<usize>,
    ) -> AroResult<Vec<aro_core::Episode>> {
        if let Some(conv_id) = conversation_id {
            self.store.list_episodes(conv_id)
        } else {
            self.store.list_all_episodes(limit.unwrap_or(100))
        }
    }

    pub async fn search_memories(
        &self,
        query: &str,
        limit: usize,
    ) -> AroResult<Vec<LongTermMemory>> {
        let trimmed = query.trim();
        let candidate_limit = limit.saturating_mul(2).max(10);

        // 1. Lexical hits from FTS5
        let lexical_memories = if !trimmed.is_empty() {
            self.store.search_memories(trimmed, candidate_limit)?
        } else {
            Vec::new()
        };
        let fts_ids: Vec<uuid::Uuid> = lexical_memories.iter().map(|m| m.id).collect();

        // 2. Vector hits from VectorMemoryEngine
        let vector_hits = if !trimmed.is_empty() {
            self.vector
                .search(trimmed, &MemoryVectorScope::local(), candidate_limit)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // 3. Reciprocal Rank Fusion
        let fused_scores = aro_vector::reciprocal_rank_fusion(
            &fts_ids,
            &vector_hits,
            60.0,
            0.40,
            0.60,
        );

        // 4. Retrieve candidate memory records by Top-K IDs (no list_memories full scan)
        let top_ids: Vec<uuid::Uuid> = fused_scores
            .iter()
            .take(candidate_limit)
            .map(|s| s.memory_id)
            .collect();

        let candidate_memories = if !top_ids.is_empty() {
            self.store.get_memories_by_ids(&top_ids)?
        } else {
            Vec::new()
        };

        let mut by_id: std::collections::HashMap<uuid::Uuid, LongTermMemory> = candidate_memories
            .into_iter()
            .map(|m| (m.id, m))
            .collect();
        for m in lexical_memories {
            by_id.entry(m.id).or_insert(m);
        }

        // 5. Pinned memories: fetch pinned memories (permanent decay immunity, highest priority)
        let pinned = self.store.list_pinned_memories().unwrap_or_default();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let mut pinned_approved: Vec<_> = pinned
            .into_iter()
            .filter(|m| m.approved_for_recall())
            .collect();
        pinned_approved.sort_by(|a, b| {
            b.salience
                .partial_cmp(&a.salience)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
                .then_with(|| a.id.cmp(&b.id))
        });
        for p in pinned_approved {
            if seen.insert(p.id) {
                results.push(p);
            }
        }

        // Fused unpinned memories sorted by dynamic utility score
        let max_possible_rrf = (0.40_f32 + 0.60_f32) / (60.0_f32 + 1.0_f32);
        let now = chrono::Utc::now();
        let mut scored_unpinned = Vec::new();
        for score in fused_scores {
            if let Some(mem) = by_id.remove(&score.memory_id) {
                if !seen.contains(&mem.id) && mem.status == aro_core::MEMORY_STATUS_APPROVED {
                    let normalized_hybrid = if max_possible_rrf > 0.0 {
                        (score.rrf_score / max_possible_rrf).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let last_used = mem.last_used_at.unwrap_or(mem.created_at);
                    let recency = compute_recency_decay(last_used, now, mem.pinned);
                    let utility = compute_memory_utility(
                        normalized_hybrid,
                        mem.salience,
                        recency,
                        mem.recall_count,
                    );
                    scored_unpinned.push((utility, mem));
                }
            }
        }

        scored_unpinned.sort_by(|(u1, m1), (u2, m2)| {
            u2.partial_cmp(u1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| m2.updated_at.cmp(&m1.updated_at))
                .then_with(|| m1.id.cmp(&m2.id))
        });

        for (_, m) in scored_unpinned {
            if seen.insert(m.id) {
                results.push(m);
            }
        }

        results.truncate(limit.max(1));
        Ok(results)
    }

    pub async fn upsert_memory(&self, memory: &LongTermMemory) -> AroResult<LongTermMemory> {
        let saved = self.store.upsert_memory(memory)?;
        self.vector
            .index_memory(&saved, &MemoryVectorScope::local())
            .await?;
        Ok(saved)
    }

    pub async fn delete_memory(&self, memory_id: uuid::Uuid) -> AroResult<()> {
        self.store.delete_memory(memory_id)?;
        self.vector
            .delete_memory(memory_id, &MemoryVectorScope::local())
            .await?;
        Ok(())
    }

    pub async fn memory_index_status(&self) -> AroResult<MemoryIndexStatus> {
        self.vector.status().await
    }

    pub async fn memory_index_reindex(&self) -> AroResult<MemoryReindexReport> {
        let memories = self.store.list_memories()?;
        self.vector
            .reindex(&memories, &MemoryVectorScope::local())
            .await
    }

    pub async fn execute_tool(
        &self,
        run: &AgentRun,
        request: ToolExecutionRequest,
    ) -> AroResult<ToolExecutionResult> {
        let policy = WebAccessPolicy::unrestricted();
        let mut on_step = None;
        self.execute_tool_request(run, request, &policy, 1, &mut on_step)
            .await
    }

    async fn memory_context_sources(
        &self,
        query: &str,
        limit: usize,
    ) -> AroResult<Vec<ContextSource>> {
        let memories = self.search_memories(query, limit).await?;
        Ok(memories_to_context_sources(memories))
    }

    fn touch_recalled_memories(&self, sources: &[ContextSource]) -> AroResult<()> {
        let ids = sources
            .iter()
            .filter_map(|source| source.uri.as_deref())
            .filter_map(|uri| uri.strip_prefix("memory://"))
            .filter_map(|id| uuid::Uuid::parse_str(id).ok())
            .collect::<Vec<_>>();
        self.store.touch_memories_used(&ids)
    }

    fn promote_lane_queue(&self, lane: &AgentLane) -> AroResult<()> {
        if lane.status == AgentLaneStatus::Paused {
            return Ok(());
        }
        let global_running = self.store.count_running_agent_runs()?;
        let lane_running = self.store.count_running_agent_runs_for_lane(lane.id)?;
        if global_running >= self.agent.max_global_running()
            || lane_running >= lane.max_concurrent_runs
        {
            return Ok(());
        }
        let available_slots = (self.agent.max_global_running() - global_running)
            .min(lane.max_concurrent_runs - lane_running);

        let mut candidates = self
            .store
            .list_agent_runs_for_lane(lane.id, 100)?
            .into_iter()
            .filter(|run| matches!(run.status, AgentRunStatus::Queued | AgentRunStatus::Paused))
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .priority
                .cmp(&left.priority)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });

        for run in candidates.into_iter().take(available_slots as usize) {
            let _ = self
                .store
                .update_agent_run_status(run.id, AgentRunStatus::Running, None)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_tool_loop(
        &self,
        agent_run: &mut AgentRun,
        mut history: Vec<ChatMessage>,
        mut context_sources: Vec<ContextSource>,
        mut context_pack: ContextPack,
        system_prompt: String,
        user_input: String,
        provider: &dyn ModelProvider,
        web_access: WebAccessMode,
        search_settings: Option<&aro_core::settings::SearchSettings>,
        requested_max_steps: Option<u32>,
        mut next_sequence: i32,
        mut on_chunk: Option<&mut (dyn FnMut(String) + Send)>,
        on_step: &mut Option<&mut (dyn FnMut(AgentStep) + Send)>,
    ) -> AroResult<AgentLoopOutcome> {
        let policy = web_policy_for(web_access.clone(), search_settings);
        let _ = requested_max_steps;
        let mut token_estimate: Option<u32>;
        let window_mgr = ContextWindowManager::default();

        // Initial bounds enforcement on working history
        let (bounded_history, _, _) = window_mgr.fit_working_messages(&history);
        history = bounded_history;

        loop {
            let max_gen_tokens = provider.max_tokens().min(1392);
            let generation_request = self.agent.model_request(
                agent_run,
                &context_pack,
                system_prompt.clone(),
                history.clone(),
                user_input.clone(),
                provider.temperature(),
                max_gen_tokens,
                ModelResponseFormat::AgentActionJson,
            );
            let generation = provider.generate(generation_request).await?;
            token_estimate = generation.token_estimate;
            let action = self.agent.parse_model_action(&generation.content);
            let validation_error = self.agent.validate_action(&action, &context_pack).err();
            self.add_and_notify_step(
                &self.agent.model_step(
                    agent_run,
                    next_sequence,
                    &generation.content,
                    &action,
                    validation_error.as_deref(),
                ),
                on_step,
            )?;
            next_sequence += 1;

            if let Some(error) = validation_error {
                let content = assistant_content_for_action(&action, Some(&error));
                emit_final_chunks(&mut on_chunk, &content);
                return Ok(AgentLoopOutcome {
                    content,
                    token_estimate,
                    status: AgentRunStatus::Failed,
                    last_error: Some(error),
                    next_sequence,
                });
            }

            match action.action_type {
                AgentActionType::Final => {
                    let content = action.content.clone().unwrap_or_default();
                    emit_final_chunks(&mut on_chunk, &content);
                    return Ok(AgentLoopOutcome {
                        content,
                        token_estimate,
                        status: AgentRunStatus::Completed,
                        last_error: None,
                        next_sequence,
                    });
                }
                AgentActionType::Pause => {
                    let content = assistant_content_for_action(&action, None);
                    emit_final_chunks(&mut on_chunk, &content);
                    return Ok(AgentLoopOutcome {
                        content,
                        token_estimate,
                        status: AgentRunStatus::Waiting,
                        last_error: None,
                        next_sequence,
                    });
                }
                AgentActionType::Tool => {
                    let result = self
                        .execute_agent_tool(agent_run, &action, &policy, next_sequence, on_step)
                        .await?;
                    next_sequence += 1;
                    context_sources.extend(result.context_sources.clone());

                    let conv_id = agent_run.conversation_id.unwrap_or(agent_run.id);
                    let tool_name = action.tool_id.as_deref().unwrap_or("unknown");
                    history.push(ChatMessage::new(
                        conv_id,
                        MessageRole::Assistant,
                        format!(
                            "Calling tool `{}` with arguments: {}",
                            tool_name, action.input
                        ),
                    ));
                    history.push(ChatMessage::new(
                        conv_id,
                        MessageRole::User,
                        format!(
                            "Tool `{}` returned: {}\n{}",
                            tool_name, result.title, result.summary
                        ),
                    ));

                    // Re-apply sliding window bounds on working history after tool step
                    let (bounded_history, _, _) = window_mgr.fit_working_messages(&history);
                    history = bounded_history;

                    context_pack = self.agent.build_context_pack(
                        agent_run,
                        &history,
                        &context_sources,
                        &[],
                        EnvironmentSnapshot::desktop_local(
                            agent_run.model_provider_id.clone(),
                            agent_run.model_id.clone(),
                        ),
                    );
                    self.add_and_notify_step(
                        &self
                            .agent
                            .context_step_at(agent_run, next_sequence, &context_pack),
                        on_step,
                    )?;
                    next_sequence += 1;
                }
            }
        }
    }

    fn run_started_step(&self, run: &AgentRun, max_steps: Option<u32>) -> AgentStep {
        let mut step = self.agent.run_started_step(run);
        step.input["maxSteps"] = max_steps.map(|v| json!(v)).unwrap_or(Value::Null);
        step
    }

    async fn prefetch_url_context(
        &self,
        run: &AgentRun,
        input: &str,
        web_access: WebAccessMode,
        search_settings: Option<&aro_core::settings::SearchSettings>,
        mut next_sequence: i32,
        on_step: &mut Option<&mut (dyn FnMut(AgentStep) + Send)>,
    ) -> AroResult<PrefetchOutcome> {
        let policy = web_policy_for(web_access, search_settings);
        if !policy.allow_network {
            return Ok(PrefetchOutcome {
                sources: Vec::new(),
                next_sequence,
            });
        }
        let urls = extract_urls(input, 3);
        if urls.is_empty() {
            return Ok(PrefetchOutcome {
                sources: Vec::new(),
                next_sequence,
            });
        }
        let mut sources = Vec::new();
        for url in urls {
            let request = ToolExecutionRequest::new(
                run.id,
                run.conversation_id,
                TOOL_CORE_WEB_PAGE_READ,
                json!(WebFetchRequest {
                    url,
                    max_chars: Some(12_000),
                }),
            );
            let result = self
                .execute_tool_request(run, request, &policy, next_sequence, on_step)
                .await?;
            next_sequence += 1;
            sources.extend(result.context_sources);
        }
        Ok(PrefetchOutcome {
            sources,
            next_sequence,
        })
    }

    async fn execute_agent_tool(
        &self,
        run: &AgentRun,
        action: &AgentAction,
        policy: &WebAccessPolicy,
        sequence: i32,
        on_step: &mut Option<&mut (dyn FnMut(AgentStep) + Send)>,
    ) -> AroResult<ToolExecutionResult> {
        let tool_id = action
            .tool_id
            .clone()
            .ok_or_else(|| AroError::Configuration("tool action is missing toolId".to_string()))?;
        let mut input = action.input.clone();
        if input.get("root_path").is_none() && input.get("rootPath").is_none() {
            if let Some(conv_id) = run.conversation_id {
                if let Ok(Some(rp)) = self.store.resolve_effective_root_path(conv_id) {
                    input["root_path"] = json!(rp);
                }
            }
        }
        let request = ToolExecutionRequest::new(run.id, run.conversation_id, tool_id, input);
        self.execute_tool_request(run, request, policy, sequence, on_step)
            .await
    }

    async fn execute_tool_request(
        &self,
        run: &AgentRun,
        request: ToolExecutionRequest,
        policy: &WebAccessPolicy,
        sequence: i32,
        on_step: &mut Option<&mut (dyn FnMut(AgentStep) + Send)>,
    ) -> AroResult<ToolExecutionResult> {
        let tool_id = request.tool_id.as_str();
        let mut result = if tool_id.starts_with("core.plan.") || tool_id.starts_with("plan.") {
            match self.execute_plan_tool(run, &request).await {
                Ok(res) => res,
                Err(err) => failed_tool_result(&request, err.to_string()),
            }
        } else if is_memory_tool(tool_id) {
            match self.execute_memory_tool(run, &request).await {
                Ok(res) => res,
                Err(err) => failed_tool_result(&request, err.to_string()),
            }
        } else if tool_id == TOOL_CORE_CONTEXT_SEARCH || tool_id == "context.search" {
            match self.execute_context_tool(&request).await {
                Ok(res) => res,
                Err(err) => failed_tool_result(&request, err.to_string()),
            }
        } else if tool_id == TOOL_CORE_AGENT_DELEGATE
            || tool_id == "agent.delegate"
            || tool_id == TOOL_CORE_AGENT_SPAWN
            || tool_id == "agent.spawn"
            || tool_id == TOOL_CORE_AGENT_STATUS
            || tool_id == "agent.status"
        {
            match self.execute_orchestration_tool(run, &request).await {
                Ok(res) => res,
                Err(err) => failed_tool_result(&request, err.to_string()),
            }
        } else if tool_id == TOOL_CORE_SKILL_LIST
            || tool_id == "skill.list"
            || tool_id == TOOL_CORE_SKILL_INVOKE
            || tool_id == "skill.invoke"
        {
            match self.execute_skill_tool(&request).await {
                Ok(res) => res,
                Err(err) => failed_tool_result(&request, err.to_string()),
            }
        } else if tool_id == TOOL_CORE_CONNECTOR_LIST
            || tool_id == "connector.list"
            || tool_id == TOOL_CORE_CONNECTOR_CALL
            || tool_id == "connector.call"
        {
            match self.execute_connector_tool(&request).await {
                Ok(res) => res,
                Err(err) => failed_tool_result(&request, err.to_string()),
            }
        } else if tool_id == TOOL_CORE_MCP_CALL || tool_id == "mcp.call" {
            match self.execute_mcp_tool(&request).await {
                Ok(res) => res,
                Err(err) => failed_tool_result(&request, err.to_string()),
            }
        } else {
            match self.tools.execute(request.clone(), policy).await {
                Ok(result) => result,
                Err(err) => failed_tool_result(&request, err.to_string()),
            }
        };

        if result.context_sources.is_empty() {
            result.context_sources = vec![ContextSource {
                id: format!(
                    "tool:{}:{}",
                    request.tool_id,
                    &result.invocation_id.to_string()[..8]
                ),
                kind: "tool-result".to_string(),
                title: format!("Tool result: {}", request.tool_id),
                excerpt: format!("{}: {}", result.title, result.summary),
                uri: None,
                score: 0.95,
                created_at: Some(Utc::now()),
            }];
        }

        self.store_tool_execution(run, &request, &result, sequence, on_step)?;
        Ok(result)
    }

    fn store_tool_execution(
        &self,
        run: &AgentRun,
        request: &ToolExecutionRequest,
        result: &ToolExecutionResult,
        sequence: i32,
        on_step: &mut Option<&mut (dyn FnMut(AgentStep) + Send)>,
    ) -> AroResult<()> {
        let status = match result.status {
            ToolExecutionStatus::Completed => AgentStepStatus::Completed,
            ToolExecutionStatus::Running => AgentStepStatus::Running,
            ToolExecutionStatus::Failed => AgentStepStatus::Failed,
            ToolExecutionStatus::Blocked => AgentStepStatus::Failed,
        };
        let step = AgentStep {
            id: request.invocation_id,
            run_id: run.id,
            sequence,
            kind: AgentStepKind::Tool,
            status,
            title: result.title.clone(),
            input: json!({
                "toolId": request.tool_id,
                "input": request.input,
                "requestedAt": request.requested_at,
            }),
            output: serde_json::to_value(result).unwrap_or(Value::Null),
            error: result.error.clone(),
            started_at: result.started_at,
            finished_at: Some(result.finished_at),
        };
        self.add_and_notify_step(&step, on_step)?;
        for artifact in &result.artifacts {
            self.store.add_agent_artifact(artifact)?;
        }
        for item in result_context_items(result, run.conversation_id) {
            self.store.add_agent_context_item(&item)?;
        }
        Ok(())
    }

    pub async fn regenerate_message(
        &self,
        conversation_id: uuid::Uuid,
        system_prompt: Option<String>,
        provider: &dyn ModelProvider,
    ) -> AroResult<ChatMessage> {
        let mut conversation = self
            .store
            .get_conversation(conversation_id)?
            .ok_or_else(|| AroError::Memory("Conversation not found".to_string()))?;

        let history = self.store.list_messages(conversation_id)?;
        let last_user_idx = history
            .iter()
            .rposition(|msg| msg.role == MessageRole::User)
            .ok_or_else(|| {
                AroError::Memory("No user message found to regenerate from".to_string())
            })?;

        let last_user_message = &history[last_user_idx];

        // Delete all messages created after the last user message (like old assistant replies)
        self.store
            .delete_messages_after(conversation_id, last_user_message.created_at)?;

        // Reload cleaned history
        let clean_history = self.store.list_messages(conversation_id)?;

        // Continuous Consolidation Daemon Trigger
        let compactor = ContinuousCompactor::new(self.store.clone());
        let _ = compactor.consolidate_if_needed(conversation_id);

        let last_user_content = last_user_message.content.clone();
        let run = AgentRun::new(
            last_user_content.clone(),
            conversation.mode.clone(),
            Some(conversation.id),
            None,
            None,
            None,
        );

        let base_system_prompt =
            system_prompt.unwrap_or_else(|| conversation.mode.system_instruction());
        let semantic_candidates = self.store.list_pinned_memories().unwrap_or_default();
        let stored_episodes = self.store.list_episodes(conversation_id).unwrap_or_default();
        let episodic_summaries: Vec<EpisodeSummary> =
            stored_episodes.iter().map(EpisodeSummary::from).collect();

        let window_mgr = ContextWindowManager::default();
        let assembled = window_mgr.assemble_context(
            &base_system_prompt,
            &semantic_candidates,
            &episodic_summaries,
            &clean_history,
        )?;
        let composite_system_prompt = window_mgr.render_system_prompt(&assembled);

        let memory_sources = self.memory_context_sources(&last_user_content, 8).await?;
        let context_pack = self.agent.build_context_pack(
            &run,
            &assembled.working_messages,
            &memory_sources,
            &[],
            EnvironmentSnapshot::desktop_local(None, None),
        );
        self.touch_recalled_memories(&memory_sources)?;
        let max_gen_tokens = provider.max_tokens().min(1392);
        let generation_request = self.agent.model_request(
            &run,
            &context_pack,
            composite_system_prompt,
            assembled.working_messages,
            last_user_content,
            provider.temperature(),
            max_gen_tokens,
            ModelResponseFormat::DirectText,
        );

        let generation = provider.generate(generation_request).await?;
        let mut assistant_message =
            ChatMessage::new(conversation.id, MessageRole::Assistant, generation.content);
        assistant_message.token_estimate = generation.token_estimate;
        self.store.add_message(&assistant_message)?;

        conversation.updated_at = Utc::now();
        self.store.upsert_conversation(&conversation)?;

        Ok(assistant_message)
    }

    pub async fn reset_memory(&self) -> AroResult<()> {
        self.store.reset()?;
        self.vector.clear_scope(&MemoryVectorScope::local()).await?;
        Ok(())
    }

    pub fn list_plans(&self, conversation_id: uuid::Uuid) -> AroResult<Vec<aro_core::Plan>> {
        self.store.list_plans(conversation_id)
    }

    pub fn create_plan(&self, plan: &aro_core::Plan) -> AroResult<()> {
        self.store.create_plan(plan)
    }

    pub fn update_plan(&self, plan: &aro_core::Plan) -> AroResult<()> {
        self.store.update_plan(plan)
    }

    pub fn delete_plan(&self, id: uuid::Uuid) -> AroResult<()> {
        self.store.delete_plan(id)
    }

    pub async fn check_runtime(&self, provider: &dyn ModelProvider) -> RuntimeStatus {
        provider.status().await
    }

    fn add_and_notify_step(
        &self,
        step: &AgentStep,
        on_step: &mut Option<&mut (dyn FnMut(AgentStep) + Send)>,
    ) -> AroResult<()> {
        self.store.add_agent_step(step)?;
        if let Some(callback) = on_step.as_mut() {
            callback(step.clone());
        }
        Ok(())
    }

    pub async fn send_message_stream(
        &self,
        request: SendMessageRequest,
        provider: &dyn ModelProvider,
        temp_message_id: uuid::Uuid,
        on_chunk: &mut (dyn FnMut(String) + Send),
        on_step: &mut Option<&mut (dyn FnMut(AgentStep) + Send)>,
    ) -> AroResult<SendMessageResponse> {
        let trimmed = request.content.trim();
        if trimmed.is_empty() && request.attachments.is_empty() {
            return Err(AroError::Configuration(
                "message content cannot be empty".to_string(),
            ));
        }

        let mut conversation = match request.conversation_id {
            Some(id) => self.store.get_conversation(id)?.unwrap_or_else(|| {
                Conversation::with_id(id, make_title(trimmed), request.mode.clone())
            }),
            None => Conversation::new(make_title(trimmed), request.mode.clone()),
        };

        conversation.mode = request.mode.clone();
        conversation.updated_at = Utc::now();
        self.store.upsert_conversation(&conversation)?;
        let lane = self
            .store
            .ensure_agent_lane(Some(conversation.id), conversation.title.clone())?;

        let run_request = AgentRunStartRequest {
            run_id: None,
            lane_id: Some(lane.id),
            conversation_id: Some(conversation.id),
            goal: trimmed.to_string(),
            mode: request.mode.clone(),
            system_prompt: request.system_prompt.clone(),
            model_id: request.model_id.clone(),
            provider: request.provider.clone(),
            autonomy_profile_id: None,
            priority: Some(lane.priority.clone()),
            max_steps: None,
        };
        let mut agent_run = self.agent.start_run(&run_request, None);
        agent_run.status = AgentRunStatus::Running;
        self.store.upsert_agent_run(&agent_run)?;
        self.add_and_notify_step(
            &self.run_started_step(&agent_run, run_request.max_steps),
            on_step,
        )?;

        let mut user_message =
            ChatMessage::new(conversation.id, MessageRole::User, trimmed.to_string());
        user_message.attachments = request.attachments.iter().map(Into::into).collect();
        self.store.add_message(&user_message)?;

        // Continuous Consolidation Daemon Trigger (10 turns or >= 2400 tokens)
        let compactor = ContinuousCompactor::new(self.store.clone());
        if let Ok(Some(compaction)) = compactor.consolidate_if_needed(conversation.id) {
            for mem in &compaction.extracted_memories {
                let _ = self
                    .vector
                    .index_memory(mem, &MemoryVectorScope::local())
                    .await;
            }
        }

        let base_system_prompt = request
            .system_prompt
            .clone()
            .unwrap_or_else(|| request.mode.system_instruction());
        let semantic_candidates = self.store.list_pinned_memories().unwrap_or_default();
        let stored_episodes = self.store.list_episodes(conversation.id).unwrap_or_default();
        let episodic_summaries: Vec<EpisodeSummary> =
            stored_episodes.iter().map(EpisodeSummary::from).collect();
        let full_history = self.store.list_messages(conversation.id)?;

        let window_mgr = ContextWindowManager::default();
        let assembled = window_mgr.assemble_context(
            &base_system_prompt,
            &semantic_candidates,
            &episodic_summaries,
            &full_history,
        )?;
        let composite_system_prompt = window_mgr.render_system_prompt(&assembled);

        let memory_sources = self.memory_context_sources(trimmed, 8).await?;
        let mut context_sources = memory_sources.clone();
        let prefetch = self
            .prefetch_url_context(
                &agent_run,
                trimmed,
                request.web_access.clone(),
                request.search_settings.as_ref(),
                2,
                on_step,
            )
            .await?;
        context_sources.extend(prefetch.sources);
        let mut next_sequence = prefetch.next_sequence;
        let context_pack = self.agent.build_context_pack(
            &agent_run,
            &assembled.working_messages,
            &context_sources,
            &[],
            EnvironmentSnapshot::desktop_local(request.provider.clone(), request.model_id.clone()),
        );
        self.touch_recalled_memories(&memory_sources)?;
        self.add_and_notify_step(
            &self
                .agent
                .context_step_at(&agent_run, next_sequence, &context_pack),
            on_step,
        )?;
        next_sequence += 1;
        let outcome = match self
            .run_tool_loop(
                &mut agent_run,
                assembled.working_messages,
                context_sources,
                context_pack,
                composite_system_prompt,
                trimmed.to_string(),
                provider,
                request.web_access,
                request.search_settings.as_ref(),
                run_request.max_steps,
                next_sequence,
                Some(on_chunk),
                on_step,
            )
            .await
        {
            Ok(outcome) => outcome,
            Err(err) => {
                agent_run.status = AgentRunStatus::Failed;
                agent_run.updated_at = Utc::now();
                agent_run.last_error = Some(err.to_string());
                let _ = self.store.upsert_agent_run(&agent_run);
                return Err(err);
            }
        };
        next_sequence = outcome.next_sequence;
        let mut assistant_message =
            ChatMessage::new(conversation.id, MessageRole::Assistant, outcome.content);
        assistant_message.id = temp_message_id;
        assistant_message.token_estimate = outcome.token_estimate;
        assistant_message.agent_run_id = Some(agent_run.id);
        self.store.add_message(&assistant_message)?;
        let final_history = self.store.list_messages(conversation.id)?;
        let checkpoint = self
            .agent
            .checkpoint_step(&agent_run, next_sequence, &final_history);
        next_sequence += 1;
        if let Some(summary) = checkpoint
            .output
            .get("summary")
            .and_then(|value| value.as_str())
        {
            agent_run.checkpoint_summary = Some(summary.to_string());
        }
        self.add_and_notify_step(&checkpoint, on_step)?;
        agent_run.status = outcome.status;
        agent_run.updated_at = Utc::now();
        agent_run.last_error = outcome.last_error;
        if matches!(agent_run.status, AgentRunStatus::Completed) {
            self.add_and_notify_step(
                &self
                    .agent
                    .final_step(&agent_run, next_sequence, &assistant_message.content),
                on_step,
            )?;
            agent_run.completed_at = Some(agent_run.updated_at);
        }
        agent_run.heartbeat_at = Some(agent_run.updated_at);
        self.store.upsert_agent_run(&agent_run)?;

        conversation.updated_at = Utc::now();
        self.store.upsert_conversation(&conversation)?;

        Ok(SendMessageResponse {
            conversation,
            user_message,
            assistant_message,
            agent_run_id: Some(agent_run.id),
        })
    }

    pub async fn regenerate_message_stream(
        &self,
        conversation_id: uuid::Uuid,
        system_prompt: Option<String>,
        provider: &dyn ModelProvider,
        temp_message_id: uuid::Uuid,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) -> AroResult<ChatMessage> {
        let mut conversation = self
            .store
            .get_conversation(conversation_id)?
            .ok_or_else(|| AroError::Memory("Conversation not found".to_string()))?;

        let history = self.store.list_messages(conversation_id)?;
        let last_user_idx = history
            .iter()
            .rposition(|msg| msg.role == MessageRole::User)
            .ok_or_else(|| {
                AroError::Memory("No user message found to regenerate from".to_string())
            })?;

        let last_user_message = &history[last_user_idx];

        // Delete all messages created after the last user message
        self.store
            .delete_messages_after(conversation_id, last_user_message.created_at)?;

        // Reload cleaned history
        let clean_history = self.store.list_messages(conversation_id)?;

        // Continuous Consolidation Daemon Trigger
        let compactor = ContinuousCompactor::new(self.store.clone());
        let _ = compactor.consolidate_if_needed(conversation_id);

        let last_user_content = last_user_message.content.clone();
        let run = AgentRun::new(
            last_user_content.clone(),
            conversation.mode.clone(),
            Some(conversation.id),
            None,
            None,
            None,
        );

        let base_system_prompt =
            system_prompt.unwrap_or_else(|| conversation.mode.system_instruction());
        let semantic_candidates = self.store.list_pinned_memories().unwrap_or_default();
        let stored_episodes = self.store.list_episodes(conversation_id).unwrap_or_default();
        let episodic_summaries: Vec<EpisodeSummary> =
            stored_episodes.iter().map(EpisodeSummary::from).collect();

        let window_mgr = ContextWindowManager::default();
        let assembled = window_mgr.assemble_context(
            &base_system_prompt,
            &semantic_candidates,
            &episodic_summaries,
            &clean_history,
        )?;
        let composite_system_prompt = window_mgr.render_system_prompt(&assembled);

        let memory_sources = self.memory_context_sources(&last_user_content, 8).await?;
        let context_pack = self.agent.build_context_pack(
            &run,
            &assembled.working_messages,
            &memory_sources,
            &[],
            EnvironmentSnapshot::desktop_local(None, None),
        );
        self.touch_recalled_memories(&memory_sources)?;
        let max_gen_tokens = provider.max_tokens().min(1392);
        let generation_request = self.agent.model_request(
            &run,
            &context_pack,
            composite_system_prompt,
            assembled.working_messages,
            last_user_content,
            provider.temperature(),
            max_gen_tokens,
            ModelResponseFormat::DirectText,
        );

        let generation = provider
            .generate_stream(generation_request, on_chunk)
            .await?;
        let mut assistant_message =
            ChatMessage::new(conversation.id, MessageRole::Assistant, generation.content);
        assistant_message.id = temp_message_id;
        assistant_message.token_estimate = generation.token_estimate;
        self.store.add_message(&assistant_message)?;

        conversation.updated_at = Utc::now();
        self.store.upsert_conversation(&conversation)?;

        Ok(assistant_message)
    }

    async fn execute_plan_tool(
        &self,
        run: &AgentRun,
        request: &ToolExecutionRequest,
    ) -> AroResult<ToolExecutionResult> {
        let started_at = Utc::now();
        let tool_id = request.tool_id.as_str();

        let res = match tool_id {
            "core.plan.create" => {
                let conversation_id = run.conversation_id.ok_or_else(|| {
                    AroError::Configuration("no conversation associated with agent run".to_string())
                })?;
                let title = request
                    .input
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = request
                    .input
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let tasks_val = request
                    .input
                    .get("tasks")
                    .cloned()
                    .unwrap_or_else(|| json!([]));

                // Parse tasks
                let mut tasks = vec![];
                if let Some(arr) = tasks_val.as_array() {
                    for (i, item) in arr.iter().enumerate() {
                        let text = item
                            .get("text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let completed = item
                            .get("completed")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let status = item
                            .get("status")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .or_else(aro_core::default_task_status);
                        let error = item
                            .get("error")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        tasks.push(aro_core::TaskStep {
                            id: format!("{}", i + 1),
                            text,
                            completed,
                            status,
                            error,
                        });
                    }
                }

                let plan = aro_core::Plan {
                    id: uuid::Uuid::new_v4(),
                    conversation_id,
                    title,
                    description,
                    tasks,
                    status: "active".to_string(),
                    created_at: Utc::now().to_rfc3339(),
                    updated_at: Utc::now().to_rfc3339(),
                };

                self.store.create_plan(&plan)?;

                (
                    json!({ "success": true, "plan": plan }),
                    format!("Created plan '{}'", plan.title),
                )
            }
            "core.plan.update" => {
                let id_str = request
                    .input
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let id = uuid::Uuid::parse_str(id_str)
                    .map_err(|e| AroError::Configuration(format!("invalid uuid: {e}")))?;

                let mut plan = self
                    .store
                    .get_plan(id)?
                    .ok_or_else(|| AroError::Configuration(format!("plan not found: {id}")))?;

                if let Some(title) = request.input.get("title").and_then(|v| v.as_str()) {
                    plan.title = title.to_string();
                }
                if let Some(desc_val) = request.input.get("description") {
                    plan.description = desc_val.as_str().map(|s| s.to_string());
                }
                if let Some(status) = request.input.get("status").and_then(|v| v.as_str()) {
                    plan.status = status.to_string();
                }
                if let Some(tasks_val) = request.input.get("tasks") {
                    let mut tasks = vec![];
                    if let Some(arr) = tasks_val.as_array() {
                        for (i, item) in arr.iter().enumerate() {
                            let text = item
                                .get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let completed = item
                                .get("completed")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);
                            let status = item
                                .get("status")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .or_else(aro_core::default_task_status);
                            let error = item
                                .get("error")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let tid = item
                                .get("id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| format!("{}", i + 1));
                            tasks.push(aro_core::TaskStep {
                                id: tid,
                                text,
                                completed,
                                status,
                                error,
                            });
                        }
                    }
                    plan.tasks = tasks;
                }
                plan.updated_at = Utc::now().to_rfc3339();

                self.store.update_plan(&plan)?;

                (
                    json!({ "success": true, "plan": plan }),
                    format!("Updated plan '{}'", plan.title),
                )
            }
            "core.plan.delete" => {
                let id_str = request
                    .input
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let id = uuid::Uuid::parse_str(id_str)
                    .map_err(|e| AroError::Configuration(format!("invalid uuid: {e}")))?;

                self.store.delete_plan(id)?;

                (json!({ "success": true }), format!("Deleted plan {id}"))
            }
            "core.plan.list" => {
                let conversation_id = run.conversation_id.ok_or_else(|| {
                    AroError::Configuration("no conversation associated with agent run".to_string())
                })?;
                let plans = self.store.list_plans(conversation_id)?;
                (
                    json!({ "plans": plans }),
                    format!("Listed {} plan(s)", plans.len()),
                )
            }
            other => {
                return Err(AroError::Configuration(format!(
                    "unknown plan tool: {other}"
                )))
            }
        };

        Ok(ToolExecutionResult {
            invocation_id: request.invocation_id,
            run_id: request.run_id,
            tool_id: request.tool_id.clone(),
            status: aro_core::ToolExecutionStatus::Completed,
            title: res.1.clone(),
            output: res.0,
            summary: res.1,
            context_sources: vec![],
            artifacts: vec![],
            error: None,
            started_at,
            finished_at: Utc::now(),
        })
    }

    // ──────────────────────────────────────────────────────────
    // Memory tool handlers
    // ──────────────────────────────────────────────────────────

    async fn execute_memory_tool(
        &self,
        run: &AgentRun,
        request: &ToolExecutionRequest,
    ) -> AroResult<ToolExecutionResult> {
        let started_at = Utc::now();
        let normalized = normalize_tool_id(&request.tool_id);
        let (output, title) = match normalized {
            TOOL_CORE_MEMORY_SAVE => {
                let content = request
                    .input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if content.is_empty() {
                    return Err(AroError::Configuration("content cannot be empty".to_string()));
                }
                let category = request
                    .input
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("technical")
                    .to_string();
                let scope = request
                    .input
                    .get("scope")
                    .and_then(|v| v.as_str())
                    .unwrap_or("conversation")
                    .to_string();
                let pinned = request
                    .input
                    .get("pinned")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let category_enum = MemoryCategory::from_str_loose(&category);
                let salience = request
                    .input
                    .get("salience")
                    .and_then(|v| v.as_f64())
                    .map(|v| (v as f32).clamp(0.1, 1.0))
                    .unwrap_or_else(|| compute_initial_salience(&content, pinned, category_enum));

                let memory = LongTermMemory {
                    id: uuid::Uuid::new_v4(),
                    client_id: None,
                    content: content.clone(),
                    category,
                    scope,
                    status: "approved".to_string(),
                    source_conversation_id: run.conversation_id,
                    source_message_ids: vec![],
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    pinned,
                    salience,
                    recall_count: 0,
                    last_used_at: None,
                };

                let saved = self.store.upsert_memory(&memory)?;
                // Index asynchronously (best effort – don't block the agent on vector errors)
                let _ = self
                    .vector
                    .index_memory(&saved, &MemoryVectorScope::local())
                    .await;

                (
                    json!({ "success": true, "id": saved.id, "content": saved.content }),
                    format!("Saved memory: {}", &content[..content.len().min(60)]),
                )
            }
            TOOL_CORE_MEMORY_SEARCH => {
                let query = request
                    .input
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let limit = request
                    .input
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(8)
                    .clamp(1, 50) as usize;
                let category_filter = request.input.get("category").and_then(|v| v.as_str());
                let scope_filter = request.input.get("scope").and_then(|v| v.as_str());

                let memories = self.search_memories(&query, limit.saturating_mul(2)).await?;
                let filtered: Vec<_> = memories
                    .into_iter()
                    .filter(|m| {
                        if let Some(cat) = category_filter {
                            if m.category != cat {
                                return false;
                            }
                        }
                        if let Some(sc) = scope_filter {
                            if m.scope != sc {
                                return false;
                            }
                        }
                        true
                    })
                    .take(limit)
                    .collect();

                let scored_memories: Vec<ScoredMemory> = filtered
                    .into_iter()
                    .map(|m| ScoredMemory {
                        id: m.id,
                        content: m.content,
                        category: m.category,
                        scope: m.scope,
                        score: if m.pinned { 1.0 } else { m.salience.clamp(0.0, 1.0) },
                        pinned: m.pinned,
                        salience: m.salience,
                        created_at: m.created_at,
                    })
                    .collect();

                let count = scored_memories.len();
                (
                    json!({ "count": count, "memories": scored_memories }),
                    format!(
                        "Found {} memories for '{}'",
                        count,
                        &query[..query.len().min(40)]
                    ),
                )
            }
            TOOL_CORE_MEMORY_RECALL => {
                let id_opt = request
                    .input
                    .get("id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| uuid::Uuid::parse_str(s).ok());
                let entity_key = request
                    .input
                    .get("entityKey")
                    .or_else(|| request.input.get("entity_key"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let include_episodes = request
                    .input
                    .get("includeEpisodes")
                    .or_else(|| request.input.get("include_episodes"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let memory_opt = if let Some(id) = id_opt {
                    self.store.get_memory(id)?
                } else if let Some(ref ek) = entity_key {
                    let matches = self.store.search_memories(ek, 1)?;
                    matches.into_iter().next()
                } else {
                    None
                };

                if let Some(ref mem) = memory_opt {
                    let _ = self.store.touch_memories_used(&[mem.id]);
                }

                let related_episodes = if include_episodes {
                    if let Some(conv_id) = run.conversation_id {
                        let query_str = entity_key
                            .as_deref()
                            .or_else(|| memory_opt.as_ref().map(|m| m.content.as_str()))
                            .unwrap_or("");
                        if !query_str.trim().is_empty() {
                            Some(self.store.search_episodes(conv_id, query_str, 5).unwrap_or_default())
                        } else {
                            Some(Vec::new())
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                let found = memory_opt.is_some();
                let title = if found {
                    "Recalled memory successfully".to_string()
                } else {
                    "No memory found".to_string()
                };

                let mut out = json!({
                    "found": found,
                });
                if let Some(mem) = memory_opt {
                    out["memory"] = json!(mem);
                }
                if let Some(eps) = related_episodes {
                    out["relatedEpisodes"] = json!(&eps);
                    out["related_episodes"] = json!(eps);
                }

                (out, title)
            }
            TOOL_CORE_MEMORY_UPDATE => {
                let id_str = request
                    .input
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AroError::Configuration("missing memory id".to_string()))?;
                let id = uuid::Uuid::parse_str(id_str)
                    .map_err(|e| AroError::Configuration(format!("invalid memory id: {e}")))?;

                let mut existing = self
                    .store
                    .get_memory(id)?
                    .ok_or_else(|| AroError::Configuration(format!("memory {id} not found")))?;

                if let Some(new_content) = request.input.get("content").and_then(|v| v.as_str()) {
                    let trimmed = new_content.trim();
                    if trimmed.is_empty() {
                        return Err(AroError::Configuration("content cannot be empty".to_string()));
                    }
                    existing.content = trimmed.to_string();
                }
                if let Some(salience) = request.input.get("salience").and_then(|v| v.as_f64()) {
                    existing.salience = (salience as f32).clamp(0.1, 1.0);
                }
                if let Some(pinned) = request.input.get("pinned").and_then(|v| v.as_bool()) {
                    existing.pinned = pinned;
                }
                if let Some(category) = request.input.get("category").and_then(|v| v.as_str()) {
                    existing.category = category.to_string();
                }
                existing.updated_at = Utc::now();

                let updated = self.store.upsert_memory(&existing)?;
                let _ = self
                    .vector
                    .index_memory(&updated, &MemoryVectorScope::local())
                    .await;

                (
                    json!({
                        "success": true,
                        "id": updated.id,
                        "updatedAt": updated.updated_at.to_rfc3339()
                    }),
                    format!("Updated memory {id}"),
                )
            }
            TOOL_CORE_MEMORY_FORGET | TOOL_CORE_MEMORY_DELETE => {
                let id_str = request
                    .input
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AroError::Configuration("missing memory id".to_string()))?;
                let id = uuid::Uuid::parse_str(id_str)
                    .map_err(|e| AroError::Configuration(format!("invalid memory id: {e}")))?;

                self.store.delete_memory(id)?;
                let _ = self
                    .vector
                    .delete_memory(id, &MemoryVectorScope::local())
                    .await;
                (
                    json!({ "success": true, "id": id, "status": "archived" }),
                    format!("Archived memory {id}"),
                )
            }
            TOOL_CORE_MEMORY_LIST => {
                let limit = request
                    .input
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;

                let all_memories = self.store.list_memories()?;
                let category_filter = request.input.get("category").and_then(|v| v.as_str());
                let scope_filter = request.input.get("scope").and_then(|v| v.as_str());

                let filtered: Vec<_> = all_memories
                    .into_iter()
                    .filter(|m| {
                        if let Some(cat) = category_filter {
                            if m.category != cat {
                                return false;
                            }
                        }
                        if let Some(sc) = scope_filter {
                            if m.scope != sc {
                                return false;
                            }
                        }
                        true
                    })
                    .take(limit)
                    .collect();
                let count = filtered.len();
                (
                    json!({ "memories": filtered, "count": count }),
                    format!("Listed {} memories", count),
                )
            }
            other => {
                return Err(AroError::Configuration(format!(
                    "unknown memory tool: {other}"
                )))
            }
        };

        Ok(ToolExecutionResult {
            invocation_id: request.invocation_id,
            run_id: request.run_id,
            tool_id: request.tool_id.clone(),
            status: ToolExecutionStatus::Completed,
            title: title.clone(),
            output,
            summary: title,
            context_sources: vec![],
            artifacts: vec![],
            error: None,
            started_at,
            finished_at: Utc::now(),
        })
    }

    // ──────────────────────────────────────────────────────────
    // Context search tool handler
    // ──────────────────────────────────────────────────────────

    async fn execute_context_tool(
        &self,
        request: &ToolExecutionRequest,
    ) -> AroResult<ToolExecutionResult> {
        let started_at = Utc::now();
        let query = request
            .input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let limit = request
            .input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let items = self.store.search_agent_context(&query, limit)?;
        let count = items.len();
        let title = format!(
            "Found {} context items for '{}'",
            count,
            &query[..query.len().min(40)]
        );

        Ok(ToolExecutionResult {
            invocation_id: request.invocation_id,
            run_id: request.run_id,
            tool_id: request.tool_id.clone(),
            status: ToolExecutionStatus::Completed,
            title: title.clone(),
            output: json!({ "items": items, "count": count }),
            summary: title,
            context_sources: vec![],
            artifacts: vec![],
            error: None,
            started_at,
            finished_at: Utc::now(),
        })
    }

    // ──────────────────────────────────────────────────────────
    // Agent orchestration tool handlers
    // ──────────────────────────────────────────────────────────

    async fn execute_orchestration_tool(
        &self,
        parent_run: &AgentRun,
        request: &ToolExecutionRequest,
    ) -> AroResult<ToolExecutionResult> {
        let started_at = Utc::now();
        let (output, title) = match request.tool_id.as_str() {
            TOOL_CORE_AGENT_DELEGATE | TOOL_CORE_AGENT_SPAWN => {
                let goal = request
                    .input
                    .get("goal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let mode_str = request
                    .input
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("code");
                let mode = match mode_str {
                    "chat" => AssistantMode::Chat,
                    "think" => AssistantMode::Think,
                    _ => AssistantMode::Code,
                };
                let max_steps = request
                    .input
                    .get("max_steps")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);
                let system_prompt = request
                    .input
                    .get("system_prompt")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let sub_request = AgentRunStartRequest {
                    run_id: None,
                    lane_id: parent_run.lane_id,
                    conversation_id: parent_run.conversation_id,
                    goal: goal.clone(),
                    mode,
                    system_prompt,
                    model_id: parent_run.model_id.clone(),
                    provider: parent_run.model_provider_id.clone(),
                    autonomy_profile_id: parent_run.autonomy_profile_id,
                    priority: parent_run.priority.clone().into(),
                    max_steps,
                };

                let view = self.start_agent_run(sub_request).await?;
                let status_str = format!("{:?}", view.run.status).to_lowercase();
                (
                    json!({
                        "run_id": view.run.id,
                        "status": status_str,
                        "goal": goal
                    }),
                    format!(
                        "Delegated agent run {} for goal: {}",
                        view.run.id,
                        &goal[..goal.len().min(50)]
                    ),
                )
            }
            TOOL_CORE_AGENT_STATUS => {
                let run_id_str = request
                    .input
                    .get("run_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let run_id = uuid::Uuid::parse_str(run_id_str)
                    .map_err(|e| AroError::Configuration(format!("invalid run_id: {e}")))?;
                let view = self.agent_run_view(run_id)?;
                let status_str = format!("{:?}", view.run.status).to_lowercase();
                let last_step = view.steps.last().cloned();
                (
                    json!({
                        "run_id": view.run.id,
                        "status": status_str,
                        "goal": view.run.goal,
                        "steps_count": view.steps.len(),
                        "last_step": last_step
                    }),
                    format!("Agent run {} status: {}", run_id, status_str),
                )
            }
            other => {
                return Err(AroError::Configuration(format!(
                    "unknown orchestration tool: {other}"
                )))
            }
        };

        Ok(ToolExecutionResult {
            invocation_id: request.invocation_id,
            run_id: request.run_id,
            tool_id: request.tool_id.clone(),
            status: ToolExecutionStatus::Completed,
            title: title.clone(),
            output,
            summary: title,
            context_sources: vec![],
            artifacts: vec![],
            error: None,
            started_at,
            finished_at: Utc::now(),
        })
    }

    // ──────────────────────────────────────────────────────────
    // Skill tool handlers (stub — extensible)
    // ──────────────────────────────────────────────────────────

    async fn execute_skill_tool(
        &self,
        request: &ToolExecutionRequest,
    ) -> AroResult<ToolExecutionResult> {
        let started_at = Utc::now();
        let (output, title, status, err_msg) = match request.tool_id.as_str() {
            TOOL_CORE_SKILL_LIST => {
                let skills = self.plugins.skill_registry().list().await;
                let count = skills.len();
                let skills_val = serde_json::to_value(&skills).unwrap_or_else(|_| json!([]));
                (
                    json!({ "skills": skills_val, "count": count }),
                    format!("Listed {} available skills", count),
                    ToolExecutionStatus::Completed,
                    None,
                )
            }
            TOOL_CORE_SKILL_INVOKE => {
                let skill_id = request
                    .input
                    .get("skill_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = request
                    .input
                    .get("input")
                    .cloned()
                    .unwrap_or(request.input.clone());
                match self
                    .plugins
                    .skill_registry()
                    .invoke(&skill_id, &input)
                    .await
                {
                    Ok(skill_res) => {
                        let status = if skill_res.success {
                            ToolExecutionStatus::Completed
                        } else {
                            ToolExecutionStatus::Failed
                        };
                        let title = if skill_res.success {
                            format!("Skill '{}' invoked successfully", skill_id)
                        } else {
                            format!("Skill '{}' invocation failed", skill_id)
                        };
                        (
                            json!({ "success": skill_res.success, "output": skill_res.output, "error": skill_res.error }),
                            title,
                            status,
                            skill_res.error,
                        )
                    }
                    Err(err) => {
                        let err_str = err.to_string();
                        (
                            json!({ "success": false, "error": err_str }),
                            format!("Skill '{}' not found or failed: {err}", skill_id),
                            ToolExecutionStatus::Failed,
                            Some(err_str),
                        )
                    }
                }
            }
            other => {
                return Err(AroError::Configuration(format!(
                    "unknown skill tool: {other}"
                )))
            }
        };

        Ok(ToolExecutionResult {
            invocation_id: request.invocation_id,
            run_id: request.run_id,
            tool_id: request.tool_id.clone(),
            status,
            title: title.clone(),
            output,
            summary: title,
            context_sources: vec![],
            artifacts: vec![],
            error: err_msg,
            started_at,
            finished_at: Utc::now(),
        })
    }

    // ──────────────────────────────────────────────────────────
    // Connector/plugin tool handlers (stub — extensible)
    // ──────────────────────────────────────────────────────────

    async fn execute_connector_tool(
        &self,
        request: &ToolExecutionRequest,
    ) -> AroResult<ToolExecutionResult> {
        let started_at = Utc::now();
        let (output, title) = match request.tool_id.as_str() {
            TOOL_CORE_CONNECTOR_LIST => {
                // Vrais connecteurs = serveurs MCP des plugins installés.
                let installed = self.plugins.list_installed().await;
                let mut connectors: Vec<serde_json::Value> = Vec::new();
                for plugin in &installed {
                    let accounts = self.plugins.list_accounts(Some(&plugin.id)).unwrap_or_default();
                    let default_account = accounts.iter().find(|a| a.is_default).or_else(|| accounts.first());
                    let accounts_json: Vec<serde_json::Value> = accounts.iter().map(|acc| json!({
                        "id": acc.id,
                        "label": acc.label,
                        "accountIdentifier": acc.account_identifier,
                        "email": acc.email,
                        "displayName": acc.display_name,
                        "isDefault": acc.is_default,
                        "status": acc.status,
                        "authMethod": acc.auth_method,
                    })).collect();
                    let active_account_json = default_account.map(|acc| json!({
                        "id": acc.id,
                        "label": acc.label,
                        "accountIdentifier": acc.account_identifier,
                        "email": acc.email,
                    }));

                    for server in &plugin.mcp_servers {
                        connectors.push(json!({
                            "connectorId": plugin.id,
                            "pluginName": plugin.name,
                            "server": server.name,
                            "transport": server.transport_type,
                            "status": server.status,
                            "enabled": plugin.enabled,
                            "accounts": accounts_json,
                            "activeAccount": active_account_json,
                        }));
                    }
                }
                let count = connectors.len();
                (
                    json!({ "connectors": connectors, "count": count }),
                    format!("Listed {count} connector(s) from installed plugins"),
                )
            }
            TOOL_CORE_CONNECTOR_CALL => {
                let connector_id = request
                    .input
                    .get("connector_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let account_hint = request
                    .input
                    .get("account")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let server = request
                    .input
                    .get("server")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let tool = request
                    .input
                    .get("tool")
                    .or_else(|| request.input.get("action"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = request
                    .input
                    .get("arguments")
                    .or_else(|| request.input.get("input"))
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let installed = self.plugins.list_installed().await;
                // Résolution : id de plugin exact, ou propriétaire du serveur
                // nommé, ou serveur unique du plugin désigné.
                let mut resolved: Option<(String, String)> = None;
                for plugin in &installed {
                    if plugin.id == connector_id && !server.is_empty() {
                        resolved = Some((plugin.id.clone(), server.clone()));
                        break;
                    }
                    if plugin.id == connector_id
                        && server.is_empty()
                        && plugin.mcp_servers.len() == 1
                    {
                        resolved = Some((plugin.id.clone(), plugin.mcp_servers[0].name.clone()));
                        break;
                    }
                    if !connector_id.is_empty()
                        && plugin.mcp_servers.iter().any(|s| s.name == connector_id)
                    {
                        resolved = Some((plugin.id.clone(), connector_id.clone()));
                        break;
                    }
                    if !server.is_empty()
                        && plugin.mcp_servers.iter().any(|s| {
                            s.name == server
                                && (connector_id.is_empty() || plugin.id == connector_id)
                        })
                    {
                        resolved = Some((plugin.id.clone(), server.clone()));
                        break;
                    }
                }
                let Some((plugin_id, server_name)) = resolved else {
                    let error_msg = format!(
                        "Connector '{connector_id}' (server '{server}') is not registered in any installed agent plugin. Install plugins with MCP servers via Settings > Plugins."
                    );
                    return Ok(failed_connector_result(request, &error_msg, started_at));
                };
                match self
                    .plugins
                    .call_mcp_tool(&plugin_id, &server_name, &tool, arguments)
                    .await
                {
                    Ok(call_result) => {
                        let is_error = call_result.is_error.unwrap_or(false);
                        let plain = call_result.plain_text();
                        let status = if is_error {
                            ToolExecutionStatus::Failed
                        } else {
                            ToolExecutionStatus::Completed
                        };
                        let title = format!("Connector call to {server_name}/{tool} completed");
                        return Ok(ToolExecutionResult {
                            invocation_id: request.invocation_id,
                            run_id: request.run_id,
                            tool_id: request.tool_id.clone(),
                            status,
                            title: title.clone(),
                            output: json!({
                                "success": !is_error,
                                "connectorId": plugin_id,
                                "server": server_name,
                                "tool": tool,
                                "account": account_hint,
                                "content": call_result.content,
                                "text": plain,
                            }),
                            summary: title,
                            context_sources: vec![],
                            artifacts: vec![],
                            error: if is_error { Some(plain) } else { None },
                            started_at,
                            finished_at: Utc::now(),
                        });
                    }
                    Err(err) => {
                        let error_msg =
                            format!("Connector call to {server_name}/{tool} failed: {err}");
                        return Ok(failed_connector_result(request, &error_msg, started_at));
                    }
                }
            }
            other => {
                return Err(AroError::Configuration(format!(
                    "unknown connector tool: {other}"
                )))
            }
        };

        Ok(ToolExecutionResult {
            invocation_id: request.invocation_id,
            run_id: request.run_id,
            tool_id: request.tool_id.clone(),
            status: ToolExecutionStatus::Completed,
            title: title.clone(),
            output,
            summary: title,
            context_sources: vec![],
            artifacts: vec![],
            error: None,
            started_at,
            finished_at: Utc::now(),
        })
    }

    // ──────────────────────────────────────────────────────────
    // MCP bridge tool handler
    // ──────────────────────────────────────────────────────────

    async fn execute_mcp_tool(
        &self,
        request: &ToolExecutionRequest,
    ) -> AroResult<ToolExecutionResult> {
        let started_at = Utc::now();
        let server = request
            .input
            .get("server")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let tool = request
            .input
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let plugin_id = request
            .input
            .get("plugin_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let arguments = request
            .input
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let resolved_plugin_id = if !plugin_id.is_empty() {
            plugin_id.to_string()
        } else {
            let installed = self.plugins.list_installed().await;
            let mut found = String::new();
            for p in installed {
                if p.mcp_servers.iter().any(|s| s.name == server) {
                    found = p.id;
                    break;
                }
            }
            found
        };

        if resolved_plugin_id.is_empty() {
            let error_msg = format!(
                "MCP server '{}' is not registered in any installed agent plugin. Install plugins with MCP servers via Settings > Plugins.",
                server
            );
            let title = format!("MCP call to {server}/{tool} failed: server not found");
            return Ok(ToolExecutionResult {
                invocation_id: request.invocation_id,
                run_id: request.run_id,
                tool_id: request.tool_id.clone(),
                status: ToolExecutionStatus::Failed,
                title: title.clone(),
                output: json!({ "success": false, "error": error_msg }),
                summary: title,
                context_sources: vec![],
                artifacts: vec![],
                error: Some(error_msg),
                started_at,
                finished_at: Utc::now(),
            });
        }

        match self
            .plugins
            .call_mcp_tool(&resolved_plugin_id, &server, &tool, arguments)
            .await
        {
            Ok(call_result) => {
                let is_error = call_result.is_error.unwrap_or(false);
                let status = if is_error {
                    ToolExecutionStatus::Failed
                } else {
                    ToolExecutionStatus::Completed
                };
                let plain = call_result.plain_text();
                let title = format!("MCP call to {server}/{tool} completed");
                let err_msg = if is_error { Some(plain.clone()) } else { None };
                Ok(ToolExecutionResult {
                    invocation_id: request.invocation_id,
                    run_id: request.run_id,
                    tool_id: request.tool_id.clone(),
                    status,
                    title: title.clone(),
                    output: json!({
                        "success": !is_error,
                        "content": call_result.content,
                        "text": plain
                    }),
                    summary: title,
                    context_sources: vec![],
                    artifacts: vec![],
                    error: err_msg,
                    started_at,
                    finished_at: Utc::now(),
                })
            }
            Err(err) => {
                let err_str = err.to_string();
                let title = format!("MCP call to {server}/{tool} failed: {err}");
                Ok(ToolExecutionResult {
                    invocation_id: request.invocation_id,
                    run_id: request.run_id,
                    tool_id: request.tool_id.clone(),
                    status: ToolExecutionStatus::Failed,
                    title: title.clone(),
                    output: json!({ "success": false, "error": err_str }),
                    summary: title,
                    context_sources: vec![],
                    artifacts: vec![],
                    error: Some(err_str),
                    started_at,
                    finished_at: Utc::now(),
                })
            }
        }
    }
}

struct AgentLoopOutcome {
    content: String,
    token_estimate: Option<u32>,
    status: AgentRunStatus,
    last_error: Option<String>,
    next_sequence: i32,
}

struct PrefetchOutcome {
    sources: Vec<ContextSource>,
    next_sequence: i32,
}

fn web_policy_for(
    mode: WebAccessMode,
    settings: Option<&aro_core::settings::SearchSettings>,
) -> WebAccessPolicy {
    let mut policy = match mode {
        WebAccessMode::Off | WebAccessMode::Auto => WebAccessPolicy::disabled(),
        WebAccessMode::On => WebAccessPolicy::unrestricted(),
    };
    if let Some(s) = settings {
        policy = policy.with_search_settings(
            Some(s.provider.clone()),
            s.api_key.clone(),
            s.endpoint.clone(),
        );
    }
    policy
}

fn failed_connector_result(
    request: &ToolExecutionRequest,
    error: &str,
    started_at: chrono::DateTime<chrono::Utc>,
) -> ToolExecutionResult {
    ToolExecutionResult {
        invocation_id: request.invocation_id,
        run_id: request.run_id,
        tool_id: request.tool_id.clone(),
        status: ToolExecutionStatus::Failed,
        title: error.to_string(),
        output: json!({ "success": false, "error": error }),
        summary: error.to_string(),
        context_sources: vec![],
        artifacts: vec![],
        error: Some(error.to_string()),
        started_at,
        finished_at: Utc::now(),
    }
}

fn failed_tool_result(request: &ToolExecutionRequest, error: String) -> ToolExecutionResult {
    let now = Utc::now();
    ToolExecutionResult {
        invocation_id: request.invocation_id,
        run_id: request.run_id,
        tool_id: request.tool_id.clone(),
        status: ToolExecutionStatus::Failed,
        title: format!("Tool {} failed", request.tool_id),
        output: json!({ "error": error }),
        summary: format!("Tool `{}` failed: {}", request.tool_id, error),
        context_sources: vec![ContextSource {
            id: format!("tool:{}:error", request.invocation_id),
            kind: "tool-error".to_string(),
            title: format!("Tool {} failed", request.tool_id),
            excerpt: error.clone(),
            uri: None,
            score: 0.0,
            created_at: Some(now),
        }],
        artifacts: Vec::new(),
        error: Some(error),
        started_at: request.requested_at,
        finished_at: now,
    }
}

fn emit_final_chunks(on_chunk: &mut Option<&mut (dyn FnMut(String) + Send)>, content: &str) {
    if let Some(callback) = on_chunk.as_deref_mut() {
        for chunk in content.split_inclusive(' ') {
            callback(chunk.to_string());
        }
    }
}

fn assistant_content_for_action(action: &AgentAction, validation_error: Option<&str>) -> String {
    if let Some(error) = validation_error {
        return format!(
            "I could not continue because the model returned an invalid agent action: {error}"
        );
    }

    match action.action_type {
        AgentActionType::Final => action.content.clone().unwrap_or_default(),
        AgentActionType::Tool => {
            let tool_id = action.tool_id.as_deref().unwrap_or("unknown tool");
            format!("I need to use `{tool_id}` next, so I paused this run and recorded the requested action in the agent timeline.")
        }
        AgentActionType::Pause => action
            .reason
            .clone()
            .unwrap_or_else(|| "I paused this run because more input is required.".to_string()),
    }
}

fn make_title(input: &str) -> String {
    let mut title = input
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    if title.chars().count() > 64 {
        title = title.chars().take(64).collect();
    }
    if title.is_empty() {
        "New conversation".to_string()
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use aro_core::{ModelGeneration, ModelGenerationRequest, ModelProviderKind};

    #[test]
    fn creates_compact_titles() {
        assert_eq!(make_title("hello world"), "hello world");
        assert_eq!(
            make_title("one two three four five six seven eight nine"),
            "one two three four five six seven eight"
        );
    }

    #[test]
    fn agent_run_started_step_records_unlimited_budget() {
        let db_path = std::env::temp_dir().join(format!(
            "aro-runtime-unlimited-step-test-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let engine = AssistantEngine::new(
            SqliteMemoryStore::new(&db_path).expect("temporary runtime store"),
        );
        let run = AgentRun::new(
            "unlimited goal",
            AssistantMode::Code,
            None,
            Some("test".to_string()),
            Some("test".to_string()),
            None,
        );
        let step = engine.run_started_step(&run, None);
        assert_eq!(step.input["maxSteps"], Value::Null);

        drop(engine);
        std::fs::remove_file(db_path).expect("remove temporary runtime store");
    }

    #[test]
    fn web_auto_is_deny_by_default_while_on_remains_explicit() {
        let automatic = web_policy_for(WebAccessMode::Auto, None);
        assert!(!automatic.allow_network);
        assert!(automatic.allowed_domains.is_empty());
        assert!(!automatic.domain_allowed("example.com"));

        let explicit = web_policy_for(WebAccessMode::On, None);
        assert!(explicit.allow_network);
        assert!(explicit.domain_allowed("example.com"));
    }

    #[tokio::test]
    async fn web_auto_does_not_attempt_url_prefetch() {
        let db_path = std::env::temp_dir().join(format!(
            "aro-runtime-auto-prefetch-test-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let engine = AssistantEngine::new(
            SqliteMemoryStore::new(&db_path).expect("temporary runtime store"),
        );
        let run = AgentRun::new(
            "read https://example.com",
            AssistantMode::Chat,
            None,
            Some("test".to_string()),
            Some("test".to_string()),
            None,
        );
        let mut on_step = None;

        let outcome = engine
            .prefetch_url_context(
                &run,
                "read https://example.com",
                WebAccessMode::Auto,
                None,
                7,
                &mut on_step,
            )
            .await
            .expect("automatic web mode should skip prefetch");

        assert!(outcome.sources.is_empty());
        assert_eq!(outcome.next_sequence, 7);
        assert!(engine
            .store
            .list_agent_steps(run.id)
            .expect("stored steps")
            .is_empty());

        drop(engine);
        std::fs::remove_file(db_path).expect("remove temporary runtime store");
    }

    struct FinalAfterNProvider {
        calls: AtomicUsize,
        tool_calls_before_final: usize,
    }

    impl FinalAfterNProvider {
        fn new(tool_calls_before_final: usize) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                tool_calls_before_final,
            }
        }
    }

    #[async_trait::async_trait]
    impl ModelProvider for FinalAfterNProvider {
        async fn generate(&self, _request: ModelGenerationRequest) -> AroResult<ModelGeneration> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let content = if call < self.tool_calls_before_final {
                json!({
                    "type": "tool",
                    "toolId": TOOL_CORE_WEB_PAGE_READ,
                    "input": { "url": "https://example.com", "maxChars": 1000 },
                    "reason": "continue working"
                })
                .to_string()
            } else {
                json!({
                    "type": "final",
                    "content": "done after many steps"
                })
                .to_string()
            };
            Ok(ModelGeneration {
                content,
                provider_detail: "final-after-n test provider".to_string(),
                token_estimate: Some(1),
            })
        }

        async fn generate_stream(
            &self,
            request: ModelGenerationRequest,
            on_chunk: &mut (dyn FnMut(String) + Send),
        ) -> AroResult<ModelGeneration> {
            let generation = self.generate(request).await?;
            on_chunk(generation.content.clone());
            Ok(generation)
        }

        async fn status(&self) -> RuntimeStatus {
            RuntimeStatus {
                model_provider: ModelProviderKind::Mock,
                model_id: "final-after-n-test".to_string(),
                model_ready: true,
                voice_ready: false,
                endpoint: None,
                detail: "test provider is ready".to_string(),
                checked_at: Utc::now(),
            }
        }

        fn temperature(&self) -> f32 {
            0.0
        }

        fn max_tokens(&self) -> u32 {
            128
        }
    }

    #[tokio::test]
    async fn tool_loop_runs_past_the_old_default_limit_until_final() {
        assert_tool_loop_completes_after(12).await;
    }

    #[tokio::test]
    async fn tool_loop_runs_past_the_old_hard_cap_until_final() {
        assert_tool_loop_completes_after(40).await;
    }

    async fn assert_tool_loop_completes_after(tool_calls_before_final: usize) {
        let db_path = std::env::temp_dir().join(format!(
            "aro-runtime-step-limit-test-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let engine = AssistantEngine::new(
            SqliteMemoryStore::new(&db_path).expect("temporary runtime store"),
        );
        let mut run = AgentRun::new(
            "keep working until done",
            AssistantMode::Code,
            None,
            Some("test".to_string()),
            Some("repeating-test".to_string()),
            None,
        );
        run.status = AgentRunStatus::Running;
        engine.store.upsert_agent_run(&run).expect("store run");
        let context_pack =
            engine
                .agent
                .build_context_pack(&run, &[], &[], &[], EnvironmentSnapshot::default());
        let provider = FinalAfterNProvider::new(tool_calls_before_final);
        let mut on_step = None;

        let outcome = engine
            .run_tool_loop(
                &mut run,
                Vec::new(),
                Vec::new(),
                context_pack,
                "test system prompt".to_string(),
                "keep going".to_string(),
                &provider,
                WebAccessMode::Off,
                None,
                None,
                1,
                None,
                &mut on_step,
            )
            .await
            .expect("unbounded loop outcome");

        assert_eq!(outcome.status, AgentRunStatus::Completed);
        assert_eq!(outcome.content, "done after many steps");
        assert!(outcome.last_error.is_none());
        let steps = engine.store.list_agent_steps(run.id).expect("stored steps");
        assert_eq!(
            steps
                .iter()
                .filter(|step| matches!(step.kind, AgentStepKind::Model))
                .count(),
            tool_calls_before_final + 1
        );
        assert!(steps
            .iter()
            .all(|step| !matches!(step.kind, AgentStepKind::Error)));

        drop(engine);
        std::fs::remove_file(db_path).expect("remove temporary runtime store");
    }
}
