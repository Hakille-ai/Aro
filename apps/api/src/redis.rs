use std::env;

use aro_core::{AroError, AroResult};
use aro_store::OutboxEvent;
use axum::http::Method;
use uuid::Uuid;

use crate::DeploymentConfig;

const RATE_LIMIT_WINDOW_SECONDS: u64 = 60;
const AUTH_SESSION_LIMIT_PER_MINUTE: u32 = 60;
const LOCK_RELEASE_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
  return redis.call("DEL", KEYS[1])
else
  return 0
end
"#;
const RATE_LIMIT_SCRIPT: &str = r#"
local current = redis.call("INCR", KEYS[1])
if current == 1 then
  redis.call("EXPIRE", KEYS[1], ARGV[1])
end
return current
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedisHealth {
    Disabled,
    Ok,
}

impl RedisHealth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Ok => "ok",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitClass {
    Auth,
    AuthSession,
    Assistant,
    Write,
}

impl RateLimitClass {
    fn slug(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::AuthSession => "auth-session",
            Self::Assistant => "assistant",
            Self::Write => "write",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub limit: u32,
    pub current: u32,
}

#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub key_prefix: String,
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub auth_per_minute: u32,
    pub write_per_minute: u32,
    pub assistant_per_minute: u32,
}

#[derive(Debug, Clone)]
pub struct OutboxStreamConfig {
    pub enabled: bool,
    pub stream_key: String,
    pub max_len: usize,
    pub batch_size: i64,
}

#[derive(Clone)]
pub struct RedisServices {
    client: Option<::redis::aio::ConnectionManager>,
    config: RedisConfig,
    rate_limits: RateLimitConfig,
    outbox: OutboxStreamConfig,
}

pub struct RedisLock {
    client: Option<::redis::aio::ConnectionManager>,
    key: String,
    token: String,
}

impl RedisServices {
    pub async fn from_env(deployment: &DeploymentConfig) -> AroResult<Self> {
        let key_prefix = env::var("ARO_REDIS_KEY_PREFIX")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("aro:{}", deployment.environment));
        let configured = deployment.redis_url.clone();
        if deployment.environment == "production" && configured.is_none() {
            return Err(AroError::Configuration(
                "ARO_REDIS_URL is required in production".to_string(),
            ));
        }

        let rate_limits = RateLimitConfig {
            enabled: env_bool("ARO_RATE_LIMIT_ENABLED").unwrap_or(configured.is_some()),
            auth_per_minute: env_u32("ARO_RATE_LIMIT_AUTH_PER_MINUTE", 10),
            write_per_minute: env_u32("ARO_RATE_LIMIT_WRITE_PER_MINUTE", 120),
            assistant_per_minute: env_u32("ARO_RATE_LIMIT_ASSISTANT_PER_MINUTE", 30),
        };
        let outbox = OutboxStreamConfig {
            enabled: env_bool("ARO_OUTBOX_STREAM_ENABLED").unwrap_or(true),
            stream_key: env::var("ARO_OUTBOX_STREAM_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| format!("{key_prefix}:outbox")),
            max_len: env_usize("ARO_OUTBOX_STREAM_MAX_LEN", 10_000),
            batch_size: env_i64("ARO_OUTBOX_BATCH_SIZE", 50).clamp(1, 500),
        };

        let client = match configured.as_deref() {
            Some(url) => {
                let client = ::redis::Client::open(url).map_err(map_redis_config)?;
                let manager = client
                    .get_connection_manager()
                    .await
                    .map_err(map_redis_runtime)?;
                let services = Self {
                    client: Some(manager),
                    config: RedisConfig { key_prefix },
                    rate_limits,
                    outbox,
                };
                services.ping().await?;
                return Ok(services);
            }
            None => None,
        };

        if client.is_none() {
            // Mode dégradé mono-instance : les locks deviennent des no-op.
            // Explicite au démarrage pour ne jamais croire à une exclusion
            // mutuelle qui n'existe pas (multi-réplicas sans Redis).
            tracing::warn!(
                "ARO_REDIS_URL is not set: rate limiting is off and distributed locks are no-ops (single-instance dev only)"
            );
        }

        Ok(Self {
            client,
            config: RedisConfig { key_prefix },
            rate_limits: RateLimitConfig {
                enabled: false,
                ..rate_limits
            },
            outbox,
        })
    }

    #[cfg(test)]
    pub fn disabled_for_tests() -> Self {
        Self {
            client: None,
            config: RedisConfig {
                key_prefix: "aro:test".to_string(),
            },
            rate_limits: RateLimitConfig {
                enabled: false,
                auth_per_minute: 10,
                write_per_minute: 120,
                assistant_per_minute: 30,
            },
            outbox: OutboxStreamConfig {
                enabled: true,
                stream_key: "aro:test:outbox".to_string(),
                max_len: 10_000,
                batch_size: 50,
            },
        }
    }

    pub fn configured(&self) -> bool {
        self.client.is_some()
    }

    pub fn rate_limit_enabled(&self) -> bool {
        self.client.is_some() && self.rate_limits.enabled
    }

