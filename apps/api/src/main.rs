mod agent_runner;
mod auth;
mod handlers;
mod invitation_delivery;
mod redis;

use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use aro_core::AroResult;
use aro_files::{build_storage, FileStorage, FileStorageSettings};
use aro_secrets::{
    AwsKmsProvider, AwsSdkKmsClient, EnvelopeCrypto, LocalKeyProvider, VaultTransitProvider,
};
use aro_store::{hash_secret, AroStore, ClaimedFileScanJob, NewUserWithOrg};
use aro_tools::{ToolExecutor, ToolExecutorConfig};
use aro_vector::MemoryVectorService;
use axum::{
    extract::{connect_info::ConnectInfo, DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, HeaderName, HeaderValue, Method},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use url::Url;
use uuid::Uuid;

use crate::{
    auth::{ApiError, JwtService},
    invitation_delivery::InvitationDeliveryService,
    redis::{classify_rate_limit, RedisServices},
};

#[derive(Clone)]
pub struct ApiState {
    pub store: AroStore,
    pub vector: MemoryVectorService,
    pub redis: RedisServices,
    pub jwt: Arc<JwtService>,
    pub secrets_key: String,
    pub started_at: DateTime<Utc>,
    pub deployment: DeploymentConfig,
    pub metrics_token_hash: Option<String>,
    pub access_token_minutes: i64,
    pub refresh_token_days: i64,
    pub refresh_token_family_days: i64,
    pub invitation_delivery_enabled: bool,
    pub integration_flags: IntegrationFeatureFlags,
    pub trust_proxy_headers: bool,
    pub agent_web_access_enabled: bool,
    pub agent_durable_execution_enabled: bool,
    /// Direct network tool execution is intentionally disabled by default. A durable worker
    /// is the normal execution path; this flag is only for explicitly approved operations.
    pub agent_direct_tool_execution_enabled: bool,
    pub file_settings: FileStorageSettings,
    pub file_storage: Arc<dyn FileStorage>,
    pub tools: ToolExecutor,
    pub plugins: Arc<aro_plugins::PluginManager>,
    pub envelope_crypto: EnvelopeCrypto,
    pub agent_keyring: Option<Arc<aro_store::AgentSnapshotKeyring>>,
}

#[derive(Clone, Debug)]
pub struct DeploymentConfig {
    pub environment: String,
    pub deployment_mode: String,
    pub signup_mode: String,
    pub public_base_url: Option<String>,
    pub redis_url: Option<String>,
    pub metrics_bind: Option<String>,
    pub secret_provider: String,
    pub auto_migrate_on_startup: bool,
}

impl DeploymentConfig {
    fn from_env() -> AroResult<Self> {
        let environment = env::var("ARO_ENV")
            .unwrap_or_else(|_| "development".to_string())
            .trim()
            .to_ascii_lowercase();
        let deployment_mode = env::var("ARO_DEPLOYMENT_MODE")
            .unwrap_or_else(|_| "local-dev".to_string())
            .trim()
            .to_ascii_lowercase();
        let signup_mode = env::var("ARO_SIGNUP_MODE")
            .unwrap_or_else(|_| "open".to_string())
            .trim()
            .to_ascii_lowercase();
        validate_choice(
            "ARO_SIGNUP_MODE",
            &signup_mode,
            &["open", "closed", "invite-only"],
        )?;
        let public_base_url = optional_clean_url("ARO_PUBLIC_BASE_URL")?;
        let metrics_bind = env::var("ARO_METRICS_BIND")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let secret_provider =
            env::var("ARO_SECRET_PROVIDER").unwrap_or_else(|_| "local-key".to_string());
        let auto_migrate_on_startup = env_flag("ARO_AUTO_MIGRATE")
            || (environment != "production" && deployment_mode == "local-dev");

        if environment == "production" {
            if auto_migrate_on_startup {
                return Err(aro_core::AroError::Configuration(
                    "ARO_AUTO_MIGRATE must be false in production; use the dedicated migration job"
                        .to_string(),
                ));
            }
            if signup_mode == "open" {
                return Err(aro_core::AroError::Configuration(
                    "ARO_SIGNUP_MODE must be closed or invite-only in production".to_string(),
                ));
            }
            if public_base_url.is_none() {
                return Err(aro_core::AroError::Configuration(
                    "ARO_PUBLIC_BASE_URL is required in production".to_string(),
                ));
            }
            if env::var("ARO_CORS_ALLOWED_ORIGINS")
                .ok()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(aro_core::AroError::Configuration(
                    "ARO_CORS_ALLOWED_ORIGINS must be explicitly configured in production"
                        .to_string(),
                ));
            }
        }

        Ok(Self {
            environment,
            deployment_mode,
            signup_mode,
            public_base_url,
            redis_url: env::var("ARO_REDIS_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            metrics_bind,
            secret_provider,
            auto_migrate_on_startup,
        })
    }

    pub fn public_signup_enabled(&self) -> bool {
        self.signup_mode == "open"
    }
}

#[derive(Clone, Debug, Default)]
pub struct IntegrationFeatureFlags {
    pub api_v3: bool,
    pub workers: bool,
    pub webhooks: bool,
    pub execute: bool,
    pub api_v2: bool,
    pub dual_write: bool,
    pub shadow_read: bool,
    pub read_v2: bool,
    pub oauth_connect: bool,
    pub third_party_network: bool,
}

impl IntegrationFeatureFlags {
    fn from_env() -> Self {
        Self {
            api_v3: env_flag("ARO_INTEGRATIONS_API_V3"),
            workers: env_flag("ARO_INTEGRATIONS_WORKERS"),
            webhooks: env_flag("ARO_INTEGRATIONS_WEBHOOKS"),
            execute: env_flag("ARO_INTEGRATIONS_EXECUTE"),
            api_v2: env_flag("ARO_INTEGRATIONS_API_V2"),
            dual_write: env_flag("ARO_INTEGRATIONS_DUAL_WRITE"),
            shadow_read: env_flag("ARO_INTEGRATIONS_SHADOW_READ"),
            read_v2: env_flag("ARO_INTEGRATIONS_READ_V2"),
            // Third-party credentials are a high-risk capability. They must be explicitly
            // enabled after the deployment has provisioned real provider credentials.
            oauth_connect: env_flag("ARO_INTEGRATIONS_OAUTH_CONNECT"),
            third_party_network: env_flag("ARO_INTEGRATIONS_THIRD_PARTY_NETWORK"),
        }
    }
}

/// The worker uses clamd's INSTREAM protocol. This keeps signature updates in a dedicated
/// antivirus service instead of putting a mutable scanner database in application containers.
#[derive(Clone, Debug)]
struct FileScannerConfig {
    clamd_address: Option<String>,
    timeout_seconds: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileScanVerdict {
    Clean,
    Blocked,
}

impl FileScannerConfig {
    fn from_env(required: bool) -> AroResult<Self> {
        let clamd_address = env::var("ARO_CLAMAV_ADDRESS")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if required && clamd_address.is_none() {
            return Err(aro_core::AroError::Configuration(
                "ARO_CLAMAV_ADDRESS is required when production file scanning is enabled"
                    .to_string(),
            ));
        }
        if let Some(address) = clamd_address.as_deref() {
            let endpoint = Url::parse(&format!("http://{address}")).map_err(|_| {
                aro_core::AroError::Configuration(
                    "ARO_CLAMAV_ADDRESS must be a host:port pair".to_string(),
                )
            })?;
            if endpoint.host_str().is_none()
                || endpoint.port().is_none()
                || !endpoint.username().is_empty()
                || endpoint.password().is_some()
                || endpoint.query().is_some()
                || endpoint.fragment().is_some()
                || endpoint.path() != "/"
            {
                return Err(aro_core::AroError::Configuration(
                    "ARO_CLAMAV_ADDRESS must be a host:port pair".to_string(),
                ));
            }
        }
        let timeout_seconds = match env::var("ARO_FILE_SCAN_TIMEOUT_SECONDS") {
            Ok(value) => value.parse::<u64>().map_err(|_| {
                aro_core::AroError::Configuration(
                    "ARO_FILE_SCAN_TIMEOUT_SECONDS must be an integer between 5 and 300"
                        .to_string(),
                )
            })?,
            Err(_) => 90,
        };
        if !(5..=300).contains(&timeout_seconds) {
            return Err(aro_core::AroError::Configuration(
                "ARO_FILE_SCAN_TIMEOUT_SECONDS must be between 5 and 300".to_string(),
            ));
        }
        Ok(Self {
            clamd_address,
            timeout_seconds,
        })
    }

    fn is_configured(&self) -> bool {
        self.clamd_address.is_some()
    }

    async fn scan(&self, bytes: &[u8]) -> Result<FileScanVerdict, &'static str> {
        let Some(address) = self.clamd_address.as_deref() else {
            return Err("file scanner is not configured");
        };
        let response = tokio::time::timeout(Duration::from_secs(self.timeout_seconds), async {
            let mut stream = tokio::net::TcpStream::connect(address)
                .await
                .map_err(|_| "file scanner is unavailable")?;
            stream
                .write_all(b"zINSTREAM\0")
                .await
                .map_err(|_| "file scanner did not accept the request")?;
            for chunk in bytes.chunks(1024 * 1024) {
                stream
                    .write_all(&(chunk.len() as u32).to_be_bytes())
                    .await
                    .map_err(|_| "file scanner did not accept the request")?;
                stream
                    .write_all(chunk)
                    .await
                    .map_err(|_| "file scanner did not accept the request")?;
            }
            stream
                .write_all(&0_u32.to_be_bytes())
                .await
                .map_err(|_| "file scanner did not accept the request")?;
            stream
                .flush()
                .await
                .map_err(|_| "file scanner did not accept the request")?;
            let mut response = Vec::with_capacity(256);
            let mut buffer = [0_u8; 512];
            loop {
                let read = stream
                    .read(&mut buffer)
                    .await
                    .map_err(|_| "file scanner did not return a result")?;
                if read == 0 {
                    break;
                }
                let terminator = buffer[..read]
                    .iter()
                    .position(|byte| matches!(*byte, 0 | b'\n'));
                let payload_len = terminator.unwrap_or(read);
                if response.len().saturating_add(payload_len) > 4096 {
                    return Err("file scanner returned an invalid result");
                }
                response.extend_from_slice(&buffer[..payload_len]);
                if terminator.is_some() {
                    break;
                }
            }
            Ok::<_, &'static str>(response)
        })
        .await
        .map_err(|_| "file scan timed out")??;
        let response = String::from_utf8_lossy(&response);
        if response.contains(" FOUND") {
            Ok(FileScanVerdict::Blocked)
        } else if response.contains(" OK") {
            Ok(FileScanVerdict::Clean)
        } else {
            Err("file scanner did not complete")
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    match ApiCommand::from_args(env::args().skip(1).collect())? {
        ApiCommand::Serve => serve().await?,
        ApiCommand::Migrate => migrate().await?,
        ApiCommand::Worker => worker().await?,
        ApiCommand::AdminCreateOwner(request) => admin_create_owner(request).await?,
    }
    Ok(())
}

fn init_tracing() {
    let env_filter =
        env::var("RUST_LOG").unwrap_or_else(|_| "aro_api=info,tower_http=info".to_string());
    if env::var("ARO_LOG_FORMAT")
        .map(|value| value.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
    {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .try_init()
            .ok();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .try_init()
            .ok();
    }
}

async fn serve() -> anyhow::Result<()> {
    let deployment = DeploymentConfig::from_env()?;
    let state = build_state_from_env_with_deployment(deployment).await?;
    let bind = env::var("ARO_API_BIND").unwrap_or_else(|_| "127.0.0.1:8710".to_string());
    let addr: SocketAddr = bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("ARO API listening on http://{addr}");
    let http_config = HttpConfig::from_env()?;
    tracing::info!(
        cors_allowed_origins = ?http_config.cors_allowed_origin_labels,
        max_body_bytes = http_config.max_body_bytes,
        max_concurrent_requests = http_config.max_concurrent_requests,
        request_timeout_seconds = http_config.request_timeout.as_secs(),
        "ARO API HTTP hardening configured"
    );
    axum::serve(
        listener,
        router_with_http_config(state, http_config)
            .into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("ARO API shutdown completed");
    Ok(())
}

async fn migrate() -> anyhow::Result<()> {
    let deployment = DeploymentConfig::from_env()?;
    let database_url = runtime_database_url(&deployment, "ARO_MIGRATION_DATABASE_URL")?;
    require_separate_database_identity(&deployment, "ARO_MIGRATION_DATABASE_URL", &database_url)?;
    let store = connect_store(&database_url, false).await?;
    store.migrate().await?;
    tracing::info!("ARO database migrations completed");
    Ok(())
}

async fn worker() -> anyhow::Result<()> {
    let deployment = DeploymentConfig::from_env()?;
    let redis = RedisServices::from_env(&deployment).await?;
    let database_url = runtime_database_url(&deployment, "ARO_WORKER_DATABASE_URL")?;
    require_separate_database_identity(&deployment, "ARO_WORKER_DATABASE_URL", &database_url)?;
    let store = connect_store(&database_url, false).await?;
    store.ensure_migrations_ready().await?;
    if deployment.environment == "production" {
        store.ensure_restricted_database_role().await?;
    }
    let integration_workers_enabled = env_flag("ARO_INTEGRATIONS_WORKERS");
    let integration_worker_id = format!("integration-worker-{}", Uuid::new_v4());
    let invitation_delivery_enabled = InvitationDeliveryService::enabled_from_env()?;
    let invitation_delivery =
        InvitationDeliveryService::from_env(&deployment.environment, invitation_delivery_enabled)?;
    let invitation_secrets_key = if invitation_delivery.is_some() {
        let key = env::var("ARO_SECRETS_KEY").map_err(|_| {
            aro_core::AroError::Configuration(
                "ARO_SECRETS_KEY is required when invitation delivery is enabled".to_string(),
            )
        })?;
        validate_secret_material("ARO_SECRETS_KEY", &key)?;
        Some(key)
    } else {
        None
    };

    let agent_secrets_key = env::var("ARO_SECRETS_KEY").ok();
    let agent_keyring = if let Some(ref key) = agent_secrets_key {
        match aro_store::AgentSnapshotKeyring::single("current", key) {
            Ok(keyring) => Some(std::sync::Arc::new(keyring)),
            Err(err) => {
                tracing::warn!(?err, "failed to initialize agent snapshot keyring; background agent jobs will not be processed");
                None
            }
        }
    } else {
        tracing::warn!(
            "ARO_SECRETS_KEY is not configured; background agent jobs will not be processed"
        );
        None
    };

    let file_settings = FileStorageSettings::from_env()?;
    let file_scanner = FileScannerConfig::from_env(
        deployment.environment == "production" && file_settings.enabled,
    )?;
    let file_storage = if file_scanner.is_configured() {
        Some(build_storage(&file_settings)?)
    } else {
        None
    };
    tracing::info!(
        redis_configured = redis.configured(),
        outbox_stream_enabled = redis.outbox_stream_enabled(),
        file_scanner_configured = file_scanner.is_configured(),
        invitation_delivery_configured = invitation_delivery.is_some(),
        integration_workers_enabled,
        "ARO worker started"
    );
    if file_settings.enabled && !file_scanner.is_configured() {
        tracing::warn!(
            "file scanner is not configured; completed uploads will remain pending and inaccessible"
        );
    }
    let sleep_seconds = env::var("ARO_WORKER_POLL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30);
    let outbox_lease_seconds = env::var("ARO_OUTBOX_LEASE_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(60)
        .clamp(5, 300);
    let file_scan_batch_size = env::var("ARO_FILE_SCAN_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(10)
        .clamp(1, 100);
    let file_scan_max_attempts = env::var("ARO_FILE_SCAN_MAX_ATTEMPTS")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(3)
        .clamp(1, 20);
    let invitation_delivery_batch_size =
        env_i64_in_range("ARO_INVITATION_DELIVERY_BATCH_SIZE", 20, 1, 100)?;
    let invitation_delivery_max_attempts = i32::try_from(env_i64_in_range(
        "ARO_INVITATION_DELIVERY_MAX_ATTEMPTS",
        5,
        1,
        20,
    )?)
    .expect("validated invitation attempt limit fits in i32");
    let idempotency_purge_batch = env::var("ARO_IDEMPOTENCY_PURGE_BATCH")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(1_000)
        .clamp(1, 10_000);
    let refresh_family_purge_batch = env::var("ARO_REFRESH_TOKEN_FAMILY_PURGE_BATCH")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(1_000)
        .clamp(1, 10_000);
    let refresh_family_retention_days = env::var("ARO_REFRESH_TOKEN_FAMILY_RETENTION_DAYS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(30)
        .clamp(1, 365);
    let file_scan_lease_seconds = env::var("ARO_FILE_SCAN_LEASE_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or((file_scanner.timeout_seconds + 30) as i64)
        .clamp(15, 600);
    let invitation_delivery_lease_seconds = env_i64_in_range(
        "ARO_INVITATION_DELIVERY_LEASE_SECONDS",
        invitation_delivery
            .as_ref()
            .map(|delivery| delivery.timeout_seconds() as i64 + 30)
            .unwrap_or(60),
        15,
        600,
    )?;
    let integration_job_batch_size =
        env_i64_in_range("ARO_INTEGRATION_JOB_BATCH_SIZE", 20, 1, 100)?;
    let integration_job_lease_seconds =
        env_i64_in_range("ARO_INTEGRATION_JOB_LEASE_SECONDS", 60, 15, 600)?;
    if file_scanner.is_configured()
        && file_scan_lease_seconds <= file_scanner.timeout_seconds as i64
    {
        return Err(anyhow::anyhow!(
            "ARO_FILE_SCAN_LEASE_SECONDS must exceed ARO_FILE_SCAN_TIMEOUT_SECONDS"
        ));
    }
    if invitation_delivery.as_ref().is_some_and(|delivery| {
        invitation_delivery_lease_seconds <= delivery.timeout_seconds() as i64
    }) {
        return Err(anyhow::anyhow!(
            "ARO_INVITATION_DELIVERY_LEASE_SECONDS must exceed ARO_SMTP_TIMEOUT_SECONDS"
        ));
    }
    let mut interval = tokio::time::interval(Duration::from_secs(sleep_seconds));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = shutdown_signal() => {
                tracing::info!("ARO worker received shutdown signal");
                break;
            }
            _ = interval.tick() => {
                store.ping().await?;
                let exhausted = store
                    .fail_exhausted_file_scan_jobs(file_scan_max_attempts)
                    .await?;
                if exhausted > 0 {
                    tracing::error!(exhausted, "file scans exhausted their retry budget");
                }
                let purged = store
                    .purge_expired_idempotency_requests(idempotency_purge_batch)
                    .await?;
                if purged > 0 {
                    tracing::debug!(purged, "purged expired idempotency records");
                }
                let purged_refresh_families = store
                    .purge_refresh_token_families(
                        refresh_family_purge_batch,
                        refresh_family_retention_days,
                    )
                    .await?;
                if purged_refresh_families > 0 {
                    tracing::debug!(
                        purged_refresh_families,
                        "purged retained refresh-token families"
                    );
                }
                let expired_invitations = store
                    .fail_expired_invitation_deliveries(invitation_delivery_max_attempts)
                    .await?;
                if expired_invitations > 0 {
                    tracing::warn!(
                        expired_invitations,
                        "invitation deliveries expired or exhausted their retry budget"
                    );
                }
                if let (Some(delivery), Some(secrets_key)) =
                    (invitation_delivery.as_ref(), invitation_secrets_key.as_deref())
                {
                    let processed = process_invitation_delivery_batch(
                        &store,
                        delivery,
                        secrets_key,
                        invitation_delivery_batch_size,
                        invitation_delivery_lease_seconds,
                        invitation_delivery_max_attempts,
                    )
                    .await?;
                    if processed > 0 {
                        tracing::info!(processed, "processed invitation delivery jobs");
                    }
                }
                if let (Some(file_storage), true) =
                    (file_storage.as_ref(), file_scanner.is_configured())
                {
                    let processed = process_file_scan_batch(
                        &store,
                        file_storage.as_ref(),
                        &file_scanner,
                        file_scan_batch_size,
                        file_scan_lease_seconds,
                        file_scan_max_attempts,
                    )
                    .await?;
                    if processed > 0 {
                        tracing::info!(processed, "processed file scan jobs");
                    }
                }
                if let Some(ref keyring) = agent_keyring {
                    if let Err(err) = agent_runner::process_agent_jobs(&store, keyring).await {
                        tracing::error!(?err, "error processing background agent jobs");
                    }
                }
                if integration_workers_enabled {
                    let processed = process_integration_job_batch(
                        &store,
                        &integration_worker_id,
                        integration_job_batch_size,
                        integration_job_lease_seconds,
                    ).await?;
                    if processed > 0 {
                        tracing::info!(processed, "processed integration jobs");
                    }
                }
                if redis.outbox_stream_enabled() {
                    let published = publish_outbox_batch(&store, &redis, outbox_lease_seconds).await?;
                    if published > 0 {
                        tracing::info!(published, "published pending outbox events to Redis");
                    } else {
                        tracing::debug!("ARO worker heartbeat; no pending outbox events");
                    }
                } else {
                    tracing::debug!("ARO worker heartbeat; Redis outbox stream disabled");
                }
            }
        }
    }
    Ok(())
}

async fn process_invitation_delivery_batch(
    store: &AroStore,
    delivery_service: &InvitationDeliveryService,
    secrets_key: &str,
    batch_size: i64,
    lease_seconds: i64,
    max_attempts: i32,
) -> anyhow::Result<u64> {
    let mut processed = 0_u64;
    // Claim immediately before each network call. Pre-leasing a batch would let slow earlier SMTP
    // calls consume the lease budget of messages that have not started yet.
    for _ in 0..batch_size.clamp(1, 100) {
        let lease_token = Uuid::new_v4();
        let mut jobs = store
            .claim_invitation_deliveries(1, lease_token, lease_seconds, max_attempts, secrets_key)
            .await?;
        let Some(job) = jobs.pop() else {
            break;
        };
        let invitation_id = job.id;
        let organization_id = job.organization_id;
        let attempts = job.attempts;
        match delivery_service.send(&job).await {
            Ok(()) => {
                let acknowledged = store
                    .acknowledge_invitation_delivery(invitation_id, job.lease_token)
                    .await?;
                if acknowledged {
                    tracing::info!(
                        %invitation_id,
                        %organization_id,
                        attempts,
                        "invitation accepted by SMTP relay"
                    );
                } else {
                    tracing::warn!(
                        %invitation_id,
                        %organization_id,
                        attempts,
                        "invitation delivery lease expired before acknowledgement"
                    );
                }
            }
            Err(failure) => {
                let retryable = failure.retryable();
                let effective_max_attempts = if retryable { max_attempts } else { 1 };
                let exponent = u32::try_from(attempts.saturating_sub(1).clamp(0, 6)).unwrap_or(0);
                let base_delay = 30_i64.saturating_mul(2_i64.pow(exponent));
                let jitter = i64::from(invitation_id.as_bytes()[0] % 17);
                let released = store
                    .release_invitation_delivery_for_retry(
                        invitation_id,
                        job.lease_token,
                        effective_max_attempts,
                        (base_delay + jitter).min(3_600),
                        failure.safe_code(),
                    )
                    .await?;
                tracing::warn!(
                    %invitation_id,
                    %organization_id,
                    attempts,
                    retryable,
                    failure_code = failure.safe_code(),
                    lease_released = released,
                    "invitation delivery failed"
                );
            }
        }
        processed += 1;
    }
    Ok(processed)
}

async fn process_integration_job_batch(
    store: &AroStore,
    worker_id: &str,
    batch_size: i64,
    lease_seconds: i64,
) -> anyhow::Result<u64> {
    let jobs = store
        .claim_integration_jobs(worker_id, batch_size, lease_seconds)
        .await?;
    let mut processed = 0_u64;
    for job in jobs {
        let result = match job.job_type.as_str() {
            "health" | "revoke" => store.finalize_local_integration_job(&job).await,
            _ => {
                store
                    .fail_integration_job(
                        &job,
                        "connector_unavailable",
                        "provider connector execution is not enabled for this job type",
                    )
                    .await
            }
        };
        match result {
            Ok(true) => processed += 1,
            Ok(false) => tracing::warn!(
                job_id = %job.id,
                lease_token = %job.lease_token,
                "integration job lease was lost before completion"
            ),
            Err(error) => {
                tracing::error!(job_id = %job.id, job_type = %job.job_type, ?error, "integration job failed");
                let _ = store
                    .fail_integration_job(&job, "worker_error", "integration worker failed")
                    .await;
            }
        }
    }
    Ok(processed)
}

async fn process_file_scan_batch(
    store: &AroStore,
    file_storage: &dyn FileStorage,
    scanner: &FileScannerConfig,
    batch_size: i64,
    lease_seconds: i64,
    max_attempts: i32,
) -> AroResult<usize> {
    let mut processed = 0;
    for _ in 0..batch_size {
        // Lease only the job that is about to be scanned. Pre-leasing a sequential batch lets
        // later jobs spend most (or all) of their lease waiting behind a slow scanner call.
        let lease_token = Uuid::new_v4();
        let Some(job) = store
            .claim_pending_file_scan_jobs(1, lease_token, lease_seconds, max_attempts)
            .await?
            .into_iter()
            .next()
        else {
            break;
        };
        if process_claimed_file_scan_job(store, file_storage, scanner, job, max_attempts).await? {
            processed += 1;
        }
    }
    Ok(processed)
}

async fn process_claimed_file_scan_job(
    store: &AroStore,
    file_storage: &dyn FileStorage,
    scanner: &FileScannerConfig,
    job: ClaimedFileScanJob,
    max_attempts: i32,
) -> AroResult<bool> {
    let bytes = match file_storage.get(&job.object_key).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(
                job_id = %job.id,
                file_id = %job.file_id,
                error = %error,
                "file scan could not retrieve the object; retrying"
            );
            let _ = store
                .release_file_scan_job_for_retry(
                    job.id,
                    job.lease_token,
                    30,
                    max_attempts,
                    "file scanner could not retrieve object",
                )
                .await?;
            return Ok(false);
        }
    };
    match scanner.scan(&bytes).await {
        Ok(FileScanVerdict::Clean) => {
            if store
                .acknowledge_file_scan_clean(job.id, job.lease_token)
                .await?
                .is_some()
            {
                Ok(true)
            } else {
                tracing::warn!(job_id = %job.id, "file scan lease was lost before clean acknowledgement");
                Ok(false)
            }
        }
        Ok(FileScanVerdict::Blocked) => {
            if store
                .quarantine_file_scan_job(job.id, job.lease_token)
                .await?
                .is_some()
            {
                tracing::warn!(job_id = %job.id, file_id = %job.file_id, "file was quarantined by the scanner");
                Ok(true)
            } else {
                tracing::warn!(job_id = %job.id, "file scan lease was lost before quarantine");
                Ok(false)
            }
        }
        Err(reason) => {
            tracing::warn!(job_id = %job.id, file_id = %job.file_id, reason, "file scan failed; retrying");
            let outcome = store
                .release_file_scan_job_for_retry(job.id, job.lease_token, 30, max_attempts, reason)
                .await?;
            if outcome == Some(true) {
                tracing::error!(job_id = %job.id, file_id = %job.file_id, "file scan permanently failed");
            }
            Ok(false)
        }
    }
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let mut terminate = signal(SignalKind::terminate())
            .expect("failed to install SIGTERM shutdown signal handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C shutdown signal handler");
}

async fn publish_outbox_batch(
    store: &AroStore,
    redis: &RedisServices,
    lease_seconds: i64,
) -> AroResult<usize> {
    let lease_token = Uuid::new_v4();
    let events = store
        .claim_pending_outbox_events_for_worker(
            redis.outbox_batch_size(),
            lease_token,
            lease_seconds,
        )
        .await?;
    let mut published = 0;
    for claimed in events {
        match redis.publish_outbox_event(&claimed.event).await {
            Ok(Some(stream_id)) => {
                if store
                    .acknowledge_outbox_event_by_worker(claimed.event.id, claimed.lease_token)
                    .await?
                    .is_some()
                {
                    tracing::debug!(
                        event_id = %claimed.event.id,
                        organization_id = %claimed.event.organization_id,
                        redis_stream_id = %stream_id,
                        "published outbox event"
                    );
                    published += 1;
                } else {
                    tracing::warn!(
                        event_id = %claimed.event.id,
                        "outbox lease was lost before acknowledgement"
                    );
                }
            }
            Ok(None) => {
                let _ = store
                    .release_outbox_event_for_retry(
                        claimed.event.id,
                        claimed.lease_token,
                        "outbox stream publication is disabled",
                        30,
                    )
                    .await?;
            }
            Err(error) => {
                tracing::warn!(
                    event_id = %claimed.event.id,
                    error = %error,
                    "outbox publication failed; event will be retried"
                );
                let _ = store
                    .release_outbox_event_for_retry(
                        claimed.event.id,
                        claimed.lease_token,
                        "outbox publication failed",
                        30,
                    )
                    .await?;
            }
        }
    }
    Ok(published)
}

async fn admin_create_owner(request: AdminCreateOwnerRequest) -> anyhow::Result<()> {
    let deployment = DeploymentConfig::from_env()?;
    let database_url = runtime_database_url(&deployment, "DATABASE_URL")?;
    let store = connect_store(
        &database_url,
        deployment.environment != "production" && deployment.auto_migrate_on_startup,
    )
    .await?;
    store.ensure_migrations_ready().await?;
    if deployment.environment == "production" {
        store.ensure_restricted_database_role().await?;
    }
    let password_hash = hash_admin_password(&request.password)?;
    let principal = store
        .create_user_with_org(NewUserWithOrg {
            email: request.email,
            name: request.name,
            role_title: None,
            avatar_color: None,
            password_hash,
            organization_name: request.organization_name,
            organization_domain: request.organization_domain,
            organization_description: None,
        })
        .await?;
    tracing::info!(
        user_id = %principal.user.id,
        organization_id = %principal.active_organization.id,
        "ARO owner account created"
    );
    Ok(())
}

#[derive(Debug)]
enum ApiCommand {
    Serve,
    Migrate,
    Worker,
    AdminCreateOwner(AdminCreateOwnerRequest),
}

#[derive(Debug)]
struct AdminCreateOwnerRequest {
    email: String,
    password: String,
    name: String,
    organization_name: String,
    organization_domain: Option<String>,
}

impl ApiCommand {
    fn from_args(args: Vec<String>) -> anyhow::Result<Self> {
        let Some(command) = args.first().map(String::as_str) else {
            return Ok(Self::Serve);
        };
        match command {
            "serve" => Ok(Self::Serve),
            "migrate" => Ok(Self::Migrate),
            "worker" => Ok(Self::Worker),
            "admin" if args.get(1).map(String::as_str) == Some("create-owner") => Ok(
                Self::AdminCreateOwner(AdminCreateOwnerRequest::from_args(&args[2..])?),
            ),
            _ => Err(anyhow::anyhow!(
                "unknown command. Use: aro-api [serve|migrate|worker|admin create-owner]"
            )),
        }
    }
}

impl AdminCreateOwnerRequest {
    fn from_args(args: &[String]) -> anyhow::Result<Self> {
        Ok(Self {
            email: cli_value(args, "--email")
                .or_else(|| env::var("ARO_ADMIN_EMAIL").ok())
                .ok_or_else(|| anyhow::anyhow!("--email or ARO_ADMIN_EMAIL is required"))?,
            password: cli_value(args, "--password")
                .or_else(|| env::var("ARO_ADMIN_PASSWORD").ok())
                .ok_or_else(|| anyhow::anyhow!("--password or ARO_ADMIN_PASSWORD is required"))?,
            name: cli_value(args, "--name")
                .or_else(|| env::var("ARO_ADMIN_NAME").ok())
                .unwrap_or_else(|| "ARO Owner".to_string()),
            organization_name: cli_value(args, "--organization")
                .or_else(|| env::var("ARO_ADMIN_ORGANIZATION").ok())
                .unwrap_or_else(|| "ARO".to_string()),
            organization_domain: cli_value(args, "--domain")
                .or_else(|| env::var("ARO_ADMIN_ORGANIZATION_DOMAIN").ok()),
        })
    }
}

fn cli_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn router(state: ApiState) -> Router {
    router_with_http_config(
        state,
        HttpConfig::from_env().expect("valid API HTTP configuration"),
    )
}

fn router_with_http_config(state: ApiState, http_config: HttpConfig) -> Router {
    let max_body_bytes = http_config.max_body_bytes;
    let upload_body_bytes = state.file_settings.max_bytes.min(512 * 1024 * 1024) as usize;
    let rate_limit_state = state.clone();
    let application_routes = Router::new()
        .route("/.well-known/jwks.json", get(handlers::well_known_jwks))
        .route("/auth/register", post(handlers::auth_register))
        .route("/auth/login", post(handlers::auth_login))
        .route(
            "/auth/password-reset/request",
            post(handlers::auth_password_reset_request),
        )
        .route(
            "/auth/password-reset/confirm",
            post(handlers::auth_password_reset_confirm),
        )
        .route("/auth/mfa/totp/setup", post(handlers::auth_mfa_totp_setup))
        .route(
            "/auth/mfa/totp/enable",
            post(handlers::auth_mfa_totp_enable),
        )
        .route(
            "/auth/mfa/totp/disable",
            post(handlers::auth_mfa_totp_disable),
        )
        .route(
            "/auth/invitations/accept",
            post(handlers::auth_accept_new_invitation),
        )
        .route(
            "/auth/invitations/accept-account",
            post(handlers::auth_accept_invitation),
        )
        .route(
            "/auth/invitations/accept-existing",
            post(handlers::auth_accept_existing_invitation),
        )
        .route(
            "/auth/invitations/accept-existing-with-password",
            post(handlers::auth_accept_existing_invitation_with_password),
        )
        .route("/auth/refresh", post(handlers::auth_refresh))
        .route("/auth/logout", post(handlers::auth_logout))
        .route(
            "/auth/switch-organization",
            post(handlers::auth_switch_organization),
        )
        .route("/bootstrap", get(handlers::bootstrap))
        .route(
            "/users/me",
            axum::routing::patch(handlers::user_profile_update),
        )
        .route(
            "/organizations",
            get(handlers::organizations_list).post(handlers::organization_create),
        )
        .route(
            "/organizations/{organization_id}",
            axum::routing::patch(handlers::organization_update),
        )
        .route(
            "/memberships",
            get(handlers::memberships_list).post(handlers::membership_create),
        )
        .route(
            "/memberships/{membership_id}",
            axum::routing::patch(handlers::membership_update).delete(handlers::membership_delete),
        )
        .route("/invitations", get(handlers::invitations_list))
        .route(
            "/invitations/{invitation_id}",
            axum::routing::delete(handlers::invitation_delete),
        )
        .route(
            "/api-keys",
            get(handlers::api_keys_list).post(handlers::api_key_create),
        )
        .route(
            "/api-keys/{api_key_id}",
            axum::routing::delete(handlers::api_key_revoke),
        )
        .route(
            "/projects",
            get(handlers::projects_list).post(handlers::project_create),
        )
        .route(
            "/projects/{project_id}",
            get(handlers::project_get)
                .patch(handlers::project_update)
                .delete(handlers::project_delete),
        )
        .route(
            "/folders",
            get(handlers::folders_list).post(handlers::folder_create),
        )
        .route(
            "/folders/{folder_id}",
            axum::routing::patch(handlers::folder_update).delete(handlers::folder_delete),
        )
        .route(
            "/conversations",
            get(handlers::conversations_list).post(handlers::conversation_create),
        )
        .route(
            "/conversations/{conversation_id}",
            get(handlers::conversation_get)
                .patch(handlers::conversation_update)
                .delete(handlers::conversation_delete),
        )
        .route(
            "/conversations/{conversation_id}/move",
            axum::routing::patch(handlers::conversation_move),
        )
        .route(
            "/conversations/{conversation_id}/messages",
            get(handlers::messages_list),
        )
        .route(
            "/conversations/{conversation_id}/export",
            get(handlers::conversation_export),
        )
        .route(
            "/messages/{message_id}",
            axum::routing::patch(handlers::message_update),
        )
        .route(
            "/assistant/local-result",
            post(handlers::assistant_local_result),
        )
        .route("/assistant/stream", post(handlers::assistant_stream))
        .route("/files", get(handlers::files_list))
        .route("/files/quota", get(handlers::files_storage_quota))
        .route("/files/quarantined", get(handlers::files_quarantined_list))
        .route("/files/uploads", post(handlers::file_upload_create))
        .route(
            "/files/uploads/{upload_id}/content",
            put(handlers::file_upload_content).layer(DefaultBodyLimit::max(upload_body_bytes)),
        )
        .route(
            "/files/{file_id}",
            get(handlers::file_get).delete(handlers::file_delete),
        )
        .route("/files/{file_id}/content", get(handlers::file_download))
        .route(
            "/agent/runs",
            get(handlers::agent_runs_list).post(handlers::agent_runs_create),
        )
        .route(
            "/agent/permission-profiles",
            get(handlers::agent_permission_profiles_list)
                .put(handlers::agent_permission_profile_upsert),
        )
        .route("/agent/lanes", get(handlers::agent_lanes_list))
        .route(
            "/agent/lanes/{lane_id}/pause",
            post(handlers::agent_lane_pause),
        )
        .route(
            "/agent/lanes/{lane_id}/resume",
            post(handlers::agent_lane_resume),
        )
        .route(
            "/agent/lanes/{lane_id}/priority",
            post(handlers::agent_lane_priority),
        )
        .route("/agent/runs/{run_id}", get(handlers::agent_run_get))
        .route(
            "/agent/runs/{run_id}/pause",
            post(handlers::agent_run_pause),
        )
        .route(
            "/agent/runs/{run_id}/resume",
            post(handlers::agent_run_resume),
        )
        .route(
            "/agent/runs/{run_id}/cancel",
            post(handlers::agent_run_cancel),
        )
        .route(
            "/agent/runs/{run_id}/events",
            get(handlers::agent_run_events),
        )
        .route("/agent/context/search", get(handlers::agent_context_search))
        .route("/tools", get(handlers::tools_list))
        .route("/tools/{tool_id}", get(handlers::tool_get))
        .route("/tools/code/execute", post(handlers::tools_code_execute))
        .route(
            "/tools/document/create",
            post(handlers::tools_document_create),
        )
        .route("/agent/tools/execute", post(handlers::agent_tool_execute))
        .route(
            "/settings",
            get(handlers::settings_get).put(handlers::settings_update),
        )
        .route(
            "/preferences",
            get(handlers::preferences_get).put(handlers::preferences_update),
        )
        .route(
            "/devices",
            get(handlers::devices_list).post(handlers::device_upsert),
        )
        .route("/client-state", get(handlers::client_state_list))
        .route(
            "/client-state/{key}",
            get(handlers::client_state_get)
                .put(handlers::client_state_set)
                .delete(handlers::client_state_delete),
        )
        .route("/admin/event-stats", get(handlers::admin_event_stats))
        .route("/admin/outbox", get(handlers::admin_outbox_pending))
        .route(
            "/admin/outbox/{event_id}/processed",
            post(handlers::admin_outbox_mark_processed),
        )
        .route(
            "/collections/{collection}",
            get(handlers::collection_get).post(handlers::collection_create),
        )
        .route(
            "/collections/{collection}/{item_id}",
            get(handlers::collection_item_get)
                .patch(handlers::collection_item_update)
                .delete(handlers::collection_item_delete),
        )
        .route(
            "/memories",
            get(handlers::collection_get_memories).post(handlers::collection_create_memories),
        )
        .route("/memories/search", get(handlers::memories_search))
        .route(
            "/memories/{item_id}",
            get(handlers::collection_item_get_memories)
                .patch(handlers::collection_item_update_memories)
                .delete(handlers::collection_item_delete_memories),
        )
        .route("/memory-index/status", get(handlers::memory_index_status))
        .route(
            "/memory-index/reindex",
            post(handlers::memory_index_reindex),
        )
        .route(
            "/skills",
            get(handlers::collection_get_skills).post(handlers::collection_create_skills),
        )
        .route(
            "/skills/{item_id}",
            get(handlers::collection_item_get_skills)
                .patch(handlers::collection_item_update_skills)
                .delete(handlers::collection_item_delete_skills),
        )
        .route(
            "/plugins",
            get(handlers::plugins_list_installed).post(handlers::collection_create_plugins),
        )
        .route(
            "/plugin-connections",
            get(handlers::collection_get_plugins).post(handlers::collection_create_plugins),
        )
        .route(
            "/plugin-connections/{item_id}",
            get(handlers::collection_item_get_plugins)
                .patch(handlers::collection_item_update_plugins)
                .delete(handlers::collection_item_delete_plugins),
        )
        .route(
            "/plugins/marketplace",
            get(handlers::plugins_list_marketplace),
        )
        .route("/plugins/install", post(handlers::plugins_install))
        .route("/plugins/custom", post(handlers::plugins_install_custom))
        .route(
            "/plugins/{plugin_id}",
            get(handlers::plugins_get).delete(handlers::plugins_uninstall),
        )
        .route(
            "/plugins/{plugin_id}/toggle",
            post(handlers::plugins_toggle),
        )
        .route(
            "/plugins/{plugin_id}/mcp/{server_name}/test",
            post(handlers::plugins_mcp_test),
        )
        .route(
            "/plugins/{plugin_id}/mcp/{server_name}/call",
            post(handlers::plugins_mcp_call),
        )
        .route(
            "/plugins/{plugin_id}/skills/{skill_id}/invoke",
            post(handlers::plugins_skill_invoke),
        )
        .route(
            "/integration-catalog",
            get(handlers::integration_catalog_v3_list),
        )
        .route(
            "/integration-catalog/{provider_id}",
            get(handlers::integration_catalog_v3_get),
        )
        .route(
            "/integration-catalog/{provider_id}/credentials",
            post(handlers::integration_credentials_v3_create),
        )
        .route(
            "/integration-catalog/{provider_id}/authorization-attempts",
            post(handlers::integration_authorization_attempts_v3_create),
        )
        .route(
            "/integration-authorization-attempts/{attempt_id}",
            get(handlers::integration_authorization_attempt_v3_get)
                .delete(handlers::integration_authorization_attempt_v3_cancel),
        )
        .route(
            "/integration-oauth/callback/{provider_id}",
            get(handlers::integration_oauth_v3_callback_get)
                .post(handlers::integration_oauth_v3_callback_post),
        )
        .route(
            "/integration-installations",
            get(handlers::integration_installations_v3_list),
        )
        .route(
            "/integration-installations/{installation_id}",
            get(handlers::integration_installations_v3_get)
                .patch(handlers::integration_installations_v3_patch),
        )
        .route(
            "/integration-installations/{installation_id}/health-check",
            post(handlers::integration_installations_v3_health_check),
        )
        .route(
            "/integration-installations/{installation_id}/disconnect",
            post(handlers::integration_installations_v3_disconnect),
        )
        .route(
            "/integration-installations/{installation_id}/reauthorize",
            post(handlers::integration_installations_v3_reauthorize),
        )
        .route(
            "/integration-installations/{installation_id}/audit",
            get(handlers::integration_installations_v3_audit),
        )
        .route(
            "/admin/integration-client-profiles",
            post(handlers::admin_integration_client_profile_v3_upsert),
        )
        .route(
            "/integration-providers",
            get(handlers::integration_providers_list),
        )
        .route("/integrations", get(handlers::integrations_list))
        .route(
            "/integrations/connect/status",
            get(handlers::integration_connect_status),
        )
        .route(
            "/integrations/{provider_id}/api-key",
            post(handlers::integration_api_key_create),
        )
        .route(
            "/integrations/{provider_id}/connect/start",
            post(handlers::integration_connect_start),
        )
        .route(
            "/integrations/{provider_id}/oauth/callback",
            get(handlers::integration_oauth_callback),
        )
        .route(
            "/integrations/{connection_id}/validate",
            post(handlers::integration_validate),
        )
        .route(
            "/integrations/{connection_id}",
            get(handlers::integration_get)
                .patch(handlers::integration_patch)
                .delete(handlers::integration_delete),
        )
        .route(
            "/mcp",
            get(handlers::collection_get_mcp).post(handlers::collection_create_mcp),
        )
        .route(
            "/mcp/{item_id}",
            get(handlers::collection_item_get_mcp)
                .patch(handlers::collection_item_update_mcp)
                .delete(handlers::collection_item_delete_mcp),
        )
        .route(
            "/hooks",
            get(handlers::collection_get_hooks).post(handlers::collection_create_hooks),
        )
        .route(
            "/hooks/{item_id}",
            get(handlers::collection_item_get_hooks)
                .patch(handlers::collection_item_update_hooks)
                .delete(handlers::collection_item_delete_hooks),
        )
        .route(
            "/scheduled-tasks",
            get(handlers::collection_get_scheduled_tasks)
                .post(handlers::collection_create_scheduled_tasks),
        )
        .route(
            "/scheduled-tasks/{item_id}",
            get(handlers::collection_item_get_scheduled_tasks)
                .patch(handlers::collection_item_update_scheduled_tasks)
                .delete(handlers::collection_item_delete_scheduled_tasks),
        )
        .route(
            "/scheduled-tasks/{item_id}/run",
            post(handlers::collection_item_run_scheduled_task),
        )
        .route(
            "/teams",
            get(handlers::collection_get_teams).post(handlers::collection_create_teams),
        )
        .route(
            "/teams/{item_id}",
            get(handlers::collection_item_get_teams)
                .patch(handlers::collection_item_update_teams)
                .delete(handlers::collection_item_delete_teams),
        )
        .route(
            "/usage",
            get(handlers::collection_get_usage).post(handlers::collection_create_usage),
        )
        .route(
            "/usage/{item_id}",
            get(handlers::collection_item_get_usage).delete(handlers::collection_item_delete_usage),
        );

    let legacy_routes = application_routes
        .clone()
        .layer(middleware::from_fn(add_legacy_api_headers));

    Router::new()
        .route("/live", get(handlers::live))
        .route("/health", get(handlers::health))
        .route("/healthz", get(handlers::health))
        .route("/ready", get(handlers::ready))
        .route("/readyz", get(handlers::ready))
        .route("/metrics", get(handlers::metrics))
        // `/v1` is the canonical contract. Unversioned routes remain as a compatibility
        // facade until every shipped desktop version has migrated.
        .nest("/v1", application_routes)
        .merge(legacy_routes)
        .layer(middleware::from_fn_with_state(
            rate_limit_state,
            rate_limit_middleware,
        ))
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(ConcurrencyLimitLayer::new(
            http_config.max_concurrent_requests,
        ))
        .layer(middleware::from_fn_with_state(
            http_config.request_timeout,
            request_timeout_middleware,
        ))
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                let request_id = request
                    .headers()
                    .get("x-request-id")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("generated-after-span");
                tracing::info_span!(
                    "http.request",
                    method = %request.method(),
                    path = %request.uri().path(),
                    request_id = %request_id
                )
            },
        ))
        .layer(middleware::from_fn(add_response_headers))
        .layer(cors_layer(&http_config.cors_allowed_origins))
        .with_state(state)
}

async fn build_state_from_env_with_deployment(deployment: DeploymentConfig) -> AroResult<ApiState> {
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| aro_core::AroError::Configuration("DATABASE_URL is required".to_string()))?;
    let jwt_secret = env::var("ARO_JWT_SECRET")
        .map_err(|_| aro_core::AroError::Configuration("ARO_JWT_SECRET is required".to_string()))?;
    let secrets_key = env::var("ARO_SECRETS_KEY").map_err(|_| {
        aro_core::AroError::Configuration("ARO_SECRETS_KEY is required".to_string())
    })?;
    validate_secret_material("ARO_JWT_SECRET", &jwt_secret)?;
    validate_secret_material("ARO_SECRETS_KEY", &secrets_key)?;
    let metrics_token_hash = env::var("ARO_METRICS_TOKEN")
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .map(|token| {
            validate_secret_material("ARO_METRICS_TOKEN", &token)?;
            Ok::<_, aro_core::AroError>(hash_secret(&token))
        })
        .transpose()?;
    if deployment.environment == "production" && metrics_token_hash.is_none() {
        return Err(aro_core::AroError::Configuration(
            "ARO_METRICS_TOKEN is required in production".to_string(),
        ));
    }
    let max_connections = env::var("ARO_DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);
    let access_token_minutes = env_i64_in_range("ARO_ACCESS_TOKEN_MINUTES", 15, 5, 60)?;
    let refresh_token_days = env_i64_in_range("ARO_REFRESH_TOKEN_DAYS", 30, 1, 90)?;
    let refresh_token_family_days = env_i64_in_range("ARO_REFRESH_TOKEN_FAMILY_DAYS", 90, 1, 365)?;
    if refresh_token_family_days < refresh_token_days {
        return Err(aro_core::AroError::Configuration(
            "ARO_REFRESH_TOKEN_FAMILY_DAYS must be greater than or equal to ARO_REFRESH_TOKEN_DAYS"
                .to_string(),
        ));
    }

    let store = if deployment.auto_migrate_on_startup {
        AroStore::connect(&database_url, max_connections).await?
    } else {
        let store = AroStore::connect_without_migrations(&database_url, max_connections).await?;
        store.ensure_migrations_ready().await?;
        store
    };
    if deployment.environment == "production" {
        store.ensure_restricted_database_role().await?;
    }
    let vector = MemoryVectorService::from_env();
    let redis = RedisServices::from_env(&deployment).await?;
    let file_settings = FileStorageSettings::from_env()?;
    if deployment.environment == "production"
        && file_settings.enabled
        && file_settings.backend != "s3"
    {
        return Err(aro_core::AroError::Configuration(
            "ARO_FILE_STORAGE_BACKEND=s3 is required when the Files API is enabled in production"
                .to_string(),
        ));
    }
    if deployment.environment == "production"
        && file_settings.backend == "s3"
        && file_settings
            .s3_endpoint
            .as_deref()
            .is_some_and(|endpoint| !endpoint.starts_with("https://"))
    {
        return Err(aro_core::AroError::Configuration(
            "ARO_S3_ENDPOINT must use HTTPS in production".to_string(),
        ));
    }
    if deployment.environment == "production"
        && file_settings.enabled
        && !env_flag("ARO_FILE_SCAN_REQUIRED")
    {
        return Err(aro_core::AroError::Configuration(
            "ARO_FILE_SCAN_REQUIRED must be enabled when the Files API is enabled in production"
                .to_string(),
        ));
    }
    // Development can deliberately leave the scanner absent; uploads are then kept pending.
    // In production, do not start the API unless its paired worker can execute ClamAV.
    let _file_scanner = FileScannerConfig::from_env(
        deployment.environment == "production" && file_settings.enabled,
    )?;
    let file_storage = build_storage(&file_settings)?;
    let tools = ToolExecutor::try_new(ToolExecutorConfig::default())?;
    let integration_flags = IntegrationFeatureFlags::from_env();
    let envelope_crypto = build_envelope_crypto(&deployment, &secrets_key).await?;
    let agent_secrets_key = env::var("ARO_SECRETS_KEY").ok();
    let agent_keyring = if let Some(ref key) = agent_secrets_key {
        aro_store::AgentSnapshotKeyring::single("current", key)
            .ok()
            .map(std::sync::Arc::new)
    } else {
        None
    };
    if deployment.environment == "production"
        && integration_flags.api_v3
        && !envelope_crypto.production_ready()
    {
        return Err(aro_core::AroError::Configuration(
            "a production KMS or Vault Transit provider is required when integrations v3 are enabled"
                .to_string(),
        ));
    }
    let plugins_dir = env::var("ARO_PLUGINS_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            directories::ProjectDirs::from("local", "aro", "ARO")
                .map(|dirs| dirs.data_local_dir().join("plugins"))
                .unwrap_or_else(|| std::env::temp_dir().join("aro_plugins"))
        });
    let skill_registry = Arc::new(aro_skills::SkillRegistry::new());
    let plugins = Arc::new(aro_plugins::PluginManager::new(plugins_dir, skill_registry));
    if let Err(err) = plugins.load_all().await {
        tracing::warn!(?err, "failed to load installed plugins on startup");
    }

    Ok(ApiState {
        store,
        vector,
        redis,
        jwt: Arc::new(JwtService::with_config(
            jwt_secret,
            env::var("ARO_JWT_ISSUER").unwrap_or_else(|_| "aro-api".to_string()),
            env::var("ARO_JWT_AUDIENCE").unwrap_or_else(|_| "aro-desktop".to_string()),
            env::var("ARO_JWT_KEY_ID")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        )),
        secrets_key,
        started_at: Utc::now(),
        deployment,
        metrics_token_hash,
        access_token_minutes,
        refresh_token_days,
        refresh_token_family_days,
        invitation_delivery_enabled: InvitationDeliveryService::enabled_from_env()?,
        integration_flags,
        trust_proxy_headers: env_flag("ARO_TRUST_PROXY_HEADERS"),
        agent_web_access_enabled: env_flag("ARO_AGENT_WEB_ACCESS"),
        agent_durable_execution_enabled: env_flag("ARO_AGENT_DURABLE_EXECUTION_ENABLED"),
        agent_direct_tool_execution_enabled: env_flag("ARO_AGENT_DIRECT_TOOL_EXECUTION"),
        file_settings,
        file_storage,
        tools,
        plugins,
        envelope_crypto,
        agent_keyring,
    })
}

async fn build_envelope_crypto(
    deployment: &DeploymentConfig,
    legacy_secrets_key: &str,
) -> AroResult<EnvelopeCrypto> {
    match deployment
        .secret_provider
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "local-key" | "local" => {
            let digest = Sha256::digest(legacy_secrets_key.as_bytes());
            let mut master_key = [0_u8; 32];
            master_key.copy_from_slice(&digest);
            Ok(EnvelopeCrypto::new(Arc::new(LocalKeyProvider::new(
                "local-development",
                "1",
                master_key,
            ))))
        }
        "vault-transit" | "vault" => {
            let base_url = env::var("ARO_VAULT_ADDR")
                .map_err(|_| {
                    aro_core::AroError::Configuration("ARO_VAULT_ADDR is required".into())
                })?
                .parse::<Url>()
                .map_err(|_| {
                    aro_core::AroError::Configuration("ARO_VAULT_ADDR is invalid".into())
                })?;
            let mount = env::var("ARO_VAULT_TRANSIT_MOUNT").unwrap_or_else(|_| "transit".into());
            let key_name = env::var("ARO_VAULT_TRANSIT_KEY").map_err(|_| {
                aro_core::AroError::Configuration("ARO_VAULT_TRANSIT_KEY is required".into())
            })?;
            let token = env::var("ARO_VAULT_TOKEN").map_err(|_| {
                aro_core::AroError::Configuration("ARO_VAULT_TOKEN is required".into())
            })?;
            let provider =
                VaultTransitProvider::new(base_url, mount, key_name, token).map_err(|_| {
                    aro_core::AroError::Configuration(
                        "Vault Transit configuration is invalid".into(),
                    )
                })?;
            Ok(EnvelopeCrypto::new(Arc::new(provider)))
        }
        "aws-kms" => {
            let key_id = env::var("ARO_KMS_KEY_ID").map_err(|_| {
                aro_core::AroError::Configuration("ARO_KMS_KEY_ID is required".into())
            })?;
            let key_version = env::var("ARO_KMS_KEY_VERSION").unwrap_or_else(|_| "managed".into());
            let endpoint = env::var("ARO_KMS_ENDPOINT")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let client = AwsSdkKmsClient::from_environment(endpoint)
                .await
                .map_err(|_| {
                    aro_core::AroError::Configuration("AWS KMS configuration is invalid".into())
                })?;
            Ok(EnvelopeCrypto::new(Arc::new(AwsKmsProvider::new(
                client,
                key_id,
                key_version,
            ))))
        }
        _ => Err(aro_core::AroError::Configuration(
            "ARO_SECRET_PROVIDER must be local-key, vault-transit, or aws-kms".into(),
        )),
    }
}

async fn connect_store(database_url: &str, run_migrations: bool) -> AroResult<AroStore> {
    let max_connections = env::var("ARO_DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);
    if run_migrations {
        AroStore::connect(database_url, max_connections).await
    } else {
        AroStore::connect_without_migrations(database_url, max_connections).await
    }
}

fn runtime_database_url(deployment: &DeploymentConfig, variable: &str) -> AroResult<String> {
    if let Ok(value) = env::var(variable) {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return Ok(value);
        }
    }
    if deployment.environment == "production" || variable == "DATABASE_URL" {
        return Err(aro_core::AroError::Configuration(format!(
            "{variable} is required"
        )));
    }
    env::var("DATABASE_URL")
        .map_err(|_| aro_core::AroError::Configuration("DATABASE_URL is required".to_string()))
}

fn require_separate_database_identity(
    deployment: &DeploymentConfig,
    variable: &str,
    database_url: &str,
) -> AroResult<()> {
    if deployment.environment != "production" {
        return Ok(());
    }
    let api_url = env::var("DATABASE_URL")
        .map_err(|_| aro_core::AroError::Configuration("DATABASE_URL is required".to_string()))?;
    let api_identity = database_role_from_url("DATABASE_URL", &api_url)?;
    let command_identity = database_role_from_url(variable, database_url)?;
    if api_identity == command_identity {
        return Err(aro_core::AroError::Configuration(format!(
            "{variable} must use a database identity distinct from DATABASE_URL"
        )));
    }
    Ok(())
}

fn database_role_from_url(variable: &str, database_url: &str) -> AroResult<String> {
    let parsed = Url::parse(database_url).map_err(|_| {
        aro_core::AroError::Configuration(format!("{variable} must be a valid PostgreSQL URL"))
    })?;
    if !matches!(parsed.scheme(), "postgres" | "postgresql") {
        return Err(aro_core::AroError::Configuration(format!(
            "{variable} must use the postgres URL scheme"
        )));
    }
    let role = parsed.username().trim();
    if role.is_empty() {
        return Err(aro_core::AroError::Configuration(format!(
            "{variable} must include a database role"
        )));
    }
    Ok(role.to_string())
}

fn env_flag(name: &str) -> bool {
    env::var(name)
        .ok()
        .is_some_and(|value| env_flag_value(&value))
}

fn env_flag_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn env_i64_in_range(name: &str, default: i64, min: i64, max: i64) -> AroResult<i64> {
    let value = match env::var(name) {
        Ok(value) => value.parse::<i64>().map_err(|_| {
            aro_core::AroError::Configuration(format!(
                "{name} must be an integer between {min} and {max}"
            ))
        })?,
        Err(_) => default,
    };
    if !(min..=max).contains(&value) {
        return Err(aro_core::AroError::Configuration(format!(
            "{name} must be between {min} and {max}"
        )));
    }
    Ok(value)
}

fn validate_choice(name: &str, value: &str, allowed: &[&str]) -> AroResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(aro_core::AroError::Configuration(format!(
            "{name} must be one of: {}",
            allowed.join(", ")
        )))
    }
}

