use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use aro_agent::{built_in_tool_descriptors, AgentRuntime, EnvironmentSnapshot};
use aro_core::{
    AgentContextItem, AgentLaneStatus, AgentLaneView, AgentRun, AgentRunPriority,
    AgentRunStartRequest, AgentRunStatus, AgentRunView, AgentStep, AgentStepKind, AgentStepStatus,
    AppSettings, AssistantMode, AttachmentRef, AuthSession, ChatMessage, ContextSource,
    Conversation, FileObject, FileStatus, FileUploadSession, Folder, LongTermMemory,
    MembershipRole, MessageAttachment, MessageRole, ModelGenerationRequest, ModelProviderKind,
    ModelResponseFormat, ModelSettings, OrganizationInvitation, PermissionProfile, Project,
    RuntimeStatus, ToolDescriptor, ToolExecutionRequest, ToolExecutionResult, ToolExecutionStatus,
    ToolStatus, WebAccessMode, WebFetchRequest, WebSearchRequest, TOOL_CORE_SEARCH_WEB,
    TOOL_CORE_WEB_PAGE_READ,
};
use aro_files::{mime_matches_declared, sha256_hex, sniff_mime, storage_key};
use aro_integrations::{
    AuthMethod, AuthorizationContext, ConnectorErrorCode, OAuthClientProfile, OAuthProtocolEngine,
    OAuthProviderConfig, OwnerRef, OwnerType,
};
use aro_policy::{
    AuditLevel, AuthorizationEvidence, AuthorizationRequest as PolicyAuthorizationRequest,
    DataClassification, DecisionConstraints, EvaluationContext, PolicyEffect, PolicyEngine,
    PolicyLayer, PolicyRule, PolicyTarget, ResourceRef, RiskLevel, SubjectKind, SubjectRef,
};
use aro_runtime::{LocalModelProvider, ModelProvider, ModelRouter};
use aro_secrets::{AuthorizationAttemptAad, CredentialAad, CredentialEnvelope};
use aro_store::{
    hash_secret, ConnectorClientProfileUpsert, IdempotencyBegin, IdempotencyCompletion,
    IdempotencyRequestHash, IdempotencyRequestKey, IdempotencyResponse, InstallationPatchV3,
    NewApiKeyIntegration, NewAuthorizationAttempt, NewAuthorizedInstallation,
    NewCredentialInstallation, NewFileUpload, NewIntegrationOAuthState, NewOAuthIntegration,
    NewOrganizationMember, NewUserWithOrg, OrganizationPatch, PersistedCollection, TenantContext,
    UserPreferencesPatch, UserProfilePatch, IDEMPOTENCY_DEFAULT_TTL_SECONDS,
};
use aro_tools::{extract_urls, result_context_items, web_request_likely, WebAccessPolicy};
use aro_vector::{memories_to_context_sources, MemoryVectorScope};
use axum::{
    body::{Body, Bytes},
    extract::{Form, Path, Query, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::{
        sse::{Event, Sse},
        Html, IntoResponse, Response,
    },
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures_util::{stream, Stream};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, convert::Infallible, time::Duration};
use url::Url;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    auth::{generate_refresh_token, ApiError, AuthContext},
    ApiState,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    status: &'static str,
}

pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

pub async fn live() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn acquire_required_lock(
    state: &ApiState,
    key: String,
    ttl_seconds: u64,
    conflict_message: &'static str,
) -> Result<crate::redis::RedisLock, ApiError> {
    state
        .redis
        .acquire_lock(&key, ttl_seconds)
        .await?
        .ok_or_else(|| ApiError::conflict(conflict_message))
}

