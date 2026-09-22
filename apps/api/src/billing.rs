//! Billing boundaries: authenticated organization, provider-verified payments, integer money.
use crate::{
    auth::{ApiError, AuthContext},
    ApiState,
};
use aro_core::{COMMERCIAL_CATALOG, MICRO_EUR_PER_CENT};
use aro_store::BillingCheckoutIntent;
use axum::{body::Bytes, extract::State, http::HeaderMap, Json};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::Row;
use std::{env, time::Duration};
use uuid::Uuid;

fn db(error: sqlx::Error) -> ApiError {
    tracing::error!(%error,"billing database failure");
    ApiError::internal("billing storage unavailable")
}
pub fn enabled() -> bool {
    env::var("ARO_BILLING_ENABLED").as_deref() == Ok("true")
}
fn required(name: &str) -> Result<String, ApiError> {
    env::var(name)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ApiError::service_unavailable("billing is not configured"))
}
fn return_url() -> Result<String, ApiError> {
    let value = required("ARO_BILLING_RETURN_URL")?;
    let url = url::Url::parse(&value)
        .map_err(|_| ApiError::service_unavailable("invalid billing return URL"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(ApiError::service_unavailable(
            "billing return URL must use HTTPS",
        ));
    }
    Ok(value)
}
fn configured() -> bool {
    enabled()
        && env::var("ARO_COMMERCIAL_TERMS_APPROVED").as_deref() == Ok("true")
        && required("ARO_STRIPE_SECRET_KEY").is_ok()
        && required("ARO_STRIPE_WEBHOOK_SECRET").is_ok()
        && required("ARO_STRIPE_PRICE_CLOUD").is_ok()
        && required("ARO_STRIPE_PRICE_BUSINESS").is_ok()
        && return_url().is_ok()
}
fn require_configured() -> Result<(), ApiError> {
    if configured() {
        Ok(())
    } else {
        Err(ApiError::service_unavailable(
            "Les paiements ne sont pas encore disponibles sur ce serveur.",
        ))
    }
}

pub async fn stripe_get(path: &str) -> Result<Value, ApiError> {
    stripe_call(path, None, None).await
}
async fn stripe_call(
    path: &str,
    form: Option<&[(String, String)]>,
    key: Option<String>,
) -> Result<Value, ApiError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| ApiError::internal("payment client unavailable"))?;
    let url = format!("https://api.stripe.com/v1/{path}");
    let mut req = if let Some(form) = form {
        client.post(url).form(form)
    } else {
        client.get(url)
    };
    req = req
        .bearer_auth(required("ARO_STRIPE_SECRET_KEY")?)
        .header("Stripe-Version", "2025-02-24.acacia");
    if let Some(key) = key {
        req = req.header("Idempotency-Key", key);
    }
    let response = req.send().await.map_err(|_| {
        ApiError::service_unavailable("payment provider unavailable; retry with the same request")
    })?;
    if !response.status().is_success() {
        tracing::warn!(status=%response.status(),"payment provider rejected request");
        return Err(ApiError::service_unavailable(
            "payment provider rejected the request",
        ));
    }
    response
        .json()
        .await
        .map_err(|_| ApiError::service_unavailable("invalid payment provider response"))
}
fn provider_id(value: &Value, prefix: &str) -> Result<String, ApiError> {
    let id = value
        .as_str()
        .or_else(|| value.get("id").and_then(Value::as_str))
        .unwrap_or("");
    if !id.starts_with(prefix)
        || id.len() > 255
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(ApiError::bad_request("invalid payment reference"));
    }
    Ok(id.into())
}