fn optional_clean_url(name: &str) -> AroResult<Option<String>> {
    let Some(value) = env::var(name)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed = Url::parse(&value).map_err(|err| {
        aro_core::AroError::Configuration(format!("{name} contains an invalid URL: {err}"))
    })?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(aro_core::AroError::Configuration(format!(
            "{name} must be a clean http(s) origin/base URL without credentials, query, or fragment"
        )));
    }
    Ok(Some(value))
}

fn validate_secret_material(name: &str, value: &str) -> AroResult<()> {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.len() < 32
        || lower.contains("replace-with")
        || lower.contains("change-me")
        || lower.contains("placeholder")
    {
        return Err(aro_core::AroError::Configuration(format!(
            "{name} must be a non-placeholder secret with at least 32 characters"
        )));
    }
    Ok(())
}

fn hash_admin_password(password: &str) -> AroResult<String> {
    if password.len() < 10 {
        return Err(aro_core::AroError::Configuration(
            "admin password must be at least 10 characters".to_string(),
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|err| aro_core::AroError::Configuration(err.to_string()))
}

#[derive(Clone, Debug)]
struct HttpConfig {
    cors_allowed_origins: Vec<HeaderValue>,
    cors_allowed_origin_labels: Vec<String>,
    max_body_bytes: usize,
    max_concurrent_requests: usize,
    request_timeout: Duration,
}

impl HttpConfig {
    fn from_env() -> AroResult<Self> {
        let cors_allowed_origins =
            parse_cors_allowed_origins(env::var("ARO_CORS_ALLOWED_ORIGINS").ok().as_deref())?;
        let cors_allowed_origin_labels = cors_allowed_origins
            .iter()
            .map(|origin| origin.to_str().unwrap_or("<non-utf8>").to_string())
            .collect();
        let max_body_bytes =
            parse_max_body_bytes(env::var("ARO_HTTP_MAX_BODY_BYTES").ok().as_deref())?;
        let max_concurrent_requests =
            parse_max_concurrent_requests(env::var("ARO_HTTP_MAX_CONCURRENCY").ok().as_deref())?;
        let request_timeout =
            parse_request_timeout(env::var("ARO_HTTP_REQUEST_TIMEOUT_SECONDS").ok().as_deref())?;

        Ok(Self {
            cors_allowed_origins,
            cors_allowed_origin_labels,
            max_body_bytes,
            max_concurrent_requests,
            request_timeout,
        })
    }
}

const DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_BODY_BYTES_CEILING: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_CONCURRENT_REQUESTS: usize = 256;
const MAX_CONCURRENT_REQUESTS_CEILING: usize = 10_000;
const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 30;
const MAX_REQUEST_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_CORS_ALLOWED_ORIGINS: &[&str] = &[
    "http://localhost:1420",
    "http://127.0.0.1:1420",
    "http://tauri.localhost",
    "https://tauri.localhost",
    "tauri://localhost",
];

fn parse_max_body_bytes(raw: Option<&str>) -> AroResult<usize> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_MAX_BODY_BYTES);
    };
    let parsed = raw.parse::<usize>().map_err(|_| {
        aro_core::AroError::Configuration(
            "ARO_HTTP_MAX_BODY_BYTES must be a positive integer".to_string(),
        )
    })?;
    if !(1024..=MAX_BODY_BYTES_CEILING).contains(&parsed) {
        return Err(aro_core::AroError::Configuration(format!(
            "ARO_HTTP_MAX_BODY_BYTES must be between 1024 and {MAX_BODY_BYTES_CEILING}"
        )));
    }
    Ok(parsed)
}

