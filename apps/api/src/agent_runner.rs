use aro_agent::{AgentRuntime, EnvironmentSnapshot};
use aro_core::{AgentActionType, ChatMessage, MessageRole, ModelResponseFormat};
use aro_runtime::{ModelProvider, ModelRouter};
use aro_store::{
    AgentRunJobCompletion, AgentRunJobLeaseRenewal, AgentRunJobUsage, AgentSnapshotKeyring,
    AroStore, TenantContext,
};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub async fn process_agent_jobs(
    store: &AroStore,
    keyring: &Arc<AgentSnapshotKeyring>,
) -> anyhow::Result<()> {
    let worker_id = format!("api-worker-{}", Uuid::new_v4());
    let lease_seconds = 60;

    // Claim and execute jobs until the queue is empty
    while let Some(claimed) = store
        .claim_agent_run_job(&worker_id, lease_seconds, keyring)
        .await?
    {
        let store_clone = store.clone();
        let keyring_clone = keyring.clone();
        tokio::spawn(async move {
            if let Err(err) = run_agent_job(store_clone, claimed, keyring_clone).await {
                tracing::error!(?err, "error executing claimed agent job");
            }
        });
    }
    Ok(())
}

async fn run_agent_job(
    store: AroStore,
    claimed: aro_store::ClaimedAgentRunJob,
    _keyring: Arc<AgentSnapshotKeyring>,
) -> anyhow::Result<()> {
    let lease = claimed.lease.clone();
    let run_id = lease.agent_run_id();
    let org_id = lease.organization_id();
    let job_id = lease.job_id();
    let user_id = claimed.submitted_by_user_id;

    tracing::info!(
        ?run_id,
        ?job_id,
        "starting background execution of agent job"
    );

    // 1. Periodic lease renewal loop
    let store_clone = store.clone();
    let lease_clone = lease.clone();
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let renewer_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(15));
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                _ = interval.tick() => {
                    let usage = AgentRunJobUsage::default();
                    match store_clone.renew_agent_run_job_lease(&lease_clone, 60, usage).await {
                        Ok(AgentRunJobLeaseRenewal::CancellationRequested) => {
                            tracing::info!(?run_id, "lease renewal detected cancellation request");
                            break;
                        }
                        Ok(AgentRunJobLeaseRenewal::PauseRequested) => {
                            tracing::info!(?run_id, "lease renewal detected pause request");
                            break;
                        }
                        Err(err) => {
                            tracing::warn!(?err, ?run_id, "failed to renew agent job lease");
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    let outcome = async {
        // 2. Fetch run settings and configuration
        let tenant = TenantContext::new(user_id, org_id)?;
        let app_settings = store.get_app_settings(user_id, org_id).await?;

        let run = store
            .get_agent_run(user_id, org_id, run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("agent run not found"))?;

        // 3. Setup Model Router & Provider. Pas de repli silencieux vers
        // un faux "mock-model" : un run sans modèle configuré échoue
        // explicitement au lieu de produire une fausse réussite.
        let model_id = match run.model_id.clone() {
            Some(id) if !id.trim().is_empty() => id,
            _ => {
                store
                    .fail_agent_run_job(&lease, "model_not_configured")
                    .await?;
                tracing::warn!(?run_id, "agent job has no model configured");
                return Ok(());
            }
        };
        let provider_id = match run.model_provider_id.clone() {
            Some(id) if !id.trim().is_empty() => id,
            _ => {
                store
                    .fail_agent_run_job(&lease, "model_not_configured")
                    .await?;
                tracing::warn!(?run_id, "agent job has no model provider configured");
                return Ok(());
            }
        };

        // Le kind est résolu depuis les connections connues, pas deviné.
        let connection = match app_settings.model.connection(&provider_id).cloned() {
            Some(connection) => connection,
            None => {
                store
                    .fail_agent_run_job(&lease, "model_provider_unknown")
                    .await?;
                tracing::warn!(
                    ?run_id,
                    provider_id,
                    "agent job references an unknown model provider"
                );
                return Ok(());
            }
        };
        let model_ref = aro_core::ModelRef::new(
            provider_id.clone(),
            connection.kind.clone(),
            model_id.clone(),
            model_id.clone(),
        );

        let router = match ModelRouter::from_model_ref(&app_settings.model, &model_ref, None) {
            Ok(router) => router,
            Err(err) => {
                store
                    .fail_agent_run_job(&lease, "model_router_unavailable")
                    .await?;
                tracing::warn!(?err, ?run_id, "agent job model router unavailable");
                return Ok(());
            }
        };

        // 4. Execution Loop (unlimited steps: runs until final/pause or interruption)
        let max_steps_budget = claimed.budgets.max_steps as u32;
        let mut next_sequence = 1;

        // If resuming, read previous steps
        let existing_steps = store.list_agent_steps(user_id, org_id, run_id).await?;
        if !existing_steps.is_empty() {
            next_sequence = existing_steps.iter().map(|s| s.sequence).max().unwrap_or(0) + 1;
        }

        let runtime = AgentRuntime::new();
        let mut history = if let Some(conv_id) = run.conversation_id {
            store
                .list_messages(tenant, conv_id)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let context_pack = runtime.build_context_pack(
            &run,
            &history,
            &[],
            &[],
            EnvironmentSnapshot::api_durable(Some(provider_id.clone()), Some(model_id.clone())),
        );

        let mut completed_content = String::new();
        let mut total_tokens: u32 = 0;
        let mut steps_executed: u32 = 0;
        let mut tool_calls_executed: u32 = 0;

        while steps_executed < max_steps_budget {
            let step_idx = next_sequence;
            // Check for pause/cancellation
            let current_job = store.get_agent_run_job(tenant, job_id).await?;
            let interrupted = current_job.as_ref().is_some_and(|job| {
                job.cancel_requested_at.is_some()
                    || job.status == aro_store::AgentRunJobStatus::Cancelled
                    || job.pause_requested_at.is_some()
                    || job.status == aro_store::AgentRunJobStatus::Waiting
            });
            if interrupted {
                tracing::info!(?run_id, "agent execution interrupted before the next step");
                break;
            }

            let system_prompt = format!(
                "You are an autonomous AI agent fulfilling the goal: {}",
                run.goal
            );

            let req = runtime.model_request(
                &run,
                &context_pack,
                system_prompt,
                history.clone(),
                format!(
                    "Goal: {}. Plan and execute next step {}.",
                    run.goal, step_idx
                ),
                0.7,
                1024,
                ModelResponseFormat::AgentActionJson,
            );

            let gen = match router.generate(req).await {
                Ok(gen) => gen,
                Err(err) => {
                    store
                        .fail_agent_run_job(&lease, "model_generation_failed")
                        .await?;
                    tracing::warn!(?err, ?run_id, "agent job model generation failed");
                    return Ok(());
                }
            };

            let tokens = gen.token_estimate.unwrap_or(50);
            total_tokens += tokens;
            steps_executed += 1;

            let action = runtime.parse_model_action(&gen.content);
            let validation_err = runtime.validate_action(&action, &context_pack).err();

            let step = runtime.model_step(
                &run,
                step_idx,
                &gen.content,
                &action,
                validation_err.as_deref(),
            );
            store.add_agent_step(user_id, org_id, &step).await?;
            next_sequence += 1;

            if let Some(err_msg) = validation_err {
                store
                    .fail_agent_run_job(&lease, &format!("action_validation_failed: {err_msg}"))
                    .await?;
                return Ok(());
            }

            match action.action_type {
                AgentActionType::Final => {
                    completed_content = action.content.unwrap_or(gen.content);
                    break;
                }
                AgentActionType::Pause => {
                    completed_content = action.reason.unwrap_or_else(|| "Agent paused".to_string());
                    break;
                }
                AgentActionType::Tool => {
                    tool_calls_executed += 1;
                    let tool_step = runtime.tool_requested_step(&run, next_sequence, &action);
                    store.add_agent_step(user_id, org_id, &tool_step).await?;
                    next_sequence += 1;

                    let conv_id = run.conversation_id.unwrap_or(run.id);
                    let tool_name = action.tool_id.as_deref().unwrap_or("unknown");
                    history.push(ChatMessage::new(
                        conv_id,
                        MessageRole::Assistant,
                        format!("Requested tool `{tool_name}` with input: {}", action.input),
                    ));

                    completed_content = format!("Tool `{tool_name}` requested");
                }
            }
        }

        // 5. Complete agent run job with token tracking
        let usage = AgentRunJobUsage {
            steps: steps_executed.max(1).min(u16::MAX as u32) as u16,
            tool_calls: tool_calls_executed.min(u16::MAX as u32) as u16,
            input_tokens: total_tokens,
            output_tokens: total_tokens,
        };
        let completion = AgentRunJobCompletion {
            content: completed_content,
            model_id: Some(model_id),
            checkpoint_summary: Some(format!("Completed execution after {steps_executed} steps")),
            token_estimate: Some(total_tokens),
            usage,
        };
        store.complete_agent_run_job(&lease, completion).await?;
        tracing::info!(
            ?run_id,
            steps = steps_executed,
            "completed agent job execution successfully"
        );

        Ok(())
    };

    let outcome_result = outcome.await;

    // Cleanup renewal task
    let _ = shutdown_tx.send(());
    let _ = renewer_task.await;

    outcome_result
}
