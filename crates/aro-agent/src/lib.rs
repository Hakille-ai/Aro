use std::{
    env,
    path::{Path, PathBuf},
};

use aro_core::{
    AgentAction, AgentActionType, AgentLane, AgentLaneStatus, AgentRun, AgentRunStartRequest,
    AgentRunStatus, AgentStep, AgentStepKind, ChatMessage, ContextPack, ContextSource, MessageRole,
    ModelGenerationRequest, ModelResponseFormat, PermissionProfile, ToolCategory,
    ToolConfirmationPolicy, ToolDataCaptureMode, ToolDependency, ToolDependencyKind,
    ToolDescriptor, ToolExecutionEnvironment, ToolExecutionKind, ToolExecutionSpec,
    ToolIdempotency, ToolObservability, ToolOwner, ToolOwnerKind, ToolPermissionEffect,
    ToolPermissionRequirement, ToolProvenance, ToolProvenanceKind, ToolRef, ToolRetryPolicy,
    ToolRetryStrategy, ToolRisk, ToolRiskLevel, ToolSideEffects, ToolSource, ToolStatus,
    ToolUsageLimits, TOOL_CODE_EXECUTE, TOOL_CORE_AGENT_DELEGATE, TOOL_CORE_AGENT_SPAWN,
    TOOL_CORE_AGENT_STATUS, TOOL_CORE_CODE_EXECUTE, TOOL_CORE_CONNECTOR_CALL,
    TOOL_CORE_CONNECTOR_LIST, TOOL_CORE_CONTEXT_SEARCH, TOOL_CORE_DOCUMENT_CREATE,
    TOOL_CORE_MCP_CALL, TOOL_CORE_MEMORY_DELETE, TOOL_CORE_MEMORY_FORGET, TOOL_CORE_MEMORY_LIST,
    TOOL_CORE_MEMORY_RECALL, TOOL_CORE_MEMORY_SAVE, TOOL_CORE_MEMORY_SEARCH,
    TOOL_CORE_MEMORY_UPDATE, TOOL_CORE_SEARCH_WEB, TOOL_CORE_SHELL_EXECUTE, TOOL_CORE_SKILL_INVOKE,
    TOOL_CORE_SKILL_LIST, TOOL_CORE_WEB_PAGE_READ, TOOL_CORE_WORKSPACE_GREP,
    TOOL_CORE_WORKSPACE_LIST, TOOL_CORE_WORKSPACE_READ, TOOL_CORE_WORKSPACE_SEARCH,
    TOOL_CORE_WORKSPACE_WRITE, TOOL_DESCRIPTOR_SCHEMA_VERSION, TOOL_DOCUMENT_CREATE,
    TOOL_MEMORY_DELETE, TOOL_MEMORY_FORGET, TOOL_MEMORY_LIST, TOOL_MEMORY_RECALL,
    TOOL_MEMORY_SAVE, TOOL_MEMORY_SEARCH, TOOL_MEMORY_UPDATE, TOOL_WEB_FETCH, TOOL_WEB_SEARCH,
    normalize_tool_id,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

pub mod context;
pub use context::*;

#[derive(Debug, Clone)]
pub struct AgentRuntime {
    context_builder: ContextBuilder,
    tool_registry: ToolRegistry,
    checkpoint_service: CheckpointService,
    orchestrator: RunOrchestrator,
}

impl Default for AgentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self {
            context_builder: ContextBuilder::new(),
            tool_registry: ToolRegistry::default(),
            checkpoint_service: CheckpointService,
            orchestrator: RunOrchestrator::default(),
        }
    }

    pub fn start_run(
        &self,
        request: &AgentRunStartRequest,
        autonomy_profile_id: Option<Uuid>,
    ) -> AgentRun {
        let mut run = AgentRun::new(
            request.goal.trim(),
            request.mode.clone(),
            request.conversation_id,
            request.provider.clone(),
            request.model_id.clone(),
            autonomy_profile_id,
        );
        if let Some(run_id) = request.run_id {
            run.id = run_id;
        }
        run.lane_id = request.lane_id;
        run.priority = request.priority.clone().unwrap_or_default();
        run
    }

    pub fn schedule_run(
        &self,
        run: &mut AgentRun,
        lane: &AgentLane,
        global_running: u32,
        lane_running: u32,
    ) {
        self.orchestrator
            .schedule_run(run, lane, global_running, lane_running);
    }

    pub fn max_global_running(&self) -> u32 {
        self.orchestrator.max_global_running
    }

    pub fn build_context_pack(
        &self,
        run: &AgentRun,
        history: &[ChatMessage],
        memories: &[ContextSource],
        skills: &[ToolRef],
        environment: EnvironmentSnapshot,
    ) -> ContextPack {
        self.context_builder.build(
            run,
            history,
            memories,
            skills,
            self.tool_registry.enabled_tools(),
            environment,
        )
    }

    pub fn run_started_step(&self, run: &AgentRun) -> AgentStep {
        AgentStep::completed(
            run.id,
            1,
            AgentStepKind::RunStarted,
            "Run started",
            json!({ "goal": run.goal, "mode": run.mode }),
            json!({ "status": run.status }),
        )
    }

    pub fn context_step(&self, run: &AgentRun, context_pack: &ContextPack) -> AgentStep {
        self.context_step_at(run, 2, context_pack)
    }

    pub fn context_step_at(
        &self,
        run: &AgentRun,
        sequence: i32,
        context_pack: &ContextPack,
    ) -> AgentStep {
        AgentStep::completed(
            run.id,
            sequence,
            AgentStepKind::ContextBuilt,
            "Context pack built",
            json!({
                "goal": context_pack.goal,
                "sourceCount": context_pack.sources.len(),
                "toolCount": context_pack.tools.len()
            }),
            serde_json::to_value(context_pack).unwrap_or(Value::Null),
        )
    }

    pub fn checkpoint_step(
        &self,
        run: &AgentRun,
        sequence: i32,
        messages: &[ChatMessage],
    ) -> AgentStep {
        let summary = self.checkpoint_service.summarize(run, messages);
        AgentStep::completed(
            run.id,
            sequence,
            AgentStepKind::Checkpoint,
            "Checkpoint saved",
            json!({ "messageCount": messages.len() }),
            json!({ "summary": summary }),
        )
    }

    pub fn final_step(&self, run: &AgentRun, sequence: i32, content: &str) -> AgentStep {
        AgentStep::completed(
            run.id,
            sequence,
            AgentStepKind::Final,
            "Final response",
            json!({}),
            json!({ "content": content }),
        )
    }

    pub fn model_step(
        &self,
        run: &AgentRun,
        sequence: i32,
        raw_output: &str,
        action: &AgentAction,
        validation_error: Option<&str>,
    ) -> AgentStep {
        let input = json!({
            "rawOutput": compact_excerpt(raw_output, 4000),
        });
        let output = json!({
            "action": action,
            "valid": validation_error.is_none(),
        });
        match validation_error {
            Some(error) => AgentStep::failed(
                run.id,
                sequence,
                AgentStepKind::Model,
                "Model action rejected",
                input,
                error,
            ),
            None => AgentStep::completed(
                run.id,
                sequence,
                AgentStepKind::Model,
                "Model action accepted",
                input,
                output,
            ),
        }
    }

    pub fn tool_requested_step(
        &self,
        run: &AgentRun,
        sequence: i32,
        action: &AgentAction,
    ) -> AgentStep {
        AgentStep::skipped(
            run.id,
            sequence,
            AgentStepKind::Tool,
            "Tool requested",
            json!({ "action": action }),
            json!({ "reason": "Tool execution is queued for the multi-step run worker." }),
        )
    }

    pub fn parse_model_action(&self, output: &str) -> AgentAction {
        parse_agent_action(output)
            .unwrap_or_else(|| AgentAction::final_response(output.trim().to_string()))
    }

    pub fn validate_action(
        &self,
        action: &AgentAction,
        context_pack: &ContextPack,
    ) -> Result<(), String> {
        match action.action_type {
            AgentActionType::Final => {
                let content = action.content.as_deref().unwrap_or_default().trim();
                if content.is_empty() {
                    Err("final action must include non-empty content".to_string())
                } else {
                    Ok(())
                }
            }
            AgentActionType::Tool => {
                let tool_id = action
                    .tool_id
                    .as_deref()
                    .ok_or_else(|| "tool action must include toolId".to_string())?;
                let is_enabled = context_pack.tools.iter().any(|tool| {
                    tool.id == tool_id
                        || tool.aliases.iter().any(|a| a == tool_id)
                        || normalize_tool_id(&tool.id) == normalize_tool_id(tool_id)
                        || tool.aliases.iter().any(|a| normalize_tool_id(a) == normalize_tool_id(tool_id))
                });
                if !is_enabled {
                    return Err(format!(
                        "tool `{tool_id}` is not enabled in the context pack"
                    ));
                }
                if !(action.input.is_object() || action.input.is_null()) {
                    return Err("tool action input must be a JSON object".to_string());
                }
                Ok(())
            }
            AgentActionType::Pause => {
                let reason = action.reason.as_deref().unwrap_or_default().trim();
                if reason.is_empty() {
                    Err("pause action must include a reason".to_string())
                } else {
                    Ok(())
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn model_request(
        &self,
        run: &AgentRun,
        context_pack: &ContextPack,
        system_prompt: String,
        messages: Vec<ChatMessage>,
        user_input: String,
        temperature: f32,
        max_tokens: u32,
        response_format: ModelResponseFormat,
    ) -> ModelGenerationRequest {
        let agent_context = render_context_pack(context_pack);
        let response_contract = match response_format {
            ModelResponseFormat::DirectText => {
                "Response contract:\n- Answer the user directly in natural text.\n- Do not wrap the answer in JSON.\n- Use Markdown only when it improves readability.\n- Cite source IDs only when context sources materially affect the answer."
            }
            ModelResponseFormat::AgentActionJson => {
                "Response contract:\nReturn exactly one JSON object and no markdown. Use {\"type\":\"final\",\"content\":\"...\"} when answering the user, {\"type\":\"tool\",\"toolId\":\"core.search.web\",\"input\":{\"query\":\"...\"},\"reason\":\"...\"} or another listed tool when external, current, file, or page context is required, or {\"type\":\"pause\",\"reason\":\"...\"} when user input is required. For URLs the user provides, prefer core.web.page.read. Cite source IDs in final content when context sources matter."
            }
        };
        ModelGenerationRequest {
            mode: run.mode.clone(),
            system_prompt: format!(
                "{system_prompt}\n\nAgent runtime context:\n{agent_context}\n\n{response_contract}"
            ),
            messages,
            user_input,
            temperature,
            max_tokens,
            response_format,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunOrchestrator {
    pub max_global_running: u32,
}

impl Default for RunOrchestrator {
    fn default() -> Self {
        Self {
            max_global_running: 3,
        }
    }
}

impl RunOrchestrator {
    pub fn schedule_run(
        &self,
        run: &mut AgentRun,
        lane: &AgentLane,
        global_running: u32,
        lane_running: u32,
    ) {
        run.lane_id = Some(lane.id);
        if run.priority < lane.priority {
            run.priority = lane.priority.clone();
        }
        run.status = if lane.status == AgentLaneStatus::Paused
            || global_running >= self.max_global_running
            || lane_running >= lane.max_concurrent_runs
        {
            AgentRunStatus::Queued
        } else {
            AgentRunStatus::Running
        };
    }
}

#[derive(Debug, Clone)]
pub struct EnvironmentSnapshot {
    pub workspace_root: Option<String>,
    pub platform: String,
    pub runtime: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub timestamp_utc: DateTime<Utc>,
}

impl Default for EnvironmentSnapshot {
    fn default() -> Self {
        Self {
            workspace_root: None,
            platform: std::env::consts::OS.to_string(),
            runtime: "desktop-local".to_string(),
            provider_id: None,
            model_id: None,
            timestamp_utc: Utc::now(),
        }
    }
}

impl EnvironmentSnapshot {
    pub fn desktop_local(provider_id: Option<String>, model_id: Option<String>) -> Self {
        Self {
            workspace_root: env::var("ARO_TRUSTED_WORKSPACE_ROOT").ok(),
            provider_id,
            model_id,
            ..Self::default()
        }
    }

    pub fn api_durable(provider_id: Option<String>, model_id: Option<String>) -> Self {
        Self {
            runtime: "api-durable".to_string(),
            provider_id,
            model_id,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContextBuilder {
    max_sources: usize,
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self { max_sources: 32 }
    }

    pub fn build(
        &self,
        run: &AgentRun,
        history: &[ChatMessage],
        memories: &[ContextSource],
        skills: &[ToolRef],
        builtin_tools: Vec<ToolRef>,
        environment: EnvironmentSnapshot,
    ) -> ContextPack {
        let mut sources = Vec::new();
        sources.push(ContextSource {
            id: format!("run:{}", run.id),
            kind: "run".to_string(),
            title: "Current goal".to_string(),
            excerpt: run.goal.clone(),
            uri: None,
            score: 1.0,
            created_at: Some(run.created_at),
        });
        sources.push(ContextSource {
            id: format!("environment:{}", run.id),
            kind: "environment".to_string(),
            title: "Runtime environment".to_string(),
            excerpt: render_environment_snapshot(&environment),
            uri: environment
                .workspace_root
                .as_ref()
                .map(|root| format!("workspace://{}", compact_excerpt(root, 240))),
            score: 0.9,
            created_at: Some(environment.timestamp_utc),
        });

        for message in history.iter().rev().take(8).rev() {
            sources.push(ContextSource {
                id: format!("message:{}", message.id),
                kind: "message".to_string(),
                title: match message.role {
                    MessageRole::User => "Recent user message".to_string(),
                    MessageRole::Assistant => "Recent assistant message".to_string(),
                    MessageRole::System => "System message".to_string(),
                },
                excerpt: compact_excerpt(&message.content, 600),
                uri: Some(format!("conversation://{}", message.conversation_id)),
                score: 0.8,
                created_at: Some(message.created_at),
            });
        }

        // Partition memories into dynamic tool results vs static background memories
        let mut dynamic_tool_sources = Vec::new();
        let mut static_memories = Vec::new();
        for mem in memories {
            if mem.kind.contains("tool")
                || mem.kind.contains("workspace")
                || mem.kind.contains("code")
                || mem.kind.contains("shell")
                || mem.kind.contains("web")
                || mem.kind.contains("document")
                || mem.kind.contains("artifact")
            {
                dynamic_tool_sources.push(mem.clone());
            } else {
                static_memories.push(mem.clone());
            }
        }

        // Retain the most recent dynamic tool sources
        let recent_tools: Vec<ContextSource> = dynamic_tool_sources
            .into_iter()
            .rev()
            .take(12)
            .rev()
            .collect();
        sources.extend(recent_tools);
        // And include relevant static memories
        sources.extend(static_memories.into_iter().take(6));
        sources.truncate(self.max_sources);

        let mut tools = builtin_tools;
        tools.extend(skills.iter().filter(|tool| tool.enabled).cloned());

        let summary = format!(
            "Goal: {}. Environment: {} on {}. Model: {}. Sources: {}. Tools: {}.",
            compact_excerpt(&run.goal, 240),
            environment.runtime,
            environment.platform,
            environment.model_id.as_deref().unwrap_or("not specified"),
            sources.len(),
            tools.len()
        );

        ContextPack {
            id: Uuid::new_v4(),
            run_id: run.id,
            goal: run.goal.clone(),
            token_estimate: estimate_tokens(&summary)
                + sources
                    .iter()
                    .map(|source| estimate_tokens(&source.excerpt))
                    .sum::<u32>(),
            summary,
            sources,
            tools,
            built_at: Utc::now(),
        }
    }
}

impl Default for ContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct ToolRegistry {
    descriptors: Vec<ToolDescriptor>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        let descriptors = vec![
            // Web tools
            core_web_search_descriptor(),
            core_web_page_read_descriptor(),
            // Workspace tools
            workspace_write_descriptor(),
            workspace_read_descriptor(),
            workspace_list_descriptor(),
            workspace_grep_descriptor(),
            workspace_search_descriptor(),
            // Shell & Code execution & Document creation
            shell_execute_descriptor(),
            core_code_execute_descriptor(),
            core_document_create_descriptor(),
            // Plan tools
            plan_create_descriptor(),
            plan_update_descriptor(),
            plan_delete_descriptor(),
            plan_list_descriptor(),
            // Memory tools
            memory_save_descriptor(),
            memory_search_descriptor(),
            memory_recall_descriptor(),
            memory_update_descriptor(),
            memory_forget_descriptor(),
            memory_list_descriptor(),
            memory_delete_descriptor(),
            // Context tools
            context_search_descriptor(),
            // Orchestration tools
            agent_delegate_descriptor(),
            agent_spawn_descriptor(),
            agent_status_descriptor(),
            // MCP bridge
            mcp_call_descriptor(),
            // Skill tools
            skill_list_descriptor(),
            skill_invoke_descriptor(),
            // Connector/plugin tools
            connector_list_descriptor(),
            connector_call_descriptor(),
        ];
        for descriptor in &descriptors {
            descriptor
                .validate()
                .unwrap_or_else(|error| panic!("invalid built-in tool descriptor: {error}"));
        }
        Self { descriptors }
    }
}

impl ToolRegistry {
    pub fn enabled_tools(&self) -> Vec<ToolRef> {
        self.descriptors
            .iter()
            .filter(|descriptor| descriptor.status == ToolStatus::Active)
            .map(tool_ref_from_descriptor)
            .collect()
    }

    pub fn descriptors(&self) -> &[ToolDescriptor] {
        &self.descriptors
    }
}

/// Returns the validated core catalogue used when no tenant override exists.
///
/// Callers receive owned descriptors so merging or filtering cannot mutate the agent runtime's
/// built-in registry.
pub fn built_in_tool_descriptors() -> Vec<ToolDescriptor> {
    ToolRegistry::default().descriptors().to_vec()
}

#[derive(Debug, Clone)]
pub struct PermissionPolicy {
    profile: PermissionProfile,
}

impl PermissionPolicy {
    pub fn new(profile: PermissionProfile) -> Self {
        Self { profile }
    }

    pub fn can_read_path(&self, path: impl AsRef<Path>) -> bool {
        self.profile.allow_read && self.path_in_trusted_root(path)
    }

    pub fn can_write_path(&self, path: impl AsRef<Path>) -> bool {
        self.profile.allow_write && self.path_in_trusted_root(path)
    }

    pub fn can_run_command(&self, cwd: impl AsRef<Path>, command: &str) -> bool {
        self.profile.allow_shell
            && self.path_in_trusted_root(cwd)
            && !looks_like_secret_export(command)
            && !looks_destructive_outside_scope(command)
    }

    pub fn can_access_domain(&self, domain: &str) -> bool {
        if !self.profile.allow_network {
            return false;
        }
        let domain = domain.trim_end_matches('.').to_ascii_lowercase();
        self.profile.allowed_domains.iter().any(|allowed| {
            let allowed = allowed.trim().trim_end_matches('.').to_ascii_lowercase();
            if allowed == "*" {
                return true;
            }
            let suffix = allowed.strip_prefix("*.").unwrap_or(&allowed);
            domain == suffix || domain.ends_with(&format!(".{suffix}"))
        })
    }

    fn path_in_trusted_root(&self, path: impl AsRef<Path>) -> bool {
        let Ok(path) = normalize_path(path.as_ref()) else {
            return false;
        };
        self.profile.trusted_roots.iter().any(|root| {
            normalize_path(Path::new(root))
                .map(|trusted| path.starts_with(trusted))
                .unwrap_or(false)
        })
    }
}

#[derive(Debug, Clone)]
pub struct CheckpointService;

impl CheckpointService {
    pub fn summarize(&self, run: &AgentRun, messages: &[ChatMessage]) -> String {
        let last_user = messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::User)
            .map(|message| compact_excerpt(&message.content, 240))
            .unwrap_or_else(|| "No user message yet.".to_string());
        format!(
            "Run {} is focused on: {}. Last user input: {}",
            run.id,
            compact_excerpt(&run.goal, 240),
            last_user
        )
    }
}

fn tool_ref_from_descriptor(descriptor: &ToolDescriptor) -> ToolRef {
    ToolRef {
        id: descriptor.id.clone(),
        name: descriptor.name.clone(),
        description: descriptor.description.clone(),
        source: match descriptor.provenance.kind {
            ToolProvenanceKind::Skill => ToolSource::Skill,
            ToolProvenanceKind::Mcp => ToolSource::Mcp,
            ToolProvenanceKind::Plugin => ToolSource::Plugin,
            ToolProvenanceKind::Core
            | ToolProvenanceKind::User
            | ToolProvenanceKind::Organization
            | ToolProvenanceKind::Generated => ToolSource::BuiltIn,
        },
        enabled: descriptor.status == ToolStatus::Active,
        dangerous: matches!(
            descriptor.risk.level,
            ToolRiskLevel::High | ToolRiskLevel::Critical
        ),
        input_schema: descriptor.input_schema.clone(),
        aliases: descriptor.aliases.clone(),
    }
}

fn core_web_search_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_SEARCH_WEB.to_string(),
        version: "1.0.0".to_string(),
        name: "Search web".to_string(),
        description: "Search approved public web pages for current or external information."
            .to_string(),
        category: ToolCategory::Search,
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "minLength": 1 },
                "limit": { "type": "integer", "minimum": 1, "maximum": 10 },
                "freshnessDays": { "type": ["integer", "null"], "minimum": 0 },
                "domains": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "results": { "type": "array" },
                "provider": { "type": "string" },
                "searchedAt": { "type": "string" }
            },
            "required": ["query", "results", "provider", "searchedAt"],
            "additionalProperties": false
        }),
        permissions: vec![ToolPermissionRequirement {
            action: "network.read".to_string(),
            resource: "destination:${input.domains}".to_string(),
        }],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![ToolPermissionEffect::ExternalRead],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["search.web".to_string(), "search.current".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "search.web.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 15_000,
        retry: ToolRetryPolicy {
            max_attempts: 3,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 250,
            max_delay_ms: 4_000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 20,
            rate_per_minute: 120,
            max_input_bytes: 32 * 1_024,
            max_output_bytes: 1_024 * 1_024,
        },
        dependencies: vec![ToolDependency {
            kind: ToolDependencyKind::Connector,
            id: "search-provider".to_string(),
            optional: false,
        }],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::MetadataOnly,
            record_output: ToolDataCaptureMode::ReferenceOnly,
            metrics_namespace: "aro_tool_search_web".to_string(),
            cost_unit: Some("request".to_string()),
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-search".to_string(),
        },
        aliases: vec![TOOL_WEB_SEARCH.to_string()],
        tags: vec!["web".to_string(), "read-only".to_string()],
    }
}