fn parse_max_concurrent_requests(raw: Option<&str>) -> AroResult<usize> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_MAX_CONCURRENT_REQUESTS);
    };
    let parsed = raw.parse::<usize>().map_err(|_| {
        aro_core::AroError::Configuration(
            "ARO_HTTP_MAX_CONCURRENCY must be a positive integer".to_string(),
        )
    })?;
    if !(1..=MAX_CONCURRENT_REQUESTS_CEILING).contains(&parsed) {
        return Err(aro_core::AroError::Configuration(format!(
            "ARO_HTTP_MAX_CONCURRENCY must be between 1 and {MAX_CONCURRENT_REQUESTS_CEILING}"
        )));
    }
    Ok(parsed)
}

fn parse_request_timeout(raw: Option<&str>) -> AroResult<Duration> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECONDS));
    };
    let parsed = raw.parse::<u64>().map_err(|_| {
        aro_core::AroError::Configuration(
            "ARO_HTTP_REQUEST_TIMEOUT_SECONDS must be a positive integer".to_string(),
        )
    })?;
    if !(1..=MAX_REQUEST_TIMEOUT_SECONDS).contains(&parsed) {
        return Err(aro_core::AroError::Configuration(format!(
            "ARO_HTTP_REQUEST_TIMEOUT_SECONDS must be between 1 and {MAX_REQUEST_TIMEOUT_SECONDS}"
        )));
    }
    Ok(Duration::from_secs(parsed))
}

