//! Agent plugin HTTP handlers (https://agent-plugins.org/).
//!
//! Extracted from `handlers.rs`: plugin catalog/install/lifecycle, uploaded
//! logo reads, MCP test/call bridges and skill invocation. Pure CRUD and
//! delegation — no auth/session state machines here.

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    auth::{ApiError, AuthContext},
    ApiState,
};

// =========================================================================
// AGENT PLUGINS (https://agent-plugins.org/)
// =========================================================================

#[derive(Debug, Deserialize)]
pub struct TogglePluginPayload {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallPluginMcpPayload {
    pub tool_name: String,
    #[serde(default)]
    pub arguments: Value,
}

pub async fn plugins_list_installed(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<aro_plugins::InstalledPlugin>>, ApiError> {
    state
        .store
        .ensure_org_access(auth.user_id, auth.organization_id)
        .await?;
    let list = state.plugins.list_installed().await;
    Ok(Json(list))
}

pub async fn plugins_list_marketplace(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<aro_plugins::MarketplacePlugin>>, ApiError> {
    state
        .store
        .ensure_org_access(auth.user_id, auth.organization_id)
        .await?;
    let list = state.plugins.list_marketplace().await;
    Ok(Json(list))
}

pub async fn plugins_install(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<aro_plugins::InstallPluginRequest>,
) -> Result<Json<aro_plugins::InstalledPlugin>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let installed = match payload.source {
        aro_plugins::PluginSourceType::Marketplace => {
            state
                .plugins
                .install_from_marketplace(&payload.target)
                .await?
        }
        aro_plugins::PluginSourceType::Local => {
            state
                .plugins
                .install_from_directory(std::path::Path::new(&payload.target))
                .await?
        }
        aro_plugins::PluginSourceType::Git => {
            state.plugins.install_from_git(&payload.target).await?
        }
    };
    Ok(Json(installed))
}

pub async fn plugins_install_custom(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<aro_plugins::CreateCustomPluginRequest>,
) -> Result<Json<aro_plugins::InstalledPlugin>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let installed = state.plugins.install_custom(payload).await?;
    Ok(Json(installed))
}

pub async fn plugins_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(plugin_id): Path<String>,
) -> Result<Json<aro_plugins::InstalledPlugin>, ApiError> {
    state
        .store
        .ensure_org_access(auth.user_id, auth.organization_id)
        .await?;
    let plugin = state
        .plugins
        .get_plugin(&plugin_id)
        .await
        .ok_or_else(|| ApiError::not_found(format!("Plugin '{plugin_id}' not found")))?;
    Ok(Json(plugin))
}

pub async fn plugins_toggle(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(plugin_id): Path<String>,
    Json(payload): Json<TogglePluginPayload>,
) -> Result<Json<aro_plugins::InstalledPlugin>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let updated = state
        .plugins
        .set_enabled(&plugin_id, payload.enabled)
        .await?;
    Ok(Json(updated))
}

pub async fn plugins_uninstall(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    state.plugins.uninstall(&plugin_id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

/// Read an uploaded plugin logo as `{ mime, dataUrl }` (`None` → `null`
/// JSON when the plugin carries no file logo). Read-only: any org member
/// may fetch it since logos render inside shared chat surfaces.
pub async fn plugins_read_logo(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(plugin_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .ensure_org_access(auth.user_id, auth.organization_id)
        .await?;
    let data_url = state.plugins.read_plugin_logo_data_url(&plugin_id).await?;
    match data_url {
        Some(data_url) => {
            let mime = data_url
                .split(';')
                .next()
                .unwrap_or("data:image/png")
                .trim_start_matches("data:");
            Ok(Json(serde_json::json!({ "mime": mime, "dataUrl": data_url })))
        }
        None => Ok(Json(Value::Null)),
    }
}

pub async fn plugins_mcp_test(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path((plugin_id, server_name)): Path<(String, String)>,
) -> Result<Json<Vec<aro_mcp::McpTool>>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let tools = state
        .plugins
        .test_mcp_server(&plugin_id, &server_name)
        .await?;
    Ok(Json(tools))
}

pub async fn plugins_mcp_call(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path((plugin_id, server_name)): Path<(String, String)>,
    Json(payload): Json<CallPluginMcpPayload>,
) -> Result<Json<aro_mcp::CallToolResult>, ApiError> {
    state
        .store
        .ensure_org_access(auth.user_id, auth.organization_id)
        .await?;
    let result = state
        .plugins
        .call_mcp_tool(
            &plugin_id,
            &server_name,
            &payload.tool_name,
            payload.arguments,
        )
        .await?;
    Ok(Json(result))
}

pub async fn plugins_skill_invoke(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path((plugin_id, skill_id)): Path<(String, String)>,
    Json(payload): Json<Value>,
) -> Result<Json<aro_skills::SkillOutput>, ApiError> {
    state
        .store
        .ensure_org_access(auth.user_id, auth.organization_id)
        .await?;
    let output = state
        .plugins
        .invoke_skill(&plugin_id, &skill_id, &payload)
        .await?;
    Ok(Json(output))
}