fn core_web_page_read_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WEB_PAGE_READ.to_string(),
        version: "1.0.0".to_string(),
        name: "Read web page".to_string(),
        description: "Fetch and extract readable text from an approved public HTTP(S) page."
            .to_string(),
        category: ToolCategory::Web,
        input_schema: json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "minLength": 1 },
                "maxChars": { "type": "integer", "minimum": 1_000, "maximum": 24_000 }
            },
            "required": ["url"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "url": { "type": "string" },
                "finalUrl": { "type": "string" },
                "title": { "type": "string" },
                "excerpt": { "type": "string" },
                "content": { "type": "string" },
                "contentHash": { "type": "string" },
                "status": { "type": "integer" },
                "fetchedAt": { "type": "string" }
            },
            "required": ["url", "finalUrl", "title", "excerpt", "content", "contentHash", "status", "fetchedAt"],
            "additionalProperties": false
        }),
        permissions: vec![ToolPermissionRequirement {
            action: "network.read".to_string(),
            resource: "destination:${input.url}".to_string(),
        }],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![ToolPermissionEffect::ExternalRead],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["web.page-read".to_string(), "web.citations".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "web.page-read.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 15_000,
        retry: ToolRetryPolicy {
            max_attempts: 2,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 250,
            max_delay_ms: 2_000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 20,
            rate_per_minute: 120,
            max_input_bytes: 8 * 1_024,
            max_output_bytes: 2 * 1_024 * 1_024,
        },
        dependencies: Vec::new(),
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::MetadataOnly,
            record_output: ToolDataCaptureMode::ReferenceOnly,
            metrics_namespace: "aro_tool_web_page_read".to_string(),
            cost_unit: Some("request".to_string()),
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-search".to_string(),
        },
        aliases: vec![TOOL_WEB_FETCH.to_string()],
        tags: vec!["web".to_string(), "read-only".to_string()],
    }
}