fn parse_cors_allowed_origins(raw: Option<&str>) -> AroResult<Vec<HeaderValue>> {
    let origin_values = raw
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|origins| !origins.is_empty())
        .unwrap_or_else(|| DEFAULT_CORS_ALLOWED_ORIGINS.to_vec());

    if origin_values
        .iter()
        .any(|origin| origin.eq_ignore_ascii_case("none"))
    {
        return Ok(Vec::new());
    }

    origin_values
        .into_iter()
        .map(parse_allowed_origin)
        .collect()
}

fn parse_allowed_origin(origin: &str) -> AroResult<HeaderValue> {
    if origin == "*" {
        return Err(aro_core::AroError::Configuration(
            "ARO_CORS_ALLOWED_ORIGINS must list explicit origins; wildcard is not allowed"
                .to_string(),
        ));
    }

    let parsed = Url::parse(origin).map_err(|err| {
        aro_core::AroError::Configuration(format!(
            "ARO_CORS_ALLOWED_ORIGINS contains an invalid origin '{origin}': {err}"
        ))
    })?;
    match parsed.scheme() {
        "http" | "https" | "tauri" => {}
        other => {
            return Err(aro_core::AroError::Configuration(format!(
                "ARO_CORS_ALLOWED_ORIGINS origin '{origin}' uses unsupported scheme '{other}'"
            )));
        }
    }
    if parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return Err(aro_core::AroError::Configuration(format!(
            "ARO_CORS_ALLOWED_ORIGINS origin '{origin}' must be scheme://host[:port] without path, query, fragment, or credentials"
        )));
    }

    origin.parse::<HeaderValue>().map_err(|_| {
        aro_core::AroError::Configuration(format!(
            "ARO_CORS_ALLOWED_ORIGINS contains an invalid header value: {origin}"
        ))
    })
}

