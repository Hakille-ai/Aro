use aro_store::TenantContext;
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use uuid::Uuid;

use crate::ApiState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Claims {
    pub sub: Uuid,
    pub organization_id: Uuid,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    audience: String,
    key_id: Option<String>,
}

impl JwtService {
    pub fn new(secret: String) -> Self {
        Self::with_config(
            secret,
            "aro-api".to_string(),
            "aro-desktop".to_string(),
            None,
        )
    }

    pub fn with_config(
        secret: String,
        issuer: String,
        audience: String,
        key_id: Option<String>,
    ) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
            audience,
            key_id,
        }
    }

    pub fn jwks(&self) -> serde_json::Value {
        let kid = self
            .key_id
            .clone()
            .unwrap_or_else(|| "aro-key-1".to_string());
        serde_json::json!({
            "keys": [
                {
                    "kty": "oct",
                    "use": "sig",
                    "alg": "HS256",
                    "kid": kid
                }
            ]
        })
    }

    pub fn issue(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        ttl_minutes: i64,
    ) -> Result<(String, chrono::DateTime<Utc>), jsonwebtoken::errors::Error> {
        let now = Utc::now();
        let expires_at = now + Duration::minutes(ttl_minutes);
        let claims = Claims {
            sub: user_id,
            organization_id,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            iat: now.timestamp() as usize,
            exp: expires_at.timestamp() as usize,
        };
        let header = Header {
            kid: self.key_id.clone(),
            ..Default::default()
        };
        encode(&header, &claims, &self.encoding_key).map(|token| (token, expires_at))
    }

    pub fn verify(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let mut validation = Validation::default();
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_audience(&[self.audience.as_str()]);
        decode::<Claims>(token, &self.decoding_key, &validation).map(|data| data.claims)
    }
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub organization_id: Uuid,
    tenant: TenantContext,
}

impl AuthContext {
    pub const fn tenant_context(&self) -> TenantContext {
        self.tenant
    }
}

impl FromRequestParts<ApiState> for AuthContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ApiState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::unauthorized("invalid authorization scheme"))?;
        let claims = state
            .jwt
            .verify(token)
            .map_err(|_| ApiError::unauthorized("invalid or expired token"))?;
        // Tenant scope is cryptographically bound to the access token. Changing organizations
        // requires the refresh-token rotation endpoint; a caller-controlled header must never
        // widen the blast radius of a stolen tenant-scoped access token.
        let organization_id = claims.organization_id;
        let tenant = TenantContext::new(claims.sub, organization_id)
            .map_err(|_| ApiError::unauthorized("invalid tenant identity"))?;

        state
            .store
            .ensure_org_access(claims.sub, organization_id)
            .await
            .map_err(ApiError::from)?;

        Ok(Self {
            user_id: claims.sub,
            organization_id,
            tenant,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// Compatibility field retained for existing desktop clients.
    pub error: String,
    #[serde(rename = "type")]
    pub type_url: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub code: String,
}

#[derive(Debug, Clone)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    code: &'static str,
}

impl ApiError {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
            code: "authentication_failed",
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
            code: "permission_denied",
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            code: "invalid_request",
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
            code: "resource_not_found",
        }
    }

    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            message: message.into(),
            code: "payload_too_large",
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
            code: "conflict",
        }
    }

    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
            code: "rate_limited",
        }
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
            code: "service_unavailable",
        }
    }

    pub fn gateway_timeout(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::GATEWAY_TIMEOUT,
            message: message.into(),
            code: "request_timeout",
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            code: "internal_error",
        }
    }
}