fn plan_create_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: "core.plan.create".to_string(),
        version: "1.0.0".to_string(),
        name: "Create plan".to_string(),
        description: "Create a new project plan or roadmap step checklist with a title, description, and list of tasks.".to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {
                "title": { "type": "string", "minLength": 1 },
                "description": { "type": "string" },
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string", "minLength": 1 },
                            "completed": { "type": "boolean" }
                        },
                        "required": ["text"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["title"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "plan": { "type": "object" }
            },
            "required": ["success", "plan"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["plan.create".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "plan.create.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy {
            max_attempts: 3,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 100,
            max_delay_ms: 1_000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 30,
            max_input_bytes: 8 * 1024,
            max_output_bytes: 8 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_plan_create".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-agent".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![],
        tags: vec!["plan".to_string()],
    }
}

fn plan_update_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: "core.plan.update".to_string(),
        version: "1.0.0".to_string(),
        name: "Update plan".to_string(),
        description:
            "Update an existing project plan's title, description, status, or task checklist."
                .to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$" },
                "title": { "type": "string", "minLength": 1 },
                "description": { "type": "string" },
                "status": { "type": "string", "enum": ["active", "completed", "archived"] },
                "tasks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "text": { "type": "string", "minLength": 1 },
                            "completed": { "type": "boolean" }
                        },
                        "required": ["text"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["id"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "plan": { "type": "object" }
            },
            "required": ["success", "plan"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["plan.update".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "plan.update.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy {
            max_attempts: 3,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 100,
            max_delay_ms: 1_000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 30,
            max_input_bytes: 8 * 1024,
            max_output_bytes: 8 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_plan_update".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-agent".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![],
        tags: vec!["plan".to_string()],
    }
}

fn plan_delete_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: "core.plan.delete".to_string(),
        version: "1.0.0".to_string(),
        name: "Delete plan".to_string(),
        description: "Permanently delete a project plan by its ID.".to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$" }
            },
            "required": ["id"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" }
            },
            "required": ["success"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["plan.delete".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "plan.delete.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Irreversible,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy {
            max_attempts: 3,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 100,
            max_delay_ms: 1_000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 30,
            max_input_bytes: 8 * 1024,
            max_output_bytes: 8 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_plan_delete".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-agent".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![],
        tags: vec!["plan".to_string()],
    }
}