pub async fn catalog() -> Json<Value> {
    let mut value: Value =
        serde_json::from_str(COMMERCIAL_CATALOG).expect("validated commercial catalog");
    value["checkoutAvailable"] = json!(configured());
    value["computeAvailable"] = json!(crate::compute::configured());
    value["salesEmail"] = json!(env::var("ARO_SALES_EMAIL").ok().filter(|s| s.contains('@')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))));
    Json(value)
}
pub async fn account(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    let account = state.store.billing_account(auth.tenant_context()).await?;
    let can_manage = state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await
        .is_ok();
    let ledger = if can_manage {
        state.store.billing_ledger(auth.tenant_context()).await?
    } else {
        vec![]
    };
    let pending = if can_manage {
        sqlx::query_scalar::<_,Uuid>("SELECT id FROM billing_checkout_intents WHERE organization_id=$1 AND kind <> 'credit' AND created_at > now()-interval '24 hours' ORDER BY created_at DESC LIMIT 1").bind(auth.organization_id).fetch_optional(state.store.pool()).await.map_err(db)?
    } else {
        None
    };
    let portal_available = can_manage && configured()
        && state.store.billing_customer(auth.tenant_context()).await?.is_some();
    Ok(Json(
        json!({"account":account,"entitlements":account.entitlements(Utc::now()),"canManage":can_manage,"ledger":ledger,"pendingCheckoutId":pending,"portalAvailable":portal_available,"checkoutAvailable":configured(),"computeAvailable":crate::compute::configured()}),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Limits {
    monthly_limit_micros: i64,
    per_request_limit_micros: i64,
}
pub async fn limits(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(req): Json<Limits>,
) -> Result<Json<Value>, ApiError> {
    state
        .store
        .billing_set_limits(
            auth.tenant_context(),
            req.monthly_limit_micros,
            req.per_request_limit_micros,
        )
        .await?;
    Ok(Json(json!({"saved":true})))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkout {
    request_id: Uuid,
    plan: String,
    #[serde(default = "one")]
    seats: i32,
    #[serde(default)]
    amount_cents: i64,
}
fn one() -> i32 {
    1
}

pub async fn checkout(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(req): Json<Checkout>,
) -> Result<Json<Value>, ApiError> {
    require_configured()?;
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    if req.request_id.is_nil() || !(1..=1000).contains(&req.seats) {
        return Err(ApiError::bad_request("invalid checkout request"));
    }
    let credit = req.plan == "credit";
    if credit
        && (!crate::compute::configured()
            || ![1000, 2500, 5000, 10000].contains(&req.amount_cents)
            || req.seats != 1)
    {
        return Err(ApiError::bad_request(
            "invalid credit purchase or compute unavailable",
        ));
    }
    if !credit
        && (req.amount_cents != 0
            || !matches!(req.plan.as_str(), "cloud" | "business")
            || (req.plan == "cloud" && req.seats != 1))
    {
        return Err(ApiError::bad_request("invalid subscription"));
    }
    if !credit {
        let members:i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memberships WHERE organization_id=$1 AND status='active' AND deleted_at IS NULL").bind(auth.organization_id).fetch_one(state.store.pool()).await.map_err(db)?;
        if members > i64::from(req.seats) {
            return Err(ApiError::bad_request(
                "Souscrivez au moins autant de sièges que de membres actifs. Cloud est personnel.",
            ));
        }
    }
    let intent = state
        .store
        .billing_create_intent(
            auth.tenant_context(),
            req.request_id,
            &req.plan,
            req.seats,
            req.amount_cents,
        )
        .await?;
    if let Some(session) = &intent.session_id {
        let value = stripe_get(&format!("checkout/sessions/{session}")).await?;
        if value["status"] != "open" {
            return Err(ApiError::conflict("checkout_closed_refresh_account"));
        }
        return Ok(Json(json!({"url": value["url"]})));
    }
    let ret = return_url()?;
    let mut form = vec![
        (
            "mode".into(),
            if credit { "payment" } else { "subscription" }.into(),
        ),
        ("success_url".into(), ret.clone()),
        ("cancel_url".into(), ret),
        ("client_reference_id".into(), intent.id.to_string()),
        ("metadata[aro_intent]".into(), intent.id.to_string()),
        ("automatic_tax[enabled]".into(), "true".into()),
        ("billing_address_collection".into(), "required".into()),
        (
            "consent_collection[terms_of_service]".into(),
            "required".into(),
        ),
        (
            "metadata[aro_catalog_version]".into(),
            serde_json::from_str::<Value>(COMMERCIAL_CATALOG).expect("catalog")["version"]
                .as_str()
                .expect("catalog version")
                .into(),
        ),
    ];
    let customer = ensure_customer(&state, auth.organization_id).await?;
    form.push(("customer".into(), customer));
    form.push(("customer_update[address]".into(), "auto".into()));
    if credit {
        form.extend([
            ("line_items[0][price_data][currency]".into(), "eur".into()),
            (
                "line_items[0][price_data][unit_amount]".into(),
                req.amount_cents.to_string(),
            ),
            (
                "line_items[0][price_data][tax_behavior]".into(),
                "exclusive".into(),
            ),
            (
                "line_items[0][price_data][product_data][name]".into(),
                "ARO — crédit de calcul IA".into(),
            ),
            ("line_items[0][quantity]".into(), "1".into()),
            (
                "payment_intent_data[metadata][aro_intent]".into(),
                intent.id.to_string(),
            ),
        ]);
    } else {
        let price_id = required(if req.plan == "cloud" {
            "ARO_STRIPE_PRICE_CLOUD"
        } else {
            "ARO_STRIPE_PRICE_BUSINESS"
        })?;
        let price_id = provider_id(&json!(price_id), "price_")?;
        let price = stripe_get(&format!("prices/{price_id}")).await?;
        if price["active"] != true {
            return Err(ApiError::service_unavailable("Stripe price is archived"));
        }
        validate_price(&req.plan, &price)?;
        form.extend([
            ("line_items[0][price]".into(), price_id),
            ("line_items[0][quantity]".into(), req.seats.to_string()),
            (
                "subscription_data[metadata][aro_intent]".into(),
                intent.id.to_string(),
            ),
        ]);
    }
    let response = stripe_call(
        "checkout/sessions",
        Some(&form),
        Some(format!("aro-checkout-{}", intent.id)),
    )
    .await?;
    let session = provider_id(&response["id"], "cs_")?;
    state
        .store
        .billing_attach_session(&intent, &session)
        .await?;
    Ok(Json(json!({"url":response["url"]})))
}
async fn ensure_customer(state: &ApiState, org: Uuid) -> Result<String, ApiError> {
    let mut tx = state.store.billing_lock(org).await?;
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT stripe_customer_id FROM billing_accounts WHERE organization_id=$1",
    )
    .bind(org)
    .fetch_one(&mut *tx)
    .await
    .map_err(db)?;
    if let Some(customer) = existing {
        return Ok(customer);
    }
    let customer = stripe_call(
        "customers",
        Some(&[("metadata[aro_organization]".into(), org.to_string())]),
        Some(format!("aro-customer-{org}")),
    )
    .await?;
    let id = provider_id(&customer["id"], "cus_")?;
    sqlx::query("UPDATE billing_accounts SET stripe_customer_id=$2 WHERE organization_id=$1")
        .bind(org)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    tx.commit().await.map_err(db)?;
    Ok(id)
}
fn validate_price(plan: &str, price: &Value) -> Result<(), ApiError> {
    let (amount, tax) = match plan {
        "cloud" => (1200, "inclusive"),
        "business" => (2900, "exclusive"),
        _ => return Err(ApiError::bad_request("unknown plan")),
    };
    if price["currency"] != "eur"
        || price["unit_amount"] != amount
        || price["tax_behavior"] != tax
        || price["recurring"]["interval"] != "month"
        || price["recurring"]["interval_count"] != 1
    {
        return Err(ApiError::service_unavailable(
            "Stripe price does not match the published ARO catalog",
        ));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResumeCheckout {
    request_id: Uuid,
}
pub async fn resume_checkout(
    State(state): State<ApiState>,
    auth: AuthContext,
    Json(req): Json<ResumeCheckout>,
) -> Result<Json<Value>, ApiError> {
    require_configured()?;
    state
        .store
        .ensure_org_admin(auth.user_id, auth.organization_id)
        .await?;
    let intent = state
        .store
        .billing_intent(req.request_id)
        .await?
        .filter(|i| i.organization_id == auth.organization_id && i.actor_id == auth.user_id)
        .ok_or_else(|| ApiError::bad_request("checkout not found"))?;
    checkout(
        State(state),
        auth,
        Json(Checkout {
            request_id: intent.id,
            plan: intent.kind,
            seats: intent.quantity,
            amount_cents: intent.amount_cents,
        }),
    )
    .await
}
pub async fn portal(
    State(state): State<ApiState>,
    auth: AuthContext,
) -> Result<Json<Value>, ApiError> {
    require_configured()?;
    let customer = state
        .store
        .billing_customer(auth.tenant_context())
        .await?
        .ok_or_else(|| ApiError::bad_request("no subscription to manage"))?;
    let response = stripe_call(
        "billing_portal/sessions",
        Some(&[
            ("customer".into(), customer),
            ("return_url".into(), return_url()?),
        ]),
        None,
    )
    .await?;
    Ok(Json(json!({"url":response["url"]})))
}

pub fn verify_signature(secret: &str, header: &str, body: &[u8], now: i64) -> bool {
    let parts: Vec<_> = header
        .split(',')
        .filter_map(|p| p.trim().split_once('='))
        .collect();
    let timestamps: Vec<_> = parts.iter().filter(|(k, _)| *k == "t").collect();
    if timestamps.len() != 1 {
        return false;
    }
    let Ok(timestamp) = timestamps[0].1.parse::<i64>() else {
        return false;
    };
    if now.abs_diff(timestamp) > 300 {
        return false;
    }
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts key lengths");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    parts.iter().filter(|(k, _)| *k == "v1").any(|(_, sig)| {
        if sig.len() != 64 || !sig.is_ascii() {
            return false;
        }
        let decoded: Result<Vec<u8>, _> = (0..64)
            .step_by(2)
            .map(|i| u8::from_str_radix(&sig[i..i + 2], 16))
            .collect();
        decoded.is_ok_and(|bytes| mac.clone().verify_slice(&bytes).is_ok())
    })
}

pub async fn webhook(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    // Keep receiving refunds/cancellations when new sales are disabled.
    let secret = required("ARO_STRIPE_WEBHOOK_SECRET")?;
    if !verify_signature(
        &secret,
        headers
            .get("stripe-signature")
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
        &body,
        Utc::now().timestamp(),
    ) {
        return Err(ApiError::bad_request("invalid webhook signature"));
    }
    let event: Value =
        serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("invalid webhook"))?;
    let kind = event["type"].as_str().unwrap_or("");
    let obj = &event["data"]["object"];
    let subscription = kind.starts_with("customer.subscription.");
    let mut intent_value = obj["metadata"]["aro_intent"].as_str().map(str::to_owned);
    if matches!(
        kind,
        "charge.refunded" | "charge.dispute.created" | "charge.dispute.closed"
    ) {
        let payment = provider_id(&obj["payment_intent"], "pi_")?;
        let pi = stripe_get(&format!("payment_intents/{payment}")).await?;
        intent_value = pi["metadata"]["aro_intent"].as_str().map(str::to_owned);
    } else if !subscription
        && !matches!(
            kind,
            "checkout.session.completed" | "checkout.session.async_payment_succeeded"
        )
    {
        return Ok(Json(json!({"received":true})));
    }
    let Some(id) = intent_value.and_then(|s| Uuid::parse_str(&s).ok()) else {
        return Ok(Json(json!({"received":true})));
    };
    let Some(intent) = state.store.billing_intent(id).await? else {
        return Err(ApiError::bad_request("unknown billing intent"));
    };
    // Lock BEFORE retrieving current provider state: delayed/concurrent events cannot restore
    // an old subscription or credit twice. The transaction serializes all ARO replicas.
    let mut tx = state.store.billing_lock(intent.organization_id).await?;
    if subscription {
        if intent.kind == "credit" {
            return Err(ApiError::bad_request("invalid subscription intent"));
        }
        let manual_contract:bool=sqlx::query_scalar("SELECT plan='enterprise' AND stripe_subscription_id IS NULL FROM billing_accounts WHERE organization_id=$1").bind(intent.organization_id).fetch_one(&mut *tx).await.map_err(db)?;
        if manual_contract {
            return Ok(Json(json!({"received":true})));
        }
        let sub_id = provider_id(&obj["id"], "sub_")?;
        let current = stripe_get(&format!("subscriptions/{sub_id}")).await?;
        if current["metadata"]["aro_intent"] != id.to_string() {
            return Err(ApiError::bad_request("subscription metadata mismatch"));
        }
        let customer = provider_id(&current["customer"], "cus_")?;
        let rows = current["items"]["data"]
            .as_array()
            .ok_or_else(|| ApiError::bad_request("invalid subscription items"))?;
        if rows.len() != 1 {
            return Err(ApiError::bad_request("unexpected subscription items"));
        }
        let item = &rows[0];
        let price = &item["price"];
        let cloud = env::var("ARO_STRIPE_PRICE_CLOUD").unwrap_or_default();
        let business = env::var("ARO_STRIPE_PRICE_BUSINESS").unwrap_or_default();
        let plan = if price["id"] == cloud && !cloud.is_empty() {
            "cloud"
        } else if price["id"] == business && !business.is_empty() {
            "business"
        } else {
            return Err(ApiError::bad_request("unrecognized subscription price"));
        };
        validate_price(plan, price)?;
        let quantity = item["quantity"]
            .as_i64()
            .filter(|n| (1..=100000).contains(n))
            .ok_or_else(|| ApiError::bad_request("invalid seat count"))?
            as i32;
        if plan == "cloud" && quantity != 1 {
            return Err(ApiError::bad_request("invalid personal subscription"));
        }
        let status = current["status"]
            .as_str()
            .ok_or_else(|| ApiError::bad_request("invalid subscription status"))?;
        let valid = current["current_period_end"]
            .as_i64()
            .or_else(|| item["current_period_end"].as_i64())
            .and_then(|n| DateTime::from_timestamp(n, 0));
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT stripe_subscription_id FROM billing_accounts WHERE organization_id=$1",
        )
        .bind(intent.organization_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        if existing.as_ref().is_some_and(|s| s != &sub_id) {
            let old_status: String =
                sqlx::query_scalar("SELECT status FROM billing_accounts WHERE organization_id=$1")
                    .bind(intent.organization_id)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(db)?;
            if !matches!(
                old_status.as_str(),
                "inactive" | "canceled" | "incomplete_expired"
            ) || status == "canceled"
            {
                return Ok(Json(json!({"received":true})));
            }
        }
        sqlx::query("UPDATE billing_accounts SET plan=$2,status=$3,seats=$4,valid_until=$5,cancel_at_period_end=$6,stripe_customer_id=$7,stripe_subscription_id=$8,updated_at=now() WHERE organization_id=$1")
            .bind(intent.organization_id).bind(plan).bind(status).bind(quantity).bind(valid).bind(current["cancel_at_period_end"].as_bool().unwrap_or(false)).bind(customer).bind(sub_id).execute(&mut *tx).await.map_err(db)?;
    } else if intent.kind == "credit" {
        reconcile_credit(&intent, &mut tx, kind == "charge.dispute.created").await?;
    }
    tx.commit().await.map_err(db)?;
    Ok(Json(json!({"received":true})))
}

async fn reconcile_credit(
    intent: &BillingCheckoutIntent,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    dispute: bool,
) -> Result<(), ApiError> {
    let session = intent
        .session_id
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("checkout not yet linked; retry webhook"))?;
    let current = stripe_get(&format!(
        "checkout/sessions/{session}?expand[]=payment_intent.latest_charge"
    ))
    .await?;
    if current["metadata"]["aro_intent"] != intent.id.to_string()
        || current["currency"] != "eur"
        || current["amount_subtotal"] != intent.amount_cents
    {
        return Err(ApiError::bad_request("credit payment mismatch"));
    }
    if current["payment_status"] != "paid" {
        return Ok(());
    }
    let pi = &current["payment_intent"];
    let payment = provider_id(&pi["id"], "pi_")?;
    let charge = &pi["latest_charge"];
    let amount = charge["amount"]
        .as_i64()
        .filter(|n| *n > 0)
        .ok_or_else(|| ApiError::bad_request("invalid payment amount"))?;
    let refunded = charge["amount_refunded"]
        .as_i64()
        .unwrap_or(0)
        .clamp(0, amount);
    let credit = intent.amount_cents * MICRO_EUR_PER_CENT;
    let reversed = if dispute || charge["disputed"] == true {
        credit
    } else {
        ((i128::from(credit) * i128::from(refunded) + i128::from(amount) - 1) / i128::from(amount))
            as i64
    };
    let old=sqlx::query("SELECT credited_micros,reversed_micros FROM billing_checkout_intents WHERE id=$1 FOR UPDATE").bind(intent.id).fetch_one(&mut **tx).await.map_err(db)?;
    let old_credit: i64 = old.try_get("credited_micros").map_err(db)?;
    let old_reversed: i64 = old.try_get("reversed_micros").map_err(db)?;
    let reversed = reversed.max(old_reversed); // A disputed credit needs an operator decision to restore.
    let delta = credit - reversed - (old_credit - old_reversed);
    if delta != 0 {
        sqlx::query("UPDATE billing_accounts SET balance_micros=balance_micros+$2,updated_at=now() WHERE organization_id=$1").bind(intent.organization_id).bind(delta).execute(&mut **tx).await.map_err(db)?;
        sqlx::query("INSERT INTO billing_ledger (id,organization_id,reference,kind,amount_micros) VALUES ($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(intent.organization_id).bind(format!("stripe:{}:{credit}:{reversed}",intent.id)).bind(if delta>0 {"credit"} else {"refund"}).bind(delta).execute(&mut **tx).await.map_err(db)?;
    }
    sqlx::query("UPDATE billing_checkout_intents SET credited_micros=$2,reversed_micros=$3,stripe_payment_intent_id=$4 WHERE id=$1").bind(intent.id).bind(credit).bind(reversed).bind(payment).execute(&mut **tx).await.map_err(db)?;
    Ok(())
}

/// A managed deployment cannot grant team administration solely from a client plan label.
pub async fn require_team_plan(state: &ApiState, auth: &AuthContext) -> Result<(), ApiError> {
    if env::var("ARO_COMMERCIAL_ENFORCEMENT").as_deref() != Ok("true") {
        return Ok(());
    }
    let account = state.store.billing_account(auth.tenant_context()).await?;
    if !account.entitlements(Utc::now()).team_administration {
        return Err(ApiError::forbidden(
            "Une licence Business ou Enterprise active est requise.",
        ));
    }
    Ok(())
}

pub fn requires_managed_sync(method: &axum::http::Method, path: &str) -> bool {
    if !matches!(
        *method,
        axum::http::Method::POST | axum::http::Method::PUT | axum::http::Method::PATCH
    ) {
        return false;
    }
    let path = path.strip_prefix("/v1").unwrap_or(path);
    [
        "/projects",
        "/folders",
        "/conversations",
        "/messages",
        "/files/uploads",
        "/collections",
        "/memories",
        "/memory-index",
        "/skills",
        "/plugin-connections",
        "/mcp",
        "/hooks",
        "/scheduled-tasks",
        "/teams",
        "/usage",
        "/client-state",
        "/plugins",
        "/tools",
    ]
    .iter()
    .any(|prefix| {
        path == *prefix
            || path
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('/'))
    }) || matches!(
        path,
        "/assistant/local-result" | "/assistant/stream" | "/agent/runs"
    )
}

pub async fn require_managed_sync(
    state: &ApiState,
    tenant: aro_store::TenantContext,
) -> Result<(), ApiError> {
    let account = state.store.billing_account(tenant).await?;
    if !account.entitlements(Utc::now()).managed_sync {
        return Err(ApiError::forbidden("Un abonnement actif est requis pour enregistrer des données dans le cloud. Lecture, export et suppression restent accessibles."));
    }
    let members:i64=sqlx::query_scalar("SELECT count(*) FROM memberships WHERE organization_id=$1 AND status='active' AND deleted_at IS NULL").bind(tenant.organization_id()).fetch_one(state.store.pool()).await.map_err(db)?;
    if members > i64::from(account.seats) {
        return Err(ApiError::forbidden(
            "Le nombre de membres dépasse les sièges souscrits. Ajustez l'équipe ou l'abonnement.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiration_blocks_cloud_writes_but_preserves_read_export_delete_and_billing() {
        use axum::http::Method;
        assert!(requires_managed_sync(
            &Method::POST,
            "/v1/assistant/local-result"
        ));
        assert!(requires_managed_sync(&Method::PATCH, "/projects/example"));
        for (method, path) in [
            (Method::GET, "/v1/conversations/123/export"),
            (Method::DELETE, "/v1/files/123"),
            (Method::POST, "/v1/billing/checkout"),
            (Method::POST, "/v1/chat/completions"),
            (Method::PUT, "/v1/billing/limits"),
            (Method::POST, "/v1/auth/refresh"),
        ] {
            assert!(!requires_managed_sync(&method, path));
        }
    }
    #[test]
    fn signature_is_raw_timestamped_and_constant_time_verified() {
        let body = br#"{"id":"evt_1"}"#;
        let now = 1_800_000_000;
        let mut h = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
        h.update(format!("{now}.").as_bytes());
        h.update(body);
        let sig = h
            .finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let header = format!("t={now},v1={sig}");
        assert!(verify_signature("secret", &header, body, now));
        assert!(!verify_signature("secret", &header, b"{}", now));
        assert!(!verify_signature("secret", &header, body, now + 301));
        assert!(!verify_signature(
            "secret",
            &format!("t={now},t={now},v1={sig}"),
            body,
            now
        ));
    }
    #[test]
    fn prices_must_match_catalog_and_tax_basis() {
        let mut p = json!({"currency":"eur","unit_amount":2900,"tax_behavior":"exclusive","active":true,"recurring":{"interval":"month","interval_count":1}});
        assert!(validate_price("business", &p).is_ok());
        p["unit_amount"] = json!(29);
        assert!(validate_price("business", &p).is_err());
    }
    #[test]
    fn provider_ids_cannot_inject_paths() {
        assert!(provider_id(&json!("sub_abc"), "sub_").is_ok());
        assert!(provider_id(&json!("sub_a/../../customers"), "sub_").is_err());
    }
}