impl From<aro_core::AroError> for ApiError {
    fn from(value: aro_core::AroError) -> Self {
        match value {
            aro_core::AroError::Security(message) => Self::forbidden(message),
            aro_core::AroError::Configuration(message) => Self::bad_request(message),
            aro_core::AroError::Memory(message) if message.contains("not found") => {
                Self::not_found(message)
            }
            aro_core::AroError::RetryableTransaction(message) => {
                tracing::warn!(error = %message, "database transaction retry budget exhausted");
                Self::service_unavailable("temporary database contention; retry the request")
            }
            other => {
                // Provider, database, and storage errors may contain implementation details.
                // Keep the diagnostics in server logs and return a stable public response.
                tracing::error!(error = %other, "unhandled API domain error");
                Self::internal("internal server error")
            }
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::bad_request(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let title = self
            .status
            .canonical_reason()
            .unwrap_or("API error")
            .to_string();
        let mut response = (
            self.status,
            Json(ErrorBody {
                error: self.message.clone(),
                type_url: format!("https://errors.aro.dev/{}", self.code),
                title,
                status: self.status.as_u16(),
                detail: self.message,
                code: self.code.to_string(),
            }),
        )
            .into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

pub fn generate_refresh_token() -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let mut bytes = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

const TOTP_PERIOD_SECONDS: i64 = 30;
const TOTP_WINDOW_STEPS: i64 = 1;

fn decode_base32_secret(secret: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bits: u32 = 0;
    let mut bit_count = 0u8;
    let mut out = Vec::new();
    for byte in secret.bytes() {
        if byte == b'=' {
            break;
        }
        let value = ALPHABET.iter().position(|candidate| candidate == &byte)? as u32;
        bits = (bits << 5) | value;
        bit_count += 5;
        if bit_count >= 8 {
            bit_count -= 8;
            out.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

fn hotp_value(key: &[u8], counter: u64) -> Option<String> {
    let mut mac = Hmac::<Sha1>::new_from_slice(key).ok()?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[digest.len() - 1] & 0x0f) as usize;
    let code = ((digest[offset] & 0x7f) as u32) << 24
        | (digest[offset + 1] as u32) << 16
        | (digest[offset + 2] as u32) << 8
        | (digest[offset + 3] as u32);
    Some(format!("{:06}", code % 1_000_000))
}

/// Vérifie un code TOTP RFC 6238 (SHA-1, 30 s, ±1 pas d'horloge).
/// Comparaison à temps constant ; secret insensible à la casse/espaces.
pub fn verify_totp_code(secret: &str, code: &str, now_unix_seconds: i64) -> bool {
    let normalized_code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    if normalized_code.len() != 6 || !normalized_code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let normalized_secret: String = secret
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let key = match decode_base32_secret(&normalized_secret) {
        Some(key) => key,
        None => return false,
    };
    let step = now_unix_seconds.div_euclid(TOTP_PERIOD_SECONDS);
    (-TOTP_WINDOW_STEPS..=TOTP_WINDOW_STEPS).any(|drift| {
        hotp_value(&key, step.saturating_add(drift) as u64).is_some_and(|expected| {
            use subtle_compare::ConstantTimeEq;
            expected.as_bytes().ct_eq(normalized_code.as_bytes())
        })
    })
}

mod subtle_compare {
    pub trait ConstantTimeEq {
        fn ct_eq(&self, other: &[u8]) -> bool;
    }

    impl ConstantTimeEq for [u8] {
        fn ct_eq(&self, other: &[u8]) -> bool {
            if self.len() != other.len() {
                return false;
            }
            self.iter()
                .zip(other.iter())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                == 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwt_round_trips_user_and_organization() {
        let jwt = JwtService::new("test-secret-that-is-long-enough".to_string());
        let user_id = Uuid::new_v4();
        let organization_id = Uuid::new_v4();

        let (token, expires_at) = jwt.issue(user_id, organization_id, 15).expect("issue");
        let claims = jwt.verify(&token).expect("verify");

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.organization_id, organization_id);
        assert_eq!(claims.iss, "aro-api");
        assert_eq!(claims.aud, "aro-desktop");
        assert!(expires_at > Utc::now());
    }

    // Vecteurs officiels RFC 6238, appendice B (SHA-1, secret ASCII
    // "12345678901234567890" encodé ci-dessous en base32).
    const RFC_SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn totp_matches_rfc6238_sha1_vectors() {
        assert!(verify_totp_code(RFC_SECRET, "287082", 59));
        assert!(verify_totp_code(RFC_SECRET, "081804", 1_111_111_109));
        assert!(verify_totp_code(RFC_SECRET, "050471", 1_111_111_111));
        assert!(verify_totp_code(RFC_SECRET, "005924", 1_234_567_890));
        assert!(verify_totp_code(RFC_SECRET, "279037", 2_000_000_000));
    }

    #[test]
    fn totp_rejects_wrong_and_malformed_codes() {
        assert!(!verify_totp_code(RFC_SECRET, "000000", 59));
        assert!(!verify_totp_code(RFC_SECRET, "28708", 59));
        assert!(!verify_totp_code(RFC_SECRET, "2870822", 59));
        assert!(!verify_totp_code(RFC_SECRET, "abcdef", 59));
        assert!(!verify_totp_code(RFC_SECRET, "", 59));
        assert!(!verify_totp_code("!!!not-base32!!!", "287082", 59));
        // Fenêtre ±1 pas : le code du pas suivant passe à la frontière…
        assert!(verify_totp_code(RFC_SECRET, "081804", 1_111_111_109 + 29));
        // …mais pas un code d'une autre époque.
        assert!(!verify_totp_code(RFC_SECRET, "287082", 1_111_111_109));
    }

    #[test]
    fn refresh_tokens_are_url_safe_and_unique() {
        let first = generate_refresh_token();
        let second = generate_refresh_token();

        assert_ne!(first, second);
        assert!(first.len() > 40);
        assert!(first
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'));
    }

    #[test]
    fn not_found_store_errors_map_to_http_not_found() {
        let error = ApiError::from(aro_core::AroError::Memory(
            "memory item not found".to_string(),
        ));
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        assert_eq!(error.message, "memory item not found");
    }

    #[test]
    fn security_errors_are_forbidden() {
        let error = ApiError::from(aro_core::AroError::Security(
            "organization access denied".to_string(),
        ));
        assert_eq!(error.status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn internal_errors_do_not_expose_implementation_details() {
        let error = ApiError::from(aro_core::AroError::RuntimeUnavailable(
            "postgres://username:password@example.invalid/database".to_string(),
        ));
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.message, "internal server error");
    }
}