fn plan_list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: "core.plan.list".to_string(),
        version: "1.0.0".to_string(),
        name: "List plans".to_string(),
        description: "List all existing plans and checklists with their current statuses, descriptions, and tasks.".to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "plans": {
                    "type": "array",
                    "items": { "type": "object" }
                }
            },
            "required": ["plans"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["plan.list".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "plan.list.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy {
            max_attempts: 3,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 100,
            max_delay_ms: 1_000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 30,
            max_input_bytes: 8 * 1024,
            max_output_bytes: 32 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_plan_list".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-agent".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![],
        tags: vec!["plan".to_string()],
    }
}

fn workspace_write_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WORKSPACE_WRITE.to_string(),
        version: "1.0.0".to_string(),
        name: "Write file".to_string(),
        description: "Create or update a file in the active project workspace.".to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "minLength": 1 },
                "content": { "type": "string" }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "bytesWritten": { "type": "integer" }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Medium,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["workspace.write".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "workspace.write.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 10_000,
        retry: ToolRetryPolicy {
            max_attempts: 1,
            strategy: ToolRetryStrategy::None,
            base_delay_ms: 0,
            max_delay_ms: 0,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 60,
            max_input_bytes: 10 * 1024 * 1024,
            max_output_bytes: 10 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_workspace_write".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec!["fs.write".to_string(), "fs.create-file".to_string()],
        tags: vec!["workspace".to_string(), "file".to_string()],
    }
}

fn workspace_read_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WORKSPACE_READ.to_string(),
        version: "1.0.0".to_string(),
        name: "Read file".to_string(),
        description: "Read the contents of a file in the active project workspace.".to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "minLength": 1 }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["workspace.read".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "workspace.read.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 10_000,
        retry: ToolRetryPolicy {
            max_attempts: 2,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 100,
            max_delay_ms: 1000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 10,
            rate_per_minute: 120,
            max_input_bytes: 5 * 1024 * 1024,
            max_output_bytes: 5 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_workspace_read".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec!["fs.read".to_string(), "fs.read-file".to_string()],
        tags: vec!["workspace".to_string(), "file".to_string()],
    }
}

fn workspace_list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WORKSPACE_LIST.to_string(),
        version: "1.0.0".to_string(),
        name: "List files".to_string(),
        description: "List files and directories in the active project workspace.".to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "files": { "type": "array" }
            },
            "required": ["files"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["workspace.list".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "workspace.list.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 10_000,
        retry: ToolRetryPolicy {
            max_attempts: 2,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 100,
            max_delay_ms: 1000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 10,
            rate_per_minute: 120,
            max_input_bytes: 5 * 1024 * 1024,
            max_output_bytes: 5 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_workspace_list".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec!["fs.list".to_string(), "fs.list-dir".to_string()],
        tags: vec!["workspace".to_string(), "file".to_string()],
    }
}

fn shell_execute_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_SHELL_EXECUTE.to_string(),
        version: "1.0.0".to_string(),
        name: "Run terminal command".to_string(),
        description: "Execute a shell command in the active project workspace terminal."
            .to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "minLength": 1 },
                "cwd": { "type": "string" }
            },
            "required": ["command"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "exitCode": { "type": "integer" },
                "stdout": { "type": "string" },
                "stderr": { "type": "string" }
            },
            "required": ["exitCode", "stdout", "stderr"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::High,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Always,
        },
        capabilities: vec!["shell.execute".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "shell.execute.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Irreversible,
        },
        timeout_ms: 30_000,
        retry: ToolRetryPolicy {
            max_attempts: 1,
            strategy: ToolRetryStrategy::None,
            base_delay_ms: 0,
            max_delay_ms: 0,
        },
        limits: ToolUsageLimits {
            max_concurrency: 3,
            rate_per_minute: 30,
            max_input_bytes: 10 * 1024 * 1024,
            max_output_bytes: 10 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_shell_execute".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![
            "terminal.run".to_string(),
            "terminal.run-command".to_string(),
        ],
        tags: vec!["terminal".to_string(), "shell".to_string()],
    }
}

fn core_code_execute_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_CODE_EXECUTE.to_string(),
        version: "1.0.0".to_string(),
        name: "Execute code".to_string(),
        description: "Execute code in Python, Node.js/JavaScript, TypeScript, PowerShell, Bash, or Windows Cmd. Returns stdout, stderr, exit code, and execution duration.".to_string(),
        category: ToolCategory::Custom,
        input_schema: json!({
            "type": "object",
            "properties": {
                "language": { "type": "string" },
                "code": { "type": "string", "minLength": 1 },
                "cwd": { "type": "string" },
                "timeout_secs": { "type": "integer", "minimum": 1, "maximum": 300 },
                "env": { "type": "object", "additionalProperties": { "type": "string" } },
                "args": { "type": "array", "items": { "type": "string" } }
            },
            "required": ["language", "code"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "exit_code": { "type": ["integer", "null"] },
                "stdout": { "type": "string" },
                "stderr": { "type": "string" },
                "duration_ms": { "type": "integer" },
                "language": { "type": "string" },
                "cwd": { "type": "string" }
            },
            "required": ["success", "stdout", "stderr", "duration_ms", "language", "cwd"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::High,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Always,
        },
        capabilities: vec!["code.execute".to_string(), "code.run".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "code.execute.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Unsupported,
            side_effects: ToolSideEffects::Irreversible,
        },
        timeout_ms: 60_000,
        retry: ToolRetryPolicy {
            max_attempts: 1,
            strategy: ToolRetryStrategy::None,
            base_delay_ms: 0,
            max_delay_ms: 0,
        },
        limits: ToolUsageLimits {
            max_concurrency: 2,
            rate_per_minute: 20,
            max_input_bytes: 10 * 1024 * 1024,
            max_output_bytes: 10 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_code_execute".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![TOOL_CODE_EXECUTE.to_string()],
        tags: vec!["code".to_string(), "execution".to_string(), "runtime".to_string()],
    }
}

fn core_document_create_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_DOCUMENT_CREATE.to_string(),
        version: "1.0.0".to_string(),
        name: "Create document".to_string(),
        description: "Generate production-grade office and data documents: Excel spreadsheets (.xlsx, .csv), Word documents (.docx), Markdown reports (.md), HTML documents (.html), and structured JSON datasets (.json).".to_string(),
        category: ToolCategory::Files,
        input_schema: json!({
            "type": "object",
            "properties": {
                "format": { "type": "string" },
                "filename": { "type": "string", "minLength": 1 },
                "path": { "type": "string" },
                "title": { "type": "string" },
                "content": { "type": "string" },
                "headers": { "type": "array", "items": { "type": "string" } },
                "rows": { "type": "array", "items": { "type": "array" } },
                "sections": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "heading": { "type": "string" },
                            "body": { "type": "string" },
                            "level": { "type": "integer" }
                        },
                        "additionalProperties": true
                    }
                },
                "data": { "type": "object" },
                "save_to_workspace": { "type": "boolean" }
            },
            "required": ["format", "filename"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "format": { "type": "string" },
                "filename": { "type": "string" },
                "path": { "type": "string" },
                "bytes_written": { "type": "integer" },
                "mime_type": { "type": "string" },
                "artifact_id": { "type": "string" },
                "preview": { "type": "string" }
            },
            "required": ["success", "format", "filename", "path", "bytes_written", "mime_type"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Medium,
            effects: vec![ToolPermissionEffect::ReversibleWrite],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["document.create".to_string(), "document.export".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "document.create.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 30_000,
        retry: ToolRetryPolicy {
            max_attempts: 2,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 200,
            max_delay_ms: 2000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 60,
            max_input_bytes: 20 * 1024 * 1024,
            max_output_bytes: 50 * 1024 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_document_create".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-tools".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![TOOL_DOCUMENT_CREATE.to_string()],
        tags: vec!["document".to_string(), "office".to_string(), "excel".to_string(), "word".to_string(), "export".to_string()],
    }
}

// ─────────────────────────────────────────────────────────────
// Workspace extended tools
// ─────────────────────────────────────────────────────────────

fn workspace_grep_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WORKSPACE_GREP.to_string(),
        version: "1.0.0".to_string(),
        name: "Search in files".to_string(),
        description: "Search for a pattern (literal or regex) in files within the workspace. Returns matched lines with file paths and line numbers.".to_string(),
        category: ToolCategory::Files,
        input_schema: json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "minLength": 1 },
                "path": { "type": "string" },
                "regex": { "type": "boolean" },
                "case_insensitive": { "type": "boolean" }
            },
            "required": ["pattern"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "matches": { "type": "array" },
                "count": { "type": "integer" }
            },
            "required": ["matches", "count"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["workspace.grep".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "workspace.grep.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 15_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 5, rate_per_minute: 60, max_input_bytes: 1024 * 1024, max_output_bytes: 5 * 1024 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_workspace_grep".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-tools".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec!["fs.grep".to_string()],
        tags: vec!["workspace".to_string(), "search".to_string()],
    }
}