fn cors_layer(origins: &[HeaderValue]) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("idempotency-key"),
        ])
        .expose_headers([
            HeaderName::from_static("x-request-id"),
            header::ETAG,
            header::CONTENT_LENGTH,
            HeaderName::from_static("deprecation"),
            header::LINK,
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("idempotency-replayed"),
        ])
        .max_age(Duration::from_secs(600));

    if origins.is_empty() {
        layer
    } else {
        layer.allow_origin(AllowOrigin::list(origins.to_vec()))
    }
}

async fn request_timeout_middleware(
    State(timeout): State<Duration>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => ApiError::gateway_timeout("request deadline exceeded").into_response(),
    }
}

async fn add_response_headers(mut request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|value| valid_request_id(value))
        .and_then(|value| HeaderValue::from_str(value).ok())
        .unwrap_or_else(|| {
            HeaderValue::from_str(&Uuid::new_v4().to_string())
                .expect("generated UUID is a valid header value")
        });
    request
        .headers_mut()
        .insert(HeaderName::from_static("x-request-id"), request_id.clone());
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(HeaderName::from_static("x-request-id"), request_id);
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'none'; style-src 'unsafe-inline'; script-src 'none'; form-action 'none'; frame-ancestors 'none'; base-uri 'none'",
        ),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    response
}

