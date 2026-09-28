use aro_core::{
    AgentMemoryContext, AgentMessageEnvelope, AgentMessagePayload, AgentParticipant,
    AgentRunPriority, AgentRunStartRequest, AgentRunView, AssistantMode,
};
use chrono::Utc;
use tauri::State;
use uuid::Uuid;

use crate::state::AppState;
use crate::CommandResult;

#[tauri::command]
pub async fn agent_get_memory(
    state: State<'_, AppState>,
    agent_id: String,
    conversation_id: String,
) -> CommandResult<AgentMemoryContext> {
    if agent_id.trim().is_empty() || conversation_id.trim().is_empty() {
        return Ok(AgentMemoryContext::new(
            conversation_id,
            agent_id,
            "Sous-Agent",
            "worker",
        ));
    }

    let local = state
        .engine
        .memory_store()
        .get_agent_memory(&conversation_id, &agent_id)
        .map_err(|e| e.to_string())?;

    if let Some(mem) = local {
        return Ok(mem);
    }

    Ok(AgentMemoryContext::new(
        conversation_id,
        agent_id,
        "Sous-Agent",
        "worker",
    ))
}

#[tauri::command]
pub async fn agent_save_memory(
    state: State<'_, AppState>,
    memory: AgentMemoryContext,
) -> CommandResult<()> {
    if memory.agent_id.trim().is_empty() || memory.conversation_id.trim().is_empty() {
        return Err("agentId and conversationId are required".to_string());
    }

    state
        .engine
        .memory_store()
        .save_agent_memory(&memory)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn agent_dispatch_directive(
    state: State<'_, AppState>,
    agent_id: String,
    directive: String,
    conversation_id: String,
) -> CommandResult<AgentRunView> {
    if agent_id.trim().is_empty()
        || directive.trim().is_empty()
        || conversation_id.trim().is_empty()
    {
        return Err("Missing required arguments".to_string());
    }

    let existing_memory = state
        .engine
        .memory_store()
        .get_agent_memory(&conversation_id, &agent_id)
        .map_err(|e| e.to_string())?;

    let (agent_name, role, autonomy_profile_id) = if let Some(ref mem) = existing_memory {
        let prof_id = mem
            .permission_profile_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok());
        (mem.agent_name.clone(), mem.role.clone(), prof_id)
    } else {
        ("Autonomous Worker".to_string(), "worker".to_string(), None)
    };

    let envelope = AgentMessageEnvelope {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.clone(),
        parent_message_id: None,
        correlation_id: None,
        sender: AgentParticipant::orchestrator("orchestrator", "Aro Orchestrator"),
        recipient: AgentParticipant::subagent(&agent_id, &agent_name, &role, None),
        message_type: aro_core::AgentMessageType::TaskDelegation,
        payload: AgentMessagePayload::text(&directive),
        permission_profile_id: autonomy_profile_id.map(|id| id.to_string()),
        priority: Some(AgentRunPriority::Normal),
        timestamp: Utc::now(),
    };

    state
        .engine
        .memory_store()
        .record_agent_envelope(&envelope)
        .map_err(|e| e.to_string())?;

    let parsed_conv_id = Uuid::parse_str(&conversation_id).ok();
    let request = AgentRunStartRequest {
        run_id: None,
        lane_id: None,
        conversation_id: parsed_conv_id,
        goal: directive.clone(),
        mode: AssistantMode::Code,
        system_prompt: None,
        model_id: None,
        provider: None,
        autonomy_profile_id,
        priority: Some(AgentRunPriority::Normal),
        max_steps: None,
    };

    let view = state
        .engine
        .start_agent_run(request.clone())
        .await
        .map_err(|e| e.to_string())?;

    if let Ok(Some(session)) = state.refresh_cloud_session_from_keyring().await {
        let mut cloud_request = request;
        cloud_request.run_id = Some(view.run.id);
        cloud_request.conversation_id = view.run.conversation_id;
        let _ = state
            .cloud
            .create_agent_run(&session.access_token, &cloud_request)
            .await;
    }

    Ok(view)
}
