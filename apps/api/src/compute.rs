//! OpenAI-compatible text inference gateway. Only this path can debit managed inference.
//! Local models and BYOK do not call the ARO credit ledger.
use crate::{
    auth::{ApiError, AuthContext},
    ApiState,
};
use aro_core::inference_charge;
use aro_store::{hash_secret, BillingReservation, TenantContext};
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::{
    env,
    sync::{Arc, OnceLock},
    time::Duration,
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedModel {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub upstream_model: String,
    pub input_micros_per_million: u64,
    pub output_micros_per_million: u64,
    pub max_output_tokens: u32,
}
fn model_catalog() -> Result<Vec<ManagedModel>, ApiError> {
    if env::var("ARO_COMPUTE_ENABLED").as_deref() != Ok("true") {
        return Err(ApiError::service_unavailable("ARO Compute is not enabled"));
    }
    let models: Vec<ManagedModel> = serde_json::from_str(
        &env::var("ARO_COMPUTE_MODELS_JSON").unwrap_or_default(),
    )
    .map_err(|_| ApiError::service_unavailable("managed model catalog is not configured"))?;
    if models.is_empty()
        || models.len() > 20
        || models.iter().any(|m| {
            m.id.is_empty()
                || m.id.len() > 100
                || !m
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                || m.upstream_model.is_empty()
                || !(1..=16384).contains(&m.max_output_tokens)
                || m.input_micros_per_million == 0
                || m.output_micros_per_million == 0
                || m.input_micros_per_million > 1_000_000_000
                || m.output_micros_per_million > 1_000_000_000
        })
    {
        return Err(ApiError::service_unavailable(
            "invalid managed model catalog",
        ));
    }
    let unique: std::collections::HashSet<_> = models.iter().map(|m| &m.id).collect();
    if unique.len() != models.len() {
        return Err(ApiError::service_unavailable("duplicate managed model IDs"));
    }
    endpoint()?;
    Ok(models)
}
fn endpoint() -> Result<String, ApiError> {
    let value = env::var("ARO_COMPUTE_BASE_URL").unwrap_or_default();
    let url = url::Url::parse(&value)
        .map_err(|_| ApiError::service_unavailable("compute endpoint not configured"))?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(ApiError::service_unavailable("invalid compute endpoint"));
    }
    Ok(value.trim_end_matches('/').into())
}
pub fn configured() -> bool {
    model_catalog().is_ok()
}
pub async fn models() -> Result<Json<Value>, ApiError> {
    let models = model_catalog()?;
    Ok(Json(
        json!({"object":"list","data":models.into_iter().map(|m|json!({"id":m.id,"object":"model","owned_by":"aro","pricing":m})).collect::<Vec<_>>()}),
    ))
}
fn db(e: sqlx::Error) -> ApiError {
    tracing::error!(error=%e,"compute storage failure");
    ApiError::internal("compute storage unavailable")
}