fn workspace_search_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_WORKSPACE_SEARCH.to_string(),
        version: "1.0.0".to_string(),
        name: "Semantic workspace search".to_string(),
        description: "Search semantically across all files in the workspace using natural language. Returns the most relevant file excerpts.".to_string(),
        category: ToolCategory::Files,
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "minLength": 1 },
                "limit": { "type": "integer", "minimum": 1, "maximum": 20 }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "results": { "type": "array" }
            },
            "required": ["results"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["workspace.search".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "workspace.search.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 15_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 5, rate_per_minute: 60, max_input_bytes: 1024 * 1024, max_output_bytes: 5 * 1024 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_workspace_search".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-tools".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec!["fs.search".to_string()],
        tags: vec!["workspace".to_string(), "search".to_string(), "semantic".to_string()],
    }
}

// ─────────────────────────────────────────────────────────────
// Memory tools — read/write long-term agent memory
// ─────────────────────────────────────────────────────────────

fn memory_save_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_MEMORY_SAVE.to_string(),
        version: "1.0.0".to_string(),
        name: "Save to memory".to_string(),
        description: "Persist a piece of information to long-term memory so it can be recalled in future conversations. Use for user preferences, facts, decisions, and important context.".to_string(),
        category: ToolCategory::Memory,
        input_schema: json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "minLength": 1 },
                "category": { "type": "string", "enum": ["personal", "technical", "system", "preference"] },
                "scope": { "type": "string", "enum": ["user", "conversation", "project"] },
                "pinned": { "type": "boolean" },
                "salience": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
            },
            "required": ["content"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "id": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["success", "id", "content"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["memory.write".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "memory.save.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 5, rate_per_minute: 60, max_input_bytes: 64 * 1024, max_output_bytes: 64 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_memory_save".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec![TOOL_MEMORY_SAVE.to_string(), "memory.save".to_string(), "memory.add".to_string()],
        tags: vec!["memory".to_string(), "persistence".to_string()],
    }
}

fn memory_search_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_MEMORY_SEARCH.to_string(),
        version: "1.0.0".to_string(),
        name: "Search memory".to_string(),
        description: "Search long-term memory using a natural language query. Returns the most relevant stored memories using semantic and lexical search.".to_string(),
        category: ToolCategory::Memory,
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "minLength": 1 },
                "limit": { "type": "integer", "minimum": 1, "maximum": 50 },
                "category": { "type": "string" },
                "scope": { "type": "string" }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "memories": { "type": "array" },
                "count": { "type": "integer" }
            },
            "required": ["memories", "count"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["memory.read".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "memory.search.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 10_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 10, rate_per_minute: 120, max_input_bytes: 8 * 1024, max_output_bytes: 512 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_memory_search".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec![TOOL_MEMORY_SEARCH.to_string(), "memory.search".to_string()],
        tags: vec!["memory".to_string(), "search".to_string()],
    }
}

fn memory_recall_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_MEMORY_RECALL.to_string(),
        version: "1.0.0".to_string(),
        name: "Recall memory".to_string(),
        description: "Recall a specific memory by ID or search by entity key, optionally including related episodic memory summaries.".to_string(),
        category: ToolCategory::Memory,
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$" },
                "entityKey": { "type": "string" },
                "entity_key": { "type": "string" },
                "includeEpisodes": { "type": "boolean" },
                "include_episodes": { "type": "boolean" }
            },
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "found": { "type": "boolean" },
                "memory": { "type": ["object", "null"] },
                "relatedEpisodes": { "type": ["array", "null"] }
            },
            "required": ["found"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["memory.read".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "memory.recall.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 10, rate_per_minute: 120, max_input_bytes: 8 * 1024, max_output_bytes: 512 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_memory_recall".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec![TOOL_MEMORY_RECALL.to_string(), "memory.recall".to_string()],
        tags: vec!["memory".to_string(), "recall".to_string()],
    }
}

fn memory_update_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_MEMORY_UPDATE.to_string(),
        version: "1.0.0".to_string(),
        name: "Update memory".to_string(),
        description: "Update an existing long-term memory's content, salience, pinned status, or category.".to_string(),
        category: ToolCategory::Memory,
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$" },
                "content": { "type": "string", "minLength": 1 },
                "salience": { "type": "number", "minimum": 0.0, "maximum": 1.0 },
                "pinned": { "type": "boolean" },
                "category": { "type": "string" }
            },
            "required": ["id"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "id": { "type": "string" },
                "updatedAt": { "type": "string" }
            },
            "required": ["success", "id"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["memory.write".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "memory.update.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 5, rate_per_minute: 60, max_input_bytes: 64 * 1024, max_output_bytes: 64 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_memory_update".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec![TOOL_MEMORY_UPDATE.to_string(), "memory.update".to_string()],
        tags: vec!["memory".to_string(), "update".to_string()],
    }
}

fn memory_forget_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_MEMORY_FORGET.to_string(),
        version: "1.0.0".to_string(),
        name: "Forget memory".to_string(),
        description: "Archive / forget a specific long-term memory so it is no longer recalled.".to_string(),
        category: ToolCategory::Memory,
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$" },
                "reason": { "type": "string" }
            },
            "required": ["id"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "id": { "type": "string" },
                "status": { "type": "string" }
            },
            "required": ["success", "id"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Medium, effects: vec![], confirmation: ToolConfirmationPolicy::Policy },
        capabilities: vec!["memory.write".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "memory.forget.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy { max_attempts: 1, strategy: ToolRetryStrategy::None, base_delay_ms: 0, max_delay_ms: 0 },
        limits: ToolUsageLimits { max_concurrency: 5, rate_per_minute: 30, max_input_bytes: 2 * 1024, max_output_bytes: 2 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_memory_forget".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec![TOOL_MEMORY_FORGET.to_string(), "memory.forget".to_string()],
        tags: vec!["memory".to_string(), "forget".to_string()],
    }
}

fn memory_list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_MEMORY_LIST.to_string(),
        version: "1.0.0".to_string(),
        name: "List memories".to_string(),
        description:
            "List all stored long-term memories, optionally filtered by category or scope."
                .to_string(),
        category: ToolCategory::Memory,
        input_schema: json!({
            "type": "object",
            "properties": {
                "category": { "type": "string" },
                "scope": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 50 }
            },
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "memories": { "type": "array" },
                "count": { "type": "integer" }
            },
            "required": ["memories", "count"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Low,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Never,
        },
        capabilities: vec!["memory.read".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "memory.list.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy {
            max_attempts: 2,
            strategy: ToolRetryStrategy::ExponentialJitter,
            base_delay_ms: 100,
            max_delay_ms: 1000,
        },
        limits: ToolUsageLimits {
            max_concurrency: 10,
            rate_per_minute: 120,
            max_input_bytes: 2 * 1024,
            max_output_bytes: 512 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_memory_list".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-runtime".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![TOOL_MEMORY_LIST.to_string(), "memory.list".to_string()],
        tags: vec!["memory".to_string()],
    }
}

