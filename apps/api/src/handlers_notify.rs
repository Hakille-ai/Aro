//! Notification and email HTTP handlers.
//!
//! Extracted from `handlers.rs`: user notification CRUD plus the SMTP/Resend
//! email bridges. Self-contained request/response shapes, no cross-domain
//! state machines.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use aro_core::{
    NotificationFilter, NotificationItem, NotificationKind, NotificationPriority,
    NotificationSource,
};

use crate::{
    auth::{ApiError, AuthContext},
    ApiState,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNotificationPayload {
    pub id: Option<String>,
    pub title: String,
    pub body: String,
    pub kind: Option<NotificationKind>,
    pub priority: Option<NotificationPriority>,
    pub source: Option<NotificationSource>,
    pub action_url: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendEmailPayload {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub is_html: Option<bool>,
}

pub async fn notifications_list(
    State(state): State<ApiState>,
    auth: AuthContext,
    Query(filter): Query<NotificationFilter>,
) -> Result<Json<Vec<NotificationItem>>, ApiError> {
    let list = state.store.list_notifications(auth.tenant_context(), &filter).await?;
    Ok(Json(list))
}

pub async fn notification_create(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(payload): Json<CreateNotificationPayload>,
) -> Result<(StatusCode, Json<NotificationItem>), ApiError> {
    let trimmed_title = payload.title.trim().to_string();
    if trimmed_title.is_empty() {
        return Err(ApiError::bad_request("Notification title cannot be empty"));
    }
    let mut notif = NotificationItem::new(
        trimmed_title,
        payload.body,
        payload.kind.unwrap_or(NotificationKind::Info),
        payload.source.unwrap_or(NotificationSource::System),
    );
    if let Some(id) = payload.id {
        notif.id = id;
    }
    if let Some(priority) = payload.priority {
        notif.priority = priority;
    }
    notif.action_url = payload.action_url;
    notif.metadata = payload.metadata;
    notif.organization_id = Some(auth.tenant_context().organization_id());
    notif.user_id = Some(auth.tenant_context().actor_id());

    let saved = state.store.create_notification(auth.tenant_context(), &notif).await?;
    Ok((StatusCode::CREATED, Json(saved)))
}

pub async fn notifications_unread_count(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    let count = state.store.get_unread_notification_count(auth.tenant_context()).await?;
    Ok(Json(json!({ "unreadCount": count })))
}

pub async fn notification_mark_read(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let updated = state.store.mark_notification_as_read(auth.tenant_context(), id).await?;
    Ok(Json(json!({ "success": updated })))
}

pub async fn notifications_mark_all_read(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    let affected = state.store.mark_all_notifications_as_read(auth.tenant_context()).await?;
    Ok(Json(json!({ "markedCount": affected })))
}

pub async fn notification_delete(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let deleted = state.store.delete_notification(auth.tenant_context(), id).await?;
    Ok(Json(json!({ "success": deleted })))
}

pub async fn notifications_clear_all(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    let cleared = state.store.clear_all_notifications(auth.tenant_context()).await?;
    Ok(Json(json!({ "clearedCount": cleared })))
}

pub async fn email_send(
    State(_state): State<ApiState>,
    _auth: AuthContext,
    Json(payload): Json<SendEmailPayload>,
) -> Result<Json<Value>, ApiError> {
    let trimmed_to = payload.to.trim().to_string();
    let trimmed_subject = payload.subject.trim().to_string();
    let trimmed_body = payload.body.trim().to_string();

    if trimmed_to.is_empty() || trimmed_subject.is_empty() {
        return Err(ApiError::bad_request("Recipient 'to' and 'subject' are required"));
    }

    if !trimmed_to.contains('@') || !trimmed_to.contains('.') || trimmed_to.len() < 5 {
        return Err(ApiError::bad_request(format!(
            "Invalid recipient email address: '{trimmed_to}'"
        )));
    }

    if trimmed_body.is_empty() {
        return Err(ApiError::bad_request("Email body cannot be empty"));
    }

    let is_html = payload.is_html.unwrap_or(false);

    // 1. Check if SMTP relay is configured
    if let Ok(relay) = std::env::var("ARO_SMTP_RELAY") {
        let relay = relay.trim();
        if !relay.is_empty() {
            use lettre::{
                message::{header::ContentType, Mailbox},
                transport::smtp::authentication::Credentials,
                AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
            };
            use std::time::Duration;

            let port = std::env::var("ARO_SMTP_PORT")
                .ok()
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(587);

            let from_str = std::env::var("ARO_SMTP_FROM")
                .unwrap_or_else(|_| "noreply@aro-ai.com".to_string());

            let tls_mode = std::env::var("ARO_SMTP_TLS_MODE")
                .unwrap_or_else(|_| "starttls".to_string());

            let from: Mailbox = from_str
                .parse()
                .map_err(|e| ApiError::bad_request(format!("Invalid sender address ARO_SMTP_FROM: {e}")))?;

            let to_mb: Mailbox = trimmed_to
                .parse()
                .map_err(|e| ApiError::bad_request(format!("Invalid recipient address: {e}")))?;

            let builder = Message::builder()
                .from(from)
                .to(to_mb)
                .subject(&trimmed_subject);

            let message = if is_html {
                builder
                    .header(ContentType::TEXT_HTML)
                    .body(trimmed_body.clone())
            } else {
                builder
                    .header(ContentType::TEXT_PLAIN)
                    .body(trimmed_body.clone())
            }
            .map_err(|e| ApiError::internal(format!("Failed to construct email: {e}")))?;

            let transport_builder = if tls_mode.eq_ignore_ascii_case("tls") {
                AsyncSmtpTransport::<Tokio1Executor>::relay(relay)
                    .map_err(|e| ApiError::internal(format!("SMTP relay configuration error: {e}")))?
                    .port(port)
                    .timeout(Some(Duration::from_secs(20)))
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(relay)
                    .map_err(|e| ApiError::internal(format!("SMTP starttls configuration error: {e}")))?
                    .port(port)
                    .timeout(Some(Duration::from_secs(20)))
            };

            let mut transport_builder = transport_builder;
            let username = std::env::var("ARO_SMTP_USERNAME").ok().filter(|u| !u.trim().is_empty());
            let password = std::env::var("ARO_SMTP_PASSWORD").ok().filter(|p| !p.trim().is_empty());
            if let (Some(u), Some(p)) = (username, password) {
                transport_builder = transport_builder.credentials(Credentials::new(u, p));
            }

            let mailer = transport_builder.build();
            mailer
                .send(message)
                .await
                .map_err(|e| ApiError::internal(format!("SMTP delivery failed: {e}")))?;

            return Ok(Json(json!({
                "success": true,
                "status": "delivered_smtp",
                "recipient": trimmed_to,
                "subject": trimmed_subject,
                "message": "Email dispatched successfully via SMTP relay"
            })));
        }
    }

    // 2. Check if Resend API key is configured
    if let Ok(resend_key) = std::env::var("RESEND_API_KEY") {
        let resend_key = resend_key.trim();
        if !resend_key.is_empty() {
            let from = std::env::var("ARO_EMAIL_FROM").unwrap_or_else(|_| "onboarding@resend.dev".to_string());
            let client = reqwest::Client::new();
            let resend_payload = json!({
                "from": from,
                "to": [&trimmed_to],
                "subject": trimmed_subject,
                "text": trimmed_body,
                "html": if is_html { Some(trimmed_body.as_str()) } else { None }
            });

            let resp = client
                .post("https://api.resend.com/emails")
                .header("Authorization", format!("Bearer {resend_key}"))
                .json(&resend_payload)
                .send()
                .await
                .map_err(|e| ApiError::internal(format!("Resend API request failed: {e}")))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let err_text = resp.text().await.unwrap_or_default();
                return Err(ApiError::internal(format!("Resend API error ({status}): {err_text}")));
            }

            return Ok(Json(json!({
                "success": true,
                "status": "delivered_resend",
                "recipient": trimmed_to,
                "subject": trimmed_subject,
                "message": "Email dispatched successfully via Resend API"
            })));
        }
    }

    // 3. Fallback: Local dev mode simulation with structured acknowledgment
    tracing::info!(
        "Email delivery queued in local environment: to={}, subject={}",
        trimmed_to,
        trimmed_subject
    );

    Ok(Json(json!({
        "success": true,
        "status": "simulated_local",
        "recipient": trimmed_to,
        "subject": trimmed_subject,
        "bodyLength": trimmed_body.len(),
        "isHtml": is_html,
        "message": "Email accepted and queued in development outbox. Configure ARO_SMTP_RELAY or RESEND_API_KEY for live delivery."
    })))
}

pub async fn email_test(
    State(_state): State<ApiState>,
    _auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    let has_smtp = std::env::var("ARO_SMTP_RELAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let has_resend = std::env::var("RESEND_API_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);

    let provider = if has_smtp {
        "smtp"
    } else if has_resend {
        "resend"
    } else {
        "local_simulation"
    };

    Ok(Json(json!({
        "success": true,
        "configuredProvider": provider,
        "smtpConfigured": has_smtp,
        "resendConfigured": has_resend,
        "message": match provider {
            "smtp" => "SMTP relay connection configured and ready",
            "resend" => "Resend API connection configured and ready",
            _ => "Email service running in local development mode (no external credentials configured)",
        }
    })))
}