fn valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

async fn add_legacy_api_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        HeaderName::from_static("deprecation"),
        HeaderValue::from_static("true"),
    );
    response.headers_mut().insert(
        header::LINK,
        HeaderValue::from_static("</v1>; rel=\"successor-version\""),
    );
    response
}

async fn rate_limit_middleware(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let Some(class) = classify_rate_limit(&method, &path) else {
        return Ok(next.run(request).await);
    };
    let subject = rate_limit_subject(&state, &request);
    let decision = state.redis.check_rate_limit(class, &subject).await?;
    if !decision.allowed {
        return Err(ApiError::too_many_requests(format!(
            "rate limit exceeded: {}/{} requests per minute",
            decision.current, decision.limit
        )));
    }
    Ok(next.run(request).await)
}

fn rate_limit_subject(state: &ApiState, request: &Request) -> String {
    if let Some((user_id, organization_id)) = claims_subject(state, request.headers()) {
        return format!("user:{user_id}:org:{organization_id}");
    }
    let peer_ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(address)| address.ip().to_string());
    format!(
        "ip:{}",
        request_ip(
            request.headers(),
            state.trust_proxy_headers,
            peer_ip.as_deref()
        )
    )
}

fn claims_subject(state: &ApiState, headers: &HeaderMap) -> Option<(Uuid, Uuid)> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))?;
    let claims = state.jwt.verify(token).ok()?;
    Some((claims.sub, claims.organization_id))
}