fn memory_delete_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_MEMORY_DELETE.to_string(),
        version: "1.0.0".to_string(),
        name: "Delete memory".to_string(),
        description: "Delete a specific memory by its ID from long-term memory storage."
            .to_string(),
        category: ToolCategory::Memory,
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$" }
            },
            "required": ["id"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": { "success": { "type": "boolean" } },
            "required": ["success"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk {
            level: ToolRiskLevel::Medium,
            effects: vec![],
            confirmation: ToolConfirmationPolicy::Policy,
        },
        capabilities: vec!["memory.write".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "memory.delete.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Irreversible,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy {
            max_attempts: 1,
            strategy: ToolRetryStrategy::None,
            base_delay_ms: 0,
            max_delay_ms: 0,
        },
        limits: ToolUsageLimits {
            max_concurrency: 5,
            rate_per_minute: 30,
            max_input_bytes: 2 * 1024,
            max_output_bytes: 2 * 1024,
        },
        dependencies: vec![],
        observability: ToolObservability {
            record_input: ToolDataCaptureMode::None,
            record_output: ToolDataCaptureMode::None,
            metrics_namespace: "aro_tool_memory_delete".to_string(),
            cost_unit: None,
        },
        status: ToolStatus::Active,
        provenance: ToolProvenance {
            kind: ToolProvenanceKind::Core,
            package: "aro-runtime".to_string(),
            signature: None,
        },
        owner: ToolOwner {
            kind: ToolOwnerKind::Team,
            id: "platform-agent".to_string(),
        },
        aliases: vec![TOOL_MEMORY_DELETE.to_string(), "memory.delete".to_string()],
        tags: vec!["memory".to_string()],
    }
}

// ─────────────────────────────────────────────────────────────
// Context tools
// ─────────────────────────────────────────────────────────────

fn context_search_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_CONTEXT_SEARCH.to_string(),
        version: "1.0.0".to_string(),
        name: "Search agent context".to_string(),
        description: "Search through the agent's past steps, tool results, and context items using a keyword query. Useful for reviewing what was already done in previous steps.".to_string(),
        category: ToolCategory::Memory,
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "minLength": 1 },
                "limit": { "type": "integer", "minimum": 1, "maximum": 20 }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "items": { "type": "array" },
                "count": { "type": "integer" }
            },
            "required": ["items", "count"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["context.read".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "context.search.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 10, rate_per_minute: 120, max_input_bytes: 8 * 1024, max_output_bytes: 512 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_context_search".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec!["ctx.search".to_string()],
        tags: vec!["context".to_string(), "search".to_string()],
    }
}

// ─────────────────────────────────────────────────────────────
// Orchestration / agent delegation tools
// ─────────────────────────────────────────────────────────────

fn agent_delegate_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_AGENT_DELEGATE.to_string(),
        version: "1.0.0".to_string(),
        name: "Delegate to agent".to_string(),
        description: "Delegate a sub-task to a new autonomous agent run. The current agent provides a goal and the sub-agent executes it independently. Returns the sub-agent's run ID and initial status. Use when a task can be parallelized or when specialized processing is needed.".to_string(),
        category: ToolCategory::Automation,
        input_schema: json!({
            "type": "object",
            "properties": {
                "goal": { "type": "string", "minLength": 1 },
                "mode": { "type": "string", "enum": ["chat", "code", "architect", "research"] },
                "max_steps": { "type": "integer", "minimum": 1 },
                "system_prompt": { "type": "string" }
            },
            "required": ["goal"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "run_id": { "type": "string" },
                "status": { "type": "string" },
                "goal": { "type": "string" }
            },
            "required": ["run_id", "status", "goal"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Medium, effects: vec![], confirmation: ToolConfirmationPolicy::Policy },
        capabilities: vec!["agent.delegate".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "agent.delegate.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 30_000,
        retry: ToolRetryPolicy { max_attempts: 1, strategy: ToolRetryStrategy::None, base_delay_ms: 0, max_delay_ms: 0 },
        limits: ToolUsageLimits { max_concurrency: 3, rate_per_minute: 10, max_input_bytes: 32 * 1024, max_output_bytes: 32 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::MetadataOnly, record_output: ToolDataCaptureMode::MetadataOnly, metrics_namespace: "aro_tool_agent_delegate".to_string(), cost_unit: Some("agent-run".to_string()) },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec!["agent.spawn".to_string()],
        tags: vec!["orchestration".to_string(), "agent".to_string(), "delegation".to_string()],
    }
}

fn agent_spawn_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_AGENT_SPAWN.to_string(),
        version: "1.0.0".to_string(),
        name: "Spawn agent run".to_string(),
        description: "Start a new agent run with a specific goal in the current conversation. The agent runs autonomously in the background. Returns immediately with the run ID.".to_string(),
        category: ToolCategory::Automation,
        input_schema: json!({
            "type": "object",
            "properties": {
                "goal": { "type": "string", "minLength": 1 },
                "mode": { "type": "string", "enum": ["chat", "code", "architect", "research"] },
                "max_steps": { "type": "integer", "minimum": 1 }
            },
            "required": ["goal"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "run_id": { "type": "string" },
                "status": { "type": "string" }
            },
            "required": ["run_id", "status"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Medium, effects: vec![], confirmation: ToolConfirmationPolicy::Policy },
        capabilities: vec!["agent.spawn".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "agent.spawn.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 10_000,
        retry: ToolRetryPolicy { max_attempts: 1, strategy: ToolRetryStrategy::None, base_delay_ms: 0, max_delay_ms: 0 },
        limits: ToolUsageLimits { max_concurrency: 3, rate_per_minute: 10, max_input_bytes: 32 * 1024, max_output_bytes: 32 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::MetadataOnly, record_output: ToolDataCaptureMode::MetadataOnly, metrics_namespace: "aro_tool_agent_spawn".to_string(), cost_unit: Some("agent-run".to_string()) },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec![],
        tags: vec!["orchestration".to_string(), "agent".to_string()],
    }
}

fn agent_status_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_AGENT_STATUS.to_string(),
        version: "1.0.0".to_string(),
        name: "Get agent status".to_string(),
        description: "Check the current status and latest steps of an agent run by its run ID. Use after delegating to monitor progress.".to_string(),
        category: ToolCategory::Automation,
        input_schema: json!({
            "type": "object",
            "properties": {
                "run_id": { "type": "string", "pattern": "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$" }
            },
            "required": ["run_id"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "run_id": { "type": "string" },
                "status": { "type": "string" },
                "goal": { "type": "string" },
                "steps_count": { "type": "integer" },
                "last_step": { "type": ["object", "null"] }
            },
            "required": ["run_id", "status"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["agent.read".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "agent.status.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 10, rate_per_minute: 120, max_input_bytes: 2 * 1024, max_output_bytes: 64 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_agent_status".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec!["agent.check".to_string()],
        tags: vec!["orchestration".to_string(), "agent".to_string()],
    }
}

// ─────────────────────────────────────────────────────────────
// MCP (Model Context Protocol) bridge tool
// ─────────────────────────────────────────────────────────────

fn mcp_call_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_MCP_CALL.to_string(),
        version: "1.0.0".to_string(),
        name: "Call MCP tool".to_string(),
        description: "Call an external tool registered via the Model Context Protocol (MCP). Specify the server name, tool name, and arguments. MCP allows connecting to external tool servers (Brave Search, GitHub, Postgres, etc.).".to_string(),
        category: ToolCategory::Integration,
        input_schema: json!({
            "type": "object",
            "properties": {
                "server": { "type": "string", "minLength": 1 },
                "tool": { "type": "string", "minLength": 1 },
                "arguments": { "type": "object" }
            },
            "required": ["server", "tool"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "result": { "type": ["object", "string", "null"] },
                "error": { "type": ["string", "null"] }
            },
            "required": ["success"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Medium, effects: vec![ToolPermissionEffect::ExternalRead], confirmation: ToolConfirmationPolicy::Policy },
        capabilities: vec!["mcp.call".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Mcp,
            handler: "mcp.call.v1".to_string(),
            environment: ToolExecutionEnvironment::Remote,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 30_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 250, max_delay_ms: 2000 },
        limits: ToolUsageLimits { max_concurrency: 5, rate_per_minute: 60, max_input_bytes: 64 * 1024, max_output_bytes: 1024 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::MetadataOnly, record_output: ToolDataCaptureMode::ReferenceOnly, metrics_namespace: "aro_tool_mcp_call".to_string(), cost_unit: Some("mcp-call".to_string()) },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Mcp, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec!["mcp.invoke".to_string()],
        tags: vec!["mcp".to_string(), "integration".to_string(), "external".to_string()],
    }
}

// ─────────────────────────────────────────────────────────────
// Skill tools
// ─────────────────────────────────────────────────────────────

fn skill_list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_SKILL_LIST.to_string(),
        version: "1.0.0".to_string(),
        name: "List skills".to_string(),
        description: "List all available skills registered in the agent's skill library. Skills are reusable capability modules that extend agent behavior for specific domains.".to_string(),
        category: ToolCategory::Automation,
        input_schema: json!({
            "type": "object",
            "properties": {
                "category": { "type": "string" }
            },
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "skills": { "type": "array" },
                "count": { "type": "integer" }
            },
            "required": ["skills", "count"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["skill.read".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "skill.list.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 10, rate_per_minute: 120, max_input_bytes: 2 * 1024, max_output_bytes: 128 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_skill_list".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Core, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec![],
        tags: vec!["skill".to_string()],
    }
}

fn skill_invoke_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_SKILL_INVOKE.to_string(),
        version: "1.0.0".to_string(),
        name: "Invoke skill".to_string(),
        description: "Invoke a specific skill by its ID with the provided arguments. Skills are specialized capability modules (e.g., code review, data analysis, summarization).".to_string(),
        category: ToolCategory::Automation,
        input_schema: json!({
            "type": "object",
            "properties": {
                "skill_id": { "type": "string", "minLength": 1 },
                "arguments": { "type": "object" }
            },
            "required": ["skill_id"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "result": { "type": ["object", "string", "null"] },
                "error": { "type": ["string", "null"] }
            },
            "required": ["success"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Medium, effects: vec![], confirmation: ToolConfirmationPolicy::Policy },
        capabilities: vec!["skill.invoke".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "skill.invoke.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 60_000,
        retry: ToolRetryPolicy { max_attempts: 1, strategy: ToolRetryStrategy::None, base_delay_ms: 0, max_delay_ms: 0 },
        limits: ToolUsageLimits { max_concurrency: 5, rate_per_minute: 30, max_input_bytes: 64 * 1024, max_output_bytes: 1024 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::MetadataOnly, record_output: ToolDataCaptureMode::MetadataOnly, metrics_namespace: "aro_tool_skill_invoke".to_string(), cost_unit: Some("skill-call".to_string()) },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Skill, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec![],
        tags: vec!["skill".to_string(), "invoke".to_string()],
    }
}

// ─────────────────────────────────────────────────────────────
// Connector/plugin tools
// ─────────────────────────────────────────────────────────────

fn connector_list_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_CONNECTOR_LIST.to_string(),
        version: "1.0.0".to_string(),
        name: "List connectors".to_string(),
        description: "List all connected third-party service integrations (plugins/connectors) that the agent can use, such as GitHub, Slack, Google Drive, databases, and APIs.".to_string(),
        category: ToolCategory::Integration,
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "connectors": { "type": "array" },
                "count": { "type": "integer" }
            },
            "required": ["connectors", "count"],
            "additionalProperties": false
        }),
        permissions: vec![],
        risk: ToolRisk { level: ToolRiskLevel::Low, effects: vec![], confirmation: ToolConfirmationPolicy::Never },
        capabilities: vec!["connector.read".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "connector.list.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::ReadOnly,
        },
        timeout_ms: 5_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 100, max_delay_ms: 1000 },
        limits: ToolUsageLimits { max_concurrency: 10, rate_per_minute: 120, max_input_bytes: 2 * 1024, max_output_bytes: 128 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::None, record_output: ToolDataCaptureMode::None, metrics_namespace: "aro_tool_connector_list".to_string(), cost_unit: None },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Plugin, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec!["plugin.list".to_string()],
        tags: vec!["connector".to_string(), "integration".to_string()],
    }
}