    pub fn outbox_stream_enabled(&self) -> bool {
        self.client.is_some() && self.outbox.enabled
    }

    pub fn outbox_batch_size(&self) -> i64 {
        self.outbox.batch_size
    }

    pub fn key_prefix(&self) -> &str {
        &self.config.key_prefix
    }

    pub async fn ping(&self) -> AroResult<RedisHealth> {
        let Some(client) = self.client.clone() else {
            return Ok(RedisHealth::Disabled);
        };
        let mut connection = client;
        let pong: String = ::redis::cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(map_redis_runtime)?;
        if pong.eq_ignore_ascii_case("PONG") {
            Ok(RedisHealth::Ok)
        } else {
            Err(AroError::RuntimeUnavailable(
                "Redis PING returned an unexpected response".to_string(),
            ))
        }
    }

    pub async fn check_rate_limit(
        &self,
        class: RateLimitClass,
        subject: &str,
    ) -> AroResult<RateLimitDecision> {
        let limit = self.limit_for(class);
        if !self.rate_limit_enabled() {
            return Ok(RateLimitDecision {
                allowed: true,
                limit,
                current: 0,
            });
        }
        let key = self.rate_limit_key(class, subject);
        let mut connection = self
            .client
            .clone()
            .ok_or_else(|| AroError::RuntimeUnavailable("Redis is not configured".to_string()))?;
        let current: i64 = ::redis::Script::new(RATE_LIMIT_SCRIPT)
            .key(key)
            .arg(RATE_LIMIT_WINDOW_SECONDS)
            .invoke_async(&mut connection)
            .await
            .map_err(map_redis_runtime)?;
        let current = current.max(0) as u32;
        Ok(RateLimitDecision {
            allowed: current <= limit,
            limit,
            current,
        })
    }

    pub async fn acquire_lock(&self, key: &str, ttl_seconds: u64) -> AroResult<Option<RedisLock>> {
        let Some(client) = self.client.clone() else {
            // Pas de Redis : verrou no-op assumé (voir warn au démarrage).
            // Les appelants qui exigent une vraie exclusion doivent vérifier
            // `configured()` avant, pas se fier au Some(..) retourné ici.
            return Ok(Some(RedisLock {
                client: None,
                key: key.to_string(),
                token: String::new(),
            }));
        };
        let token = Uuid::new_v4().to_string();
        let mut connection = client.clone();
        let acquired: Option<String> = ::redis::cmd("SET")
            .arg(key)
            .arg(&token)
            .arg("NX")
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .query_async(&mut connection)
            .await
            .map_err(map_redis_runtime)?;
        Ok(acquired.map(|_| RedisLock {
            client: Some(client),
            key: key.to_string(),
            token,
        }))
    }

    pub async fn publish_outbox_event(&self, event: &OutboxEvent) -> AroResult<Option<String>> {
        if !self.outbox_stream_enabled() {
            return Ok(None);
        }
        let payload = serde_json::to_string(&event.payload)
            .map_err(|err| AroError::Unexpected(err.to_string()))?;
        let mut connection = self
            .client
            .clone()
            .ok_or_else(|| AroError::RuntimeUnavailable("Redis is not configured".to_string()))?;
        let stream_id: String = ::redis::cmd("XADD")
            .arg(&self.outbox.stream_key)
            .arg("MAXLEN")
            .arg("~")
            .arg(self.outbox.max_len)
            .arg("*")
            .arg("eventId")
            .arg(event.id.to_string())
            .arg("organizationId")
            .arg(event.organization_id.to_string())
            .arg("eventType")
            .arg(&event.event_type)
            .arg("payload")
            .arg(payload)
            .arg("createdAt")
            .arg(event.created_at.to_rfc3339())
            .query_async(&mut connection)
            .await
            .map_err(map_redis_runtime)?;
        Ok(Some(stream_id))
    }

    pub fn rate_limit_key(&self, class: RateLimitClass, subject: &str) -> String {
        format!(
            "{}:rate:{}:{}",
            self.config.key_prefix,
            class.slug(),
            sanitize_key_part(subject)
        )
    }

    pub fn lock_key(&self, scope: &str, values: &[String]) -> String {
        let suffix = values
            .iter()
            .map(|value| sanitize_key_part(value))
            .collect::<Vec<_>>()
            .join(":");
        format!(
            "{}:lock:{}:{}",
            self.config.key_prefix,
            sanitize_key_part(scope),
            suffix
        )
    }

    fn limit_for(&self, class: RateLimitClass) -> u32 {
        match class {
            RateLimitClass::Auth => self.rate_limits.auth_per_minute,
            RateLimitClass::AuthSession => AUTH_SESSION_LIMIT_PER_MINUTE,
            RateLimitClass::Assistant => self.rate_limits.assistant_per_minute,
            RateLimitClass::Write => self.rate_limits.write_per_minute,
        }
        .max(1)
    }
}

impl RedisLock {
    pub async fn release(mut self) -> AroResult<()> {
        let Some((client, key, token)) = self.take_release_parts() else {
            return Ok(());
        };
        release_lock_parts(client, key, token).await
    }

