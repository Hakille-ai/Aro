//! Durable, fenced persistence for server-side agent execution.
//!
//! The queue tables are intentionally separate from the legacy `agent_runs` projection.  Every
//! worker mutation is fenced by both a random lease token and a monotonically increasing lease
//! generation, while API reads are scoped through [`TenantContext`].  Request snapshots are only
//! exposed by [`ClaimedAgentRunJob`], which deliberately implements neither `Debug` nor
//! `Serialize` so prompts and credentials cannot be logged accidentally.

use std::collections::BTreeMap;

use aro_core::{AgentRun, AgentRunPriority, AroError, AroResult, MessageRole};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{enum_to_string, map_sqlx, AroStore, TenantContext};

pub const AGENT_JOB_MIN_LEASE_SECONDS: i64 = 5;
pub const AGENT_JOB_MAX_LEASE_SECONDS: i64 = 15 * 60;
pub const AGENT_JOB_MAX_EVENT_PAGE_SIZE: i64 = 500;
pub const AGENT_JOB_MAX_SUBMISSION_KEY_BYTES: usize = 255;
pub const AGENT_JOB_MAX_WORKER_ID_BYTES: usize = 200;
pub const AGENT_JOB_MAX_REASON_BYTES: usize = 2_048;
pub const AGENT_JOB_MAX_ERROR_CODE_BYTES: usize = 256;
pub const AGENT_JOB_MAX_NON_TERMINAL_PER_USER: i64 = 200;
pub const AGENT_JOB_MAX_NON_TERMINAL_PER_ORGANIZATION: i64 = 2_000;

/// Versioned keyring for encrypted execution snapshots. The current key encrypts new jobs while
/// previous keys keep backlog readable during an online key rotation.
pub struct AgentSnapshotKeyring {
    current_key_id: String,
    keys: BTreeMap<String, Zeroizing<String>>,
}

impl AgentSnapshotKeyring {
    pub fn from_entries(
        current_key_id: impl Into<String>,
        entries: impl IntoIterator<Item = (String, String)>,
    ) -> AroResult<Self> {
        let current_key_id = current_key_id.into();
        validate_safe_identifier(&current_key_id, "agent snapshot key id", 128)?;
        let mut keys = BTreeMap::new();
        for (key_id, key) in entries {
            validate_safe_identifier(&key_id, "agent snapshot key id", 128)?;
            validate_agent_snapshot_key(&key)?;
            if keys.insert(key_id.clone(), Zeroizing::new(key)).is_some() {
                return Err(AroError::Configuration(format!(
                    "duplicate agent snapshot key id: {key_id}"
                )));
            }
        }
        if !keys.contains_key(&current_key_id) {
            return Err(AroError::Configuration(
                "agent snapshot keyring does not contain its current key id".to_string(),
            ));
        }
        Ok(Self {
            current_key_id,
            keys,
        })
    }

    pub fn single(key_id: impl Into<String>, key: impl Into<String>) -> AroResult<Self> {
        let key_id = key_id.into();
        Self::from_entries(key_id.clone(), [(key_id, key.into())])
    }

    fn current(&self) -> AroResult<(&str, &str)> {
        let key = self.keys.get(&self.current_key_id).ok_or_else(|| {
            AroError::Configuration(
                "agent snapshot keyring lost its current key unexpectedly".to_string(),
            )
        })?;
        Ok((&self.current_key_id, key.as_str()))
    }