fn connector_call_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        schema_version: TOOL_DESCRIPTOR_SCHEMA_VERSION,
        id: TOOL_CORE_CONNECTOR_CALL.to_string(),
        version: "1.0.0".to_string(),
        name: "Call connector".to_string(),
        description: "Call a specific action on a connected third-party integration (GitHub, Slack, Google Drive, databases, etc.). Use core.connector.list first to see available connectors and their capabilities.".to_string(),
        category: ToolCategory::Integration,
        input_schema: json!({
            "type": "object",
            "properties": {
                "connector_id": { "type": "string", "minLength": 1 },
                "account": { "type": "string" },
                "action": { "type": "string", "minLength": 1 },
                "arguments": { "type": "object" }
            },
            "required": ["connector_id", "action"],
            "additionalProperties": false
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean" },
                "result": {},
                "error": { "type": ["string", "null"] }
            },
            "required": ["success"],
            "additionalProperties": false
        }),
        permissions: vec![ToolPermissionRequirement { action: "connector.call".to_string(), resource: "connector:${input.connector_id}".to_string() }],
        risk: ToolRisk { level: ToolRiskLevel::Medium, effects: vec![ToolPermissionEffect::ExternalWrite], confirmation: ToolConfirmationPolicy::Policy },
        capabilities: vec!["connector.call".to_string()],
        execution: ToolExecutionSpec {
            kind: ToolExecutionKind::Backend,
            handler: "connector.call.v1".to_string(),
            environment: ToolExecutionEnvironment::CloudWorker,
            streaming: false,
            idempotency: ToolIdempotency::Recommended,
            side_effects: ToolSideEffects::Reversible,
        },
        timeout_ms: 30_000,
        retry: ToolRetryPolicy { max_attempts: 2, strategy: ToolRetryStrategy::ExponentialJitter, base_delay_ms: 250, max_delay_ms: 2000 },
        limits: ToolUsageLimits { max_concurrency: 5, rate_per_minute: 30, max_input_bytes: 128 * 1024, max_output_bytes: 2 * 1024 * 1024 },
        dependencies: vec![],
        observability: ToolObservability { record_input: ToolDataCaptureMode::MetadataOnly, record_output: ToolDataCaptureMode::ReferenceOnly, metrics_namespace: "aro_tool_connector_call".to_string(), cost_unit: Some("connector-call".to_string()) },
        status: ToolStatus::Active,
        provenance: ToolProvenance { kind: ToolProvenanceKind::Plugin, package: "aro-runtime".to_string(), signature: None },
        owner: ToolOwner { kind: ToolOwnerKind::Team, id: "platform-agent".to_string() },
        aliases: vec!["plugin.call".to_string(), "integration.call".to_string()],
        tags: vec!["connector".to_string(), "integration".to_string(), "external".to_string()],
    }
}

pub(crate) fn compact_excerpt(content: &str, max_chars: usize) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        normalized.chars().take(max_chars).collect::<String>()
    }
}

fn estimate_tokens(text: &str) -> u32 {
    ((text.chars().count() as f32) / 4.0).ceil() as u32
}

fn render_context_pack(pack: &ContextPack) -> String {
    let sources = pack
        .sources
        .iter()
        .map(|source| {
            let uri = source
                .uri
                .as_deref()
                .map(|uri| format!(" <{uri}>"))
                .unwrap_or_default();
            format!(
                "- [{}] ({}) {}{}: {}",
                source.id, source.kind, source.title, uri, source.excerpt
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let tools = pack
        .tools
        .iter()
        .map(|tool| {
            format!(
                "- {} ({:?}): {} Input schema: {}",
                tool.id, tool.source, tool.description, tool.input_schema
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}\n\nSources:\n{}\n\nAvailable tools:\n{}",
        pack.summary, sources, tools
    )
}

pub(crate) fn render_environment_snapshot(environment: &EnvironmentSnapshot) -> String {
    let mut parts = vec![
        format!("Runtime: {}", environment.runtime),
        format!("Platform: {}", environment.platform),
        format!("Timestamp UTC: {}", environment.timestamp_utc.to_rfc3339()),
    ];
    if let Some(provider_id) = environment.provider_id.as_deref() {
        parts.push(format!("Provider: {}", compact_excerpt(provider_id, 120)));
    }
    if let Some(model_id) = environment.model_id.as_deref() {
        parts.push(format!("Model: {}", compact_excerpt(model_id, 120)));
    }
    if let Some(root) = environment.workspace_root.as_deref() {
        parts.push(format!(
            "Trusted workspace root: {}",
            compact_excerpt(root, 240)
        ));
    }
    parts.join(". ")
}

fn parse_agent_action(output: &str) -> Option<AgentAction> {
    let candidate = json_candidate(output)?;
    let mut value: Value = serde_json::from_str(candidate).ok()?;
    if value.get("type").is_none() {
        if let Some(action_type) = value.get("action").cloned() {
            value
                .as_object_mut()?
                .insert("type".to_string(), action_type);
        }
    }
    serde_json::from_value(value).ok()
}

fn json_candidate(output: &str) -> Option<&str> {
    let trimmed = output.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed);
    }
    let fenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))?
        .trim();
    fenced.strip_suffix("```").map(str::trim)
}

fn normalize_path(path: &Path) -> std::io::Result<PathBuf> {
    if path.exists() {
        path.canonicalize()
    } else if let Some(parent) = path.parent() {
        Ok(parent
            .canonicalize()?
            .join(path.file_name().unwrap_or_default()))
    } else {
        Ok(path.to_path_buf())
    }
}