    fn take_release_parts(&mut self) -> Option<(::redis::aio::ConnectionManager, String, String)> {
        self.client.take().map(|client| {
            (
                client,
                std::mem::take(&mut self.key),
                std::mem::take(&mut self.token),
            )
        })
    }
}

impl Drop for RedisLock {
    fn drop(&mut self) {
        let Some((client, key, token)) = self.take_release_parts() else {
            return;
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        runtime.spawn(async move {
            if let Err(error) = release_lock_parts(client, key, token).await {
                tracing::warn!(%error, "failed to release cancelled Redis lock");
            }
        });
    }
}

async fn release_lock_parts(
    mut connection: ::redis::aio::ConnectionManager,
    key: String,
    token: String,
) -> AroResult<()> {
    let _: i64 = ::redis::Script::new(LOCK_RELEASE_SCRIPT)
        .key(key)
        .arg(token)
        .invoke_async(&mut connection)
        .await
        .map_err(map_redis_runtime)?;
    Ok(())
}

pub fn classify_rate_limit(method: &Method, path: &str) -> Option<RateLimitClass> {
    let path = path
        .strip_prefix("/v1")
        .filter(|stripped| stripped.starts_with('/'))
        .unwrap_or(path);
    if *method == Method::OPTIONS || matches!(path, "/live" | "/health" | "/ready" | "/metrics") {
        return None;
    }
    if *method == Method::POST
        && matches!(
            path,
            "/auth/register"
                | "/auth/login"
                | "/auth/invitations/accept"
                | "/auth/invitations/accept-account"
                | "/auth/invitations/accept-existing-with-password"
        )
    {
        return Some(RateLimitClass::Auth);
    }
    if *method == Method::POST
        && matches!(
            path,
            "/auth/refresh"
                | "/auth/logout"
                | "/auth/switch-organization"
                | "/auth/invitations/accept-existing"
        )
    {
        return Some(RateLimitClass::AuthSession);
    }
    if path.starts_with("/assistant/") {
        return Some(RateLimitClass::Assistant);
    }
    if matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        return Some(RateLimitClass::Write);
    }
    None
}

pub fn sanitize_key_part(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        sanitized = "unknown".to_string();
    }
    sanitized.chars().take(160).collect()
}

fn env_bool(name: &str) -> Option<bool> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .and_then(|value| match value.as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}

fn env_u32(name: &str, default: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn env_i64(name: &str, default: i64) -> i64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn map_redis_config(error: ::redis::RedisError) -> AroError {
    AroError::Configuration(format!("Redis configuration error: {error}"))
}

fn map_redis_runtime(error: ::redis::RedisError) -> AroError {
    AroError::RuntimeUnavailable(format!("Redis is unavailable: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_classification_matches_expected_routes() {
        assert_eq!(
            classify_rate_limit(&Method::POST, "/auth/login"),
            Some(RateLimitClass::Auth)
        );
        assert_eq!(
            classify_rate_limit(&Method::POST, "/auth/invitations/accept"),
            Some(RateLimitClass::Auth)
        );
        assert_eq!(
            classify_rate_limit(&Method::POST, "/auth/refresh"),
            Some(RateLimitClass::AuthSession)
        );
        assert_eq!(
            classify_rate_limit(&Method::POST, "/assistant/stream"),
            Some(RateLimitClass::Assistant)
        );
        assert_eq!(
            classify_rate_limit(&Method::POST, "/v1/assistant/stream"),
            Some(RateLimitClass::Assistant)
        );
        assert_eq!(
            classify_rate_limit(&Method::POST, "/v1/auth/login"),
            Some(RateLimitClass::Auth)
        );
        assert_eq!(
            classify_rate_limit(&Method::POST, "/v11/auth/login"),
            Some(RateLimitClass::Write)
        );
        assert_eq!(
            classify_rate_limit(&Method::PATCH, "/settings"),
            Some(RateLimitClass::Write)
        );
        assert_eq!(classify_rate_limit(&Method::GET, "/health"), None);
    }

    #[test]
    fn keys_are_prefixed_and_sanitized() {
        let services = RedisServices::disabled_for_tests();
        assert_eq!(
            services.rate_limit_key(RateLimitClass::Auth, "ip:127.0.0.1"),
            "aro:test:rate:auth:ip_127.0.0.1"
        );
        assert_eq!(
            services.lock_key("conversation", &["org/1".into(), "conv:2".into()]),
            "aro:test:lock:conversation:org_1:conv_2"
        );
    }

    #[tokio::test]
    async fn disabled_rate_limiter_allows_requests() {
        let services = RedisServices::disabled_for_tests();
        let decision = services
            .check_rate_limit(RateLimitClass::Assistant, "user")
            .await
            .expect("disabled limiter should allow");

        assert!(decision.allowed);
        assert_eq!(decision.current, 0);
        assert_eq!(decision.limit, 30);
    }

    #[test]
    fn sanitize_key_part_falls_back_for_empty_values() {
        assert_eq!(sanitize_key_part(""), "unknown");
        assert_eq!(sanitize_key_part("hello/world token"), "hello_world_token");
    }
}