async fn release_redis_lock(lock: crate::redis::RedisLock) {
    if let Err(err) = lock.release().await {
        tracing::warn!(error = %err, "failed to release Redis lock");
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadyResponse {
    status: &'static str,
    database: &'static str,
    migrations: &'static str,
    redis: &'static str,
    secrets: &'static str,
}

pub async fn ready(State(state): State<ApiState>) -> Result<Json<ReadyResponse>, ApiError> {
    state.store.ping().await?;
    state.store.ensure_migrations_ready().await?;
    let redis = state.redis.ping().await?;
    if state.deployment.environment == "production"
        && state.integration_flags.api_v3
        && !state.envelope_crypto.production_ready()
    {
        return Err(ApiError::internal(
            "integration secret provider is not production-ready",
        ));
    }
    Ok(Json(ReadyResponse {
        status: "ready",
        database: "ok",
        migrations: "ok",
        redis: redis.as_str(),
        secrets: if state.envelope_crypto.production_ready() {
            "ok"
        } else {
            "development"
        },
    }))
}

pub async fn metrics(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<String, ApiError> {
    if !metrics_request_authorized(&state, &headers) {
        return Err(ApiError::not_found("metrics endpoint is unavailable"));
    }
    let uptime_seconds = (Utc::now() - state.started_at).num_seconds().max(0);
    let db_ready = i32::from(state.store.ping().await.is_ok());
    let migrations_ready = i32::from(state.store.ensure_migrations_ready().await.is_ok());
    let redis_configured = i32::from(state.redis.configured());
    let redis_ready = i32::from(
        state
            .redis
            .ping()
            .await
            .is_ok_and(|health| matches!(health, crate::redis::RedisHealth::Ok)),
    );
    let rate_limit_enabled = i32::from(state.redis.rate_limit_enabled());
    let trust_proxy_headers = i32::from(state.trust_proxy_headers);
    let agent_durable_execution = i32::from(state.agent_durable_execution_enabled);
    let agent_web_access = i32::from(state.agent_web_access_enabled);
    Ok(format!(
        "# HELP aro_process_uptime_seconds Seconds since this API process started.\n\
         # TYPE aro_process_uptime_seconds gauge\n\
         aro_process_uptime_seconds {uptime_seconds}\n\
         # HELP aro_db_ready Whether the API can reach PostgreSQL.\n\
         # TYPE aro_db_ready gauge\n\
         aro_db_ready {db_ready}\n\
         # HELP aro_migrations_ready Whether all embedded SQLx migrations are applied.\n\
         # TYPE aro_migrations_ready gauge\n\
         aro_migrations_ready {migrations_ready}\n\
         # HELP aro_redis_configured Whether ARO_REDIS_URL is configured.\n\
         # TYPE aro_redis_configured gauge\n\
         aro_redis_configured {redis_configured}\n\
         # HELP aro_redis_ready Whether the API can reach configured Redis.\n\
         # TYPE aro_redis_ready gauge\n\
         aro_redis_ready {redis_ready}\n\
         # HELP aro_rate_limit_enabled Whether Redis-backed API rate limiting is enabled.\n\
         # TYPE aro_rate_limit_enabled gauge\n\
         aro_rate_limit_enabled {rate_limit_enabled}\n\
         # HELP aro_api_trust_proxy_headers Trust proxy headers status\n\
         # TYPE aro_api_trust_proxy_headers gauge\n\
         aro_api_trust_proxy_headers {trust_proxy_headers}\n\
         # HELP aro_api_agent_durable_execution Durable execution flag\n\
         # TYPE aro_api_agent_durable_execution gauge\n\
         aro_api_agent_durable_execution {agent_durable_execution}\n\
         # HELP aro_api_agent_web_access Agent web access flag\n\
         # TYPE aro_api_agent_web_access gauge\n\
         aro_api_agent_web_access {agent_web_access}\n"
    ))
}

fn metrics_request_authorized(state: &ApiState, headers: &HeaderMap) -> bool {
    let Some(expected_hash) = state.metrics_token_hash.as_deref() else {
        // Local development may expose the endpoint without a token. Production
        // startup requires one, so this branch cannot expose production metrics.
        return state.deployment.environment != "production";
    };
    let supplied = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    supplied.is_some_and(|token| hash_secret(token) == expected_hash)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
    pub organization_name: String,
    pub organization_domain: Option<String>,
    pub organization_description: Option<String>,
}

pub async fn auth_register(
    State(state): State<ApiState>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    if !state.deployment.public_signup_enabled() {
        return Err(ApiError::bad_request("public signup is disabled"));
    }
    if request.email.trim().is_empty() || request.password.len() < 10 {
        return Err(ApiError::bad_request(
            "email is required and password must be at least 10 characters",
        ));
    }

    let password_hash = hash_password(&request.password)?;
    let principal = state
        .store
        .create_user_with_org(NewUserWithOrg {
            email: request.email,
            name: request.name,
            role_title: None,
            avatar_color: None,
            password_hash,
            organization_name: request.organization_name,
            organization_domain: request.organization_domain,
            organization_description: request.organization_description,
        })
        .await?;

    issue_session(&state, principal).await.map(Json)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub async fn auth_login(
    State(state): State<ApiState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    let credentials = state
        .store
        .find_user_credentials(&request.email)
        .await?
        .ok_or_else(|| ApiError::unauthorized("invalid email or password"))?;
    verify_password(&request.password, &credentials.password_hash)?;
    let principal = state
        .store
        .principal_for_user(credentials.user.id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("user has no active organization"))?;
    issue_session(&state, principal).await.map(Json)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetConfirm {
    pub token: String,
    pub new_password: String,
}

pub async fn auth_password_reset_request(
    State(state): State<ApiState>,
    Json(payload): Json<PasswordResetRequest>,
) -> Result<Json<Value>, ApiError> {
    let email = payload.email.trim().to_lowercase();
    if email.is_empty() {
        return Err(ApiError::bad_request("email cannot be empty"));
    }

    if let Some(user_creds) = state.store.find_user_credentials(&email).await? {
        let reset_token = Uuid::new_v4().to_string();
        let token_hash = sha256_hex(reset_token.as_bytes());
        let expires_at = Utc::now()
            + chrono::Duration::try_minutes(30).unwrap_or_else(|| chrono::Duration::hours(1));

        state
            .store
            .create_password_reset_token(user_creds.user.id, &token_hash, expires_at)
            .await?;

        tracing::info!(user_id = ?user_creds.user.id, "password reset token generated");
    }

    Ok(Json(json!({
        "status": "success",
        "message": "If the account exists, a password reset token has been processed."
    })))
}

pub async fn auth_password_reset_confirm(
    State(state): State<ApiState>,
    Json(payload): Json<PasswordResetConfirm>,
) -> Result<Json<Value>, ApiError> {
    if payload.token.trim().is_empty() {
        return Err(ApiError::bad_request("token cannot be empty"));
    }
    if payload.new_password.len() < 10 {
        return Err(ApiError::bad_request(
            "password must be at least 10 characters",
        ));
    }

    let token_hash = sha256_hex(payload.token.trim().as_bytes());
    let (user_id, _reset_token_id) = state
        .store
        .consume_password_reset_token(&token_hash)
        .await?
        .ok_or_else(|| ApiError::bad_request("invalid or expired password reset token"))?;

    let new_hash = hash_password(&payload.new_password)?;
    state.store.update_user_password(user_id, &new_hash).await?;

    tracing::info!(?user_id, "password successfully reset");

    Ok(Json(json!({
        "status": "success",
        "message": "Password updated successfully."
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpEnableRequest {
    pub code: String,
}

fn generate_base32_secret(len: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rand::Rng::gen_range(&mut rng, 0..ALPHABET.len());
            ALPHABET[idx] as char
        })
        .collect()
}

pub async fn auth_mfa_totp_setup(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    let user_info = state
        .store
        .principal_for_user(auth.user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("user not found"))?;

    let base32_secret = generate_base32_secret(32);
    state
        .store
        .update_user_totp_secret(auth.user_id, &base32_secret)
        .await?;

    let otpauth_url = format!(
        "otpauth://totp/ARO:{}?secret={}&issuer=ARO",
        user_info.user.email, base32_secret
    );

    Ok(Json(json!({
        "secret": base32_secret,
        "otpauthUrl": otpauth_url,
        "enabled": false
    })))
}

pub async fn auth_mfa_totp_enable(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<TotpEnableRequest>,
) -> Result<Json<Value>, ApiError> {
    let (secret_opt, _enabled_at) = state
        .store
        .get_user_totp_info(auth.user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("user not found"))?;

    let Some(secret) = secret_opt else {
        return Err(ApiError::bad_request("TOTP setup has not been initiated"));
    };

    // Preuve de possession exigée : un code valide généré par le secret.
    // Sans ceci, n'importe quel code à 6 caractères activait le MFA.
    if !crate::auth::verify_totp_code(&secret, &payload.code, Utc::now().timestamp()) {
        return Err(ApiError::bad_request("invalid MFA code"));
    }

    state.store.enable_user_totp(auth.user_id).await?;

    Ok(Json(json!({
        "status": "success",
        "message": "TOTP MFA enabled successfully."
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpDisableRequest {
    /// Code TOTP courant : exigé quand le MFA est actif, pour qu'un simple
    /// vol de session ne suffise pas à le désactiver.
    #[serde(default)]
    pub code: Option<String>,
}

pub async fn auth_mfa_totp_disable(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<TotpDisableRequest>,
) -> Result<Json<Value>, ApiError> {
    let (secret_opt, enabled_at) = state
        .store
        .get_user_totp_info(auth.user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("user not found"))?;

    if enabled_at.is_some() {
        let secret = secret_opt
            .ok_or_else(|| ApiError::internal("TOTP is enabled without a stored secret"))?;
        let code = payload
            .code
            .as_deref()
            .ok_or_else(|| ApiError::bad_request("current MFA code is required to disable TOTP"))?;
        if !crate::auth::verify_totp_code(&secret, code, Utc::now().timestamp()) {
            return Err(ApiError::bad_request("invalid MFA code"));
        }
    }

    state.store.disable_user_totp(auth.user_id).await?;
    Ok(Json(json!({
        "status": "success",
        "message": "TOTP MFA disabled successfully."
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptNewInvitationRequest {
    pub token: String,
    pub password: String,
}

/// This public endpoint is only for accounts that were created by an invitation. Existing users
/// must authenticate first and use the authenticated acceptance endpoint below.
pub async fn auth_accept_new_invitation(
    State(state): State<ApiState>,
    Json(request): Json<AcceptNewInvitationRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    if request.token.len() < 32 || request.password.len() < 10 {
        return Err(ApiError::bad_request(
            "invitation token is invalid or password is too short",
        ));
    }
    let password_hash = hash_password(&request.password)?;
    let (user_id, organization_id) = state
        .store
        .accept_new_organization_invitation(&request.token, password_hash)
        .await?;
    let principal = state
        .store
        .principal_for_user_in_org(user_id, organization_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("invitation could not be activated"))?;
    issue_session(&state, principal).await.map(Json)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptInvitationRequest {
    pub token: String,
    pub email: String,
    pub password: String,
}

/// Unified invitation onboarding contract used by clients. The server chooses the safe path:
/// create a new identity only when none exists, otherwise require that identity's password.
pub async fn auth_accept_invitation(
    State(state): State<ApiState>,
    Json(request): Json<AcceptInvitationRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    if request.token.len() < 32 || request.email.trim().is_empty() || request.password.len() < 10 {
        return Err(ApiError::unauthorized(
            "invalid account credentials or invitation",
        ));
    }
    let (user_id, organization_id) =
        if let Some(credentials) = state.store.find_user_credentials(&request.email).await? {
            verify_password(&request.password, &credentials.password_hash)
                .map_err(|_| ApiError::unauthorized("invalid account credentials or invitation"))?;
            let organization_id = state
                .store
                .accept_existing_invitation_after_primary_auth(credentials.user.id, &request.token)
                .await
                .map_err(map_invitation_auth_error)?;
            (credentials.user.id, organization_id)
        } else {
            let password_hash = hash_password(&request.password)?;
            state
                .store
                .accept_new_organization_invitation_for_email(
                    &request.token,
                    &request.email,
                    password_hash,
                )
                .await
                .map_err(map_invitation_auth_error)?
        };
    let principal = state
        .store
        .principal_for_user_in_org(user_id, organization_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("invitation could not be activated"))?;
    issue_session(&state, principal).await.map(Json)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptExistingInvitationRequest {
    pub token: String,
    pub refresh_token: String,
}

pub async fn auth_accept_existing_invitation(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<AcceptExistingInvitationRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    if request.token.len() < 32 {
        return Err(ApiError::bad_request("invitation token is invalid"));
    }
    if request.refresh_token.len() < 32 {
        return Err(ApiError::unauthorized("invalid refresh token"));
    }
    let successor = generate_refresh_token();
    let rotated = state
        .store
        .accept_existing_organization_invitation(
            auth.user_id,
            &request.token,
            &request.refresh_token,
            &successor,
            state.refresh_token_days,
        )
        .await
        .map_err(|error| match error {
            aro_core::AroError::Security(_) => ApiError::unauthorized("invalid refresh token"),
            other => other.into(),
        })?;
    let principal = state
        .store
        .principal_for_user_in_org(auth.user_id, rotated.organization_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("invitation could not be activated"))?;
    build_session_with_refresh(&state, principal, successor).map(Json)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptExistingInvitationWithPasswordRequest {
    pub token: String,
    pub email: String,
    pub password: String,
}

/// Account-level recovery path for an existing account that has no active organization and thus
/// cannot hold a tenant-scoped access/refresh session yet.
pub async fn auth_accept_existing_invitation_with_password(
    State(state): State<ApiState>,
    Json(request): Json<AcceptExistingInvitationWithPasswordRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    if request.token.len() < 32 || request.email.trim().is_empty() || request.password.len() < 10 {
        return Err(ApiError::unauthorized(
            "invalid account credentials or invitation",
        ));
    }
    let credentials = state
        .store
        .find_user_credentials(&request.email)
        .await?
        .ok_or_else(|| ApiError::unauthorized("invalid account credentials or invitation"))?;
    verify_password(&request.password, &credentials.password_hash)
        .map_err(|_| ApiError::unauthorized("invalid account credentials or invitation"))?;
    let organization_id = state
        .store
        .accept_existing_invitation_after_primary_auth(credentials.user.id, &request.token)
        .await
        .map_err(map_invitation_auth_error)?;
    let principal = state
        .store
        .principal_for_user_in_org(credentials.user.id, organization_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("invitation could not be activated"))?;
    issue_session(&state, principal).await.map(Json)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

pub async fn auth_refresh(
    State(state): State<ApiState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    let successor = generate_refresh_token();
    let rotated = match state
        .store
        .rotate_refresh_token(
            &request.refresh_token,
            &successor,
            None,
            None,
            state.refresh_token_days,
        )
        .await
    {
        Ok(Some(rotated)) => rotated,
        Ok(None) | Err(aro_core::AroError::Security(_)) => {
            return Err(ApiError::unauthorized("invalid refresh token"));
        }
        Err(error) => return Err(error.into()),
    };
    let principal = state
        .store
        .principal_for_user_in_org(rotated.user_id, rotated.organization_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("user has no active organization"))?;
    build_session_with_refresh(&state, principal, successor).map(Json)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchOrganizationRequest {
    pub organization_id: Uuid,
    pub refresh_token: String,
}

pub async fn auth_switch_organization(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<SwitchOrganizationRequest>,
) -> Result<Json<AuthSession>, ApiError> {
    if request.refresh_token.len() < 32 {
        return Err(ApiError::unauthorized("invalid refresh token"));
    }
    let principal = state
        .store
        .principal_for_user_in_org(auth.user_id, request.organization_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("organization access denied"))?;
    rotate_session(
        &state,
        principal,
        &request.refresh_token,
        Some(auth.user_id),
    )
    .await
    .map(Json)
}

pub async fn auth_logout(
    State(state): State<ApiState>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .revoke_refresh_token(&request.refresh_token)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn bootstrap(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<aro_core::BootstrapPayloadV2>, ApiError> {
    let pending_runtime = RuntimeStatus::unavailable(
        aro_core::ModelProviderKind::Mock,
        "pending".to_string(),
        None,
        "cloud API is reachable; local runtime is checked by the desktop client",
    );
    let mut payload = state
        .store
        .bootstrap(
            auth.tenant_context(),
            pending_runtime,
            Some(&state.secrets_key),
        )
        .await?;
    payload.runtime = RuntimeStatus::unavailable(
        payload.settings.model.provider.clone(),
        payload.settings.model.model_id.clone(),
        None,
        "cloud API is reachable; local runtime is checked by the desktop client",
    );
    Ok(Json(payload))
}

pub async fn organizations_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<aro_core::Organization>>, ApiError> {
    let principal = state
        .store
        .principal_for_user(auth.user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("session user not found"))?;
    let mut organizations = Vec::new();
    for membership in principal
        .memberships
        .into_iter()
        .filter(|membership| membership.status == aro_core::MembershipStatus::Active)
    {
        if let Some(org) = state
            .store
            .get_organization(membership.organization_id)
            .await?
        {
            organizations.push(org);
        }
    }
    Ok(Json(organizations))
}

pub async fn user_profile_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<UserProfilePatch>,
) -> Result<Json<aro_core::User>, ApiError> {
    Ok(Json(
        state
            .store
            .update_user_profile(auth.user_id, request)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationCreateRequest {
    pub name: String,
    pub domain: Option<String>,
    pub description: Option<String>,
}

pub async fn organization_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<OrganizationCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if request.name.trim().is_empty() {
        return Err(ApiError::bad_request("organization name is required"));
    }
    let (organization, membership) = state
        .store
        .create_organization_for_user(
            auth.user_id,
            request.name,
            request.domain,
            request.description,
        )
        .await?;
    Ok(Json(json!({
        "organization": organization,
        "membership": membership
    })))
}

pub async fn organization_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(organization_id): Path<Uuid>,
    Json(request): Json<OrganizationPatch>,
) -> Result<Json<aro_core::Organization>, ApiError> {
    Ok(Json(
        state
            .store
            .update_organization(auth.user_id, organization_id, request)
            .await?,
    ))
}

pub async fn memberships_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<aro_core::OrganizationMember>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_organization_members(auth.user_id, auth.organization_id)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationListQuery {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    #[serde(default)]
    pub include_closed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InvitationCursor {
    created_at: DateTime<Utc>,
    id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitationPage {
    items: Vec<OrganizationInvitation>,
    next_cursor: Option<String>,
}

pub async fn invitations_list(
    State(state): State<ApiState>,
    auth: AuthContext,
    Query(query): Query<InvitationListQuery>,
) -> Result<Json<InvitationPage>, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200) as usize;
    let before = query
        .cursor
        .as_deref()
        .map(decode_invitation_cursor)
        .transpose()?
        .map(|cursor| (cursor.created_at, cursor.id));
    let mut items = state
        .store
        .list_organization_invitations(
            auth.user_id,
            auth.organization_id,
            before,
            query.include_closed,
            (limit + 1) as i64,
        )
        .await?;
    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit);
    }
    let next_cursor = if has_more {
        items.last().map(encode_invitation_cursor).transpose()?
    } else {
        None
    };
    Ok(Json(InvitationPage { items, next_cursor }))
}

pub async fn invitation_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(invitation_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .revoke_organization_invitation(auth.user_id, auth.organization_id, invitation_id)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MembershipCreateRequest {
    pub name: String,
    pub email: String,
    pub role: MembershipRole,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MembershipUpdateRequest {
    pub role: MembershipRole,
}

pub async fn membership_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<MembershipCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !state.invitation_delivery_enabled {
        return Err(ApiError::service_unavailable(
            "invitation delivery is not configured",
        ));
    }
    if request.name.trim().is_empty() {
        return Err(ApiError::bad_request("member name is required"));
    }
    if request.email.trim().is_empty() {
        return Err(ApiError::bad_request("member email is required"));
    }
    if request.role == MembershipRole::Owner {
        return Err(ApiError::bad_request("owner role cannot be invited"));
    }
    let invitation_token = generate_refresh_token();
    let expires_at = Utc::now() + ChronoDuration::hours(invitation_ttl_hours()?);
    let issued = state
        .store
        .issue_organization_invitation(
            auth.user_id,
            auth.organization_id,
            NewOrganizationMember {
                email: request.email,
                name: request.name,
                role: request.role,
            },
            invitation_token,
            expires_at,
            &state.secrets_key,
        )
        .await?;
    // The bearer token is encrypted for the server-side delivery worker and is deliberately
    // absent from this tenant-admin response.
    Ok(Json(json!({
        "invitationId": issued.id,
        "organizationId": issued.organization_id,
        "email": issued.email,
        "name": issued.name,
        "role": issued.role,
        "status": "pending",
        "expiresAt": issued.expires_at,
        "member": issued.member,
    })))
}

pub async fn membership_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(membership_id): Path<Uuid>,
    Json(request): Json<MembershipUpdateRequest>,
) -> Result<Json<aro_core::OrganizationMember>, ApiError> {
    if request.role == MembershipRole::Owner {
        return Err(ApiError::bad_request("owner role cannot be assigned here"));
    }
    Ok(Json(
        state
            .store
            .update_organization_member_role(
                auth.user_id,
                auth.organization_id,
                membership_id,
                request.role,
            )
            .await?,
    ))
}

pub async fn membership_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(membership_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .remove_organization_member(auth.user_id, auth.organization_id, membership_id)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyCreateRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyCreateResponse {
    pub key: aro_core::PublicApiKey,
    pub secret: String,
}

pub async fn api_keys_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<aro_core::PublicApiKey>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_api_keys(auth.user_id, auth.organization_id)
            .await?,
    ))
}

pub async fn api_key_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<ApiKeyCreateRequest>,
) -> Result<Json<ApiKeyCreateResponse>, ApiError> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("api key name is required"));
    }
    let secret = format!("aro_live_{}", generate_refresh_token());
    let prefix = secret.chars().take(14).collect::<String>();
    let key = state
        .store
        .create_api_key(
            auth.user_id,
            auth.organization_id,
            name.to_string(),
            prefix,
            hash_secret(&secret),
        )
        .await?;
    Ok(Json(ApiKeyCreateResponse { key, secret }))
}

pub async fn api_key_revoke(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(api_key_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .revoke_api_key(auth.user_id, auth.organization_id, api_key_id)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationApiKeyRequest {
    pub secret: String,
    #[serde(default)]
    pub public_config: Value,
    #[serde(default)]
    pub account: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationConnectStartRequest {
    #[serde(default)]
    pub scopes: Vec<String>,
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationConnectStartResponse {
    pub authorization_url: String,
    pub state: String,
    pub status_url: String,
    pub expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationConnectStatusQuery {
    pub state: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationOAuthCallbackQuery {
    pub state: String,
    pub code: Option<String>,
    pub error: Option<String>,
    #[serde(alias = "error_description")]
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialInstallationRequest {
    pub owner: OwnerRef,
    pub credential_type: String,
    pub secret: Value,
    pub label: Option<String>,
    #[serde(default)]
    pub public_config: Value,
    #[serde(default)]
    pub capability_ids: Vec<String>,
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallationPatchRequest {
    pub expected_version: i64,
    pub enabled: Option<bool>,
    #[serde(default)]
    pub label: Option<Option<String>>,
    pub public_config: Option<Value>,
    pub capability_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationAttemptRequest {
    pub owner: OwnerRef,
    pub method: AuthMethod,
    #[serde(default)]
    pub capability_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationAttemptResponse {
    pub attempt_id: Uuid,
    pub interaction: Value,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizationTransaction {
    state: String,
    nonce: String,
    pkce_verifier: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IntegrationOAuthV3Callback {
    pub state: String,
    pub code: Option<String>,
    pub error: Option<String>,
    #[serde(rename = "error_description", alias = "errorDescription")]
    pub _error_description: Option<String>,
    pub iss: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReauthorizationAttemptRequest {
    pub method: AuthMethod,
    #[serde(default)]
    pub capability_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminConnectorClientProfileRequest {
    pub provider_id: String,
    pub provider_version: String,
    pub environment: String,
    #[serde(default = "default_connector_realm")]
    pub realm: String,
    pub client_id: String,
    pub client_secret_ref: Option<String>,
    pub callback_uris: Vec<String>,
    pub enabled: bool,
}

pub async fn integration_catalog_v3_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    Ok(Json(
        state
            .store
            .integration_catalog_v3(auth.tenant_context())
            .await?,
    ))
}

pub async fn integration_catalog_v3_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(provider_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let provider = state
        .store
        .integration_catalog_entry_v3(auth.tenant_context(), &provider_id)
        .await?
        .ok_or_else(|| ApiError::not_found("integration provider not found"))?;
    Ok(Json(provider))
}

pub async fn integration_authorization_attempts_v3_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AuthorizationAttemptRequest>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let idempotency_key = required_idempotency_key(&headers)?;
    let response =
        begin_authorization_attempt_v3(&state, &auth, provider_id, request, None, idempotency_key)
            .await?;
    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(response),
    ))
}

pub async fn integration_authorization_attempt_v3_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(attempt_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let attempt = state
        .store
        .get_authorization_attempt_v3(auth.tenant_context(), attempt_id)
        .await?
        .ok_or_else(|| ApiError::not_found("authorization attempt not found"))?;
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(attempt),
    ))
}

pub async fn integration_authorization_attempt_v3_cancel(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(attempt_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let attempt = state
        .store
        .cancel_authorization_attempt_v3(auth.tenant_context(), attempt_id)
        .await?;
    Ok((
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(attempt),
    ))
}

pub async fn integration_oauth_v3_callback_get(
    State(state): State<ApiState>,
    Path(provider_id): Path<String>,
    Query(callback): Query<IntegrationOAuthV3Callback>,
) -> Response {
    complete_oauth_v3_callback(state, provider_id, callback).await
}

pub async fn integration_oauth_v3_callback_post(
    State(state): State<ApiState>,
    Path(provider_id): Path<String>,
    Form(callback): Form<IntegrationOAuthV3Callback>,
) -> Response {
    complete_oauth_v3_callback(state, provider_id, callback).await
}

pub async fn integration_installations_v3_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    Ok(Json(
        state
            .store
            .list_integration_installations_v3(auth.tenant_context())
            .await?,
    ))
}

pub async fn integration_installations_v3_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(installation_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let installation = state
        .store
        .get_integration_installation_v3(auth.tenant_context(), installation_id)
        .await?
        .ok_or_else(|| ApiError::not_found("integration installation not found"))?;
    Ok(Json(installation))
}

pub async fn integration_credentials_v3_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CredentialInstallationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let idempotency_key = headers
        .get(HeaderName::from_static("idempotency-key"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or_else(|| ApiError::bad_request("Idempotency-Key is required"))?
        .to_string();
    if !request.public_config.is_object() {
        return Err(ApiError::bad_request("publicConfig must be an object"));
    }
    if request
        .label
        .as_ref()
        .is_some_and(|label| label.chars().count() > 120)
    {
        return Err(ApiError::bad_request("integration label is too long"));
    }
    let credential_type = request.credential_type.trim().to_ascii_lowercase();
    if !matches!(
        credential_type.as_str(),
        "api_key" | "personal_access_token" | "service_account" | "external_vault"
    ) {
        return Err(ApiError::bad_request("unsupported credential type"));
    }
    let secret_payload = match &request.secret {
        Value::String(value) if !value.trim().is_empty() => value.trim().to_string(),
        Value::Object(values) if !values.is_empty() => serde_json::to_string(values)
            .map_err(|_| ApiError::bad_request("credential payload is invalid"))?,
        _ => {
            return Err(ApiError::bad_request(
                "a non-empty credential secret is required",
            ))
        }
    };
    if secret_payload.len() > 64 * 1024 {
        return Err(ApiError::payload_too_large(
            "credential payload exceeds 64 KiB",
        ));
    }
    let secret_payload = Zeroizing::new(secret_payload);
    let installation_id = Uuid::new_v4();
    let owner_type = match request.owner.owner_type {
        OwnerType::User => "user",
        OwnerType::Team => "team",
        OwnerType::Organization => "organization",
    };
    let request_hash = hash_secret(
        &serde_json::to_string(&json!({
            "providerId": provider_id,
            "owner": request.owner,
            "credentialType": credential_type,
            "secretHash": hash_secret(secret_payload.as_str()),
            "label": request.label,
            "publicConfig": request.public_config,
            "capabilityIds": request.capability_ids,
            "grantedScopes": request.granted_scopes,
            "expiresAt": request.expires_at,
        }))
        .map_err(|_| ApiError::bad_request("credential request is invalid"))?,
    );
    let aad = CredentialAad {
        organization_id: auth.organization_id,
        installation_id,
        provider_id: provider_id.clone(),
        credential_version: 1,
        environment: state.deployment.environment.clone(),
    };
    let envelope = state
        .envelope_crypto
        .encrypt(secret_payload.as_bytes(), &aad)
        .await
        .map_err(|_| ApiError::internal("credential encryption failed"))?;
    let installation = state
        .store
        .create_credential_installation_v3(
            auth.tenant_context(),
            NewCredentialInstallation {
                installation_id,
                provider_id,
                owner_type: owner_type.to_string(),
                owner_user_id: request.owner.user_id,
                owner_team_id: request.owner.team_id,
                label: request.label,
                public_config: request.public_config,
                capability_ids: request.capability_ids,
                credential_type,
                algorithm: envelope.algorithm,
                key_id: envelope.key_id,
                key_version: envelope.key_version,
                nonce: envelope.nonce,
                encrypted_dek: envelope.encrypted_dek,
                ciphertext: envelope.ciphertext,
                granted_scopes: request.granted_scopes,
                expires_at: request.expires_at,
                idempotency_key,
                request_hash,
            },
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(installation),
    ))
}

pub async fn integration_installations_v3_patch(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(installation_id): Path<Uuid>,
    Json(request): Json<InstallationPatchRequest>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    if request.expected_version < 1 {
        return Err(ApiError::bad_request("expectedVersion must be positive"));
    }
    Ok(Json(
        state
            .store
            .patch_integration_installation_v3(
                auth.tenant_context(),
                installation_id,
                InstallationPatchV3 {
                    enabled: request.enabled,
                    label: request.label,
                    public_config: request.public_config,
                    capability_ids: request.capability_ids,
                    expected_version: request.expected_version,
                },
            )
            .await?,
    ))
}

pub async fn integration_installations_v3_health_check(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(installation_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let job = state
        .store
        .enqueue_integration_action_v3(auth.tenant_context(), installation_id, "health")
        .await?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}

pub async fn integration_installations_v3_disconnect(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(installation_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let job = state
        .store
        .enqueue_integration_action_v3(auth.tenant_context(), installation_id, "revoke")
        .await?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}

pub async fn integration_installations_v3_reauthorize(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(installation_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<ReauthorizationAttemptRequest>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    let idempotency_key = required_idempotency_key(&headers)?;
    let installation = state
        .store
        .get_integration_installation_v3(auth.tenant_context(), installation_id)
        .await?
        .ok_or_else(|| ApiError::not_found("integration installation not found"))?;
    let provider_id = installation
        .get("providerId")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::internal("installation provider is invalid"))?
        .to_string();
    let owner_value = installation
        .get("owner")
        .cloned()
        .ok_or_else(|| ApiError::internal("installation owner is invalid"))?;
    let owner: OwnerRef = serde_json::from_value(owner_value)
        .map_err(|_| ApiError::internal("installation owner is invalid"))?;
    let response = begin_authorization_attempt_v3(
        &state,
        &auth,
        provider_id,
        AuthorizationAttemptRequest {
            owner,
            method: request.method,
            capability_ids: request.capability_ids,
        },
        Some(installation_id),
        idempotency_key,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        Json(response),
    ))
}

pub async fn integration_installations_v3_audit(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(installation_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    Ok(Json(
        state
            .store
            .integration_installation_audit_v3(auth.tenant_context(), installation_id)
            .await?,
    ))
}

pub async fn admin_integration_client_profile_v3_upsert(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<AdminConnectorClientProfileRequest>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_v3_enabled(&state)?;
    if request.environment != state.deployment.environment {
        return Err(ApiError::bad_request(
            "client profile environment must match this deployment",
        ));
    }
    if request.client_id.trim().is_empty() || request.client_id.len() > 512 {
        return Err(ApiError::bad_request("OAuth client ID is invalid"));
    }
    if request.realm.is_empty()
        || request.realm.len() > 255
        || !request
            .realm
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(ApiError::bad_request("client profile realm is invalid"));
    }
    if request
        .client_secret_ref
        .as_deref()
        .is_some_and(|reference| {
            reference.strip_prefix("env:").is_none_or(|name| {
                !name.starts_with("ARO_")
                    || name.len() > 128
                    || !name
                        .chars()
                        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
            })
        })
    {
        return Err(ApiError::bad_request(
            "clientSecretRef must be an env:ARO_* reference",
        ));
    }
    let base = state
        .deployment
        .public_base_url
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("ARO_PUBLIC_BASE_URL is required"))?
        .trim_end_matches('/');
    let canonical_callback = format!(
        "{base}/v1/integration-oauth/callback/{}",
        request.provider_id
    );
    if request.callback_uris.len() != 1
        || request.callback_uris.first() != Some(&canonical_callback)
    {
        return Err(ApiError::bad_request(
            "client profile must contain exactly the canonical server callback",
        ));
    }
    Ok(Json(
        state
            .store
            .upsert_connector_client_profile_v3(
                auth.tenant_context(),
                ConnectorClientProfileUpsert {
                    provider_id: request.provider_id,
                    provider_version: request.provider_version,
                    environment: request.environment,
                    realm: request.realm,
                    client_id: request.client_id.trim().to_string(),
                    client_secret_ref: request.client_secret_ref,
                    callback_uris: request.callback_uris,
                    enabled: request.enabled,
                },
            )
            .await?,
    ))
}

pub async fn integration_providers_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    Ok(Json(
        state
            .store
            .list_integration_providers(auth.user_id, auth.organization_id)
            .await?,
    ))
}

pub async fn integrations_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    Ok(Json(
        state
            .store
            .list_integrations(auth.user_id, auth.organization_id)
            .await?,
    ))
}

pub async fn integration_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(connection_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let connection = state
        .store
        .get_integration(auth.user_id, auth.organization_id, connection_id)
        .await?
        .ok_or_else(|| ApiError::not_found("integration not found"))?;
    Ok(Json(connection))
}

pub async fn integration_api_key_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(provider_id): Path<String>,
    Json(request): Json<IntegrationApiKeyRequest>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    if request.secret.trim().is_empty() {
        return Err(ApiError::bad_request("integration secret is required"));
    }
    Ok(Json(
        state
            .store
            .create_api_key_integration(
                auth.user_id,
                auth.organization_id,
                NewApiKeyIntegration {
                    provider_id,
                    secret: request.secret,
                    public_config: object_or_empty(request.public_config),
                    account: object_or_empty(request.account),
                    secrets_key: state.secrets_key.clone(),
                },
            )
            .await?,
    ))
}

pub async fn integration_connect_start(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(provider_id): Path<String>,
    Json(request): Json<IntegrationConnectStartRequest>,
) -> Result<Json<IntegrationConnectStartResponse>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    ensure_oauth_connect_enabled(&state)?;
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let provider = state
        .store
        .get_integration_provider(auth.user_id, auth.organization_id, &provider_id)
        .await?;
    let provider = provider.ok_or_else(|| ApiError::not_found("integration provider not found"))?;
    let oauth = provider_oauth_config(&provider)?;
    let authorization_url = validated_oauth_endpoint(oauth, "authorizationUrl")?;
    let redirect_uri = canonical_oauth_redirect_uri(&state, oauth)?;
    if request
        .redirect_uri
        .as_deref()
        .is_some_and(|provided| provided.trim() != redirect_uri)
    {
        return Err(ApiError::bad_request(
            "integration redirectUri must match the server-configured callback URL",
        ));
    }
    let client_id = integration_oauth_client_id(&provider_id)?;
    // Validate the full server-side provider configuration before redirecting the
    // user to a third party. Otherwise the callback would fail after consent.
    let _client_secret = integration_oauth_client_secret(&provider_id)?;

    let default_scopes = oauth_string_array(oauth, "defaultScopes");
    let requested_scopes = if request.scopes.is_empty() {
        default_scopes.clone()
    } else {
        request
            .scopes
            .into_iter()
            .map(|scope| scope.trim().to_string())
            .filter(|scope| !scope.is_empty())
            .collect::<Vec<_>>()
    };
    if !default_scopes.is_empty()
        && requested_scopes
            .iter()
            .any(|scope| !default_scopes.iter().any(|allowed| allowed == scope))
    {
        return Err(ApiError::bad_request("invalid integration OAuth scope"));
    }

    let state_token = random_urlsafe_token(32);
    let nonce = random_urlsafe_token(32);
    let code_verifier = random_urlsafe_token(48);
    let code_challenge = oauth_code_challenge(&code_verifier);
    let expires_at = Utc::now() + ChronoDuration::minutes(10);

    state
        .store
        .create_integration_oauth_state(
            auth.user_id,
            auth.organization_id,
            NewIntegrationOAuthState {
                provider_id: provider_id.clone(),
                state_hash: hash_secret(&state_token),
                nonce_hash: hash_secret(&nonce),
                pkce_verifier: code_verifier,
                redirect_uri: redirect_uri.clone(),
                requested_scopes: requested_scopes.clone(),
                expires_at,
                secrets_key: state.secrets_key.clone(),
            },
        )
        .await?;

    let mut url = Url::parse(&authorization_url)
        .map_err(|_| ApiError::internal("invalid server OAuth authorization URL"))?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs
            .append_pair("response_type", "code")
            .append_pair("client_id", &client_id)
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("state", &state_token)
            .append_pair("scope", &requested_scopes.join(" "))
            .append_pair("code_challenge", &code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("nonce", &nonce);
    }
    let returned_url = url.to_string();

    Ok(Json(IntegrationConnectStartResponse {
        authorization_url: returned_url,
        state: state_token.clone(),
        status_url: format!("/v1/integrations/connect/status?state={state_token}"),
        expires_at,
    }))
}

pub async fn integration_connect_status(
    State(state): State<ApiState>,
    auth: AuthContext,
    Query(query): Query<IntegrationConnectStatusQuery>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    Ok(Json(
        state
            .store
            .get_integration_oauth_status(
                auth.user_id,
                auth.organization_id,
                hash_secret(&query.state),
            )
            .await?,
    ))
}

fn render_success_page() -> String {
    std::fmt::format(format_args!(
        r#"
<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Connexion Réussie - ARO</title>
    <style>
        body {{
            font-family: 'Outfit', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background-color: #0b0b0c;
            color: #ffffff;
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100vh;
            margin: 0;
            overflow: hidden;
        }}
        .container {{
            text-align: center;
            padding: 40px;
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid rgba(255, 255, 255, 0.08);
            border-radius: 24px;
            backdrop-filter: blur(20px);
            box-shadow: 0 20px 50px rgba(0, 0, 0, 0.3);
            max-width: 420px;
            width: 90%;
            animation: fadeIn 0.8s ease-out;
        }}
        .icon {{
            width: 80px;
            height: 80px;
            background: linear-gradient(135deg, #0071e3 0%, #00c7f2 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            margin: 0 auto 24px;
            font-size: 40px;
            color: white;
            box-shadow: 0 8px 24px rgba(0, 113, 227, 0.3);
        }}
        h1 {{
            font-size: 24px;
            margin: 0 0 10px;
            font-weight: 700;
        }}
        p {{
            color: #86868b;
            font-size: 14px;
            line-height: 1.6;
            margin: 0 0 30px;
        }}
        .btn {{
            display: inline-block;
            background: #ffffff;
            color: #000000;
            padding: 12px 30px;
            border-radius: 30px;
            font-weight: 600;
            text-decoration: none;
            font-size: 14px;
            transition: all 0.2s ease;
            cursor: pointer;
            border: none;
            box-shadow: 0 4px 12px rgba(255,255,255,0.1);
        }}
        .btn:hover {{
            transform: translateY(-2px);
            box-shadow: 0 6px 20px rgba(255,255,255,0.2);
        }}
        @keyframes fadeIn {{
            from {{ opacity: 0; transform: translateY(20px); }}
            to {{ opacity: 1; transform: translateY(0); }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="icon">✓</div>
        <h1>Connexion réussie !</h1>
        <p>Votre compte a été associé à ARO avec succès. Vous pouvez maintenant fermer cette fenêtre et retourner sur l'application ARO.</p>
        <p class="btn">Vous pouvez fermer cette fenêtre.</p>
    </div>
</body>
</html>
"#
    ))
}

fn render_error_page(details: &str) -> String {
    let escaped_details = details.replace('<', "&lt;").replace('>', "&gt;");
    format!(
        r#"
<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Erreur de Connexion - ARO</title>
    <style>
        body {{
            font-family: 'Outfit', -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            background-color: #0b0b0c;
            color: #ffffff;
            display: flex;
            align-items: center;
            justify-content: center;
            height: 100vh;
            margin: 0;
            overflow: hidden;
        }}
        .container {{
            text-align: center;
            padding: 40px;
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid rgba(255, 255, 255, 0.08);
            border-radius: 24px;
            backdrop-filter: blur(20px);
            box-shadow: 0 20px 50px rgba(0, 0, 0, 0.3);
            max-width: 420px;
            width: 90%;
            animation: fadeIn 0.8s ease-out;
        }}
        .icon {{
            width: 80px;
            height: 80px;
            background: linear-gradient(135deg, #ff453a 0%, #ff9f0a 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            margin: 0 auto 24px;
            font-size: 40px;
            color: white;
            box-shadow: 0 8px 24px rgba(255, 69, 58, 0.3);
        }}
        h1 {{
            font-size: 24px;
            margin: 0 0 10px;
            font-weight: 700;
        }}
        p {{
            color: #ff453a;
            font-size: 14px;
            line-height: 1.6;
            margin: 0 0 30px;
        }}
        .btn {{
            display: inline-block;
            background: #ffffff;
            color: #000000;
            padding: 12px 30px;
            border-radius: 30px;
            font-weight: 600;
            text-decoration: none;
            font-size: 14px;
            transition: all 0.2s ease;
            cursor: pointer;
            border: none;
            box-shadow: 0 4px 12px rgba(255,255,255,0.1);
        }}
        .btn:hover {{
            transform: translateY(-2px);
            box-shadow: 0 6px 20px rgba(255,255,255,0.2);
        }}
        @keyframes fadeIn {{
            from {{ opacity: 0; transform: translateY(20px); }}
            to {{ opacity: 1; transform: translateY(0); }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="icon">✕</div>
        <h1>Échec de connexion</h1>
        <p>Une erreur est survenue lors de l'authentification : {escaped_details}</p>
        <p class="btn">Vous pouvez fermer cette fenêtre.</p>
    </div>
</body>
</html>
"#,
        escaped_details = escaped_details
    )
}

pub async fn integration_oauth_callback(
    State(state): State<ApiState>,
    Path(provider_id): Path<String>,
    Query(query): Query<IntegrationOAuthCallbackQuery>,
) -> impl IntoResponse {
    let result = async {
        ensure_integrations_api_enabled(&state)?;
        ensure_oauth_connect_enabled(&state)?;

        if query.state.trim().is_empty() {
            return Err(ApiError::bad_request("OAuth state is required"));
        }

        if let Some(err) = query
            .error
            .filter(|v| !v.trim().is_empty())
            .or(query.error_description)
        {
            state
                .store
                .fail_integration_oauth_state(&provider_id, hash_secret(&query.state), &err)
                .await?;
            return Err(ApiError::bad_request(format!(
                "OAuth error callback: {err}"
            )));
        }

        let code = match query.code.filter(|v| !v.trim().is_empty()) {
            Some(c) => c,
            None => {
                let err = "OAuth code missing in callback";
                state
                    .store
                    .fail_integration_oauth_state(&provider_id, hash_secret(&query.state), err)
                    .await?;
                return Err(ApiError::bad_request(err));
            }
        };

        // 1. Consume the OAuth state from DB and decrypt verifier
        let state_row = state
            .store
            .consume_integration_oauth_state(
                &provider_id,
                hash_secret(&query.state),
                &state.secrets_key,
            )
            .await
            .map_err(|e| ApiError::bad_request(format!("Invalid or expired OAuth state: {e}")))?;

        // 2. Fetch provider manifest configuration
        let provider = state
            .store
            .get_integration_provider(
                state_row.user_id,
                state_row.organization_id,
                &state_row.provider_id,
            )
            .await?
            .ok_or_else(|| ApiError::not_found("integration provider not found"))?;

        let oauth = provider_oauth_config(&provider)?;
        let token_url = validated_oauth_endpoint(oauth, "tokenUrl")?;
        let userinfo_url = optional_validated_oauth_endpoint(oauth, "userinfoUrl")?;

        // 3. Perform Token Exchange POST request
        let client_id = integration_oauth_client_id(&provider_id)?;
        let client_secret = integration_oauth_client_secret(&provider_id)?;

        let (token_payload, account_info) = {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(|_| ApiError::internal("unable to initialize OAuth client"))?;
            let mut params = std::collections::HashMap::new();
            params.insert("grant_type", "authorization_code");
            params.insert("client_id", &client_id);
            params.insert("client_secret", &client_secret);
            params.insert("code", &code);
            params.insert("redirect_uri", &state_row.redirect_uri);
            params.insert("code_verifier", &state_row.pkce_verifier);

            let token_response = client
                .post(&token_url)
                .form(&params)
                .send()
                .await
                .map_err(|_| ApiError::bad_request("OAuth token exchange failed"))?;

            if !token_response.status().is_success() {
                let err_msg = "OAuth token exchange failed";
                state
                    .store
                    .fail_integration_oauth_state(&provider_id, hash_secret(&query.state), err_msg)
                    .await?;
                return Err(ApiError::bad_request(err_msg));
            }

            let token_payload: Value = token_response
                .json()
                .await
                .map_err(|_| ApiError::bad_request("OAuth token response is invalid"))?;

            let access_token = token_payload
                .get("access_token")
                .and_then(Value::as_str)
                .ok_or_else(|| ApiError::bad_request("OAuth token response is incomplete"))?;

            let mut account_info = json!({});
            if let Some(ref url) = userinfo_url {
                let userinfo_response = client
                    .get(url)
                    .bearer_auth(access_token)
                    .header("User-Agent", "ARO-Desktop")
                    .send()
                    .await;

                if let Ok(resp) = userinfo_response {
                    if resp.status().is_success() {
                        if let Ok(info) = resp.json::<Value>().await {
                            account_info = info;
                        }
                    }
                }
            }
            (token_payload, account_info)
        };

        // 5. Normalize account info
        let normalized_account = if account_info.is_object() {
            let ext_id = account_info
                .get("sub")
                .or_else(|| account_info.get("id"))
                .or_else(|| account_info.get("login"))
                .map(|v| match v {
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => s.clone(),
                    _ => "".to_string(),
                })
                .filter(|s| !s.is_empty());

            let display_name = account_info
                .get("name")
                .or_else(|| account_info.get("displayName"))
                .or_else(|| account_info.get("login"))
                .and_then(Value::as_str)
                .map(str::to_string);

            let email = account_info
                .get("email")
                .and_then(Value::as_str)
                .map(str::to_string);

            let avatar_url = account_info
                .get("picture")
                .or_else(|| account_info.get("avatar_url"))
                .or_else(|| account_info.get("avatarUrl"))
                .and_then(Value::as_str)
                .map(str::to_string);

            json!({
                "externalAccountId": ext_id.unwrap_or_else(|| "unknown".to_string()),
                "displayName": display_name.unwrap_or_else(|| "Connected Account".to_string()),
                "email": email,
                "avatarUrl": avatar_url,
                "details": {
                    "raw": account_info
                }
            })
        } else {
            json!({
                "externalAccountId": "unknown",
                "displayName": "Connected Account",
            })
        };

        // 6. Create integration connection
        let connection_id = state
            .store
            .create_oauth_integration(NewOAuthIntegration {
                user_id: state_row.user_id,
                organization_id: state_row.organization_id,
                provider_id: state_row.provider_id.clone(),
                manifest_version: state_row.manifest_version,
                granted_scopes: state_row.requested_scopes,
                account: normalized_account,
                credential_payload: token_payload,
                secrets_key: state.secrets_key.clone(),
            })
            .await?;

        // 7. Mark OAuth state as completed
        state
            .store
            .succeed_integration_oauth_state(state_row.id, connection_id)
            .await?;

        Ok::<_, ApiError>(connection_id)
    }
    .await;

    match result {
        Ok(_) => Html(render_success_page()).into_response(),
        Err(err) => {
            tracing::warn!(error = %err.message(), "OAuth callback was not completed");
            Html(render_error_page(
                "The connection could not be completed. Return to ARO and try again.",
            ))
            .into_response()
        }
    }
}

pub async fn integration_validate(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(connection_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    Ok(Json(
        state
            .store
            .validate_integration(auth.user_id, auth.organization_id, connection_id)
            .await?,
    ))
}

pub async fn integration_patch(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(connection_id): Path<Uuid>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    Ok(Json(
        state
            .store
            .update_integration(auth.user_id, auth.organization_id, connection_id, payload)
            .await?,
    ))
}

pub async fn integration_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(connection_id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    ensure_integrations_api_enabled(&state)?;
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    state
        .store
        .delete_integration(auth.user_id, auth.organization_id, connection_id)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    /// Id client optionnel pour une création idempotente (sync desktop :
    /// le même projet repoussé ne doit pas être dupliqué côté serveur).
    #[serde(default)]
    pub id: Option<Uuid>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub root_path: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub instructions: Option<Option<String>>,
    pub root_path: Option<Option<String>>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderRequest {
    /// Id client optionnel pour une création idempotente (voir projets).
    #[serde(default)]
    pub id: Option<Uuid>,
    pub name: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub root_path: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFolderRequest {
    pub name: Option<String>,
    pub project_id: Option<Option<String>>,
    pub root_path: Option<Option<String>>,
    pub color: Option<Option<String>>,
    pub icon: Option<String>,
}

pub async fn projects_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<Project>>, ApiError> {
    // Auto-nettoyage des doublons stricts (anciennes syncs non-idempotentes).
    if let Err(err) = state
        .store
        .deduplicate_projects(auth.tenant_context())
        .await
    {
        tracing::warn!(?err, "project deduplication failed");
    }
    let list = state.store.list_projects(auth.tenant_context()).await?;
    Ok(Json(list))
}

pub async fn project_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<Project>), ApiError> {
    let trimmed_name = payload.name.trim().to_string();
    if trimmed_name.is_empty() {
        return Err(ApiError::bad_request("Project name cannot be empty"));
    }
    // Création idempotente : si le client renvoie un id déjà connu, on ne
    // duplique pas, on retourne l'existant.
    if let Some(id) = payload.id {
        if let Some(existing) = state.store.get_project(auth.tenant_context(), id).await? {
            return Ok((StatusCode::OK, Json(existing)));
        }
    }
    let now = Utc::now();
    let project = Project {
        id: payload.id.unwrap_or_else(Uuid::new_v4),
        name: trimmed_name,
        description: payload.description.filter(|s| !s.trim().is_empty()),
        instructions: payload.instructions.filter(|s| !s.trim().is_empty()),
        root_path: payload.root_path.filter(|s| !s.trim().is_empty()),
        color: payload.color.unwrap_or_else(|| "#3b82f6".into()),
        icon: payload.icon.unwrap_or_else(|| "folder-tree".into()),
        created_at: now,
        updated_at: now,
    };
    let saved = state
        .store
        .create_project(auth.tenant_context(), &project)
        .await?;
    Ok((StatusCode::CREATED, Json(saved)))
}

pub async fn project_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Project>, ApiError> {
    let project = state
        .store
        .get_project(auth.tenant_context(), project_id)
        .await?
        .ok_or_else(|| ApiError::not_found("Project not found"))?;
    Ok(Json(project))
}

pub async fn project_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<UpdateProjectRequest>,
) -> Result<Json<Project>, ApiError> {
    let updated = state
        .store
        .update_project(
            auth.tenant_context(),
            project_id,
            payload.name.map(|n| n.trim().to_string()),
            payload.description,
            payload.instructions,
            payload.root_path,
            payload.color,
            payload.icon,
        )
        .await?
        .ok_or_else(|| ApiError::not_found("Project not found"))?;
    Ok(Json(updated))
}

pub async fn project_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(project_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .store
        .delete_project(auth.tenant_context(), project_id)
        .await?;
    if !deleted {
        return Err(ApiError::not_found("Project not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn folders_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<Folder>>, ApiError> {
    if let Err(err) = state.store.deduplicate_folders(auth.tenant_context()).await {
        tracing::warn!(?err, "folder deduplication failed");
    }
    let list = state.store.list_folders(auth.tenant_context()).await?;
    Ok(Json(list))
}

pub async fn folder_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<CreateFolderRequest>,
) -> Result<(StatusCode, Json<Folder>), ApiError> {
    let trimmed_name = payload.name.trim().to_string();
    if trimmed_name.is_empty() {
        return Err(ApiError::bad_request("Folder name cannot be empty"));
    }
    if let Some(id) = payload.id {
        if let Some(existing) = state.store.get_folder(auth.tenant_context(), id).await? {
            return Ok((StatusCode::OK, Json(existing)));
        }
    }
    let proj_uuid = payload
        .project_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let now = Utc::now();
    let folder = Folder {
        id: payload.id.unwrap_or_else(Uuid::new_v4),
        project_id: proj_uuid,
        name: trimmed_name,
        root_path: payload.root_path.filter(|s| !s.trim().is_empty()),
        color: payload.color,
        icon: payload.icon.unwrap_or_else(|| "folder".into()),
        created_at: now,
        updated_at: now,
    };
    let saved = state
        .store
        .create_folder(auth.tenant_context(), &folder)
        .await?;
    Ok((StatusCode::CREATED, Json(saved)))
}

pub async fn folder_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(folder_id): Path<Uuid>,
    Json(payload): Json<UpdateFolderRequest>,
) -> Result<Json<Folder>, ApiError> {
    let proj_uuid = payload
        .project_id
        .map(|opt| opt.and_then(|s| Uuid::parse_str(&s).ok()));
    let updated = state
        .store
        .update_folder(
            auth.tenant_context(),
            folder_id,
            payload.name.map(|n| n.trim().to_string()),
            proj_uuid,
            payload.root_path,
            payload.color,
            payload.icon,
        )
        .await?
        .ok_or_else(|| ApiError::not_found("Folder not found"))?;
    Ok(Json(updated))
}

pub async fn folder_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(folder_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .store
        .delete_folder(auth.tenant_context(), folder_id)
        .await?;
    if !deleted {
        return Err(ApiError::not_found("Folder not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationRequest {
    pub title: Option<String>,
    pub mode: AssistantMode,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub folder_id: Option<Uuid>,
}

pub async fn conversation_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<CreateConversationRequest>,
) -> Result<Json<Conversation>, ApiError> {
    let title = request
        .title
        .unwrap_or_else(|| "New conversation".to_string());
    let mut conversation = state
        .store
        .create_conversation(auth.tenant_context(), title, request.mode)
        .await?;
    if request.project_id.is_some() || request.folder_id.is_some() {
        conversation = state
            .store
            .set_conversation_placement(
                auth.tenant_context(),
                conversation.id,
                request.project_id,
                request.folder_id,
            )
            .await?;
    }
    Ok(Json(conversation))
}

pub async fn conversations_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<Conversation>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_conversations(auth.tenant_context())
            .await?,
    ))
}

pub async fn conversation_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
) -> Result<Json<Conversation>, ApiError> {
    let conversation = state
        .store
        .get_conversation(auth.tenant_context(), conversation_id)
        .await?
        .ok_or_else(|| ApiError::not_found("conversation not found"))?;
    Ok(Json(conversation))
}

#[derive(Debug, Deserialize)]
pub struct ConversationExportQuery {
    pub format: Option<String>,
}

pub async fn conversation_export(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<ConversationExportQuery>,
) -> Result<(axum::http::HeaderMap, String), ApiError> {
    let convo = state
        .store
        .get_conversation(auth.tenant_context(), conversation_id)
        .await?
        .ok_or_else(|| ApiError::not_found("conversation not found"))?;

    let messages = state
        .store
        .list_messages(auth.tenant_context(), conversation_id)
        .await?;

    let fmt = query.format.as_deref().unwrap_or("markdown");
    let mut headers = axum::http::HeaderMap::new();

    if fmt == "json" {
        let content = serde_json::to_string_pretty(&json!({
            "conversation": convo,
            "messages": messages
        }))
        .map_err(|err| ApiError::internal(err.to_string()))?;

        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            axum::http::header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"conversation_{conversation_id}.json\""
            ))
            .map_err(|err| ApiError::internal(err.to_string()))?,
        );
        Ok((headers, content))
    } else {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", convo.title));
        md.push_str(&format!(
            "*Created: {}*\n\n---\n\n",
            convo.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        ));

        for msg in messages {
            let role_str = match msg.role {
                MessageRole::User => "**User**",
                MessageRole::Assistant => "**Assistant**",
                MessageRole::System => "**System**",
            };
            md.push_str(&format!("### {}\n\n{}\n\n", role_str, msg.content));
        }

        headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/markdown; charset=utf-8"),
        );
        headers.insert(
            axum::http::header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!(
                "attachment; filename=\"conversation_{conversation_id}.md\""
            ))
            .map_err(|err| ApiError::internal(err.to_string()))?,
        );
        Ok((headers, md))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationUpdateRequest {
    pub title: String,
}

pub async fn conversation_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Json(request): Json<ConversationUpdateRequest>,
) -> Result<Json<Conversation>, ApiError> {
    Ok(Json(
        state
            .store
            .update_conversation_title(auth.tenant_context(), conversation_id, request.title)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMoveRequest {
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub folder_id: Option<Uuid>,
}

pub async fn conversation_move(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
    Json(request): Json<ConversationMoveRequest>,
) -> Result<Json<Conversation>, ApiError> {
    let conversation = state
        .store
        .set_conversation_placement(
            auth.tenant_context(),
            conversation_id,
            request.project_id,
            request.folder_id,
        )
        .await?;
    Ok(Json(conversation))
}

pub async fn conversation_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .delete_conversation(auth.tenant_context(), conversation_id)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn messages_list(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(conversation_id): Path<Uuid>,
) -> Result<Json<Vec<ChatMessage>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_messages(auth.tenant_context(), conversation_id)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageUpdateRequest {
    pub content: String,
}

pub async fn message_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(message_id): Path<Uuid>,
    Json(request): Json<MessageUpdateRequest>,
) -> Result<Json<ChatMessage>, ApiError> {
    let msg = state
        .store
        .update_message(auth.tenant_context(), message_id, request.content)
        .await?;
    Ok(Json(msg))
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalResultRequest {
    pub conversation: Conversation,
    pub user_message: ChatMessage,
    pub assistant_message: ChatMessage,
}

pub async fn assistant_local_result(
    State(state): State<ApiState>,
    auth: AuthContext,
    headers: HeaderMap,
    Json(request): Json<LocalResultRequest>,
) -> Result<Response, ApiError> {
    let idempotency_key = optional_idempotency_key(&headers)?;
    let mut idempotency_lease = None;
    if let Some(key) = idempotency_key.as_deref() {
        let canonical_request = serde_json::to_vec(&request)
            .map_err(|_| ApiError::internal("could not canonicalize idempotent request"))?;
        let identity = IdempotencyRequestKey {
            organization_id: auth.organization_id,
            actor_id: auth.user_id,
            scope: "POST:/v1/assistant/local-result".to_string(),
            key: key.to_string(),
        };
        match state
            .store
            .begin_idempotency_request(
                &identity,
                IdempotencyRequestHash::digest(canonical_request),
                IDEMPOTENCY_DEFAULT_TTL_SECONDS,
            )
            .await?
        {
            IdempotencyBegin::Acquired(lease) => idempotency_lease = Some(lease),
            IdempotencyBegin::Replay(response) => {
                return idempotency_http_response(response, Some(key), true)
            }
            IdempotencyBegin::InProgress { .. } => {
                return Err(ApiError::conflict(
                    "an identical request is already being processed",
                ))
            }
            IdempotencyBegin::Conflict => {
                return Err(ApiError::conflict(
                    "idempotency key was already used with a different request",
                ))
            }
        }
    }

    let lock_key = state.redis.lock_key(
        "conversation",
        &[
            auth.organization_id.to_string(),
            request.conversation.id.to_string(),
        ],
    );
    let lock = acquire_required_lock(
        &state,
        lock_key,
        90,
        "conversation generation is already running",
    )
    .await?;
    let result = state
        .store
        .store_local_result(
            auth.tenant_context(),
            request.conversation,
            request.user_message,
            request.assistant_message,
        )
        .await;
    release_redis_lock(lock).await;
    result?;

    let response = IdempotencyResponse {
        status_code: StatusCode::OK.as_u16(),
        headers: BTreeMap::from([(
            "content-type".to_string(),
            vec!["application/json".to_string()],
        )]),
        body: serde_json::to_vec(&json!({ "ok": true }))
            .map_err(|_| ApiError::internal("could not serialize response"))?,
    };
    if let Some(lease) = idempotency_lease.as_ref() {
        match state
            .store
            .complete_idempotency_request(lease, &response)
            .await?
        {
            IdempotencyCompletion::Completed => {}
            IdempotencyCompletion::AlreadyCompleted(stored) => {
                return idempotency_http_response(stored, idempotency_key.as_deref(), true)
            }
            IdempotencyCompletion::LeaseLost => {
                return Err(ApiError::conflict(
                    "idempotency execution lease expired before completion",
                ))
            }
        }
    }
    idempotency_http_response(response, idempotency_key.as_deref(), false)
}

fn optional_idempotency_key(headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    let values = headers.get_all("idempotency-key");
    let mut values = values.iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(ApiError::bad_request(
            "idempotency key must be provided exactly once",
        ));
    }
    let value = value
        .to_str()
        .map_err(|_| ApiError::bad_request("idempotency key must be valid ASCII"))?;
    if value.is_empty()
        || value.len() > 255
        || value.trim() != value
        || !value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err(ApiError::bad_request(
            "idempotency key must contain 1 to 255 visible ASCII bytes",
        ));
    }
    Ok(Some(value.to_string()))
}

fn idempotency_http_response(
    stored: IdempotencyResponse,
    key: Option<&str>,
    replayed: bool,
) -> Result<Response, ApiError> {
    let status = StatusCode::from_u16(stored.status_code)
        .map_err(|_| ApiError::internal("stored idempotency status is invalid"))?;
    let mut response = Response::builder()
        .status(status)
        .body(Body::from(stored.body))
        .map_err(|_| ApiError::internal("could not construct idempotent response"))?;
    for (name, values) in stored.headers {
        if !matches!(
            name.as_str(),
            "content-type" | "location" | "etag" | "cache-control"
        ) {
            continue;
        }
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| ApiError::internal("stored idempotency header is invalid"))?;
        for value in values {
            response.headers_mut().append(
                name.clone(),
                HeaderValue::from_str(&value)
                    .map_err(|_| ApiError::internal("stored idempotency header is invalid"))?,
            );
        }
    }
    if let Some(key) = key {
        response.headers_mut().insert(
            HeaderName::from_static("idempotency-key"),
            HeaderValue::from_str(key)
                .map_err(|_| ApiError::internal("idempotency key response is invalid"))?,
        );
    }
    if replayed {
        response.headers_mut().insert(
            HeaderName::from_static("idempotency-replayed"),
            HeaderValue::from_static("true"),
        );
    }
    Ok(response)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantStreamRequest {
    pub conversation_id: Option<Uuid>,
    pub content: String,
    pub mode: AssistantMode,
    pub system_prompt: Option<String>,
    /// Surcharge modèle du composer (ignorée si inexploitable côté serveur).
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub attachments: Vec<AttachmentRef>,
    #[serde(default)]
    pub web_access: WebAccessMode,
    #[serde(default)]
    pub search_settings: Option<aro_core::settings::SearchSettings>,
}

pub async fn assistant_stream(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<AssistantStreamRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let trimmed = request.content.trim().to_string();
    if trimmed.is_empty() && request.attachments.is_empty() {
        return Err(ApiError::bad_request("message content cannot be empty"));
    }
    let effective_content = if trimmed.is_empty() {
        "Attachments received.".to_string()
    } else {
        trimmed.clone()
    };

    let conversation = match request.conversation_id {
        Some(id) => state
            .store
            .get_conversation(auth.tenant_context(), id)
            .await?
            .ok_or_else(|| ApiError::not_found("conversation not found"))?,
        None => Conversation::new(make_title(&effective_content), request.mode.clone()),
    };
    let lock_key = state.redis.lock_key(
        "conversation",
        &[
            auth.organization_id.to_string(),
            conversation.id.to_string(),
        ],
    );
    let lock = acquire_required_lock(
        &state,
        lock_key,
        90,
        "conversation generation is already running",
    )
    .await?;
    let result = async {
        let conversation = state
            .store
            .upsert_conversation(auth.tenant_context(), &conversation)
            .await?;
        let mut user_message = ChatMessage::new(
            conversation.id,
            MessageRole::User,
            effective_content.clone(),
        );
        user_message.attachments = request
            .attachments
            .iter()
            .map(MessageAttachment::from)
            .collect();
        let user_message = state
            .store
            .add_message(auth.tenant_context(), &user_message)
            .await?;

        let system_prompt = request
            .system_prompt
            .clone()
            .unwrap_or_else(|| request.mode.system_instruction());
        let runtime = AgentRuntime::new();
        let mut agent_run_id = None;
        let mut web_sources = Vec::new();
        let mut web_summaries = Vec::new();
        if !matches!(request.web_access, WebAccessMode::Off)
            && (!extract_urls(&effective_content, 1).is_empty()
                || matches!(request.web_access, WebAccessMode::On)
                || web_request_likely(&effective_content))
        {
            let run = create_immediate_api_run(
                &state,
                &auth,
                &runtime,
                conversation.id,
                effective_content.clone(),
                request.mode.clone(),
                Some(system_prompt.clone()),
            )
            .await?;
            let enrichment = collect_web_context_for_run(
                &state,
                &auth,
                &run,
                &effective_content,
                request.web_access.clone(),
                2,
                request.search_settings.clone(),
            )
            .await?;
            web_sources = enrichment.sources;
            web_summaries = enrichment.summaries;
            let context_pack = runtime.build_context_pack(
                &run,
                std::slice::from_ref(&user_message),
                &web_sources,
                &[],
                EnvironmentSnapshot::api_durable(None, None),
            );
            state
                .store
                .add_agent_step(
                    auth.user_id,
                    auth.organization_id,
                    &runtime.context_step_at(&run, enrichment.next_sequence, &context_pack),
                )
                .await?;
            agent_run_id = Some(run.id);
        }

        // Le modèle ne voit pas le contexte web tout seul : on l'ajoute au
        // prompt système quand la pré-lecture en a ramené.
        let mut generation_system_prompt = system_prompt.clone();
        let web_context = web_summaries
            .iter()
            .filter(|summary| !summary.trim().is_empty())
            .take(3)
            .map(|summary| format!("- {}", summary.replace('\n', " ")))
            .chain(web_sources.iter().take(5).map(|source| {
                let mut line = format!("- {}", source.title);
                if let Some(uri) = &source.uri {
                    line.push_str(&format!(" ({uri})"));
                }
                let excerpt = source.excerpt.replace('\n', " ");
                if !excerpt.trim().is_empty() {
                    line.push_str(&format!(
                        ": {}",
                        excerpt.chars().take(220).collect::<String>()
                    ));
                }
                line
            }))
            .collect::<Vec<_>>()
            .join("\n");
        if !web_context.trim().is_empty() {
            generation_system_prompt.push_str("\n\nContexte web collecté :\n");
            generation_system_prompt.push_str(&web_context);
        }

        // Historique récent (sans le message utilisateur qui vient d'être
        // enregistré : il part dans `user_input`).
        let stored_messages = state
            .store
            .list_messages(auth.tenant_context(), conversation.id)
            .await
            .unwrap_or_default();
        let mut history = stored_messages;
        if history
            .last()
            .is_some_and(|last| last.id == user_message.id)
        {
            history.pop();
        }
        if history.len() > 20 {
            history = history.split_off(history.len() - 20);
        }

        let mut chunks = Vec::new();
        let generation = generate_server_assistant_text(
            &state,
            &auth,
            &request.mode,
            &generation_system_prompt,
            &history,
            &effective_content,
            request.model_id.clone(),
            request.provider.clone(),
            &mut |chunk: String| {
                if !chunk.is_empty() {
                    chunks.push(chunk);
                }
            },
        )
        .await;
        let response_text = generation.text;
        let mut assistant_message = ChatMessage::new(
            conversation.id,
            MessageRole::Assistant,
            response_text.clone(),
        );
        assistant_message.token_estimate = generation.token_estimate;
        let assistant_message = state
            .store
            .add_message(auth.tenant_context(), &assistant_message)
            .await?;

        if chunks.is_empty() && !response_text.is_empty() {
            chunks = response_text
                .split_inclusive(' ')
                .map(str::to_string)
                .collect();
        }
        let events = chunks
            .into_iter()
            .map(|chunk| {
                Ok(Event::default()
                    .event("chunk")
                    .data(sse_json_data(&json!({ "content": chunk }))))
            })
            .chain(std::iter::once(Ok(Event::default().event("done").data(
                sse_json_data(&json!({
                    "conversation": conversation,
                    "userMessage": user_message,
                    "assistantMessage": assistant_message,
                    "agentRunId": agent_run_id,
                    "modelId": generation.model_label
                })),
            ))));

        Ok(Sse::new(stream::iter(events)).keep_alive(
            axum::response::sse::KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        ))
    }
    .await;
    release_redis_lock(lock).await;
    result
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolExecuteRequest {
    pub run_id: Uuid,
    pub tool_id: String,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub web_access: WebAccessMode,
}

pub async fn agent_tool_execute(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<AgentToolExecuteRequest>,
) -> Result<Json<ToolExecutionResult>, ApiError> {
    if !state.agent_direct_tool_execution_enabled {
        return Err(ApiError::not_found(
            "direct tool execution is disabled; submit work to the durable execution service",
        ));
    }
    let run = state
        .store
        .get_agent_run(auth.user_id, auth.organization_id, request.run_id)
        .await?
        .ok_or_else(|| ApiError::bad_request("agent run not found"))?;
    let profile_id = run.autonomy_profile_id.ok_or_else(|| {
        ApiError::forbidden("direct tool execution requires an explicit permission profile")
    })?;
    let permission_profile = state
        .store
        .list_agent_permission_profiles(auth.user_id, auth.organization_id)
        .await?
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| ApiError::forbidden("agent permission profile is unavailable"))?;
    if !permission_profile.allow_network || permission_profile.allowed_domains.is_empty() {
        return Err(ApiError::forbidden(
            "agent permission profile does not allow network access to any domain",
        ));
    }
    let steps = state
        .store
        .list_agent_steps(auth.user_id, auth.organization_id, run.id)
        .await?;
    let sequence = steps.iter().map(|step| step.sequence).max().unwrap_or(1) + 1;
    let policy = explicit_web_policy(&state, &permission_profile, request.web_access);
    let tool_request =
        ToolExecutionRequest::new(run.id, run.conversation_id, request.tool_id, request.input);
    let result = execute_api_tool(&state, &auth, &run, tool_request, &policy, sequence).await?;
    Ok(Json(result))
}

pub async fn tools_code_execute(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<Value>,
) -> Result<Json<ToolExecutionResult>, ApiError> {
    // Les exécutions directes contournent le service durable : interdites
    // sauf activation explicite (ARO_AGENT_DIRECT_TOOL_EXECUTION), comme
    // `agent_tool_execute`. Sans ceci, tout compte authentifié exécutait
    // du code arbitraire sur le serveur.
    if !state.agent_direct_tool_execution_enabled {
        return Err(ApiError::not_found(
            "direct tool execution is disabled; submit work to the durable execution service",
        ));
    }
    tracing::warn!(
        user_id = %auth.user_id,
        organization_id = %auth.organization_id,
        "direct server-side code execution invoked"
    );
    let conv_id = request
        .get("conversationId")
        .or_else(|| request.get("conversation_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    let tool_req = ToolExecutionRequest::new(
        Uuid::new_v4(),
        conv_id,
        aro_core::TOOL_CORE_CODE_EXECUTE,
        request,
    );
    let policy = WebAccessPolicy::unrestricted();
    let result = state
        .tools
        .execute(tool_req, &policy)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(result))
}

pub async fn tools_document_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<Value>,
) -> Result<Json<ToolExecutionResult>, ApiError> {
    if !state.agent_direct_tool_execution_enabled {
        return Err(ApiError::not_found(
            "direct tool execution is disabled; submit work to the durable execution service",
        ));
    }
    tracing::warn!(
        user_id = %auth.user_id,
        organization_id = %auth.organization_id,
        "direct server-side document creation invoked"
    );
    let conv_id = request
        .get("conversationId")
        .or_else(|| request.get("conversation_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    let tool_req = ToolExecutionRequest::new(
        Uuid::new_v4(),
        conv_id,
        aro_core::TOOL_CORE_DOCUMENT_CREATE,
        request,
    );
    let policy = WebAccessPolicy::unrestricted();
    let result = state
        .tools
        .execute(tool_req, &policy)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUploadCreateRequest {
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUploadCreateResponse {
    pub file: FileObject,
    pub upload: FileUploadSession,
    pub upload_url: String,
}

#[derive(Debug, Deserialize)]
pub struct FilesListQuery {
    pub limit: Option<i64>,
}

pub async fn files_storage_quota(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    let org_id = auth.organization_id;
    let usage_bytes = state.store.get_organization_storage_usage(org_id).await?;
    let quota_limit_bytes: i64 = 5 * 1024 * 1024 * 1024; // 5 GB default

    Ok(Json(json!({
        "organizationId": org_id,
        "usedBytes": usage_bytes,
        "usedMegabytes": usage_bytes / (1024 * 1024),
        "quotaLimitBytes": quota_limit_bytes,
        "quotaLimitMegabytes": quota_limit_bytes / (1024 * 1024),
        "usagePercentage": (usage_bytes as f64 / quota_limit_bytes as f64) * 100.0
    })))
}

pub async fn files_quarantined_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<FileObject>>, ApiError> {
    let org_id = auth.organization_id;
    let files = state.store.list_quarantined_files(org_id).await?;
    Ok(Json(files))
}

pub async fn file_upload_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<FileUploadCreateRequest>,
) -> Result<Json<FileUploadCreateResponse>, ApiError> {
    ensure_files_enabled(&state)?;
    let original_name = request.original_name.trim().to_string();
    if original_name.is_empty() {
        return Err(ApiError::bad_request("originalName is required"));
    }
    if request.size_bytes < 0 {
        return Err(ApiError::bad_request("sizeBytes cannot be negative"));
    }
    if request.size_bytes as u64 > state.file_settings.max_bytes {
        return Err(ApiError::payload_too_large(format!(
            "file exceeds max size of {} bytes",
            state.file_settings.max_bytes
        )));
    }
    let mime_type = normalize_mime(&request.mime_type);
    if !state.file_settings.is_allowed_mime(&mime_type) {
        return Err(ApiError::bad_request("mimeType is not allowed"));
    }
    if let Some(sha256) = &request.sha256 {
        if !is_sha256_hex(sha256) {
            return Err(ApiError::bad_request(
                "sha256 must be a lowercase hex digest",
            ));
        }
    }

    let file_id = Uuid::new_v4();
    let object_key = storage_key(auth.organization_id, file_id);
    let prepared = state
        .store
        .create_file_upload_session(
            auth.user_id,
            auth.organization_id,
            NewFileUpload {
                file_id,
                original_name,
                mime_type,
                size_bytes: request.size_bytes,
                sha256: request.sha256,
                storage_backend: state.file_storage.backend().to_string(),
                bucket: state.file_storage.bucket().map(str::to_string),
                object_key,
                expires_at: Utc::now() + ChronoDuration::minutes(30),
            },
        )
        .await?;

    Ok(Json(FileUploadCreateResponse {
        upload_url: format!("/files/uploads/{}/content", prepared.upload.id),
        file: prepared.file,
        upload: prepared.upload,
    }))
}

pub async fn file_upload_content(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(upload_id): Path<Uuid>,
    body: Bytes,
) -> Result<Json<FileObject>, ApiError> {
    ensure_files_enabled(&state)?;
    if body.len() as u64 > state.file_settings.max_bytes {
        return Err(ApiError::payload_too_large(format!(
            "file exceeds max size of {} bytes",
            state.file_settings.max_bytes
        )));
    }
    let target = state
        .store
        .get_file_upload_target(auth.user_id, auth.organization_id, upload_id)
        .await?;
    if target.upload.status != FileStatus::Pending {
        return Err(ApiError::bad_request("upload session is not pending"));
    }
    if target.upload.expires_at < Utc::now() {
        return Err(ApiError::bad_request("upload session expired"));
    }
    if target.upload.expected_size_bytes != body.len() as i64 {
        return Err(ApiError::bad_request(
            "uploaded size does not match session",
        ));
    }

    let sha256 = sha256_hex(&body);
    if let Some(expected_sha256) = target.upload.expected_sha256.as_ref() {
        if expected_sha256 != &sha256 {
            return Err(ApiError::bad_request(
                "uploaded checksum does not match session",
            ));
        }
    }

    let sniffed_mime = sniff_mime(&body, &target.upload.expected_mime_type);
    if !state.file_settings.is_allowed_mime(&sniffed_mime) {
        return Err(ApiError::bad_request("detected mimeType is not allowed"));
    }
    if !mime_matches_declared(&target.upload.expected_mime_type, &sniffed_mime) {
        return Err(ApiError::bad_request(
            "detected mimeType does not match session",
        ));
    }

    let stored = state.file_storage.put(&target.object_key, &body).await?;
    let mut file = state
        .store
        .complete_file_upload_session(
            auth.user_id,
            auth.organization_id,
            upload_id,
            stored.size_bytes,
            stored.sha256,
            sniffed_mime,
            stored.etag,
        )
        .await?;

    let scan_required = state.deployment.environment == "production"
        || std::env::var("ARO_FILE_SCAN_REQUIRED")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false)
        || std::env::var("ARO_CLAMAV_ADDRESS")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);

    if !scan_required {
        if let Ok(Some(approved)) = state
            .store
            .auto_approve_file_scan(auth.organization_id, file.id)
            .await
        {
            file = approved;
        }
    }

    Ok(Json(file))
}

pub async fn files_list(
    State(state): State<ApiState>,
    auth: AuthContext,
    Query(query): Query<FilesListQuery>,
) -> Result<Json<Vec<FileObject>>, ApiError> {
    ensure_files_enabled(&state)?;
    Ok(Json(
        state
            .store
            .list_files(
                auth.user_id,
                auth.organization_id,
                query.limit.unwrap_or(50),
            )
            .await?,
    ))
}

pub async fn file_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(file_id): Path<Uuid>,
) -> Result<Json<FileObject>, ApiError> {
    ensure_files_enabled(&state)?;
    Ok(Json(
        state
            .store
            .get_file(auth.user_id, auth.organization_id, file_id)
            .await?,
    ))
}

pub async fn file_download(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(file_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    ensure_files_enabled(&state)?;
    let stored = state
        .store
        .get_stored_file(auth.user_id, auth.organization_id, file_id)
        .await?;
    if stored.file.status != FileStatus::Available {
        return Err(ApiError::bad_request("file is not available"));
    }
    let bytes = state.file_storage.get(&stored.object_key).await?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&stored.file.mime_type)
            .map_err(|_| ApiError::internal("invalid stored mimeType"))?,
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())
            .map_err(|_| ApiError::internal("invalid content length"))?,
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{}\"", stored.file.sha256))
            .map_err(|_| ApiError::internal("invalid etag"))?,
    );
    Ok((StatusCode::OK, headers, bytes))
}

pub async fn file_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(file_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    ensure_files_enabled(&state)?;
    let stored = state
        .store
        .delete_file(auth.user_id, auth.organization_id, file_id)
        .await?;
    state.file_storage.delete(&stored.object_key).await?;
    Ok(Json(json!({ "ok": true })))
}

fn ensure_files_enabled(state: &ApiState) -> Result<(), ApiError> {
    if state.file_settings.enabled {
        Ok(())
    } else {
        Err(ApiError::bad_request("files API is disabled"))
    }
}

fn normalize_mime(mime_type: &str) -> String {
    let normalized = mime_type.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        "application/octet-stream".to_string()
    } else {
        normalized
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub async fn agent_runs_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<AgentRunStartRequest>,
) -> Result<Json<AgentRunView>, ApiError> {
    if request.goal.trim().is_empty() {
        return Err(ApiError::bad_request("agent goal cannot be empty"));
    }
    let runtime = AgentRuntime::new();
    let lane = match request.lane_id {
        Some(lane_id) => state
            .store
            .get_agent_lane(auth.user_id, auth.organization_id, lane_id)
            .await?
            .ok_or_else(|| ApiError::bad_request("agent lane not found"))?,
        None => {
            let title = if let Some(conversation_id) = request.conversation_id {
                state
                    .store
                    .get_conversation(auth.tenant_context(), conversation_id)
                    .await?
                    .map(|conversation| conversation.title)
            } else {
                None
            }
            .unwrap_or_else(|| {
                request
                    .goal
                    .split_whitespace()
                    .take(8)
                    .collect::<Vec<_>>()
                    .join(" ")
            });
            state
                .store
                .ensure_agent_lane(
                    auth.user_id,
                    auth.organization_id,
                    request.conversation_id,
                    title,
                )
                .await?
        }
    };
    let mut request = request;
    request.lane_id = Some(lane.id);
    if request.priority.is_none() {
        request.priority = Some(lane.priority.clone());
    }
    if let Some(profile_id) = request.autonomy_profile_id {
        let profile_exists = state
            .store
            .list_agent_permission_profiles(auth.user_id, auth.organization_id)
            .await?
            .iter()
            .any(|profile| profile.id == profile_id);
        if !profile_exists {
            return Err(ApiError::forbidden(
                "the requested agent permission profile is unavailable in this tenant",
            ));
        }
    }
    let lock_key = state.redis.lock_key(
        "agent-lane",
        &[auth.organization_id.to_string(), lane.id.to_string()],
    );
    let lock = acquire_required_lock(
        &state,
        lock_key,
        30,
        "agent lane is already scheduling a run",
    )
    .await?;
    let result = async {
        let mut run = runtime.start_run(&request, request.autonomy_profile_id);
        let lanes = state
            .store
            .list_agent_lane_views(auth.user_id, auth.organization_id)
            .await?;
        let global_running = lanes.iter().map(|lane| lane.running_count).sum();
        let lane_running = state
            .store
            .count_agent_runs_for_lane(
                auth.user_id,
                auth.organization_id,
                lane.id,
                AgentRunStatus::Running,
            )
            .await?;
        runtime.schedule_run(&mut run, &lane, global_running, lane_running);
        run = state
            .store
            .upsert_agent_run(auth.user_id, auth.organization_id, &run)
            .await?;
        let started = runtime.run_started_step(&run);
        state
            .store
            .add_agent_step(auth.user_id, auth.organization_id, &started)
            .await?;

        let history = match run.conversation_id {
            Some(conversation_id) => state
                .store
                .list_messages(auth.tenant_context(), conversation_id)
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let all_memories = state
            .store
            .list_memories(auth.user_id, auth.organization_id)
            .await?;
        let lexical_memories = state
            .store
            .search_memories(auth.user_id, auth.organization_id, &run.goal, 16)
            .await?;
        let memory_sources = memories_to_context_sources(
            state
                .vector
                .retrieve(
                    &run.goal,
                    all_memories,
                    lexical_memories,
                    &memory_vector_scope(&auth),
                    8,
                )
                .await?
                .memories,
        );
        let recalled_ids = memory_sources
            .iter()
            .filter_map(|source| source.uri.as_deref())
            .filter_map(|uri| uri.strip_prefix("memory://"))
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect::<Vec<_>>();
        let mut context_sources = memory_sources;
        let enrichment =
            collect_web_context_for_run(&state, &auth, &run, &run.goal, WebAccessMode::Auto, 2, None)
                .await?;
        context_sources.extend(enrichment.sources);
        let context_pack = runtime.build_context_pack(
            &run,
            &history,
            &context_sources,
            &[],
            EnvironmentSnapshot::api_durable(request.provider.clone(), request.model_id.clone()),
        );
        state
            .store
            .touch_memories_used(auth.user_id, auth.organization_id, &recalled_ids)
            .await?;
        state
            .store
            .add_agent_step(
                auth.user_id,
                auth.organization_id,
                &runtime.context_step_at(&run, enrichment.next_sequence, &context_pack),
            )
            .await?;

        if let Some(ref keyring) = state.agent_keyring {
            let submission_key = format!("submit:{}", run.id);
            let snapshot = json!({
                "goal": run.goal,
                "conversation_id": run.conversation_id,
            });
            let budgets = aro_store::AgentRunJobBudgets::default();
            if let Err(err) = state
                .store
                .submit_agent_run_job(
                    auth.tenant_context(),
                    &run,
                    &submission_key,
                    &snapshot,
                    budgets,
                    keyring,
                )
                .await
            {
                tracing::warn!(?err, ?run.id, "failed to queue agent run job for background execution");
            }
        }

        Ok(Json(
            state
                .store
                .agent_run_view(auth.user_id, auth.organization_id, run.id)
                .await?
                .unwrap_or(AgentRunView {
                    run,
                    steps: vec![started],
                    artifacts: Vec::new(),
                    context_pack: Some(context_pack),
                }),
        ))
    }
    .await;
    release_redis_lock(lock).await;
    result
}

pub async fn agent_runs_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<AgentRun>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_agent_runs(auth.user_id, auth.organization_id)
            .await?,
    ))
}

pub async fn agent_lanes_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<AgentLaneView>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_agent_lane_views(auth.user_id, auth.organization_id)
            .await?,
    ))
}

pub async fn agent_permission_profiles_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<PermissionProfile>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_agent_permission_profiles(auth.user_id, auth.organization_id)
            .await?,
    ))
}

pub async fn agent_permission_profile_upsert(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(profile): Json<PermissionProfile>,
) -> Result<Json<PermissionProfile>, ApiError> {
    if profile.name.trim().is_empty() {
        return Err(ApiError::bad_request("permission profile name is required"));
    }
    Ok(Json(
        state
            .store
            .upsert_agent_permission_profile(auth.user_id, auth.organization_id, &profile)
            .await?,
    ))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCatalogQuery {
    capability: Option<String>,
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCatalogResponse {
    items: Vec<ToolDescriptor>,
    next_cursor: Option<String>,
}

pub async fn tools_list(
    State(state): State<ApiState>,
    auth: AuthContext,
    Query(query): Query<ToolCatalogQuery>,
) -> Result<Json<ToolCatalogResponse>, ApiError> {
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(ApiError::bad_request(
            "tool catalogue limit must be between 1 and 100",
        ));
    }
    if query
        .capability
        .as_ref()
        .is_some_and(|capability| capability.is_empty() || capability.len() > 200)
    {
        return Err(ApiError::bad_request(
            "tool capability filter must contain between 1 and 200 bytes",
        ));
    }
    let status = query.status.as_deref().map(parse_tool_status).transpose()?;
    let after = query
        .cursor
        .as_deref()
        .map(decode_tool_cursor)
        .transpose()?;

    let mut tools = effective_tool_catalog(&state, auth.tenant_context()).await?;
    tools.retain(|descriptor| {
        after
            .as_ref()
            .is_none_or(|cursor| descriptor.id.as_str() > cursor.as_str())
            && status.is_none_or(|status| descriptor.status == status)
            && query.capability.as_ref().is_none_or(|capability| {
                descriptor
                    .capabilities
                    .iter()
                    .any(|candidate| candidate == capability)
            })
    });
    let has_more = tools.len() > limit;
    tools.truncate(limit);
    let next_cursor = has_more
        .then(|| tools.last().map(|tool| encode_tool_cursor(&tool.id)))
        .flatten();
    Ok(Json(ToolCatalogResponse {
        items: tools,
        next_cursor,
    }))
}

pub async fn tool_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(tool_or_alias): Path<String>,
) -> Result<Json<ToolDescriptor>, ApiError> {
    if tool_or_alias.is_empty() || tool_or_alias.len() > 200 {
        return Err(ApiError::bad_request(
            "tool id or alias must contain between 1 and 200 bytes",
        ));
    }
    effective_tool_catalog(&state, auth.tenant_context())
        .await?
        .into_iter()
        .find(|descriptor| {
            descriptor.id == tool_or_alias
                || descriptor
                    .aliases
                    .iter()
                    .any(|alias| alias == &tool_or_alias)
        })
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tool not found"))
}

async fn effective_tool_catalog(
    state: &ApiState,
    context: TenantContext,
) -> Result<Vec<ToolDescriptor>, ApiError> {
    let persisted = state.store.list_current_tool_descriptors(context).await?;
    merge_tool_catalog(
        built_in_tool_descriptors(),
        persisted
            .into_iter()
            .map(|stored| stored.descriptor)
            .collect(),
    )
    .map_err(ApiError::from)
}

fn merge_tool_catalog(
    built_in: Vec<ToolDescriptor>,
    persisted: Vec<ToolDescriptor>,
) -> aro_core::AroResult<Vec<ToolDescriptor>> {
    let mut tools = BTreeMap::new();
    for descriptor in built_in.into_iter().chain(persisted) {
        descriptor.validate()?;
        tools.insert(descriptor.id.clone(), descriptor);
    }

    let mut names = BTreeMap::<String, String>::new();
    for descriptor in tools.values() {
        for name in std::iter::once(&descriptor.id).chain(descriptor.aliases.iter()) {
            if let Some(existing) = names.insert(name.clone(), descriptor.id.clone()) {
                if existing != descriptor.id {
                    return Err(aro_core::AroError::Unexpected(format!(
                        "tool catalogue name `{name}` resolves to both `{existing}` and `{}`",
                        descriptor.id
                    )));
                }
            }
        }
    }
    Ok(tools.into_values().collect())
}

fn parse_tool_status(value: &str) -> Result<ToolStatus, ApiError> {
    match value {
        "pending" => Ok(ToolStatus::Pending),
        "active" => Ok(ToolStatus::Active),
        "disabled" => Ok(ToolStatus::Disabled),
        "deprecated" => Ok(ToolStatus::Deprecated),
        "revoked" => Ok(ToolStatus::Revoked),
        _ => Err(ApiError::bad_request("invalid tool status filter")),
    }
}

fn encode_tool_cursor(tool_id: &str) -> String {
    URL_SAFE_NO_PAD.encode(tool_id.as_bytes())
}

fn decode_tool_cursor(cursor: &str) -> Result<String, ApiError> {
    if cursor.is_empty() || cursor.len() > 512 {
        return Err(ApiError::bad_request("invalid tool catalogue cursor"));
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| ApiError::bad_request("invalid tool catalogue cursor"))?;
    let decoded = String::from_utf8(decoded)
        .map_err(|_| ApiError::bad_request("invalid tool catalogue cursor"))?;
    if decoded.is_empty() || decoded.len() > 200 {
        return Err(ApiError::bad_request("invalid tool catalogue cursor"));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tool_catalog_tests {
    use super::*;

    #[test]
    fn persisted_descriptor_overrides_built_in_version() {
        let built_in = built_in_tool_descriptors();
        let mut override_descriptor = built_in[0].clone();
        override_descriptor.version = "1.1.0".to_string();
        override_descriptor.status = ToolStatus::Disabled;
        let merged = merge_tool_catalog(built_in, vec![override_descriptor.clone()]).unwrap();
        let resolved = merged
            .iter()
            .find(|descriptor| descriptor.id == override_descriptor.id)
            .unwrap();
        assert_eq!(resolved.version, "1.1.0");
        assert_eq!(resolved.status, ToolStatus::Disabled);
    }

    #[test]
    fn catalogue_rejects_cross_tool_alias_collisions() {
        let mut descriptors = built_in_tool_descriptors();
        let conflicting_id = descriptors[1].id.clone();
        descriptors[0].aliases.push(conflicting_id);
        let error = merge_tool_catalog(descriptors, Vec::new()).unwrap_err();
        assert!(error.to_string().contains("resolves to both"));
    }

    #[test]
    fn tool_cursor_round_trips_and_rejects_invalid_input() {
        let tool_id = "core.search.web";
        assert_eq!(
            decode_tool_cursor(&encode_tool_cursor(tool_id)).unwrap(),
            tool_id
        );
        assert!(decode_tool_cursor("***").is_err());
    }
}

pub async fn agent_lane_pause(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(lane_id): Path<Uuid>,
) -> Result<Json<AgentLaneView>, ApiError> {
    state
        .store
        .update_agent_lane_status(
            auth.user_id,
            auth.organization_id,
            lane_id,
            AgentLaneStatus::Paused,
        )
        .await?;
    let lanes = state
        .store
        .list_agent_lane_views(auth.user_id, auth.organization_id)
        .await?;
    lanes
        .into_iter()
        .find(|view| view.lane.id == lane_id)
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("agent lane not found"))
}

pub async fn agent_lane_resume(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(lane_id): Path<Uuid>,
) -> Result<Json<AgentLaneView>, ApiError> {
    state
        .store
        .update_agent_lane_status(
            auth.user_id,
            auth.organization_id,
            lane_id,
            AgentLaneStatus::Active,
        )
        .await?;
    let lanes = state
        .store
        .list_agent_lane_views(auth.user_id, auth.organization_id)
        .await?;
    lanes
        .into_iter()
        .find(|view| view.lane.id == lane_id)
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("agent lane not found"))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentLanePriorityRequest {
    pub priority: AgentRunPriority,
}

pub async fn agent_lane_priority(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(lane_id): Path<Uuid>,
    Json(request): Json<AgentLanePriorityRequest>,
) -> Result<Json<AgentLaneView>, ApiError> {
    state
        .store
        .update_agent_lane_priority(
            auth.user_id,
            auth.organization_id,
            lane_id,
            request.priority,
        )
        .await?;
    let lanes = state
        .store
        .list_agent_lane_views(auth.user_id, auth.organization_id)
        .await?;
    lanes
        .into_iter()
        .find(|view| view.lane.id == lane_id)
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("agent lane not found"))
}

pub async fn agent_run_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(run_id): Path<Uuid>,
) -> Result<Json<AgentRunView>, ApiError> {
    let view = state
        .store
        .agent_run_view(auth.user_id, auth.organization_id, run_id)
        .await?
        .ok_or_else(|| ApiError::bad_request("agent run not found"))?;
    Ok(Json(view))
}

pub async fn agent_run_pause(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(run_id): Path<Uuid>,
) -> Result<Json<AgentRun>, ApiError> {
    Ok(Json(
        state
            .store
            .update_agent_run_status(
                auth.user_id,
                auth.organization_id,
                run_id,
                AgentRunStatus::Paused,
                None,
            )
            .await?,
    ))
}

pub async fn agent_run_resume(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(run_id): Path<Uuid>,
) -> Result<Json<AgentRun>, ApiError> {
    let run = state
        .store
        .get_agent_run(auth.user_id, auth.organization_id, run_id)
        .await?
        .ok_or_else(|| ApiError::bad_request("agent run not found"))?;
    let lock_target = run.lane_id.unwrap_or(run.id);
    let lock_key = state.redis.lock_key(
        "agent-lane",
        &[auth.organization_id.to_string(), lock_target.to_string()],
    );
    let lock =
        acquire_required_lock(&state, lock_key, 30, "agent lane is already resuming a run").await?;
    let result = async {
        Ok(Json(
            state
                .store
                .update_agent_run_status(
                    auth.user_id,
                    auth.organization_id,
                    run_id,
                    AgentRunStatus::Running,
                    None,
                )
                .await?,
        ))
    }
    .await;
    release_redis_lock(lock).await;
    result
}

pub async fn agent_run_cancel(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(run_id): Path<Uuid>,
) -> Result<Json<AgentRun>, ApiError> {
    Ok(Json(
        state
            .store
            .update_agent_run_status(
                auth.user_id,
                auth.organization_id,
                run_id,
                AgentRunStatus::Cancelled,
                None,
            )
            .await?,
    ))
}

pub async fn agent_run_events(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(run_id): Path<Uuid>,
) -> Result<Json<Vec<aro_core::AgentEvent>>, ApiError> {
    Ok(Json(
        state
            .store
            .agent_events(auth.user_id, auth.organization_id, run_id)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextSearchQuery {
    pub q: String,
    pub limit: Option<i64>,
}

pub async fn agent_context_search(
    State(state): State<ApiState>,
    auth: AuthContext,
    Query(query): Query<AgentContextSearchQuery>,
) -> Result<Json<Vec<AgentContextItem>>, ApiError> {
    Ok(Json(
        state
            .store
            .search_agent_context(
                auth.user_id,
                auth.organization_id,
                &query.q,
                query.limit.unwrap_or(20),
            )
            .await?,
    ))
}

pub async fn settings_get(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<AppSettings>, ApiError> {
    Ok(Json(
        state
            .store
            .get_app_settings(auth.user_id, auth.organization_id)
            .await?,
    ))
}

pub async fn settings_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(settings): Json<AppSettings>,
) -> Result<Json<AppSettings>, ApiError> {
    Ok(Json(
        state
            .store
            .update_app_settings(auth.user_id, auth.organization_id, &settings)
            .await?,
    ))
}

pub async fn preferences_get(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<aro_core::UserPreferences>, ApiError> {
    Ok(Json(state.store.get_preferences(auth.user_id).await?))
}

pub async fn preferences_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(patch): Json<UserPreferencesPatch>,
) -> Result<Json<aro_core::UserPreferences>, ApiError> {
    Ok(Json(
        state.store.update_preferences(auth.user_id, patch).await?,
    ))
}

pub async fn devices_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<aro_core::Device>>, ApiError> {
    Ok(Json(state.store.list_devices_for_user(auth.user_id).await?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceUpsertRequest {
    pub id: Option<Uuid>,
    pub name: String,
    pub platform: String,
}

pub async fn device_upsert(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(request): Json<DeviceUpsertRequest>,
) -> Result<Json<aro_core::Device>, ApiError> {
    if request.name.trim().is_empty() || request.platform.trim().is_empty() {
        return Err(ApiError::bad_request(
            "device name and platform are required",
        ));
    }
    Ok(Json(
        state
            .store
            .upsert_device(auth.user_id, request.id, request.name, request.platform)
            .await?,
    ))
}

pub async fn client_state_list(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<BTreeMap<String, serde_json::Value>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_client_state(auth.user_id, auth.organization_id, Some(&state.secrets_key))
            .await?,
    ))
}

pub async fn client_state_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    validate_client_state_key(&key)?;
    let value = state
        .store
        .get_client_state(
            auth.user_id,
            auth.organization_id,
            key,
            Some(&state.secrets_key),
        )
        .await?
        .ok_or_else(|| ApiError::bad_request("client state key not found"))?;
    Ok(Json(value))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientStateSetRequest {
    pub value: serde_json::Value,
}

pub async fn client_state_set(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(key): Path<String>,
    Json(request): Json<ClientStateSetRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    validate_client_state_key(&key)?;
    state
        .store
        .set_client_state(
            auth.user_id,
            auth.organization_id,
            key,
            request.value,
            Some(&state.secrets_key),
        )
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn client_state_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    validate_client_state_key(&key)?;
    state
        .store
        .delete_client_state(auth.user_id, auth.organization_id, key)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn collection_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(collection): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let collection = PersistedCollection::from_slug(&collection)
        .ok_or_else(|| ApiError::bad_request("unknown collection"))?;
    Ok(Json(
        state
            .store
            .get_collection(auth.user_id, auth.organization_id, collection)
            .await?,
    ))
}

pub async fn collection_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(collection): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let collection = PersistedCollection::from_slug(&collection)
        .ok_or_else(|| ApiError::bad_request("unknown collection"))?;
    create_fixed_collection(state, auth, collection, payload).await
}

pub async fn collection_item_get(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path((collection, item_id)): Path<(String, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let collection = PersistedCollection::from_slug(&collection)
        .ok_or_else(|| ApiError::bad_request("unknown collection"))?;
    get_fixed_collection_item(state, auth, collection, item_id).await
}

pub async fn collection_item_update(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path((collection, item_id)): Path<(String, Uuid)>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let collection = PersistedCollection::from_slug(&collection)
        .ok_or_else(|| ApiError::bad_request("unknown collection"))?;
    update_fixed_collection_item(state, auth, collection, item_id, payload).await
}

pub async fn collection_item_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path((collection, item_id)): Path<(String, Uuid)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let collection = PersistedCollection::from_slug(&collection)
        .ok_or_else(|| ApiError::bad_request("unknown collection"))?;
    delete_fixed_collection_item(state, auth, collection, item_id).await
}

pub async fn collection_get_memories(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Vec<LongTermMemory>>, ApiError> {
    Ok(Json(
        state
            .store
            .list_memories(auth.user_id, auth.organization_id)
            .await?,
    ))
}

pub async fn collection_create_memories(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<LongTermMemory>, ApiError> {
    let value = state
        .store
        .create_collection_item(
            auth.user_id,
            auth.organization_id,
            PersistedCollection::Memories,
            payload,
            Some(&state.secrets_key),
        )
        .await?;
    let memory = memory_from_value(value)?;
    state
        .vector
        .index_memory(&memory, &memory_vector_scope(&auth))
        .await?;
    Ok(Json(memory))
}

pub async fn collection_item_get_memories(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<LongTermMemory>, ApiError> {
    let value = state
        .store
        .get_collection_item(
            auth.user_id,
            auth.organization_id,
            PersistedCollection::Memories,
            item_id,
        )
        .await?
        .ok_or_else(|| ApiError::bad_request("memory not found"))?;
    Ok(Json(memory_from_value(value)?))
}

pub async fn collection_item_update_memories(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<LongTermMemory>, ApiError> {
    let value = state
        .store
        .update_collection_item(
            auth.user_id,
            auth.organization_id,
            PersistedCollection::Memories,
            item_id,
            payload,
            Some(&state.secrets_key),
        )
        .await?;
    let memory = memory_from_value(value)?;
    state
        .vector
        .index_memory(&memory, &memory_vector_scope(&auth))
        .await?;
    Ok(Json(memory))
}

pub async fn collection_item_delete_memories(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let scope = memory_vector_scope(&auth);
    state
        .store
        .delete_collection_item(
            auth.user_id,
            auth.organization_id,
            PersistedCollection::Memories,
            item_id,
        )
        .await?;
    state.vector.delete_memory(item_id, &scope).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchQuery {
    pub q: Option<String>,
    pub limit: Option<i64>,
}

pub async fn memories_search(
    State(state): State<ApiState>,
    auth: AuthContext,
    Query(query): Query<MemorySearchQuery>,
) -> Result<Json<Vec<LongTermMemory>>, ApiError> {
    let q = query.q.unwrap_or_default();
    let limit = query.limit.unwrap_or(8).clamp(1, 50) as usize;
    let all_memories = state
        .store
        .list_memories(auth.user_id, auth.organization_id)
        .await?;
    let lexical_memories = state
        .store
        .search_memories(
            auth.user_id,
            auth.organization_id,
            &q,
            (limit.saturating_mul(2)) as i64,
        )
        .await?;
    Ok(Json(
        state
            .vector
            .retrieve(
                &q,
                all_memories,
                lexical_memories,
                &memory_vector_scope(&auth),
                limit,
            )
            .await?
            .memories,
    ))
}

pub async fn memory_index_status(
    State(state): State<ApiState>,
    _auth: AuthContext,
) -> Result<Json<aro_vector::MemoryIndexStatus>, ApiError> {
    Ok(Json(state.vector.status().await?))
}

pub async fn memory_index_reindex(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<aro_vector::MemoryReindexReport>, ApiError> {
    let memories = state
        .store
        .list_memories(auth.user_id, auth.organization_id)
        .await?;
    Ok(Json(
        state
            .vector
            .reindex(&memories, &memory_vector_scope(&auth))
            .await?,
    ))
}

fn memory_from_value(value: serde_json::Value) -> Result<LongTermMemory, ApiError> {
    serde_json::from_value(value).map_err(|err| ApiError::bad_request(err.to_string()))
}

fn memory_vector_scope(auth: &AuthContext) -> MemoryVectorScope {
    MemoryVectorScope::cloud(auth.organization_id, auth.user_id)
}

pub async fn collection_get_skills(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection(state, auth, PersistedCollection::Skills).await
}

pub async fn collection_create_skills(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    create_fixed_collection(state, auth, PersistedCollection::Skills, payload).await
}

pub async fn collection_item_get_skills(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection_item(state, auth, PersistedCollection::Skills, item_id).await
}

pub async fn collection_item_update_skills(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    update_fixed_collection_item(state, auth, PersistedCollection::Skills, item_id, payload).await
}

pub async fn collection_item_delete_skills(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    delete_fixed_collection_item(state, auth, PersistedCollection::Skills, item_id).await
}

pub async fn collection_get_plugins(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection(state, auth, PersistedCollection::PluginConnections).await
}

pub async fn collection_create_plugins(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    create_fixed_collection(state, auth, PersistedCollection::PluginConnections, payload).await
}

pub async fn collection_item_get_plugins(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection_item(state, auth, PersistedCollection::PluginConnections, item_id).await
}

pub async fn collection_item_update_plugins(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    update_fixed_collection_item(
        state,
        auth,
        PersistedCollection::PluginConnections,
        item_id,
        payload,
    )
    .await
}

pub async fn collection_item_delete_plugins(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    delete_fixed_collection_item(state, auth, PersistedCollection::PluginConnections, item_id).await
}

pub async fn collection_get_mcp(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection(state, auth, PersistedCollection::McpServers).await
}

pub async fn collection_create_mcp(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    create_fixed_collection(state, auth, PersistedCollection::McpServers, payload).await
}

pub async fn collection_item_get_mcp(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection_item(state, auth, PersistedCollection::McpServers, item_id).await
}

pub async fn collection_item_update_mcp(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    update_fixed_collection_item(
        state,
        auth,
        PersistedCollection::McpServers,
        item_id,
        payload,
    )
    .await
}

pub async fn collection_item_delete_mcp(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    delete_fixed_collection_item(state, auth, PersistedCollection::McpServers, item_id).await
}

pub async fn collection_get_hooks(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection(state, auth, PersistedCollection::Hooks).await
}

pub async fn collection_create_hooks(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    create_fixed_collection(state, auth, PersistedCollection::Hooks, payload).await
}

pub async fn collection_item_get_hooks(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection_item(state, auth, PersistedCollection::Hooks, item_id).await
}

pub async fn collection_item_update_hooks(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    update_fixed_collection_item(state, auth, PersistedCollection::Hooks, item_id, payload).await
}

pub async fn collection_item_delete_hooks(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    delete_fixed_collection_item(state, auth, PersistedCollection::Hooks, item_id).await
}

pub async fn collection_get_scheduled_tasks(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection(state, auth, PersistedCollection::ScheduledTasks).await
}

pub async fn collection_create_scheduled_tasks(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    create_fixed_collection(state, auth, PersistedCollection::ScheduledTasks, payload).await
}

pub async fn collection_item_get_scheduled_tasks(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection_item(state, auth, PersistedCollection::ScheduledTasks, item_id).await
}

pub async fn collection_item_update_scheduled_tasks(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    update_fixed_collection_item(
        state,
        auth,
        PersistedCollection::ScheduledTasks,
        item_id,
        payload,
    )
    .await
}

pub async fn collection_item_run_scheduled_task(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
    payload: Option<Json<serde_json::Value>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let payload = payload
        .map(|Json(payload)| payload)
        .unwrap_or_else(|| json!({}));
    Ok(Json(
        state
            .store
            .run_scheduled_task(auth.user_id, auth.organization_id, item_id, payload)
            .await?,
    ))
}

pub async fn collection_item_delete_scheduled_tasks(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    delete_fixed_collection_item(state, auth, PersistedCollection::ScheduledTasks, item_id).await
}

pub async fn collection_get_teams(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection(state, auth, PersistedCollection::Teams).await
}

pub async fn collection_create_teams(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    create_fixed_collection(state, auth, PersistedCollection::Teams, payload).await
}

pub async fn collection_item_get_teams(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection_item(state, auth, PersistedCollection::Teams, item_id).await
}

pub async fn collection_item_update_teams(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    update_fixed_collection_item(state, auth, PersistedCollection::Teams, item_id, payload).await
}

pub async fn collection_item_delete_teams(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    delete_fixed_collection_item(state, auth, PersistedCollection::Teams, item_id).await
}

pub async fn collection_get_usage(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection(state, auth, PersistedCollection::UsageEvents).await
}

pub async fn collection_create_usage(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    create_fixed_collection(state, auth, PersistedCollection::UsageEvents, payload).await
}

pub async fn collection_item_get_usage(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    get_fixed_collection_item(state, auth, PersistedCollection::UsageEvents, item_id).await
}

pub async fn collection_item_delete_usage(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(item_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    delete_fixed_collection_item(state, auth, PersistedCollection::UsageEvents, item_id).await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxPendingQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminEventStats {
    pub audit_count: i64,
    pub outbox_count: i64,
}

pub async fn admin_event_stats(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<AdminEventStats>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let audit_count = state
        .store
        .count_audit_events_for_org(auth.organization_id)
        .await?;
    let outbox_count = state
        .store
        .count_outbox_events_for_org(auth.organization_id)
        .await?;
    Ok(Json(AdminEventStats {
        audit_count,
        outbox_count,
    }))
}

pub async fn admin_outbox_pending(
    State(state): State<ApiState>,
    auth: AuthContext,
    Query(query): Query<OutboxPendingQuery>,
) -> Result<Json<Vec<aro_store::OutboxEvent>>, ApiError> {
    let events = state
        .store
        .list_pending_outbox_events(
            auth.user_id,
            auth.organization_id,
            query.limit.unwrap_or(50),
        )
        .await?;
    Ok(Json(events))
}

pub async fn admin_outbox_mark_processed(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(event_id): Path<Uuid>,
) -> Result<Json<aro_store::OutboxEvent>, ApiError> {
    let event = state
        .store
        .mark_outbox_event_processed(auth.user_id, auth.organization_id, event_id)
        .await?
        .ok_or_else(|| ApiError::bad_request("outbox event not found"))?;
    Ok(Json(event))
}

async fn get_fixed_collection(
    state: ApiState,
    auth: AuthContext,
    collection: PersistedCollection,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .store
            .get_collection(auth.user_id, auth.organization_id, collection)
            .await?,
    ))
}

async fn create_fixed_collection(
    state: ApiState,
    auth: AuthContext,
    collection: PersistedCollection,
    payload: serde_json::Value,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .store
            .create_collection_item(
                auth.user_id,
                auth.organization_id,
                collection,
                payload,
                Some(&state.secrets_key),
            )
            .await?,
    ))
}

async fn get_fixed_collection_item(
    state: ApiState,
    auth: AuthContext,
    collection: PersistedCollection,
    item_id: Uuid,
) -> Result<Json<serde_json::Value>, ApiError> {
    let item = state
        .store
        .get_collection_item(auth.user_id, auth.organization_id, collection, item_id)
        .await?
        .ok_or_else(|| ApiError::bad_request("collection item not found"))?;
    Ok(Json(item))
}

async fn update_fixed_collection_item(
    state: ApiState,
    auth: AuthContext,
    collection: PersistedCollection,
    item_id: Uuid,
    payload: serde_json::Value,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .store
            .update_collection_item(
                auth.user_id,
                auth.organization_id,
                collection,
                item_id,
                payload,
                Some(&state.secrets_key),
            )
            .await?,
    ))
}

async fn delete_fixed_collection_item(
    state: ApiState,
    auth: AuthContext,
    collection: PersistedCollection,
    item_id: Uuid,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .store
        .delete_collection_item(auth.user_id, auth.organization_id, collection, item_id)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

fn ensure_integrations_api_enabled(state: &ApiState) -> Result<(), ApiError> {
    if state.integration_flags.api_v2 {
        Ok(())
    } else {
        Err(ApiError::not_found("integrations API is disabled"))
    }
}

fn ensure_integrations_v3_enabled(state: &ApiState) -> Result<(), ApiError> {
    if state.integration_flags.api_v3 {
        Ok(())
    } else {
        Err(ApiError::not_found("integrations v3 API is disabled"))
    }
}

fn ensure_oauth_connect_enabled(state: &ApiState) -> Result<(), ApiError> {
    if state.integration_flags.oauth_connect {
        Ok(())
    } else {
        Err(ApiError::not_found("OAuth connections are disabled"))
    }
}

async fn begin_authorization_attempt_v3(
    state: &ApiState,
    auth: &AuthContext,
    provider_id: String,
    request: AuthorizationAttemptRequest,
    installation_id: Option<Uuid>,
    idempotency_key: String,
) -> Result<AuthorizationAttemptResponse, ApiError> {
    if !state.integration_flags.third_party_network {
        return Err(ApiError::not_found(
            "third-party integration authorization is disabled",
        ));
    }
    if provider_id.is_empty()
        || provider_id.len() > 128
        || !provider_id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_'))
    {
        return Err(ApiError::bad_request("provider ID is invalid"));
    }
    if !matches!(
        request.method,
        AuthMethod::AuthorizationCodePkce | AuthMethod::OpenIdConnect | AuthMethod::AppInstallation
    ) {
        return Err(ApiError::bad_request(
            "authorization method does not use a browser redirect",
        ));
    }
    let base = state
        .deployment
        .public_base_url
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("ARO_PUBLIC_BASE_URL is required"))?
        .trim_end_matches('/');
    let callback_uri = format!("{base}/v1/integration-oauth/callback/{provider_id}");
    let parsed_callback = Url::parse(&callback_uri)
        .map_err(|_| ApiError::internal("canonical integration callback is invalid"))?;
    if parsed_callback.query().is_some()
        || parsed_callback.fragment().is_some()
        || (state.deployment.environment == "production" && parsed_callback.scheme() != "https")
    {
        return Err(ApiError::internal(
            "canonical integration callback is invalid",
        ));
    }

    let attempt_id = Uuid::new_v4();
    let state_token = Zeroizing::new(random_urlsafe_token(32));
    let nonce_token = Zeroizing::new(random_urlsafe_token(32));
    let pkce_verifier = Zeroizing::new(random_urlsafe_token(64));
    let expires_at = Utc::now() + ChronoDuration::minutes(10);
    let transaction = Zeroizing::new(
        serde_json::to_string(&AuthorizationTransaction {
            state: state_token.to_string(),
            nonce: nonce_token.to_string(),
            pkce_verifier: pkce_verifier.to_string(),
        })
        .map_err(|_| ApiError::internal("authorization transaction could not be encoded"))?,
    );
    let transaction_aad = AuthorizationAttemptAad {
        organization_id: auth.organization_id,
        attempt_id,
        provider_id: provider_id.clone(),
        environment: state.deployment.environment.clone(),
    };
    let envelope = state
        .envelope_crypto
        .encrypt_authorization_attempt(transaction.as_bytes(), &transaction_aad)
        .await
        .map_err(|_| ApiError::internal("authorization transaction encryption failed"))?;
    let owner_type = owner_type_name(request.owner.owner_type);
    let auth_method = auth_method_name(request.method);
    let request_hash = hash_secret(
        &serde_json::to_string(&json!({
            "providerId": provider_id,
            "owner": request.owner,
            "method": request.method,
            "capabilityIds": request.capability_ids,
            "installationId": installation_id,
        }))
        .map_err(|_| ApiError::bad_request("authorization request is invalid"))?,
    );
    let setup = state
        .store
        .create_authorization_attempt_v3(
            auth.tenant_context(),
            NewAuthorizationAttempt {
                attempt_id,
                installation_id,
                provider_id,
                owner_type: owner_type.into(),
                owner_user_id: request.owner.user_id,
                owner_team_id: request.owner.team_id,
                auth_method: auth_method.into(),
                capability_ids: request.capability_ids,
                state_digest: hash_secret(state_token.as_str()),
                nonce_digest: hash_secret(nonce_token.as_str()),
                algorithm: envelope.algorithm,
                key_id: envelope.key_id,
                key_version: envelope.key_version,
                nonce: envelope.nonce,
                encrypted_dek: envelope.encrypted_dek,
                ciphertext: envelope.ciphertext,
                callback_uri,
                environment: state.deployment.environment.clone(),
                idempotency_key,
                request_hash,
                expires_at,
            },
        )
        .await?;
    let stored_transaction = decrypt_authorization_transaction(state, &setup).await?;
    if hash_secret(&stored_transaction.state) != setup.state_digest
        || hash_secret(&stored_transaction.nonce) != setup.nonce_digest
    {
        return Err(ApiError::internal("authorization state validation failed"));
    }
    let engine = oauth_engine_from_setup(&setup)?;
    let context = authorization_context_from_setup(&setup, stored_transaction)?;
    let profile = OAuthClientProfile {
        client_id: setup.client_id.clone(),
        client_secret: None,
    };
    let authorization_url = engine
        .authorization_url(&context, &profile)
        .await
        .map_err(map_connector_error)?;
    Ok(AuthorizationAttemptResponse {
        attempt_id: setup.attempt_id,
        interaction: json!({
            "type": "browser_redirect",
            "url": authorization_url.as_str(),
        }),
        expires_at: setup.expires_at,
    })
}

async fn complete_oauth_v3_callback(
    state: ApiState,
    provider_id: String,
    callback: IntegrationOAuthV3Callback,
) -> Response {
    if !state.integration_flags.api_v3 || !state.integration_flags.third_party_network {
        return integration_callback_page(None, "failed");
    }
    if callback.state.is_empty() || callback.state.len() > 2048 {
        return integration_callback_page(None, "failed");
    }
    let claim_token = Uuid::new_v4();
    let claimed = match state
        .store
        .claim_authorization_attempt_v3(
            &hash_secret(&callback.state),
            &provider_id,
            claim_token,
            &state.deployment.environment,
        )
        .await
    {
        Ok(Some(claimed)) => claimed,
        Ok(None) => return integration_callback_page(None, "failed"),
        Err(error) => {
            tracing::warn!(provider_id, error = %error, "integration callback claim failed");
            return integration_callback_page(None, "failed");
        }
    };
    let attempt_id = claimed.setup.attempt_id;
    if claimed.setup.status != "processing" {
        return integration_callback_page(Some(attempt_id), &claimed.setup.status);
    }
    if claimed.claim_token != Some(claim_token) {
        return integration_callback_page(Some(attempt_id), "processing");
    }
    let context =
        match TenantContext::new(claimed.setup.actor_user_id, claimed.setup.organization_id) {
            Ok(context) => context,
            Err(_) => return integration_callback_page(Some(attempt_id), "failed"),
        };
    if let Some(provider_error) = callback.error.as_deref() {
        let (status, code) = if provider_error == "access_denied" {
            ("denied", "access_denied")
        } else {
            ("failed", normalize_provider_callback_error(provider_error))
        };
        let _ = state
            .store
            .fail_authorization_attempt_v3(
                context,
                attempt_id,
                claim_token,
                status,
                code,
                "Provider rejected the authorization request",
            )
            .await;
        return integration_callback_page(Some(attempt_id), status);
    }
    let Some(code) = callback
        .code
        .as_deref()
        .filter(|code| !code.trim().is_empty() && code.len() <= 4096)
    else {
        let _ = state
            .store
            .fail_authorization_attempt_v3(
                context,
                attempt_id,
                claim_token,
                "failed",
                "protocol_error",
                "Authorization code is missing",
            )
            .await;
        return integration_callback_page(Some(attempt_id), "failed");
    };
    let transaction = match decrypt_authorization_transaction(&state, &claimed.setup).await {
        Ok(transaction)
            if constant_time_equal(transaction.state.as_bytes(), callback.state.as_bytes())
                && hash_secret(&transaction.nonce) == claimed.setup.nonce_digest =>
        {
            transaction
        }
        _ => {
            let _ = state
                .store
                .fail_authorization_attempt_v3(
                    context,
                    attempt_id,
                    claim_token,
                    "failed",
                    "protocol_error",
                    "Authorization state validation failed",
                )
                .await;
            return integration_callback_page(Some(attempt_id), "failed");
        }
    };
    let engine = match oauth_engine_from_setup(&claimed.setup) {
        Ok(engine) => engine,
        Err(_) => {
            let _ = state
                .store
                .fail_authorization_attempt_v3(
                    context,
                    attempt_id,
                    claim_token,
                    "failed",
                    "misconfigured",
                    "Connector authorization is misconfigured",
                )
                .await;
            return integration_callback_page(Some(attempt_id), "failed");
        }
    };
    let authorization_context = match authorization_context_from_setup(&claimed.setup, transaction)
    {
        Ok(context) => context,
        Err(_) => {
            let _ = state
                .store
                .fail_authorization_attempt_v3(
                    context,
                    attempt_id,
                    claim_token,
                    "failed",
                    "protocol_error",
                    "Authorization transaction is invalid",
                )
                .await;
            return integration_callback_page(Some(attempt_id), "failed");
        }
    };
    let profile = match oauth_client_profile(&claimed.setup) {
        Ok(profile) => profile,
        Err(_) => {
            let _ = state
                .store
                .fail_authorization_attempt_v3(
                    context,
                    attempt_id,
                    claim_token,
                    "failed",
                    "misconfigured",
                    "OAuth client secret reference is unavailable",
                )
                .await;
            return integration_callback_page(Some(attempt_id), "failed");
        }
    };
    let exchange = match engine
        .exchange_code(
            &authorization_context,
            &profile,
            code,
            callback.iss.as_deref(),
        )
        .await
    {
        Ok(exchange) => exchange,
        Err(error) => {
            let error_code = connector_error_code_name(error.code);
            let _ = state
                .store
                .fail_authorization_attempt_v3(
                    context,
                    attempt_id,
                    claim_token,
                    "failed",
                    error_code,
                    "Provider authorization exchange failed",
                )
                .await;
            return integration_callback_page(Some(attempt_id), "failed");
        }
    };
    let installation_id = claimed.setup.installation_id.unwrap_or_else(Uuid::new_v4);
    let credential_aad = CredentialAad {
        organization_id: claimed.setup.organization_id,
        installation_id,
        provider_id: claimed.setup.provider_id.clone(),
        credential_version: claimed.setup.credential_version,
        environment: state.deployment.environment.clone(),
    };
    let credential_envelope = match state
        .envelope_crypto
        .encrypt(exchange.credential.secret.as_bytes(), &credential_aad)
        .await
    {
        Ok(envelope) => envelope,
        Err(_) => {
            if let Err(revocation_error) = engine
                .revoke_credential(&profile, &exchange.credential)
                .await
            {
                tracing::error!(
                    attempt_id = %attempt_id,
                    provider_id,
                    error_code = connector_error_code_name(revocation_error.code),
                    "authorization compensation revocation after encryption failure was not confirmed"
                );
            }
            let _ = state
                .store
                .fail_authorization_attempt_v3(
                    context,
                    attempt_id,
                    claim_token,
                    "failed",
                    "encryption_failed",
                    "Credential encryption failed",
                )
                .await;
            return integration_callback_page(Some(attempt_id), "failed");
        }
    };
    let external_identity = exchange.identity.map(|identity| {
        json!({
            "subject": identity.external_account_id,
            "displayName": identity.display_name,
            "realm": identity.realm,
            "avatarUrl": identity.avatar_url,
            "attributes": identity.details,
        })
    });
    let finalized = state
        .store
        .finalize_authorization_attempt_v3(
            context,
            NewAuthorizedInstallation {
                attempt_id,
                claim_token,
                installation_id,
                external_identity,
                credential_type: exchange.credential.credential_type.clone(),
                credential_version: claimed.setup.credential_version,
                algorithm: credential_envelope.algorithm,
                key_id: credential_envelope.key_id,
                key_version: credential_envelope.key_version,
                nonce: credential_envelope.nonce,
                encrypted_dek: credential_envelope.encrypted_dek,
                ciphertext: credential_envelope.ciphertext,
                granted_scopes: exchange.credential.granted_scopes.clone(),
                expires_at: exchange.credential.expires_at,
            },
        )
        .await;
    if let Err(error) = finalized {
        tracing::error!(attempt_id = %attempt_id, provider_id, error = %error, "authorization persistence failed after token exchange");
        if let Err(revocation_error) = engine
            .revoke_credential(&profile, &exchange.credential)
            .await
        {
            tracing::error!(
                attempt_id = %attempt_id,
                provider_id,
                error_code = connector_error_code_name(revocation_error.code),
                "authorization compensation revocation was not confirmed"
            );
        }
        let _ = state
            .store
            .fail_authorization_attempt_v3(
                context,
                attempt_id,
                claim_token,
                "failed",
                "persistence_failed",
                "Credential persistence failed after authorization",
            )
            .await;
        return integration_callback_page(Some(attempt_id), "failed");
    }
    integration_callback_page(Some(attempt_id), "authorized")
}

async fn decrypt_authorization_transaction(
    state: &ApiState,
    setup: &aro_store::AuthorizationAttemptSetup,
) -> Result<AuthorizationTransaction, ApiError> {
    let envelope = CredentialEnvelope {
        algorithm: setup.algorithm.clone(),
        key_id: setup.key_id.clone(),
        key_version: setup.key_version.clone(),
        nonce: setup.nonce.clone(),
        encrypted_dek: setup.encrypted_dek.clone(),
        ciphertext: setup.ciphertext.clone(),
    };
    let aad = AuthorizationAttemptAad {
        organization_id: setup.organization_id,
        attempt_id: setup.attempt_id,
        provider_id: setup.provider_id.clone(),
        environment: state.deployment.environment.clone(),
    };
    let plaintext = state
        .envelope_crypto
        .decrypt_authorization_attempt(&envelope, &aad)
        .await
        .map_err(|_| ApiError::internal("authorization transaction decryption failed"))?;
    serde_json::from_slice(&plaintext)
        .map_err(|_| ApiError::internal("authorization transaction is invalid"))
}

fn oauth_engine_from_setup(
    setup: &aro_store::AuthorizationAttemptSetup,
) -> Result<OAuthProtocolEngine, ApiError> {
    let config: OAuthProviderConfig = serde_json::from_value(
        setup
            .descriptor
            .get("oauth")
            .cloned()
            .ok_or_else(|| ApiError::bad_request("connector has no OAuth protocol manifest"))?,
    )
    .map_err(|_| ApiError::internal("connector OAuth protocol manifest is invalid"))?;
    if setup.auth_method == "open_id_connect" && config.issuer.is_none() {
        return Err(ApiError::internal("OIDC connector has no issuer"));
    }
    let allowed_hosts = setup
        .descriptor
        .get("allowedHosts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    OAuthProtocolEngine::new(config, allowed_hosts).map_err(map_connector_error)
}

fn authorization_context_from_setup(
    setup: &aro_store::AuthorizationAttemptSetup,
    transaction: AuthorizationTransaction,
) -> Result<AuthorizationContext, ApiError> {
    Ok(AuthorizationContext {
        attempt_id: setup.attempt_id,
        organization_id: setup.organization_id,
        actor_user_id: setup.actor_user_id,
        owner: OwnerRef {
            owner_type: parse_owner_type(&setup.owner_type)?,
            user_id: setup.owner_user_id,
            team_id: setup.owner_team_id,
        },
        method: parse_auth_method(&setup.auth_method)?,
        capabilities: setup.capability_ids.clone(),
        requested_scopes: setup.requested_scopes.clone(),
        callback_uri: Url::parse(&setup.callback_uri)
            .map_err(|_| ApiError::internal("stored callback URI is invalid"))?,
        state: transaction.state,
        nonce: transaction.nonce,
        pkce_verifier: Zeroizing::new(transaction.pkce_verifier),
    })
}

fn oauth_client_profile(
    setup: &aro_store::AuthorizationAttemptSetup,
) -> Result<OAuthClientProfile, ApiError> {
    let client_secret = match setup.client_secret_ref.as_deref() {
        None => None,
        Some(reference) => {
            let env_name = reference
                .strip_prefix("env:")
                .filter(|name| {
                    name.starts_with("ARO_")
                        && name.len() <= 128
                        && name
                            .chars()
                            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
                })
                .ok_or_else(|| ApiError::internal("OAuth client secret reference is invalid"))?;
            let value = std::env::var(env_name)
                .ok()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ApiError::internal("OAuth client secret is unavailable"))?;
            Some(Zeroizing::new(value))
        }
    };
    Ok(OAuthClientProfile {
        client_id: setup.client_id.clone(),
        client_secret,
    })
}

fn integration_callback_page(attempt_id: Option<Uuid>, status: &str) -> Response {
    let (safe_status, message) = match status {
        "authorized" => (
            "authorized",
            "Connexion autorisée. Vous pouvez revenir dans ARO.",
        ),
        "denied" => ("denied", "Autorisation refusée."),
        "expired" => ("expired", "Cette tentative de connexion a expiré."),
        "cancelled" => ("cancelled", "Cette tentative de connexion a été annulée."),
        "processing" => ("processing", "Connexion en cours de finalisation."),
        _ => ("failed", "La connexion n’a pas pu être finalisée."),
    };
    let deep_link = attempt_id.map(|attempt_id| {
        format!("aro://integrations/complete?attempt={attempt_id}&status={safe_status}")
    });
    let refresh = deep_link
        .as_ref()
        .map(|url| format!(r#"<meta http-equiv="refresh" content="0;url={url}">"#))
        .unwrap_or_default();
    let link = deep_link
        .as_ref()
        .map(|url| format!(r#"<p><a href="{url}">Revenir dans ARO</a></p>"#))
        .unwrap_or_default();
    let html = format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">{refresh}<title>ARO — Intégration</title></head><body><main><h1>ARO</h1><p>{message}</p>{link}</main></body></html>"
    );
    let mut response = Html(html).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn required_idempotency_key(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get(HeaderName::from_static("idempotency-key"))
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .map(str::to_string)
        .ok_or_else(|| ApiError::bad_request("Idempotency-Key is required"))
}

fn owner_type_name(owner_type: OwnerType) -> &'static str {
    match owner_type {
        OwnerType::User => "user",
        OwnerType::Team => "team",
        OwnerType::Organization => "organization",
    }
}

fn default_connector_realm() -> String {
    "default".to_string()
}

fn parse_owner_type(value: &str) -> Result<OwnerType, ApiError> {
    match value {
        "user" => Ok(OwnerType::User),
        "team" => Ok(OwnerType::Team),
        "organization" => Ok(OwnerType::Organization),
        _ => Err(ApiError::internal("stored integration owner is invalid")),
    }
}

fn auth_method_name(method: AuthMethod) -> &'static str {
    match method {
        AuthMethod::AuthorizationCodePkce => "authorization_code_pkce",
        AuthMethod::OpenIdConnect => "open_id_connect",
        AuthMethod::AppInstallation => "app_installation",
        AuthMethod::DeviceCode => "device_code",
        AuthMethod::ApiKey => "api_key",
        AuthMethod::PersonalAccessToken => "personal_access_token",
        AuthMethod::ServiceAccount => "service_account",
        AuthMethod::ClientCredentials => "client_credentials",
        AuthMethod::LocalCredential => "local_credential",
        AuthMethod::ExternalVault => "external_vault",
    }
}

fn parse_auth_method(value: &str) -> Result<AuthMethod, ApiError> {
    serde_json::from_value(Value::String(value.to_string()))
        .map_err(|_| ApiError::internal("stored authorization method is invalid"))
}

fn map_connector_error(error: aro_integrations::ConnectorError) -> ApiError {
    match error.code {
        ConnectorErrorCode::RateLimited => {
            ApiError::too_many_requests("provider rate limited the request")
        }
        ConnectorErrorCode::ProviderUnavailable => {
            ApiError::service_unavailable("provider is unavailable")
        }
        ConnectorErrorCode::Timeout => ApiError::gateway_timeout("provider request timed out"),
        ConnectorErrorCode::Misconfigured => ApiError::bad_request("connector is misconfigured"),
        _ => ApiError::bad_request("provider authorization request is invalid"),
    }
}

fn connector_error_code_name(code: ConnectorErrorCode) -> &'static str {
    match code {
        ConnectorErrorCode::AccessDenied => "access_denied",
        ConnectorErrorCode::InvalidGrant => "invalid_grant",
        ConnectorErrorCode::InteractionRequired => "interaction_required",
        ConnectorErrorCode::ConsentRequired => "consent_required",
        ConnectorErrorCode::InsufficientScope => "insufficient_scope",
        ConnectorErrorCode::CredentialsExpired => "credentials_expired",
        ConnectorErrorCode::RateLimited => "rate_limited",
        ConnectorErrorCode::ProviderUnavailable => "provider_unavailable",
        ConnectorErrorCode::Timeout => "timeout",
        ConnectorErrorCode::Misconfigured => "misconfigured",
        ConnectorErrorCode::ProtocolError => "protocol_error",
        ConnectorErrorCode::Revoked => "revoked",
    }
}

fn normalize_provider_callback_error(value: &str) -> &'static str {
    match value {
        "interaction_required" => "interaction_required",
        "consent_required" => "consent_required",
        "invalid_grant" => "invalid_grant",
        _ => "protocol_error",
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn object_or_empty(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        json!({})
    }
}

fn provider_oauth_config(provider: &Value) -> Result<&serde_json::Map<String, Value>, ApiError> {
    provider
        .get("auth")
        .and_then(|auth| auth.get("oauth"))
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::bad_request("integration provider does not support OAuth"))
}

fn oauth_string(oauth: &serde_json::Map<String, Value>, key: &str) -> Result<String, ApiError> {
    oauth
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::bad_request(format!("integration OAuth {key} is missing")))
}

fn oauth_string_array(oauth: &serde_json::Map<String, Value>, key: &str) -> Vec<String> {
    oauth
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn integration_oauth_client_id(provider_id: &str) -> Result<String, ApiError> {
    let suffix = provider_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let env_name = format!("ARO_INTEGRATION_{suffix}_CLIENT_ID");
    std::env::var(&env_name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("OAuth client ID is not configured"))
}

fn integration_oauth_client_secret(provider_id: &str) -> Result<String, ApiError> {
    let suffix = provider_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    let env_name = format!("ARO_INTEGRATION_{suffix}_CLIENT_SECRET");
    std::env::var(&env_name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApiError::bad_request("OAuth client secret is not configured"))
}

fn canonical_oauth_redirect_uri(
    state: &ApiState,
    oauth: &serde_json::Map<String, Value>,
) -> Result<String, ApiError> {
    let base = state
        .deployment
        .public_base_url
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("ARO_PUBLIC_BASE_URL is required for OAuth"))?;
    let redirect_path = oauth_string(oauth, "redirectPath")?;
    if !redirect_path.starts_with('/')
        || redirect_path.contains('?')
        || redirect_path.contains('#')
        || redirect_path.starts_with("//")
    {
        return Err(ApiError::internal(
            "integration OAuth redirect path is invalid",
        ));
    }
    let redirect_uri = format!("{base}{redirect_path}");
    let parsed = Url::parse(&redirect_uri)
        .map_err(|_| ApiError::internal("integration OAuth redirect URI is invalid"))?;
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(ApiError::internal(
            "integration OAuth redirect URI is invalid",
        ));
    }
    Ok(redirect_uri)
}

fn validated_oauth_endpoint(
    oauth: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, ApiError> {
    let value = oauth_string(oauth, key)?;
    let parsed = Url::parse(&value)
        .map_err(|_| ApiError::internal(format!("integration OAuth {key} is invalid")))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(ApiError::internal(format!(
            "integration OAuth {key} must be a credential-free HTTPS URL"
        )));
    }
    Ok(value)
}

fn optional_validated_oauth_endpoint(
    oauth: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ApiError> {
    if oauth
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        validated_oauth_endpoint(oauth, key).map(Some)
    } else {
        Ok(None)
    }
}

fn random_urlsafe_token(byte_len: usize) -> String {
    let mut bytes = vec![0_u8; byte_len];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn oauth_code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

struct WebContextEnrichment {
    sources: Vec<ContextSource>,
    summaries: Vec<String>,
    next_sequence: i32,
}

async fn create_immediate_api_run(
    state: &ApiState,
    auth: &AuthContext,
    runtime: &AgentRuntime,
    conversation_id: Uuid,
    goal: String,
    mode: AssistantMode,
    system_prompt: Option<String>,
) -> Result<AgentRun, ApiError> {
    let title = make_title(&goal);
    let lane = state
        .store
        .ensure_agent_lane(
            auth.user_id,
            auth.organization_id,
            Some(conversation_id),
            title,
        )
        .await?;
    let request = AgentRunStartRequest {
        run_id: None,
        lane_id: Some(lane.id),
        conversation_id: Some(conversation_id),
        goal,
        mode,
        system_prompt,
        model_id: None,
        provider: None,
        autonomy_profile_id: None,
        priority: Some(lane.priority.clone()),
        max_steps: None,
    };
    let mut run = runtime.start_run(&request, None);
    run.status = AgentRunStatus::Running;
    let run = state
        .store
        .upsert_agent_run(auth.user_id, auth.organization_id, &run)
        .await?;
    state
        .store
        .add_agent_step(
            auth.user_id,
            auth.organization_id,
            &runtime.run_started_step(&run),
        )
        .await?;
    Ok(run)
}

async fn collect_web_context_for_run(
    state: &ApiState,
    auth: &AuthContext,
    run: &AgentRun,
    input: &str,
    web_access: WebAccessMode,
    mut next_sequence: i32,
    search_override: Option<aro_core::settings::SearchSettings>,
) -> Result<WebContextEnrichment, ApiError> {
    if matches!(web_access, WebAccessMode::Off) {
        return Ok(WebContextEnrichment {
            sources: Vec::new(),
            summaries: Vec::new(),
            next_sequence,
        });
    }
    let mut policy = web_policy_for_api(state, web_access.clone());
    if !policy.allow_network {
        return Ok(WebContextEnrichment {
            sources: Vec::new(),
            summaries: Vec::new(),
            next_sequence,
        });
    }
    // Le composer peut surcharger la config de recherche (prioritaire),
    // sinon on retombe sur les réglages stockés de l'utilisateur.
    if let Some(search) = search_override {
        policy =
            policy.with_search_settings(Some(search.provider), search.api_key, search.endpoint);
    } else if let Ok(app_settings) = state
        .store
        .get_app_settings(auth.user_id, auth.organization_id)
        .await
    {
        policy = policy.with_search_settings(
            Some(app_settings.search.provider),
            app_settings.search.api_key,
            app_settings.search.endpoint,
        );
    }
    let mut sources = Vec::new();
    let mut summaries = Vec::new();
    let urls = extract_urls(input, 3);
    if urls.is_empty() {
        if matches!(web_access, WebAccessMode::On) || web_request_likely(input) {
            let request = ToolExecutionRequest::new(
                run.id,
                run.conversation_id,
                TOOL_CORE_SEARCH_WEB,
                json!(WebSearchRequest {
                    query: input.to_string(),
                    limit: Some(5),
                    freshness_days: None,
                    domains: Vec::new(),
                }),
            );
            let result =
                execute_api_tool(state, auth, run, request, &policy, next_sequence).await?;
            next_sequence += 1;
            summaries.push(result.summary.clone());
            sources.extend(result.context_sources);
        }
    } else {
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
            let result =
                execute_api_tool(state, auth, run, request, &policy, next_sequence).await?;
            next_sequence += 1;
            summaries.push(result.summary.clone());
            sources.extend(result.context_sources);
        }
    }

    Ok(WebContextEnrichment {
        sources,
        summaries,
        next_sequence,
    })
}

async fn execute_api_tool(
    state: &ApiState,
    auth: &AuthContext,
    run: &AgentRun,
    request: ToolExecutionRequest,
    policy: &WebAccessPolicy,
    sequence: i32,
) -> Result<ToolExecutionResult, ApiError> {
    let (authorization_request, authorization_decision) =
        authorize_api_tool(state, auth, run, &request, policy).await?;
    let permits_execution = authorization_decision.permits_execution();
    state
        .store
        .record_permission_evaluation(
            auth.tenant_context(),
            &authorization_request,
            &authorization_decision,
            Some(if permits_execution {
                "authorized_pending_execution"
            } else {
                "blocked_before_execution"
            }),
        )
        .await?;

    if !permits_execution {
        let result = blocked_tool_result(
            &request,
            authorization_decision
                .error_code
                .as_deref()
                .unwrap_or("permission_denied"),
            &authorization_decision.reason,
        );
        persist_api_tool_execution(state, auth, run, &request, &result, sequence).await?;
        return Ok(result);
    }

    let result = match state.tools.execute(request.clone(), policy).await {
        Ok(result) => result,
        Err(err) => failed_tool_result(&request, err.to_string()),
    };
    persist_api_tool_execution(state, auth, run, &request, &result, sequence).await?;
    Ok(result)
}

async fn authorize_api_tool(
    state: &ApiState,
    auth: &AuthContext,
    run: &AgentRun,
    request: &ToolExecutionRequest,
    web_policy: &WebAccessPolicy,
) -> Result<
    (
        PolicyAuthorizationRequest,
        aro_policy::AuthorizationDecision,
    ),
    ApiError,
> {
    let now = Utc::now();
    let ephemeral_agent_id = format!("agent-run:{}", run.id);
    let expires_at = now + ChronoDuration::minutes(5);
    let canonical_intent = serde_json::to_vec(&json!({
        "subjectId": ephemeral_agent_id.clone(),
        "action": "network.read",
        "resourceId": request.tool_id.clone(),
        "runId": run.id,
        "conversationId": run.conversation_id,
        "input": request.input.clone(),
    }))
    .map_err(|_| ApiError::internal("tool authorization intent could not be canonicalized"))?;
    let intent_digest = format!("sha256:{:x}", Sha256::digest(canonical_intent));
    let authorization_request = PolicyAuthorizationRequest {
        id: request.invocation_id,
        intent_digest,
        subject: SubjectRef {
            kind: SubjectKind::Agent,
            id: ephemeral_agent_id.clone(),
            actor_user_id: Some(auth.user_id),
            organization_id: auth.organization_id,
            parent_subject_id: None,
            attributes: Default::default(),
        },
        action: "network.read".to_string(),
        resource: ResourceRef {
            kind: "tool".to_string(),
            id: request.tool_id.clone(),
            organization_id: auth.organization_id,
            owner_id: None,
            classification: DataClassification::Internal,
            relations: Default::default(),
            attributes: Default::default(),
        },
        context: EvaluationContext {
            workspace_id: None,
            conversation_id: run.conversation_id,
            agent_id: Some(ephemeral_agent_id.clone()),
            run_id: Some(run.id),
            task_id: None,
            tool_id: Some(request.tool_id.clone()),
            plugin_id: None,
            skill_id: None,
            mcp_server_id: None,
            environment: state.deployment.environment.clone(),
            model_id: run.model_id.clone(),
            provider_id: run.model_provider_id.clone(),
            external_provider: true,
            risk: RiskLevel::Moderate,
            amount_minor: None,
            actions_already_used: 0,
            cost_minor: None,
            budget_already_used_minor: 0,
            device_id: None,
            ip_address: None,
            origin: "api.tool-execution".to_string(),
            attributes: Default::default(),
        },
        requested_scopes: vec!["network.read".to_string()],
        evidence: AuthorizationEvidence::default(),
        requested_at: request.requested_at,
        expires_at: Some(expires_at),
    };

    let mut policies = state
        .store
        .list_active_policy_rules(auth.tenant_context(), now)
        .await?;
    let legacy_constraints = DecisionConstraints {
        read_only: true,
        allowed_resource_ids: vec![request.tool_id.clone()],
        allowed_scopes: vec!["network.read".to_string()],
        allowed_tool_ids: vec![request.tool_id.clone()],
        allowed_environments: vec![state.deployment.environment.clone()],
        max_actions: Some(1),
        valid_until: Some(expires_at),
        external_transfer_allowed: true,
        audit_level: AuditLevel::Enhanced,
        ..DecisionConstraints::default()
    };
    let legacy_target = PolicyTarget {
        subject_ids: vec![ephemeral_agent_id],
        organization_ids: vec![auth.organization_id],
        actions: vec!["network.read".to_string()],
        resource_kinds: vec!["tool".to_string()],
        resource_ids: vec![request.tool_id.clone()],
        environments: vec![state.deployment.environment.clone()],
        tool_ids: vec![request.tool_id.clone()],
        ..PolicyTarget::default()
    };
    let legacy_allowed = web_policy.allow_network && !web_policy.allowed_domains.is_empty();
    policies.push(PolicyRule {
        id: if legacy_allowed {
            "legacy.agent-permission-profile.network-allow".to_string()
        } else {
            "legacy.agent-permission-profile.network-deny".to_string()
        },
        version: 1,
        layer: PolicyLayer::LegacyAdapter,
        priority: 0,
        effect: if legacy_allowed {
            PolicyEffect::Allow
        } else {
            PolicyEffect::Deny
        },
        target: legacy_target,
        constraints: legacy_constraints,
        reason: if legacy_allowed {
            "The legacy agent profile grants one read-only network tool call within its domain allowlist."
                .to_string()
        } else {
            "The effective legacy agent profile does not grant network access with a non-empty domain allowlist."
                .to_string()
        },
        valid_from: Some(now),
        valid_until: Some(expires_at),
        enabled: true,
    });

    let decision = PolicyEngine.evaluate(&authorization_request, &policies, now);
    Ok((authorization_request, decision))
}

async fn persist_api_tool_execution(
    state: &ApiState,
    auth: &AuthContext,
    run: &AgentRun,
    request: &ToolExecutionRequest,
    result: &ToolExecutionResult,
    sequence: i32,
) -> Result<(), ApiError> {
    let status = match result.status {
        ToolExecutionStatus::Completed => AgentStepStatus::Completed,
        ToolExecutionStatus::Running => AgentStepStatus::Running,
        ToolExecutionStatus::Failed | ToolExecutionStatus::Blocked => AgentStepStatus::Failed,
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
    state
        .store
        .add_agent_step(auth.user_id, auth.organization_id, &step)
        .await?;
    for artifact in &result.artifacts {
        state
            .store
            .add_agent_artifact(auth.user_id, auth.organization_id, artifact)
            .await?;
    }
    for item in result_context_items(result, run.conversation_id) {
        state
            .store
            .add_agent_context_item(auth.user_id, auth.organization_id, &item)
            .await?;
    }
    Ok(())
}

fn web_policy_for_api(_state: &ApiState, _web_access: WebAccessMode) -> WebAccessPolicy {
    // A streamed assistant request must never silently make a third-party network call.
    // Explicit, user-approved tool requests use `explicit_web_policy` below and carry a
    // persisted permission profile. Durable workers will replace this direct endpoint.
    WebAccessPolicy::disabled()
}

fn explicit_web_policy(
    state: &ApiState,
    permission_profile: &aro_core::PermissionProfile,
    web_access: WebAccessMode,
) -> WebAccessPolicy {
    if matches!(web_access, WebAccessMode::Off)
        || !state.agent_web_access_enabled
        || !state.agent_direct_tool_execution_enabled
    {
        WebAccessPolicy::disabled()
    } else {
        WebAccessPolicy::from_permission_profile(permission_profile)
    }
}

fn failed_tool_result(request: &ToolExecutionRequest, error: String) -> ToolExecutionResult {
    ToolExecutionResult {
        invocation_id: request.invocation_id,
        run_id: request.run_id,
        tool_id: request.tool_id.clone(),
        status: ToolExecutionStatus::Failed,
        title: format!("{} failed", request.tool_id),
        output: json!({ "error": error }),
        summary: error.clone(),
        context_sources: Vec::new(),
        artifacts: Vec::new(),
        error: Some(error),
        started_at: request.requested_at,
        finished_at: Utc::now(),
    }
}

fn blocked_tool_result(
    request: &ToolExecutionRequest,
    error_code: &str,
    reason: &str,
) -> ToolExecutionResult {
    ToolExecutionResult {
        invocation_id: request.invocation_id,
        run_id: request.run_id,
        tool_id: request.tool_id.clone(),
        status: ToolExecutionStatus::Blocked,
        title: format!("{} blocked by policy", request.tool_id),
        output: json!({ "errorCode": error_code, "reason": reason }),
        summary: reason.to_string(),
        context_sources: Vec::new(),
        artifacts: Vec::new(),
        error: Some(format!("{error_code}: {reason}")),
        started_at: request.requested_at,
        finished_at: Utc::now(),
    }
}

/// Sérialisation SSE infaillible (`Event::data` ne peut pas échouer,
/// contrairement à `json_data(...).expect(..)` qui paniquait le stream).
fn sse_json_data(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{\"error\":\"serialization\"}".to_string())
}

struct ServerGeneration {
    text: String,
    token_estimate: Option<u32>,
    model_label: String,
}

/// Génère la réponse assistant côté serveur pour `/assistant/stream` :
/// 1. provider actif s'il est local (mock/ollama/llama.cpp — aucune clé requise) ;
/// 2. sinon repli local-first : premier vrai modèle de chat trouvé sur
///    l'Ollama local (cohérent avec `fallbackPolicy: "local-first"`) ;
/// 3. sinon message d'erreur honnête et actionnable — jamais de fausse réponse.
#[allow(clippy::too_many_arguments)]
async fn generate_server_assistant_text(
    state: &ApiState,
    auth: &AuthContext,
    mode: &AssistantMode,
    system_prompt: &str,
    history: &[ChatMessage],
    user_input: &str,
    requested_model_id: Option<String>,
    requested_provider_id: Option<String>,
    on_chunk: &mut (dyn FnMut(String) + Send),
) -> ServerGeneration {
    let app_settings = match state
        .store
        .get_app_settings(auth.user_id, auth.organization_id)
        .await
    {
        Ok(settings) => settings,
        Err(err) => return ServerGeneration::unavailable(format!("app settings: {err}")),
    };
    let model_settings = app_settings.model.clone();

    let request_for = |settings: &ModelSettings| ModelGenerationRequest {
        mode: mode.clone(),
        system_prompt: system_prompt.to_string(),
        messages: history.to_vec(),
        user_input: user_input.to_string(),
        temperature: settings.temperature,
        max_tokens: settings.max_tokens,
        response_format: ModelResponseFormat::DirectText,
    };

    // 0. Surcharge explicite du composer : honorée uniquement si elle
    // pointe vers un provider local (les clés distantes restent sur le
    // desktop). Sinon, on retombe sur la résolution standard ci-dessous.
    if let Some(wanted_model) = requested_model_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let mut found = None;
        if let Some(pid) = requested_provider_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if let Some(conn) = model_settings.connection(pid) {
                found = Some((
                    conn.kind.clone(),
                    aro_core::ModelRef::new(pid, conn.kind.clone(), wanted_model, wanted_model),
                ));
            }
        } else {
            for conn in &model_settings.providers {
                if conn.models.iter().any(|m| m.model_id == wanted_model) {
                    found = Some((
                        conn.kind.clone(),
                        aro_core::ModelRef::new(
                            conn.id.clone(),
                            conn.kind.clone(),
                            wanted_model,
                            wanted_model,
                        ),
                    ));
                    break;
                }
            }
        }
        if let Some((kind, model_ref)) = found {
            if kind.is_local() {
                match ModelRouter::from_model_ref(&model_settings, &model_ref, None) {
                    Ok(router) => match router
                        .generate_stream(request_for(&model_settings), on_chunk)
                        .await
                    {
                        Ok(generation) if !generation.content.trim().is_empty() => {
                            return ServerGeneration {
                                text: generation.content,
                                token_estimate: generation.token_estimate,
                                model_label: wanted_model.to_string(),
                            }
                        }
                        Ok(_) => {}
                        Err(err) => {
                            tracing::warn!(
                                ?err,
                                "requested local model failed, using default resolution"
                            );
                        }
                    },
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            "requested model router unusable, using default resolution"
                        );
                    }
                }
            }
        }
    }

    // 1. Provider actif s'il est local : les clés API restent dans le
    // trousseau du desktop et ne sont jamais envoyées au serveur.
    if model_settings.active_model_ref.provider_kind.is_local() {
        let label = model_settings.active_model_ref.model_id.clone();
        match ModelRouter::from_active(&model_settings, None) {
            Ok(router) => match router
                .generate_stream(request_for(&model_settings), on_chunk)
                .await
            {
                Ok(generation) if !generation.content.trim().is_empty() => {
                    return ServerGeneration {
                        text: generation.content,
                        token_estimate: generation.token_estimate,
                        model_label: label,
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(?err, "active local provider failed, trying Ollama fallback");
                }
            },
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "active local provider unusable, trying Ollama fallback"
                );
            }
        }
    }

    // 2. Repli Ollama local : on interroge /api/tags et on prend le premier
    // modèle de chat réel (les embeddings seuls ne savent pas répondre).
    let ollama_endpoint = std::env::var("ARO_OLLAMA_ENDPOINT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| model_settings.ollama_endpoint.clone());
    if let Some(model_id) = discover_ollama_chat_model(&ollama_endpoint).await {
        let mut fallback = model_settings.clone();
        fallback.provider = ModelProviderKind::Ollama;
        fallback.model_id = model_id.clone();
        fallback.ollama_endpoint = ollama_endpoint;
        match LocalModelProvider::from_settings(&fallback) {
            Ok(provider) => match provider
                .generate_stream(request_for(&fallback), on_chunk)
                .await
            {
                Ok(generation) if !generation.content.trim().is_empty() => {
                    return ServerGeneration {
                        text: generation.content,
                        token_estimate: generation.token_estimate,
                        model_label: model_id,
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    let unavail = ServerGeneration::unavailable(format!("Ollama: {err}"));
                    on_chunk(unavail.text.clone());
                    return unavail;
                }
            },
            Err(err) => {
                let unavail = ServerGeneration::unavailable(format!("Ollama: {err}"));
                on_chunk(unavail.text.clone());
                return unavail;
            }
        }
    }

    // 3. Rien d'exploitable : on le dit clairement au lieu d'inventer.
    let active_label = model_settings.active_model_ref.model_id.clone();
    let unavail = ServerGeneration::unavailable(format!(
        "le provider actif ({active_label}) demande une clé API conservée sur le desktop, et aucun modèle de chat Ollama n'est joignable sur le serveur. Démarrez Ollama avec un modèle de chat (`ollama pull gemma3:1b`), ou utilisez l'application desktop."
    ));
    on_chunk(unavail.text.clone());
    unavail
}

/// Interroge `{endpoint}/api/tags` et retourne le premier modèle de chat
/// (on écarte les modèles d'embeddings seuls comme `nomic-embed-text`).
async fn discover_ollama_chat_model(endpoint: &str) -> Option<String> {
    let url = format!("{}/api/tags", endpoint.trim_end_matches('/'));
    let response = reqwest::Client::new()
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body: Value = response.json().await.ok()?;
    body.get("models")?.as_array()?.iter().find_map(|model| {
        let name = model.get("name")?.as_str()?;
        let lower = name.to_lowercase();
        if lower.contains("embed") {
            return None;
        }
        Some(name.to_string())
    })
}

impl ServerGeneration {
    fn unavailable(reason: String) -> Self {
        Self {
            text: format!("Je n'ai pas pu générer de réponse côté serveur : {reason}"),
            token_estimate: None,
            model_label: "unavailable".to_string(),
        }
    }
}

async fn issue_session(
    state: &ApiState,
    principal: aro_store::AuthPrincipal,
) -> Result<AuthSession, ApiError> {
    let refresh_token = generate_refresh_token();
    state
        .store
        .create_refresh_token(
            principal.user.id,
            principal.active_organization.id,
            None,
            &refresh_token,
            state.refresh_token_days,
            state.refresh_token_family_days,
        )
        .await?;
    build_session_with_refresh(state, principal, refresh_token)
}

async fn rotate_session(
    state: &ApiState,
    principal: aro_store::AuthPrincipal,
    current_refresh_token: &str,
    expected_user_id: Option<Uuid>,
) -> Result<AuthSession, ApiError> {
    let refresh_token = generate_refresh_token();
    let rotated = state
        .store
        .rotate_refresh_token(
            current_refresh_token,
            &refresh_token,
            Some(principal.active_organization.id),
            expected_user_id,
            state.refresh_token_days,
        )
        .await;
    match rotated {
        Ok(Some(_)) => {}
        Ok(None) | Err(aro_core::AroError::Security(_)) => {
            return Err(ApiError::unauthorized("invalid refresh token"));
        }
        Err(error) => return Err(error.into()),
    }
    build_session_with_refresh(state, principal, refresh_token)
}

fn build_session_with_refresh(
    state: &ApiState,
    principal: aro_store::AuthPrincipal,
    refresh_token: String,
) -> Result<AuthSession, ApiError> {
    let (access_token, expires_at) = state
        .jwt
        .issue(
            principal.user.id,
            principal.active_organization.id,
            state.access_token_minutes,
        )
        .map_err(|err| ApiError::internal(err.to_string()))?;
    Ok(AuthSession {
        access_token,
        refresh_token,
        expires_at,
        user: principal.user,
        active_organization: principal.active_organization,
        memberships: principal.memberships,
    })
}

fn map_invitation_auth_error(error: aro_core::AroError) -> ApiError {
    match error {
        aro_core::AroError::Security(_) => {
            ApiError::unauthorized("invalid account credentials or invitation")
        }
        other => other.into(),
    }
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| ApiError::internal(err.to_string()))
}

fn encode_invitation_cursor(invitation: &OrganizationInvitation) -> Result<String, ApiError> {
    let payload = serde_json::to_vec(&InvitationCursor {
        created_at: invitation.created_at.to_owned(),
        id: invitation.id,
    })
    .map_err(|_| ApiError::internal("could not encode invitation cursor"))?;
    Ok(URL_SAFE_NO_PAD.encode(payload))
}

fn decode_invitation_cursor(value: &str) -> Result<InvitationCursor, ApiError> {
    if value.is_empty() || value.len() > 512 {
        return Err(ApiError::bad_request("invalid invitation cursor"));
    }
    let payload = URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|_| ApiError::bad_request("invalid invitation cursor"))?;
    serde_json::from_slice(&payload).map_err(|_| ApiError::bad_request("invalid invitation cursor"))
}

fn invitation_ttl_hours() -> Result<i64, ApiError> {
    let hours = std::env::var("ARO_INVITATION_TTL_HOURS")
        .ok()
        .map(|value| {
            value.parse::<i64>().map_err(|_| {
                ApiError::internal("ARO_INVITATION_TTL_HOURS must be an integer between 1 and 168")
            })
        })
        .transpose()?
        .unwrap_or(168);
    if !(1..=168).contains(&hours) {
        return Err(ApiError::internal(
            "ARO_INVITATION_TTL_HOURS must be between 1 and 168",
        ));
    }
    Ok(hours)
}

fn verify_password(password: &str, password_hash: &str) -> Result<(), ApiError> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|_| ApiError::unauthorized("invalid email or password"))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::unauthorized("invalid email or password"))
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

fn validate_client_state_key(key: &str) -> Result<(), ApiError> {
    let valid = key.starts_with("aro-")
        && key.len() <= 120
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    if valid {
        Ok(())
    } else {
        Err(ApiError::bad_request("invalid client state key"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_cursors_are_opaque_bounded_and_round_trip() {
        let created_at = Utc::now();
        let invitation = OrganizationInvitation {
            id: Uuid::new_v4(),
            organization_id: Uuid::new_v4(),
            membership_id: None,
            email: "invitee@example.com".to_string(),
            name: "Invitee".to_string(),
            role: MembershipRole::Member,
            status: aro_core::OrganizationInvitationStatus::Pending,
            delivery_status: aro_core::InvitationDeliveryStatus::Pending,
            delivery_attempts: 0,
            expires_at: created_at + ChronoDuration::hours(1),
            delivered_at: None,
            created_at,
            updated_at: created_at,
        };
        let encoded = encode_invitation_cursor(&invitation).expect("encode cursor");
        let decoded = decode_invitation_cursor(&encoded).expect("decode cursor");
        assert_eq!(decoded.id, invitation.id);
        assert_eq!(decoded.created_at, invitation.created_at);
        assert!(decode_invitation_cursor("").is_err());
        assert!(decode_invitation_cursor(&"x".repeat(513)).is_err());
    }

    #[test]
    fn idempotency_keys_are_strict_visible_ascii() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "idempotency-key",
            HeaderValue::from_static("local-result_01.abc"),
        );
        assert_eq!(
            optional_idempotency_key(&headers).expect("valid key"),
            Some("local-result_01.abc".to_string())
        );

        headers.insert(
            "idempotency-key",
            HeaderValue::from_static(" leading-space"),
        );
        assert!(optional_idempotency_key(&headers).is_err());

        let mut headers = HeaderMap::new();
        headers.append("idempotency-key", HeaderValue::from_static("request-1"));
        headers.append("idempotency-key", HeaderValue::from_static("request-2"));
        assert!(optional_idempotency_key(&headers).is_err());
    }

    #[test]
    fn replay_response_marks_replayed_requests() {
        let response = idempotency_http_response(
            IdempotencyResponse {
                status_code: 200,
                headers: BTreeMap::from([(
                    "content-type".to_string(),
                    vec!["application/json".to_string()],
                )]),
                body: br#"{"ok":true}"#.to_vec(),
            },
            Some("request-1"),
            true,
        )
        .expect("replay response");
        assert_eq!(response.headers()["idempotency-key"], "request-1");
    }
}

pub async fn well_known_jwks(State(state): State<ApiState>) -> Json<Value> {
    Json(state.jwt.jwks())
}

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
    axum::extract::Path(plugin_id): axum::extract::Path<String>,
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
    axum::extract::Path(plugin_id): axum::extract::Path<String>,
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
    axum::extract::Path(plugin_id): axum::extract::Path<String>,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    state.plugins.uninstall(&plugin_id).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

pub async fn plugins_mcp_test(
    State(state): State<ApiState>,
    auth: AuthContext,
    axum::extract::Path((plugin_id, server_name)): axum::extract::Path<(String, String)>,
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
    axum::extract::Path((plugin_id, server_name)): axum::extract::Path<(String, String)>,
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
    axum::extract::Path((plugin_id, skill_id)): axum::extract::Path<(String, String)>,
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