    fn key(&self, key_id: &str) -> Option<&str> {
        self.keys.get(key_id).map(|key| key.as_str())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentRunJobStatus {
    Queued,
    Leased,
    Running,
    Waiting,
    RetryWait,
    Completed,
    Failed,
    Cancelled,
}

impl AgentRunJobStatus {
    fn from_database(value: &str) -> AroResult<Self> {
        match value {
            "queued" => Ok(Self::Queued),
            "leased" => Ok(Self::Leased),
            "running" => Ok(Self::Running),
            "waiting" => Ok(Self::Waiting),
            "retry_wait" => Ok(Self::RetryWait),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(AroError::Memory(format!(
                "unknown durable agent job status: {other}"
            ))),
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunJobBudgets {
    pub max_attempts: u16,
    pub max_steps: u16,
    pub max_tool_calls: u16,
    pub max_input_tokens: u32,
    pub max_output_tokens: u32,
    pub max_wall_time_seconds: u32,
}

impl Default for AgentRunJobBudgets {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            // Effectively unlimited (Postgres SMALLINT max): agents run until
            // final/pause, not a step cap.
            max_steps: i16::MAX as u16,
            max_tool_calls: i16::MAX as u16,
            max_input_tokens: 131_072,
            max_output_tokens: 8_192,
            max_wall_time_seconds: 15 * 60,
        }
    }
}

impl AgentRunJobBudgets {
    fn validate(self) -> AroResult<Self> {
        // Bornes alignées sur le CHECK SQL (migrations agent_execution_queue
        // + agent_unlimited_steps) : un rejet doit être explicite ici,
        // pas une erreur brute de contrainte DB.
        if !(1..=20).contains(&self.max_attempts)
            || !(1..=32_767).contains(&self.max_steps)
            || self.max_tool_calls > 32_767
            || self.max_input_tokens == 0
            || self.max_input_tokens > 1_048_576
            || self.max_output_tokens == 0
            || self.max_output_tokens > 131_072
            || !(1..=86_400).contains(&self.max_wall_time_seconds)
        {
            return Err(AroError::Configuration(
                "durable agent job budgets are outside their supported bounds".to_string(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunJobUsage {
    pub steps: u16,
    pub tool_calls: u16,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunJobView {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub agent_run_id: Uuid,
    pub status: AgentRunJobStatus,
    pub priority: AgentRunPriority,
    pub budgets: AgentRunJobBudgets,
    pub attempts: u16,
    pub usage: AgentRunJobUsage,
    pub available_at: DateTime<Utc>,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub pause_requested_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub state_reason: Option<String>,
    pub last_error: Option<String>,
    pub result_message_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRunJobEvent {
    pub cursor: i64,
    pub event_id: Uuid,
    pub agent_run_id: Uuid,
    pub job_id: Option<Uuid>,
    pub event_type: String,
    pub event_version: u16,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

/// Deliberately content-free worker events. Prompts, model output, tool arguments, HTTP headers,
/// and credentials have no representation here and therefore cannot be persisted accidentally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentRunJobWorkerEvent {
    ModelStarted {
        step: u16,
    },
    ModelCompleted {
        step: u16,
        input_tokens: u32,
        output_tokens: u32,
        duration_ms: u64,
    },
    ToolApprovalRequired {
        step: u16,
        tool_id: String,
    },
    ToolStarted {
        step: u16,
        tool_id: String,
    },
    ToolCompleted {
        step: u16,
        tool_id: String,
        success: bool,
        duration_ms: u64,
    },
    CheckpointSaved {
        step: u16,
    },
}

impl AgentRunJobWorkerEvent {
    fn into_database_parts(self) -> AroResult<(&'static str, Value)> {
        match self {
            Self::ModelStarted { step } => Ok(("model.started", json!({ "step": step }))),
            Self::ModelCompleted {
                step,
                input_tokens,
                output_tokens,
                duration_ms,
            } => Ok((
                "model.completed",
                json!({
                    "step": step,
                    "inputTokens": input_tokens,
                    "outputTokens": output_tokens,
                    "durationMs": duration_ms,
                }),
            )),
            Self::ToolApprovalRequired { step, tool_id } => {
                validate_safe_identifier(&tool_id, "agent tool id", 120)?;
                Ok((
                    "tool.approval_required",
                    json!({ "step": step, "toolId": tool_id }),
                ))
            }
            Self::ToolStarted { step, tool_id } => {
                validate_safe_identifier(&tool_id, "agent tool id", 120)?;
                Ok(("tool.started", json!({ "step": step, "toolId": tool_id })))
            }
            Self::ToolCompleted {
                step,
                tool_id,
                success,
                duration_ms,
            } => {
                validate_safe_identifier(&tool_id, "agent tool id", 120)?;
                Ok((
                    "tool.completed",
                    json!({
                        "step": step,
                        "toolId": tool_id,
                        "success": success,
                        "durationMs": duration_ms,
                    }),
                ))
            }
            Self::CheckpointSaved { step } => Ok(("checkpoint.saved", json!({ "step": step }))),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AgentRunJobSubmission {
    Created(AgentRunJobView),
    Replayed(AgentRunJobView),
    Conflict,
}

/// Capability proving ownership of one live worker attempt.
#[derive(Clone, PartialEq, Eq)]
pub struct AgentRunJobLease {
    job_id: Uuid,
    organization_id: Uuid,
    agent_run_id: Uuid,
    token: Uuid,
    generation: i64,
    expires_at: DateTime<Utc>,
}

impl AgentRunJobLease {
    pub const fn job_id(&self) -> Uuid {
        self.job_id
    }

    pub const fn organization_id(&self) -> Uuid {
        self.organization_id
    }

    pub const fn agent_run_id(&self) -> Uuid {
        self.agent_run_id
    }

    pub const fn generation(&self) -> i64 {
        self.generation
    }

    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

/// A claimed request snapshot. This type intentionally implements neither `Debug` nor
/// `Serialize`; a snapshot can contain user prompts and short-lived integration credentials.
pub struct ClaimedAgentRunJob {
    pub lease: AgentRunJobLease,
    pub submitted_by_user_id: Uuid,
    pub request_snapshot: Value,
    pub priority: AgentRunPriority,
    pub budgets: AgentRunJobBudgets,
    pub attempts: u16,
    pub usage: AgentRunJobUsage,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub pause_requested_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRunJobLeaseRenewal {
    Renewed { expires_at: DateTime<Utc> },
    CancellationRequested,
    PauseRequested,
    BudgetExceeded,
    ProgressRejected,
    DeadlineExceeded,
    LeaseLost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRunJobRetryOutcome {
    RetryScheduled { available_at: DateTime<Utc> },
    Failed,
    Cancelled,
    Paused,
    LeaseLost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRunJobCompletionOutcome {
    Completed { result_message_id: Option<Uuid> },
    AlreadyCompleted { result_message_id: Option<Uuid> },
    CancellationRequested,
    PauseRequested,
    BudgetExceeded,
    ProgressRejected,
    DeadlineExceeded,
    LeaseLost,
}

#[derive(Debug, Clone)]
pub struct AgentRunJobCompletion {
    pub content: String,
    pub model_id: Option<String>,
    pub token_estimate: Option<u32>,
    pub checkpoint_summary: Option<String>,
    pub usage: AgentRunJobUsage,
}

impl AroStore {
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_agent_run_job(
        &self,
        context: TenantContext,
        run: &AgentRun,
        submission_key: &str,
        request_snapshot: &Value,
        budgets: AgentRunJobBudgets,
        keyring: &AgentSnapshotKeyring,
    ) -> AroResult<AgentRunJobSubmission> {
        validate_bounded_trimmed(
            submission_key,
            "agent job submission key",
            AGENT_JOB_MAX_SUBMISSION_KEY_BYTES,
        )?;
        validate_json_object(request_snapshot, "agent job request snapshot")?;
        let (snapshot_key_id, snapshot_key) = keyring.current()?;
        let budgets = budgets.validate()?;
        if run.id.is_nil() || run.goal.trim().is_empty() {
            return Err(AroError::Configuration(
                "agent run id and goal are required".to_string(),
            ));
        }
        let mode = enum_to_string(&run.mode)?;
        let priority = enum_to_string(&run.priority)?;
        let request_identity = json!({
            "schemaVersion": 1,
            "request": request_snapshot,
            "run": {
                "laneId": run.lane_id,
                "conversationId": run.conversation_id,
                "goal": run.goal.trim(),
                "mode": mode,
                "priority": priority,
                "modelProviderId": run.model_provider_id,
                "modelId": run.model_id,
                "autonomyProfileId": run.autonomy_profile_id,
            },
            "budgets": budgets,
        });
        let request_hash: [u8; 32] =
            Sha256::digest(canonical_json_bytes(&request_identity)?).into();

        let mut tx = self.begin_tenant_tx(context).await?;
        // Serialize only identical idempotency identities. Hash collisions merely add harmless
        // serialization and cannot merge data because the full key is checked below.
        sqlx::query(
            r#"
            SELECT pg_advisory_xact_lock(
              hashtextextended($1 || ':' || $2 || ':' || $3, 0)
            )
            "#,
        )
        .bind(context.organization_id().to_string())
        .bind(context.actor_id().to_string())
        .bind(submission_key)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        if let Some(existing) =
            load_submission_by_key(&mut tx, context, submission_key, &request_hash).await?
        {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(existing);
        }

        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('aro-agent-quota:' || $1, 0))")
            .bind(context.organization_id().to_string())
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let (organization_jobs, user_jobs): (i64, i64) = sqlx::query_as(
            r#"
            SELECT
              COUNT(*)::bigint,
              COUNT(*) FILTER (WHERE submitted_by_user_id = $2)::bigint
            FROM agent_run_jobs
            WHERE organization_id = $1
              AND status NOT IN ('completed', 'failed', 'cancelled')
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if organization_jobs >= AGENT_JOB_MAX_NON_TERMINAL_PER_ORGANIZATION
            || user_jobs >= AGENT_JOB_MAX_NON_TERMINAL_PER_USER
        {
            return Err(AroError::Configuration(
                "durable agent queue capacity has been reached".to_string(),
            ));
        }

        if let Some(conversation_id) = run.conversation_id {
            let accessible: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS (
                  SELECT 1
                  FROM conversations
                  WHERE id = $1
                    AND organization_id = $2
                    AND owner_user_id = $3
                    AND deleted_at IS NULL
                )
                "#,
            )
            .bind(conversation_id)
            .bind(context.organization_id())
            .bind(context.actor_id())
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            if !accessible {
                return Err(AroError::Security(
                    "agent conversation access denied".to_string(),
                ));
            }
        }

        if let Some(lane_id) = run.lane_id {
            let accessible: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS (
                  SELECT 1
                  FROM agent_lanes
                  WHERE id = $1
                    AND organization_id = $2
                    AND owner_user_id = $3
                    AND deleted_at IS NULL
                )
                "#,
            )
            .bind(lane_id)
            .bind(context.organization_id())
            .bind(context.actor_id())
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            if !accessible {
                return Err(AroError::Security("agent lane access denied".to_string()));
            }
        }

        let permission_ceiling = if let Some(profile_id) = run.autonomy_profile_id {
            Some(
                sqlx::query_scalar::<_, Value>(
                    r#"
                    SELECT jsonb_build_object(
                      'id', id,
                      'name', name,
                      'trustedRoots', trusted_roots,
                      'allowedDomains', allowed_domains,
                      'allowRead', allow_read,
                      'allowWrite', allow_write,
                      'allowShell', allow_shell,
                      'allowNetwork', allow_network,
                      'commandApproval', command_approval,
                      'redactSecrets', redact_secrets,
                      'updatedAt', updated_at
                    )
                    FROM agent_permission_profiles
                    WHERE id = $1
                      AND organization_id = $2
                      AND owner_user_id = $3
                      AND deleted_at IS NULL
                    FOR SHARE
                    "#,
                )
                .bind(profile_id)
                .bind(context.organization_id())
                .bind(context.actor_id())
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_sqlx)?
                .ok_or_else(|| {
                    AroError::Security("agent permission profile access denied".to_string())
                })?,
            )
        } else {
            None
        };
        let execution_envelope = json!({
            "schemaVersion": 1,
            "request": request_snapshot,
            "run": {
                "laneId": run.lane_id,
                "conversationId": run.conversation_id,
                "goal": run.goal.trim(),
                "mode": mode,
                "priority": priority,
                "modelProviderId": run.model_provider_id,
                "modelId": run.model_id,
                "autonomyProfileId": run.autonomy_profile_id,
            },
            "budgets": budgets,
            "permissionCeiling": permission_ceiling,
        });
        validate_json_object(&execution_envelope, "agent job execution envelope")?;
        sqlx::query(
            r#"
            INSERT INTO agent_runs (
              id, organization_id, owner_user_id, lane_id, conversation_id, goal, mode,
              status, priority, model_provider_id, model_id, autonomy_profile_id,
              checkpoint_summary, created_at, updated_at
            )
            VALUES (
              $1, $2, $3, $4, $5, $6, $7,
              'queued', $8, $9, $10, $11,
              $12, now(), now()
            )
            "#,
        )
        .bind(run.id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(run.lane_id)
        .bind(run.conversation_id)
        .bind(run.goal.trim())
        .bind(&mode)
        .bind(&priority)
        .bind(&run.model_provider_id)
        .bind(&run.model_id)
        .bind(run.autonomy_profile_id)
        .bind(&run.checkpoint_summary)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let row = sqlx::query(
            r#"
            INSERT INTO agent_run_jobs (
              organization_id, agent_run_id, submitted_by_user_id, submission_key,
              request_hash, request_snapshot_encrypted, request_snapshot_key_id,
              priority, max_attempts, max_steps,
              max_tool_calls, max_input_tokens, max_output_tokens, max_wall_time_seconds
            )
            VALUES (
              $1, $2, $3, $4, $5,
              pgp_sym_encrypt($6::text, $15, 'cipher-algo=aes256,compress-algo=1'),
              $7, $8, $9, $10, $11, $12, $13, $14
            )
            RETURNING
              id, organization_id, agent_run_id, status, priority, max_attempts, max_steps,
              max_tool_calls, max_input_tokens, max_output_tokens, max_wall_time_seconds,
              attempts, steps_consumed, tool_calls_consumed, input_tokens_consumed,
              output_tokens_consumed, available_at, cancel_requested_at, pause_requested_at, started_at,
              completed_at, state_reason, last_error, result_message_id, created_at, updated_at
            "#,
        )
        .bind(context.organization_id())
        .bind(run.id)
        .bind(context.actor_id())
        .bind(submission_key)
        .bind(request_hash.as_slice())
        .bind(&execution_envelope)
        .bind(snapshot_key_id)
        .bind(&priority)
        .bind(i16::try_from(budgets.max_attempts).map_err(|_| budget_conversion_error())?)
        .bind(i16::try_from(budgets.max_steps).map_err(|_| budget_conversion_error())?)
        .bind(i16::try_from(budgets.max_tool_calls).map_err(|_| budget_conversion_error())?)
        .bind(i32::try_from(budgets.max_input_tokens).map_err(|_| budget_conversion_error())?)
        .bind(i32::try_from(budgets.max_output_tokens).map_err(|_| budget_conversion_error())?)
        .bind(i32::try_from(budgets.max_wall_time_seconds).map_err(|_| budget_conversion_error())?)
        .bind(snapshot_key)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let job = map_agent_job_view(&row)?;
        insert_agent_job_event(
            &mut tx,
            context.organization_id(),
            run.id,
            Some(job.id),
            "job.submitted",
            json!({
                "priority": job.priority,
                "budgets": job.budgets,
            }),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(AgentRunJobSubmission::Created(job))
    }

    pub async fn get_agent_run_job(
        &self,
        context: TenantContext,
        run_id: Uuid,
    ) -> AroResult<Option<AgentRunJobView>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            SELECT
              j.id, j.organization_id, j.agent_run_id, j.status, j.priority,
              j.max_attempts, j.max_steps, j.max_tool_calls, j.max_input_tokens,
              j.max_output_tokens, j.max_wall_time_seconds, j.attempts,
              j.steps_consumed, j.tool_calls_consumed, j.input_tokens_consumed,
              j.output_tokens_consumed, j.available_at, j.cancel_requested_at, j.pause_requested_at,
              j.started_at, j.completed_at, j.state_reason, j.last_error,
              j.result_message_id, j.created_at, j.updated_at
            FROM agent_run_jobs AS j
            JOIN agent_runs AS r
              ON r.organization_id = j.organization_id
             AND r.id = j.agent_run_id
            WHERE j.organization_id = $1
              AND j.agent_run_id = $2
              AND r.owner_user_id = $3
              AND r.deleted_at IS NULL
            "#,
        )
        .bind(context.organization_id())
        .bind(run_id)
        .bind(context.actor_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let job = row.as_ref().map(map_agent_job_view).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(job)
    }

    pub async fn list_durable_agent_run_events(
        &self,
        context: TenantContext,
        run_id: Uuid,
        after_cursor: Option<i64>,
        limit: i64,
    ) -> AroResult<Vec<AgentRunJobEvent>> {
        if !(1..=AGENT_JOB_MAX_EVENT_PAGE_SIZE).contains(&limit) {
            return Err(AroError::Configuration(format!(
                "agent event page size must be between 1 and {AGENT_JOB_MAX_EVENT_PAGE_SIZE}"
            )));
        }
        if after_cursor.is_some_and(|cursor| cursor < 0) {
            return Err(AroError::Configuration(
                "agent event cursor cannot be negative".to_string(),
            ));
        }
        let mut tx = self.begin_tenant_tx(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT
              e.event_cursor, e.event_id, e.agent_run_id, e.job_id, e.event_type,
              e.event_version, e.payload, e.created_at
            FROM agent_run_events AS e
            JOIN agent_runs AS r
              ON r.organization_id = e.organization_id
             AND r.id = e.agent_run_id
            WHERE e.organization_id = $1
              AND e.agent_run_id = $2
              AND r.owner_user_id = $3
              AND r.deleted_at IS NULL
              AND e.event_cursor > COALESCE($4, 0)
            ORDER BY e.event_cursor ASC
            LIMIT $5
            "#,
        )
        .bind(context.organization_id())
        .bind(run_id)
        .bind(context.actor_id())
        .bind(after_cursor)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let events = rows
            .iter()
            .map(map_agent_job_event)
            .collect::<AroResult<Vec<_>>>()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(events)
    }
}

impl AroStore {
    pub async fn claim_agent_run_job(
        &self,
        worker_id: &str,
        lease_seconds: i64,
        keyring: &AgentSnapshotKeyring,
    ) -> AroResult<Option<ClaimedAgentRunJob>> {
        validate_bounded_trimmed(worker_id, "agent worker id", AGENT_JOB_MAX_WORKER_ID_BYTES)?;
        validate_lease_seconds(lease_seconds)?;
        let lease_seconds = i32::try_from(lease_seconds).map_err(|_| {
            AroError::Configuration("agent job lease duration is too large".to_string())
        })?;
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        // Claim transactions are short and globally serialized so two replicas cannot both pass
        // the same lane-capacity COUNT while locking different job rows. Execution remains fully
        // concurrent; only the millisecond-scale scheduling decision is serialized.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('aro-agent-job-claim', 0))")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        let row = sqlx::query(
            r#"
            WITH candidate AS (
              SELECT j.id
              FROM agent_run_jobs AS j
              JOIN agent_runs AS r
                ON r.organization_id = j.organization_id
               AND r.id = j.agent_run_id
              JOIN organizations AS o
                ON o.id = j.organization_id
               AND o.deleted_at IS NULL
              JOIN users AS u
                ON u.id = j.submitted_by_user_id
               AND u.deleted_at IS NULL
              JOIN memberships AS membership
                ON membership.organization_id = j.organization_id
               AND membership.user_id = j.submitted_by_user_id
               AND membership.status = 'active'
               AND membership.deleted_at IS NULL
              LEFT JOIN agent_lanes AS lane
                ON lane.organization_id = r.organization_id
               AND lane.id = r.lane_id
              LEFT JOIN conversations AS conversation
                ON conversation.organization_id = r.organization_id
               AND conversation.id = r.conversation_id
              WHERE r.deleted_at IS NULL
                AND r.owner_user_id = j.submitted_by_user_id
                AND (r.conversation_id IS NULL OR conversation.deleted_at IS NULL)
                AND j.cancel_requested_at IS NULL
                AND j.pause_requested_at IS NULL
                AND j.attempts < j.max_attempts
                AND j.status IN ('queued', 'retry_wait')
                AND j.available_at <= now()
                AND j.steps_consumed < j.max_steps
                AND j.input_tokens_consumed < j.max_input_tokens
                AND j.output_tokens_consumed < j.max_output_tokens
                AND (
                  j.started_at IS NULL
                  OR j.started_at + make_interval(secs => j.max_wall_time_seconds) > now()
                )
                AND (
                  r.lane_id IS NULL
                  OR (
                    lane.id IS NOT NULL
                    AND lane.deleted_at IS NULL
                    AND lane.status = 'active'
                    AND (
                      SELECT COUNT(*)
                      FROM agent_run_jobs AS active_job
                      JOIN agent_runs AS active_run
                        ON active_run.organization_id = active_job.organization_id
                       AND active_run.id = active_job.agent_run_id
                      WHERE active_run.organization_id = r.organization_id
                        AND active_run.lane_id = r.lane_id
                        AND active_job.status IN ('leased', 'running')
                        AND active_job.lease_expires_at > now()
                    ) < lane.max_concurrent_runs
                  )
                )
              ORDER BY
                GREATEST(
                  0,
                  CASE j.priority
                    WHEN 'critical' THEN 0
                    WHEN 'high' THEN 1
                    WHEN 'normal' THEN 2
                    ELSE 3
                  END
                  - LEAST(
                      3,
                      FLOOR(EXTRACT(EPOCH FROM (now() - j.created_at)) / 300)::integer
                    )
                ),
                j.available_at,
                j.created_at,
                j.id
              FOR UPDATE OF j SKIP LOCKED
              LIMIT 1
            )
            UPDATE agent_run_jobs AS j
            SET status = 'leased',
                attempts = j.attempts + 1,
                lease_generation = j.lease_generation + 1,
                lease_token = gen_random_uuid(),
                lease_owner = $1,
                leased_at = now(),
                started_at = COALESCE(j.started_at, now()),
                lease_expires_at = LEAST(
                  now() + make_interval(secs => $2),
                  COALESCE(j.started_at, now())
                    + make_interval(secs => j.max_wall_time_seconds)
                ),
                state_reason = NULL,
                last_error = NULL,
                updated_at = now()
            FROM candidate
            WHERE j.id = candidate.id
            RETURNING
              j.id, j.organization_id, j.agent_run_id, j.submitted_by_user_id,
              j.request_snapshot_key_id, j.priority, j.max_attempts, j.max_steps,
              j.max_tool_calls, j.max_input_tokens, j.max_output_tokens,
              j.max_wall_time_seconds, j.attempts, j.steps_consumed,
              j.tool_calls_consumed, j.input_tokens_consumed,
              j.output_tokens_consumed, j.cancel_requested_at, j.pause_requested_at, j.started_at,
              j.lease_token, j.lease_generation, j.lease_expires_at
            "#,
        )
        .bind(worker_id)
        .bind(lease_seconds)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(None);
        };
        let snapshot_key_id: String = row.get("request_snapshot_key_id");
        let snapshot_key = keyring.key(&snapshot_key_id).ok_or_else(|| {
            AroError::Configuration(format!(
                "agent snapshot keyring is missing key id {snapshot_key_id}"
            ))
        })?;
        let request_snapshot: Value = sqlx::query_scalar(
            r#"
            SELECT pgp_sym_decrypt(request_snapshot_encrypted, $2)::jsonb
            FROM agent_run_jobs
            WHERE id = $1
            "#,
        )
        .bind(row.get::<Uuid, _>("id"))
        .bind(snapshot_key)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let claimed = map_claimed_agent_job(&row, request_snapshot)?;
        if !lock_and_revalidate_agent_job_authorization(
            &mut tx,
            claimed.lease.job_id,
            claimed.lease.organization_id,
            claimed.submitted_by_user_id,
        )
        .await?
        {
            tx.rollback().await.map_err(map_sqlx)?;
            return Ok(None);
        }
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = 'running',
                heartbeat_at = now(),
                updated_at = now(),
                completed_at = NULL,
                last_error = NULL
            WHERE organization_id = $1
              AND id = $2
              AND deleted_at IS NULL
            "#,
        )
        .bind(claimed.lease.organization_id)
        .bind(claimed.lease.agent_run_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        insert_agent_job_event(
            &mut tx,
            claimed.lease.organization_id,
            claimed.lease.agent_run_id,
            Some(claimed.lease.job_id),
            "job.leased",
            json!({
                "attempt": claimed.attempts,
                "generation": claimed.lease.generation,
            }),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(Some(claimed))
    }

    pub async fn renew_agent_run_job_lease(
        &self,
        lease: &AgentRunJobLease,
        lease_seconds: i64,
        usage: AgentRunJobUsage,
    ) -> AroResult<AgentRunJobLeaseRenewal> {
        validate_lease_seconds(lease_seconds)?;
        let lease_seconds = i32::try_from(lease_seconds).map_err(|_| {
            AroError::Configuration("agent job lease duration is too large".to_string())
        })?;
        let usage_db = usage_as_database(usage)?;
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        let row = sqlx::query(
            r#"
            SELECT
              submitted_by_user_id, cancel_requested_at, pause_requested_at,
              steps_consumed, tool_calls_consumed, input_tokens_consumed,
              output_tokens_consumed, max_steps, max_tool_calls, max_input_tokens,
              max_output_tokens,
              started_at + make_interval(secs => max_wall_time_seconds) <= now()
                AS deadline_exceeded
            FROM agent_run_jobs
            WHERE id = $1
              AND organization_id = $2
              AND agent_run_id = $3
              AND lease_token = $4
              AND lease_generation = $5
              AND status IN ('leased', 'running')
              AND lease_expires_at > now()
            FOR UPDATE
            "#,
        )
        .bind(lease.job_id)
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(lease.token)
        .bind(lease.generation)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            let diagnosis = diagnose_agent_job_lease(&mut tx, lease, usage).await?;
            if matches!(diagnosis, AgentRunJobLeaseRenewal::DeadlineExceeded) {
                fail_agent_job_with_live_lease(&mut tx, lease, "wall_time_budget_exceeded").await?;
            }
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(diagnosis);
        };
        if row
            .get::<Option<DateTime<Utc>>, _>("cancel_requested_at")
            .is_some()
        {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobLeaseRenewal::CancellationRequested);
        }
        if row
            .get::<Option<DateTime<Utc>>, _>("pause_requested_at")
            .is_some()
        {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobLeaseRenewal::PauseRequested);
        }
        if row.get::<bool, _>("deadline_exceeded") {
            fail_agent_job_with_live_lease(&mut tx, lease, "wall_time_budget_exceeded").await?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobLeaseRenewal::DeadlineExceeded);
        }
        if usage_exceeds_budget(&row, &usage_db) {
            fail_agent_job_with_live_lease(&mut tx, lease, "execution_budget_exceeded").await?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobLeaseRenewal::BudgetExceeded);
        }
        if usage_regresses(&row, &usage_db) {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobLeaseRenewal::ProgressRejected);
        }
        let actor_id: Uuid = row.get("submitted_by_user_id");
        if !lock_and_revalidate_agent_job_authorization(
            &mut tx,
            lease.job_id,
            lease.organization_id,
            actor_id,
        )
        .await?
        {
            cancel_agent_job_after_authorization_loss(&mut tx, lease).await?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobLeaseRenewal::LeaseLost);
        }
        let expires_at = sqlx::query_scalar::<_, DateTime<Utc>>(
            r#"
            UPDATE agent_run_jobs
            SET status = 'running',
                steps_consumed = $6,
                tool_calls_consumed = $7,
                input_tokens_consumed = $8,
                output_tokens_consumed = $9,
                lease_expires_at = LEAST(
                  now() + make_interval(secs => $10),
                  started_at + make_interval(secs => max_wall_time_seconds)
                ),
                updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND agent_run_id = $3
              AND lease_token = $4
              AND lease_generation = $5
            RETURNING lease_expires_at
            "#,
        )
        .bind(lease.job_id)
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(lease.token)
        .bind(lease.generation)
        .bind(usage_db.steps)
        .bind(usage_db.tool_calls)
        .bind(usage_db.input_tokens)
        .bind(usage_db.output_tokens)
        .bind(lease_seconds)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = 'running', heartbeat_at = now(), updated_at = now()
            WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(AgentRunJobLeaseRenewal::Renewed { expires_at })
    }

    pub async fn append_durable_agent_run_event(
        &self,
        lease: &AgentRunJobLease,
        event: AgentRunJobWorkerEvent,
    ) -> AroResult<Option<AgentRunJobEvent>> {
        let (event_type, payload) = event.into_database_parts()?;
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        let row = sqlx::query(
            r#"
            WITH live_lease AS (
              SELECT organization_id, agent_run_id, id
              FROM agent_run_jobs
              WHERE id = $1
                AND organization_id = $2
                AND agent_run_id = $3
                AND lease_token = $4
                AND lease_generation = $5
                AND status IN ('leased', 'running')
                AND lease_expires_at > now()
                AND cancel_requested_at IS NULL
                AND pause_requested_at IS NULL
                AND started_at + make_interval(secs => max_wall_time_seconds) > now()
              FOR SHARE
            )
            INSERT INTO agent_run_events (
              organization_id, agent_run_id, job_id, event_type, payload
            )
            SELECT organization_id, agent_run_id, id, $6, $7
            FROM live_lease
            RETURNING
              event_cursor, event_id, agent_run_id, job_id, event_type,
              event_version, payload, created_at
            "#,
        )
        .bind(lease.job_id)
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(lease.token)
        .bind(lease.generation)
        .bind(event_type)
        .bind(payload)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let event = row.as_ref().map(map_agent_job_event).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(event)
    }

    pub async fn request_agent_run_job_cancellation(
        &self,
        context: TenantContext,
        run_id: Uuid,
    ) -> AroResult<AgentRunJobView> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            SELECT
              j.id, j.organization_id, j.agent_run_id, j.status, j.priority,
              j.max_attempts, j.max_steps, j.max_tool_calls, j.max_input_tokens,
              j.max_output_tokens, j.max_wall_time_seconds, j.attempts,
              j.steps_consumed, j.tool_calls_consumed, j.input_tokens_consumed,
              j.output_tokens_consumed, j.available_at, j.cancel_requested_at, j.pause_requested_at,
              j.started_at, j.completed_at, j.state_reason, j.last_error,
              j.result_message_id, j.created_at, j.updated_at
            FROM agent_run_jobs AS j
            JOIN agent_runs AS r
              ON r.organization_id = j.organization_id
             AND r.id = j.agent_run_id
            WHERE j.organization_id = $1
              AND j.agent_run_id = $2
              AND r.owner_user_id = $3
              AND r.deleted_at IS NULL
            FOR UPDATE OF j, r
            "#,
        )
        .bind(context.organization_id())
        .bind(run_id)
        .bind(context.actor_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Memory("durable agent run not found".to_string()))?;
        let current = map_agent_job_view(&row)?;
        if current.status.is_terminal() {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(current);
        }

        let has_active_worker = matches!(
            current.status,
            AgentRunJobStatus::Leased | AgentRunJobStatus::Running
        );
        let row = if has_active_worker {
            sqlx::query(
                r#"
                UPDATE agent_run_jobs
                SET cancel_requested_at = COALESCE(cancel_requested_at, now()),
                    pause_requested_at = NULL,
                    state_reason = 'cancellation_requested',
                    updated_at = now()
                WHERE id = $1
                RETURNING
                  id, organization_id, agent_run_id, status, priority, max_attempts,
                  max_steps, max_tool_calls, max_input_tokens, max_output_tokens,
                  max_wall_time_seconds, attempts, steps_consumed, tool_calls_consumed,
                  input_tokens_consumed, output_tokens_consumed, available_at,
                  cancel_requested_at, pause_requested_at, started_at, completed_at, state_reason,
                  last_error, result_message_id, created_at, updated_at
                "#,
            )
            .bind(current.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?
        } else {
            let row = sqlx::query(
                r#"
                UPDATE agent_run_jobs
                SET status = 'cancelled',
                    cancel_requested_at = COALESCE(cancel_requested_at, now()),
                    pause_requested_at = NULL,
                    completed_at = now(),
                    state_reason = 'cancelled_by_user',
                    lease_token = NULL,
                    lease_owner = NULL,
                    leased_at = NULL,
                    lease_expires_at = NULL,
                    updated_at = now()
                WHERE id = $1
                RETURNING
                  id, organization_id, agent_run_id, status, priority, max_attempts,
                  max_steps, max_tool_calls, max_input_tokens, max_output_tokens,
                  max_wall_time_seconds, attempts, steps_consumed, tool_calls_consumed,
                  input_tokens_consumed, output_tokens_consumed, available_at,
                  cancel_requested_at, pause_requested_at, started_at, completed_at, state_reason,
                  last_error, result_message_id, created_at, updated_at
                "#,
            )
            .bind(current.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            sqlx::query(
                r#"
                UPDATE agent_runs
                SET status = 'cancelled', completed_at = now(), heartbeat_at = now(),
                    updated_at = now(), last_error = NULL
                WHERE organization_id = $1 AND id = $2
                "#,
            )
            .bind(context.organization_id())
            .bind(run_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            row
        };
        let cancelled = map_agent_job_view(&row)?;
        insert_agent_job_event(
            &mut tx,
            context.organization_id(),
            run_id,
            Some(cancelled.id),
            if has_active_worker {
                "job.cancellation_requested"
            } else {
                "job.cancelled"
            },
            json!({}),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(cancelled)
    }

    pub async fn request_agent_run_job_pause(
        &self,
        context: TenantContext,
        run_id: Uuid,
    ) -> AroResult<AgentRunJobView> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = load_owned_agent_job_for_update(&mut tx, context, run_id)
            .await?
            .ok_or_else(|| AroError::Memory("durable agent run not found".to_string()))?;
        let current = map_agent_job_view(&row)?;
        if current.status.is_terminal() {
            return Err(AroError::Configuration(
                "a terminal agent run cannot be paused".to_string(),
            ));
        }
        if current.cancel_requested_at.is_some() {
            return Err(AroError::Configuration(
                "agent run cancellation is already pending".to_string(),
            ));
        }
        if current.pause_requested_at.is_some()
            || matches!(current.status, AgentRunJobStatus::Waiting)
        {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(current);
        }
        let has_active_worker = matches!(
            current.status,
            AgentRunJobStatus::Leased | AgentRunJobStatus::Running
        );
        let row = if has_active_worker {
            sqlx::query(
                r#"
                UPDATE agent_run_jobs
                SET pause_requested_at = now(),
                    state_reason = 'pause_requested',
                    updated_at = now()
                WHERE id = $1
                RETURNING
                  id, organization_id, agent_run_id, status, priority, max_attempts,
                  max_steps, max_tool_calls, max_input_tokens, max_output_tokens,
                  max_wall_time_seconds, attempts, steps_consumed, tool_calls_consumed,
                  input_tokens_consumed, output_tokens_consumed, available_at,
                  cancel_requested_at, pause_requested_at, started_at, completed_at,
                  state_reason, last_error, result_message_id, created_at, updated_at
                "#,
            )
            .bind(current.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?
        } else {
            sqlx::query(
                r#"
                UPDATE agent_run_jobs
                SET status = 'waiting',
                    pause_requested_at = now(),
                    state_reason = 'paused_by_user',
                    updated_at = now()
                WHERE id = $1
                RETURNING
                  id, organization_id, agent_run_id, status, priority, max_attempts,
                  max_steps, max_tool_calls, max_input_tokens, max_output_tokens,
                  max_wall_time_seconds, attempts, steps_consumed, tool_calls_consumed,
                  input_tokens_consumed, output_tokens_consumed, available_at,
                  cancel_requested_at, pause_requested_at, started_at, completed_at,
                  state_reason, last_error, result_message_id, created_at, updated_at
                "#,
            )
            .bind(current.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?
        };
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = 'paused', heartbeat_at = now(), updated_at = now()
            WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(context.organization_id())
        .bind(run_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let paused = map_agent_job_view(&row)?;
        insert_agent_job_event(
            &mut tx,
            context.organization_id(),
            run_id,
            Some(paused.id),
            if has_active_worker {
                "job.pause_requested"
            } else {
                "job.paused"
            },
            json!({}),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(paused)
    }

    pub async fn resume_agent_run_job(
        &self,
        context: TenantContext,
        run_id: Uuid,
    ) -> AroResult<AgentRunJobView> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = load_owned_agent_job_for_update(&mut tx, context, run_id)
            .await?
            .ok_or_else(|| AroError::Memory("durable agent run not found".to_string()))?;
        let current = map_agent_job_view(&row)?;
        if current.status.is_terminal() {
            return Err(AroError::Configuration(
                "a terminal agent run cannot be resumed".to_string(),
            ));
        }
        if current.cancel_requested_at.is_some() {
            return Err(AroError::Configuration(
                "an agent run pending cancellation cannot be resumed".to_string(),
            ));
        }
        if current.pause_requested_at.is_none()
            && !matches!(current.status, AgentRunJobStatus::Waiting)
        {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(current);
        }
        let has_active_worker = matches!(
            current.status,
            AgentRunJobStatus::Leased | AgentRunJobStatus::Running
        );
        let row = sqlx::query(
            r#"
            UPDATE agent_run_jobs
            SET status = CASE WHEN $2 THEN status ELSE 'queued' END,
                pause_requested_at = NULL,
                available_at = CASE WHEN $2 THEN available_at ELSE now() END,
                state_reason = 'resumed_by_user',
                updated_at = now()
            WHERE id = $1
            RETURNING
              id, organization_id, agent_run_id, status, priority, max_attempts,
              max_steps, max_tool_calls, max_input_tokens, max_output_tokens,
              max_wall_time_seconds, attempts, steps_consumed, tool_calls_consumed,
              input_tokens_consumed, output_tokens_consumed, available_at,
              cancel_requested_at, pause_requested_at, started_at, completed_at,
              state_reason, last_error, result_message_id, created_at, updated_at
            "#,
        )
        .bind(current.id)
        .bind(has_active_worker)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = $3, heartbeat_at = now(), updated_at = now(), completed_at = NULL
            WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(context.organization_id())
        .bind(run_id)
        .bind(if has_active_worker {
            "running"
        } else {
            "queued"
        })
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let resumed = map_agent_job_view(&row)?;
        insert_agent_job_event(
            &mut tx,
            context.organization_id(),
            run_id,
            Some(resumed.id),
            "job.resumed",
            json!({}),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(resumed)
    }

    pub async fn release_agent_run_job_for_retry(
        &self,
        lease: &AgentRunJobLease,
        error_code: &str,
        retry_delay_seconds: i64,
    ) -> AroResult<AgentRunJobRetryOutcome> {
        validate_bounded_trimmed(
            error_code,
            "agent job error code",
            AGENT_JOB_MAX_ERROR_CODE_BYTES,
        )?;
        if !(1..=86_400).contains(&retry_delay_seconds) {
            return Err(AroError::Configuration(
                "agent retry delay must be between 1 second and 24 hours".to_string(),
            ));
        }
        let retry_delay = i32::try_from(retry_delay_seconds)
            .map_err(|_| AroError::Configuration("agent retry delay is too large".to_string()))?;
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        let row = sqlx::query(
            r#"
            SELECT
              attempts, max_attempts, cancel_requested_at, pause_requested_at,
              started_at + make_interval(secs => max_wall_time_seconds) <= now()
                AS deadline_exceeded,
              steps_consumed >= max_steps
                OR input_tokens_consumed >= max_input_tokens
                OR output_tokens_consumed >= max_output_tokens AS budget_exhausted
            FROM agent_run_jobs
            WHERE id = $1
              AND organization_id = $2
              AND agent_run_id = $3
              AND lease_token = $4
              AND lease_generation = $5
              AND status IN ('leased', 'running')
              AND lease_expires_at > now()
            FOR UPDATE
            "#,
        )
        .bind(lease.job_id)
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(lease.token)
        .bind(lease.generation)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobRetryOutcome::LeaseLost);
        };
        let attempts: i16 = row.get("attempts");
        let max_attempts: i16 = row.get("max_attempts");
        let cancel_requested_at: Option<DateTime<Utc>> = row.get("cancel_requested_at");
        let pause_requested_at: Option<DateTime<Utc>> = row.get("pause_requested_at");
        let deadline_exceeded: bool = row.get("deadline_exceeded");
        let budget_exhausted: bool = row.get("budget_exhausted");
        let (status, run_status, reason, completed, event_type) = if cancel_requested_at.is_some() {
            (
                "cancelled",
                "cancelled",
                "cancelled_by_worker",
                true,
                "job.cancelled",
            )
        } else if deadline_exceeded {
            (
                "failed",
                "failed",
                "wall_time_budget_exceeded",
                true,
                "job.failed",
            )
        } else if budget_exhausted {
            (
                "failed",
                "failed",
                "execution_budget_exceeded",
                true,
                "job.failed",
            )
        } else if pause_requested_at.is_some() {
            ("waiting", "paused", "paused_by_worker", false, "job.paused")
        } else if attempts >= max_attempts {
            ("failed", "failed", "attempts_exhausted", true, "job.failed")
        } else {
            (
                "retry_wait",
                "queued",
                "retry_scheduled",
                false,
                "job.retry_scheduled",
            )
        };
        let stored_error = if matches!(
            reason,
            "wall_time_budget_exceeded" | "execution_budget_exceeded"
        ) {
            reason
        } else {
            error_code
        };
        let row = sqlx::query(
            r#"
            UPDATE agent_run_jobs
            SET status = $6,
                available_at = CASE
                  WHEN $6 = 'retry_wait' THEN now() + make_interval(secs => $7)
                  ELSE available_at
                END,
                lease_token = NULL,
                lease_owner = NULL,
                leased_at = NULL,
                lease_expires_at = NULL,
                completed_at = CASE WHEN $8 THEN now() ELSE NULL END,
                state_reason = $9,
                last_error = $10,
                updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND agent_run_id = $3
              AND lease_token = $4
              AND lease_generation = $5
            RETURNING available_at
            "#,
        )
        .bind(lease.job_id)
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(lease.token)
        .bind(lease.generation)
        .bind(status)
        .bind(retry_delay)
        .bind(completed)
        .bind(reason)
        .bind(stored_error)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobRetryOutcome::LeaseLost);
        };
        let available_at: DateTime<Utc> = row.get("available_at");
        let retry_available_at = (status == "retry_wait").then_some(available_at);
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = $3,
                last_error = $4,
                heartbeat_at = now(),
                updated_at = now(),
                completed_at = CASE WHEN $5 THEN now() ELSE NULL END
            WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(run_status)
        .bind(stored_error)
        .bind(completed)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        insert_agent_job_event(
            &mut tx,
            lease.organization_id,
            lease.agent_run_id,
            Some(lease.job_id),
            event_type,
            json!({
                "attempt": attempts,
                "errorCode": stored_error,
                "availableAt": retry_available_at,
            }),
        )
        .await?;
        let outcome = match status {
            "cancelled" => AgentRunJobRetryOutcome::Cancelled,
            "waiting" => AgentRunJobRetryOutcome::Paused,
            "failed" => AgentRunJobRetryOutcome::Failed,
            _ => AgentRunJobRetryOutcome::RetryScheduled { available_at },
        };
        tx.commit().await.map_err(map_sqlx)?;
        Ok(outcome)
    }

    /// Échec explicite d'un job depuis le worker qui détient le lease.
    /// No-op (Ok) si le lease est perdu. À préférer à un `complete_*` au
    /// contenu inventé quand la génération est impossible (pas de modèle
    /// configuré, provider en panne…).
    pub async fn fail_agent_run_job(
        &self,
        lease: &AgentRunJobLease,
        reason: &str,
    ) -> AroResult<()> {
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        fail_agent_job_with_live_lease(&mut tx, lease, reason).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn complete_agent_run_job(
        &self,
        lease: &AgentRunJobLease,
        completion: AgentRunJobCompletion,
    ) -> AroResult<AgentRunJobCompletionOutcome> {
        if completion.content.trim().is_empty() || completion.content.len() > 1024 * 1024 {
            return Err(AroError::Configuration(
                "agent completion content must be non-empty and at most 1 MiB".to_string(),
            ));
        }
        if let Some(model_id) = completion.model_id.as_deref() {
            validate_bounded_trimmed(model_id, "agent completion model id", 255)?;
        }
        if let Some(summary) = completion.checkpoint_summary.as_deref() {
            validate_bounded_trimmed(summary, "agent checkpoint summary", 2_048)?;
        }
        let usage = usage_as_database(completion.usage)?;
        let token_estimate = completion
            .token_estimate
            .map(|value| i32::try_from(value).map_err(|_| budget_conversion_error()))
            .transpose()?;
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        let row = sqlx::query(
            r#"
            SELECT
              j.cancel_requested_at, j.pause_requested_at, j.steps_consumed, j.tool_calls_consumed,
              j.input_tokens_consumed, j.output_tokens_consumed, j.max_steps,
              j.max_tool_calls, j.max_input_tokens, j.max_output_tokens,
              j.started_at + make_interval(secs => j.max_wall_time_seconds) <= now()
                AS deadline_exceeded,
              r.owner_user_id, r.conversation_id
            FROM agent_run_jobs AS j
            JOIN agent_runs AS r
              ON r.organization_id = j.organization_id
             AND r.id = j.agent_run_id
            WHERE j.id = $1
              AND j.organization_id = $2
              AND j.agent_run_id = $3
              AND j.lease_token = $4
              AND j.lease_generation = $5
              AND j.status IN ('leased', 'running')
              AND j.lease_expires_at > now()
              AND r.deleted_at IS NULL
            FOR UPDATE OF j, r
            "#,
        )
        .bind(lease.job_id)
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(lease.token)
        .bind(lease.generation)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            let prior = sqlx::query(
                r#"
                SELECT status, lease_generation, result_message_id
                FROM agent_run_jobs
                WHERE id = $1 AND organization_id = $2 AND agent_run_id = $3
                "#,
            )
            .bind(lease.job_id)
            .bind(lease.organization_id)
            .bind(lease.agent_run_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            let already_completed = match prior {
                Some(prior)
                    if prior.get::<String, _>("status") == "completed"
                        && prior.get::<i64, _>("lease_generation") == lease.generation =>
                {
                    Some(AgentRunJobCompletionOutcome::AlreadyCompleted {
                        result_message_id: prior.get("result_message_id"),
                    })
                }
                _ => None,
            };
            let outcome = if let Some(outcome) = already_completed {
                outcome
            } else {
                match diagnose_agent_job_lease(&mut tx, lease, completion.usage).await? {
                    AgentRunJobLeaseRenewal::DeadlineExceeded => {
                        fail_agent_job_with_live_lease(&mut tx, lease, "wall_time_budget_exceeded")
                            .await?;
                        AgentRunJobCompletionOutcome::DeadlineExceeded
                    }
                    AgentRunJobLeaseRenewal::CancellationRequested => {
                        AgentRunJobCompletionOutcome::CancellationRequested
                    }
                    AgentRunJobLeaseRenewal::PauseRequested => {
                        AgentRunJobCompletionOutcome::PauseRequested
                    }
                    AgentRunJobLeaseRenewal::BudgetExceeded => {
                        AgentRunJobCompletionOutcome::BudgetExceeded
                    }
                    AgentRunJobLeaseRenewal::ProgressRejected => {
                        AgentRunJobCompletionOutcome::ProgressRejected
                    }
                    AgentRunJobLeaseRenewal::Renewed { .. }
                    | AgentRunJobLeaseRenewal::LeaseLost => AgentRunJobCompletionOutcome::LeaseLost,
                }
            };
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(outcome);
        };
        if row
            .get::<Option<DateTime<Utc>>, _>("cancel_requested_at")
            .is_some()
        {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobCompletionOutcome::CancellationRequested);
        }
        if row
            .get::<Option<DateTime<Utc>>, _>("pause_requested_at")
            .is_some()
        {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobCompletionOutcome::PauseRequested);
        }
        if row.get::<bool, _>("deadline_exceeded") {
            fail_agent_job_with_live_lease(&mut tx, lease, "wall_time_budget_exceeded").await?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobCompletionOutcome::DeadlineExceeded);
        }
        if usage_exceeds_budget(&row, &usage) {
            fail_agent_job_with_live_lease(&mut tx, lease, "execution_budget_exceeded").await?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobCompletionOutcome::BudgetExceeded);
        }
        if usage_regresses(&row, &usage) {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobCompletionOutcome::ProgressRejected);
        }

        let owner_user_id: Uuid = row.get("owner_user_id");
        let conversation_id: Option<Uuid> = row.get("conversation_id");
        if !lock_and_revalidate_agent_job_authorization(
            &mut tx,
            lease.job_id,
            lease.organization_id,
            owner_user_id,
        )
        .await?
        {
            cancel_agent_job_after_authorization_loss(&mut tx, lease).await?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(AgentRunJobCompletionOutcome::LeaseLost);
        }
        set_trusted_worker_tenant_context(&mut tx, owner_user_id, lease.organization_id).await?;

        let message_role = enum_to_string(&MessageRole::Assistant)?;
        let result_message_id = if let Some(conversation_id) = conversation_id {
            let owns_conversation = sqlx::query_scalar::<_, i32>(
                r#"
                SELECT 1
                FROM conversations
                WHERE id = $1
                  AND organization_id = $2
                  AND owner_user_id = $3
                  AND deleted_at IS NULL
                FOR SHARE
                "#,
            )
            .bind(conversation_id)
            .bind(lease.organization_id)
            .bind(owner_user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?
            .is_some();
            if !owns_conversation {
                return Err(AroError::Memory(
                    "agent result conversation is no longer available".to_string(),
                ));
            }
            let message_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO messages (
                  id, organization_id, conversation_id, role, content, token_estimate,
                  model_id, source_agent_run_id
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(message_id)
            .bind(lease.organization_id)
            .bind(conversation_id)
            .bind(message_role)
            .bind(&completion.content)
            .bind(token_estimate)
            .bind(&completion.model_id)
            .bind(lease.agent_run_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            sqlx::query(
                r#"
                UPDATE conversations
                SET updated_at = now()
                WHERE organization_id = $1 AND id = $2 AND owner_user_id = $3
                "#,
            )
            .bind(lease.organization_id)
            .bind(conversation_id)
            .bind(owner_user_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            Some(message_id)
        } else {
            None
        };

        let next_sequence: i32 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(sequence), 0) + 1
            FROM agent_steps
            WHERE organization_id = $1 AND run_id = $2
            "#,
        )
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO agent_steps (
              organization_id, run_id, sequence, kind, status, title, input, output,
              finished_at
            )
            VALUES (
              $1, $2, $3, 'final', 'completed', 'Final response', '{}'::jsonb, $4, now()
            )
            "#,
        )
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(next_sequence)
        .bind(json!({
            "content": completion.content,
            "messageId": result_message_id,
        }))
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let updated = sqlx::query(
            r#"
            UPDATE agent_run_jobs
            SET status = 'completed',
                steps_consumed = $6,
                tool_calls_consumed = $7,
                input_tokens_consumed = $8,
                output_tokens_consumed = $9,
                lease_token = NULL,
                lease_owner = NULL,
                leased_at = NULL,
                lease_expires_at = NULL,
                completed_at = now(),
                state_reason = 'completed',
                last_error = NULL,
                result_message_id = $10,
                updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND agent_run_id = $3
              AND lease_token = $4
              AND lease_generation = $5
            RETURNING id
            "#,
        )
        .bind(lease.job_id)
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(lease.token)
        .bind(lease.generation)
        .bind(usage.steps)
        .bind(usage.tool_calls)
        .bind(usage.input_tokens)
        .bind(usage.output_tokens)
        .bind(result_message_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if updated.is_none() {
            return Err(AroError::Unexpected(
                "agent completion lease changed while locked".to_string(),
            ));
        }
        sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = 'completed',
                checkpoint_summary = COALESCE($3, checkpoint_summary),
                last_error = NULL,
                heartbeat_at = now(),
                completed_at = now(),
                updated_at = now()
            WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(lease.organization_id)
        .bind(lease.agent_run_id)
        .bind(completion.checkpoint_summary)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        insert_agent_job_event(
            &mut tx,
            lease.organization_id,
            lease.agent_run_id,
            Some(lease.job_id),
            "job.completed",
            json!({ "resultMessageId": result_message_id }),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(AgentRunJobCompletionOutcome::Completed { result_message_id })
    }

    pub async fn reap_expired_agent_run_job_leases(
        &self,
        limit: i64,
        retry_delay_seconds: i64,
    ) -> AroResult<u64> {
        if !(1..=1_000).contains(&limit) {
            return Err(AroError::Configuration(
                "agent lease reap limit must be between 1 and 1000".to_string(),
            ));
        }
        if !(1..=86_400).contains(&retry_delay_seconds) {
            return Err(AroError::Configuration(
                "agent lease recovery delay must be between 1 second and 24 hours".to_string(),
            ));
        }
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        let rows = sqlx::query(
            r#"
            SELECT
              id, organization_id, agent_run_id, attempts, max_attempts,
              cancel_requested_at, pause_requested_at,
              started_at + make_interval(secs => max_wall_time_seconds) <= now()
                AS deadline_exceeded,
              steps_consumed >= max_steps
                OR input_tokens_consumed >= max_input_tokens
                OR output_tokens_consumed >= max_output_tokens AS budget_exhausted
            FROM agent_run_jobs
            WHERE status IN ('leased', 'running')
              AND lease_expires_at <= now()
            ORDER BY lease_expires_at, id
            FOR UPDATE SKIP LOCKED
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        for row in &rows {
            let job_id: Uuid = row.get("id");
            let organization_id: Uuid = row.get("organization_id");
            let agent_run_id: Uuid = row.get("agent_run_id");
            let attempts: i16 = row.get("attempts");
            let max_attempts: i16 = row.get("max_attempts");
            let cancellation_requested = row
                .get::<Option<DateTime<Utc>>, _>("cancel_requested_at")
                .is_some();
            let pause_requested = row
                .get::<Option<DateTime<Utc>>, _>("pause_requested_at")
                .is_some();
            let deadline_exceeded: bool = row.get("deadline_exceeded");
            let budget_exhausted: bool = row.get("budget_exhausted");
            let exhausted = attempts >= max_attempts;
            let (job_status, run_status, reason, completed, event_type) = if cancellation_requested
            {
                (
                    "cancelled",
                    "cancelled",
                    "cancelled_after_lease_expiry",
                    true,
                    "job.cancelled",
                )
            } else if deadline_exceeded {
                (
                    "failed",
                    "failed",
                    "wall_time_budget_exceeded",
                    true,
                    "job.failed",
                )
            } else if budget_exhausted {
                (
                    "failed",
                    "failed",
                    "execution_budget_exceeded",
                    true,
                    "job.failed",
                )
            } else if pause_requested {
                (
                    "waiting",
                    "paused",
                    "paused_after_lease_expiry",
                    false,
                    "job.paused",
                )
            } else if exhausted {
                ("failed", "failed", "attempts_exhausted", true, "job.failed")
            } else {
                (
                    "retry_wait",
                    "queued",
                    "lease_expired",
                    false,
                    "job.lease_expired",
                )
            };
            sqlx::query(
                r#"
                UPDATE agent_run_jobs
                SET status = $2,
                    available_at = CASE
                      WHEN $3 THEN available_at
                      ELSE now() + make_interval(secs => $4::int)
                    END,
                    lease_token = NULL,
                    lease_owner = NULL,
                    leased_at = NULL,
                    lease_expires_at = NULL,
                    completed_at = CASE WHEN $3 THEN now() ELSE NULL END,
                    state_reason = $5,
                    last_error = CASE
                      WHEN $2 = 'failed' THEN $5
                      WHEN $2 = 'retry_wait' THEN 'worker_lease_expired'
                      ELSE NULL
                    END,
                    updated_at = now()
                WHERE id = $1
                "#,
            )
            .bind(job_id)
            .bind(job_status)
            .bind(completed)
            .bind(i32::try_from(retry_delay_seconds).map_err(|_| {
                AroError::Configuration("agent lease recovery delay is too large".to_string())
            })?)
            .bind(reason)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            sqlx::query(
                r#"
                UPDATE agent_runs
                SET status = $3,
                    last_error = CASE
                      WHEN $3 = 'failed' THEN $5
                      WHEN $3 = 'queued' THEN 'worker_lease_expired'
                      ELSE NULL
                    END,
                    heartbeat_at = now(),
                    completed_at = CASE WHEN $4 THEN now() ELSE NULL END,
                    updated_at = now()
                WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(organization_id)
            .bind(agent_run_id)
            .bind(run_status)
            .bind(completed)
            .bind(reason)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            insert_agent_job_event(
                &mut tx,
                organization_id,
                agent_run_id,
                Some(job_id),
                event_type,
                json!({ "attempt": attempts }),
            )
            .await?;
        }
        let count = u64::try_from(rows.len())
            .map_err(|_| AroError::Unexpected("agent lease recovery count overflow".to_string()))?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(count)
    }

    /// Terminalizes non-terminal jobs that can no longer be authorized or can no longer make
    /// progress within their immutable budgets. This prevents encrypted snapshots from being
    /// retained forever in an unclaimable state.
    pub async fn reconcile_ineligible_agent_run_jobs(&self, limit: i64) -> AroResult<u64> {
        if !(1..=1_000).contains(&limit) {
            return Err(AroError::Configuration(
                "agent reconciliation limit must be between 1 and 1000".to_string(),
            ));
        }
        let mut tx = self.pool().begin().await.map_err(map_sqlx)?;
        let rows = sqlx::query(
            r#"
            SELECT
              j.id, j.organization_id, j.agent_run_id,
              COALESCE(
                j.started_at + make_interval(secs => j.max_wall_time_seconds) <= now(),
                false
              ) AS deadline_exceeded,
              j.steps_consumed >= j.max_steps
                OR j.input_tokens_consumed >= j.max_input_tokens
                OR j.output_tokens_consumed >= j.max_output_tokens AS budget_exhausted,
              NOT EXISTS (
                SELECT 1
                FROM agent_runs AS r
                JOIN memberships AS membership
                  ON membership.organization_id = r.organization_id
                 AND membership.user_id = r.owner_user_id
                 AND membership.status = 'active'
                 AND membership.deleted_at IS NULL
                JOIN users AS u
                  ON u.id = membership.user_id
                 AND u.deleted_at IS NULL
                JOIN organizations AS o
                  ON o.id = membership.organization_id
                 AND o.deleted_at IS NULL
                WHERE r.organization_id = j.organization_id
                  AND r.id = j.agent_run_id
                  AND r.owner_user_id = j.submitted_by_user_id
                  AND r.deleted_at IS NULL
                  AND (
                    r.conversation_id IS NULL
                    OR EXISTS (
                      SELECT 1
                      FROM conversations AS conversation
                      WHERE conversation.organization_id = r.organization_id
                        AND conversation.id = r.conversation_id
                        AND conversation.owner_user_id = r.owner_user_id
                        AND conversation.deleted_at IS NULL
                    )
                  )
                  AND (
                    r.lane_id IS NULL
                    OR EXISTS (
                      SELECT 1
                      FROM agent_lanes AS lane
                      WHERE lane.organization_id = r.organization_id
                        AND lane.id = r.lane_id
                        AND lane.owner_user_id = r.owner_user_id
                        AND lane.deleted_at IS NULL
                    )
                  )
                  AND (
                    r.autonomy_profile_id IS NULL
                    OR EXISTS (
                      SELECT 1
                      FROM agent_permission_profiles AS profile
                      WHERE profile.organization_id = r.organization_id
                        AND profile.id = r.autonomy_profile_id
                        AND profile.owner_user_id = r.owner_user_id
                        AND profile.deleted_at IS NULL
                    )
                  )
              ) AS authorization_lost
            FROM agent_run_jobs AS j
            WHERE j.status NOT IN ('completed', 'failed', 'cancelled')
              AND (
                COALESCE(
                  j.started_at + make_interval(secs => j.max_wall_time_seconds) <= now(),
                  false
                )
                OR j.steps_consumed >= j.max_steps
                OR j.input_tokens_consumed >= j.max_input_tokens
                OR j.output_tokens_consumed >= j.max_output_tokens
                OR NOT EXISTS (
                  SELECT 1
                  FROM agent_runs AS eligible_run
                  JOIN memberships AS eligible_membership
                    ON eligible_membership.organization_id = eligible_run.organization_id
                   AND eligible_membership.user_id = eligible_run.owner_user_id
                   AND eligible_membership.status = 'active'
                   AND eligible_membership.deleted_at IS NULL
                  JOIN users AS eligible_user
                    ON eligible_user.id = eligible_membership.user_id
                   AND eligible_user.deleted_at IS NULL
                  JOIN organizations AS eligible_organization
                    ON eligible_organization.id = eligible_membership.organization_id
                   AND eligible_organization.deleted_at IS NULL
                  WHERE eligible_run.organization_id = j.organization_id
                    AND eligible_run.id = j.agent_run_id
                    AND eligible_run.owner_user_id = j.submitted_by_user_id
                    AND eligible_run.deleted_at IS NULL
                    AND (
                      eligible_run.conversation_id IS NULL
                      OR EXISTS (
                        SELECT 1 FROM conversations AS eligible_conversation
                        WHERE eligible_conversation.organization_id = eligible_run.organization_id
                          AND eligible_conversation.id = eligible_run.conversation_id
                          AND eligible_conversation.owner_user_id = eligible_run.owner_user_id
                          AND eligible_conversation.deleted_at IS NULL
                      )
                    )
                    AND (
                      eligible_run.lane_id IS NULL
                      OR EXISTS (
                        SELECT 1 FROM agent_lanes AS eligible_lane
                        WHERE eligible_lane.organization_id = eligible_run.organization_id
                          AND eligible_lane.id = eligible_run.lane_id
                          AND eligible_lane.owner_user_id = eligible_run.owner_user_id
                          AND eligible_lane.deleted_at IS NULL
                      )
                    )
                    AND (
                      eligible_run.autonomy_profile_id IS NULL
                      OR EXISTS (
                        SELECT 1 FROM agent_permission_profiles AS eligible_profile
                        WHERE eligible_profile.organization_id = eligible_run.organization_id
                          AND eligible_profile.id = eligible_run.autonomy_profile_id
                          AND eligible_profile.owner_user_id = eligible_run.owner_user_id
                          AND eligible_profile.deleted_at IS NULL
                      )
                    )
                )
              )
            ORDER BY j.updated_at, j.id
            FOR UPDATE OF j SKIP LOCKED
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        for row in &rows {
            let job_id: Uuid = row.get("id");
            let organization_id: Uuid = row.get("organization_id");
            let agent_run_id: Uuid = row.get("agent_run_id");
            let deadline_exceeded: bool = row.get("deadline_exceeded");
            let budget_exhausted: bool = row.get("budget_exhausted");
            let authorization_lost: bool = row.get("authorization_lost");
            let (status, reason) = if deadline_exceeded {
                ("failed", "wall_time_budget_exceeded")
            } else if budget_exhausted {
                ("failed", "execution_budget_exceeded")
            } else if authorization_lost {
                ("cancelled", "authorization_revoked")
            } else {
                continue;
            };
            sqlx::query(
                r#"
                UPDATE agent_run_jobs
                SET status = $2,
                    cancel_requested_at = CASE
                      WHEN $2 = 'cancelled' THEN COALESCE(cancel_requested_at, now())
                      ELSE cancel_requested_at
                    END,
                    pause_requested_at = CASE WHEN $2 = 'cancelled' THEN NULL ELSE pause_requested_at END,
                    lease_token = NULL,
                    lease_owner = NULL,
                    leased_at = NULL,
                    lease_expires_at = NULL,
                    completed_at = now(),
                    state_reason = $3,
                    last_error = CASE WHEN $2 = 'failed' THEN $3 ELSE NULL END,
                    updated_at = now()
                WHERE id = $1
                "#,
            )
            .bind(job_id)
            .bind(status)
            .bind(reason)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            sqlx::query(
                r#"
                UPDATE agent_runs
                SET status = $3,
                    last_error = CASE WHEN $3 = 'failed' THEN $4 ELSE NULL END,
                    completed_at = now(),
                    heartbeat_at = now(),
                    updated_at = now()
                WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(organization_id)
            .bind(agent_run_id)
            .bind(status)
            .bind(reason)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            insert_agent_job_event(
                &mut tx,
                organization_id,
                agent_run_id,
                Some(job_id),
                if status == "failed" {
                    "job.failed"
                } else {
                    "job.cancelled"
                },
                json!({ "reason": reason }),
            )
            .await?;
        }
        let count = u64::try_from(rows.len())
            .map_err(|_| AroError::Unexpected("agent reconciliation count overflow".to_string()))?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(count)
    }

    pub async fn purge_terminal_agent_run_jobs(
        &self,
        limit: i64,
        retention_days: i64,
    ) -> AroResult<u64> {
        if !(1..=10_000).contains(&limit) || !(1..=3_650).contains(&retention_days) {
            return Err(AroError::Configuration(
                "agent retention purge bounds are invalid".to_string(),
            ));
        }
        let deleted: i64 = sqlx::query_scalar(
            "SELECT aro_purge_terminal_agent_run_jobs($1::integer, $2::integer)",
        )
        .bind(i32::try_from(limit).map_err(|_| {
            AroError::Configuration("agent retention batch is too large".to_string())
        })?)
        .bind(i32::try_from(retention_days).map_err(|_| {
            AroError::Configuration("agent retention period is too large".to_string())
        })?)
        .fetch_one(self.pool())
        .await
        .map_err(map_sqlx)?;
        u64::try_from(deleted).map_err(|_| {
            AroError::Unexpected("agent retention purge returned a negative count".to_string())
        })
    }
}

async fn load_submission_by_key(
    tx: &mut Transaction<'_, Postgres>,
    context: TenantContext,
    submission_key: &str,
    request_hash: &[u8; 32],
) -> AroResult<Option<AgentRunJobSubmission>> {
    let row = sqlx::query(
        r#"
        SELECT
          j.id, j.organization_id, j.agent_run_id, j.request_hash, j.status, j.priority,
          j.max_attempts, j.max_steps, j.max_tool_calls, j.max_input_tokens,
          j.max_output_tokens, j.max_wall_time_seconds, j.attempts,
          j.steps_consumed, j.tool_calls_consumed, j.input_tokens_consumed,
          j.output_tokens_consumed, j.available_at, j.cancel_requested_at, j.pause_requested_at,
          j.started_at, j.completed_at, j.state_reason, j.last_error,
          j.result_message_id, j.created_at, j.updated_at
        FROM agent_run_jobs AS j
        JOIN agent_runs AS r
          ON r.organization_id = j.organization_id
         AND r.id = j.agent_run_id
        WHERE j.organization_id = $1
          AND j.submitted_by_user_id = $2
          AND j.submission_key = $3
          AND r.owner_user_id = $2
          AND r.deleted_at IS NULL
        FOR SHARE OF j
        "#,
    )
    .bind(context.organization_id())
    .bind(context.actor_id())
    .bind(submission_key)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored_hash = row.get::<Vec<u8>, _>("request_hash");
    if stored_hash.as_slice() != request_hash {
        return Ok(Some(AgentRunJobSubmission::Conflict));
    }
    Ok(Some(AgentRunJobSubmission::Replayed(map_agent_job_view(
        &row,
    )?)))
}

async fn load_owned_agent_job_for_update(
    tx: &mut Transaction<'_, Postgres>,
    context: TenantContext,
    run_id: Uuid,
) -> AroResult<Option<sqlx::postgres::PgRow>> {
    sqlx::query(
        r#"
        SELECT
          j.id, j.organization_id, j.agent_run_id, j.status, j.priority,
          j.max_attempts, j.max_steps, j.max_tool_calls, j.max_input_tokens,
          j.max_output_tokens, j.max_wall_time_seconds, j.attempts,
          j.steps_consumed, j.tool_calls_consumed, j.input_tokens_consumed,
          j.output_tokens_consumed, j.available_at, j.cancel_requested_at,
          j.pause_requested_at, j.started_at, j.completed_at, j.state_reason,
          j.last_error, j.result_message_id, j.created_at, j.updated_at
        FROM agent_run_jobs AS j
        JOIN agent_runs AS r
          ON r.organization_id = j.organization_id
         AND r.id = j.agent_run_id
        WHERE j.organization_id = $1
          AND j.agent_run_id = $2
          AND r.owner_user_id = $3
          AND r.deleted_at IS NULL
        FOR UPDATE OF j, r
        "#,
    )
    .bind(context.organization_id())
    .bind(run_id)
    .bind(context.actor_id())
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)
}

async fn insert_agent_job_event(
    tx: &mut Transaction<'_, Postgres>,
    organization_id: Uuid,
    agent_run_id: Uuid,
    job_id: Option<Uuid>,
    event_type: &str,
    payload: Value,
) -> AroResult<AgentRunJobEvent> {
    validate_bounded_trimmed(event_type, "agent event type", 120)?;
    validate_event_payload(&payload)?;
    let row = sqlx::query(
        r#"
        INSERT INTO agent_run_events (
          organization_id, agent_run_id, job_id, event_type, payload
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
          event_cursor, event_id, agent_run_id, job_id, event_type,
          event_version, payload, created_at
        "#,
    )
    .bind(organization_id)
    .bind(agent_run_id)
    .bind(job_id)
    .bind(event_type)
    .bind(payload)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    map_agent_job_event(&row)
}

fn map_agent_job_view(row: &sqlx::postgres::PgRow) -> AroResult<AgentRunJobView> {
    let priority = parse_priority(row.get::<String, _>("priority").as_str())?;
    Ok(AgentRunJobView {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        agent_run_id: row.get("agent_run_id"),
        status: AgentRunJobStatus::from_database(row.get::<String, _>("status").as_str())?,
        priority,
        budgets: AgentRunJobBudgets {
            max_attempts: unsigned_i16(row.get("max_attempts"), "max_attempts")?,
            max_steps: unsigned_i16(row.get("max_steps"), "max_steps")?,
            max_tool_calls: unsigned_i16(row.get("max_tool_calls"), "max_tool_calls")?,
            max_input_tokens: unsigned_i32(row.get("max_input_tokens"), "max_input_tokens")?,
            max_output_tokens: unsigned_i32(row.get("max_output_tokens"), "max_output_tokens")?,
            max_wall_time_seconds: unsigned_i32(
                row.get("max_wall_time_seconds"),
                "max_wall_time_seconds",
            )?,
        },
        attempts: unsigned_i16(row.get("attempts"), "attempts")?,
        usage: AgentRunJobUsage {
            steps: unsigned_i16(row.get("steps_consumed"), "steps_consumed")?,
            tool_calls: unsigned_i16(row.get("tool_calls_consumed"), "tool_calls_consumed")?,
            input_tokens: unsigned_i32(row.get("input_tokens_consumed"), "input_tokens_consumed")?,
            output_tokens: unsigned_i32(
                row.get("output_tokens_consumed"),
                "output_tokens_consumed",
            )?,
        },
        available_at: row.get("available_at"),
        cancel_requested_at: row.get("cancel_requested_at"),
        pause_requested_at: row.get("pause_requested_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        state_reason: row.get("state_reason"),
        last_error: row.get("last_error"),
        result_message_id: row.get("result_message_id"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_agent_job_event(row: &sqlx::postgres::PgRow) -> AroResult<AgentRunJobEvent> {
    Ok(AgentRunJobEvent {
        cursor: row.get("event_cursor"),
        event_id: row.get("event_id"),
        agent_run_id: row.get("agent_run_id"),
        job_id: row.get("job_id"),
        event_type: row.get("event_type"),
        event_version: unsigned_i16(row.get("event_version"), "event_version")?,
        payload: row.get("payload"),
        created_at: row.get("created_at"),
    })
}

fn map_claimed_agent_job(
    row: &sqlx::postgres::PgRow,
    request_snapshot: Value,
) -> AroResult<ClaimedAgentRunJob> {
    Ok(ClaimedAgentRunJob {
        lease: AgentRunJobLease {
            job_id: row.get("id"),
            organization_id: row.get("organization_id"),
            agent_run_id: row.get("agent_run_id"),
            token: row.get("lease_token"),
            generation: row.get("lease_generation"),
            expires_at: row.get("lease_expires_at"),
        },
        submitted_by_user_id: row.get("submitted_by_user_id"),
        request_snapshot,
        priority: parse_priority(row.get::<String, _>("priority").as_str())?,
        budgets: AgentRunJobBudgets {
            max_attempts: unsigned_i16(row.get("max_attempts"), "max_attempts")?,
            max_steps: unsigned_i16(row.get("max_steps"), "max_steps")?,
            max_tool_calls: unsigned_i16(row.get("max_tool_calls"), "max_tool_calls")?,
            max_input_tokens: unsigned_i32(row.get("max_input_tokens"), "max_input_tokens")?,
            max_output_tokens: unsigned_i32(row.get("max_output_tokens"), "max_output_tokens")?,
            max_wall_time_seconds: unsigned_i32(
                row.get("max_wall_time_seconds"),
                "max_wall_time_seconds",
            )?,
        },
        attempts: unsigned_i16(row.get("attempts"), "attempts")?,
        usage: AgentRunJobUsage {
            steps: unsigned_i16(row.get("steps_consumed"), "steps_consumed")?,
            tool_calls: unsigned_i16(row.get("tool_calls_consumed"), "tool_calls_consumed")?,
            input_tokens: unsigned_i32(row.get("input_tokens_consumed"), "input_tokens_consumed")?,
            output_tokens: unsigned_i32(
                row.get("output_tokens_consumed"),
                "output_tokens_consumed",
            )?,
        },
        cancel_requested_at: row.get("cancel_requested_at"),
        pause_requested_at: row.get("pause_requested_at"),
        started_at: row.get("started_at"),
    })
}

struct DatabaseAgentRunJobUsage {
    steps: i16,
    tool_calls: i16,
    input_tokens: i32,
    output_tokens: i32,
}

fn usage_as_database(usage: AgentRunJobUsage) -> AroResult<DatabaseAgentRunJobUsage> {
    Ok(DatabaseAgentRunJobUsage {
        steps: i16::try_from(usage.steps).map_err(|_| budget_conversion_error())?,
        tool_calls: i16::try_from(usage.tool_calls).map_err(|_| budget_conversion_error())?,
        input_tokens: i32::try_from(usage.input_tokens).map_err(|_| budget_conversion_error())?,
        output_tokens: i32::try_from(usage.output_tokens).map_err(|_| budget_conversion_error())?,
    })
}

async fn diagnose_agent_job_lease(
    tx: &mut Transaction<'_, Postgres>,
    lease: &AgentRunJobLease,
    usage_delta: AgentRunJobUsage,
) -> AroResult<AgentRunJobLeaseRenewal> {
    let usage = usage_as_database(usage_delta)?;
    let row = sqlx::query(
        r#"
        SELECT
          COALESCE(lease_token = $4 AND lease_generation = $5, false) AS same_attempt,
          COALESCE(lease_expires_at > now(), false) AS lease_is_live,
          cancel_requested_at IS NOT NULL AS cancellation_requested,
          pause_requested_at IS NOT NULL AS pause_requested,
          COALESCE(
            started_at + make_interval(secs => max_wall_time_seconds) <= now(),
            false
          ) AS deadline_exceeded,
          $6 > max_steps
            OR $7 > max_tool_calls
            OR $8 > max_input_tokens
            OR $9 > max_output_tokens AS budget_exceeded,
          $6 < steps_consumed
            OR $7 < tool_calls_consumed
            OR $8 < input_tokens_consumed
            OR $9 < output_tokens_consumed AS progress_regressed
        FROM agent_run_jobs
        WHERE id = $1
          AND organization_id = $2
          AND agent_run_id = $3
        "#,
    )
    .bind(lease.job_id)
    .bind(lease.organization_id)
    .bind(lease.agent_run_id)
    .bind(lease.token)
    .bind(lease.generation)
    .bind(usage.steps)
    .bind(usage.tool_calls)
    .bind(usage.input_tokens)
    .bind(usage.output_tokens)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    let Some(row) = row else {
        return Ok(AgentRunJobLeaseRenewal::LeaseLost);
    };
    let same_attempt: bool = row.get("same_attempt");
    let lease_is_live: bool = row.get("lease_is_live");
    if same_attempt && row.get::<bool, _>("cancellation_requested") {
        return Ok(AgentRunJobLeaseRenewal::CancellationRequested);
    }
    if same_attempt && row.get::<bool, _>("pause_requested") {
        return Ok(AgentRunJobLeaseRenewal::PauseRequested);
    }
    if same_attempt && row.get::<bool, _>("deadline_exceeded") {
        return Ok(AgentRunJobLeaseRenewal::DeadlineExceeded);
    }
    if same_attempt && lease_is_live && row.get::<bool, _>("budget_exceeded") {
        return Ok(AgentRunJobLeaseRenewal::BudgetExceeded);
    }
    if same_attempt && lease_is_live && row.get::<bool, _>("progress_regressed") {
        return Ok(AgentRunJobLeaseRenewal::ProgressRejected);
    }
    Ok(AgentRunJobLeaseRenewal::LeaseLost)
}

fn usage_exceeds_budget(row: &sqlx::postgres::PgRow, usage: &DatabaseAgentRunJobUsage) -> bool {
    usage.steps > row.get::<i16, _>("max_steps")
        || usage.tool_calls > row.get::<i16, _>("max_tool_calls")
        || usage.input_tokens > row.get::<i32, _>("max_input_tokens")
        || usage.output_tokens > row.get::<i32, _>("max_output_tokens")
}

fn usage_regresses(row: &sqlx::postgres::PgRow, usage: &DatabaseAgentRunJobUsage) -> bool {
    usage.steps < row.get::<i16, _>("steps_consumed")
        || usage.tool_calls < row.get::<i16, _>("tool_calls_consumed")
        || usage.input_tokens < row.get::<i32, _>("input_tokens_consumed")
        || usage.output_tokens < row.get::<i32, _>("output_tokens_consumed")
}

async fn set_trusted_worker_tenant_context(
    tx: &mut Transaction<'_, Postgres>,
    actor_id: Uuid,
    organization_id: Uuid,
) -> AroResult<()> {
    sqlx::query(
        r#"
        SELECT
          set_config('aro.actor_id', $1, true),
          set_config('aro.organization_id', $2, true)
        "#,
    )
    .bind(actor_id.to_string())
    .bind(organization_id.to_string())
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn lock_and_revalidate_agent_job_authorization(
    tx: &mut Transaction<'_, Postgres>,
    job_id: Uuid,
    organization_id: Uuid,
    actor_id: Uuid,
) -> AroResult<bool> {
    let row = sqlx::query(
        r#"
        SELECT r.lane_id, r.conversation_id, r.autonomy_profile_id
        FROM agent_run_jobs AS j
        JOIN agent_runs AS r
          ON r.organization_id = j.organization_id
         AND r.id = j.agent_run_id
         AND r.owner_user_id = j.submitted_by_user_id
         AND r.deleted_at IS NULL
        JOIN memberships AS membership
          ON membership.organization_id = j.organization_id
         AND membership.user_id = j.submitted_by_user_id
         AND membership.status = 'active'
         AND membership.deleted_at IS NULL
        JOIN users AS u
          ON u.id = membership.user_id
         AND u.deleted_at IS NULL
        JOIN organizations AS o
          ON o.id = membership.organization_id
         AND o.deleted_at IS NULL
        WHERE j.id = $1
          AND j.organization_id = $2
          AND j.submitted_by_user_id = $3
        FOR SHARE OF r, membership, u, o
        "#,
    )
    .bind(job_id)
    .bind(organization_id)
    .bind(actor_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    let Some(row) = row else {
        return Ok(false);
    };
    if let Some(lane_id) = row.get::<Option<Uuid>, _>("lane_id") {
        let lane_is_active = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT 1
            FROM agent_lanes
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND status = 'active'
              AND deleted_at IS NULL
            FOR SHARE
            "#,
        )
        .bind(lane_id)
        .bind(organization_id)
        .bind(actor_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_sqlx)?
        .is_some();
        if !lane_is_active {
            return Ok(false);
        }
    }
    if let Some(conversation_id) = row.get::<Option<Uuid>, _>("conversation_id") {
        let conversation_is_active = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT 1
            FROM conversations
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            FOR SHARE
            "#,
        )
        .bind(conversation_id)
        .bind(organization_id)
        .bind(actor_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_sqlx)?
        .is_some();
        if !conversation_is_active {
            return Ok(false);
        }
    }
    if let Some(profile_id) = row.get::<Option<Uuid>, _>("autonomy_profile_id") {
        let profile_is_active = sqlx::query_scalar::<_, i32>(
            r#"
            SELECT 1
            FROM agent_permission_profiles
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            FOR SHARE
            "#,
        )
        .bind(profile_id)
        .bind(organization_id)
        .bind(actor_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_sqlx)?
        .is_some();
        if !profile_is_active {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn cancel_agent_job_after_authorization_loss(
    tx: &mut Transaction<'_, Postgres>,
    lease: &AgentRunJobLease,
) -> AroResult<()> {
    let updated = sqlx::query(
        r#"
        UPDATE agent_run_jobs
        SET status = 'cancelled',
            cancel_requested_at = COALESCE(cancel_requested_at, now()),
            pause_requested_at = NULL,
            lease_token = NULL,
            lease_owner = NULL,
            leased_at = NULL,
            lease_expires_at = NULL,
            completed_at = now(),
            state_reason = 'authorization_revoked',
            last_error = NULL,
            updated_at = now()
        WHERE id = $1
          AND organization_id = $2
          AND agent_run_id = $3
          AND lease_token = $4
          AND lease_generation = $5
        RETURNING id
        "#,
    )
    .bind(lease.job_id)
    .bind(lease.organization_id)
    .bind(lease.agent_run_id)
    .bind(lease.token)
    .bind(lease.generation)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if updated.is_none() {
        return Ok(());
    }
    sqlx::query(
        r#"
        UPDATE agent_runs
        SET status = 'cancelled', completed_at = now(), heartbeat_at = now(),
            updated_at = now(), last_error = NULL
        WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(lease.organization_id)
    .bind(lease.agent_run_id)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    insert_agent_job_event(
        tx,
        lease.organization_id,
        lease.agent_run_id,
        Some(lease.job_id),
        "job.cancelled",
        json!({ "reason": "authorization_revoked" }),
    )
    .await?;
    Ok(())
}

async fn fail_agent_job_with_live_lease(
    tx: &mut Transaction<'_, Postgres>,
    lease: &AgentRunJobLease,
    reason: &str,
) -> AroResult<()> {
    let updated = sqlx::query(
        r#"
        UPDATE agent_run_jobs
        SET status = 'failed',
            lease_token = NULL,
            lease_owner = NULL,
            leased_at = NULL,
            lease_expires_at = NULL,
            completed_at = now(),
            state_reason = $6,
            last_error = $6,
            updated_at = now()
        WHERE id = $1
          AND organization_id = $2
          AND agent_run_id = $3
          AND lease_token = $4
          AND lease_generation = $5
        RETURNING id
        "#,
    )
    .bind(lease.job_id)
    .bind(lease.organization_id)
    .bind(lease.agent_run_id)
    .bind(lease.token)
    .bind(lease.generation)
    .bind(reason)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if updated.is_none() {
        return Ok(());
    }
    sqlx::query(
        r#"
        UPDATE agent_runs
        SET status = 'failed', completed_at = now(), heartbeat_at = now(),
            updated_at = now(), last_error = $3
        WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(lease.organization_id)
    .bind(lease.agent_run_id)
    .bind(reason)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    insert_agent_job_event(
        tx,
        lease.organization_id,
        lease.agent_run_id,
        Some(lease.job_id),
        "job.failed",
        json!({ "reason": reason }),
    )
    .await?;
    Ok(())
}

fn parse_priority(value: &str) -> AroResult<AgentRunPriority> {
    match value {
        "low" => Ok(AgentRunPriority::Low),
        "normal" => Ok(AgentRunPriority::Normal),
        "high" => Ok(AgentRunPriority::High),
        "critical" => Ok(AgentRunPriority::Critical),
        other => Err(AroError::Memory(format!(
            "unknown durable agent job priority: {other}"
        ))),
    }
}

fn unsigned_i16(value: i16, field: &str) -> AroResult<u16> {
    u16::try_from(value)
        .map_err(|_| AroError::Memory(format!("invalid negative durable agent field: {field}")))
}

fn unsigned_i32(value: i32, field: &str) -> AroResult<u32> {
    u32::try_from(value)
        .map_err(|_| AroError::Memory(format!("invalid negative durable agent field: {field}")))
}

fn validate_bounded_trimmed(value: &str, field: &str, max_bytes: usize) -> AroResult<()> {
    if value.is_empty() || value.trim() != value || value.len() > max_bytes {
        return Err(AroError::Configuration(format!(
            "{field} must be trimmed and between 1 and {max_bytes} bytes"
        )));
    }
    Ok(())
}

fn validate_safe_identifier(value: &str, field: &str, max_bytes: usize) -> AroResult<()> {
    validate_bounded_trimmed(value, field, max_bytes)?;
    if !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err(AroError::Configuration(format!(
            "{field} must contain visible ASCII only"
        )));
    }
    Ok(())
}

fn validate_event_payload(value: &Value) -> AroResult<()> {
    if !value.is_object() {
        return Err(AroError::Configuration(
            "agent event payload must be a JSON object".to_string(),
        ));
    }
    let size = serde_json::to_vec(value)
        .map_err(|error| AroError::Configuration(format!("invalid agent event: {error}")))?
        .len();
    if size > 64 * 1024 {
        return Err(AroError::Configuration(
            "agent event payload exceeds the 64 KiB limit".to_string(),
        ));
    }
    Ok(())
}

fn validate_json_object(value: &Value, field: &str) -> AroResult<()> {
    if !value.is_object() {
        return Err(AroError::Configuration(format!(
            "{field} must be a JSON object"
        )));
    }
    let size = serde_json::to_vec(value)
        .map_err(|error| AroError::Configuration(format!("invalid {field}: {error}")))?
        .len();
    if size > 1024 * 1024 {
        return Err(AroError::Configuration(format!(
            "{field} exceeds the 1 MiB limit"
        )));
    }
    Ok(())
}

fn canonical_json_bytes(value: &Value) -> AroResult<Vec<u8>> {
    fn canonicalize(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let mut entries = object.iter().collect::<Vec<_>>();
                entries.sort_by_key(|(key, _)| *key);
                let mut canonical = serde_json::Map::with_capacity(entries.len());
                for (key, value) in entries {
                    canonical.insert(key.clone(), canonicalize(value));
                }
                Value::Object(canonical)
            }
            Value::Array(values) => Value::Array(values.iter().map(canonicalize).collect()),
            primitive => primitive.clone(),
        }
    }

    serde_json::to_vec(&canonicalize(value)).map_err(|error| {
        AroError::Configuration(format!("agent request canonicalization failed: {error}"))
    })
}

fn validate_agent_snapshot_key(value: &str) -> AroResult<()> {
    if value.len() < 32 || value.len() > 4096 || value.trim() != value {
        return Err(AroError::Configuration(
            "agent snapshot encryption key must contain 32 to 4096 trimmed bytes".to_string(),
        ));
    }
    Ok(())
}

fn budget_conversion_error() -> AroError {
    AroError::Configuration("durable agent job budget conversion failed".to_string())
}

fn validate_lease_seconds(value: i64) -> AroResult<()> {
    if !(AGENT_JOB_MIN_LEASE_SECONDS..=AGENT_JOB_MAX_LEASE_SECONDS).contains(&value) {
        return Err(AroError::Configuration(format!(
            "agent job lease must be between {AGENT_JOB_MIN_LEASE_SECONDS} and {AGENT_JOB_MAX_LEASE_SECONDS} seconds"
        )));
    }
    Ok(())
}