fn request_ip(headers: &HeaderMap, trust_proxy_headers: bool, peer_ip: Option<&str>) -> String {
    if !trust_proxy_headers {
        return peer_ip.unwrap_or("unknown").to_string();
    }
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .or(peer_ip)
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn clamd_test_listener(response: &'static [u8]) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind clamd test listener");
        let address = listener
            .local_addr()
            .expect("test listener address")
            .to_string();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept scan client");
            let mut header = [0_u8; 10];
            stream
                .read_exact(&mut header)
                .await
                .expect("read clamd command");
            assert_eq!(&header, b"zINSTREAM\0");
            let mut received = Vec::new();
            loop {
                let mut length = [0_u8; 4];
                stream
                    .read_exact(&mut length)
                    .await
                    .expect("read stream chunk length");
                let length = u32::from_be_bytes(length) as usize;
                if length == 0 {
                    break;
                }
                let mut chunk = vec![0_u8; length];
                stream
                    .read_exact(&mut chunk)
                    .await
                    .expect("read stream chunk");
                received.extend_from_slice(&chunk);
            }
            assert_eq!(received, b"scanner test payload");
            stream
                .write_all(response)
                .await
                .expect("write clamd response");
        });
        (address, server)
    }

    #[tokio::test]
    async fn clamd_stream_protocol_gates_clean_and_blocked_files() {
        let (address, server) = clamd_test_listener(b"stream: OK\0").await;
        let scanner = FileScannerConfig {
            clamd_address: Some(address),
            timeout_seconds: 5,
        };
        assert_eq!(
            scanner.scan(b"scanner test payload").await,
            Ok(FileScanVerdict::Clean)
        );
        server.await.expect("clean clamd server");

        let (address, server) = clamd_test_listener(b"stream: Eicar-Test-Signature FOUND\0").await;
        let scanner = FileScannerConfig {
            clamd_address: Some(address),
            timeout_seconds: 5,
        };
        assert_eq!(
            scanner.scan(b"scanner test payload").await,
            Ok(FileScanVerdict::Blocked)
        );
        server.await.expect("blocked clamd server");
    }

    #[test]
    fn secret_material_rejects_placeholders_and_short_values() {
        assert!(validate_secret_material("ARO_JWT_SECRET", "short").is_err());
        assert!(validate_secret_material(
            "ARO_SECRETS_KEY",
            "replace-with-at-least-32-random-bytes"
        )
        .is_err());
    }

    #[test]
    fn secret_material_accepts_long_non_placeholder_values() {
        assert!(validate_secret_material(
            "ARO_JWT_SECRET",
            "local-jwt-secret-that-is-long-enough-for-tests"
        )
        .is_ok());
    }

    #[test]
    fn database_urls_expose_a_stable_role_identity() {
        assert_eq!(
            database_role_from_url(
                "DATABASE_URL",
                "postgres://aro_app:secret@postgres:5432/aro"
            )
            .expect("valid database URL"),
            "aro_app"
        );
        assert!(database_role_from_url("DATABASE_URL", "https://postgres/aro").is_err());
        assert!(database_role_from_url("DATABASE_URL", "postgres://postgres/aro").is_err());
    }

    #[test]
    fn cors_defaults_to_explicit_local_origins() {
        let origins = parse_cors_allowed_origins(None).expect("parse default origins");

        assert!(!origins.is_empty());
        assert!(origins
            .iter()
            .any(|origin| origin == HeaderValue::from_static("http://localhost:1420")));
        assert!(origins
            .iter()
            .all(|origin| origin != HeaderValue::from_static("*")));
    }

    #[test]
    fn cors_rejects_wildcard_origin() {
        assert!(parse_cors_allowed_origins(Some("*")).is_err());
    }

    #[test]
    fn cors_rejects_non_origin_urls() {
        assert!(parse_cors_allowed_origins(Some("https://app.example.com/path")).is_err());
        assert!(parse_cors_allowed_origins(Some("https://app.example.com?token=secret")).is_err());
        assert!(parse_cors_allowed_origins(Some("file://app/index.html")).is_err());
    }

    #[test]
    fn cors_can_be_disabled_for_browser_origins() {
        let origins = parse_cors_allowed_origins(Some("none")).expect("parse none");

        assert!(origins.is_empty());
    }

    #[test]
    fn max_body_bytes_are_bounded() {
        assert_eq!(
            parse_max_body_bytes(None).expect("default max body"),
            DEFAULT_MAX_BODY_BYTES
        );
        assert_eq!(
            parse_max_body_bytes(Some("2097152")).expect("custom max body"),
            2 * 1024 * 1024
        );
        assert!(parse_max_body_bytes(Some("12")).is_err());
        assert!(parse_max_body_bytes(Some("999999999")).is_err());
    }

    #[test]
    fn max_concurrent_requests_are_bounded() {
        assert_eq!(
            parse_max_concurrent_requests(None).expect("default request concurrency"),
            DEFAULT_MAX_CONCURRENT_REQUESTS
        );
        assert_eq!(
            parse_max_concurrent_requests(Some("512")).expect("custom request concurrency"),
            512
        );
        assert!(parse_max_concurrent_requests(Some("0")).is_err());
        assert!(parse_max_concurrent_requests(Some("10001")).is_err());
    }

    #[test]
    fn request_timeout_is_bounded() {
        assert_eq!(
            parse_request_timeout(None).expect("default timeout"),
            Duration::from_secs(DEFAULT_REQUEST_TIMEOUT_SECONDS)
        );
        assert_eq!(
            parse_request_timeout(Some("60")).expect("custom timeout"),
            Duration::from_secs(60)
        );
        assert!(parse_request_timeout(Some("0")).is_err());
        assert!(parse_request_timeout(Some("301")).is_err());
    }

    #[test]
    fn request_ids_are_safe_for_structured_logs() {
        assert!(valid_request_id("019f58f0-6d21-7f30-a878-3b9d8f72c8df"));
        assert!(valid_request_id("desktop_01.request-42"));
        assert!(!valid_request_id(""));
        assert!(!valid_request_id("contains spaces"));
        assert!(!valid_request_id(&"x".repeat(129)));
    }

    #[test]
    fn request_ip_uses_forwarded_for_first_hop() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.10, 10.0.0.1"),
        );

        assert_eq!(
            request_ip(&headers, true, Some("192.0.2.20")),
            "203.0.113.10"
        );
    }

    #[test]
    fn request_ip_ignores_proxy_headers_when_the_proxy_is_not_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.10".parse().unwrap());

        assert_eq!(
            request_ip(&headers, false, Some("192.0.2.20")),
            "192.0.2.20"
        );
    }
}