fn looks_like_secret_export(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    (lower.contains("env") || lower.contains("set") || lower.contains("print"))
        && [
            "secret",
            "token",
            "api_key",
            "apikey",
            "password",
            "credential",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn looks_destructive_outside_scope(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    lower.contains("format ")
        || lower.contains("diskpart")
        || lower.contains("remove-item -recurse c:\\")
        || lower.contains("rm -rf /")
}

#[cfg(test)]
mod tests {
    use super::*;
    use aro_core::AssistantMode;

    #[test]
    fn context_pack_contains_goal_recent_messages_and_tools() {
        let runtime = AgentRuntime::new();
        let request = AgentRunStartRequest {
            run_id: None,
            lane_id: None,
            conversation_id: None,
            goal: "Build durable agent runtime".to_string(),
            mode: AssistantMode::Code,
            system_prompt: None,
            model_id: Some("mock".to_string()),
            provider: Some("mock-local".to_string()),
            autonomy_profile_id: None,
            priority: None,
            max_steps: Some(1),
        };
        let run = runtime.start_run(&request, None);
        let context =
            runtime.build_context_pack(&run, &[], &[], &[], EnvironmentSnapshot::default());

        assert_eq!(context.run_id, run.id);
        assert!(context.summary.contains("Build durable"));
        assert!(context
            .tools
            .iter()
            .any(|tool| tool.id == TOOL_CORE_SEARCH_WEB));
        assert!(!context.tools.iter().any(|tool| tool.id == "workspace.read"));
        assert!(context
            .sources
            .iter()
            .any(|source| source.kind == "environment"));
    }

    #[test]
    fn start_run_freezes_the_validated_permission_profile() {
        let runtime = AgentRuntime::new();
        let profile_id = Uuid::new_v4();
        let request = AgentRunStartRequest {
            run_id: None,
            lane_id: None,
            conversation_id: None,
            goal: "Use only explicitly granted capabilities".to_string(),
            mode: AssistantMode::Code,
            system_prompt: None,
            model_id: Some("mock".to_string()),
            provider: Some("mock-local".to_string()),
            autonomy_profile_id: Some(profile_id),
            priority: None,
            max_steps: Some(1),
        };

        let run = runtime.start_run(&request, request.autonomy_profile_id);

        assert_eq!(run.autonomy_profile_id, Some(profile_id));
    }

    #[test]
    fn permission_policy_blocks_untrusted_writes() {
        let root = std::env::current_dir().expect("cwd");
        let profile = PermissionProfile::trusted_workspace(root.display().to_string());
        let policy = PermissionPolicy::new(profile);

        assert!(policy.can_write_path(root.join("agent-test.txt")));
        assert!(!policy.can_write_path(Path::new("C:\\Windows\\system32\\drivers\\etc\\hosts")));
    }

    #[test]
    fn permission_policy_blocks_secret_export_commands() {
        let root = std::env::current_dir().expect("cwd");
        let profile = PermissionProfile::trusted_workspace(root.display().to_string());
        let policy = PermissionPolicy::new(profile);

        assert!(!policy.can_run_command(&root, "Get-ChildItem Env: | Select-String SECRET"));
    }

    #[test]
    fn parses_final_json_action() {
        let runtime = AgentRuntime::new();
        let action =
            runtime.parse_model_action(r#"{"type":"final","content":"Done with source [run:1]."}"#);

        assert_eq!(action.action_type, AgentActionType::Final);
        assert_eq!(action.content.as_deref(), Some("Done with source [run:1]."));
    }

    #[test]
    fn falls_back_to_plain_text_final_action() {
        let runtime = AgentRuntime::new();
        let action = runtime.parse_model_action("Plain assistant answer");

        assert_eq!(action.action_type, AgentActionType::Final);
        assert_eq!(action.content.as_deref(), Some("Plain assistant answer"));
    }

    #[test]
    fn validates_tool_action_against_context_pack() {
        let runtime = AgentRuntime::new();
        let request = AgentRunStartRequest {
            run_id: None,
            lane_id: None,
            conversation_id: None,
            goal: "Search current information".to_string(),
            mode: AssistantMode::Code,
            system_prompt: None,
            model_id: Some("mock".to_string()),
            provider: Some("mock-local".to_string()),
            autonomy_profile_id: None,
            priority: None,
            max_steps: Some(1),
        };
        let run = runtime.start_run(&request, None);
        let context =
            runtime.build_context_pack(&run, &[], &[], &[], EnvironmentSnapshot::default());

        let allowed = AgentAction::tool(
            TOOL_CORE_SEARCH_WEB,
            json!({ "query": "ARO architecture" }),
            Some("Need source".to_string()),
        );
        let blocked = AgentAction::tool("browser.open", json!({}), None);

        assert!(runtime.validate_action(&allowed, &context).is_ok());
        assert!(runtime.validate_action(&blocked, &context).is_err());
    }

    #[test]
    fn direct_model_request_does_not_require_json() {
        let runtime = AgentRuntime::new();
        let request = AgentRunStartRequest {
            run_id: None,
            lane_id: None,
            conversation_id: None,
            goal: "Answer normally".to_string(),
            mode: AssistantMode::Chat,
            system_prompt: None,
            model_id: Some("mock".to_string()),
            provider: Some("mock-local".to_string()),
            autonomy_profile_id: None,
            priority: None,
            max_steps: Some(1),
        };
        let run = runtime.start_run(&request, None);
        let context = runtime.build_context_pack(
            &run,
            &[],
            &[],
            &[],
            EnvironmentSnapshot::desktop_local(
                Some("mock-local".to_string()),
                Some("mock".to_string()),
            ),
        );
        let model_request = runtime.model_request(
            &run,
            &context,
            "You are ARO.".to_string(),
            Vec::new(),
            "hello".to_string(),
            0.2,
            256,
            ModelResponseFormat::DirectText,
        );

        assert!(model_request
            .system_prompt
            .contains("Answer the user directly"));
        assert!(!model_request
            .system_prompt
            .contains("Return exactly one JSON object"));
    }

    #[test]
    fn agent_model_request_keeps_json_contract() {
        let runtime = AgentRuntime::new();
        let request = AgentRunStartRequest {
            run_id: None,
            lane_id: None,
            conversation_id: None,
            goal: "Use tools if needed".to_string(),
            mode: AssistantMode::Code,
            system_prompt: None,
            model_id: Some("mock".to_string()),
            provider: Some("mock-local".to_string()),
            autonomy_profile_id: None,
            priority: None,
            max_steps: Some(4),
        };
        let run = runtime.start_run(&request, None);
        let context =
            runtime.build_context_pack(&run, &[], &[], &[], EnvironmentSnapshot::default());
        let model_request = runtime.model_request(
            &run,
            &context,
            "You are ARO.".to_string(),
            Vec::new(),
            "research current sources".to_string(),
            0.2,
            256,
            ModelResponseFormat::AgentActionJson,
        );

        assert!(model_request
            .system_prompt
            .contains("Return exactly one JSON object"));
        assert!(model_request.system_prompt.contains(TOOL_CORE_SEARCH_WEB));
        assert!(model_request
            .system_prompt
            .contains(TOOL_CORE_WORKSPACE_READ));
    }

    #[test]
    fn built_in_registry_exposes_only_valid_executable_descriptors() {
        let registry = ToolRegistry::default();
        assert_eq!(registry.descriptors().len(), 30);
        assert!(registry
            .descriptors()
            .iter()
            .all(|descriptor| descriptor.validate().is_ok()));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_WEB_PAGE_READ));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_CODE_EXECUTE));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_DOCUMENT_CREATE));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_MEMORY_RECALL));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_MEMORY_UPDATE));
        assert!(registry
            .descriptors()
            .iter()
            .any(|descriptor| descriptor.id == TOOL_CORE_MEMORY_FORGET));
        assert!(!registry.enabled_tools().is_empty());
    }

    #[test]
    fn environment_context_contains_runtime_without_secret_words() {
        let environment = EnvironmentSnapshot {
            workspace_root: Some("C:\\Users\\Stagiaire\\Documents\\ARO".to_string()),
            provider_id: Some("ollama-local".to_string()),
            model_id: Some("gemma3:1b".to_string()),
            ..EnvironmentSnapshot::default()
        };
        let rendered = render_environment_snapshot(&environment);

        assert!(rendered.contains("desktop-local"));
        assert!(rendered.contains("gemma3:1b"));
        assert!(!rendered.to_ascii_lowercase().contains("api_key"));
        assert!(!rendered.to_ascii_lowercase().contains("password"));
    }

    #[test]
    fn orchestrator_queues_when_lane_is_at_capacity() {
        let runtime = AgentRuntime::new();
        let lane = AgentLane::new(None, "Coding lane");
        let request = AgentRunStartRequest {
            run_id: None,
            lane_id: Some(lane.id),
            conversation_id: None,
            goal: "Do work".to_string(),
            mode: AssistantMode::Code,
            system_prompt: None,
            model_id: Some("mock".to_string()),
            provider: Some("mock-local".to_string()),
            autonomy_profile_id: None,
            priority: None,
            max_steps: Some(8),
        };
        let mut run = runtime.start_run(&request, None);

        runtime.schedule_run(&mut run, &lane, 0, lane.max_concurrent_runs);

        assert_eq!(run.lane_id, Some(lane.id));
        assert_eq!(run.status, AgentRunStatus::Queued);
    }
}