async fn authenticate(state: &ApiState, headers: &HeaderMap) -> Result<TenantContext, ApiError> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::unauthorized("missing compute key"))?;
    if !token.starts_with("aro_compute_") || token.len() != 76 {
        return Err(ApiError::unauthorized("invalid compute key"));
    }
    let row=sqlx::query("SELECT actor_id,organization_id FROM billing_compute_keys WHERE key_hash=$1 AND revoked_at IS NULL").bind(hash_secret(token)).fetch_optional(state.store.pool()).await.map_err(db)?.ok_or_else(||ApiError::unauthorized("invalid compute key"))?;
    let actor: Uuid = row.try_get("actor_id").map_err(db)?;
    let org: Uuid = row.try_get("organization_id").map_err(db)?;
    state.store.ensure_org_access(actor, org).await?;
    Ok(TenantContext::new(actor, org)?)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewKey {
    name: String,
}
pub async fn create_key(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(req): Json<NewKey>,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    if !configured() {
        return Err(ApiError::service_unavailable(
            "ARO Compute is not configured",
        ));
    }
    if req.name.trim().is_empty() || req.name.len() > 80 {
        return Err(ApiError::bad_request("key name must be 1–80 characters"));
    }
    let mut tx = state.store.billing_lock(auth.organization_id).await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_compute_keys WHERE organization_id=$1 AND revoked_at IS NULL",
    )
    .bind(auth.organization_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(db)?;
    if count >= 20 {
        return Err(ApiError::bad_request(
            "revoke an existing key before creating another",
        ));
    }
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let secret = format!(
        "aro_compute_{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let id = Uuid::new_v4();
    let prefix = &secret[..20];
    sqlx::query("INSERT INTO billing_compute_keys (id,organization_id,actor_id,name,key_hash,prefix) VALUES ($1,$2,$3,$4,$5,$6)").bind(id).bind(auth.organization_id).bind(auth.user_id).bind(req.name.trim()).bind(hash_secret(&secret)).bind(prefix).execute(&mut *tx).await.map_err(db)?;
    tx.commit().await.map_err(db)?;
    Ok(Json(
        json!({"id":id,"name":req.name.trim(),"secret":secret,"prefix":prefix}),
    ))
}
pub async fn list_keys(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let rows=sqlx::query("SELECT id,name,prefix,created_at FROM billing_compute_keys WHERE organization_id=$1 AND revoked_at IS NULL ORDER BY created_at DESC").bind(auth.organization_id).fetch_all(state.store.pool()).await.map_err(db)?;
    let keys:Result<Vec<Value>,ApiError>=rows.iter().map(|r|Ok(json!({"id":r.try_get::<Uuid,_>("id").map_err(db)?,"name":r.try_get::<String,_>("name").map_err(db)?,"prefix":r.try_get::<String,_>("prefix").map_err(db)?,"createdAt":r.try_get::<chrono::DateTime<Utc>,_>("created_at").map_err(db)?}))).collect();
    Ok(Json(json!(keys?)))
}
pub async fn revoke_key(
    State(state): State<ApiState>,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    sqlx::query("UPDATE billing_compute_keys SET revoked_at=now() WHERE id=$1 AND organization_id=$2 AND revoked_at IS NULL").bind(id).bind(auth.organization_id).execute(state.store.pool()).await.map_err(db)?;
    Ok(Json(json!({"revoked":true})))
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeMessage {
    role: String,
    content: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionRequest {
    model: String,
    messages: Vec<ComputeMessage>,
    #[serde(default = "default_tokens")]
    max_tokens: u32,
    #[serde(default)]
    stream: bool,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    response_format: Option<ResponseFormat>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    kind: ResponseKind,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseKind {
    Text,
    JsonObject,
}
fn default_tokens() -> u32 {
    1024
}
fn reserve_price(req: &CompletionRequest, model: &ManagedModel) -> Result<i64, ApiError> {
    if req.messages.is_empty()
        || req.messages.len() > 100
        || req
            .messages
            .iter()
            .any(|m| !matches!(m.role.as_str(), "system" | "user" | "assistant"))
        || req
            .temperature
            .is_some_and(|t| !t.is_finite() || !(0.0..=2.0).contains(&t))
    {
        return Err(ApiError::bad_request("unsupported message or temperature"));
    }
    let input_bytes: usize = req.messages.iter().map(|m| m.content.len()).sum();
    if input_bytes > 131072 || req.max_tokens == 0 || req.max_tokens > model.max_output_tokens {
        return Err(ApiError::bad_request(
            "input or output exceeds model limits",
        ));
    }
    // Conservative reservation for byte-level tokenizers and chat-template overhead.
    let upper_input = (input_bytes + req.messages.len() * 128 + 2048) as u64;
    inference_charge(
        upper_input,
        u64::from(req.max_tokens),
        model.input_micros_per_million,
        model.output_micros_per_million,
    )
    .filter(|c| *c > 0)
    .ok_or_else(|| ApiError::bad_request("request cost exceeds supported limit"))
}
pub async fn completion(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(req): Json<CompletionRequest>,
) -> Result<Response, ApiError> {
    let tenant = authenticate(&state, &headers).await?;
    let model = model_catalog()?
        .into_iter()
        .find(|m| m.id == req.model)
        .ok_or_else(|| ApiError::bad_request("unknown managed model"))?;
    let key = match headers.get("idempotency-key") {
        Some(value) => Uuid::parse_str(
            value
                .to_str()
                .map_err(|_| ApiError::bad_request("invalid idempotency key"))?,
        )
        .map_err(|_| ApiError::bad_request("idempotency key must be a UUID"))?,
        None => Uuid::new_v4(),
    };
    let max_cost = reserve_price(&req, &model)?;
    static CAPACITY: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let permit = CAPACITY
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(8)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::too_many_requests("compute capacity busy; retry later"))?;
    let streamed = req.stream;
    // Provider work survives HTTP disconnects/timeouts and persists its settlement.
    let output = tokio::spawn(async move {
        let _permit = permit;
        execute(state, tenant, key, req, model, max_cost).await
    })
    .await
    .map_err(|_| {
        ApiError::internal("compute task interrupted; inspect the reservation before retrying")
    })??;
    let mut response = if streamed {
        let id = &output["id"];
        let model = &output["model"];
        let content = &output["choices"][0]["message"]["content"];
        let chunk = json!({"id":id,"object":"chat.completion.chunk","created":output["created"],"model":model,"choices":[{"index":0,"delta":{"role":"assistant","content":content},"finish_reason":null}]});
        let done = json!({"id":id,"object":"chat.completion.chunk","created":output["created"],"model":model,"choices":[{"index":0,"delta":{},"finish_reason":output["choices"][0]["finish_reason"]}],"usage":output["usage"]});
        (
            [
                (header::CONTENT_TYPE, "text/event-stream"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n"),
        )
            .into_response()
    } else {
        Json(output).into_response()
    };
    response.headers_mut().insert(
        "idempotency-key",
        key.to_string().parse().expect("UUID is a header value"),
    );
    Ok(response)
}
async fn execute(
    state: ApiState,
    tenant: TenantContext,
    key: Uuid,
    req: CompletionRequest,
    model: ManagedModel,
    max_cost: i64,
) -> Result<Value, ApiError> {
    let hash = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&req).map_err(|_| ApiError::bad_request("invalid request"))?
        )
    );
    let reservation = match state
        .store
        .billing_reserve(tenant, key, &hash, max_cost)
        .await?
    {
        BillingReservation::Replay(result) => {
            return if result.get("reconciliation").is_some() {
                Err(ApiError::conflict(
                    "request reconciled by operator; original response unavailable",
                ))
            } else {
                Ok(result)
            }
        }
        BillingReservation::Pending => return Err(ApiError::conflict(
            "request already running or awaiting reconciliation; do not resubmit with another key",
        )),
        BillingReservation::New(id) => id,
    };
    let outcome = generate(&req, &model).await;
    match outcome {
        Ok(mut result) => {
            let input = result["usage"]["prompt_tokens"].as_u64();
            let output = result["usage"]["completion_tokens"].as_u64();
            let charge = input.zip(output).and_then(|(i, o)| {
                inference_charge(
                    i,
                    o,
                    model.input_micros_per_million,
                    model.output_micros_per_million,
                )
            });
            let Some(charge) = charge.filter(|c| *c <= max_cost) else {
                state
                    .store
                    .billing_settle(tenant.organization_id(), reservation, None, None, true)
                    .await?;
                return Err(ApiError::service_unavailable(
                    "provider usage requires reconciliation; reserved funds have not been charged",
                ));
            };
            result["model"] = json!(model.id);
            result["aroBilling"] = json!({"chargedMicros":charge,"currency":"EUR","requestId":key});
            state
                .store
                .billing_settle(
                    tenant.organization_id(),
                    reservation,
                    Some(charge),
                    Some(&result),
                    false,
                )
                .await?;
            Ok(result)
        }
        Err((error, uncertain)) => {
            state
                .store
                .billing_settle(tenant.organization_id(), reservation, None, None, uncertain)
                .await?;
            Err(error)
        }
    }
}
async fn generate(
    req: &CompletionRequest,
    model: &ManagedModel,
) -> Result<Value, (ApiError, bool)> {
    let endpoint = endpoint().map_err(|e| (e, false))?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| (ApiError::internal("compute client unavailable"), false))?;
    let mut body = json!({"model":model.upstream_model,"messages":req.messages,"max_tokens":req.max_tokens,"temperature":req.temperature.unwrap_or(0.7),"stream":false});
    if let Some(format) = &req.response_format {
        body["response_format"] = json!(format);
    }
    let mut request = client
        .post(format!("{endpoint}/chat/completions"))
        .json(&body);
    if let Ok(secret) = env::var("ARO_COMPUTE_UPSTREAM_KEY") {
        if !secret.is_empty() {
            request = request.bearer_auth(secret);
        }
    }
    let mut response = request.send().await.map_err(|_| {
        (
            ApiError::service_unavailable(
                "compute request interrupted; reservation retained for reconciliation",
            ),
            true,
        )
    })?;
    if !response.status().is_success() {
        let uncertain = !response.status().is_client_error() || response.status().as_u16() == 408;
        return Err((
            ApiError::service_unavailable("model provider rejected the request"),
            uncertain,
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        (
            ApiError::service_unavailable("incomplete compute response"),
            true,
        )
    })? {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err((
                ApiError::service_unavailable("compute response too large"),
                true,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        (
            ApiError::service_unavailable("invalid compute response"),
            true,
        )
    })?;
    let text = value["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            (
                ApiError::service_unavailable("unsupported model response"),
                true,
            )
        })?;
    Ok(
        json!({"id":format!("chatcmpl-{}",Uuid::new_v4()),"object":"chat.completion","created":Utc::now().timestamp(),"model":model.id,"choices":[{"index":0,"message":{"role":"assistant","content":text},"finish_reason":value["choices"][0]["finish_reason"].as_str().unwrap_or("stop")}],"usage":value["usage"]}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reservation_bounds_utf8_and_output_before_calling_provider() {
        let model = ManagedModel {
            id: "m".into(),
            name: "M".into(),
            upstream_model: "m".into(),
            input_micros_per_million: 1_000_000,
            output_micros_per_million: 2_000_000,
            max_output_tokens: 2048,
        };
        let mut req = CompletionRequest {
            model: "m".into(),
            messages: vec![ComputeMessage {
                role: "user".into(),
                content: "é😊".into(),
            }],
            max_tokens: 100,
            stream: false,
            temperature: None,
            response_format: None,
        };
        assert_eq!(reserve_price(&req, &model).unwrap(), 6 + 128 + 2048 + 200);
        req.max_tokens = 2049;
        assert!(reserve_price(&req, &model).is_err());
        req.max_tokens = 1;
        req.messages[0].role = "tool".into();
        assert!(reserve_price(&req, &model).is_err());
    }
}
