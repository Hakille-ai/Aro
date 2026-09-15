use std::collections::BTreeMap;

use aro_core::{
    AgentArtifact, AgentContextItem, AgentEvent, AgentLane, AgentLaneStatus, AgentLaneView,
    AgentRun, AgentRunPriority, AgentRunStatus, AgentRunView, AgentStep, AppSettings, AroError,
    AroResult, AssistantMode, AttachmentMode, ChatMessage, ContextPack, Conversation, Device,
    FileObject, FileScanStatus, FileStatus, FileUploadSession, Folder, InferenceMode,
    InvitationDeliveryStatus, LongTermMemory, Membership, MembershipRole, MembershipStatus,
    MessageAttachment, Organization, OrganizationInvitation, OrganizationInvitationStatus,
    OrganizationMember, PermissionCommandApproval, PermissionProfile, Project, PublicApiKey,
    SyncHealth, SyncStatus, User, UserPreferences, MEMORY_STATUS_APPROVED,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;
use zeroize::Zeroize;

mod agent_jobs;
mod governance;
mod integrations_v3;
mod tool_registry;

pub use agent_jobs::*;
pub use integrations_v3::*;
pub use tool_registry::*;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct AroStore {
    pool: PgPool,
}

/// Authenticated tenant identity carried into every row-level-security transaction.
///
/// The fields are intentionally private: callers must construct a non-nil pair and cannot mutate
/// either identifier after validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantContext {
    actor_id: Uuid,
    organization_id: Uuid,
}

impl TenantContext {
    pub fn new(actor_id: Uuid, organization_id: Uuid) -> AroResult<Self> {
        if actor_id.is_nil() || organization_id.is_nil() {
            return Err(AroError::Security(
                "tenant context identifiers must not be nil".to_string(),
            ));
        }
        Ok(Self {
            actor_id,
            organization_id,
        })
    }

    pub const fn actor_id(self) -> Uuid {
        self.actor_id
    }

    pub const fn organization_id(self) -> Uuid {
        self.organization_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseRoleSecurity {
    pub role_name: String,
    pub is_superuser: bool,
    pub bypasses_rls: bool,
    pub row_security_enabled: bool,
    pub read_committed_isolation: bool,
    pub owned_application_tables: i64,
    pub privileged_role_memberships: i64,
}

#[derive(Debug, Clone)]
pub struct NewFileUpload {
    pub file_id: Uuid,
    pub original_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: Option<String>,
    pub storage_backend: String,
    pub bucket: Option<String>,
    pub object_key: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PreparedFileUpload {
    pub file: FileObject,
    pub upload: FileUploadSession,
    pub object_key: String,
    pub storage_backend: String,
    pub bucket: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewApiKeyIntegration {
    pub provider_id: String,
    pub secret: String,
    pub public_config: Value,
    pub account: Value,
    pub secrets_key: String,
}

#[derive(Debug, Clone)]
pub struct NewIntegrationOAuthState {
    pub provider_id: String,
    pub state_hash: String,
    pub nonce_hash: String,
    pub pkce_verifier: String,
    pub redirect_uri: String,
    pub requested_scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
    pub secrets_key: String,
}

#[derive(Debug, Clone)]
pub struct NewOAuthIntegration {
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub provider_id: String,
    pub manifest_version: String,
    pub granted_scopes: Vec<String>,
    pub account: Value,
    pub credential_payload: Value,
    pub secrets_key: String,
}

#[derive(Debug, Clone)]
pub struct StoredIntegrationOAuthState {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub provider_id: String,
    pub manifest_version: String,
    pub pkce_verifier: String,
    pub redirect_uri: String,
    pub requested_scopes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StoredFileObject {
    pub file: FileObject,
    pub storage_backend: String,
    pub bucket: Option<String>,
    pub object_key: String,
}

/// A file scan job claimed by exactly one worker. The lease token is required for every
/// state transition so a slow or restarted worker cannot approve another worker's job.
#[derive(Debug, Clone)]
pub struct ClaimedFileScanJob {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub file_id: Uuid,
    pub owner_user_id: Uuid,
    pub object_key: String,
    pub attempts: i32,
    pub lease_token: Uuid,
}

/// A leased invitation delivery. This type intentionally does not implement `Debug` or
/// `Serialize`: `token` is a live bearer credential that must exist only in worker memory.
pub struct ClaimedInvitationDelivery {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub organization_name: String,
    pub email: String,
    pub name: String,
    pub role: MembershipRole,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub attempts: i32,
    pub lease_token: Uuid,
}

impl Drop for ClaimedInvitationDelivery {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

pub const IDEMPOTENCY_DEFAULT_TTL_SECONDS: i64 = 24 * 60 * 60;
pub const IDEMPOTENCY_EXECUTION_LEASE_SECONDS: i64 = 2 * 60;
pub const IDEMPOTENCY_MIN_TTL_SECONDS: i64 = IDEMPOTENCY_EXECUTION_LEASE_SECONDS;
pub const IDEMPOTENCY_MAX_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
pub const IDEMPOTENCY_MAX_RESPONSE_HEADERS_BYTES: usize = 16 * 1024;
pub const IDEMPOTENCY_MAX_RESPONSE_BODY_BYTES: usize = 1024 * 1024;

/// The durable identity of one idempotent operation. Keeping the actor and scope in the
/// uniqueness boundary prevents a key chosen by one user or endpoint from suppressing another
/// operation in the same organization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyRequestKey {
    pub organization_id: Uuid,
    pub actor_id: Uuid,
    pub scope: String,
    pub key: String,
}

/// SHA-256 of the canonical request representation selected by the transport layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdempotencyRequestHash([u8; 32]);

impl IdempotencyRequestHash {
    pub fn digest(canonical_request: impl AsRef<[u8]>) -> Self {
        Self(Sha256::digest(canonical_request.as_ref()).into())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// An HTTP response stored in replayable form. The body remains encoded bytes so replay does not
/// change JSON formatting, signatures, or other representation details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyResponse {
    pub status_code: u16,
    pub headers: BTreeMap<String, Vec<String>>,
    pub body: Vec<u8>,
}

/// Capability returned only to the request that atomically acquired an idempotency key. The
/// execution token changes whenever an expired key is reused, fencing late completions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyLease {
    record_id: Uuid,
    identity: IdempotencyRequestKey,
    request_hash: IdempotencyRequestHash,
    execution_token: Uuid,
    lease_expires_at: DateTime<Utc>,
    record_expires_at: DateTime<Utc>,
}

impl IdempotencyLease {
    pub fn record_id(&self) -> Uuid {
        self.record_id
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.lease_expires_at
    }

    pub fn record_expires_at(&self) -> DateTime<Utc> {
        self.record_expires_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyBegin {
    Acquired(IdempotencyLease),
    InProgress { expires_at: DateTime<Utc> },
    Replay(IdempotencyResponse),
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyCompletion {
    Completed,
    AlreadyCompleted(IdempotencyResponse),
    LeaseLost,
}

impl AroStore {
    pub async fn connect(database_url: &str, max_connections: u32) -> AroResult<Self> {
        let store = Self::connect_without_migrations(database_url, max_connections).await?;
        store.migrate().await?;
        Ok(store)
    }

    pub async fn connect_without_migrations(
        database_url: &str,
        max_connections: u32,
    ) -> AroResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .after_connect(|connection, _metadata| {
                Box::pin(async move {
                    sqlx::query(
                        "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL READ COMMITTED",
                    )
                    .execute(&mut *connection)
                    .await?;
                    Ok(())
                })
            })
            .connect(database_url)
            .await
            .map_err(map_sqlx)?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Starts a short-lived tenant transaction and binds its RLS identity with transaction-local
    /// PostgreSQL settings. Membership is revalidated and share-locked in the same snapshot, so
    /// revocation cannot race the protected operation after this method returns.
    pub async fn begin_tenant_tx(
        &self,
        context: TenantContext,
    ) -> AroResult<Transaction<'_, Postgres>> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query(
            r#"
            SELECT set_config('aro.actor_id', $1, true),
                   set_config('aro.organization_id', $2, true)
            "#,
        )
        .bind(context.actor_id().to_string())
        .bind(context.organization_id().to_string())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let membership_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT m.id
            FROM memberships m
            JOIN users u
              ON u.id = m.user_id
             AND u.deleted_at IS NULL
            JOIN organizations o
              ON o.id = m.organization_id
             AND o.deleted_at IS NULL
            WHERE m.user_id = $1
              AND m.organization_id = $2
              AND m.status = 'active'
              AND m.deleted_at IS NULL
            FOR SHARE OF m, u, o
            "#,
        )
        .bind(context.actor_id())
        .bind(context.organization_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if membership_id.is_none() {
            return Err(AroError::Security("organization access denied".to_string()));
        }
        Ok(tx)
    }

    pub async fn migrate(&self) -> AroResult<()> {
        MIGRATOR.run(&self.pool).await.map_err(map_migrate)
    }

    pub async fn ping(&self) -> AroResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(map_sqlx)
    }

    pub async fn database_role_security(&self) -> AroResult<DatabaseRoleSecurity> {
        let row = sqlx::query(
            r#"
            SELECT
              current_user AS role_name,
              r.rolsuper AS is_superuser,
              r.rolbypassrls AS bypasses_rls,
              current_setting('row_security') = 'on' AS row_security_enabled,
              current_setting('transaction_isolation') = 'read committed'
                AS read_committed_isolation,
              (
                SELECT COUNT(*)
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE c.relowner = r.oid
                  AND c.relkind IN ('r', 'p')
                  AND n.nspname = 'public'
                  AND c.relname <> '_sqlx_migrations'
              ) AS owned_application_tables
              ,(
                SELECT COUNT(*)
                FROM pg_roles inherited
                WHERE inherited.rolname <> current_user
                  AND pg_has_role(current_user, inherited.oid, 'MEMBER')
                  AND (
                    inherited.rolsuper
                    OR inherited.rolbypassrls
                    OR EXISTS (
                      SELECT 1
                      FROM pg_class owned
                      JOIN pg_namespace owned_namespace
                        ON owned_namespace.oid = owned.relnamespace
                      WHERE owned.relowner = inherited.oid
                        AND owned.relkind IN ('r', 'p')
                        AND owned_namespace.nspname = 'public'
                        AND owned.relname <> '_sqlx_migrations'
                    )
                  )
              ) AS privileged_role_memberships
            FROM pg_roles r
            WHERE r.rolname = current_user
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(DatabaseRoleSecurity {
            role_name: row.get("role_name"),
            is_superuser: row.get("is_superuser"),
            bypasses_rls: row.get("bypasses_rls"),
            row_security_enabled: row.get("row_security_enabled"),
            read_committed_isolation: row.get("read_committed_isolation"),
            owned_application_tables: row.get("owned_application_tables"),
            privileged_role_memberships: row.get("privileged_role_memberships"),
        })
    }

    /// Production API and worker connections must never use the migration/table-owner role.
    pub async fn ensure_restricted_database_role(&self) -> AroResult<DatabaseRoleSecurity> {
        let security = self.database_role_security().await?;
        if security.is_superuser {
            return Err(AroError::Configuration(format!(
                "database role `{}` must be NOSUPERUSER",
                security.role_name
            )));
        }
        if security.bypasses_rls {
            return Err(AroError::Configuration(format!(
                "database role `{}` must be NOBYPASSRLS",
                security.role_name
            )));
        }
        if !security.row_security_enabled {
            return Err(AroError::Configuration(
                "PostgreSQL row_security must be enabled".to_string(),
            ));
        }
        if !security.read_committed_isolation {
            return Err(AroError::Configuration(
                "PostgreSQL transaction isolation must be READ COMMITTED".to_string(),
            ));
        }
        if security.owned_application_tables > 0 {
            return Err(AroError::Configuration(format!(
                "database role `{}` owns {} application tables; use the restricted runtime role instead of the migrator",
                security.role_name, security.owned_application_tables
            )));
        }
        if security.privileged_role_memberships > 0 {
            return Err(AroError::Configuration(format!(
                "database role `{}` is a member of a privileged or table-owning role",
                security.role_name
            )));
        }
        Ok(security)
    }

    pub async fn ensure_migrations_ready(&self) -> AroResult<()> {
        let applied_count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM _sqlx_migrations
            WHERE success = true
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        let expected_count = MIGRATOR.migrations.len() as i64;
        if applied_count < expected_count {
            return Err(AroError::Configuration(format!(
                "database migrations are not ready: {applied_count}/{expected_count} applied"
            )));
        }
        Ok(())
    }

    /// Atomically acquires an idempotency key or reports the state created by another request.
    /// A matching completed request returns its canonical response; reusing the key with a
    /// different request hash is always a conflict until the record expires.
    pub async fn begin_idempotency_request(
        &self,
        identity: &IdempotencyRequestKey,
        request_hash: IdempotencyRequestHash,
        ttl_seconds: i64,
    ) -> AroResult<IdempotencyBegin> {
        validate_idempotency_request_key(identity)?;
        validate_idempotency_ttl(ttl_seconds)?;

        let execution_token = Uuid::new_v4();
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let acquired = sqlx::query(
            r#"
            INSERT INTO idempotency_requests
              (organization_id, actor_id, scope, idempotency_key, request_hash,
               execution_token, lease_expires_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6,
                    now() + ($7::bigint * interval '1 second'),
                    now() + ($8::bigint * interval '1 second'))
            ON CONFLICT (organization_id, actor_id, scope, idempotency_key)
            DO UPDATE SET
              request_hash = excluded.request_hash,
              status = 'in_progress',
              execution_token = excluded.execution_token,
              lease_expires_at = excluded.lease_expires_at,
              response_status = NULL,
              response_headers = NULL,
              response_body = NULL,
              created_at = now(),
              updated_at = now(),
              completed_at = NULL,
              expires_at = excluded.expires_at
            WHERE idempotency_requests.expires_at <= now()
               OR (
                    idempotency_requests.status = 'in_progress'
                    AND idempotency_requests.lease_expires_at <= now()
                    AND idempotency_requests.request_hash = excluded.request_hash
                  )
            RETURNING id, execution_token, lease_expires_at, expires_at
            "#,
        )
        .bind(identity.organization_id)
        .bind(identity.actor_id)
        .bind(&identity.scope)
        .bind(&identity.key)
        .bind(request_hash.as_bytes().to_vec())
        .bind(execution_token)
        .bind(IDEMPOTENCY_EXECUTION_LEASE_SECONDS)
        .bind(ttl_seconds)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let outcome = if let Some(row) = acquired {
            IdempotencyBegin::Acquired(IdempotencyLease {
                record_id: row.get("id"),
                identity: identity.clone(),
                request_hash,
                execution_token: row.get("execution_token"),
                lease_expires_at: row.get("lease_expires_at"),
                record_expires_at: row.get("expires_at"),
            })
        } else {
            // This is intentionally a second statement. Under READ COMMITTED, an INSERT with
            // ON CONFLICT may wait for a concurrent uncommitted insert; the fresh snapshot here
            // then observes the winning row and classifies it deterministically.
            let row = sqlx::query(
                r#"
                SELECT request_hash, status, response_status, response_headers, response_body,
                       lease_expires_at, expires_at
                FROM idempotency_requests
                WHERE organization_id = $1
                  AND actor_id = $2
                  AND scope = $3
                  AND idempotency_key = $4
                "#,
            )
            .bind(identity.organization_id)
            .bind(identity.actor_id)
            .bind(&identity.scope)
            .bind(&identity.key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?
            .ok_or_else(|| {
                AroError::Memory(
                    "idempotency record disappeared while it was being acquired".to_string(),
                )
            })?;
            let stored_hash: Vec<u8> = row.get("request_hash");
            if stored_hash.as_slice() != request_hash.as_bytes() {
                IdempotencyBegin::Conflict
            } else {
                match row.get::<String, _>("status").as_str() {
                    "in_progress" => IdempotencyBegin::InProgress {
                        expires_at: row.get("lease_expires_at"),
                    },
                    "completed" => IdempotencyBegin::Replay(map_idempotency_response_row(&row)?),
                    status => {
                        return Err(AroError::Memory(format!(
                            "invalid persisted idempotency status '{status}'"
                        )));
                    }
                }
            }
        };

        tx.commit().await.map_err(map_sqlx)?;
        Ok(outcome)
    }

    /// Completes only the generation that acquired the key. A late owner cannot overwrite a
    /// response after expiration/reacquisition, and retrying the same completion returns the
    /// already persisted canonical response.
    pub async fn complete_idempotency_request(
        &self,
        lease: &IdempotencyLease,
        response: &IdempotencyResponse,
    ) -> AroResult<IdempotencyCompletion> {
        let response_headers = validate_idempotency_response(response)?;
        let response_status = response.status_code as i16;
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let completed = sqlx::query(
            r#"
            UPDATE idempotency_requests
            SET status = 'completed',
                response_status = $8,
                response_headers = $9,
                response_body = $10,
                completed_at = now(),
                updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND actor_id = $3
              AND scope = $4
              AND idempotency_key = $5
              AND request_hash = $6
              AND execution_token = $7
              AND status = 'in_progress'
              AND lease_expires_at > now()
              AND expires_at > now()
            RETURNING id
            "#,
        )
        .bind(lease.record_id)
        .bind(lease.identity.organization_id)
        .bind(lease.identity.actor_id)
        .bind(&lease.identity.scope)
        .bind(&lease.identity.key)
        .bind(lease.request_hash.as_bytes().to_vec())
        .bind(lease.execution_token)
        .bind(response_status)
        .bind(response_headers)
        .bind(&response.body)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let outcome = if completed.is_some() {
            IdempotencyCompletion::Completed
        } else {
            let row = sqlx::query(
                r#"
                SELECT status, response_status, response_headers, response_body
                FROM idempotency_requests
                WHERE id = $1
                  AND organization_id = $2
                  AND actor_id = $3
                  AND scope = $4
                  AND idempotency_key = $5
                  AND request_hash = $6
                  AND execution_token = $7
                  AND expires_at > now()
                FOR UPDATE
                "#,
            )
            .bind(lease.record_id)
            .bind(lease.identity.organization_id)
            .bind(lease.identity.actor_id)
            .bind(&lease.identity.scope)
            .bind(&lease.identity.key)
            .bind(lease.request_hash.as_bytes().to_vec())
            .bind(lease.execution_token)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?;

            match row {
                Some(row) if row.get::<String, _>("status") == "completed" => {
                    IdempotencyCompletion::AlreadyCompleted(map_idempotency_response_row(&row)?)
                }
                _ => IdempotencyCompletion::LeaseLost,
            }
        };

        tx.commit().await.map_err(map_sqlx)?;
        Ok(outcome)
    }

    /// Deletes expired records in bounded, skip-locked batches so cleanup can safely run from
    /// several maintenance workers without blocking active completions.
    pub async fn purge_expired_idempotency_requests(&self, limit: i64) -> AroResult<u64> {
        let bounded_limit = limit.clamp(1, 10_000) as i32;
        let deleted: i64 = sqlx::query_scalar("SELECT aro_purge_expired_idempotency_requests($1)")
            .bind(bounded_limit)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx)?;
        u64::try_from(deleted)
            .map_err(|_| AroError::Unexpected("idempotency purge returned a negative count".into()))
    }

    /// Purges complete refresh-token lineages only after their absolute expiry or revocation has
    /// aged beyond the configured forensic/reuse-detection retention window.
    pub async fn purge_refresh_token_families(
        &self,
        limit: i64,
        retention_days: i64,
    ) -> AroResult<u64> {
        let bounded_limit = limit.clamp(1, 10_000) as i32;
        let bounded_retention_days = retention_days.clamp(1, 365) as i32;
        let deleted: i64 = sqlx::query_scalar("SELECT aro_purge_refresh_token_families($1, $2)")
            .bind(bounded_limit)
            .bind(bounded_retention_days)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx)?;
        u64::try_from(deleted).map_err(|_| {
            AroError::Unexpected("refresh-token family purge returned a negative count".into())
        })
    }

    pub async fn create_user_with_org(&self, request: NewUserWithOrg) -> AroResult<AuthPrincipal> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let user = sqlx::query(
            r#"
            INSERT INTO users (email, name, role_title, avatar_color, password_hash)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, email, name, role_title, avatar_color, created_at, updated_at
            "#,
        )
        .bind(request.email.trim().to_lowercase())
        .bind(request.name)
        .bind(request.role_title)
        .bind(request.avatar_color)
        .bind(request.password_hash)
        .fetch_one(&mut *tx)
        .await
        .map(map_user_row)
        .map_err(map_sqlx)?;

        let organization = sqlx::query(
            r#"
            INSERT INTO organizations (name, domain, description)
            VALUES ($1, $2, $3)
            RETURNING id, name, domain, description, created_at, updated_at
            "#,
        )
        .bind(request.organization_name)
        .bind(request.organization_domain)
        .bind(request.organization_description)
        .fetch_one(&mut *tx)
        .await
        .map(map_organization_row)
        .map_err(map_sqlx)?;

        let membership = create_membership(
            &mut tx,
            user.id,
            organization.id,
            MembershipRole::Owner,
            MembershipStatus::Active,
        )
        .await?;

        upsert_default_preferences(&mut tx, user.id).await?;
        upsert_default_settings(&mut tx, organization.id, user.id).await?;

        tx.commit().await.map_err(map_sqlx)?;
        Ok(AuthPrincipal {
            user,
            active_organization: organization,
            memberships: vec![membership],
        })
    }

    pub async fn find_user_credentials(&self, email: &str) -> AroResult<Option<UserCredentials>> {
        sqlx::query(
            r#"
            SELECT id, email, name, role_title, avatar_color, password_hash, created_at, updated_at
            FROM users
            WHERE email = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(email.trim().to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(map_user_credentials_row))
        .map_err(map_sqlx)
    }

    pub async fn create_password_reset_token(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> AroResult<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO password_reset_tokens (id, user_id, token_hash, expires_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(token_hash)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(id)
    }

    pub async fn consume_password_reset_token(
        &self,
        token_hash: &str,
    ) -> AroResult<Option<(Uuid, Uuid)>> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let row = sqlx::query(
            r#"
            SELECT id, user_id, expires_at, used_at
            FROM password_reset_tokens
            WHERE token_hash = $1 AND used_at IS NULL AND expires_at > NOW()
            FOR UPDATE
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let Some(row) = row else {
            return Ok(None);
        };

        let token_id: Uuid = row.get("id");
        let user_id: Uuid = row.get("user_id");

        sqlx::query(
            r#"
            UPDATE password_reset_tokens
            SET used_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(token_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        tx.commit().await.map_err(map_sqlx)?;

        Ok(Some((user_id, token_id)))
    }

    pub async fn update_user_password(
        &self,
        user_id: Uuid,
        new_password_hash: &str,
    ) -> AroResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE users
            SET password_hash = $1, updated_at = NOW()
            WHERE id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(new_password_hash)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;

        if result.rows_affected() == 0 {
            return Err(AroError::Unexpected("user not found".to_string()));
        }

        Ok(())
    }

    pub async fn update_user_totp_secret(&self, user_id: Uuid, secret: &str) -> AroResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET totp_secret = $1, updated_at = NOW()
            WHERE id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(secret)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn enable_user_totp(&self, user_id: Uuid) -> AroResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET totp_enabled_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL AND totp_secret IS NOT NULL
            "#,
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn disable_user_totp(&self, user_id: Uuid) -> AroResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET totp_secret = NULL, totp_enabled_at = NULL, updated_at = NOW()
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn get_user_totp_info(
        &self,
        user_id: Uuid,
    ) -> AroResult<Option<(Option<String>, Option<DateTime<Utc>>)>> {
        let row = sqlx::query(
            r#"
            SELECT totp_secret, totp_enabled_at
            FROM users
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(row.map(|r| (r.get("totp_secret"), r.get("totp_enabled_at"))))
    }

    // Huit paramètres stables d'API publique : on garde la signature.
    #[allow(clippy::too_many_arguments)]
    pub async fn log_audit_event(
        &self,
        organization_id: Uuid,
        actor_user_id: Option<Uuid>,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        details: Value,
        ip_address: Option<&str>,
    ) -> AroResult<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO audit_logs (id, organization_id, actor_user_id, action, resource_type, resource_id, details, ip_address)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(id)
        .bind(organization_id)
        .bind(actor_user_id)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(details)
        .bind(ip_address)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(id)
    }

    pub async fn principal_for_user(&self, user_id: Uuid) -> AroResult<Option<AuthPrincipal>> {
        let user = sqlx::query(
            r#"
            SELECT id, email, name, role_title, avatar_color, created_at, updated_at
            FROM users
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .map(map_user_row);

        let Some(user) = user else {
            return Ok(None);
        };

        // Invitations and suspended memberships are never principals. Keeping this filter at
        // the authentication boundary prevents a valid password from turning an unaccepted
        // invite into an authenticated organization session.
        let memberships = self
            .list_memberships_for_user(user.id)
            .await?
            .into_iter()
            .filter(|membership| membership.status == MembershipStatus::Active)
            .collect::<Vec<_>>();
        let Some(active_membership) = memberships.first() else {
            return Ok(None);
        };
        let active_organization = self
            .get_organization(active_membership.organization_id)
            .await?
            .ok_or_else(|| AroError::Memory("active organization missing".to_string()))?;

        Ok(Some(AuthPrincipal {
            user,
            active_organization,
            memberships,
        }))
    }

    pub async fn principal_for_user_in_org(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Option<AuthPrincipal>> {
        let user = sqlx::query(
            r#"
            SELECT id, email, name, role_title, avatar_color, created_at, updated_at
            FROM users
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .map(map_user_row);

        let Some(user) = user else {
            return Ok(None);
        };

        self.ensure_org_access(user.id, organization_id).await?;
        let memberships = self
            .list_memberships_for_user(user.id)
            .await?
            .into_iter()
            .filter(|membership| membership.status == MembershipStatus::Active)
            .collect();
        let active_organization = self
            .get_organization(organization_id)
            .await?
            .ok_or_else(|| AroError::Security("organization not found".to_string()))?;

        Ok(Some(AuthPrincipal {
            user,
            active_organization,
            memberships,
        }))
    }

    pub async fn create_refresh_token(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        device_id: Option<Uuid>,
        refresh_token: &str,
        ttl_days: i64,
        family_ttl_days: i64,
    ) -> AroResult<DateTime<Utc>> {
        if !(1..=90).contains(&ttl_days) {
            return Err(AroError::Configuration(
                "refresh token lifetime must be between 1 and 90 days".to_string(),
            ));
        }
        if !(1..=365).contains(&family_ttl_days) || family_ttl_days < ttl_days {
            return Err(AroError::Configuration(
                "refresh token family lifetime must be between the token lifetime and 365 days"
                    .to_string(),
            ));
        }
        let now = Utc::now();
        let expires_at = now + Duration::days(ttl_days);
        let absolute_expires_at = now + Duration::days(family_ttl_days);
        let family_id = Uuid::new_v4();
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO refresh_token_families
              (id, user_id, device_id, absolute_expires_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            "#,
        )
        .bind(family_id)
        .bind(user_id)
        .bind(device_id)
        .bind(absolute_expires_at)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens
              (user_id, organization_id, device_id, token_hash, expires_at, family_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(user_id)
        .bind(organization_id)
        .bind(device_id)
        .bind(hash_secret(refresh_token))
        .bind(expires_at)
        .bind(family_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(expires_at)
    }

    pub async fn consume_refresh_token(
        &self,
        refresh_token: &str,
    ) -> AroResult<Option<RefreshTokenPrincipal>> {
        let mut retry = 0_u64;
        loop {
            match self.consume_refresh_token_once(refresh_token).await {
                Err(AroError::RetryableTransaction(_)) if retry < 2 => {
                    retry += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(10 * retry)).await;
                }
                result => return result,
            }
        }
    }

    async fn consume_refresh_token_once(
        &self,
        refresh_token: &str,
    ) -> AroResult<Option<RefreshTokenPrincipal>> {
        let hash = hash_secret(refresh_token);
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let family_id: Option<Uuid> =
            sqlx::query_scalar("SELECT family_id FROM refresh_tokens WHERE token_hash = $1")
                .bind(&hash)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        let Some(family_id) = family_id else {
            return Ok(None);
        };
        let family = lock_refresh_token_family(&mut tx, family_id).await?;
        let row = sqlx::query(
            r#"
            SELECT id, user_id, organization_id, device_id, family_id, revoked_at, rotated_at,
                   expires_at > now() AS is_unexpired
            FROM refresh_tokens
            WHERE token_hash = $1
            FOR UPDATE
            "#,
        )
        .bind(&hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let Some(row) = row else {
            return Ok(None);
        };
        let token_id: Uuid = row.get("id");
        let user_id: Uuid = row.get("user_id");
        let organization_id: Option<Uuid> = row.get("organization_id");
        let device_id: Option<Uuid> = row.get("device_id");
        let locked_family_id: Uuid = row.get("family_id");
        let revoked_at: Option<DateTime<Utc>> = row.get("revoked_at");
        let rotated_at: Option<DateTime<Utc>> = row.get("rotated_at");
        let is_unexpired: bool = row.get("is_unexpired");
        if locked_family_id != family_id {
            return Err(AroError::Security(
                "refresh token family changed during consumption".to_string(),
            ));
        }
        if family.user_id != user_id
            || family.device_id != device_id
            || family.revoked_at.is_some()
            || family.absolute_expires_at <= Utc::now()
        {
            return Ok(None);
        }
        if revoked_at.is_some() {
            if rotated_at.is_some() {
                revoke_refresh_token_family_for_reuse(&mut tx, family_id, token_id).await?;
                tx.commit().await.map_err(map_sqlx)?;
            }
            return Ok(None);
        }
        if !is_unexpired {
            return Ok(None);
        }
        sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE id = $1")
            .bind(token_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;

        Ok(Some(RefreshTokenPrincipal {
            user_id,
            organization_id,
        }))
    }

    /// Atomically rotates a refresh token and, when requested, changes its organization scope.
    /// The old token remains valid if any authorization check or successor insertion fails.
    pub async fn rotate_refresh_token(
        &self,
        refresh_token: &str,
        successor_token: &str,
        target_organization_id: Option<Uuid>,
        expected_user_id: Option<Uuid>,
        ttl_days: i64,
    ) -> AroResult<Option<RotatedRefreshToken>> {
        if !(1..=90).contains(&ttl_days) {
            return Err(AroError::Configuration(
                "refresh token lifetime must be between 1 and 90 days".to_string(),
            ));
        }
        let current_hash = hash_secret(refresh_token);
        let successor_hash = hash_secret(successor_token);
        if current_hash == successor_hash {
            return Err(AroError::Security(
                "refresh token rotation requires a new token".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let family_id: Option<Uuid> =
            sqlx::query_scalar("SELECT family_id FROM refresh_tokens WHERE token_hash = $1")
                .bind(&current_hash)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        let Some(family_id) = family_id else {
            return Ok(None);
        };
        let family = lock_refresh_token_family(&mut tx, family_id).await?;
        let row = sqlx::query(
            r#"
            SELECT id, user_id, organization_id, device_id, family_id,
                   revoked_at, rotated_at, expires_at > now() AS is_unexpired
            FROM refresh_tokens
            WHERE token_hash = $1
            FOR UPDATE
            "#,
        )
        .bind(current_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let token_id: Uuid = row.get("id");
        let user_id: Uuid = row.get("user_id");
        let current_organization_id: Uuid = row.get("organization_id");
        let device_id: Option<Uuid> = row.get("device_id");
        let locked_family_id: Uuid = row.get("family_id");
        let revoked_at: Option<DateTime<Utc>> = row.get("revoked_at");
        let rotated_at: Option<DateTime<Utc>> = row.get("rotated_at");
        let is_unexpired: bool = row.get("is_unexpired");
        if locked_family_id != family_id {
            return Err(AroError::Security(
                "refresh token family changed during rotation".to_string(),
            ));
        }
        if family.user_id != user_id
            || family.device_id != device_id
            || family.revoked_at.is_some()
            || family.absolute_expires_at <= Utc::now()
        {
            return Ok(None);
        }
        if revoked_at.is_some() {
            if rotated_at.is_some() {
                revoke_refresh_token_family_for_reuse(&mut tx, family_id, token_id).await?;
                tx.commit().await.map_err(map_sqlx)?;
            }
            return Ok(None);
        }
        if expected_user_id.is_some_and(|expected| expected != user_id) {
            return Err(AroError::Security(
                "refresh token does not belong to the authenticated user".to_string(),
            ));
        }
        if !is_unexpired {
            return Ok(None);
        }
        let organization_id = target_organization_id.unwrap_or(current_organization_id);
        let authorized: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1
              FROM memberships m
              JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL
              JOIN organizations o ON o.id = m.organization_id AND o.deleted_at IS NULL
              WHERE m.user_id = $1
                AND m.organization_id = $2
                AND m.status = 'active'
                AND m.deleted_at IS NULL
            )
            "#,
        )
        .bind(user_id)
        .bind(organization_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if !authorized {
            return Err(AroError::Security("organization access denied".to_string()));
        }

        let expires_at = (Utc::now() + Duration::days(ttl_days)).min(family.absolute_expires_at);
        if expires_at <= Utc::now() {
            return Ok(None);
        }
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens
              (user_id, organization_id, device_id, token_hash, expires_at, family_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(user_id)
        .bind(organization_id)
        .bind(device_id)
        .bind(successor_hash)
        .bind(expires_at)
        .bind(family_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = now(), rotated_at = now()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(token_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE refresh_token_families
            SET last_rotated_at = now(), updated_at = now()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(family_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(Some(RotatedRefreshToken {
            user_id,
            organization_id,
            expires_at,
        }))
    }

    pub async fn revoke_refresh_token(&self, refresh_token: &str) -> AroResult<()> {
        let token_hash = hash_secret(refresh_token);
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let family_id: Option<Uuid> =
            sqlx::query_scalar("SELECT family_id FROM refresh_tokens WHERE token_hash = $1")
                .bind(&token_hash)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_sqlx)?;
        let Some(family_id) = family_id else {
            return Ok(());
        };
        let _family = lock_refresh_token_family(&mut tx, family_id).await?;
        let still_in_family: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM refresh_tokens WHERE token_hash = $1 AND family_id = $2
            )
            "#,
        )
        .bind(token_hash)
        .bind(family_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if still_in_family {
            // Logout invalidates the complete device session. Serializing with rotation ensures a
            // concurrently-created successor cannot survive after logout has returned success.
            sqlx::query(
                r#"
                UPDATE refresh_token_families
                SET revoked_at = COALESCE(revoked_at, now()), updated_at = now()
                WHERE id = $1
                "#,
            )
            .bind(family_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            sqlx::query(
                r#"
                UPDATE refresh_tokens
                SET revoked_at = COALESCE(revoked_at, now())
                WHERE family_id = $1
                "#,
            )
            .bind(family_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    async fn emit_domain_event(
        &self,
        organization_id: Uuid,
        actor_user_id: Uuid,
        action: &str,
        target_type: &str,
        target_id: Option<Uuid>,
        payload: Value,
    ) -> AroResult<()> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        emit_domain_event_in_tx(
            &mut tx,
            organization_id,
            actor_user_id,
            action,
            target_type,
            target_id,
            payload,
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn count_audit_events_for_org(&self, organization_id: Uuid) -> AroResult<i64> {
        sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM audit_events
            WHERE organization_id = $1
            "#,
        )
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await
        .map(|row| row.get("count"))
        .map_err(map_sqlx)
    }

    pub async fn count_outbox_events_for_org(&self, organization_id: Uuid) -> AroResult<i64> {
        sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM outbox_events
            WHERE organization_id = $1
            "#,
        )
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await
        .map(|row| row.get("count"))
        .map_err(map_sqlx)
    }

    pub async fn list_pending_outbox_events(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        limit: i64,
    ) -> AroResult<Vec<OutboxEvent>> {
        self.ensure_org_admin(user_id, organization_id).await?;
        let limit = limit.clamp(1, 100);
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, event_type, payload, available_at, processed_at, created_at
            FROM outbox_events
            WHERE organization_id = $1
              AND processed_at IS NULL
              AND available_at <= now()
            ORDER BY available_at ASC, created_at ASC
            LIMIT $2
            "#,
        )
        .bind(organization_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows.into_iter().map(map_outbox_event_row).collect())
    }

    pub async fn mark_outbox_event_processed(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        event_id: Uuid,
    ) -> AroResult<Option<OutboxEvent>> {
        self.ensure_org_admin(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            UPDATE outbox_events
            SET processed_at = COALESCE(processed_at, now()),
                lease_token = NULL,
                lease_until = NULL
            WHERE id = $1 AND organization_id = $2
            RETURNING id, organization_id, event_type, payload, available_at, processed_at, created_at
            "#,
        )
        .bind(event_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(map_outbox_event_row))
    }

    pub async fn claim_pending_outbox_events_for_worker(
        &self,
        limit: i64,
        lease_token: Uuid,
        lease_seconds: i64,
    ) -> AroResult<Vec<ClaimedOutboxEvent>> {
        self.claim_pending_outbox_events_for_worker_scoped(limit, lease_token, lease_seconds, None)
            .await
    }

    /// Allows a worker shard to claim events for one organization without contending with every
    /// other tenant. The default worker uses the unscoped method above.
    pub async fn claim_pending_outbox_events_for_organization(
        &self,
        organization_id: Uuid,
        limit: i64,
        lease_token: Uuid,
        lease_seconds: i64,
    ) -> AroResult<Vec<ClaimedOutboxEvent>> {
        self.claim_pending_outbox_events_for_worker_scoped(
            limit,
            lease_token,
            lease_seconds,
            Some(organization_id),
        )
        .await
    }

    async fn claim_pending_outbox_events_for_worker_scoped(
        &self,
        limit: i64,
        lease_token: Uuid,
        lease_seconds: i64,
        organization_id: Option<Uuid>,
    ) -> AroResult<Vec<ClaimedOutboxEvent>> {
        let limit = limit.clamp(1, 500);
        let lease_seconds = lease_seconds.clamp(5, 300);
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let rows = sqlx::query(
            r#"
            WITH candidates AS (
              SELECT id
              FROM outbox_events
              WHERE processed_at IS NULL
                AND available_at <= now()
                AND (lease_until IS NULL OR lease_until <= now())
                AND ($4::uuid IS NULL OR organization_id = $4)
              ORDER BY available_at ASC, created_at ASC
              LIMIT $1
              FOR UPDATE SKIP LOCKED
            )
            UPDATE outbox_events event
            SET lease_token = $2,
                lease_until = now() + make_interval(secs => $3),
                attempts = event.attempts + 1,
                last_attempt_at = now(),
                last_error = NULL
            FROM candidates
            WHERE event.id = candidates.id
            RETURNING event.id, event.organization_id, event.event_type, event.payload,
                      event.available_at, event.processed_at, event.created_at
            "#,
        )
        .bind(limit)
        .bind(lease_token)
        .bind(lease_seconds)
        .bind(organization_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(rows
            .into_iter()
            .map(|row| ClaimedOutboxEvent {
                event: map_outbox_event_row(row),
                lease_token,
            })
            .collect())
    }

    pub async fn acknowledge_outbox_event_by_worker(
        &self,
        event_id: Uuid,
        lease_token: Uuid,
    ) -> AroResult<Option<OutboxEvent>> {
        let row = sqlx::query(
            r#"
            UPDATE outbox_events
            SET processed_at = now(),
                lease_token = NULL,
                lease_until = NULL,
                last_error = NULL
            WHERE id = $1
              AND lease_token = $2
              AND processed_at IS NULL
            RETURNING id, organization_id, event_type, payload, available_at, processed_at, created_at
            "#,
        )
        .bind(event_id)
        .bind(lease_token)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(map_outbox_event_row))
    }

    pub async fn release_outbox_event_for_retry(
        &self,
        event_id: Uuid,
        lease_token: Uuid,
        error: &str,
        retry_after_seconds: i64,
    ) -> AroResult<bool> {
        let retry_after_seconds = retry_after_seconds.clamp(1, 300);
        let safe_error = error.chars().take(500).collect::<String>();
        let result = sqlx::query(
            r#"
            UPDATE outbox_events
            SET lease_token = NULL,
                lease_until = NULL,
                available_at = now() + make_interval(secs => $3),
                last_error = $4
            WHERE id = $1
              AND lease_token = $2
              AND processed_at IS NULL
            "#,
        )
        .bind(event_id)
        .bind(lease_token)
        .bind(retry_after_seconds)
        .bind(safe_error)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn list_memberships_for_user(&self, user_id: Uuid) -> AroResult<Vec<Membership>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, organization_id, role, status, created_at, updated_at
            FROM memberships
            WHERE user_id = $1 AND deleted_at IS NULL
            ORDER BY created_at ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_membership_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn list_organization_members(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Vec<OrganizationMember>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT
              m.id,
              m.user_id,
              m.organization_id,
              u.name,
              u.email,
              m.role,
              m.status,
              m.created_at,
              m.updated_at
            FROM memberships m
            JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL
            WHERE m.organization_id = $1 AND m.deleted_at IS NULL
            ORDER BY
              CASE m.role
                WHEN 'owner' THEN 0
                WHEN 'admin' THEN 1
                WHEN 'manager' THEN 2
                WHEN 'member' THEN 3
                ELSE 4
              END,
              u.name ASC,
              u.email ASC
            "#,
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_organization_member_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn list_organization_invitations(
        &self,
        actor_user_id: Uuid,
        organization_id: Uuid,
        before: Option<(DateTime<Utc>, Uuid)>,
        include_closed: bool,
        limit: i64,
    ) -> AroResult<Vec<OrganizationInvitation>> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        lock_organization_admin(&mut tx, actor_user_id, organization_id).await?;
        let (before_created_at, before_id) = before
            .map(|(created_at, id)| (Some(created_at), Some(id)))
            .unwrap_or((None, None));
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, membership_id, invited_email, invited_name,
                   invited_role, delivery_status, delivery_attempts, expires_at, delivered_at,
                   accepted_at, revoked_at, created_at, updated_at
            FROM organization_invitations
            WHERE organization_id = $1
              AND (
                $2
                OR (
                  accepted_at IS NULL
                  AND revoked_at IS NULL
                  AND expires_at > now()
                )
              )
              AND (
                $3::timestamptz IS NULL
                OR (created_at, id) < ($3, $4::uuid)
              )
            ORDER BY created_at DESC, id DESC
            LIMIT $5
            "#,
        )
        .bind(organization_id)
        .bind(include_closed)
        .bind(before_created_at)
        .bind(before_id)
        .bind(limit.clamp(1, 201))
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_organization_invitation_row)
            .collect()
    }

    pub async fn revoke_organization_invitation(
        &self,
        actor_user_id: Uuid,
        organization_id: Uuid,
        invitation_id: Uuid,
    ) -> AroResult<()> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        lock_organization_admin(&mut tx, actor_user_id, organization_id).await?;
        let invitation = sqlx::query(
            r#"
            SELECT membership_id, invited_user_id
            FROM organization_invitations
            WHERE id = $1
              AND organization_id = $2
              AND accepted_at IS NULL
              AND revoked_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(invitation_id)
        .bind(organization_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Memory("invitation not found".to_string()))?;
        let membership_id: Option<Uuid> = invitation.get("membership_id");
        let invited_user_id: Option<Uuid> = invitation.get("invited_user_id");

        if let Some(membership_id) = membership_id {
            let membership = sqlx::query(
                r#"
                SELECT user_id, role, status
                FROM memberships
                WHERE id = $1 AND organization_id = $2
                FOR UPDATE
                "#,
            )
            .bind(membership_id)
            .bind(organization_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?
            .ok_or_else(|| {
                AroError::Security("invitation membership is inconsistent".to_string())
            })?;
            let membership_user_id: Uuid = membership.get("user_id");
            let membership_role: String = membership.get("role");
            let membership_status: String = membership.get("status");
            if invited_user_id != Some(membership_user_id)
                || membership_role == "owner"
                || membership_status != "invited"
            {
                return Err(AroError::Security(
                    "invitation membership is protected or inconsistent".to_string(),
                ));
            }
            sqlx::query(
                r#"
                UPDATE memberships
                SET deleted_at = now(), updated_at = now()
                WHERE id = $1 AND organization_id = $2 AND status = 'invited'
                "#,
            )
            .bind(membership_id)
            .bind(organization_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        }

        let revoked = sqlx::query(
            r#"
            UPDATE organization_invitations
            SET revoked_at = now(),
                delivery_status = 'failed',
                delivery_token_encrypted = NULL,
                delivery_lease_token = NULL,
                delivery_lease_until = NULL,
                delivery_last_error = 'invitation_revoked',
                updated_at = now()
            WHERE id = $1 AND organization_id = $2
              AND accepted_at IS NULL AND revoked_at IS NULL
            "#,
        )
        .bind(invitation_id)
        .bind(organization_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if revoked.rows_affected() != 1 {
            return Err(AroError::Memory("invitation not found".to_string()));
        }
        emit_domain_event_in_tx(
            &mut tx,
            organization_id,
            actor_user_id,
            "membership.invitation.revoked",
            "organization_invitation",
            Some(invitation_id),
            serde_json::json!({ "invitationId": invitation_id }),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn claim_invitation_deliveries(
        &self,
        limit: i64,
        lease_token: Uuid,
        lease_seconds: i64,
        max_attempts: i32,
        secrets_key: &str,
    ) -> AroResult<Vec<ClaimedInvitationDelivery>> {
        if secrets_key.trim().len() < 32 {
            return Err(AroError::Configuration(
                "invitation delivery encryption key is invalid".to_string(),
            ));
        }
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let rows = sqlx::query(
            r#"
            WITH candidates AS (
              SELECT invitation.id
              FROM organization_invitations invitation
              WHERE invitation.accepted_at IS NULL
                AND invitation.revoked_at IS NULL
                AND invitation.expires_at > now()
                AND invitation.delivery_token_encrypted IS NOT NULL
                AND invitation.delivery_attempts < $4
                AND (
                  (invitation.delivery_status = 'pending'
                    AND invitation.delivery_available_at <= now())
                  OR (invitation.delivery_status = 'processing'
                    AND invitation.delivery_lease_until <= now())
                )
              ORDER BY invitation.delivery_available_at ASC,
                       invitation.created_at ASC,
                       invitation.id ASC
              LIMIT $1
              FOR UPDATE SKIP LOCKED
            )
            UPDATE organization_invitations invitation
            SET delivery_status = 'processing',
                delivery_lease_token = $2,
                delivery_lease_until = now() + make_interval(secs => $3),
                delivery_last_attempt_at = now(),
                delivery_attempts = invitation.delivery_attempts + 1,
                delivery_last_error = NULL,
                updated_at = now()
            FROM candidates, organizations organization
            WHERE invitation.id = candidates.id
              AND organization.id = invitation.organization_id
              AND organization.deleted_at IS NULL
            RETURNING invitation.id, invitation.organization_id,
                      organization.name AS organization_name,
                      invitation.invited_email, invitation.invited_name,
                      invitation.invited_role, invitation.expires_at,
                      invitation.delivery_attempts,
                      pgp_sym_decrypt(invitation.delivery_token_encrypted, $5)::text
                        AS delivery_token
            "#,
        )
        .bind(limit.clamp(1, 100))
        .bind(lease_token)
        .bind(lease_seconds.clamp(15, 600))
        .bind(max_attempts.clamp(1, 20))
        .bind(secrets_key)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        rows.into_iter()
            .map(|row| {
                Ok(ClaimedInvitationDelivery {
                    id: row.get("id"),
                    organization_id: row.get("organization_id"),
                    organization_name: row.get("organization_name"),
                    email: row.get("invited_email"),
                    name: row.get("invited_name"),
                    role: enum_from_string(row.get::<String, _>("invited_role"))?,
                    token: row.get("delivery_token"),
                    expires_at: row.get("expires_at"),
                    attempts: row.get("delivery_attempts"),
                    lease_token,
                })
            })
            .collect()
    }

    pub async fn acknowledge_invitation_delivery(
        &self,
        invitation_id: Uuid,
        lease_token: Uuid,
    ) -> AroResult<bool> {
        let result = sqlx::query(
            r#"
            UPDATE organization_invitations
            SET delivery_status = 'delivered',
                delivered_at = COALESCE(delivered_at, now()),
                delivery_token_encrypted = NULL,
                delivery_lease_token = NULL,
                delivery_lease_until = NULL,
                delivery_last_error = NULL,
                updated_at = now()
            WHERE id = $1
              AND delivery_status = 'processing'
              AND delivery_lease_token = $2
              AND delivery_lease_until > now()
              AND accepted_at IS NULL
              AND revoked_at IS NULL
            "#,
        )
        .bind(invitation_id)
        .bind(lease_token)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn release_invitation_delivery_for_retry(
        &self,
        invitation_id: Uuid,
        lease_token: Uuid,
        max_attempts: i32,
        retry_after_seconds: i64,
        safe_error_code: &str,
    ) -> AroResult<bool> {
        let safe_error_code = safe_error_code.chars().take(500).collect::<String>();
        let result = sqlx::query(
            r#"
            UPDATE organization_invitations
            SET delivery_status = CASE
                  WHEN delivery_attempts >= $3 THEN 'failed'
                  ELSE 'pending'
                END,
                delivery_available_at = now() + make_interval(secs => $4),
                delivery_lease_token = NULL,
                delivery_lease_until = NULL,
                delivery_last_error = $5,
                delivery_token_encrypted = CASE
                  WHEN delivery_attempts >= $3 THEN NULL
                  ELSE delivery_token_encrypted
                END,
                updated_at = now()
            WHERE id = $1
              AND delivery_status = 'processing'
              AND delivery_lease_token = $2
              AND delivery_lease_until > now()
              AND accepted_at IS NULL
              AND revoked_at IS NULL
            "#,
        )
        .bind(invitation_id)
        .bind(lease_token)
        .bind(max_attempts.clamp(1, 20))
        .bind(retry_after_seconds.clamp(1, 3_600))
        .bind(safe_error_code)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn fail_expired_invitation_deliveries(&self, max_attempts: i32) -> AroResult<u64> {
        let result = sqlx::query(
            r#"
            UPDATE organization_invitations
            SET delivery_status = 'failed',
                delivery_token_encrypted = NULL,
                delivery_lease_token = NULL,
                delivery_lease_until = NULL,
                delivery_last_error = CASE
                  WHEN expires_at <= now() THEN 'invitation_expired'
                  ELSE 'delivery_attempts_exhausted'
                END,
                updated_at = now()
            WHERE accepted_at IS NULL
              AND revoked_at IS NULL
              AND delivery_status IN ('pending', 'processing')
              AND (delivery_lease_until IS NULL OR delivery_lease_until <= now())
              AND (expires_at <= now() OR delivery_attempts >= $1)
            "#,
        )
        .bind(max_attempts.clamp(1, 20))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected())
    }

    pub async fn issue_organization_invitation(
        &self,
        actor_user_id: Uuid,
        organization_id: Uuid,
        request: NewOrganizationMember,
        token: String,
        expires_at: DateTime<Utc>,
        secrets_key: &str,
    ) -> AroResult<IssuedOrganizationInvitation> {
        if request.role == MembershipRole::Owner {
            return Err(AroError::Security(
                "owner role cannot be granted through invite".to_string(),
            ));
        }
        let email = request.email.trim().to_lowercase();
        let name = non_empty_or_none(Some(request.name))
            .unwrap_or_else(|| email.split('@').next().unwrap_or("ARO User").to_string());
        if email.is_empty() || !email.contains('@') {
            return Err(AroError::Configuration(
                "valid member email is required".to_string(),
            ));
        }
        if token.len() < 32 || expires_at <= Utc::now() {
            return Err(AroError::Configuration(
                "invitation token or expiry is invalid".to_string(),
            ));
        }
        if secrets_key.trim().len() < 32 {
            return Err(AroError::Configuration(
                "invitation delivery encryption key is invalid".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        // Lock the authorization row so a concurrent demotion/removal cannot race issuance.
        let actor_membership = sqlx::query(
            r#"
            SELECT m.id
            FROM memberships m
            JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL
            JOIN organizations o ON o.id = m.organization_id AND o.deleted_at IS NULL
            WHERE m.user_id = $1
              AND m.organization_id = $2
              AND m.status = 'active'
              AND m.role IN ('owner', 'admin')
              AND m.deleted_at IS NULL
            FOR SHARE OF m
            "#,
        )
        .bind(actor_user_id)
        .bind(organization_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if actor_membership.is_none() {
            return Err(AroError::Security("admin role required".to_string()));
        }
        let existing_user_id = sqlx::query(
            r#"
            SELECT id
            FROM users
            WHERE email = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(&email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .map(|row| row.get::<Uuid, _>("id"));

        let role = enum_to_string(&request.role)?;
        sqlx::query(
            r#"
            SELECT id
            FROM organization_invitations
            WHERE organization_id = $1
              AND invited_email = $2
              AND accepted_at IS NULL
              AND revoked_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(organization_id)
        .bind(&email)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let membership_id = if let Some(user_id) = existing_user_id {
            if let Some(row) = sqlx::query(
                r#"
                SELECT role, status, deleted_at
                FROM memberships
                WHERE user_id = $1 AND organization_id = $2
                FOR UPDATE
                "#,
            )
            .bind(user_id)
            .bind(organization_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?
            {
                let existing_role: String = row.get("role");
                let existing_status: String = row.get("status");
                let deleted_at: Option<DateTime<Utc>> = row.get("deleted_at");
                if existing_role == "owner" {
                    return Err(AroError::Security(
                        "owner membership cannot be changed through invite".to_string(),
                    ));
                }
                if deleted_at.is_none() && existing_status != "invited" {
                    return Err(AroError::Security(
                        "member already belongs to this organization".to_string(),
                    ));
                }
            }
            let status = enum_to_string(&MembershipStatus::Invited)?;
            let row = sqlx::query(
                r#"
                INSERT INTO memberships (user_id, organization_id, role, status)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (user_id, organization_id)
                DO UPDATE SET
                  role = excluded.role,
                  status = excluded.status,
                  deleted_at = NULL,
                  updated_at = now()
                RETURNING id
                "#,
            )
            .bind(user_id)
            .bind(organization_id)
            .bind(&role)
            .bind(status)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            Some(row.get::<Uuid, _>("id"))
        } else {
            None
        };

        // Reissuing an invitation invalidates the previous bearer credential first.
        sqlx::query(
            r#"
            UPDATE organization_invitations
            SET revoked_at = now(),
                delivery_status = 'failed',
                delivery_token_encrypted = NULL,
                delivery_lease_token = NULL,
                delivery_lease_until = NULL,
                delivery_last_error = 'invitation_superseded',
                updated_at = now()
            WHERE organization_id = $1
              AND invited_email = $2
              AND accepted_at IS NULL
              AND revoked_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(&email)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let invitation_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO organization_invitations
              (organization_id, membership_id, invited_user_id, invited_by_user_id, token_hash,
               invited_email, invited_name, invited_role, account_existed_at_issue,
               delivery_token_encrypted, delivery_status, expires_at, accepted_at, revoked_at)
            VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9,
              pgp_sym_encrypt($10::text, $11), 'pending', $12, NULL, NULL
            )
            RETURNING id
            "#,
        )
        .bind(organization_id)
        .bind(membership_id)
        .bind(existing_user_id)
        .bind(actor_user_id)
        .bind(hash_secret(&token))
        .bind(&email)
        .bind(&name)
        .bind(&role)
        .bind(existing_user_id.is_some())
        .bind(&token)
        .bind(secrets_key)
        .bind(expires_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        emit_domain_event_in_tx(
            &mut tx,
            organization_id,
            actor_user_id,
            "membership.invitation.issued",
            "organization_invitation",
            Some(invitation_id),
            serde_json::json!({
                "invitationId": invitation_id,
                "email": email,
                "expiresAt": expires_at,
                "deliveryStatus": "pending"
            }),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;

        let member = match membership_id {
            Some(membership_id) => {
                self.get_organization_member(actor_user_id, organization_id, membership_id)
                    .await?
            }
            None => None,
        };
        Ok(IssuedOrganizationInvitation {
            id: invitation_id,
            organization_id,
            email,
            name,
            role: request.role,
            member,
            expires_at,
        })
    }

    pub async fn invite_organization_member(
        &self,
        actor_user_id: Uuid,
        organization_id: Uuid,
        request: NewOrganizationMember,
        secrets_key: &str,
    ) -> AroResult<IssuedOrganizationInvitation> {
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        self.issue_organization_invitation(
            actor_user_id,
            organization_id,
            request,
            token,
            Utc::now() + Duration::days(7),
            secrets_key,
        )
        .await
    }

    /// Creates an account only after the recipient presents the bearer token delivered to the
    /// invited mailbox. Issuing an invitation never reserves a global e-mail identity.
    pub async fn accept_new_organization_invitation(
        &self,
        token: &str,
        password_hash: String,
    ) -> AroResult<(Uuid, Uuid)> {
        self.accept_new_organization_invitation_inner(token, None, password_hash)
            .await
    }

    pub async fn accept_new_organization_invitation_for_email(
        &self,
        token: &str,
        expected_email: &str,
        password_hash: String,
    ) -> AroResult<(Uuid, Uuid)> {
        let expected_email = expected_email.trim().to_lowercase();
        self.accept_new_organization_invitation_inner(
            token,
            Some(expected_email.as_str()),
            password_hash,
        )
        .await
    }

    async fn accept_new_organization_invitation_inner(
        &self,
        token: &str,
        expected_email: Option<&str>,
        password_hash: String,
    ) -> AroResult<(Uuid, Uuid)> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let invitation = sqlx::query(
            r#"
            SELECT i.id, i.organization_id, i.invited_email, i.invited_name, i.invited_role
            FROM organization_invitations i
            WHERE i.token_hash = $1
              AND ($2::text IS NULL OR i.invited_email = $2)
              AND i.account_existed_at_issue = false
              AND i.invited_user_id IS NULL
              AND i.membership_id IS NULL
              AND i.accepted_at IS NULL
              AND i.revoked_at IS NULL
              AND i.expires_at > now()
            FOR UPDATE OF i
            "#,
        )
        .bind(hash_secret(token))
        .bind(expected_email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(invitation) = invitation else {
            return Err(AroError::Security(
                "invitation is invalid, expired, or already accepted".to_string(),
            ));
        };
        let invitation_id: Uuid = invitation.get("id");
        let organization_id: Uuid = invitation.get("organization_id");
        let email: String = invitation.get("invited_email");
        let name: String = invitation.get("invited_name");
        let role: String = invitation.get("invited_role");
        let user_id = sqlx::query_scalar(
            r#"
            INSERT INTO users (email, name, password_hash, email_verified_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (email) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(&email)
        .bind(&name)
        .bind(password_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| {
            AroError::Security(
                "an account now exists for this email; sign in to accept the invitation"
                    .to_string(),
            )
        })?;
        upsert_default_preferences(&mut tx, user_id).await?;
        let active = enum_to_string(&MembershipStatus::Active)?;
        let membership_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO memberships (user_id, organization_id, role, status)
            VALUES ($1, $2, $3, $4)
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(organization_id)
        .bind(role)
        .bind(active)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE organization_invitations
            SET invited_user_id = $2,
                membership_id = $3,
                accepted_at = now(),
                delivery_status = 'delivered',
                delivered_at = COALESCE(delivered_at, now()),
                delivery_token_encrypted = NULL,
                delivery_lease_token = NULL,
                delivery_lease_until = NULL,
                delivery_last_error = NULL,
                updated_at = now()
            WHERE id = $1 AND accepted_at IS NULL AND revoked_at IS NULL
            "#,
        )
        .bind(invitation_id)
        .bind(user_id)
        .bind(membership_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        upsert_default_settings(&mut tx, organization_id, user_id).await?;
        emit_domain_event_in_tx(
            &mut tx,
            organization_id,
            user_id,
            "membership.invitation.accepted",
            "membership",
            Some(membership_id),
            serde_json::json!({ "membershipId": membership_id }),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok((user_id, organization_id))
    }

    /// Existing accounts must authenticate as the invited user before activating their
    /// membership. The invite token alone is deliberately insufficient to create a session.
    pub async fn accept_existing_organization_invitation(
        &self,
        actor_user_id: Uuid,
        token: &str,
        refresh_token: &str,
        successor_token: &str,
        ttl_days: i64,
    ) -> AroResult<RotatedRefreshToken> {
        if !(1..=90).contains(&ttl_days) {
            return Err(AroError::Configuration(
                "refresh token lifetime must be between 1 and 90 days".to_string(),
            ));
        }
        let current_refresh_hash = hash_secret(refresh_token);
        let successor_hash = hash_secret(successor_token);
        if current_refresh_hash == successor_hash {
            return Err(AroError::Security(
                "refresh token rotation requires a new token".to_string(),
            ));
        }
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let invitation =
            lock_existing_organization_invitation(&mut tx, actor_user_id, token).await?;
        let organization_id = invitation.organization_id;
        let family_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT family_id
            FROM refresh_tokens
            WHERE token_hash = $1
            "#,
        )
        .bind(&current_refresh_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(family_id) = family_id else {
            return Err(AroError::Security(
                "valid refresh token required".to_string(),
            ));
        };
        let family = lock_refresh_token_family(&mut tx, family_id).await?;
        let refresh = sqlx::query(
            r#"
            SELECT id, user_id, device_id, family_id, revoked_at, rotated_at,
                   expires_at > now() AS is_unexpired
            FROM refresh_tokens
            WHERE token_hash = $1
            FOR UPDATE
            "#,
        )
        .bind(current_refresh_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(refresh) = refresh else {
            return Err(AroError::Security(
                "valid refresh token required".to_string(),
            ));
        };
        let refresh_token_id: Uuid = refresh.get("id");
        let refresh_user_id: Uuid = refresh.get("user_id");
        let device_id: Option<Uuid> = refresh.get("device_id");
        let locked_family_id: Uuid = refresh.get("family_id");
        let revoked_at: Option<DateTime<Utc>> = refresh.get("revoked_at");
        let rotated_at: Option<DateTime<Utc>> = refresh.get("rotated_at");
        let is_unexpired: bool = refresh.get("is_unexpired");
        if locked_family_id != family_id {
            return Err(AroError::Security(
                "valid refresh token required".to_string(),
            ));
        }
        if family.user_id != refresh_user_id
            || family.device_id != device_id
            || family.revoked_at.is_some()
            || family.absolute_expires_at <= Utc::now()
        {
            return Err(AroError::Security(
                "valid refresh token required".to_string(),
            ));
        }
        if revoked_at.is_some() {
            if rotated_at.is_some() {
                revoke_refresh_token_family_for_reuse(&mut tx, family_id, refresh_token_id).await?;
                tx.commit().await.map_err(map_sqlx)?;
            }
            return Err(AroError::Security(
                "valid refresh token required".to_string(),
            ));
        }
        if refresh_user_id != actor_user_id {
            return Err(AroError::Security(
                "valid refresh token required".to_string(),
            ));
        }
        if !is_unexpired {
            return Err(AroError::Security(
                "valid refresh token required".to_string(),
            ));
        }
        activate_locked_organization_invitation(&mut tx, actor_user_id, &invitation).await?;
        let expires_at = (Utc::now() + Duration::days(ttl_days)).min(family.absolute_expires_at);
        if expires_at <= Utc::now() {
            return Err(AroError::Security(
                "valid refresh token required".to_string(),
            ));
        }
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens
              (user_id, organization_id, device_id, token_hash, expires_at, family_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(actor_user_id)
        .bind(organization_id)
        .bind(device_id)
        .bind(successor_hash)
        .bind(expires_at)
        .bind(family_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = now(), rotated_at = now()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(refresh_token_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE refresh_token_families
            SET last_rotated_at = now(), updated_at = now()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(family_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(RotatedRefreshToken {
            user_id: actor_user_id,
            organization_id,
            expires_at,
        })
    }

    /// Accepts an existing-account invitation after the API has verified that account's primary
    /// credential. This recovery path is required when the account has no active organization and
    /// therefore cannot possess an authenticated tenant-scoped session yet.
    pub async fn accept_existing_invitation_after_primary_auth(
        &self,
        actor_user_id: Uuid,
        token: &str,
    ) -> AroResult<Uuid> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let invitation =
            lock_existing_organization_invitation(&mut tx, actor_user_id, token).await?;
        activate_locked_organization_invitation(&mut tx, actor_user_id, &invitation).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(invitation.organization_id)
    }

    pub async fn update_organization_member_role(
        &self,
        actor_user_id: Uuid,
        organization_id: Uuid,
        membership_id: Uuid,
        role: MembershipRole,
    ) -> AroResult<OrganizationMember> {
        self.ensure_org_admin(actor_user_id, organization_id)
            .await?;
        if role == MembershipRole::Owner {
            return Err(AroError::Security(
                "owner role cannot be assigned from member settings".to_string(),
            ));
        }
        let role = enum_to_string(&role)?;
        sqlx::query(
            r#"
            UPDATE memberships
            SET role = $3, updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND role <> 'owner'
              AND deleted_at IS NULL
            "#,
        )
        .bind(membership_id)
        .bind(organization_id)
        .bind(role)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        self.get_organization_member(actor_user_id, organization_id, membership_id)
            .await?
            .ok_or_else(|| AroError::Security("member not found or protected".to_string()))
    }

    pub async fn remove_organization_member(
        &self,
        actor_user_id: Uuid,
        organization_id: Uuid,
        membership_id: Uuid,
    ) -> AroResult<()> {
        self.ensure_org_admin(actor_user_id, organization_id)
            .await?;
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        // Invitation rows are always locked before membership rows. Acceptance uses the same
        // order, preventing a remove/accept deadlock and ensuring only one transition wins.
        sqlx::query(
            r#"
            SELECT id
            FROM organization_invitations
            WHERE membership_id = $1
              AND organization_id = $2
              AND accepted_at IS NULL
              AND revoked_at IS NULL
            FOR UPDATE
            "#,
        )
        .bind(membership_id)
        .bind(organization_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let result = sqlx::query(
            r#"
            UPDATE memberships
            SET deleted_at = now(), updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND user_id <> $3
              AND role <> 'owner'
              AND deleted_at IS NULL
            "#,
        )
        .bind(membership_id)
        .bind(organization_id)
        .bind(actor_user_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(AroError::Security(
                "member not found or protected".to_string(),
            ));
        }
        sqlx::query(
            r#"
            UPDATE organization_invitations
            SET revoked_at = now(),
                delivery_status = 'failed',
                delivery_token_encrypted = NULL,
                delivery_lease_token = NULL,
                delivery_lease_until = NULL,
                delivery_last_error = 'membership_removed',
                updated_at = now()
            WHERE membership_id = $1 AND accepted_at IS NULL AND revoked_at IS NULL
            "#,
        )
        .bind(membership_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    async fn get_organization_member(
        &self,
        actor_user_id: Uuid,
        organization_id: Uuid,
        membership_id: Uuid,
    ) -> AroResult<Option<OrganizationMember>> {
        self.ensure_org_access(actor_user_id, organization_id)
            .await?;
        sqlx::query(
            r#"
            SELECT
              m.id,
              m.user_id,
              m.organization_id,
              u.name,
              u.email,
              m.role,
              m.status,
              m.created_at,
              m.updated_at
            FROM memberships m
            JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL
            WHERE m.id = $1
              AND m.organization_id = $2
              AND m.deleted_at IS NULL
            "#,
        )
        .bind(membership_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .map(map_organization_member_row)
        .transpose()
    }

    pub async fn get_organization(&self, organization_id: Uuid) -> AroResult<Option<Organization>> {
        sqlx::query(
            r#"
            SELECT id, name, domain, description, created_at, updated_at
            FROM organizations
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(map_organization_row))
        .map_err(map_sqlx)
    }

    pub async fn update_user_profile(
        &self,
        user_id: Uuid,
        patch: UserProfilePatch,
    ) -> AroResult<User> {
        let existing = sqlx::query(
            r#"
            SELECT id, email, name, role_title, avatar_color, created_at, updated_at
            FROM users
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .map(map_user_row)
        .ok_or_else(|| AroError::Security("user not found".to_string()))?;

        let row = sqlx::query(
            r#"
            UPDATE users
            SET
              name = COALESCE($2, name),
              role_title = COALESCE($3, role_title),
              avatar_color = COALESCE($4, avatar_color),
              updated_at = now()
            WHERE id = $1 AND deleted_at IS NULL
            RETURNING id, email, name, role_title, avatar_color, created_at, updated_at
            "#,
        )
        .bind(existing.id)
        .bind(non_empty_or_none(patch.name))
        .bind(patch.role_title)
        .bind(patch.avatar_color)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(map_user_row(row))
    }

    pub async fn update_organization(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        patch: OrganizationPatch,
    ) -> AroResult<Organization> {
        self.ensure_org_admin(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            UPDATE organizations
            SET
              name = COALESCE($2, name),
              domain = $3,
              description = $4,
              updated_at = now()
            WHERE id = $1 AND deleted_at IS NULL
            RETURNING id, name, domain, description, created_at, updated_at
            "#,
        )
        .bind(organization_id)
        .bind(non_empty_or_none(patch.name))
        .bind(patch.domain)
        .bind(patch.description)
        .fetch_one(&self.pool)
        .await
        .map(map_organization_row)
        .map_err(map_sqlx)?;
        Ok(row)
    }

    pub async fn create_organization_for_user(
        &self,
        user_id: Uuid,
        name: String,
        domain: Option<String>,
        description: Option<String>,
    ) -> AroResult<(Organization, Membership)> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let organization = sqlx::query(
            r#"
            INSERT INTO organizations (name, domain, description)
            VALUES ($1, $2, $3)
            RETURNING id, name, domain, description, created_at, updated_at
            "#,
        )
        .bind(name)
        .bind(domain)
        .bind(description)
        .fetch_one(&mut *tx)
        .await
        .map(map_organization_row)
        .map_err(map_sqlx)?;
        let membership = create_membership(
            &mut tx,
            user_id,
            organization.id,
            MembershipRole::Owner,
            MembershipStatus::Active,
        )
        .await?;
        upsert_default_settings(&mut tx, organization.id, user_id).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok((organization, membership))
    }

    pub async fn ensure_org_access(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Membership> {
        let row = sqlx::query(
            r#"
            SELECT m.id, m.user_id, m.organization_id, m.role, m.status, m.created_at, m.updated_at
            FROM memberships m
            JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL
            JOIN organizations o ON o.id = m.organization_id AND o.deleted_at IS NULL
            WHERE m.user_id = $1
              AND m.organization_id = $2
              AND m.status = 'active'
              AND m.deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        row.map(map_membership_row)
            .transpose()?
            .ok_or_else(|| AroError::Security("organization access denied".to_string()))
    }

    pub async fn ensure_org_admin(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Membership> {
        let membership = self.ensure_org_access(user_id, organization_id).await?;
        if matches!(
            membership.role,
            MembershipRole::Owner | MembershipRole::Admin
        ) {
            Ok(membership)
        } else {
            Err(AroError::Security(
                "organization admin permission required".to_string(),
            ))
        }
    }

    pub async fn ensure_org_manager(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Membership> {
        let membership = self.ensure_org_access(user_id, organization_id).await?;
        if matches!(
            membership.role,
            MembershipRole::Owner | MembershipRole::Admin | MembershipRole::Manager
        ) {
            Ok(membership)
        } else {
            Err(AroError::Security(
                "organization manager permission required".to_string(),
            ))
        }
    }

    /// Conversations are private to their creator until an explicit share/ACL
    /// model exists. Organization membership alone is not sufficient access.
    async fn ensure_conversation_owner(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        conversation_id: Uuid,
    ) -> AroResult<()> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1
              FROM conversations
              WHERE id = $1
                AND organization_id = $2
                AND owner_user_id = $3
                AND deleted_at IS NULL
            )
            "#,
        )
        .bind(conversation_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        if exists {
            Ok(())
        } else {
            Err(AroError::Security("conversation access denied".to_string()))
        }
    }

    async fn ensure_agent_run_owner(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        run_id: Uuid,
    ) -> AroResult<()> {
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
              SELECT 1
              FROM agent_runs
              WHERE id = $1
                AND organization_id = $2
                AND owner_user_id = $3
                AND deleted_at IS NULL
            )
            "#,
        )
        .bind(run_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        if exists {
            Ok(())
        } else {
            Err(AroError::Security("agent run access denied".to_string()))
        }
    }

    pub async fn list_api_keys(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Vec<PublicApiKey>> {
        self.ensure_org_admin(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, name, prefix, created_at, last_used_at
            FROM api_keys
            WHERE organization_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(map_public_api_key_row).collect()
    }

    pub async fn create_api_key(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        name: String,
        prefix: String,
        secret_hash: String,
    ) -> AroResult<PublicApiKey> {
        self.ensure_org_admin(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            INSERT INTO api_keys (organization_id, name, prefix, secret_hash)
            VALUES ($1, $2, $3, $4)
            RETURNING id, organization_id, name, prefix, created_at, last_used_at
            "#,
        )
        .bind(organization_id)
        .bind(name)
        .bind(prefix)
        .bind(secret_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        let key = map_public_api_key_row(row)?;
        self.emit_domain_event(
            organization_id,
            user_id,
            "api_key.created",
            "api_key",
            Some(key.id),
            serde_json::to_value(&key).map_err(|err| AroError::Memory(err.to_string()))?,
        )
        .await?;
        Ok(key)
    }

    pub async fn revoke_api_key(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        api_key_id: Uuid,
    ) -> AroResult<()> {
        self.ensure_org_admin(user_id, organization_id).await?;
        sqlx::query(
            r#"
            UPDATE api_keys
            SET deleted_at = now()
            WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(api_key_id)
        .bind(organization_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        self.emit_domain_event(
            organization_id,
            user_id,
            "api_key.revoked",
            "api_key",
            Some(api_key_id),
            serde_json::json!({ "id": api_key_id }),
        )
        .await?;
        Ok(())
    }

    pub async fn list_integration_providers(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Value> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            WITH latest AS (
              SELECT DISTINCT ON (provider_id)
                provider_id, manifest_version, kind, display_name, description, category, icon,
                auth, permissions, capabilities
              FROM integration_providers
              WHERE deleted_at IS NULL AND enabled = true
              ORDER BY provider_id, CASE WHEN source = 'seed' THEN 0 ELSE 1 END, manifest_version DESC
            )
            SELECT COALESCE(jsonb_agg(
              jsonb_build_object(
                'id', provider_id,
                'version', manifest_version,
                'kind', kind,
                'displayName', display_name,
                'description', description,
                'category', category,
                'icon', icon,
                'auth', auth,
                'permissions', permissions,
                'capabilities', capabilities
              )
              ORDER BY category, display_name
            ), '[]'::jsonb) AS data
            FROM latest
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    // Retained only as unreachable source context for the legacy contract period. Dynamic global
    // provider creation is deliberately excluded from every build; v3 manifests are migration-
    // managed and instance-admin controlled.
    #[cfg(any())]
    pub async fn get_or_create_integration_provider(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        provider_id: &str,
    ) -> AroResult<Value> {
        self.ensure_org_access(user_id, organization_id).await?;
        if let Some(provider) = self
            .get_integration_provider(user_id, organization_id, provider_id)
            .await?
        {
            return Ok(provider);
        }
        let display_name = provider_id
            .split('-')
            .map(|s| {
                let mut chars = s.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let auth = serde_json::json!({
            "mode": "oauth2",
            "credentialStore": "server-vault",
            "oauth": {
                "authorizationUrl": "https://example.com/oauth/authorize",
                "tokenUrl": "https://example.com/oauth/token",
                "userinfoUrl": "https://example.com/oauth/userinfo",
                "revocationUrl": null,
                "redirectPath": format!("/integrations/{}/oauth/callback", provider_id),
                "defaultScopes": [],
                "pkceRequired": false,
                "oidc": false
            },
            "configFields": []
        });

        sqlx::query(
            r#"
            INSERT INTO integration_providers (
                provider_id, manifest_version, kind, display_name, description, category, icon,
                auth, permissions, capabilities, source, enabled
            ) VALUES (
                $1, 'legacy', 'app', $2, $3, 'Other', $4,
                $5, '[]'::jsonb, '[]'::jsonb, 'seed', true
            )
            ON CONFLICT (provider_id, manifest_version) DO NOTHING
            "#,
        )
        .bind(provider_id)
        .bind(&display_name)
        .bind(format!("Connexion à {display_name} via redirection OAuth"))
        .bind(provider_id)
        .bind(auth)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;

        let provider = self
            .get_integration_provider(user_id, organization_id, provider_id)
            .await?
            .ok_or_else(|| {
                AroError::Unexpected(format!("Failed to retrieve created provider {provider_id}"))
            })?;
        Ok(provider)
    }

    pub async fn get_integration_provider(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        provider_id: &str,
    ) -> AroResult<Option<Value>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT jsonb_build_object(
              'id', provider_id,
              'version', manifest_version,
              'kind', kind,
              'displayName', display_name,
              'description', description,
              'category', category,
              'icon', icon,
              'auth', auth,
              'permissions', permissions,
              'capabilities', capabilities
            ) AS data
            FROM integration_providers
            WHERE provider_id = $1 AND deleted_at IS NULL AND enabled = true
            ORDER BY CASE WHEN source = 'seed' THEN 0 ELSE 1 END, manifest_version DESC
            LIMIT 1
            "#,
        )
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(|row| row.get("data")))
    }

    pub async fn list_integrations(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Value> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT COALESCE(jsonb_agg(integration_connection_public_json(c, a, cred) ORDER BY c.updated_at DESC), '[]'::jsonb) AS data
            FROM integration_connections c
            LEFT JOIN integration_accounts a
              ON a.organization_id = c.organization_id
              AND a.id = c.account_id
              AND a.deleted_at IS NULL
            LEFT JOIN LATERAL (
              SELECT *
              FROM integration_credentials candidate
              WHERE candidate.organization_id = c.organization_id
                AND candidate.connection_id = c.id
                AND candidate.deleted_at IS NULL
                AND candidate.revoked_at IS NULL
              ORDER BY candidate.created_at DESC
              LIMIT 1
            ) cred ON true
            WHERE c.organization_id = $1 AND c.deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    pub async fn get_integration(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        connection_id: Uuid,
    ) -> AroResult<Option<Value>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT integration_connection_public_json(c, a, cred) AS data
            FROM integration_connections c
            LEFT JOIN integration_accounts a
              ON a.organization_id = c.organization_id
              AND a.id = c.account_id
              AND a.deleted_at IS NULL
            LEFT JOIN LATERAL (
              SELECT *
              FROM integration_credentials candidate
              WHERE candidate.organization_id = c.organization_id
                AND candidate.connection_id = c.id
                AND candidate.deleted_at IS NULL
                AND candidate.revoked_at IS NULL
              ORDER BY candidate.created_at DESC
              LIMIT 1
            ) cred ON true
            WHERE c.id = $1 AND c.organization_id = $2 AND c.deleted_at IS NULL
            "#,
        )
        .bind(connection_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(|row| row.get("data")))
    }

    pub async fn create_api_key_integration(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        request: NewApiKeyIntegration,
    ) -> AroResult<Value> {
        let NewApiKeyIntegration {
            provider_id,
            secret,
            public_config,
            account,
            secrets_key,
        } = request;
        self.ensure_org_admin(user_id, organization_id).await?;
        if secret.trim().is_empty() {
            return Err(AroError::Configuration(
                "integration secret is required".to_string(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let provider = sqlx::query(
            r#"
            SELECT provider_id, manifest_version, auth
            FROM integration_providers
            WHERE provider_id = $1 AND deleted_at IS NULL AND enabled = true
            ORDER BY CASE WHEN source = 'seed' THEN 0 ELSE 1 END, manifest_version DESC
            LIMIT 1
            "#,
        )
        .bind(&provider_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Memory("integration provider not found".to_string()))?;

        let manifest_version: String = provider.get("manifest_version");
        let auth: Value = provider.get("auth");
        let mode = auth
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("api-key")
            .to_string();
        if mode != "api-key" {
            return Err(AroError::Configuration(
                "integration provider does not accept API-key credentials".to_string(),
            ));
        }

        let external_account_id =
            optional_value_string(&account, &["externalAccountId", "external_account_id"]);
        let account_row = if account.is_object()
            && (external_account_id.is_some()
                || optional_value_string(&account, &["displayName", "display_name"]).is_some()
                || optional_value_string(&account, &["email"]).is_some())
        {
            Some(
                sqlx::query(
                    r#"
                    INSERT INTO integration_accounts
                      (organization_id, provider_id, external_account_id, display_name, email, avatar_url, details, last_seen_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, now())
                    ON CONFLICT (organization_id, provider_id, external_account_id) DO UPDATE SET
                      display_name = COALESCE(excluded.display_name, integration_accounts.display_name),
                      email = COALESCE(excluded.email, integration_accounts.email),
                      avatar_url = COALESCE(excluded.avatar_url, integration_accounts.avatar_url),
                      details = excluded.details,
                      last_seen_at = now(),
                      deleted_at = NULL
                    RETURNING id
                    "#,
                )
                .bind(organization_id)
                .bind(&provider_id)
                .bind(external_account_id)
                .bind(optional_value_string(&account, &["displayName", "display_name"]))
                .bind(optional_value_string(&account, &["email"]))
                .bind(optional_value_string(&account, &["avatarUrl", "avatar_url"]))
                .bind(account.get("details").cloned().unwrap_or_else(|| serde_json::json!({})))
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?,
            )
        } else {
            None
        };
        let account_id = account_row.as_ref().map(|row| row.get::<Uuid, _>("id"));

        let connection_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO integration_connections
              (id, organization_id, owner_user_id, provider_id, manifest_version, account_id,
               status, enabled, granted_scopes, public_config, last_checked_at, connected_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'connected', true, ARRAY[]::TEXT[], $7, now(), now())
            "#,
        )
        .bind(connection_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(&provider_id)
        .bind(manifest_version)
        .bind(account_id)
        .bind(public_config)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let credential_payload = serde_json::json!({
            "secret": secret,
            "kind": mode,
        });
        sqlx::query(
            r#"
            INSERT INTO integration_credentials
              (organization_id, connection_id, account_id, provider_id, credential_type, store,
               key_id, key_version, encrypted_payload, metadata)
            VALUES (
              $1, $2, $3, $4, $5, 'server-vault', 'aro-secrets-key', 'pgp-symmetric',
              pgp_sym_encrypt($6::text, $7),
              '{"source":"api-key-one-shot"}'::jsonb
            )
            "#,
        )
        .bind(organization_id)
        .bind(connection_id)
        .bind(account_id)
        .bind(&provider_id)
        .bind(mode)
        .bind(credential_payload.to_string())
        .bind(&secrets_key)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        tx.commit().await.map_err(map_sqlx)?;
        let connection = self
            .get_integration(user_id, organization_id, connection_id)
            .await?
            .ok_or_else(|| AroError::Memory("integration not found".to_string()))?;
        self.emit_domain_event(
            organization_id,
            user_id,
            "integration.connected",
            "integration",
            Some(connection_id),
            redact_sensitive_json(connection.clone()),
        )
        .await?;
        Ok(connection)
    }

    pub async fn update_integration(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        connection_id: Uuid,
        patch: Value,
    ) -> AroResult<Value> {
        self.ensure_org_admin(user_id, organization_id).await?;
        let public_config = optional_json(&patch, &["publicConfig", "public_config"]);
        sqlx::query(
            r#"
            UPDATE integration_connections
            SET
              enabled = COALESCE($3, enabled),
              status = COALESCE($4, status),
              public_config = COALESCE($5, public_config)
            WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(connection_id)
        .bind(organization_id)
        .bind(optional_bool(&patch, &["enabled"]))
        .bind(optional_string(&patch, &["status"]))
        .bind(public_config)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        self.get_integration(user_id, organization_id, connection_id)
            .await?
            .ok_or_else(|| AroError::Memory("integration not found".to_string()))
    }

    pub async fn validate_integration(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        connection_id: Uuid,
    ) -> AroResult<Value> {
        self.ensure_org_admin(user_id, organization_id).await?;
        sqlx::query(
            r#"
            UPDATE integration_connections
            SET
              last_checked_at = now(),
              status = CASE WHEN revoked_at IS NULL AND enabled = true THEN 'connected' ELSE status END
            WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(connection_id)
        .bind(organization_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        self.get_integration(user_id, organization_id, connection_id)
            .await?
            .ok_or_else(|| AroError::Memory("integration not found".to_string()))
    }

    pub async fn delete_integration(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        connection_id: Uuid,
    ) -> AroResult<()> {
        self.ensure_org_admin(user_id, organization_id).await?;
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE integration_credentials
            SET revoked_at = COALESCE(revoked_at, now()), deleted_at = COALESCE(deleted_at, now())
            WHERE organization_id = $1 AND connection_id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(connection_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let connection_result = sqlx::query(
            r#"
            UPDATE integration_connections
            SET status = 'revoked', revoked_at = COALESCE(revoked_at, now()), deleted_at = COALESCE(deleted_at, now())
            WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(connection_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if connection_result.rows_affected() == 0 {
            return Err(AroError::Memory("integration not found".to_string()));
        }
        tx.commit().await.map_err(map_sqlx)?;
        self.emit_domain_event(
            organization_id,
            user_id,
            "integration.revoked",
            "integration",
            Some(connection_id),
            serde_json::json!({ "id": connection_id, "providerRevocation": "not_attempted" }),
        )
        .await?;
        Ok(())
    }

    pub async fn create_integration_oauth_state(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        request: NewIntegrationOAuthState,
    ) -> AroResult<Value> {
        let NewIntegrationOAuthState {
            provider_id,
            state_hash,
            nonce_hash,
            pkce_verifier,
            redirect_uri,
            requested_scopes,
            expires_at,
            secrets_key,
        } = request;
        self.ensure_org_admin(user_id, organization_id).await?;
        let provider = self
            .get_integration_provider(user_id, organization_id, &provider_id)
            .await?
            .ok_or_else(|| AroError::Memory("integration provider not found".to_string()))?;
        let auth = provider
            .get("auth")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let mode = auth.get("mode").and_then(Value::as_str).unwrap_or("none");
        if !matches!(mode, "oauth2" | "oidc") {
            return Err(AroError::Configuration(
                "integration provider does not support OAuth".to_string(),
            ));
        }
        let manifest_version = provider
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("2026-07-01");
        sqlx::query(
            r#"
            INSERT INTO integration_oauth_states
              (organization_id, user_id, provider_id, manifest_version, state_hash, nonce_hash,
               pkce_verifier_encrypted, redirect_uri, requested_scopes, status, expires_at)
            VALUES (
              $1, $2, $3, $4, $5, $6,
              pgp_sym_encrypt($7::text, $8),
              $9, $10, 'pending', $11
            )
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(&provider_id)
        .bind(manifest_version)
        .bind(state_hash)
        .bind(nonce_hash)
        .bind(pkce_verifier)
        .bind(&secrets_key)
        .bind(redirect_uri)
        .bind(requested_scopes)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(provider)
    }

    pub async fn get_integration_oauth_status(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        state_hash: String,
    ) -> AroResult<Value> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT jsonb_build_object(
              'providerId', provider_id,
              'status', CASE WHEN status = 'pending' AND expires_at < now() THEN 'expired' ELSE status END,
              'connectionId', connection_id,
              'error', error,
              'expiresAt', expires_at,
              'completedAt', completed_at
            ) AS data
            FROM integration_oauth_states
            WHERE organization_id = $1 AND user_id = $2 AND state_hash = $3
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(state_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Memory("OAuth state not found".to_string()))?;
        Ok(row.get("data"))
    }

    pub async fn fail_integration_oauth_state(
        &self,
        provider_id: &str,
        state_hash: String,
        error: &str,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH candidate AS (
              SELECT id, provider_id, status, connection_id, error, expires_at, completed_at
              FROM integration_oauth_states
              WHERE provider_id = $1 AND state_hash = $2
            ),
            updated AS (
              UPDATE integration_oauth_states
              SET
                status = CASE
                  WHEN expires_at < now() THEN 'expired'
                  ELSE 'failed'
                END,
                consumed_at = COALESCE(consumed_at, now()),
                completed_at = COALESCE(completed_at, now()),
                error = $3
              WHERE id IN (
                SELECT id
                FROM candidate
                WHERE status IN ('pending', 'processing')
              )
              RETURNING provider_id, status, connection_id, error, expires_at, completed_at
            ),
            selected AS (
              SELECT provider_id, status, connection_id, error, expires_at, completed_at
              FROM updated
              UNION ALL
              SELECT provider_id, status, connection_id, error, expires_at, completed_at
              FROM candidate
              WHERE NOT EXISTS (SELECT 1 FROM updated)
            )
            SELECT jsonb_build_object(
              'providerId', provider_id,
              'status', status,
              'connectionId', connection_id,
              'error', error,
              'expiresAt', expires_at,
              'completedAt', completed_at
            ) AS data
            FROM selected
            "#,
        )
        .bind(provider_id)
        .bind(state_hash)
        .bind(error)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Memory("OAuth state not found".to_string()))?;
        Ok(row.get("data"))
    }

    pub async fn consume_integration_oauth_state(
        &self,
        provider_id: &str,
        state_hash: String,
        secrets_key: &str,
    ) -> AroResult<StoredIntegrationOAuthState> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;

        let row = sqlx::query(
            r#"
            SELECT
              id, organization_id, user_id, provider_id, manifest_version, redirect_uri, requested_scopes, status, expires_at,
              pgp_sym_decrypt(pkce_verifier_encrypted, $3) AS pkce_verifier
            FROM integration_oauth_states
            WHERE provider_id = $1 AND state_hash = $2 AND consumed_at IS NULL
            FOR UPDATE
            "#
        )
        .bind(provider_id)
        .bind(&state_hash)
        .bind(secrets_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let row = match row {
            Some(r) => r,
            None => return Err(AroError::Memory("OAuth state not found".to_string())),
        };

        let status: String = row.get("status");
        let expires_at: DateTime<Utc> = row.get("expires_at");

        if status != "pending" {
            return Err(AroError::Configuration(format!(
                "OAuth state is already {status}"
            )));
        }

        if expires_at < Utc::now() {
            sqlx::query(
                "UPDATE integration_oauth_states SET status = 'expired', consumed_at = now() WHERE id = $1"
            )
            .bind(row.get::<Uuid, _>("id"))
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            tx.commit().await.map_err(map_sqlx)?;
            return Err(AroError::Configuration(
                "OAuth state has expired".to_string(),
            ));
        }

        sqlx::query(
            "UPDATE integration_oauth_states SET status = 'processing', consumed_at = now() WHERE id = $1 AND status = 'pending' AND consumed_at IS NULL"
        )
            .bind(row.get::<Uuid, _>("id"))
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;

        let pkce_verifier: String = row.get("pkce_verifier");
        let requested_scopes: Vec<String> = row.get("requested_scopes");

        let state = StoredIntegrationOAuthState {
            id: row.get("id"),
            organization_id: row.get("organization_id"),
            user_id: row.get("user_id"),
            provider_id: row.get("provider_id"),
            manifest_version: row.get("manifest_version"),
            pkce_verifier,
            redirect_uri: row.get("redirect_uri"),
            requested_scopes,
        };

        tx.commit().await.map_err(map_sqlx)?;
        Ok(state)
    }

    pub async fn succeed_integration_oauth_state(
        &self,
        id: Uuid,
        connection_id: Uuid,
    ) -> AroResult<()> {
        sqlx::query(
            "UPDATE integration_oauth_states SET status = 'completed', connection_id = $2, completed_at = now() WHERE id = $1"
        )
        .bind(id)
        .bind(connection_id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn create_oauth_integration(&self, request: NewOAuthIntegration) -> AroResult<Uuid> {
        let NewOAuthIntegration {
            user_id,
            organization_id,
            provider_id,
            manifest_version,
            granted_scopes,
            account,
            credential_payload,
            secrets_key,
        } = request;
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;

        let external_account_id =
            optional_value_string(&account, &["externalAccountId", "external_account_id"]);

        let account_row = if account.is_object()
            && (external_account_id.is_some()
                || optional_value_string(&account, &["displayName", "display_name"]).is_some()
                || optional_value_string(&account, &["email"]).is_some())
        {
            Some(
                sqlx::query(
                    r#"
                    INSERT INTO integration_accounts
                      (organization_id, provider_id, external_account_id, display_name, email, avatar_url, details, last_seen_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, now())
                    ON CONFLICT (organization_id, provider_id, external_account_id) DO UPDATE SET
                      display_name = COALESCE(excluded.display_name, integration_accounts.display_name),
                      email = COALESCE(excluded.email, integration_accounts.email),
                      avatar_url = COALESCE(excluded.avatar_url, integration_accounts.avatar_url),
                      details = excluded.details,
                      last_seen_at = now(),
                      deleted_at = NULL
                    RETURNING id
                    "#,
                )
                .bind(organization_id)
                .bind(&provider_id)
                .bind(external_account_id)
                .bind(optional_value_string(&account, &["displayName", "display_name"]))
                .bind(optional_value_string(&account, &["email"]))
                .bind(optional_value_string(&account, &["avatarUrl", "avatar_url"]))
                .bind(account.get("details").cloned().unwrap_or_else(|| serde_json::json!({})))
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?,
            )
        } else {
            None
        };
        let account_id = account_row.as_ref().map(|row| row.get::<Uuid, _>("id"));

        let connection_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO integration_connections
              (id, organization_id, owner_user_id, provider_id, manifest_version, account_id,
               status, enabled, granted_scopes, public_config, last_checked_at, connected_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'connected', true, $7, '{}'::jsonb, now(), now())
            "#,
        )
        .bind(connection_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(&provider_id)
        .bind(manifest_version)
        .bind(account_id)
        .bind(granted_scopes)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        sqlx::query(
            r#"
            INSERT INTO integration_credentials
              (organization_id, connection_id, account_id, provider_id, credential_type, store,
               key_id, key_version, encrypted_payload, metadata)
            VALUES (
              $1, $2, $3, $4, 'oauth2', 'server-vault', 'aro-secrets-key', 'pgp-symmetric',
              pgp_sym_encrypt($5::text, $6),
              '{"source":"oauth-redirect"}'::jsonb
            )
            "#,
        )
        .bind(organization_id)
        .bind(connection_id)
        .bind(account_id)
        .bind(&provider_id)
        .bind(credential_payload.to_string())
        .bind(secrets_key)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        tx.commit().await.map_err(map_sqlx)?;
        Ok(connection_id)
    }

    pub async fn list_devices_for_user(&self, user_id: Uuid) -> AroResult<Vec<Device>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, name, platform, last_seen_at, created_at
            FROM devices
            WHERE user_id = $1 AND deleted_at IS NULL
            ORDER BY COALESCE(last_seen_at, created_at) DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(map_device_row).collect()
    }

    pub async fn upsert_device(
        &self,
        user_id: Uuid,
        device_id: Option<Uuid>,
        name: String,
        platform: String,
    ) -> AroResult<Device> {
        let id = device_id.unwrap_or_else(Uuid::new_v4);
        let row = sqlx::query(
            r#"
            INSERT INTO devices (id, user_id, name, platform, last_seen_at)
            VALUES ($1, $2, $3, $4, now())
            ON CONFLICT (id)
            DO UPDATE SET name = excluded.name, platform = excluded.platform, last_seen_at = now()
            WHERE devices.user_id = excluded.user_id
            RETURNING id, user_id, name, platform, last_seen_at, created_at
            "#,
        )
        .bind(id)
        .bind(user_id)
        .bind(name)
        .bind(platform)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_device_row)
            .transpose()?
            .ok_or_else(|| AroError::Security("device belongs to another user".to_string()))
    }

    pub async fn bootstrap(
        &self,
        context: TenantContext,
        runtime: aro_core::RuntimeStatus,
        secrets_key: Option<&str>,
    ) -> AroResult<aro_core::BootstrapPayloadV2> {
        let user_id = context.actor_id();
        let organization_id = context.organization_id();
        let mut tx = self.begin_tenant_tx(context).await?;

        let current_user = sqlx::query(
            r#"
            SELECT id, email, name, role_title, avatar_color, created_at, updated_at
            FROM users
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .map(map_user_row)
        .ok_or_else(|| AroError::Security("session user not found".to_string()))?;

        let membership_rows = sqlx::query(
            r#"
            SELECT id, user_id, organization_id, role, status, created_at, updated_at
            FROM memberships
            WHERE user_id = $1
              AND status = 'active'
              AND deleted_at IS NULL
            ORDER BY created_at ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let memberships = membership_rows
            .into_iter()
            .map(map_membership_row)
            .collect::<AroResult<Vec<_>>>()?;

        let active_organization = sqlx::query(
            r#"
            SELECT id, name, domain, description, created_at, updated_at
            FROM organizations
            WHERE id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .map(map_organization_row)
        .ok_or_else(|| AroError::Security("organization not found".to_string()))?;

        let settings = match sqlx::query(
            r#"
            SELECT settings
            FROM app_settings
            WHERE organization_id = $1 AND user_id = $2
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        {
            Some(row) => serde_json::from_value(row.get::<Value, _>("settings"))
                .map_err(|err| AroError::Memory(err.to_string()))?,
            None => AppSettings::default(),
        };

        let preferences = match sqlx::query(
            r#"
            SELECT user_id, theme, language, wake_word_enabled, inference_mode, updated_at
            FROM user_preferences
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        {
            Some(row) => map_preferences_row(row)?,
            None => UserPreferences {
                user_id,
                theme: "light".to_string(),
                language: "fr".to_string(),
                wake_word_enabled: false,
                inference_mode: InferenceMode::Local,
                updated_at: Utc::now(),
            },
        };

        let conversation_rows = sqlx::query(
            r#"
            SELECT id, title, mode, created_at, updated_at, project_id, folder_id, root_path
            FROM conversations
            WHERE organization_id = $1
              AND owner_user_id = $2
              AND deleted_at IS NULL
            ORDER BY updated_at DESC
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let conversations = conversation_rows
            .into_iter()
            .map(map_conversation_row)
            .collect::<AroResult<Vec<_>>>()?;

        let state_rows = sqlx::query(
            r#"
            SELECT key, value
            FROM client_state
            WHERE organization_id = $1
              AND user_id = $2
              AND sensitive = false
              AND deleted_at IS NULL
            ORDER BY key ASC
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let mut client_state = state_rows
            .into_iter()
            .map(|row| (row.get::<String, _>("key"), row.get::<Value, _>("value")))
            .collect::<BTreeMap<_, _>>();

        if let Some(secrets_key) = secrets_key {
            let sensitive_keys = sqlx::query(
                r#"
                SELECT key
                FROM client_state
                WHERE organization_id = $1
                  AND user_id = $2
                  AND sensitive = true
                  AND deleted_at IS NULL
                ORDER BY key ASC
                "#,
            )
            .bind(organization_id)
            .bind(user_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(map_sqlx)?;

            for row in sensitive_keys {
                let key = row.get::<String, _>("key");
                let decrypted = sqlx::query(
                    r#"
                    SELECT pgp_sym_decrypt(encrypted_value, $4)::jsonb AS value
                    FROM client_state
                    WHERE organization_id = $1
                      AND user_id = $2
                      AND key = $3
                      AND sensitive = true
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(organization_id)
                .bind(user_id)
                .bind(&key)
                .bind(secrets_key)
                .fetch_optional(&mut *tx)
                .await;
                match decrypted {
                    Ok(Some(row)) => {
                        client_state.insert(key, row.get("value"));
                    }
                    Ok(None) => {}
                    Err(err) if is_client_state_decrypt_error(&err) => {
                        sqlx::query(
                            r#"
                            UPDATE client_state
                            SET deleted_at = now(), updated_at = now()
                            WHERE organization_id = $1
                              AND user_id = $2
                              AND key = $3
                              AND sensitive = true
                              AND deleted_at IS NULL
                            "#,
                        )
                        .bind(organization_id)
                        .bind(user_id)
                        .bind(&key)
                        .execute(&mut *tx)
                        .await
                        .map_err(map_sqlx)?;
                    }
                    Err(err) => return Err(map_sqlx(err)),
                }
            }
        }
        tx.commit().await.map_err(map_sqlx)?;

        Ok(aro_core::BootstrapPayloadV2 {
            current_user,
            active_organization,
            memberships,
            settings,
            preferences,
            sync_status: SyncStatus {
                health: SyncHealth::Online,
                pending_events: 0,
                last_synced_at: Some(Utc::now()),
            },
            runtime,
            conversations,
            client_state,
        })
    }

    pub async fn get_app_settings(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<AppSettings> {
        let row = sqlx::query(
            r#"
            SELECT settings
            FROM app_settings
            WHERE organization_id = $1 AND user_id = $2
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        match row {
            Some(row) => {
                let mut settings: AppSettings =
                    serde_json::from_value(row.get::<Value, _>("settings"))
                        .map_err(|err| AroError::Memory(err.to_string()))?;
                // Search credentials historically lived in this JSON document. Never return
                // or execute with those values; the cleanup migration removes existing copies.
                settings.search.api_key = None;
                settings.search.auth_configured = false;
                Ok(settings)
            }
            None => Ok(AppSettings::default()),
        }
    }

    pub async fn update_app_settings(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        settings: &AppSettings,
    ) -> AroResult<AppSettings> {
        let mut sanitized = settings.clone();
        sanitized.search.api_key = None;
        sanitized.search.auth_configured = false;
        let value =
            serde_json::to_value(&sanitized).map_err(|err| AroError::Memory(err.to_string()))?;
        sqlx::query(
            r#"
            INSERT INTO app_settings (organization_id, user_id, settings, updated_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (organization_id, user_id)
            DO UPDATE SET settings = excluded.settings, updated_at = now()
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(sanitized)
    }

    pub async fn get_preferences(&self, user_id: Uuid) -> AroResult<UserPreferences> {
        let row = sqlx::query(
            r#"
            SELECT user_id, theme, language, wake_word_enabled, inference_mode, updated_at
            FROM user_preferences
            WHERE user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        match row {
            Some(row) => map_preferences_row(row),
            None => Ok(UserPreferences {
                user_id,
                theme: "light".to_string(),
                language: "fr".to_string(),
                wake_word_enabled: false,
                inference_mode: InferenceMode::Local,
                updated_at: Utc::now(),
            }),
        }
    }

    pub async fn update_preferences(
        &self,
        user_id: Uuid,
        preferences: UserPreferencesPatch,
    ) -> AroResult<UserPreferences> {
        let existing = self.get_preferences(user_id).await?;
        let theme = preferences.theme.unwrap_or(existing.theme);
        let language = preferences.language.unwrap_or(existing.language);
        let wake_word_enabled = preferences
            .wake_word_enabled
            .unwrap_or(existing.wake_word_enabled);
        let inference_mode = preferences
            .inference_mode
            .unwrap_or(existing.inference_mode);
        let inference_mode_text = enum_to_string(&inference_mode)?;

        let row = sqlx::query(
            r#"
            INSERT INTO user_preferences
              (user_id, theme, language, wake_word_enabled, inference_mode, updated_at)
            VALUES ($1, $2, $3, $4, $5, now())
            ON CONFLICT (user_id)
            DO UPDATE SET
              theme = excluded.theme,
              language = excluded.language,
              wake_word_enabled = excluded.wake_word_enabled,
              inference_mode = excluded.inference_mode,
              updated_at = now()
            RETURNING user_id, theme, language, wake_word_enabled, inference_mode, updated_at
            "#,
        )
        .bind(user_id)
        .bind(theme)
        .bind(language)
        .bind(wake_word_enabled)
        .bind(inference_mode_text)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        map_preferences_row(row)
    }

    pub async fn upsert_conversation(
        &self,
        context: TenantContext,
        conversation: &Conversation,
    ) -> AroResult<Conversation> {
        let user_id = context.actor_id();
        let organization_id = context.organization_id();
        let mut tx = self.begin_tenant_tx(context).await?;
        let mode = enum_to_string(&conversation.mode)?;
        let row = sqlx::query(
            r#"
            INSERT INTO conversations
              (id, organization_id, owner_user_id, title, mode, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id)
            DO UPDATE SET title = excluded.title, mode = excluded.mode, updated_at = excluded.updated_at
            WHERE conversations.organization_id = excluded.organization_id
              AND conversations.owner_user_id = excluded.owner_user_id
            RETURNING id, title, mode, created_at, updated_at, project_id, folder_id, root_path
            "#,
        )
        .bind(conversation.id)
        .bind(organization_id)
        .bind(user_id)
        .bind(&conversation.title)
        .bind(mode)
        .bind(conversation.created_at)
        .bind(conversation.updated_at)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let stored = row.map(map_conversation_row).transpose()?.ok_or_else(|| {
            AroError::Security("conversation belongs to another organization".to_string())
        })?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(stored)
    }

    pub async fn create_conversation(
        &self,
        context: TenantContext,
        title: String,
        mode: AssistantMode,
    ) -> AroResult<Conversation> {
        let conversation = Conversation::new(title, mode);
        self.upsert_conversation(context, &conversation).await
    }

    pub async fn set_conversation_placement(
        &self,
        context: TenantContext,
        conversation_id: Uuid,
        project_id: Option<Uuid>,
        folder_id: Option<Uuid>,
    ) -> AroResult<Conversation> {
        // Le projet/dossier doit appartenir au tenant : sinon un id arbitraire
        // (y compris d'un autre tenant) serait accepté silencieusement.
        if let Some(project_id) = project_id {
            let owned: bool = sqlx::query_scalar(
                r#"SELECT EXISTS (SELECT 1 FROM projects WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3)"#,
            )
            .bind(project_id)
            .bind(context.organization_id())
            .bind(context.actor_id())
            .fetch_one(self.pool())
            .await
            .map_err(map_sqlx)?;
            if !owned {
                return Err(AroError::Memory("project not found".to_string()));
            }
        }
        if let Some(folder_id) = folder_id {
            let owned: bool = sqlx::query_scalar(
                r#"SELECT EXISTS (SELECT 1 FROM folders WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3)"#,
            )
            .bind(folder_id)
            .bind(context.organization_id())
            .bind(context.actor_id())
            .fetch_one(self.pool())
            .await
            .map_err(map_sqlx)?;
            if !owned {
                return Err(AroError::Memory("folder not found".to_string()));
            }
        }
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            UPDATE conversations
            SET project_id = $4,
                folder_id = $5,
                updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            RETURNING id, title, mode, created_at, updated_at, project_id, folder_id, root_path
            "#,
        )
        .bind(conversation_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(project_id)
        .bind(folder_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            tx.commit().await.map_err(map_sqlx)?;
            return Err(AroError::Memory("conversation not found".to_string()));
        };
        let conversation = map_conversation_row(row)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(conversation)
    }

    pub async fn list_conversations(&self, context: TenantContext) -> AroResult<Vec<Conversation>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, title, mode, created_at, updated_at, project_id, folder_id, root_path
            FROM conversations
            WHERE organization_id = $1
              AND owner_user_id = $2
              AND deleted_at IS NULL
            ORDER BY updated_at DESC
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let conversations = rows
            .into_iter()
            .map(map_conversation_row)
            .collect::<AroResult<Vec<_>>>()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(conversations)
    }

    pub async fn get_conversation(
        &self,
        context: TenantContext,
        conversation_id: Uuid,
    ) -> AroResult<Option<Conversation>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            SELECT id, title, mode, created_at, updated_at, project_id, folder_id, root_path
            FROM conversations
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(conversation_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let conversation = row.map(map_conversation_row).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(conversation)
    }

    pub async fn delete_conversation(
        &self,
        context: TenantContext,
        conversation_id: Uuid,
    ) -> AroResult<()> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let result = sqlx::query(
            r#"
            UPDATE conversations
            SET deleted_at = now(), updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(conversation_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(AroError::Memory("conversation not found".to_string()));
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn update_conversation_title(
        &self,
        context: TenantContext,
        conversation_id: Uuid,
        title: String,
    ) -> AroResult<Conversation> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            UPDATE conversations
            SET title = $4, updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            RETURNING id, title, mode, created_at, updated_at, project_id, folder_id, root_path
            "#,
        )
        .bind(conversation_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(title)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let conversation = row
            .map(map_conversation_row)
            .transpose()?
            .ok_or_else(|| AroError::Memory("conversation not found".to_string()))?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(conversation)
    }

    pub async fn list_projects(&self, context: TenantContext) -> AroResult<Vec<Project>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, name, description, instructions, root_path, color, icon, created_at, updated_at
            FROM projects
            WHERE organization_id = $1
              AND owner_user_id = $2
            ORDER BY updated_at DESC
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let projects = rows
            .into_iter()
            .map(map_project_row)
            .collect::<AroResult<Vec<_>>>()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(projects)
    }

    /// Fusionne les projets strictement identiques (même nom insensible à
    /// la casse + mêmes description/instructions/root) créés par les
    /// anciennes boucles de sync non-idempotentes. Les conversations et
    /// dossiers des doublons sont remappés vers le projet gardé (celui qui
    /// porte le plus de conversations, sinon le plus récent). Les homonymes
    /// volontairement différents ne fusionnent pas. Retourne les ids
    /// supprimés. Scope : organisation + propriétaire du contexte.
    pub async fn deduplicate_projects(&self, context: TenantContext) -> AroResult<Vec<Uuid>> {
        let projects = self.list_projects(context).await?;
        let mut groups: std::collections::HashMap<String, Vec<Project>> =
            std::collections::HashMap::new();
        for p in projects {
            let key = format!(
                "{}\n{}\n{}\n{}",
                p.name.trim().to_lowercase(),
                p.root_path.clone().unwrap_or_default(),
                p.description.clone().unwrap_or_default(),
                p.instructions.clone().unwrap_or_default()
            );
            groups.entry(key).or_default().push(p);
        }
        let mut removed = Vec::new();
        for members in groups.values().filter(|m| m.len() > 1) {
            let mut tx = self.begin_tenant_tx(context).await?;
            // Nombre de conversations par projet pour choisir le gardé.
            let mut counts: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();
            for m in members {
                let count: i64 = sqlx::query_scalar(
                    r#"SELECT COUNT(*) FROM conversations WHERE project_id = $1 AND organization_id = $2 AND deleted_at IS NULL"#,
                )
                .bind(m.id)
                .bind(context.organization_id())
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                counts.insert(m.id, count);
            }
            let mut ranked = members.clone();
            ranked.sort_by(|a, b| {
                counts
                    .get(&b.id)
                    .unwrap_or(&0)
                    .cmp(counts.get(&a.id).unwrap_or(&0))
                    .then_with(|| b.updated_at.cmp(&a.updated_at))
            });
            let keeper = ranked[0].id;
            for dup in &ranked[1..] {
                sqlx::query(
                    r#"UPDATE conversations SET project_id = $1, updated_at = now() WHERE project_id = $2 AND organization_id = $3"#,
                )
                .bind(keeper)
                .bind(dup.id)
                .bind(context.organization_id())
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                sqlx::query(
                    r#"UPDATE folders SET project_id = $1 WHERE project_id = $2 AND organization_id = $3"#,
                )
                .bind(keeper)
                .bind(dup.id)
                .bind(context.organization_id())
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                sqlx::query(
                    r#"DELETE FROM projects WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3"#,
                )
                .bind(dup.id)
                .bind(context.organization_id())
                .bind(context.actor_id())
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                removed.push(dup.id);
            }
            tx.commit().await.map_err(map_sqlx)?;
        }
        Ok(removed)
    }

    /// Idem pour les dossiers : même nom sous le même projet et même root.
    pub async fn deduplicate_folders(&self, context: TenantContext) -> AroResult<Vec<Uuid>> {
        let folders = self.list_folders(context).await?;
        let mut groups: std::collections::HashMap<String, Vec<Folder>> =
            std::collections::HashMap::new();
        for f in folders {
            let key = format!(
                "{}\n{}\n{}",
                f.project_id.map(|id| id.to_string()).unwrap_or_default(),
                f.name.trim().to_lowercase(),
                f.root_path.clone().unwrap_or_default()
            );
            groups.entry(key).or_default().push(f);
        }
        let mut removed = Vec::new();
        for members in groups.values().filter(|m| m.len() > 1) {
            let mut tx = self.begin_tenant_tx(context).await?;
            let mut counts: std::collections::HashMap<Uuid, i64> = std::collections::HashMap::new();
            for m in members {
                let count: i64 = sqlx::query_scalar(
                    r#"SELECT COUNT(*) FROM conversations WHERE folder_id = $1 AND organization_id = $2 AND deleted_at IS NULL"#,
                )
                .bind(m.id)
                .bind(context.organization_id())
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                counts.insert(m.id, count);
            }
            let mut ranked = members.clone();
            ranked.sort_by(|a, b| {
                counts
                    .get(&b.id)
                    .unwrap_or(&0)
                    .cmp(counts.get(&a.id).unwrap_or(&0))
                    .then_with(|| b.updated_at.cmp(&a.updated_at))
            });
            let keeper = ranked[0].id;
            for dup in &ranked[1..] {
                sqlx::query(
                    r#"UPDATE conversations SET folder_id = $1, updated_at = now() WHERE folder_id = $2 AND organization_id = $3"#,
                )
                .bind(keeper)
                .bind(dup.id)
                .bind(context.organization_id())
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                sqlx::query(
                    r#"DELETE FROM folders WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3"#,
                )
                .bind(dup.id)
                .bind(context.organization_id())
                .bind(context.actor_id())
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                removed.push(dup.id);
            }
            tx.commit().await.map_err(map_sqlx)?;
        }
        Ok(removed)
    }

    pub async fn get_project(
        &self,
        context: TenantContext,
        project_id: Uuid,
    ) -> AroResult<Option<Project>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            SELECT id, name, description, instructions, root_path, color, icon, created_at, updated_at
            FROM projects
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
            "#,
        )
        .bind(project_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let project = row.map(map_project_row).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(project)
    }

    pub async fn create_project(
        &self,
        context: TenantContext,
        project: &Project,
    ) -> AroResult<Project> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            INSERT INTO projects (id, organization_id, owner_user_id, name, description, instructions, root_path, color, icon, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, name, description, instructions, root_path, color, icon, created_at, updated_at
            "#,
        )
        .bind(project.id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&project.name)
        .bind(&project.description)
        .bind(&project.instructions)
        .bind(&project.root_path)
        .bind(&project.color)
        .bind(&project.icon)
        .bind(project.created_at)
        .bind(project.updated_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let saved = map_project_row(row)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(saved)
    }

    // Huit paramètres stables d'API publique : on garde la signature.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_project(
        &self,
        context: TenantContext,
        project_id: Uuid,
        name: Option<String>,
        description: Option<Option<String>>,
        instructions: Option<Option<String>>,
        root_path: Option<Option<String>>,
        color: Option<String>,
        icon: Option<String>,
    ) -> AroResult<Option<Project>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let existing = sqlx::query(
            r#"
            SELECT id, name, description, instructions, root_path, color, icon, created_at, updated_at
            FROM projects
            WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3
            "#,
        )
        .bind(project_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(existing_row) = existing else {
            return Ok(None);
        };
        let mut proj = map_project_row(existing_row)?;
        if let Some(n) = name {
            proj.name = n;
        }
        if let Some(d) = description {
            proj.description = d;
        }
        if let Some(i) = instructions {
            proj.instructions = i;
        }
        if let Some(r) = root_path {
            proj.root_path = r;
        }
        if let Some(c) = color {
            proj.color = c;
        }
        if let Some(ic) = icon {
            proj.icon = ic;
        }
        proj.updated_at = Utc::now();

        let row = sqlx::query(
            r#"
            UPDATE projects
            SET name = $4, description = $5, instructions = $6, root_path = $7, color = $8, icon = $9, updated_at = $10
            WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3
            RETURNING id, name, description, instructions, root_path, color, icon, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&proj.name)
        .bind(&proj.description)
        .bind(&proj.instructions)
        .bind(&proj.root_path)
        .bind(&proj.color)
        .bind(&proj.icon)
        .bind(proj.updated_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let updated = map_project_row(row)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(Some(updated))
    }

    pub async fn delete_project(
        &self,
        context: TenantContext,
        project_id: Uuid,
    ) -> AroResult<bool> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let res = sqlx::query(
            r#"
            DELETE FROM projects
            WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3
            "#,
        )
        .bind(project_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn list_folders(&self, context: TenantContext) -> AroResult<Vec<Folder>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, name, root_path, color, icon, created_at, updated_at
            FROM folders
            WHERE organization_id = $1
              AND owner_user_id = $2
            ORDER BY updated_at DESC
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let folders = rows
            .into_iter()
            .map(map_folder_row)
            .collect::<AroResult<Vec<_>>>()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(folders)
    }

    pub async fn get_folder(
        &self,
        context: TenantContext,
        folder_id: Uuid,
    ) -> AroResult<Option<Folder>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            SELECT id, project_id, name, root_path, color, icon, created_at, updated_at
            FROM folders
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
            "#,
        )
        .bind(folder_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let folder = row.map(map_folder_row).transpose()?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(folder)
    }

    pub async fn create_folder(
        &self,
        context: TenantContext,
        folder: &Folder,
    ) -> AroResult<Folder> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            INSERT INTO folders (id, organization_id, owner_user_id, project_id, name, root_path, color, icon, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, project_id, name, root_path, color, icon, created_at, updated_at
            "#,
        )
        .bind(folder.id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(folder.project_id)
        .bind(&folder.name)
        .bind(&folder.root_path)
        .bind(&folder.color)
        .bind(&folder.icon)
        .bind(folder.created_at)
        .bind(folder.updated_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let saved = map_folder_row(row)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(saved)
    }

    // Huit paramètres stables d'API publique : on garde la signature.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_folder(
        &self,
        context: TenantContext,
        folder_id: Uuid,
        name: Option<String>,
        project_id: Option<Option<Uuid>>,
        root_path: Option<Option<String>>,
        color: Option<Option<String>>,
        icon: Option<String>,
    ) -> AroResult<Option<Folder>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let existing = sqlx::query(
            r#"
            SELECT id, project_id, name, root_path, color, icon, created_at, updated_at
            FROM folders
            WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3
            "#,
        )
        .bind(folder_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(existing_row) = existing else {
            return Ok(None);
        };
        let mut fold = map_folder_row(existing_row)?;
        if let Some(n) = name {
            fold.name = n;
        }
        if let Some(pid) = project_id {
            fold.project_id = pid;
        }
        if let Some(r) = root_path {
            fold.root_path = r;
        }
        if let Some(c) = color {
            fold.color = c;
        }
        if let Some(ic) = icon {
            fold.icon = ic;
        }
        fold.updated_at = Utc::now();

        let row = sqlx::query(
            r#"
            UPDATE folders
            SET name = $4, project_id = $5, root_path = $6, color = $7, icon = $8, updated_at = $9
            WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3
            RETURNING id, project_id, name, root_path, color, icon, created_at, updated_at
            "#,
        )
        .bind(folder_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&fold.name)
        .bind(fold.project_id)
        .bind(&fold.root_path)
        .bind(&fold.color)
        .bind(&fold.icon)
        .bind(fold.updated_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let updated = map_folder_row(row)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(Some(updated))
    }

    pub async fn delete_folder(&self, context: TenantContext, folder_id: Uuid) -> AroResult<bool> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let res = sqlx::query(
            r#"
            DELETE FROM folders
            WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3
            "#,
        )
        .bind(folder_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn add_message(
        &self,
        context: TenantContext,
        message: &ChatMessage,
    ) -> AroResult<ChatMessage> {
        let organization_id = context.organization_id();
        let mut tx = self.begin_tenant_tx(context).await?;
        ensure_conversation_owner_in_tx(&mut tx, context, message.conversation_id).await?;
        let role = enum_to_string(&message.role)?;
        let row = sqlx::query(
            r#"
            INSERT INTO messages
              (id, organization_id, conversation_id, role, content, created_at, token_estimate, model_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, conversation_id, role, content, created_at, token_estimate, model_id
            "#,
        )
        .bind(message.id)
        .bind(organization_id)
        .bind(message.conversation_id)
        .bind(role)
        .bind(&message.content)
        .bind(message.created_at)
        .bind(message.token_estimate.map(|value| value as i32))
        .bind(&message.model_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE conversations
            SET updated_at = now()
            WHERE organization_id = $1
              AND id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(message.conversation_id)
        .bind(context.actor_id())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let mut stored = map_message_row(row)?;
        if !message.attachments.is_empty() {
            self.link_message_attachments(
                &mut tx,
                context,
                message.conversation_id,
                stored.id,
                &message.attachments,
            )
            .await?;
            stored.attachments = message.attachments.clone();
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(stored)
    }

    pub async fn update_message(
        &self,
        context: TenantContext,
        message_id: Uuid,
        content: String,
    ) -> AroResult<ChatMessage> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            UPDATE messages m
            SET content = $4
            FROM conversations c
            WHERE m.id = $1
              AND m.organization_id = $2
              AND c.id = m.conversation_id
              AND c.organization_id = m.organization_id
              AND c.owner_user_id = $3
              AND m.deleted_at IS NULL
              AND c.deleted_at IS NULL
            RETURNING m.id, m.conversation_id, m.role, m.content, m.created_at, m.token_estimate, m.model_id
            "#,
        )
        .bind(message_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(content)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| aro_core::AroError::Memory("message not found".to_string()))?;

        let message = map_message_row(row)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(message)
    }

    pub async fn list_messages(
        &self,
        context: TenantContext,
        conversation_id: Uuid,
    ) -> AroResult<Vec<ChatMessage>> {
        let organization_id = context.organization_id();
        let mut tx = self.begin_tenant_tx(context).await?;
        match ensure_conversation_owner_in_tx(&mut tx, context, conversation_id).await {
            Ok(()) => {}
            Err(AroError::Security(_)) => {
                return Err(AroError::Memory("conversation not found".to_string()));
            }
            Err(error) => return Err(error),
        }
        let rows = sqlx::query(
            r#"
            SELECT m.id, m.conversation_id, m.role, m.content, m.created_at, m.token_estimate, m.model_id
            FROM messages m
            JOIN conversations c ON c.organization_id = m.organization_id AND c.id = m.conversation_id
            WHERE m.organization_id = $1
              AND m.conversation_id = $2
              AND c.owner_user_id = $3
              AND m.deleted_at IS NULL
              AND c.deleted_at IS NULL
            ORDER BY m.created_at ASC
            "#,
        )
        .bind(organization_id)
        .bind(conversation_id)
        .bind(context.actor_id())
        .fetch_all(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let mut messages = rows
            .into_iter()
            .map(map_message_row)
            .collect::<AroResult<Vec<_>>>()?;
        self.hydrate_message_attachments(&mut tx, context, &mut messages)
            .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(messages)
    }

    pub async fn store_local_result(
        &self,
        context: TenantContext,
        conversation: Conversation,
        user_message: ChatMessage,
        assistant_message: ChatMessage,
    ) -> AroResult<()> {
        if user_message.conversation_id != conversation.id
            || assistant_message.conversation_id != conversation.id
        {
            return Err(AroError::Memory(
                "local result messages must belong to the supplied conversation".to_string(),
            ));
        }
        let user_id = context.actor_id();
        let organization_id = context.organization_id();
        let mut tx = self.begin_tenant_tx(context).await?;
        let mode = enum_to_string(&conversation.mode)?;
        let conversation_result = sqlx::query(
            r#"
            INSERT INTO conversations
              (id, organization_id, owner_user_id, title, mode, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (id)
            DO UPDATE SET title = excluded.title, mode = excluded.mode, updated_at = excluded.updated_at
            WHERE conversations.organization_id = excluded.organization_id
              AND conversations.owner_user_id = excluded.owner_user_id
            "#,
        )
        .bind(conversation.id)
        .bind(organization_id)
        .bind(user_id)
        .bind(conversation.title)
        .bind(mode)
        .bind(conversation.created_at)
        .bind(conversation.updated_at)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if conversation_result.rows_affected() == 0 {
            return Err(AroError::Security(
                "conversation belongs to another organization".to_string(),
            ));
        }
        insert_message_in_tx(&mut tx, organization_id, &user_message).await?;
        insert_message_in_tx(&mut tx, organization_id, &assistant_message).await?;
        if !user_message.attachments.is_empty() {
            self.link_message_attachments(
                &mut tx,
                context,
                user_message.conversation_id,
                user_message.id,
                &user_message.attachments,
            )
            .await?;
        }
        if !assistant_message.attachments.is_empty() {
            self.link_message_attachments(
                &mut tx,
                context,
                assistant_message.conversation_id,
                assistant_message.id,
                &assistant_message.attachments,
            )
            .await?;
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn get_organization_storage_usage(&self, organization_id: Uuid) -> AroResult<i64> {
        let row = sqlx::query(
            r#"
            SELECT COALESCE(SUM(size_bytes), 0)::BIGINT as total_bytes
            FROM file_objects
            WHERE organization_id = $1 AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;

        let total: i64 = row.get("total_bytes");
        Ok(total)
    }

    pub async fn list_quarantined_files(
        &self,
        organization_id: Uuid,
    ) -> AroResult<Vec<FileObject>> {
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, owner_user_id, original_name, mime_type, size_bytes,
                   sha256, status, scan_status, created_at, updated_at
            FROM file_objects
            WHERE organization_id = $1 AND scan_status = 'infected' AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        rows.into_iter().map(map_file_object_row).collect()
    }

    pub async fn create_file_upload_session(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        request: NewFileUpload,
    ) -> AroResult<PreparedFileUpload> {
        self.ensure_org_access(user_id, organization_id).await?;
        if request.size_bytes < 0 {
            return Err(AroError::Memory("file size cannot be negative".to_string()));
        }

        let current_bytes = self.get_organization_storage_usage(organization_id).await?;
        let quota_limit: i64 = 5 * 1024 * 1024 * 1024; // 5 GB limit
        if current_bytes + request.size_bytes > quota_limit {
            return Err(AroError::Security(format!(
                "organization storage quota exceeded: current {} MB, requested {} MB, quota limit {} MB",
                current_bytes / 1024 / 1024,
                request.size_bytes / 1024 / 1024,
                quota_limit / 1024 / 1024
            )));
        }

        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let pending = enum_to_string(&FileStatus::Pending)?;
        let scan_pending = enum_to_string(&FileScanStatus::Pending)?;
        let file_row = sqlx::query(
            r#"
            INSERT INTO file_objects
              (id, organization_id, owner_user_id, original_name, mime_type, size_bytes,
               sha256, storage_backend, bucket, object_key, status, scan_status)
            VALUES ($1, $2, $3, $4, $5, $6, '', $7, $8, $9, $10, $11)
            RETURNING id, organization_id, owner_user_id, original_name, mime_type, size_bytes,
                      sha256, status, scan_status, created_at, updated_at
            "#,
        )
        .bind(request.file_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(request.original_name.trim())
        .bind(request.mime_type.trim())
        .bind(request.size_bytes)
        .bind(&request.storage_backend)
        .bind(&request.bucket)
        .bind(&request.object_key)
        .bind(&pending)
        .bind(&scan_pending)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let file = map_file_object_row(file_row)?;

        let upload_row = sqlx::query(
            r#"
            INSERT INTO file_upload_sessions
              (organization_id, owner_user_id, file_id, expected_name, expected_mime_type,
               expected_size_bytes, expected_sha256, status, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, file_id, organization_id, owner_user_id, expected_name,
                      expected_mime_type, expected_size_bytes, expected_sha256,
                      status, expires_at, created_at
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(request.file_id)
        .bind(request.original_name.trim())
        .bind(request.mime_type.trim())
        .bind(request.size_bytes)
        .bind(&request.sha256)
        .bind(&pending)
        .bind(request.expires_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let upload = map_file_upload_session_row(upload_row)?;
        tx.commit().await.map_err(map_sqlx)?;

        self.emit_domain_event(
            organization_id,
            user_id,
            "file.upload.created",
            "file_object",
            Some(file.id),
            serde_json::json!({
                "fileId": file.id,
                "uploadId": upload.id,
                "mimeType": file.mime_type,
                "sizeBytes": file.size_bytes,
                "status": "pending"
            }),
        )
        .await?;

        Ok(PreparedFileUpload {
            file,
            upload,
            object_key: request.object_key,
            storage_backend: request.storage_backend,
            bucket: request.bucket,
        })
    }

    pub async fn get_file_upload_target(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        upload_id: Uuid,
    ) -> AroResult<PreparedFileUpload> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT fo.id, fo.organization_id, fo.owner_user_id, fo.original_name,
                   fo.mime_type, fo.size_bytes, fo.sha256, fo.status, fo.scan_status,
                   fo.created_at, fo.updated_at, fo.storage_backend, fo.bucket, fo.object_key,
                   fus.id AS upload_id, fus.file_id, fus.expected_name, fus.expected_mime_type,
                   fus.expected_size_bytes, fus.expected_sha256, fus.status AS upload_status,
                   fus.expires_at, fus.created_at AS upload_created_at
            FROM file_upload_sessions fus
            JOIN file_objects fo
              ON fo.organization_id = fus.organization_id
             AND fo.id = fus.file_id
            WHERE fus.id = $1
              AND fus.organization_id = $2
              AND fus.owner_user_id = $3
              AND fus.deleted_at IS NULL
              AND fo.deleted_at IS NULL
            "#,
        )
        .bind(upload_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            return Err(AroError::Memory("upload session not found".to_string()));
        };
        let file = FileObject {
            id: row.get("id"),
            organization_id: row.get("organization_id"),
            owner_user_id: row.get("owner_user_id"),
            original_name: row.get("original_name"),
            mime_type: row.get("mime_type"),
            size_bytes: row.get("size_bytes"),
            sha256: row.get("sha256"),
            status: enum_from_string(row.get::<String, _>("status"))?,
            scan_status: enum_from_string(row.get::<String, _>("scan_status"))?,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };
        let upload = FileUploadSession {
            id: row.get("upload_id"),
            file_id: row.get("file_id"),
            organization_id,
            owner_user_id: user_id,
            expected_name: row.get("expected_name"),
            expected_mime_type: row.get("expected_mime_type"),
            expected_size_bytes: row.get("expected_size_bytes"),
            expected_sha256: row.get("expected_sha256"),
            status: enum_from_string(row.get::<String, _>("upload_status"))?,
            expires_at: row.get("expires_at"),
            created_at: row.get("upload_created_at"),
        };

        Ok(PreparedFileUpload {
            object_key: row.get("object_key"),
            storage_backend: row.get("storage_backend"),
            bucket: row.get("bucket"),
            file,
            upload,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn complete_file_upload_session(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        upload_id: Uuid,
        size_bytes: i64,
        sha256: String,
        mime_type: String,
        etag: Option<String>,
    ) -> AroResult<FileObject> {
        self.ensure_org_access(user_id, organization_id).await?;
        let session_row = sqlx::query(
            r#"
            SELECT id, file_id, organization_id, owner_user_id, expected_name,
                   expected_mime_type, expected_size_bytes, expected_sha256,
                   status, expires_at, created_at
            FROM file_upload_sessions
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(upload_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        let Some(session_row) = session_row else {
            return Err(AroError::Memory("upload session not found".to_string()));
        };
        let session = map_file_upload_session_row(session_row)?;
        if session.status != FileStatus::Pending {
            return Err(AroError::Memory(
                "upload session is not pending".to_string(),
            ));
        }
        if session.expires_at < Utc::now() {
            return Err(AroError::Security("upload session expired".to_string()));
        }
        if size_bytes != session.expected_size_bytes {
            return Err(AroError::Security(
                "uploaded file size does not match the session".to_string(),
            ));
        }
        if let Some(expected_sha256) = &session.expected_sha256 {
            if expected_sha256 != &sha256 {
                return Err(AroError::Security(
                    "uploaded checksum does not match the session".to_string(),
                ));
            }
        }

        // Upload completion only proves that object storage received the bytes.
        // A worker must scan and approve the object before it becomes available
        // to download, attach, index, or include in model context.
        let pending = enum_to_string(&FileStatus::Pending)?;
        let scan_pending = enum_to_string(&FileScanStatus::Pending)?;
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let file_row = sqlx::query(
            r#"
            UPDATE file_objects
            SET size_bytes = $4,
                sha256 = $5,
                mime_type = $6,
                etag = $7,
                status = $8,
                scan_status = $9,
                updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            RETURNING id, organization_id, owner_user_id, original_name, mime_type, size_bytes,
                      sha256, status, scan_status, created_at, updated_at
            "#,
        )
        .bind(session.file_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(size_bytes)
        .bind(&sha256)
        .bind(mime_type)
        .bind(&etag)
        .bind(&pending)
        .bind(&scan_pending)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let file = map_file_object_row(file_row)?;

        sqlx::query(
            r#"
            UPDATE file_upload_sessions
            SET status = $4, completed_at = now()
            WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3
            "#,
        )
        .bind(upload_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(&pending)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        sqlx::query(
            r#"
            INSERT INTO file_scan_jobs (organization_id, file_id, status)
            VALUES ($1, $2, 'queued')
            ON CONFLICT (organization_id, file_id) DO NOTHING
            "#,
        )
        .bind(organization_id)
        .bind(file.id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;

        self.emit_domain_event(
            organization_id,
            user_id,
            "file.upload.completed",
            "file_object",
            Some(file.id),
            serde_json::json!({
                "fileId": file.id,
                "uploadId": upload_id,
                "mimeType": file.mime_type,
                "sizeBytes": file.size_bytes,
                "sha256": file.sha256,
                "status": "pending"
            }),
        )
        .await?;

        Ok(file)
    }

    /// Claims pending or abandoned scan jobs. A claim is guarded by a worker-generated lease
    /// token; only the worker that holds it may approve, quarantine, or retry the file.
    pub async fn claim_pending_file_scan_jobs(
        &self,
        limit: i64,
        lease_token: Uuid,
        lease_seconds: i64,
        max_attempts: i32,
    ) -> AroResult<Vec<ClaimedFileScanJob>> {
        let rows = sqlx::query(
            r#"
            WITH candidates AS (
                SELECT fsj.id
                FROM file_scan_jobs fsj
                JOIN file_objects fo
                  ON fo.organization_id = fsj.organization_id
                 AND fo.id = fsj.file_id
                WHERE fsj.deleted_at IS NULL
                  AND fo.deleted_at IS NULL
                  AND fo.status = 'pending'
                  AND fsj.attempts < $4
                  AND (
                    fsj.status = 'queued'
                    OR (fsj.status = 'processing' AND fsj.lease_until <= now())
                  )
                ORDER BY fsj.created_at ASC
                FOR UPDATE OF fsj SKIP LOCKED
                LIMIT $1
            ), claimed AS (
                UPDATE file_scan_jobs fsj
                SET status = 'processing',
                    attempts = fsj.attempts + 1,
                    lease_token = $2,
                    lease_until = now() + ($3 * interval '1 second'),
                    last_error = NULL
                FROM candidates
                WHERE fsj.id = candidates.id
                RETURNING fsj.id, fsj.organization_id, fsj.file_id, fsj.attempts, fsj.lease_token
            )
            SELECT claimed.id,
                   claimed.organization_id,
                   claimed.file_id,
                   claimed.attempts,
                   claimed.lease_token,
                   fo.owner_user_id,
                   fo.object_key
            FROM claimed
            JOIN file_objects fo
              ON fo.organization_id = claimed.organization_id
             AND fo.id = claimed.file_id
            "#,
        )
        .bind(limit.clamp(1, 100))
        .bind(lease_token)
        .bind(lease_seconds.clamp(5, 300))
        .bind(max_attempts.clamp(1, 20))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        Ok(rows
            .into_iter()
            .map(|row| ClaimedFileScanJob {
                id: row.get("id"),
                organization_id: row.get("organization_id"),
                file_id: row.get("file_id"),
                owner_user_id: row.get("owner_user_id"),
                object_key: row.get("object_key"),
                attempts: row.get("attempts"),
                lease_token: row.get("lease_token"),
            })
            .collect())
    }

    /// Marks a scanned file available and atomically queues text indexing. If the lease has
    /// expired or was reclaimed, no state is changed and `None` is returned.
    pub async fn acknowledge_file_scan_clean(
        &self,
        job_id: Uuid,
        lease_token: Uuid,
    ) -> AroResult<Option<FileObject>> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let job = sqlx::query(
            r#"
            SELECT fsj.organization_id, fsj.file_id, fo.owner_user_id
            FROM file_scan_jobs fsj
            JOIN file_objects fo
              ON fo.organization_id = fsj.organization_id
             AND fo.id = fsj.file_id
            WHERE fsj.id = $1
              AND fsj.status = 'processing'
              AND fsj.lease_token = $2
              AND fsj.lease_until > now()
              AND fsj.deleted_at IS NULL
            FOR UPDATE OF fsj, fo
            "#,
        )
        .bind(job_id)
        .bind(lease_token)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(job) = job else {
            return Ok(None);
        };
        let organization_id: Uuid = job.get("organization_id");
        let file_id: Uuid = job.get("file_id");
        let owner_user_id: Uuid = job.get("owner_user_id");

        sqlx::query(
            r#"
            UPDATE file_scan_jobs
            SET status = 'completed', lease_token = NULL, lease_until = NULL,
                completed_at = now(), last_error = NULL
            WHERE id = $1 AND lease_token = $2 AND status = 'processing'
            "#,
        )
        .bind(job_id)
        .bind(lease_token)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let file = sqlx::query(
            r#"
            UPDATE file_objects
            SET status = 'available', scan_status = 'clean', updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND status = 'pending'
              AND deleted_at IS NULL
            RETURNING id, organization_id, owner_user_id, original_name, mime_type, size_bytes,
                      sha256, status, scan_status, created_at, updated_at
            "#,
        )
        .bind(file_id)
        .bind(organization_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(file) = file else {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(None);
        };
        let file = map_file_object_row(file)?;

        sqlx::query(
            r#"
            INSERT INTO file_index_jobs (organization_id, file_id, status, content_hash)
            VALUES ($1, $2, 'queued', $3)
            "#,
        )
        .bind(organization_id)
        .bind(file.id)
        .bind(&file.sha256)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        emit_domain_event_in_tx(
            &mut tx,
            organization_id,
            owner_user_id,
            "file.scan.clean",
            "file_object",
            Some(file.id),
            serde_json::json!({
                "fileId": file.id,
                "status": "available",
                "scanStatus": "clean"
            }),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(Some(file))
    }

    /// Automatically approves a file scan when ClamAV is not required (e.g. in dev/test without ClamAV daemon).
    pub async fn auto_approve_file_scan(
        &self,
        organization_id: Uuid,
        file_id: Uuid,
    ) -> AroResult<Option<FileObject>> {
        let lease_token = Uuid::new_v4();
        let job_id: Option<Uuid> = sqlx::query_scalar(
            r#"
            UPDATE file_scan_jobs
            SET status = 'processing',
                attempts = attempts + 1,
                lease_token = $3,
                lease_until = now() + interval '60 seconds',
                last_error = NULL
            WHERE organization_id = $1
              AND file_id = $2
              AND deleted_at IS NULL
              AND status IN ('queued', 'processing')
            RETURNING id
            "#,
        )
        .bind(organization_id)
        .bind(file_id)
        .bind(lease_token)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        if let Some(job_id) = job_id {
            self.acknowledge_file_scan_clean(job_id, lease_token).await
        } else {
            Ok(None)
        }
    }

    /// Quarantines a file when the scanner identifies malicious content. The raw scanner
    /// signature is intentionally not retained in the database or event stream.
    pub async fn quarantine_file_scan_job(
        &self,
        job_id: Uuid,
        lease_token: Uuid,
    ) -> AroResult<Option<FileObject>> {
        self.finish_file_scan_job(job_id, lease_token, "blocked", "quarantined", "blocked")
            .await
    }

    /// Releases a temporary scanner failure for retry, or permanently fails the file once the
    /// configured attempt budget is exhausted. Error text must be generic and non-sensitive.
    pub async fn release_file_scan_job_for_retry(
        &self,
        job_id: Uuid,
        lease_token: Uuid,
        retry_after_seconds: i64,
        max_attempts: i32,
        error: &str,
    ) -> AroResult<Option<bool>> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let job = sqlx::query(
            r#"
            SELECT fsj.organization_id, fsj.file_id, fsj.attempts, fo.owner_user_id
            FROM file_scan_jobs fsj
            JOIN file_objects fo
              ON fo.organization_id = fsj.organization_id
             AND fo.id = fsj.file_id
            WHERE fsj.id = $1
              AND fsj.status = 'processing'
              AND fsj.lease_token = $2
              AND fsj.deleted_at IS NULL
            FOR UPDATE OF fsj, fo
            "#,
        )
        .bind(job_id)
        .bind(lease_token)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(job) = job else {
            return Ok(None);
        };
        let organization_id: Uuid = job.get("organization_id");
        let file_id: Uuid = job.get("file_id");
        let owner_user_id: Uuid = job.get("owner_user_id");
        let permanently_failed = job.get::<i32, _>("attempts") >= max_attempts.clamp(1, 20);
        let safe_error = error.chars().take(160).collect::<String>();

        if permanently_failed {
            sqlx::query(
                r#"
                UPDATE file_scan_jobs
                SET status = 'failed', lease_token = NULL, lease_until = NULL,
                    completed_at = now(), last_error = $3
                WHERE id = $1 AND lease_token = $2 AND status = 'processing'
                "#,
            )
            .bind(job_id)
            .bind(lease_token)
            .bind(&safe_error)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            sqlx::query(
                r#"
                UPDATE file_objects
                SET status = 'failed', updated_at = now()
                WHERE id = $1 AND organization_id = $2 AND status = 'pending' AND deleted_at IS NULL
                "#,
            )
            .bind(file_id)
            .bind(organization_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            emit_domain_event_in_tx(
                &mut tx,
                organization_id,
                owner_user_id,
                "file.scan.failed",
                "file_object",
                Some(file_id),
                serde_json::json!({ "fileId": file_id, "status": "failed" }),
            )
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE file_scan_jobs
                SET status = 'queued', lease_token = NULL,
                    lease_until = now() + ($3 * interval '1 second'), last_error = $4
                WHERE id = $1 AND lease_token = $2 AND status = 'processing'
                "#,
            )
            .bind(job_id)
            .bind(lease_token)
            .bind(retry_after_seconds.clamp(5, 3600))
            .bind(&safe_error)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(Some(permanently_failed))
    }

    /// Converts abandoned jobs that have already consumed their retry budget into failed files.
    /// This is necessary if a worker dies after claiming its final attempt.
    pub async fn fail_exhausted_file_scan_jobs(&self, max_attempts: i32) -> AroResult<u64> {
        let result = sqlx::query(
            r#"
            WITH exhausted AS (
                UPDATE file_scan_jobs
                SET status = 'failed', lease_token = NULL, lease_until = NULL,
                    completed_at = now(), last_error = 'file scan worker did not complete'
                WHERE status = 'processing'
                  AND lease_until <= now()
                  AND attempts >= $1
                  AND deleted_at IS NULL
                RETURNING organization_id, file_id
            )
            UPDATE file_objects fo
            SET status = 'failed', updated_at = now()
            FROM exhausted
            WHERE fo.organization_id = exhausted.organization_id
              AND fo.id = exhausted.file_id
              AND fo.status = 'pending'
              AND fo.deleted_at IS NULL
            "#,
        )
        .bind(max_attempts.clamp(1, 20))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected())
    }

    async fn finish_file_scan_job(
        &self,
        job_id: Uuid,
        lease_token: Uuid,
        job_status: &str,
        file_status: &str,
        scan_status: &str,
    ) -> AroResult<Option<FileObject>> {
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let job = sqlx::query(
            r#"
            SELECT fsj.organization_id, fsj.file_id, fo.owner_user_id
            FROM file_scan_jobs fsj
            JOIN file_objects fo
              ON fo.organization_id = fsj.organization_id
             AND fo.id = fsj.file_id
            WHERE fsj.id = $1
              AND fsj.status = 'processing'
              AND fsj.lease_token = $2
              AND fsj.lease_until > now()
              AND fsj.deleted_at IS NULL
            FOR UPDATE OF fsj, fo
            "#,
        )
        .bind(job_id)
        .bind(lease_token)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(job) = job else {
            return Ok(None);
        };
        let organization_id: Uuid = job.get("organization_id");
        let file_id: Uuid = job.get("file_id");
        let owner_user_id: Uuid = job.get("owner_user_id");

        sqlx::query(
            r#"
            UPDATE file_scan_jobs
            SET status = $3, lease_token = NULL, lease_until = NULL,
                completed_at = now(), last_error = NULL
            WHERE id = $1 AND lease_token = $2 AND status = 'processing'
            "#,
        )
        .bind(job_id)
        .bind(lease_token)
        .bind(job_status)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let file = sqlx::query(
            r#"
            UPDATE file_objects
            SET status = $3, scan_status = $4, updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND status = 'pending'
              AND deleted_at IS NULL
            RETURNING id, organization_id, owner_user_id, original_name, mime_type, size_bytes,
                      sha256, status, scan_status, created_at, updated_at
            "#,
        )
        .bind(file_id)
        .bind(organization_id)
        .bind(file_status)
        .bind(scan_status)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let Some(file) = file else {
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(None);
        };
        let file = map_file_object_row(file)?;
        emit_domain_event_in_tx(
            &mut tx,
            organization_id,
            owner_user_id,
            "file.scan.blocked",
            "file_object",
            Some(file.id),
            serde_json::json!({
                "fileId": file.id,
                "status": "quarantined",
                "scanStatus": "blocked"
            }),
        )
        .await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(Some(file))
    }

    pub async fn list_files(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        limit: i64,
    ) -> AroResult<Vec<FileObject>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, organization_id, owner_user_id, original_name, mime_type, size_bytes,
                   sha256, status, scan_status, created_at, updated_at
            FROM file_objects
            WHERE organization_id = $1
              AND owner_user_id = $2
              AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(limit.clamp(1, 200))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(map_file_object_row).collect()
    }

    pub async fn get_file(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        file_id: Uuid,
    ) -> AroResult<FileObject> {
        Ok(self
            .get_stored_file(user_id, organization_id, file_id)
            .await?
            .file)
    }

    pub async fn get_stored_file(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        file_id: Uuid,
    ) -> AroResult<StoredFileObject> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT id, organization_id, owner_user_id, original_name, mime_type, size_bytes,
                   sha256, status, scan_status, created_at, updated_at,
                   storage_backend, bucket, object_key
            FROM file_objects
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(file_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        let Some(row) = row else {
            return Err(AroError::Memory("file not found".to_string()));
        };
        map_stored_file_object_row(row)
    }

    pub async fn delete_file(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        file_id: Uuid,
    ) -> AroResult<StoredFileObject> {
        self.ensure_org_access(user_id, organization_id).await?;
        let stored = self
            .get_stored_file(user_id, organization_id, file_id)
            .await?;
        let deleted = enum_to_string(&FileStatus::Deleted)?;
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE file_objects
            SET status = $4, deleted_at = now(), updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(file_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(&deleted)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            "UPDATE file_links SET deleted_at = now() WHERE organization_id = $1 AND file_id = $2 AND deleted_at IS NULL",
        )
        .bind(organization_id)
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;

        self.emit_domain_event(
            organization_id,
            user_id,
            "file.deleted",
            "file_object",
            Some(file_id),
            serde_json::json!({
                "fileId": file_id,
                "status": "deleted"
            }),
        )
        .await?;

        Ok(stored)
    }

    async fn link_message_attachments(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context: TenantContext,
        conversation_id: Uuid,
        message_id: Uuid,
        attachments: &[MessageAttachment],
    ) -> AroResult<()> {
        let user_id = context.actor_id();
        let organization_id = context.organization_id();
        let file_ids: Vec<Uuid> = attachments
            .iter()
            .filter(|attachment| attachment.mode == AttachmentMode::CloudObject)
            .filter_map(|attachment| attachment.file_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        if file_ids.is_empty() {
            return Ok(());
        }
        // Vérification d'accès groupée (était : 1 SELECT par pièce jointe).
        let available = enum_to_string(&FileStatus::Available)?;
        let found: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT id
            FROM file_objects
            WHERE id = ANY($1)
              AND organization_id = $2
              AND owner_user_id = $3
              AND status = $4
              AND deleted_at IS NULL
            "#,
        )
        .bind(&file_ids)
        .bind(organization_id)
        .bind(user_id)
        .bind(&available)
        .fetch_all(&mut **tx)
        .await
        .map_err(map_sqlx)?;
        if found.len() != file_ids.len() {
            return Err(AroError::Security(
                "attachment file is unavailable or belongs to another principal".to_string(),
            ));
        }
        // Insertion groupée via unnest (était : 1 INSERT par pièce jointe).
        sqlx::query(
            r#"
            INSERT INTO file_links
              (organization_id, file_id, conversation_id, message_id, link_kind)
            SELECT $1, unnest($2::uuid[]), $3, $4, 'message-attachment'
            "#,
        )
        .bind(organization_id)
        .bind(&file_ids)
        .bind(conversation_id)
        .bind(message_id)
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn hydrate_message_attachments(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context: TenantContext,
        messages: &mut [ChatMessage],
    ) -> AroResult<()> {
        let message_ids = messages
            .iter()
            .map(|message| message.id)
            .collect::<Vec<_>>();
        if message_ids.is_empty() {
            return Ok(());
        }
        let rows = sqlx::query(
            r#"
            SELECT fl.message_id,
                   fo.id AS file_id,
                   fo.original_name,
                   fo.mime_type,
                   fo.size_bytes
            FROM file_links fl
            JOIN file_objects fo
              ON fo.organization_id = fl.organization_id
             AND fo.id = fl.file_id
            WHERE fl.organization_id = $1
              AND fo.owner_user_id = $2
              AND fl.message_id = ANY($3)
              AND fl.deleted_at IS NULL
              AND fo.deleted_at IS NULL
            ORDER BY fl.created_at ASC
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&message_ids)
        .fetch_all(&mut **tx)
        .await
        .map_err(map_sqlx)?;

        let mut grouped: BTreeMap<Uuid, Vec<MessageAttachment>> = BTreeMap::new();
        for row in rows {
            let message_id: Uuid = row.get("message_id");
            grouped
                .entry(message_id)
                .or_default()
                .push(MessageAttachment {
                    file_id: Some(row.get("file_id")),
                    mode: AttachmentMode::CloudObject,
                    display_name: row.get("original_name"),
                    size_bytes: row.get("size_bytes"),
                    mime_type: row.get("mime_type"),
                });
        }
        for message in messages {
            if let Some(attachments) = grouped.remove(&message.id) {
                message.attachments = attachments;
            }
        }
        Ok(())
    }

    pub async fn upsert_agent_permission_profile(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        profile: &PermissionProfile,
    ) -> AroResult<PermissionProfile> {
        self.ensure_org_access(user_id, organization_id).await?;
        let command_approval = enum_to_string(&profile.command_approval)?;
        let row = sqlx::query(
            r#"
            INSERT INTO agent_permission_profiles
              (id, organization_id, owner_user_id, name, trusted_roots, allowed_domains,
               allow_read, allow_write, allow_shell, allow_network, command_approval,
               redact_secrets, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (id)
            DO UPDATE SET
              name = excluded.name,
              trusted_roots = excluded.trusted_roots,
              allowed_domains = excluded.allowed_domains,
              allow_read = excluded.allow_read,
              allow_write = excluded.allow_write,
              allow_shell = excluded.allow_shell,
              allow_network = excluded.allow_network,
              command_approval = excluded.command_approval,
              redact_secrets = excluded.redact_secrets,
              updated_at = excluded.updated_at
            WHERE agent_permission_profiles.organization_id = excluded.organization_id
              AND agent_permission_profiles.owner_user_id = excluded.owner_user_id
            RETURNING id, name, trusted_roots, allowed_domains, allow_read, allow_write,
                      allow_shell, allow_network, command_approval, redact_secrets,
                      created_at, updated_at
            "#,
        )
        .bind(profile.id)
        .bind(organization_id)
        .bind(user_id)
        .bind(&profile.name)
        .bind(&profile.trusted_roots)
        .bind(&profile.allowed_domains)
        .bind(profile.allow_read)
        .bind(profile.allow_write)
        .bind(profile.allow_shell)
        .bind(profile.allow_network)
        .bind(command_approval)
        .bind(profile.redact_secrets)
        .bind(profile.created_at)
        .bind(profile.updated_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_permission_profile_row)
            .transpose()?
            .ok_or_else(|| {
                AroError::Security("permission profile belongs to another organization".to_string())
            })
    }

    pub async fn list_agent_permission_profiles(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Vec<PermissionProfile>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, name, trusted_roots, allowed_domains, allow_read, allow_write,
                   allow_shell, allow_network, command_approval, redact_secrets,
                   created_at, updated_at
            FROM agent_permission_profiles
            WHERE organization_id = $1
              AND owner_user_id = $2
              AND deleted_at IS NULL
            ORDER BY updated_at DESC
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_permission_profile_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn upsert_agent_lane(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        lane: &AgentLane,
    ) -> AroResult<AgentLane> {
        self.ensure_org_access(user_id, organization_id).await?;
        if let Some(conversation_id) = lane.conversation_id {
            self.ensure_conversation_owner(user_id, organization_id, conversation_id)
                .await?;
        }
        let status = enum_to_string(&lane.status)?;
        let priority = enum_to_string(&lane.priority)?;
        let row = sqlx::query(
            r#"
            INSERT INTO agent_lanes
              (id, organization_id, owner_user_id, conversation_id, title, status,
               priority, max_concurrent_runs, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id)
            DO UPDATE SET
              conversation_id = excluded.conversation_id,
              title = excluded.title,
              status = excluded.status,
              priority = excluded.priority,
              max_concurrent_runs = excluded.max_concurrent_runs,
              updated_at = excluded.updated_at
            WHERE agent_lanes.organization_id = excluded.organization_id
              AND agent_lanes.owner_user_id = excluded.owner_user_id
            RETURNING id, conversation_id, title, status, priority, max_concurrent_runs,
                      created_at, updated_at
            "#,
        )
        .bind(lane.id)
        .bind(organization_id)
        .bind(user_id)
        .bind(lane.conversation_id)
        .bind(&lane.title)
        .bind(status)
        .bind(priority)
        .bind(lane.max_concurrent_runs as i32)
        .bind(lane.created_at)
        .bind(lane.updated_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_agent_lane_row).transpose()?.ok_or_else(|| {
            AroError::Security("agent lane belongs to another organization".to_string())
        })
    }

    pub async fn ensure_agent_lane(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        conversation_id: Option<Uuid>,
        title: impl Into<String>,
    ) -> AroResult<AgentLane> {
        if let Some(conversation_id) = conversation_id {
            if let Some(lane) = self
                .agent_lane_for_conversation(user_id, organization_id, conversation_id)
                .await?
            {
                return Ok(lane);
            }
        }
        let lane = AgentLane::new(conversation_id, title);
        self.upsert_agent_lane(user_id, organization_id, &lane)
            .await
    }

    pub async fn agent_lane_for_conversation(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        conversation_id: Uuid,
    ) -> AroResult<Option<AgentLane>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT id, conversation_id, title, status, priority, max_concurrent_runs,
                   created_at, updated_at
            FROM agent_lanes
            WHERE organization_id = $1
              AND conversation_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(conversation_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_agent_lane_row).transpose()
    }

    pub async fn get_agent_lane(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        lane_id: Uuid,
    ) -> AroResult<Option<AgentLane>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT id, conversation_id, title, status, priority, max_concurrent_runs,
                   created_at, updated_at
            FROM agent_lanes
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(lane_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_agent_lane_row).transpose()
    }

    pub async fn list_agent_lanes(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Vec<AgentLane>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, title, status, priority, max_concurrent_runs,
                   created_at, updated_at
            FROM agent_lanes
            WHERE organization_id = $1
              AND owner_user_id = $2
              AND deleted_at IS NULL
            ORDER BY updated_at DESC
            LIMIT 100
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_agent_lane_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn list_agent_lane_views(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Vec<AgentLaneView>> {
        let lanes = self.list_agent_lanes(user_id, organization_id).await?;
        if lanes.is_empty() {
            return Ok(Vec::new());
        }
        let lane_ids: Vec<Uuid> = lanes.iter().map(|lane| lane.id).collect();
        let queued_status = enum_to_string(&AgentRunStatus::Queued)?;
        let running_status = enum_to_string(&AgentRunStatus::Running)?;
        let waiting_status = enum_to_string(&AgentRunStatus::Waiting)?;
        let paused_status = enum_to_string(&AgentRunStatus::Paused)?;

        // Compteurs de toutes les lanes en UNE requête (était : 4 par lane).
        let count_rows = sqlx::query(
            r#"
            SELECT lane_id, status, COUNT(*) AS run_count
            FROM agent_runs
            WHERE organization_id = $1
              AND owner_user_id = $2
              AND lane_id = ANY($3)
              AND deleted_at IS NULL
            GROUP BY lane_id, status
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(&lane_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        let mut counts: std::collections::HashMap<(Uuid, String), u32> =
            std::collections::HashMap::new();
        for row in count_rows {
            let lane_id: Uuid = row.get("lane_id");
            let status: String = row.get("status");
            let count: i64 = row.get("run_count");
            counts.insert((lane_id, status), count.max(0) as u32);
        }
        let count_for = |lane_id: Uuid, status: &str| -> u32 {
            counts
                .get(&(lane_id, status.to_string()))
                .copied()
                .unwrap_or(0)
        };

        // 4 derniers runs par lane en UNE requête (fenêtre ROW_NUMBER).
        let run_rows = sqlx::query(
            r#"
            SELECT id, lane_id, conversation_id, goal, mode, status, priority,
                   model_provider_id, model_id, autonomy_profile_id,
                   checkpoint_summary, last_error, created_at, updated_at,
                   heartbeat_at, completed_at
            FROM (
                SELECT r.*,
                       ROW_NUMBER() OVER (PARTITION BY r.lane_id ORDER BY r.updated_at DESC) AS rn
                FROM agent_runs r
                JOIN agent_lanes l
                  ON l.id = r.lane_id
                 AND l.organization_id = r.organization_id
                WHERE r.organization_id = $1
                  AND r.owner_user_id = $2
                  AND l.owner_user_id = $2
                  AND r.lane_id = ANY($3)
                  AND r.deleted_at IS NULL
            ) ranked
            WHERE rn <= 4
            ORDER BY lane_id, updated_at DESC
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(&lane_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        let mut latest_by_lane: std::collections::HashMap<Uuid, Vec<AgentRun>> =
            std::collections::HashMap::new();
        for row in run_rows {
            let run = map_agent_run_row(row)?;
            if let Some(lane_id) = run.lane_id {
                latest_by_lane.entry(lane_id).or_default().push(run);
            }
        }

        let views = lanes
            .into_iter()
            .map(|lane| {
                let waiting_count =
                    count_for(lane.id, &waiting_status) + count_for(lane.id, &paused_status);
                AgentLaneView {
                    queued_count: count_for(lane.id, &queued_status),
                    running_count: count_for(lane.id, &running_status),
                    waiting_count,
                    latest_runs: latest_by_lane.remove(&lane.id).unwrap_or_default(),
                    lane,
                }
            })
            .collect();
        Ok(views)
    }

    pub async fn update_agent_lane_status(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        lane_id: Uuid,
        status: AgentLaneStatus,
    ) -> AroResult<AgentLane> {
        self.ensure_org_access(user_id, organization_id).await?;
        let status = enum_to_string(&status)?;
        let row = sqlx::query(
            r#"
            UPDATE agent_lanes
            SET status = $4, updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            RETURNING id, conversation_id, title, status, priority, max_concurrent_runs,
                      created_at, updated_at
            "#,
        )
        .bind(lane_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(status)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_agent_lane_row)
            .transpose()?
            .ok_or_else(|| AroError::Memory("agent lane not found".to_string()))
    }

    pub async fn update_agent_lane_priority(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        lane_id: Uuid,
        priority: AgentRunPriority,
    ) -> AroResult<AgentLane> {
        self.ensure_org_access(user_id, organization_id).await?;
        let priority = enum_to_string(&priority)?;
        let row = sqlx::query(
            r#"
            UPDATE agent_lanes
            SET priority = $4, updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            RETURNING id, conversation_id, title, status, priority, max_concurrent_runs,
                      created_at, updated_at
            "#,
        )
        .bind(lane_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(priority)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_agent_lane_row)
            .transpose()?
            .ok_or_else(|| AroError::Memory("agent lane not found".to_string()))
    }

    pub async fn upsert_agent_run(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        run: &AgentRun,
    ) -> AroResult<AgentRun> {
        self.ensure_org_access(user_id, organization_id).await?;
        if let Some(conversation_id) = run.conversation_id {
            self.ensure_conversation_owner(user_id, organization_id, conversation_id)
                .await?;
        }
        if let Some(lane_id) = run.lane_id {
            if self
                .get_agent_lane(user_id, organization_id, lane_id)
                .await?
                .is_none()
            {
                return Err(AroError::Security("agent lane access denied".to_string()));
            }
        }
        let mode = enum_to_string(&run.mode)?;
        let status = enum_to_string(&run.status)?;
        let priority = enum_to_string(&run.priority)?;
        let row = sqlx::query(
            r#"
            INSERT INTO agent_runs
              (id, organization_id, owner_user_id, lane_id, conversation_id, goal, mode, status,
               priority, model_provider_id, model_id, autonomy_profile_id, checkpoint_summary,
               last_error, created_at, updated_at, heartbeat_at, completed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            ON CONFLICT (id)
            DO UPDATE SET
              lane_id = excluded.lane_id,
              conversation_id = excluded.conversation_id,
              goal = excluded.goal,
              mode = excluded.mode,
              status = excluded.status,
              priority = excluded.priority,
              model_provider_id = excluded.model_provider_id,
              model_id = excluded.model_id,
              autonomy_profile_id = excluded.autonomy_profile_id,
              checkpoint_summary = excluded.checkpoint_summary,
              last_error = excluded.last_error,
              updated_at = excluded.updated_at,
              heartbeat_at = excluded.heartbeat_at,
              completed_at = excluded.completed_at
            WHERE agent_runs.organization_id = excluded.organization_id
              AND agent_runs.owner_user_id = excluded.owner_user_id
            RETURNING id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id,
                      autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at,
                      heartbeat_at, completed_at
            "#,
        )
        .bind(run.id)
        .bind(organization_id)
        .bind(user_id)
        .bind(run.lane_id)
        .bind(run.conversation_id)
        .bind(&run.goal)
        .bind(mode)
        .bind(status)
        .bind(priority)
        .bind(&run.model_provider_id)
        .bind(&run.model_id)
        .bind(run.autonomy_profile_id)
        .bind(&run.checkpoint_summary)
        .bind(&run.last_error)
        .bind(run.created_at)
        .bind(run.updated_at)
        .bind(run.heartbeat_at)
        .bind(run.completed_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_agent_run_row).transpose()?.ok_or_else(|| {
            AroError::Security("agent run belongs to another organization".to_string())
        })
    }

    pub async fn list_agent_runs(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Vec<AgentRun>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id,
                   autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at,
                   heartbeat_at, completed_at
            FROM agent_runs
            WHERE organization_id = $1
              AND owner_user_id = $2
              AND deleted_at IS NULL
            ORDER BY updated_at DESC
            LIMIT 100
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_agent_run_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn list_agent_runs_for_lane(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        lane_id: Uuid,
        limit: i64,
    ) -> AroResult<Vec<AgentRun>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT r.id, r.lane_id, r.conversation_id, r.goal, r.mode, r.status, r.priority, r.model_provider_id, r.model_id,
                   r.autonomy_profile_id, r.checkpoint_summary, r.last_error, r.created_at, r.updated_at,
                   r.heartbeat_at, r.completed_at
            FROM agent_runs r
            JOIN agent_lanes l
              ON l.id = r.lane_id
             AND l.organization_id = r.organization_id
            WHERE r.organization_id = $1
              AND r.lane_id = $2
              AND r.owner_user_id = $3
              AND l.owner_user_id = $3
              AND r.deleted_at IS NULL
            ORDER BY r.updated_at DESC
            LIMIT $4
            "#,
        )
        .bind(organization_id)
        .bind(lane_id)
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_agent_run_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn count_agent_runs_for_lane(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        lane_id: Uuid,
        status: AgentRunStatus,
    ) -> AroResult<u32> {
        self.ensure_org_access(user_id, organization_id).await?;
        let status = enum_to_string(&status)?;
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM agent_runs
            WHERE organization_id = $1
              AND lane_id = $2
              AND status = $3
              AND owner_user_id = $4
              AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(lane_id)
        .bind(status)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(count.max(0) as u32)
    }

    pub async fn get_agent_run(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        run_id: Uuid,
    ) -> AroResult<Option<AgentRun>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id,
                   autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at,
                   heartbeat_at, completed_at
            FROM agent_runs
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(run_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_agent_run_row).transpose()
    }

    pub async fn update_agent_run_status(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        run_id: Uuid,
        status: AgentRunStatus,
        last_error: Option<String>,
    ) -> AroResult<AgentRun> {
        self.ensure_org_access(user_id, organization_id).await?;
        let status_text = enum_to_string(&status)?;
        let completed_at = if matches!(
            status,
            AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Cancelled
        ) {
            Some(Utc::now())
        } else {
            None
        };
        let row = sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = $4,
                last_error = $5,
                updated_at = now(),
                heartbeat_at = now(),
                completed_at = COALESCE($6, completed_at)
            WHERE id = $1
              AND organization_id = $2
              AND owner_user_id = $3
              AND deleted_at IS NULL
            RETURNING id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id,
                      autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at,
                      heartbeat_at, completed_at
            "#,
        )
        .bind(run_id)
        .bind(organization_id)
        .bind(user_id)
        .bind(status_text)
        .bind(last_error)
        .bind(completed_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(map_agent_run_row)
            .transpose()?
            .ok_or_else(|| AroError::Memory("agent run not found".to_string()))
    }

    pub async fn mark_stale_agent_runs_paused(
        &self,
        organization_id: Uuid,
        older_than: DateTime<Utc>,
    ) -> AroResult<u64> {
        let paused = enum_to_string(&AgentRunStatus::Paused)?;
        let running = enum_to_string(&AgentRunStatus::Running)?;
        let result = sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = $3,
                updated_at = now(),
                checkpoint_summary = COALESCE(checkpoint_summary, 'Paused after stale heartbeat.')
            WHERE organization_id = $1
              AND status = $2
              AND heartbeat_at < $4
              AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(running)
        .bind(paused)
        .bind(older_than)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(result.rows_affected())
    }

    pub async fn add_agent_step(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        step: &AgentStep,
    ) -> AroResult<AgentStep> {
        self.ensure_org_access(user_id, organization_id).await?;
        self.ensure_agent_run_owner(user_id, organization_id, step.run_id)
            .await?;
        let kind = enum_to_string(&step.kind)?;
        let status = enum_to_string(&step.status)?;
        let row = sqlx::query(
            r#"
            INSERT INTO agent_steps
              (id, organization_id, run_id, sequence, kind, status, title, input, output,
               error, started_at, finished_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (run_id, sequence)
            DO UPDATE SET
              id = excluded.id,
              kind = excluded.kind,
              status = excluded.status,
              title = excluded.title,
              input = excluded.input,
              output = excluded.output,
              error = excluded.error,
              finished_at = excluded.finished_at
            RETURNING id, run_id, sequence, kind, status, title, input, output,
                      error, started_at, finished_at
            "#,
        )
        .bind(step.id)
        .bind(organization_id)
        .bind(step.run_id)
        .bind(step.sequence)
        .bind(kind)
        .bind(status)
        .bind(&step.title)
        .bind(&step.input)
        .bind(&step.output)
        .bind(&step.error)
        .bind(step.started_at)
        .bind(step.finished_at)
        .fetch_one(&self.pool)
        .await
        .map(map_agent_step_row)
        .map_err(map_sqlx)??;
        Ok(row)
    }

    pub async fn list_agent_steps(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        run_id: Uuid,
    ) -> AroResult<Vec<AgentStep>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT s.id, s.run_id, s.sequence, s.kind, s.status, s.title, s.input, s.output,
                   s.error, s.started_at, s.finished_at
            FROM agent_steps s
            JOIN agent_runs r ON r.id = s.run_id AND r.organization_id = s.organization_id
            WHERE s.run_id = $1
              AND s.organization_id = $2
              AND r.owner_user_id = $3
              AND r.deleted_at IS NULL
            ORDER BY s.sequence ASC
            "#,
        )
        .bind(run_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_agent_step_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn add_agent_artifact(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        artifact: &AgentArtifact,
    ) -> AroResult<AgentArtifact> {
        self.ensure_org_access(user_id, organization_id).await?;
        self.ensure_agent_run_owner(user_id, organization_id, artifact.run_id)
            .await?;
        let row = sqlx::query(
            r#"
            INSERT INTO agent_artifacts
              (id, organization_id, run_id, kind, title, uri, content, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (id)
            DO UPDATE SET
              title = excluded.title,
              uri = excluded.uri,
              content = excluded.content,
              metadata = excluded.metadata
            RETURNING id, run_id, kind, title, uri, content, metadata, created_at
            "#,
        )
        .bind(artifact.id)
        .bind(organization_id)
        .bind(artifact.run_id)
        .bind(&artifact.kind)
        .bind(&artifact.title)
        .bind(&artifact.uri)
        .bind(&artifact.content)
        .bind(&artifact.metadata)
        .bind(artifact.created_at)
        .fetch_one(&self.pool)
        .await
        .map(map_agent_artifact_row)
        .map_err(map_sqlx);
        row?
    }

    pub async fn list_agent_artifacts(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        run_id: Uuid,
    ) -> AroResult<Vec<AgentArtifact>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT a.id, a.run_id, a.kind, a.title, a.uri, a.content, a.metadata, a.created_at
            FROM agent_artifacts a
            JOIN agent_runs r ON r.id = a.run_id AND r.organization_id = a.organization_id
            WHERE a.run_id = $1
              AND a.organization_id = $2
              AND r.owner_user_id = $3
              AND r.deleted_at IS NULL
            ORDER BY a.created_at ASC
            "#,
        )
        .bind(run_id)
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_agent_artifact_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn add_agent_context_item(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        item: &AgentContextItem,
    ) -> AroResult<AgentContextItem> {
        self.ensure_org_access(user_id, organization_id).await?;
        match (item.run_id, item.conversation_id) {
            (Some(run_id), _) => {
                self.ensure_agent_run_owner(user_id, organization_id, run_id)
                    .await?
            }
            (None, Some(conversation_id)) => {
                self.ensure_conversation_owner(user_id, organization_id, conversation_id)
                    .await?
            }
            (None, None) => {
                return Err(AroError::Security(
                    "agent context must be attached to an owned run or conversation".to_string(),
                ))
            }
        }
        let row = sqlx::query(
            r#"
            INSERT INTO agent_context_items
              (id, organization_id, run_id, conversation_id, kind, title, content, uri, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id)
            DO UPDATE SET
              title = excluded.title,
              content = excluded.content,
              uri = excluded.uri,
              metadata = excluded.metadata
            RETURNING id, run_id, conversation_id, kind, title, content, uri, metadata, created_at
            "#,
        )
        .bind(item.id)
        .bind(organization_id)
        .bind(item.run_id)
        .bind(item.conversation_id)
        .bind(&item.kind)
        .bind(&item.title)
        .bind(&item.content)
        .bind(&item.uri)
        .bind(&item.metadata)
        .bind(item.created_at)
        .fetch_one(&self.pool)
        .await
        .map(map_agent_context_item_row)
        .map_err(map_sqlx);
        row?
    }

    pub async fn search_agent_context(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        query: &str,
        limit: i64,
    ) -> AroResult<Vec<AgentContextItem>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let limit = limit.clamp(1, 50);
        let rows = sqlx::query(
            r#"
            SELECT c.id, c.run_id, c.conversation_id, c.kind, c.title, c.content, c.uri, c.metadata, c.created_at
            FROM agent_context_items c
            JOIN agent_runs r
              ON r.id = c.run_id
             AND r.organization_id = c.organization_id
            WHERE c.organization_id = $1
              AND r.owner_user_id = $2
              AND r.deleted_at IS NULL
              AND c.search_document @@ plainto_tsquery('simple', $3)
            ORDER BY ts_rank(c.search_document, plainto_tsquery('simple', $3)) DESC, c.created_at DESC
            LIMIT $4
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter()
            .map(map_agent_context_item_row)
            .collect::<AroResult<Vec<_>>>()
    }

    pub async fn agent_run_view(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        run_id: Uuid,
    ) -> AroResult<Option<AgentRunView>> {
        let Some(run) = self.get_agent_run(user_id, organization_id, run_id).await? else {
            return Ok(None);
        };
        let steps = self
            .list_agent_steps(user_id, organization_id, run_id)
            .await?;
        let artifacts = self
            .list_agent_artifacts(user_id, organization_id, run_id)
            .await?;
        let context_pack = steps
            .iter()
            .rev()
            .find(|step| matches!(step.kind, aro_core::AgentStepKind::ContextBuilt))
            .and_then(|step| serde_json::from_value::<ContextPack>(step.output.clone()).ok());
        Ok(Some(AgentRunView {
            run,
            steps,
            artifacts,
            context_pack,
        }))
    }

    pub async fn agent_events(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        run_id: Uuid,
    ) -> AroResult<Vec<AgentEvent>> {
        let steps = self
            .list_agent_steps(user_id, organization_id, run_id)
            .await?;
        Ok(steps
            .into_iter()
            .map(|step| AgentEvent {
                run_id,
                sequence: Some(step.sequence),
                event_type: enum_to_string(&step.kind).unwrap_or_else(|_| "step".to_string()),
                data: serde_json::to_value(&step).unwrap_or(Value::Null),
                created_at: step.finished_at.unwrap_or(step.started_at),
            })
            .collect())
    }

    pub async fn get_collection(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        collection: PersistedCollection,
    ) -> AroResult<Value> {
        self.ensure_org_access(user_id, organization_id).await?;
        let table = collection.table_name();
        let order_column = format!("t.{}", collection.order_column());
        let public_expr = collection.public_json_expr("t");
        let owner_clause = collection.owner_clause("t", 2);
        let sql = format!(
            "SELECT COALESCE(jsonb_agg({public_expr} ORDER BY {order_column} DESC), '[]'::jsonb) AS data \
             FROM (SELECT * FROM {table} t WHERE t.organization_id = $1 {owner_clause} {deleted_clause}) t",
            owner_clause = owner_clause,
            deleted_clause = collection.deleted_clause("t")
        );
        let mut query = sqlx::query(&sql).bind(organization_id);
        if collection.is_user_owned() {
            query = query.bind(user_id);
        }
        let row = query.fetch_one(&self.pool).await.map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    pub async fn get_collection_item(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        collection: PersistedCollection,
        item_id: Uuid,
    ) -> AroResult<Option<Value>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let table = collection.table_name();
        let public_expr = collection.public_json_expr("t");
        let owner_clause = collection.owner_clause("t", 3);
        let sql = format!(
            "SELECT {public_expr} AS data FROM {table} t WHERE t.id = $1 AND t.organization_id = $2 {owner_clause} {deleted_clause}",
            owner_clause = owner_clause,
            deleted_clause = collection.deleted_clause("t")
        );
        let mut query = sqlx::query(&sql).bind(item_id).bind(organization_id);
        if collection.is_user_owned() {
            query = query.bind(user_id);
        }
        query
            .fetch_optional(&self.pool)
            .await
            .map(|row| row.map(|row| row.get("data")))
            .map_err(map_sqlx)
    }

    pub async fn delete_collection_item(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        collection: PersistedCollection,
        item_id: Uuid,
    ) -> AroResult<()> {
        self.ensure_collection_write_access(user_id, organization_id, collection)
            .await?;
        let table = collection.table_name();
        let owner_clause = collection.owner_clause("", 3);
        if collection.has_soft_delete() {
            let sql = format!(
                "UPDATE {table} SET deleted_at = now(), updated_at = now() WHERE id = $1 AND organization_id = $2 {owner_clause}"
            );
            let mut query = sqlx::query(&sql).bind(item_id).bind(organization_id);
            if collection.is_user_owned() {
                query = query.bind(user_id);
            }
            let result = query.execute(&self.pool).await.map_err(map_sqlx)?;
            if result.rows_affected() == 0 {
                return Err(AroError::Memory(format!(
                    "{} item not found",
                    collection.slug()
                )));
            }
        } else {
            let sql = format!(
                "DELETE FROM {table} WHERE id = $1 AND organization_id = $2 {owner_clause}"
            );
            let mut query = sqlx::query(&sql).bind(item_id).bind(organization_id);
            if collection.is_user_owned() {
                query = query.bind(user_id);
            }
            let result = query.execute(&self.pool).await.map_err(map_sqlx)?;
            if result.rows_affected() == 0 {
                return Err(AroError::Memory(format!(
                    "{} item not found",
                    collection.slug()
                )));
            }
        }
        let action = format!("{}.deleted", collection.slug());
        self.emit_domain_event(
            organization_id,
            user_id,
            &action,
            collection.slug(),
            Some(item_id),
            serde_json::json!({
                "collection": collection.slug(),
                "id": item_id,
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn create_collection_item(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        collection: PersistedCollection,
        payload: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<Value> {
        self.ensure_collection_write_access(user_id, organization_id, collection)
            .await?;
        let item = match collection {
            PersistedCollection::Memories => {
                self.create_memory(organization_id, user_id, payload).await
            }
            PersistedCollection::Personalities => {
                self.create_personality(organization_id, user_id, payload)
                    .await
            }
            PersistedCollection::SystemPrompts => {
                self.create_system_prompt(organization_id, user_id, payload)
                    .await
            }
            PersistedCollection::VoiceProfiles => {
                self.create_voice_profile(organization_id, user_id, payload)
                    .await
            }
            PersistedCollection::SkillGroups => {
                self.create_skill_group(organization_id, payload).await
            }
            PersistedCollection::Skills => self.create_skill(organization_id, payload).await,
            PersistedCollection::PluginConnections => {
                self.create_plugin_connection(organization_id, payload, secrets_key)
                    .await
            }
            PersistedCollection::McpServers => {
                self.create_mcp_server(organization_id, payload, secrets_key)
                    .await
            }
            PersistedCollection::Hooks => {
                self.create_hook(organization_id, payload, secrets_key)
                    .await
            }
            PersistedCollection::ScheduledTasks => {
                self.create_scheduled_task(organization_id, payload).await
            }
            PersistedCollection::Teams => self.create_team(user_id, organization_id, payload).await,
            PersistedCollection::UsageEvents => {
                self.create_usage_event(organization_id, user_id, payload)
                    .await
            }
            PersistedCollection::CustomAgentDefinitions => {
                self.create_custom_agent_definition(organization_id, user_id, payload)
                    .await
            }
            PersistedCollection::CustomModelDefinitions => {
                self.create_custom_model_definition(organization_id, user_id, payload)
                    .await
            }
            PersistedCollection::Plans => self.create_plan(organization_id, user_id, payload).await,
        }?;
        let action = format!("{}.created", collection.slug());
        self.emit_domain_event(
            organization_id,
            user_id,
            &action,
            collection.slug(),
            json_uuid_field(&item, "id"),
            collection.safe_event_payload(&item),
        )
        .await?;
        Ok(item)
    }

    pub async fn update_collection_item(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        collection: PersistedCollection,
        item_id: Uuid,
        payload: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<Value> {
        self.ensure_collection_write_access(user_id, organization_id, collection)
            .await?;
        let item = match collection {
            PersistedCollection::Memories => {
                self.update_memory(organization_id, user_id, item_id, payload)
                    .await
            }
            PersistedCollection::Personalities => {
                self.update_personality(organization_id, user_id, item_id, payload)
                    .await
            }
            PersistedCollection::SystemPrompts => {
                self.update_system_prompt(organization_id, user_id, item_id, payload)
                    .await
            }
            PersistedCollection::VoiceProfiles => {
                self.update_voice_profile(organization_id, user_id, item_id, payload)
                    .await
            }
            PersistedCollection::SkillGroups => {
                self.update_skill_group(organization_id, item_id, payload)
                    .await
            }
            PersistedCollection::CustomAgentDefinitions => {
                self.update_custom_agent_definition(organization_id, user_id, item_id, payload)
                    .await
            }
            PersistedCollection::CustomModelDefinitions => {
                self.update_custom_model_definition(organization_id, user_id, item_id, payload)
                    .await
            }
            PersistedCollection::Skills => {
                self.update_skill(organization_id, item_id, payload).await
            }
            PersistedCollection::PluginConnections => {
                self.update_plugin_connection(organization_id, item_id, payload, secrets_key)
                    .await
            }
            PersistedCollection::McpServers => {
                self.update_mcp_server(organization_id, item_id, payload, secrets_key)
                    .await
            }
            PersistedCollection::Hooks => {
                self.update_hook(organization_id, item_id, payload, secrets_key)
                    .await
            }
            PersistedCollection::ScheduledTasks => {
                self.update_scheduled_task(organization_id, item_id, payload)
                    .await
            }
            PersistedCollection::Teams => {
                self.update_team(user_id, organization_id, item_id, payload)
                    .await
            }
            PersistedCollection::Plans => {
                self.update_plan(organization_id, user_id, item_id, payload)
                    .await
            }
            unsupported => Err(AroError::Configuration(format!(
                "collection '{}' does not support generic updates yet",
                unsupported.slug()
            ))),
        }?;
        let action = format!("{}.updated", collection.slug());
        self.emit_domain_event(
            organization_id,
            user_id,
            &action,
            collection.slug(),
            Some(item_id),
            collection.safe_event_payload(&item),
        )
        .await?;
        Ok(item)
    }

    async fn ensure_collection_write_access(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        collection: PersistedCollection,
    ) -> AroResult<()> {
        if collection.is_admin_managed() {
            self.ensure_org_admin(user_id, organization_id).await?;
        } else if collection.is_manager_managed() {
            self.ensure_org_manager(user_id, organization_id).await?;
        } else {
            self.ensure_org_access(user_id, organization_id).await?;
        }
        Ok(())
    }

    async fn ensure_memory_sources_belong_to_owner(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        source_conversation_id: Option<Uuid>,
        source_message_ids: &[Uuid],
    ) -> AroResult<()> {
        if let Some(conversation_id) = source_conversation_id {
            let exists: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS (
                  SELECT 1
                  FROM conversations
                  WHERE organization_id = $1
                    AND owner_user_id = $2
                    AND id = $3
                    AND deleted_at IS NULL
                )
                "#,
            )
            .bind(organization_id)
            .bind(owner_user_id)
            .bind(conversation_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx)?;
            if !exists {
                return Err(AroError::Security(
                    "memory source conversation is not accessible".to_string(),
                ));
            }
        }

        if source_message_ids.is_empty() {
            return Ok(());
        }

        let matching: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(DISTINCT m.id)::BIGINT
            FROM messages m
            JOIN conversations c
              ON c.organization_id = m.organization_id
             AND c.id = m.conversation_id
            WHERE m.organization_id = $1
              AND c.owner_user_id = $2
              AND m.id = ANY($3)
              AND m.deleted_at IS NULL
              AND c.deleted_at IS NULL
              AND ($4::uuid IS NULL OR m.conversation_id = $4)
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(source_message_ids)
        .bind(source_conversation_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        if matching != source_message_ids.len() as i64 {
            return Err(AroError::Security(
                "memory source messages are not accessible".to_string(),
            ));
        }
        Ok(())
    }

    async fn create_memory(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let source_conversation_id = optional_uuid(
            &payload,
            &["sourceConversationId", "source_conversation_id"],
        )?;
        let source_message_ids =
            optional_uuid_vec(&payload, &["sourceMessageIds", "source_message_ids"])
                .map(dedupe_uuids)
                .unwrap_or_default();
        self.ensure_memory_sources_belong_to_owner(
            organization_id,
            owner_user_id,
            source_conversation_id,
            &source_message_ids,
        )
        .await?;
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO memories
                (organization_id, owner_user_id, client_id, content, category, scope, status,
                 source_conversation_id, source_message_ids, pinned, salience)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["content"])?)
        .bind(optional_string(&payload, &["category"]).unwrap_or_else(|| "personal".to_string()))
        .bind(optional_string(&payload, &["scope"]).unwrap_or_else(|| "user".to_string()))
        .bind(
            optional_string(&payload, &["status"])
                .unwrap_or_else(|| MEMORY_STATUS_APPROVED.to_string()),
        )
        .bind(source_conversation_id)
        .bind(source_message_ids)
        .bind(bool_or(&payload, &["pinned"], false))
        .bind(optional_f32(&payload, &["salience"]).unwrap_or(0.5))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_memory(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let source_conversation_id = optional_uuid(
            &payload,
            &["sourceConversationId", "source_conversation_id"],
        )?;
        let source_message_ids =
            optional_uuid_vec(&payload, &["sourceMessageIds", "source_message_ids"])
                .map(dedupe_uuids);
        self.ensure_memory_sources_belong_to_owner(
            organization_id,
            owner_user_id,
            source_conversation_id,
            source_message_ids.as_deref().unwrap_or(&[]),
        )
        .await?;
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE memories
              SET
                client_id = COALESCE($4, client_id),
                content = COALESCE($5, content),
                category = COALESCE($6, category),
                scope = COALESCE($7, scope),
                status = COALESCE($8, status),
                source_conversation_id = COALESCE($9, source_conversation_id),
                source_message_ids = COALESCE($10, source_message_ids),
                pinned = COALESCE($11, pinned),
                salience = COALESCE($12, salience),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["content"]))
        .bind(optional_string(&payload, &["category"]))
        .bind(optional_string(&payload, &["scope"]))
        .bind(optional_string(&payload, &["status"]))
        .bind(source_conversation_id)
        .bind(source_message_ids)
        .bind(optional_bool(&payload, &["pinned"]))
        .bind(optional_f32(&payload, &["salience"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("memory not found".to_string()))
    }

    pub async fn list_memories(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
    ) -> AroResult<Vec<LongTermMemory>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let sql = memory_select_sql(
            "WHERE organization_id = $1 AND owner_user_id = $2 AND deleted_at IS NULL
             ORDER BY pinned DESC, updated_at DESC",
        );
        let rows = sqlx::query(&sql)
            .bind(organization_id)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx)?;
        rows.into_iter().map(map_memory_row).collect()
    }

    pub async fn search_memories(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        query: &str,
        limit: i64,
    ) -> AroResult<Vec<LongTermMemory>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let limit = limit.clamp(1, 50);
        let trimmed = query.trim();
        if trimmed.is_empty() {
            let sql = memory_select_sql(
                "WHERE organization_id = $1
                   AND owner_user_id = $2
                   AND deleted_at IS NULL
                   AND status = 'approved'
                 ORDER BY pinned DESC, salience DESC, updated_at DESC
                 LIMIT $3",
            );
            let rows = sqlx::query(&sql)
                .bind(organization_id)
                .bind(user_id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx)?;
            return rows.into_iter().map(map_memory_row).collect();
        }

        let sql = memory_select_sql(
            "WHERE organization_id = $1
               AND owner_user_id = $2
               AND deleted_at IS NULL
               AND status = 'approved'
               AND (pinned = true OR search_document @@ plainto_tsquery('simple', $3))
             ORDER BY pinned DESC,
                      ts_rank(search_document, plainto_tsquery('simple', $3)) DESC,
                      salience DESC,
                      updated_at DESC
             LIMIT $4",
        );
        let rows = sqlx::query(&sql)
            .bind(organization_id)
            .bind(user_id)
            .bind(trimmed)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx)?;
        rows.into_iter().map(map_memory_row).collect()
    }

    pub async fn touch_memories_used(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        memory_ids: &[Uuid],
    ) -> AroResult<()> {
        self.ensure_org_access(user_id, organization_id).await?;
        if memory_ids.is_empty() {
            return Ok(());
        }
        sqlx::query(
            r#"
            UPDATE memories
            SET last_used_at = now(), updated_at = now()
            WHERE organization_id = $1
              AND owner_user_id = $2
              AND id = ANY($3)
              AND deleted_at IS NULL
              AND status = 'approved'
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(memory_ids)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn create_personality(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO personalities
                (organization_id, owner_user_id, client_id, name, description, prompt, icon, avatar_color, temperature, voice_id, is_default)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(optional_string(&payload, &["description"]))
        .bind(required_string(&payload, &["prompt"])?)
        .bind(optional_string(&payload, &["icon"]))
        .bind(optional_string(&payload, &["avatarColor", "avatar_color"]))
        .bind(optional_f32(&payload, &["temperature"]).unwrap_or(0.7))
        .bind(optional_uuid_if_valid(&payload, &["voiceId", "voice_id"]))
        .bind(bool_or(&payload, &["isDefault", "is_default"], false))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_personality(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE personalities
              SET
                client_id = COALESCE($4, client_id),
                name = COALESCE($5, name),
                description = COALESCE($6, description),
                prompt = COALESCE($7, prompt),
                icon = COALESCE($8, icon),
                avatar_color = COALESCE($9, avatar_color),
                temperature = COALESCE($10, temperature),
                voice_id = COALESCE($11, voice_id),
                is_default = COALESCE($12, is_default),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["description"]))
        .bind(optional_string(&payload, &["prompt"]))
        .bind(optional_string(&payload, &["icon"]))
        .bind(optional_string(&payload, &["avatarColor", "avatar_color"]))
        .bind(optional_f32(&payload, &["temperature"]))
        .bind(optional_uuid_if_valid(&payload, &["voiceId", "voice_id"]))
        .bind(optional_bool(&payload, &["isDefault", "is_default"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("personality not found".to_string()))
    }

    async fn create_system_prompt(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let identity = required_string(&payload, &["identity"])?;
        let rules = string_or(&payload, &["rules"], "");
        let formatting = string_or(&payload, &["formatting"], "");
        let compiled_prompt = optional_string(&payload, &["compiledPrompt", "compiled_prompt"])
            .unwrap_or_else(|| compile_prompt(&identity, &rules, &formatting));
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO system_prompts
                (organization_id, owner_user_id, client_id, mode, identity, rules, formatting, compiled_prompt, enabled)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
              ON CONFLICT (organization_id, owner_user_id, mode)
              DO UPDATE SET
                client_id = excluded.client_id,
                identity = excluded.identity,
                rules = excluded.rules,
                formatting = excluded.formatting,
                compiled_prompt = excluded.compiled_prompt,
                enabled = excluded.enabled,
                updated_at = now(),
                deleted_at = NULL
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["mode"])?)
        .bind(identity)
        .bind(rules)
        .bind(formatting)
        .bind(compiled_prompt)
        .bind(bool_or(&payload, &["enabled"], true))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_system_prompt(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE system_prompts
              SET
                client_id = COALESCE($4, client_id),
                mode = COALESCE($5, mode),
                identity = COALESCE($6, identity),
                rules = COALESCE($7, rules),
                formatting = COALESCE($8, formatting),
                compiled_prompt = COALESCE($9, compiled_prompt),
                enabled = COALESCE($10, enabled),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["mode"]))
        .bind(optional_string(&payload, &["identity"]))
        .bind(optional_string(&payload, &["rules"]))
        .bind(optional_string(&payload, &["formatting"]))
        .bind(optional_string(
            &payload,
            &["compiledPrompt", "compiled_prompt"],
        ))
        .bind(optional_bool(&payload, &["enabled"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("system prompt not found".to_string()))
    }

    async fn create_voice_profile(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO voice_profiles
                (organization_id, owner_user_id, client_id, name, description, path, speaker_id, language, avatar_color, is_default)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(optional_string(&payload, &["description"]))
        .bind(required_string(&payload, &["path"])?)
        .bind(optional_i32(&payload, &["speakerId", "speaker_id"]))
        .bind(string_or(&payload, &["language"], "fr"))
        .bind(optional_string(&payload, &["avatarColor", "avatar_color"]))
        .bind(bool_or(&payload, &["isDefault", "is_default"], false))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_voice_profile(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE voice_profiles
              SET
                client_id = COALESCE($4, client_id),
                name = COALESCE($5, name),
                description = COALESCE($6, description),
                path = COALESCE($7, path),
                speaker_id = COALESCE($8, speaker_id),
                language = COALESCE($9, language),
                avatar_color = COALESCE($10, avatar_color),
                is_default = COALESCE($11, is_default),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["description"]))
        .bind(optional_string(&payload, &["path"]))
        .bind(optional_i32(&payload, &["speakerId", "speaker_id"]))
        .bind(optional_string(&payload, &["language"]))
        .bind(optional_string(&payload, &["avatarColor", "avatar_color"]))
        .bind(optional_bool(&payload, &["isDefault", "is_default"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("voice profile not found".to_string()))
    }

    async fn create_skill_group(&self, organization_id: Uuid, payload: Value) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO skill_groups (organization_id, client_id, name, description)
              VALUES ($1, $2, $3, $4)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(optional_string(&payload, &["description"]))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_skill_group(
        &self,
        organization_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE skill_groups
              SET
                client_id = COALESCE($3, client_id),
                name = COALESCE($4, name),
                description = COALESCE($5, description),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["description"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("skill group not found".to_string()))
    }

    async fn create_skill(&self, organization_id: Uuid, payload: Value) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO skills
                (organization_id, client_id, group_id, client_group_id, name, description, icon, category, triggers, kind, content, enabled)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(optional_uuid_if_valid(&payload, &["groupId", "group_id"]))
        .bind(optional_string(&payload, &["clientGroupId", "client_group_id", "groupId"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(optional_string(&payload, &["description"]))
        .bind(optional_string(&payload, &["icon"]))
        .bind(optional_string(&payload, &["category"]))
        .bind(string_vec_or(&payload, &["triggers"], Vec::new()))
        .bind(string_or(&payload, &["kind", "type"], "system_prompt"))
        .bind(required_string(&payload, &["content"])?)
        .bind(bool_or(&payload, &["enabled"], true))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_skill(
        &self,
        organization_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE skills
              SET
                client_id = COALESCE($3, client_id),
                group_id = COALESCE($4, group_id),
                client_group_id = COALESCE($5, client_group_id),
                name = COALESCE($6, name),
                description = COALESCE($7, description),
                icon = COALESCE($8, icon),
                category = COALESCE($9, category),
                triggers = COALESCE($10, triggers),
                kind = COALESCE($11, kind),
                content = COALESCE($12, content),
                enabled = COALESCE($13, enabled),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_uuid_if_valid(&payload, &["groupId", "group_id"]))
        .bind(optional_string(
            &payload,
            &["clientGroupId", "client_group_id", "groupId"],
        ))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["description"]))
        .bind(optional_string(&payload, &["icon"]))
        .bind(optional_string(&payload, &["category"]))
        .bind(optional_string_vec(&payload, &["triggers"]))
        .bind(optional_string(&payload, &["kind", "type"]))
        .bind(optional_string(&payload, &["content"]))
        .bind(optional_bool(&payload, &["enabled"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("skill not found".to_string()))
    }

    async fn create_plugin_connection(
        &self,
        organization_id: Uuid,
        payload: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<Value> {
        let (public_config, secret_config) = split_plugin_payload(&payload);
        let encrypted_secret = secret_config
            .map(|secret| encrypted_payload_text(secret, secrets_key))
            .transpose()?;
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO plugin_connections
                (organization_id, client_id, name, description, category, status, auth_type, config, encrypted_secret, enabled)
              VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8,
                CASE WHEN $9::text IS NULL THEN NULL ELSE pgp_sym_encrypt($9::text, $10) END,
                $11
              )
              RETURNING *
            )
            SELECT to_jsonb(inserted) - 'encrypted_secret' AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(optional_string(&payload, &["description"]))
        .bind(optional_string(&payload, &["category"]))
        .bind(string_or(&payload, &["status"], "setup"))
        .bind(string_or(&payload, &["authType", "auth_type"], "none"))
        .bind(public_config)
        .bind(encrypted_secret)
        .bind(secrets_key.unwrap_or(""))
        .bind(bool_or(&payload, &["enabled"], true))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_plugin_connection(
        &self,
        organization_id: Uuid,
        item_id: Uuid,
        payload: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<Value> {
        let (public_config, encrypted_secret) =
            if optional_json(&payload, &["config", "fields"]).is_some() {
                let (public_config, secret_config) = split_plugin_payload(&payload);
                let encrypted_secret = secret_config
                    .map(|secret| encrypted_payload_text(secret, secrets_key))
                    .transpose()?;
                (Some(public_config), encrypted_secret)
            } else {
                (None, None)
            };
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE plugin_connections
              SET
                client_id = COALESCE($3, client_id),
                name = COALESCE($4, name),
                description = COALESCE($5, description),
                category = COALESCE($6, category),
                status = COALESCE($7, status),
                auth_type = COALESCE($8, auth_type),
                config = COALESCE($9, config),
                encrypted_secret = CASE
                  WHEN $10::text IS NULL THEN encrypted_secret
                  ELSE pgp_sym_encrypt($10::text, $11)
                END,
                enabled = COALESCE($12, enabled),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) - 'encrypted_secret' AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["description"]))
        .bind(optional_string(&payload, &["category"]))
        .bind(optional_string(&payload, &["status"]))
        .bind(optional_string(&payload, &["authType", "auth_type"]))
        .bind(public_config)
        .bind(encrypted_secret)
        .bind(secrets_key.unwrap_or(""))
        .bind(optional_bool(&payload, &["enabled"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("plugin connection not found".to_string()))
    }

    async fn create_mcp_server(
        &self,
        organization_id: Uuid,
        payload: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<Value> {
        let encrypted_env = optional_json(&payload, &["env"])
            .filter(|value| !value.as_object().is_some_and(|object| object.is_empty()))
            .map(|env| encrypted_payload_text(env, secrets_key))
            .transpose()?;
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO mcp_servers
                (organization_id, client_id, name, transport, command, args, encrypted_env, url, enabled, status, tools, resources)
              VALUES (
                $1, $2, $3, $4, $5, $6,
                CASE WHEN $7::text IS NULL THEN NULL ELSE pgp_sym_encrypt($7::text, $8) END,
                $9, $10, $11, $12, $13
              )
              RETURNING *
            )
            SELECT to_jsonb(inserted) - 'encrypted_env' AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(string_or(&payload, &["transport", "type"], "stdio"))
        .bind(optional_string(&payload, &["command"]))
        .bind(string_vec_or(&payload, &["args"], Vec::new()))
        .bind(encrypted_env)
        .bind(secrets_key.unwrap_or(""))
        .bind(optional_string(&payload, &["url"]))
        .bind(bool_or(&payload, &["enabled"], true))
        .bind(string_or(&payload, &["status"], "disconnected"))
        .bind(json_or(&payload, &["tools"], serde_json::json!([])))
        .bind(json_or(&payload, &["resources"], serde_json::json!([])))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_mcp_server(
        &self,
        organization_id: Uuid,
        item_id: Uuid,
        payload: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<Value> {
        let encrypted_env = optional_json(&payload, &["env"])
            .filter(|value| !value.as_object().is_some_and(|object| object.is_empty()))
            .map(|env| encrypted_payload_text(env, secrets_key))
            .transpose()?;
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE mcp_servers
              SET
                client_id = COALESCE($3, client_id),
                name = COALESCE($4, name),
                transport = COALESCE($5, transport),
                command = COALESCE($6, command),
                args = COALESCE($7, args),
                encrypted_env = CASE
                  WHEN $8::text IS NULL THEN encrypted_env
                  ELSE pgp_sym_encrypt($8::text, $9)
                END,
                url = COALESCE($10, url),
                enabled = COALESCE($11, enabled),
                status = COALESCE($12, status),
                tools = COALESCE($13, tools),
                resources = COALESCE($14, resources),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) - 'encrypted_env' AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["transport", "type"]))
        .bind(optional_string(&payload, &["command"]))
        .bind(optional_string_vec(&payload, &["args"]))
        .bind(encrypted_env)
        .bind(secrets_key.unwrap_or(""))
        .bind(optional_string(&payload, &["url"]))
        .bind(optional_bool(&payload, &["enabled"]))
        .bind(optional_string(&payload, &["status"]))
        .bind(optional_json(&payload, &["tools"]))
        .bind(optional_json(&payload, &["resources"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("MCP server not found".to_string()))
    }

    async fn create_hook(
        &self,
        organization_id: Uuid,
        payload: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<Value> {
        let encrypted_secret = optional_string(&payload, &["secret"])
            .filter(|secret| !secret.trim().is_empty())
            .map(|secret| {
                encrypted_payload_text(serde_json::json!({ "secret": secret }), secrets_key)
            })
            .transpose()?;
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO hooks (organization_id, client_id, name, url, encrypted_secret, events, enabled, status)
              VALUES (
                $1, $2, $3, $4,
                CASE WHEN $5::text IS NULL THEN NULL ELSE pgp_sym_encrypt($5::text, $6) END,
                $7, $8, $9
              )
              RETURNING *
            )
            SELECT to_jsonb(inserted) - 'encrypted_secret' AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(required_string(&payload, &["url"])?)
        .bind(encrypted_secret)
        .bind(secrets_key.unwrap_or(""))
        .bind(string_vec_or(&payload, &["events"], Vec::new()))
        .bind(bool_or(&payload, &["enabled"], true))
        .bind(string_or(&payload, &["status"], "idle"))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_hook(
        &self,
        organization_id: Uuid,
        item_id: Uuid,
        payload: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<Value> {
        let encrypted_secret = optional_string(&payload, &["secret"])
            .filter(|secret| !secret.trim().is_empty())
            .map(|secret| {
                encrypted_payload_text(serde_json::json!({ "secret": secret }), secrets_key)
            })
            .transpose()?;
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE hooks
              SET
                client_id = COALESCE($3, client_id),
                name = COALESCE($4, name),
                url = COALESCE($5, url),
                encrypted_secret = CASE
                  WHEN $6::text IS NULL THEN encrypted_secret
                  ELSE pgp_sym_encrypt($6::text, $7)
                END,
                events = COALESCE($8, events),
                enabled = COALESCE($9, enabled),
                status = COALESCE($10, status),
                last_triggered_at = COALESCE($11, last_triggered_at),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) - 'encrypted_secret' AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["url"]))
        .bind(encrypted_secret)
        .bind(secrets_key.unwrap_or(""))
        .bind(optional_string_vec(&payload, &["events"]))
        .bind(optional_bool(&payload, &["enabled"]))
        .bind(optional_string(&payload, &["status"]))
        .bind(optional_datetime(
            &payload,
            &["lastTriggered", "last_triggered_at"],
        ))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("hook not found".to_string()))
    }

    async fn create_scheduled_task(
        &self,
        organization_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO scheduled_tasks
                (organization_id, client_id, name, prompt, schedule_type, duration_minutes, cron_expression, enabled, status, last_run_at)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(required_string(&payload, &["prompt"])?)
        .bind(string_or(&payload, &["scheduleType", "schedule_type", "type"], "timer"))
        .bind(optional_i32(&payload, &["durationMinutes", "duration_minutes"]))
        .bind(optional_string(&payload, &["cronExpression", "cron_expression"]))
        .bind(bool_or(&payload, &["enabled"], true))
        .bind(string_or(&payload, &["status"], "idle"))
        .bind(optional_datetime(&payload, &["lastRun", "last_run_at"]))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_scheduled_task(
        &self,
        organization_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE scheduled_tasks
              SET
                client_id = COALESCE($3, client_id),
                name = COALESCE($4, name),
                prompt = COALESCE($5, prompt),
                schedule_type = COALESCE($6, schedule_type),
                duration_minutes = COALESCE($7, duration_minutes),
                cron_expression = COALESCE($8, cron_expression),
                enabled = COALESCE($9, enabled),
                status = COALESCE($10, status),
                last_run_at = COALESCE($11, last_run_at),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["prompt"]))
        .bind(optional_string(
            &payload,
            &["scheduleType", "schedule_type", "type"],
        ))
        .bind(optional_i32(
            &payload,
            &["durationMinutes", "duration_minutes"],
        ))
        .bind(optional_string(
            &payload,
            &["cronExpression", "cron_expression"],
        ))
        .bind(optional_bool(&payload, &["enabled"]))
        .bind(optional_string(&payload, &["status"]))
        .bind(optional_datetime(&payload, &["lastRun", "last_run_at"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("scheduled task not found".to_string()))
    }

    pub async fn run_scheduled_task(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        task_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        self.ensure_org_manager(user_id, organization_id).await?;
        let run_status = string_or(&payload, &["status", "runStatus", "run_status"], "running");
        let task_status = string_or(&payload, &["taskStatus", "task_status"], &run_status);
        let output = value_for(&payload, &["output"]).map(|value| match value {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        });
        let started_at = optional_datetime(&payload, &["startedAt", "started_at"]);
        let finished_at = optional_datetime(&payload, &["finishedAt", "finished_at"]);
        let row = sqlx::query(
            r#"
            WITH updated_task AS (
              UPDATE scheduled_tasks
              SET
                status = $4,
                last_run_at = COALESCE($5, now()),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
              RETURNING *
            ),
            inserted_run AS (
              INSERT INTO task_runs
                (organization_id, scheduled_task_id, status, output, started_at, finished_at)
              SELECT $2, updated_task.id, $3, $6, COALESCE($5, now()), $7
              FROM updated_task
              RETURNING *
            )
            SELECT jsonb_build_object(
              'taskRun', to_jsonb(inserted_run),
              'scheduledTask', to_jsonb(updated_task)
            ) AS data
            FROM inserted_run
            JOIN updated_task ON updated_task.id = inserted_run.scheduled_task_id
            "#,
        )
        .bind(task_id)
        .bind(organization_id)
        .bind(run_status)
        .bind(task_status)
        .bind(started_at)
        .bind(output)
        .bind(finished_at)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        let data: Value = row
            .map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("scheduled task not found".to_string()))?;
        self.emit_domain_event(
            organization_id,
            user_id,
            "scheduled-tasks.run",
            "scheduled-tasks",
            Some(task_id),
            serde_json::json!({
                "collection": "scheduled-tasks",
                "item": data["scheduledTask"].clone(),
                "taskRun": data["taskRun"].clone(),
            }),
        )
        .await?;
        Ok(data)
    }

    async fn create_team(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        self.ensure_org_manager(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO teams (organization_id, client_id, name, description, member_ids)
              VALUES ($1, $2, $3, $4, $5)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id", "id"]))
        .bind(required_string(&payload, &["name"])?)
        .bind(optional_string(&payload, &["description"]))
        .bind(string_vec_or(
            &payload,
            &["memberIds", "member_ids"],
            Vec::new(),
        ))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_team(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        self.ensure_org_manager(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE teams
              SET
                client_id = COALESCE($3, client_id),
                name = COALESCE($4, name),
                description = COALESCE($5, description),
                member_ids = COALESCE($6, member_ids),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(optional_string(&payload, &["clientId", "client_id"]))
        .bind(optional_string(&payload, &["name"]))
        .bind(optional_string(&payload, &["description"]))
        .bind(optional_string_vec(&payload, &["memberIds", "member_ids"]))
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| row.get("data"))
            .ok_or_else(|| AroError::Memory("team not found".to_string()))
    }

    async fn create_usage_event(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO usage_events
                (organization_id, user_id, conversation_id, event_type, data)
              VALUES ($1, $2, $3, $4, $5)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(optional_uuid(
            &payload,
            &["conversationId", "conversation_id"],
        )?)
        .bind(required_string(&payload, &["eventType", "event_type"])?)
        .bind(json_or(&payload, &["data"], serde_json::json!({})))
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    pub async fn list_client_state(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        secrets_key: Option<&str>,
    ) -> AroResult<BTreeMap<String, Value>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT key, value
            FROM client_state
            WHERE organization_id = $1
              AND user_id = $2
              AND sensitive = false
              AND deleted_at IS NULL
            ORDER BY key ASC
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        let mut state = rows
            .into_iter()
            .map(|row| (row.get::<String, _>("key"), row.get::<Value, _>("value")))
            .collect::<BTreeMap<_, _>>();

        let Some(secrets_key) = secrets_key else {
            return Ok(state);
        };

        let sensitive_keys = sqlx::query(
            r#"
            SELECT key
            FROM client_state
            WHERE organization_id = $1
              AND user_id = $2
              AND sensitive = true
              AND deleted_at IS NULL
            ORDER BY key ASC
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;

        for row in sensitive_keys {
            let key = row.get::<String, _>("key");
            match self
                .decrypt_client_state_value(user_id, organization_id, &key, secrets_key)
                .await
            {
                Ok(Some(value)) => {
                    state.insert(key, value);
                }
                Ok(None) => {}
                Err(err) if is_client_state_decrypt_error(&err) => {
                    self.soft_delete_client_state_key(organization_id, user_id, &key)
                        .await?;
                }
                Err(err) => return Err(map_sqlx(err)),
            }
        }

        Ok(state)
    }

    pub async fn get_client_state(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        key: String,
        secrets_key: Option<&str>,
    ) -> AroResult<Option<Value>> {
        self.ensure_org_access(user_id, organization_id).await?;
        let row = sqlx::query(
            r#"
            SELECT sensitive, value
            FROM client_state
            WHERE organization_id = $1
              AND user_id = $2
              AND key = $3
              AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(&key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx)?;

        let Some(row) = row else {
            return Ok(None);
        };

        if !row.get::<bool, _>("sensitive") {
            return Ok(Some(row.get("value")));
        }

        let Some(secrets_key) = secrets_key else {
            return Ok(None);
        };

        match self
            .decrypt_client_state_value(user_id, organization_id, &key, secrets_key)
            .await
        {
            Ok(value) => Ok(value),
            Err(err) if is_client_state_decrypt_error(&err) => {
                self.soft_delete_client_state_key(organization_id, user_id, &key)
                    .await?;
                Ok(None)
            }
            Err(err) => Err(map_sqlx(err)),
        }
    }

    async fn decrypt_client_state_value(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        key: &str,
        secrets_key: &str,
    ) -> Result<Option<Value>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT pgp_sym_decrypt(encrypted_value, $4)::jsonb AS value
            FROM client_state
            WHERE organization_id = $1
              AND user_id = $2
              AND key = $3
              AND sensitive = true
              AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(key)
        .bind(secrets_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| row.get("value")))
    }

    async fn soft_delete_client_state_key(
        &self,
        organization_id: Uuid,
        user_id: Uuid,
        key: &str,
    ) -> AroResult<()> {
        sqlx::query(
            r#"
            UPDATE client_state
            SET deleted_at = now(), updated_at = now()
            WHERE organization_id = $1
              AND user_id = $2
              AND key = $3
              AND sensitive = true
              AND deleted_at IS NULL
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(key)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn set_client_state(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        key: String,
        value: Value,
        secrets_key: Option<&str>,
    ) -> AroResult<()> {
        self.ensure_org_access(user_id, organization_id).await?;
        if is_sensitive_client_state_key(&key) {
            let secrets_key = secrets_key.ok_or_else(|| {
                AroError::Configuration(
                    "ARO_SECRETS_KEY is required for sensitive client state".to_string(),
                )
            })?;
            sqlx::query(
                r#"
                INSERT INTO client_state
                  (organization_id, user_id, key, value, encrypted_value, sensitive, updated_at, deleted_at)
                VALUES ($1, $2, $3, NULL, pgp_sym_encrypt($4::text, $5), true, now(), NULL)
                ON CONFLICT (organization_id, user_id, key)
                DO UPDATE SET
                  value = NULL,
                  encrypted_value = excluded.encrypted_value,
                  sensitive = true,
                  updated_at = now(),
                  deleted_at = NULL
                "#,
            )
            .bind(organization_id)
            .bind(user_id)
            .bind(key)
            .bind(value.to_string())
            .bind(secrets_key)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO client_state
                  (organization_id, user_id, key, value, encrypted_value, sensitive, updated_at, deleted_at)
                VALUES ($1, $2, $3, $4, NULL, false, now(), NULL)
                ON CONFLICT (organization_id, user_id, key)
                DO UPDATE SET
                  value = excluded.value,
                  encrypted_value = NULL,
                  sensitive = false,
                  updated_at = now(),
                  deleted_at = NULL
                "#,
            )
            .bind(organization_id)
            .bind(user_id)
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await
            .map_err(map_sqlx)?;
        }
        Ok(())
    }

    async fn create_custom_agent_definition(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let name = required_string(&payload, &["name"])?;
        let description = optional_string(&payload, &["description"]);
        let system_prompt = optional_string(&payload, &["systemPrompt", "system_prompt"]);
        let model_provider_id =
            optional_string(&payload, &["modelProviderId", "model_provider_id"]);
        let model_id = optional_string(&payload, &["modelId", "model_id"]);
        let autonomy_profile_id =
            optional_uuid_if_valid(&payload, &["autonomyProfileId", "autonomy_profile_id"]);
        let enabled_tools =
            optional_string_vec(&payload, &["enabledTools", "enabled_tools"]).unwrap_or_default();

        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO custom_agent_definitions
                (organization_id, owner_user_id, name, description, system_prompt, model_provider_id, model_id, autonomy_profile_id, enabled_tools)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(name)
        .bind(description)
        .bind(system_prompt)
        .bind(model_provider_id)
        .bind(model_id)
        .bind(autonomy_profile_id)
        .bind(enabled_tools)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_custom_agent_definition(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let name = optional_string(&payload, &["name"]);
        let description = optional_string(&payload, &["description"]);
        let system_prompt = optional_string(&payload, &["systemPrompt", "system_prompt"]);
        let model_provider_id =
            optional_string(&payload, &["modelProviderId", "model_provider_id"]);
        let model_id = optional_string(&payload, &["modelId", "model_id"]);
        let autonomy_profile_id = autonomy_profile_id_param(&payload);
        let enabled_tools = optional_string_vec(&payload, &["enabledTools", "enabled_tools"]);

        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE custom_agent_definitions
              SET
                name = COALESCE($4, name),
                description = COALESCE($5, description),
                system_prompt = COALESCE($6, system_prompt),
                model_provider_id = COALESCE($7, model_provider_id),
                model_id = COALESCE($8, model_id),
                autonomy_profile_id = COALESCE($9, autonomy_profile_id),
                enabled_tools = COALESCE($10, enabled_tools),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(name)
        .bind(description)
        .bind(system_prompt)
        .bind(model_provider_id)
        .bind(model_id)
        .bind(autonomy_profile_id)
        .bind(enabled_tools)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn create_custom_model_definition(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let provider_kind = required_string(&payload, &["providerKind", "provider_kind"])?;
        let provider_id = required_string(&payload, &["providerId", "provider_id"])?;
        let model_id = required_string(&payload, &["modelId", "model_id"])?;
        let label = required_string(&payload, &["label"])?;
        let endpoint = required_string(&payload, &["endpoint"])?;
        let api_key_vault_key = optional_string(&payload, &["apiKeyVaultKey", "api_key_vault_key"]);

        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO custom_model_definitions
                (organization_id, owner_user_id, provider_kind, provider_id, model_id, label, endpoint, api_key_vault_key)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(provider_kind)
        .bind(provider_id)
        .bind(model_id)
        .bind(label)
        .bind(endpoint)
        .bind(api_key_vault_key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_custom_model_definition(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let provider_kind = optional_string(&payload, &["providerKind", "provider_kind"]);
        let provider_id = optional_string(&payload, &["providerId", "provider_id"]);
        let model_id = optional_string(&payload, &["modelId", "model_id"]);
        let label = optional_string(&payload, &["label"]);
        let endpoint = optional_string(&payload, &["endpoint"]);
        let api_key_vault_key = optional_string(&payload, &["apiKeyVaultKey", "api_key_vault_key"]);

        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE custom_model_definitions
              SET
                provider_kind = COALESCE($4, provider_kind),
                provider_id = COALESCE($5, provider_id),
                model_id = COALESCE($6, model_id),
                label = COALESCE($7, label),
                endpoint = COALESCE($8, endpoint),
                api_key_vault_key = COALESCE($9, api_key_vault_key),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(provider_kind)
        .bind(provider_id)
        .bind(model_id)
        .bind(label)
        .bind(endpoint)
        .bind(api_key_vault_key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn create_plan(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let conversation_id = optional_uuid(&payload, &["conversationId", "conversation_id"])?
            .ok_or_else(|| AroError::Configuration("a plan requires a conversationId".into()))?;
        let tenant = TenantContext::new(owner_user_id, organization_id)?;
        if self.get_conversation(tenant, conversation_id).await?.is_none() {
            return Err(AroError::Security("plan conversation is not accessible".into()));
        }
        let title = required_string(&payload, &["title"])?;
        let description = optional_string(&payload, &["description"]);
        let tasks = payload
            .get("tasks")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        let status = string_or(&payload, &["status"], "active");

        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO plans
                (organization_id, owner_user_id, title, description, tasks, status, conversation_id)
              VALUES ($1, $2, $3, $4, $5, $6, $7)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(title)
        .bind(description)
        .bind(tasks)
        .bind(status)
        .bind(conversation_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_plan(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let title = optional_string(&payload, &["title"]);
        let description = optional_string(&payload, &["description"]);
        let tasks = payload.get("tasks").cloned();
        let status = optional_string(&payload, &["status"]);

        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE plans
              SET
                title = COALESCE($4, title),
                description = COALESCE($5, description),
                tasks = COALESCE($6, tasks),
                status = COALESCE($7, status),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(title)
        .bind(description)
        .bind(tasks)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    pub async fn delete_client_state(
        &self,
        user_id: Uuid,
        organization_id: Uuid,
        key: String,
    ) -> AroResult<()> {
        self.ensure_org_access(user_id, organization_id).await?;
        sqlx::query(
            r#"
            UPDATE client_state
            SET deleted_at = now(), updated_at = now()
            WHERE organization_id = $1 AND user_id = $2 AND key = $3
            "#,
        )
        .bind(organization_id)
        .bind(user_id)
        .bind(key)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

fn validate_idempotency_request_key(identity: &IdempotencyRequestKey) -> AroResult<()> {
    let valid_component = |value: &str, max_len: usize| {
        !value.is_empty()
            && value.len() <= max_len
            && value.bytes().all(|byte| byte.is_ascii_graphic())
    };
    if !valid_component(&identity.scope, 200) {
        return Err(AroError::Configuration(
            "idempotency scope must contain 1 to 200 visible ASCII bytes".to_string(),
        ));
    }
    if !valid_component(&identity.key, 255) {
        return Err(AroError::Configuration(
            "idempotency key must contain 1 to 255 visible ASCII bytes".to_string(),
        ));
    }
    Ok(())
}

fn validate_idempotency_ttl(ttl_seconds: i64) -> AroResult<()> {
    if !(IDEMPOTENCY_MIN_TTL_SECONDS..=IDEMPOTENCY_MAX_TTL_SECONDS).contains(&ttl_seconds) {
        return Err(AroError::Configuration(format!(
            "idempotency TTL must be between {IDEMPOTENCY_MIN_TTL_SECONDS} and {IDEMPOTENCY_MAX_TTL_SECONDS} seconds"
        )));
    }
    Ok(())
}

fn validate_idempotency_response(response: &IdempotencyResponse) -> AroResult<Value> {
    if !(200..=599).contains(&response.status_code) {
        return Err(AroError::Configuration(
            "idempotency response status must be between 200 and 599".to_string(),
        ));
    }
    if response.body.len() > IDEMPOTENCY_MAX_RESPONSE_BODY_BYTES {
        return Err(AroError::Configuration(format!(
            "idempotency response body exceeds {IDEMPOTENCY_MAX_RESPONSE_BODY_BYTES} bytes"
        )));
    }
    for (name, values) in &response.headers {
        if name.is_empty()
            || name.len() > 256
            || name != &name.to_ascii_lowercase()
            || !name.bytes().all(is_http_token_byte)
        {
            return Err(AroError::Configuration(
                "idempotency response header names must be lowercase HTTP tokens".to_string(),
            ));
        }
        if values.is_empty()
            || values.iter().any(|value| {
                value.len() > 8192
                    || !value
                        .bytes()
                        .all(|byte| byte == b'\t' || (0x20..=0x7e).contains(&byte))
            })
        {
            return Err(AroError::Configuration(
                "idempotency response header values are invalid".to_string(),
            ));
        }
    }
    let headers =
        serde_json::to_value(&response.headers).map_err(|err| AroError::Memory(err.to_string()))?;
    let encoded_len = serde_json::to_vec(&headers)
        .map_err(|err| AroError::Memory(err.to_string()))?
        .len();
    if encoded_len > IDEMPOTENCY_MAX_RESPONSE_HEADERS_BYTES {
        return Err(AroError::Configuration(format!(
            "idempotency response headers exceed {IDEMPOTENCY_MAX_RESPONSE_HEADERS_BYTES} bytes"
        )));
    }
    Ok(headers)
}

fn is_http_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn map_idempotency_response_row(row: &sqlx::postgres::PgRow) -> AroResult<IdempotencyResponse> {
    let status_code = row
        .get::<Option<i16>, _>("response_status")
        .and_then(|status| u16::try_from(status).ok())
        .ok_or_else(|| {
            AroError::Memory("completed idempotency response has no valid status".to_string())
        })?;
    let headers = row
        .get::<Option<Value>, _>("response_headers")
        .ok_or_else(|| {
            AroError::Memory("completed idempotency response has no headers".to_string())
        })?;
    let headers = serde_json::from_value(headers).map_err(|err| {
        AroError::Memory(format!(
            "completed idempotency response headers are invalid: {err}"
        ))
    })?;
    let body = row
        .get::<Option<Vec<u8>>, _>("response_body")
        .ok_or_else(|| {
            AroError::Memory("completed idempotency response has no body".to_string())
        })?;
    Ok(IdempotencyResponse {
        status_code,
        headers,
        body,
    })
}

#[derive(Debug, Clone)]
pub struct NewUserWithOrg {
    pub email: String,
    pub name: String,
    pub role_title: Option<String>,
    pub avatar_color: Option<String>,
    pub password_hash: String,
    pub organization_name: String,
    pub organization_domain: Option<String>,
    pub organization_description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserCredentials {
    pub user: User,
    pub password_hash: String,
}

#[derive(Debug, Clone)]
pub struct RefreshTokenPrincipal {
    pub user_id: Uuid,
    pub organization_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct RotatedRefreshToken {
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AuthPrincipal {
    pub user: User,
    pub active_organization: Organization,
    pub memberships: Vec<Membership>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxEvent {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub event_type: String,
    pub payload: Value,
    pub available_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// An event leased to one durable worker. The lease token is required to
/// acknowledge or retry it, preventing a late worker from acknowledging work
/// reclaimed after its lease expired.
#[derive(Debug, Clone)]
pub struct ClaimedOutboxEvent {
    pub event: OutboxEvent,
    pub lease_token: Uuid,
}

#[derive(Debug, Clone)]
pub struct NewOrganizationMember {
    pub email: String,
    pub name: String,
    pub role: MembershipRole,
}

/// Safe receipt returned to the issuing administrator. It intentionally contains no bearer
/// credential or acceptance URL.
#[derive(Debug, Clone)]
pub struct IssuedOrganizationInvitation {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: MembershipRole,
    /// Existing accounts get an invited membership immediately. New e-mail addresses do not
    /// get a user or membership row until the mailbox owner accepts the server-delivered token.
    pub member: Option<OrganizationMember>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferencesPatch {
    pub theme: Option<String>,
    pub language: Option<String>,
    pub wake_word_enabled: Option<bool>,
    pub inference_mode: Option<InferenceMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfilePatch {
    pub name: Option<String>,
    pub role_title: Option<String>,
    pub avatar_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationPatch {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum PersistedCollection {
    Memories,
    Personalities,
    SystemPrompts,
    VoiceProfiles,
    SkillGroups,
    Skills,
    PluginConnections,
    McpServers,
    Hooks,
    ScheduledTasks,
    Teams,
    UsageEvents,
    CustomAgentDefinitions,
    CustomModelDefinitions,
    Plans,
}

impl PersistedCollection {
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "memories" => Some(Self::Memories),
            "personalities" => Some(Self::Personalities),
            "system-prompts" => Some(Self::SystemPrompts),
            "voice-profiles" => Some(Self::VoiceProfiles),
            "skill-groups" => Some(Self::SkillGroups),
            "skills" => Some(Self::Skills),
            "plugins" | "plugin-connections" => Some(Self::PluginConnections),
            "mcp" | "mcp-servers" => Some(Self::McpServers),
            "hooks" => Some(Self::Hooks),
            "scheduled-tasks" => Some(Self::ScheduledTasks),
            "teams" => Some(Self::Teams),
            "usage" | "usage-events" => Some(Self::UsageEvents),
            "agent-definitions" | "agents" => Some(Self::CustomAgentDefinitions),
            "model-definitions" | "models" => Some(Self::CustomModelDefinitions),
            "plans" => Some(Self::Plans),
            _ => None,
        }
    }

    fn table_name(self) -> &'static str {
        match self {
            Self::Memories => "memories",
            Self::Personalities => "personalities",
            Self::SystemPrompts => "system_prompts",
            Self::VoiceProfiles => "voice_profiles",
            Self::SkillGroups => "skill_groups",
            Self::Skills => "skills",
            Self::PluginConnections => "plugin_connections",
            Self::McpServers => "mcp_servers",
            Self::Hooks => "hooks",
            Self::ScheduledTasks => "scheduled_tasks",
            Self::Teams => "teams",
            Self::UsageEvents => "usage_events",
            Self::CustomAgentDefinitions => "custom_agent_definitions",
            Self::CustomModelDefinitions => "custom_model_definitions",
            Self::Plans => "plans",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Memories => "memories",
            Self::Personalities => "personalities",
            Self::SystemPrompts => "system-prompts",
            Self::VoiceProfiles => "voice-profiles",
            Self::SkillGroups => "skill-groups",
            Self::Skills => "skills",
            Self::PluginConnections => "plugins",
            Self::McpServers => "mcp",
            Self::Hooks => "hooks",
            Self::ScheduledTasks => "scheduled-tasks",
            Self::Teams => "teams",
            Self::UsageEvents => "usage",
            Self::CustomAgentDefinitions => "agent-definitions",
            Self::CustomModelDefinitions => "model-definitions",
            Self::Plans => "plans",
        }
    }

    fn order_column(self) -> &'static str {
        match self {
            Self::UsageEvents => "created_at",
            _ => "updated_at",
        }
    }

    fn has_soft_delete(self) -> bool {
        !matches!(self, Self::UsageEvents)
    }

    fn deleted_clause(self, alias: &str) -> String {
        let prefix = if alias.is_empty() {
            String::new()
        } else {
            format!("{alias}.")
        };
        if self.has_soft_delete() {
            format!("AND {prefix}deleted_at IS NULL")
        } else {
            String::new()
        }
    }

    fn is_user_owned(self) -> bool {
        matches!(
            self,
            Self::Memories
                | Self::Personalities
                | Self::SystemPrompts
                | Self::VoiceProfiles
                | Self::CustomAgentDefinitions
                | Self::CustomModelDefinitions
                | Self::Plans
        )
    }

    fn owner_clause(self, alias: &str, bind_index: usize) -> String {
        if !self.is_user_owned() {
            return String::new();
        }
        let prefix = if alias.is_empty() {
            String::new()
        } else {
            format!("{alias}.")
        };
        format!("AND {prefix}owner_user_id = ${bind_index}")
    }

    fn is_admin_managed(self) -> bool {
        matches!(
            self,
            Self::PluginConnections | Self::McpServers | Self::Hooks | Self::UsageEvents
        )
    }

    fn is_manager_managed(self) -> bool {
        matches!(
            self,
            Self::SkillGroups | Self::Skills | Self::ScheduledTasks | Self::Teams
        )
    }

    fn public_json_expr(self, alias: &str) -> String {
        match self {
            Self::PluginConnections | Self::Hooks => {
                format!("to_jsonb({alias}) - 'encrypted_secret'")
            }
            Self::McpServers => format!("to_jsonb({alias}) - 'encrypted_env'"),
            _ => format!("to_jsonb({alias})"),
        }
    }

    fn safe_event_payload(self, item: &Value) -> Value {
        if matches!(self, Self::Memories) {
            serde_json::json!({
                "collection": self.slug(),
                "item": memory_event_item(item),
            })
        } else {
            serde_json::json!({
                "collection": self.slug(),
                "item": item.clone(),
            })
        }
    }
}

fn memory_event_item(item: &Value) -> Value {
    let content_hash = optional_string(item, &["content"]).map(|content| hash_secret(&content));
    serde_json::json!({
        "id": item.get("id").cloned().unwrap_or(Value::Null),
        "clientId": item.get("client_id").or_else(|| item.get("clientId")).cloned().unwrap_or(Value::Null),
        "category": item.get("category").cloned().unwrap_or(Value::Null),
        "scope": item.get("scope").cloned().unwrap_or(Value::Null),
        "status": item.get("status").cloned().unwrap_or(Value::Null),
        "pinned": item.get("pinned").cloned().unwrap_or(Value::Bool(false)),
        "contentHash": content_hash,
    })
}

#[derive(Debug, Clone)]
struct LockedRefreshTokenFamily {
    user_id: Uuid,
    device_id: Option<Uuid>,
    absolute_expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

/// Serializes rotations within one family without blocking unrelated sessions and returns the
/// family's authoritative absolute-lifetime and revocation state.
async fn lock_refresh_token_family(
    tx: &mut Transaction<'_, Postgres>,
    family_id: Uuid,
) -> AroResult<LockedRefreshTokenFamily> {
    let row = sqlx::query(
        r#"
        SELECT user_id, device_id, absolute_expires_at, revoked_at
        FROM refresh_token_families
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(family_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    let Some(row) = row else {
        return Err(AroError::Security(
            "refresh token family is unavailable".to_string(),
        ));
    };
    Ok(LockedRefreshTokenFamily {
        user_id: row.get("user_id"),
        device_id: row.get("device_id"),
        absolute_expires_at: row.get("absolute_expires_at"),
        revoked_at: row.get("revoked_at"),
    })
}

/// Marks a rotated credential as reused and revokes every still-active token in its family.
/// The caller must hold a row lock on `reused_token_id`; keeping this update in the same
/// transaction makes concurrent rotations deterministic and prevents a descendant from escaping
/// the compromise response.
async fn revoke_refresh_token_family_for_reuse(
    tx: &mut Transaction<'_, Postgres>,
    family_id: Uuid,
    reused_token_id: Uuid,
) -> AroResult<()> {
    sqlx::query(
        r#"
        UPDATE refresh_token_families
        SET revoked_at = COALESCE(revoked_at, now()),
            reuse_detected_at = COALESCE(reuse_detected_at, now()),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(family_id)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    sqlx::query(
        r#"
        UPDATE refresh_tokens
        SET revoked_at = COALESCE(revoked_at, now()),
            reuse_detected_at = CASE
              WHEN id = $2 THEN COALESCE(reuse_detected_at, now())
              ELSE reuse_detected_at
            END
        WHERE family_id = $1
        "#,
    )
    .bind(family_id)
    .bind(reused_token_id)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn create_membership(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    organization_id: Uuid,
    role: MembershipRole,
    status: MembershipStatus,
) -> AroResult<Membership> {
    let role = enum_to_string(&role)?;
    let status = enum_to_string(&status)?;
    let row = sqlx::query(
        r#"
        INSERT INTO memberships (user_id, organization_id, role, status)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, organization_id, role, status, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(organization_id)
    .bind(role)
    .bind(status)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    map_membership_row(row)
}

#[derive(Debug, Clone)]
struct LockedExistingOrganizationInvitation {
    id: Uuid,
    organization_id: Uuid,
    membership_id: Option<Uuid>,
    invited_role: String,
}

async fn lock_organization_admin(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    organization_id: Uuid,
) -> AroResult<()> {
    let authorized = sqlx::query(
        r#"
        SELECT m.id
        FROM memberships m
        JOIN users u ON u.id = m.user_id AND u.deleted_at IS NULL
        JOIN organizations o ON o.id = m.organization_id AND o.deleted_at IS NULL
        WHERE m.user_id = $1
          AND m.organization_id = $2
          AND m.status = 'active'
          AND m.role IN ('owner', 'admin')
          AND m.deleted_at IS NULL
        FOR SHARE OF m
        "#,
    )
    .bind(actor_user_id)
    .bind(organization_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if authorized.is_none() {
        return Err(AroError::Security("admin role required".to_string()));
    }
    Ok(())
}

async fn lock_existing_organization_invitation(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    token: &str,
) -> AroResult<LockedExistingOrganizationInvitation> {
    let invitation = sqlx::query(
        r#"
        SELECT i.id, i.organization_id, i.membership_id, i.invited_role
        FROM organization_invitations i
        JOIN users u
          ON u.id = $2
         AND lower(btrim(u.email)) = i.invited_email
         AND u.deleted_at IS NULL
        LEFT JOIN memberships m ON m.id = i.membership_id
        WHERE i.token_hash = $1
          AND (i.invited_user_id IS NULL OR i.invited_user_id = $2)
          AND i.accepted_at IS NULL
          AND i.revoked_at IS NULL
          AND i.expires_at > now()
          AND (m.id IS NULL OR (m.status = 'invited' AND m.deleted_at IS NULL))
        FOR UPDATE OF i, u
        "#,
    )
    .bind(hash_secret(token))
    .bind(actor_user_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_sqlx)?
    .ok_or_else(|| {
        AroError::Security(
            "invitation is invalid, expired, or belongs to another account".to_string(),
        )
    })?;
    Ok(LockedExistingOrganizationInvitation {
        id: invitation.get("id"),
        organization_id: invitation.get("organization_id"),
        membership_id: invitation.get("membership_id"),
        invited_role: invitation.get("invited_role"),
    })
}

async fn activate_locked_organization_invitation(
    tx: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    invitation: &LockedExistingOrganizationInvitation,
) -> AroResult<Uuid> {
    let active = enum_to_string(&MembershipStatus::Active)?;
    let membership_id = if let Some(membership_id) = invitation.membership_id {
        let locked_membership = sqlx::query(
            r#"
            SELECT status, deleted_at
            FROM memberships
            WHERE id = $1 AND organization_id = $2 AND user_id = $3
            FOR UPDATE
            "#,
        )
        .bind(membership_id)
        .bind(invitation.organization_id)
        .bind(actor_user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| {
            AroError::Security("invited membership is no longer available".to_string())
        })?;
        let status: String = locked_membership.get("status");
        let deleted_at: Option<DateTime<Utc>> = locked_membership.get("deleted_at");
        if status != "invited" || deleted_at.is_some() {
            return Err(AroError::Security(
                "invited membership is no longer available".to_string(),
            ));
        }
        let updated = sqlx::query(
            r#"
            UPDATE memberships
            SET status = $4, updated_at = now()
            WHERE id = $1
              AND organization_id = $2
              AND user_id = $3
              AND status = 'invited'
              AND deleted_at IS NULL
            "#,
        )
        .bind(membership_id)
        .bind(invitation.organization_id)
        .bind(actor_user_id)
        .bind(&active)
        .execute(&mut **tx)
        .await
        .map_err(map_sqlx)?;
        if updated.rows_affected() != 1 {
            return Err(AroError::Security(
                "invited membership is no longer available".to_string(),
            ));
        }
        membership_id
    } else {
        sqlx::query_scalar(
            r#"
            INSERT INTO memberships (user_id, organization_id, role, status)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id, organization_id)
            DO UPDATE SET
              role = CASE
                WHEN memberships.role = 'owner' THEN memberships.role
                ELSE excluded.role
              END,
              status = CASE
                WHEN memberships.role = 'owner' THEN memberships.status
                ELSE excluded.status
              END,
              deleted_at = CASE
                WHEN memberships.role = 'owner' THEN memberships.deleted_at
                ELSE NULL
              END,
              updated_at = now()
            RETURNING id
            "#,
        )
        .bind(actor_user_id)
        .bind(invitation.organization_id)
        .bind(&invitation.invited_role)
        .bind(&active)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_sqlx)?
    };
    let accepted = sqlx::query(
        r#"
        UPDATE organization_invitations
        SET invited_user_id = $2,
            membership_id = $3,
            accepted_at = now(),
            delivery_status = 'delivered',
            delivered_at = COALESCE(delivered_at, now()),
            delivery_token_encrypted = NULL,
            delivery_lease_token = NULL,
            delivery_lease_until = NULL,
            delivery_last_error = NULL,
            updated_at = now()
        WHERE id = $1 AND accepted_at IS NULL AND revoked_at IS NULL
        "#,
    )
    .bind(invitation.id)
    .bind(actor_user_id)
    .bind(membership_id)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if accepted.rows_affected() != 1 {
        return Err(AroError::Security(
            "invitation is no longer available".to_string(),
        ));
    }
    sqlx::query(
        r#"
        UPDATE users
        SET email_verified_at = COALESCE(email_verified_at, now()), updated_at = now()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(actor_user_id)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    upsert_default_settings(tx, invitation.organization_id, actor_user_id).await?;
    emit_domain_event_in_tx(
        tx,
        invitation.organization_id,
        actor_user_id,
        "membership.invitation.accepted",
        "membership",
        Some(membership_id),
        serde_json::json!({ "membershipId": membership_id }),
    )
    .await?;
    Ok(membership_id)
}

async fn upsert_default_preferences(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> AroResult<()> {
    sqlx::query(
        r#"
        INSERT INTO user_preferences (user_id, theme, language, wake_word_enabled, inference_mode)
        VALUES ($1, 'light', 'fr', false, 'local')
        ON CONFLICT (user_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn upsert_default_settings(
    tx: &mut Transaction<'_, Postgres>,
    organization_id: Uuid,
    user_id: Uuid,
) -> AroResult<()> {
    let settings = serde_json::to_value(AppSettings::default())
        .map_err(|err| AroError::Memory(err.to_string()))?;
    sqlx::query(
        r#"
        INSERT INTO app_settings (organization_id, user_id, settings)
        VALUES ($1, $2, $3)
        ON CONFLICT (organization_id, user_id) DO NOTHING
        "#,
    )
    .bind(organization_id)
    .bind(user_id)
    .bind(settings)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

async fn ensure_conversation_owner_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    context: TenantContext,
    conversation_id: Uuid,
) -> AroResult<()> {
    let exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
          SELECT 1
          FROM conversations
          WHERE id = $1
            AND organization_id = $2
            AND owner_user_id = $3
            AND deleted_at IS NULL
        )
        "#,
    )
    .bind(conversation_id)
    .bind(context.organization_id())
    .bind(context.actor_id())
    .fetch_one(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if exists {
        Ok(())
    } else {
        Err(AroError::Security("conversation access denied".to_string()))
    }
}

async fn insert_message_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    organization_id: Uuid,
    message: &ChatMessage,
) -> AroResult<()> {
    let role = enum_to_string(&message.role)?;
    let result = sqlx::query(
        r#"
        INSERT INTO messages
          (id, organization_id, conversation_id, role, content, created_at, token_estimate, model_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (id)
        DO UPDATE SET
          content = excluded.content,
          token_estimate = excluded.token_estimate,
          model_id = excluded.model_id
        WHERE messages.organization_id = excluded.organization_id
          AND messages.conversation_id = excluded.conversation_id
        "#,
    )
    .bind(message.id)
    .bind(organization_id)
    .bind(message.conversation_id)
    .bind(role)
    .bind(&message.content)
    .bind(message.created_at)
    .bind(message.token_estimate.map(|value| value as i32))
    .bind(&message.model_id)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if result.rows_affected() == 0 {
        return Err(AroError::Security(
            "message belongs to another organization or conversation".to_string(),
        ));
    }
    Ok(())
}

fn map_user_row(row: sqlx::postgres::PgRow) -> User {
    User {
        id: row.get("id"),
        email: row.get("email"),
        name: row.get("name"),
        role_title: row.get("role_title"),
        avatar_color: row.get("avatar_color"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_user_credentials_row(row: sqlx::postgres::PgRow) -> UserCredentials {
    let password_hash = row.get("password_hash");
    UserCredentials {
        user: map_user_row(row),
        password_hash,
    }
}

fn map_organization_row(row: sqlx::postgres::PgRow) -> Organization {
    Organization {
        id: row.get("id"),
        name: row.get("name"),
        domain: row.get("domain"),
        description: row.get("description"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn map_outbox_event_row(row: sqlx::postgres::PgRow) -> OutboxEvent {
    OutboxEvent {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        event_type: row.get("event_type"),
        payload: row.get("payload"),
        available_at: row.get("available_at"),
        processed_at: row.get("processed_at"),
        created_at: row.get("created_at"),
    }
}

fn map_device_row(row: sqlx::postgres::PgRow) -> AroResult<Device> {
    Ok(Device {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        platform: row.get("platform"),
        last_seen_at: row.get("last_seen_at"),
        created_at: row.get("created_at"),
    })
}

fn map_public_api_key_row(row: sqlx::postgres::PgRow) -> AroResult<PublicApiKey> {
    Ok(PublicApiKey {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        name: row.get("name"),
        prefix: row.get("prefix"),
        created_at: row.get("created_at"),
        last_used_at: row.get("last_used_at"),
    })
}

fn map_membership_row(row: sqlx::postgres::PgRow) -> AroResult<Membership> {
    Ok(Membership {
        id: row.get("id"),
        user_id: row.get("user_id"),
        organization_id: row.get("organization_id"),
        role: enum_from_string(row.get::<String, _>("role"))?,
        status: enum_from_string(row.get::<String, _>("status"))?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_organization_member_row(row: sqlx::postgres::PgRow) -> AroResult<OrganizationMember> {
    Ok(OrganizationMember {
        id: row.get("id"),
        user_id: row.get("user_id"),
        organization_id: row.get("organization_id"),
        name: row.get("name"),
        email: row.get("email"),
        role: enum_from_string(row.get::<String, _>("role"))?,
        status: enum_from_string(row.get::<String, _>("status"))?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_organization_invitation_row(
    row: sqlx::postgres::PgRow,
) -> AroResult<OrganizationInvitation> {
    let delivery_status: InvitationDeliveryStatus =
        enum_from_string(row.get::<String, _>("delivery_status"))?;
    let expires_at: DateTime<Utc> = row.get("expires_at");
    let accepted_at: Option<DateTime<Utc>> = row.get("accepted_at");
    let revoked_at: Option<DateTime<Utc>> = row.get("revoked_at");
    let status = if accepted_at.is_some() {
        OrganizationInvitationStatus::Accepted
    } else if revoked_at.is_some() {
        OrganizationInvitationStatus::Revoked
    } else if expires_at <= Utc::now() {
        OrganizationInvitationStatus::Expired
    } else {
        match &delivery_status {
            InvitationDeliveryStatus::Delivered => OrganizationInvitationStatus::Delivered,
            InvitationDeliveryStatus::Failed => OrganizationInvitationStatus::Failed,
            InvitationDeliveryStatus::Pending | InvitationDeliveryStatus::Processing => {
                OrganizationInvitationStatus::Pending
            }
        }
    };
    Ok(OrganizationInvitation {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        membership_id: row.get("membership_id"),
        email: row.get("invited_email"),
        name: row.get("invited_name"),
        role: enum_from_string(row.get::<String, _>("invited_role"))?,
        status,
        delivery_status,
        delivery_attempts: row.get("delivery_attempts"),
        expires_at,
        delivered_at: row.get("delivered_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_preferences_row(row: sqlx::postgres::PgRow) -> AroResult<UserPreferences> {
    Ok(UserPreferences {
        user_id: row.get("user_id"),
        theme: row.get("theme"),
        language: row.get("language"),
        wake_word_enabled: row.get("wake_word_enabled"),
        inference_mode: enum_from_string(row.get::<String, _>("inference_mode"))?,
        updated_at: row.get("updated_at"),
    })
}

fn map_conversation_row(row: sqlx::postgres::PgRow) -> AroResult<Conversation> {
    Ok(Conversation {
        id: row.get("id"),
        title: row.get("title"),
        mode: enum_from_string(row.get::<String, _>("mode"))?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        folder_id: row.try_get("folder_id").ok(),
        project_id: row.try_get("project_id").ok(),
        root_path: row.try_get("root_path").ok(),
    })
}

fn map_project_row(row: sqlx::postgres::PgRow) -> AroResult<Project> {
    Ok(Project {
        id: row.get("id"),
        name: row.get("name"),
        description: row.try_get("description").ok(),
        instructions: row.try_get("instructions").ok(),
        root_path: row.try_get("root_path").ok(),
        color: row.get("color"),
        icon: row.get("icon"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_folder_row(row: sqlx::postgres::PgRow) -> AroResult<Folder> {
    Ok(Folder {
        id: row.get("id"),
        project_id: row.try_get("project_id").ok(),
        name: row.get("name"),
        root_path: row.try_get("root_path").ok(),
        color: row.try_get("color").ok(),
        icon: row.get("icon"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_message_row(row: sqlx::postgres::PgRow) -> AroResult<ChatMessage> {
    Ok(ChatMessage {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        role: enum_from_string(row.get::<String, _>("role"))?,
        content: row.get("content"),
        created_at: row.get("created_at"),
        token_estimate: row
            .get::<Option<i32>, _>("token_estimate")
            .map(|value| value as u32),
        model_id: row.get("model_id"),
        attachments: Vec::new(),
        agent_run_id: None,
        steps: None,
    })
}

fn map_file_object_row(row: sqlx::postgres::PgRow) -> AroResult<FileObject> {
    Ok(FileObject {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        owner_user_id: row.get("owner_user_id"),
        original_name: row.get("original_name"),
        mime_type: row.get("mime_type"),
        size_bytes: row.get("size_bytes"),
        sha256: row.get("sha256"),
        status: enum_from_string(row.get::<String, _>("status"))?,
        scan_status: enum_from_string(row.get::<String, _>("scan_status"))?,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_file_upload_session_row(row: sqlx::postgres::PgRow) -> AroResult<FileUploadSession> {
    Ok(FileUploadSession {
        id: row.get("id"),
        file_id: row.get("file_id"),
        organization_id: row.get("organization_id"),
        owner_user_id: row.get("owner_user_id"),
        expected_name: row.get("expected_name"),
        expected_mime_type: row.get("expected_mime_type"),
        expected_size_bytes: row.get("expected_size_bytes"),
        expected_sha256: row.get("expected_sha256"),
        status: enum_from_string(row.get::<String, _>("status"))?,
        expires_at: row.get("expires_at"),
        created_at: row.get("created_at"),
    })
}

fn map_stored_file_object_row(row: sqlx::postgres::PgRow) -> AroResult<StoredFileObject> {
    Ok(StoredFileObject {
        file: FileObject {
            id: row.get("id"),
            organization_id: row.get("organization_id"),
            owner_user_id: row.get("owner_user_id"),
            original_name: row.get("original_name"),
            mime_type: row.get("mime_type"),
            size_bytes: row.get("size_bytes"),
            sha256: row.get("sha256"),
            status: enum_from_string(row.get::<String, _>("status"))?,
            scan_status: enum_from_string(row.get::<String, _>("scan_status"))?,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        },
        storage_backend: row.get("storage_backend"),
        bucket: row.get("bucket"),
        object_key: row.get("object_key"),
    })
}

fn memory_select_sql(where_clause: &str) -> String {
    format!(
        "SELECT id, client_id, content, category, scope, status, source_conversation_id, \
         source_message_ids, pinned, salience, last_used_at, created_at, updated_at \
         FROM memories {where_clause}"
    )
}

fn map_memory_row(row: sqlx::postgres::PgRow) -> AroResult<LongTermMemory> {
    Ok(LongTermMemory {
        id: row.get("id"),
        client_id: row.get("client_id"),
        content: row.get("content"),
        category: row
            .get::<Option<String>, _>("category")
            .unwrap_or_else(|| "personal".to_string()),
        scope: row.get("scope"),
        status: row.get("status"),
        source_conversation_id: row.get("source_conversation_id"),
        source_message_ids: row.get("source_message_ids"),
        pinned: row.get("pinned"),
        salience: row.get::<f32, _>("salience"),
        recall_count: row
            .try_get::<i64, _>("recall_count")
            .or_else(|_| row.try_get::<i32, _>("recall_count").map(|v| v as i64))
            .unwrap_or(0) as u32,
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_permission_profile_row(row: sqlx::postgres::PgRow) -> AroResult<PermissionProfile> {
    Ok(PermissionProfile {
        id: row.get("id"),
        name: row.get("name"),
        trusted_roots: row.get("trusted_roots"),
        allowed_domains: row.get("allowed_domains"),
        allow_read: row.get("allow_read"),
        allow_write: row.get("allow_write"),
        allow_shell: row.get("allow_shell"),
        allow_network: row.get("allow_network"),
        command_approval: enum_from_string::<PermissionCommandApproval>(
            row.get::<String, _>("command_approval"),
        )?,
        redact_secrets: row.get("redact_secrets"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_agent_run_row(row: sqlx::postgres::PgRow) -> AroResult<AgentRun> {
    Ok(AgentRun {
        id: row.get("id"),
        lane_id: row.get("lane_id"),
        conversation_id: row.get("conversation_id"),
        goal: row.get("goal"),
        mode: enum_from_string(row.get::<String, _>("mode"))?,
        status: enum_from_string(row.get::<String, _>("status"))?,
        priority: enum_from_string(row.get::<String, _>("priority"))?,
        model_provider_id: row.get("model_provider_id"),
        model_id: row.get("model_id"),
        autonomy_profile_id: row.get("autonomy_profile_id"),
        checkpoint_summary: row.get("checkpoint_summary"),
        last_error: row.get("last_error"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        heartbeat_at: row.get("heartbeat_at"),
        completed_at: row.get("completed_at"),
    })
}

fn map_agent_lane_row(row: sqlx::postgres::PgRow) -> AroResult<AgentLane> {
    Ok(AgentLane {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        title: row.get("title"),
        status: enum_from_string(row.get::<String, _>("status"))?,
        priority: enum_from_string(row.get::<String, _>("priority"))?,
        max_concurrent_runs: row.get::<i32, _>("max_concurrent_runs").max(1) as u32,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_agent_step_row(row: sqlx::postgres::PgRow) -> AroResult<AgentStep> {
    Ok(AgentStep {
        id: row.get("id"),
        run_id: row.get("run_id"),
        sequence: row.get("sequence"),
        kind: enum_from_string(row.get::<String, _>("kind"))?,
        status: enum_from_string(row.get::<String, _>("status"))?,
        title: row.get("title"),
        input: row.get("input"),
        output: row.get("output"),
        error: row.get("error"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    })
}

fn map_agent_artifact_row(row: sqlx::postgres::PgRow) -> AroResult<AgentArtifact> {
    Ok(AgentArtifact {
        id: row.get("id"),
        run_id: row.get("run_id"),
        kind: row.get("kind"),
        title: row.get("title"),
        uri: row.get("uri"),
        content: row.get("content"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    })
}

fn map_agent_context_item_row(row: sqlx::postgres::PgRow) -> AroResult<AgentContextItem> {
    Ok(AgentContextItem {
        id: row.get("id"),
        run_id: row.get("run_id"),
        conversation_id: row.get("conversation_id"),
        kind: row.get("kind"),
        title: row.get("title"),
        content: row.get("content"),
        uri: row.get("uri"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    })
}

pub fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn is_sensitive_client_state_key(key: &str) -> bool {
    matches!(
        key,
        "aro-api-keys"
            | "aro-user-plugins"
            | "aro-mcp-servers"
            | "aro-hooks"
            | "aro-longterm-memories"
    ) || key.contains("token")
        || key.contains("secret")
        || key.contains("key")
}

async fn emit_domain_event_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    organization_id: Uuid,
    actor_user_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    payload: Value,
) -> AroResult<()> {
    let audit_payload = serde_json::json!({ "payload": payload });
    let outbox_payload = serde_json::json!({
        "action": action,
        "actorUserId": actor_user_id,
        "targetType": target_type,
        "targetId": target_id,
        "payload": audit_payload["payload"],
    });
    sqlx::query(
        r#"
        INSERT INTO audit_events
          (organization_id, actor_user_id, action, target_type, target_id, data)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(organization_id)
    .bind(actor_user_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(audit_payload)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    sqlx::query(
        r#"
        INSERT INTO outbox_events (organization_id, event_type, payload)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(organization_id)
    .bind(action)
    .bind(outbox_payload)
    .execute(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    Ok(())
}

fn enum_to_string<T: Serialize>(value: &T) -> AroResult<String> {
    let raw = serde_json::to_string(value).map_err(|err| AroError::Memory(err.to_string()))?;
    Ok(raw.trim_matches('"').to_string())
}

fn enum_from_string<T: for<'de> Deserialize<'de>>(value: String) -> AroResult<T> {
    serde_json::from_str(&format!("\"{value}\"")).map_err(|err| AroError::Memory(err.to_string()))
}

fn map_sqlx(err: sqlx::Error) -> AroError {
    if let Some(code) = err
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .filter(|code| matches!(code.as_ref(), "40P01" | "40001"))
    {
        return AroError::RetryableTransaction(code.into_owned());
    }
    AroError::Memory(err.to_string())
}

fn is_client_state_decrypt_error(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .map(|error| {
            error
                .message()
                .to_ascii_lowercase()
                .contains("wrong key or corrupt data")
        })
        .unwrap_or(false)
}

fn map_migrate(err: sqlx::migrate::MigrateError) -> AroError {
    AroError::Memory(err.to_string())
}

fn value_for<'a>(payload: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| payload.get(*key))
}

fn non_empty_or_none(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn autonomy_profile_id_param(payload: &Value) -> Option<Option<Uuid>> {
    value_for(payload, &["autonomyProfileId", "autonomy_profile_id"]).map(|val| match val {
        Value::Null => None,
        Value::String(s) => Uuid::parse_str(s).ok(),
        _ => None,
    })
}

fn required_string(payload: &Value, keys: &[&str]) -> AroResult<String> {
    optional_string(payload, keys)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AroError::Configuration(format!("missing required field '{}'", keys[0])))
}

fn optional_string(payload: &Value, keys: &[&str]) -> Option<String> {
    value_for(payload, keys).and_then(|value| match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    })
}

fn optional_value_string(payload: &Value, keys: &[&str]) -> Option<String> {
    optional_string(payload, keys).and_then(|value| {
        let value = value.trim();
        if value.is_empty() {
            None
        } else {
            Some(value.to_string())
        }
    })
}

fn string_or(payload: &Value, keys: &[&str], fallback: &str) -> String {
    optional_string(payload, keys).unwrap_or_else(|| fallback.to_string())
}

fn optional_bool(payload: &Value, keys: &[&str]) -> Option<bool> {
    value_for(payload, keys).and_then(|value| match value {
        Value::Bool(value) => Some(*value),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

fn bool_or(payload: &Value, keys: &[&str], fallback: bool) -> bool {
    optional_bool(payload, keys).unwrap_or(fallback)
}

fn optional_i32(payload: &Value, keys: &[&str]) -> Option<i32> {
    value_for(payload, keys).and_then(|value| match value {
        Value::Number(value) => value.as_i64().and_then(|value| i32::try_from(value).ok()),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

fn optional_f32(payload: &Value, keys: &[&str]) -> Option<f32> {
    value_for(payload, keys).and_then(|value| match value {
        Value::Number(value) => value.as_f64().map(|value| value as f32),
        Value::String(value) => value.parse().ok(),
        _ => None,
    })
}

fn optional_datetime(payload: &Value, keys: &[&str]) -> Option<DateTime<Utc>> {
    optional_string(payload, keys).and_then(|value| {
        DateTime::parse_from_rfc3339(&value)
            .map(|value| value.with_timezone(&Utc))
            .ok()
    })
}

fn optional_uuid(payload: &Value, keys: &[&str]) -> AroResult<Option<Uuid>> {
    optional_string(payload, keys)
        .map(|value| {
            Uuid::parse_str(&value).map_err(|err| AroError::Configuration(err.to_string()))
        })
        .transpose()
}

fn optional_uuid_vec(payload: &Value, keys: &[&str]) -> Option<Vec<Uuid>> {
    value_for(payload, keys).and_then(|value| match value {
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|value| Uuid::parse_str(value).ok())
                .collect::<Vec<_>>(),
        ),
        Value::String(value) => Uuid::parse_str(value).ok().map(|value| vec![value]),
        _ => None,
    })
}

fn dedupe_uuids(mut ids: Vec<Uuid>) -> Vec<Uuid> {
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn optional_uuid_if_valid(payload: &Value, keys: &[&str]) -> Option<Uuid> {
    optional_string(payload, keys).and_then(|value| Uuid::parse_str(&value).ok())
}

fn optional_json(payload: &Value, keys: &[&str]) -> Option<Value> {
    value_for(payload, keys).cloned()
}

fn json_uuid_field(payload: &Value, key: &str) -> Option<Uuid> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn json_or(payload: &Value, keys: &[&str], fallback: Value) -> Value {
    optional_json(payload, keys).unwrap_or(fallback)
}

fn optional_string_vec(payload: &Value, keys: &[&str]) -> Option<Vec<String>> {
    value_for(payload, keys).map(|value| match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.clone()),
                Value::Number(number) => Some(number.to_string()),
                Value::Bool(value) => Some(value.to_string()),
                _ => None,
            })
            .collect(),
        Value::String(text) => text
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    })
}

fn string_vec_or(payload: &Value, keys: &[&str], fallback: Vec<String>) -> Vec<String> {
    optional_string_vec(payload, keys).unwrap_or(fallback)
}

fn split_sensitive_json(value: Value) -> (Value, Option<Value>) {
    let Some(object) = value.as_object() else {
        return (value, None);
    };

    let mut public = serde_json::Map::new();
    let mut secret = serde_json::Map::new();
    for (key, value) in object {
        if is_sensitive_field_name(key) {
            secret.insert(key.clone(), value.clone());
        } else {
            public.insert(key.clone(), value.clone());
        }
    }

    let secret = if secret.is_empty() {
        None
    } else {
        Some(Value::Object(secret))
    };
    (Value::Object(public), secret)
}

fn redact_sensitive_json(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| {
                    if is_sensitive_field_name(&key)
                        || key == "encrypted_payload"
                        || key == "encrypted_env"
                    {
                        (key, Value::String("[redacted]".to_string()))
                    } else {
                        (key, redact_sensitive_json(value))
                    }
                })
                .collect(),
        ),
        Value::Array(values) => {
            Value::Array(values.into_iter().map(redact_sensitive_json).collect())
        }
        value => value,
    }
}

fn split_plugin_payload(payload: &Value) -> (Value, Option<Value>) {
    let config_input = optional_json(payload, &["config"]).unwrap_or_else(|| serde_json::json!({}));
    let (mut public_config, config_secret) = split_sensitive_json(config_input);
    if !public_config.is_object() {
        public_config = serde_json::json!({ "value": public_config });
    }

    let mut secret = serde_json::Map::new();
    if let Some(Value::Object(config_secret)) = config_secret {
        secret.extend(config_secret);
    }

    if let Some(fields) = optional_json(payload, &["fields"]) {
        let (public_fields, secret_fields) = split_sensitive_json(fields);
        if let Some(public_object) = public_config.as_object_mut() {
            if public_fields
                .as_object()
                .is_some_and(|object| !object.is_empty())
            {
                public_object.insert("fields".to_string(), public_fields);
            }
        }
        if let Some(Value::Object(secret_fields)) = secret_fields {
            secret.extend(secret_fields);
        }
    }

    let secret = if secret.is_empty() {
        None
    } else {
        Some(Value::Object(secret))
    };
    (public_config, secret)
}

fn encrypted_payload_text(value: Value, secrets_key: Option<&str>) -> AroResult<String> {
    let _ = secrets_key.ok_or_else(|| {
        AroError::Configuration("ARO_SECRETS_KEY is required for sensitive values".to_string())
    })?;
    Ok(value.to_string())
}

fn is_sensitive_field_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("token")
        || name.contains("secret")
        || name.contains("password")
        || name.contains("private")
        || name == "api_key"
        || name.ends_with("_api_key")
        || name.ends_with("key")
}

fn compile_prompt(identity: &str, rules: &str, formatting: &str) -> String {
    let rules_section = if rules.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n[COGNITIVE GUARDRAILS]\n{rules}")
    };
    let formatting_section = if formatting.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n[OUTPUT PROTOCOL]\n{formatting}")
    };
    format!("[IDENTITY & MISSION]\n{identity}{rules_section}{formatting_section}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use aro_core::{AgentContextItem, AgentRun, AgentStep, AssistantMode, MessageRole};

    const TEST_SECRETS_KEY: &str = "aro-test-secrets-key-at-least-32-bytes-long";

    async fn postgres_store() -> AroResult<Option<AroStore>> {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            return Ok(None);
        };
        AroStore::connect(&database_url, 5).await.map(Some)
    }

    async fn create_test_principal(store: &AroStore, label: &str) -> AroResult<AuthPrincipal> {
        let unique = Uuid::new_v4();
        store
            .create_user_with_org(NewUserWithOrg {
                email: format!("{label}-{unique}@aro.local"),
                name: format!("{label} User"),
                role_title: None,
                avatar_color: None,
                password_hash: "argon2-test-hash".to_string(),
                organization_name: format!("{label} Org {unique}"),
                organization_domain: None,
                organization_description: None,
            })
            .await
    }

    fn tenant_context(user_id: Uuid, organization_id: Uuid) -> TenantContext {
        TenantContext::new(user_id, organization_id).expect("non-nil test tenant context")
    }

    #[test]
    fn tenant_context_rejects_nil_identifiers_and_exposes_valid_identity() {
        let actor_id = Uuid::new_v4();
        let organization_id = Uuid::new_v4();
        assert!(TenantContext::new(Uuid::nil(), organization_id).is_err());
        assert!(TenantContext::new(actor_id, Uuid::nil()).is_err());

        let context = TenantContext::new(actor_id, organization_id).expect("valid context");
        assert_eq!(context.actor_id(), actor_id);
        assert_eq!(context.organization_id(), organization_id);
    }

    #[tokio::test]
    async fn tenant_transaction_binds_identity_locally_and_revalidates_membership() -> AroResult<()>
    {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "tenant-transaction").await?;
        let context = tenant_context(principal.user.id, principal.active_organization.id);

        let mut tx = store.begin_tenant_tx(context).await?;
        let (actor_id, organization_id): (String, String) = sqlx::query_as(
            r#"
            SELECT current_setting('aro.actor_id'),
                   current_setting('aro.organization_id')
            "#,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        assert_eq!(actor_id, context.actor_id().to_string());
        assert_eq!(organization_id, context.organization_id().to_string());
        tx.commit().await.map_err(map_sqlx)?;

        let (actor_after, organization_after): (Option<String>, Option<String>) = sqlx::query_as(
            r#"
            SELECT NULLIF(current_setting('aro.actor_id', true), ''),
                   NULLIF(current_setting('aro.organization_id', true), '')
            "#,
        )
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert!(actor_after.is_none());
        assert!(organization_after.is_none());

        assert_security(
            store
                .begin_tenant_tx(tenant_context(principal.user.id, Uuid::new_v4()))
                .await,
            "organization access denied",
        );
        Ok(())
    }

    fn assert_security<T>(result: AroResult<T>, expected_fragment: &str) {
        match result {
            Err(AroError::Security(message)) => assert!(
                message.contains(expected_fragment),
                "expected security error containing '{expected_fragment}', got '{message}'"
            ),
            Err(error) => panic!("expected security error, got {error:?}"),
            Ok(_) => panic!("expected security error, got success"),
        }
    }

    fn assert_memory_error<T>(result: AroResult<T>, expected_fragment: &str) {
        match result {
            Err(AroError::Memory(message)) => assert!(
                message.contains(expected_fragment),
                "expected memory error containing '{expected_fragment}', got '{message}'"
            ),
            Err(error) => panic!("expected memory error, got {error:?}"),
            Ok(_) => panic!("expected memory error, got success"),
        }
    }

    #[tokio::test]
    async fn plans_require_an_accessible_conversation_and_persist_it() -> AroResult<()> {
        let Some(store) = postgres_store().await? else { return Ok(()); };
        let owner = create_test_principal(&store, "mobile-plan").await?;
        let other = create_test_principal(&store, "mobile-plan-other").await?;
        let org = owner.active_organization.id;
        let user = owner.user.id;
        let conversation = store.create_conversation(tenant_context(user, org), "Plan source".into(), AssistantMode::Chat).await?;
        assert!(store.create_plan(org, user, serde_json::json!({"title":"Missing"})).await.is_err());
        assert_security(store.create_plan(other.active_organization.id, other.user.id,
            serde_json::json!({"title":"Forbidden", "conversationId":conversation.id})).await, "conversation");
        let plan = store.create_plan(org, user, serde_json::json!({"title":"Valid", "conversationId":conversation.id})).await?;
        assert_eq!(plan["conversation_id"], serde_json::json!(conversation.id));
        Ok(())
    }

    fn test_idempotency_response(label: &str) -> IdempotencyResponse {
        IdempotencyResponse {
            status_code: 201,
            headers: BTreeMap::from([
                (
                    "content-type".to_string(),
                    vec!["application/json".to_string()],
                ),
                (
                    "location".to_string(),
                    vec![format!("/test-resources/{label}")],
                ),
            ]),
            body: serde_json::to_vec(&serde_json::json!({ "created": label }))
                .expect("test response should serialize"),
        }
    }

    #[test]
    fn hashes_are_stable_and_do_not_expose_secret() {
        let hash = hash_secret("aro_live_secret");
        assert_eq!(hash, hash_secret("aro_live_secret"));
        assert_ne!(hash, "aro_live_secret");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn idempotency_request_hashes_canonical_bytes() {
        let hash = IdempotencyRequestHash::digest(br#"{"name":"first"}"#);
        assert_eq!(hash, IdempotencyRequestHash::digest(br#"{"name":"first"}"#));
        assert_ne!(
            hash,
            IdempotencyRequestHash::digest(br#"{"name":"second"}"#)
        );
        assert_eq!(hash.as_bytes().len(), 32);
    }

    #[test]
    fn idempotency_ttl_cannot_be_shorter_than_the_execution_lease() {
        assert!(validate_idempotency_ttl(IDEMPOTENCY_EXECUTION_LEASE_SECONDS).is_ok());
        assert!(validate_idempotency_ttl(IDEMPOTENCY_EXECUTION_LEASE_SECONDS - 1).is_err());
    }

    #[test]
    fn collection_slugs_map_to_tables() {
        assert_eq!(
            PersistedCollection::from_slug("mcp").unwrap().table_name(),
            "mcp_servers"
        );
        assert_eq!(
            PersistedCollection::from_slug("teams")
                .unwrap()
                .table_name(),
            "teams"
        );
        assert_eq!(
            PersistedCollection::from_slug("plans")
                .unwrap()
                .table_name(),
            "plans"
        );
        assert!(PersistedCollection::from_slug("unknown").is_none());
    }

    #[test]
    fn sensitive_client_state_keys_are_detected() {
        assert!(is_sensitive_client_state_key("aro-api-keys"));
        assert!(is_sensitive_client_state_key("aro-user-plugins"));
        assert!(is_sensitive_client_state_key("aro-longterm-memories"));
        assert!(is_sensitive_client_state_key("aro-custom-token"));
        assert!(!is_sensitive_client_state_key("aro-theme"));
    }

    #[tokio::test]
    async fn idempotency_requests_replay_conflict_and_fence_expired_owners() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "idempotency-lifecycle").await?;
        let identity = IdempotencyRequestKey {
            organization_id: principal.active_organization.id,
            actor_id: principal.user.id,
            scope: "POST:/v1/test-resources".to_string(),
            key: format!("request-{}", Uuid::new_v4()),
        };
        let request_hash = IdempotencyRequestHash::digest(br#"{"name":"first"}"#);
        let lease = match store
            .begin_idempotency_request(&identity, request_hash, IDEMPOTENCY_DEFAULT_TTL_SECONDS)
            .await?
        {
            IdempotencyBegin::Acquired(lease) => lease,
            outcome => panic!("first request should acquire its key, got {outcome:?}"),
        };

        assert!(matches!(
            store
                .begin_idempotency_request(
                    &identity,
                    request_hash,
                    IDEMPOTENCY_DEFAULT_TTL_SECONDS,
                )
                .await?,
            IdempotencyBegin::InProgress { .. }
        ));
        assert_eq!(
            store
                .begin_idempotency_request(
                    &identity,
                    IdempotencyRequestHash::digest(br#"{"name":"conflict"}"#),
                    IDEMPOTENCY_DEFAULT_TTL_SECONDS,
                )
                .await?,
            IdempotencyBegin::Conflict
        );

        let response = test_idempotency_response("first");
        assert_eq!(
            store
                .complete_idempotency_request(&lease, &response)
                .await?,
            IdempotencyCompletion::Completed
        );
        assert_eq!(
            store
                .begin_idempotency_request(
                    &identity,
                    request_hash,
                    IDEMPOTENCY_DEFAULT_TTL_SECONDS,
                )
                .await?,
            IdempotencyBegin::Replay(response.clone())
        );
        assert_eq!(
            store
                .complete_idempotency_request(&lease, &response)
                .await?,
            IdempotencyCompletion::AlreadyCompleted(response.clone())
        );

        sqlx::query(
            r#"
            UPDATE idempotency_requests
            SET created_at = now() - interval '3 seconds',
                lease_expires_at = now() - interval '2 seconds',
                completed_at = now() - interval '2 seconds',
                expires_at = now() - interval '1 second'
            WHERE id = $1
            "#,
        )
        .bind(lease.record_id())
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;

        let replacement_hash = IdempotencyRequestHash::digest(br#"{"name":"replacement"}"#);
        let replacement = match store
            .begin_idempotency_request(&identity, replacement_hash, IDEMPOTENCY_DEFAULT_TTL_SECONDS)
            .await?
        {
            IdempotencyBegin::Acquired(lease) => lease,
            outcome => panic!("expired key should be reacquired, got {outcome:?}"),
        };
        assert_eq!(lease.record_id(), replacement.record_id());
        assert_ne!(lease.execution_token, replacement.execution_token);
        assert_eq!(
            store
                .complete_idempotency_request(&lease, &response)
                .await?,
            IdempotencyCompletion::LeaseLost
        );

        let replacement_response = test_idempotency_response("replacement");
        assert_eq!(
            store
                .complete_idempotency_request(&replacement, &replacement_response)
                .await?,
            IdempotencyCompletion::Completed
        );
        Ok(())
    }

    #[tokio::test]
    async fn stale_idempotency_execution_lease_can_be_reacquired() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "idempotency-stale-lease").await?;
        let identity = IdempotencyRequestKey {
            organization_id: principal.active_organization.id,
            actor_id: principal.user.id,
            scope: "POST:/v1/stale-lease-test".to_string(),
            key: format!("stale-{}", Uuid::new_v4()),
        };
        let request_hash = IdempotencyRequestHash::digest(b"same canonical request");
        let original = match store
            .begin_idempotency_request(&identity, request_hash, IDEMPOTENCY_DEFAULT_TTL_SECONDS)
            .await?
        {
            IdempotencyBegin::Acquired(lease) => lease,
            outcome => panic!("first request should acquire its key, got {outcome:?}"),
        };

        sqlx::query(
            r#"
            UPDATE idempotency_requests
            SET created_at = now() - interval '3 minutes',
                lease_expires_at = now() - interval '1 minute'
            WHERE id = $1
            "#,
        )
        .bind(original.record_id())
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;

        assert_eq!(
            store
                .begin_idempotency_request(
                    &identity,
                    IdempotencyRequestHash::digest(b"different canonical request"),
                    IDEMPOTENCY_DEFAULT_TTL_SECONDS,
                )
                .await?,
            IdempotencyBegin::Conflict
        );

        let replacement = match store
            .begin_idempotency_request(&identity, request_hash, IDEMPOTENCY_DEFAULT_TTL_SECONDS)
            .await?
        {
            IdempotencyBegin::Acquired(lease) => lease,
            outcome => panic!("stale execution lease should be reacquired, got {outcome:?}"),
        };
        assert_eq!(original.record_id(), replacement.record_id());
        assert_ne!(original.execution_token, replacement.execution_token);
        assert_eq!(
            store
                .complete_idempotency_request(&original, &test_idempotency_response("stale"))
                .await?,
            IdempotencyCompletion::LeaseLost
        );
        assert_eq!(
            store
                .complete_idempotency_request(
                    &replacement,
                    &test_idempotency_response("replacement"),
                )
                .await?,
            IdempotencyCompletion::Completed
        );
        Ok(())
    }

    #[tokio::test]
    async fn idempotency_acquisition_and_completion_are_concurrency_safe() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "idempotency-concurrency").await?;
        let identity = IdempotencyRequestKey {
            organization_id: principal.active_organization.id,
            actor_id: principal.user.id,
            scope: "POST:/v1/concurrent-test".to_string(),
            key: format!("concurrent-{}", Uuid::new_v4()),
        };
        let request_hash = IdempotencyRequestHash::digest(b"same canonical request");

        let (left, right) = tokio::join!(
            store.begin_idempotency_request(
                &identity,
                request_hash,
                IDEMPOTENCY_DEFAULT_TTL_SECONDS,
            ),
            store.begin_idempotency_request(
                &identity,
                request_hash,
                IDEMPOTENCY_DEFAULT_TTL_SECONDS,
            )
        );
        let mut lease = None;
        let mut in_progress = 0;
        for outcome in [left?, right?] {
            match outcome {
                IdempotencyBegin::Acquired(acquired) => lease = Some(acquired),
                IdempotencyBegin::InProgress { .. } => in_progress += 1,
                outcome => panic!("unexpected concurrent begin outcome: {outcome:?}"),
            }
        }
        assert_eq!(in_progress, 1);
        let lease = lease.expect("exactly one request should acquire the key");

        let response = test_idempotency_response("concurrent");
        let (left, right) = tokio::join!(
            store.complete_idempotency_request(&lease, &response),
            store.complete_idempotency_request(&lease, &response)
        );
        let mut completed = 0;
        let mut already_completed = 0;
        for outcome in [left?, right?] {
            match outcome {
                IdempotencyCompletion::Completed => completed += 1,
                IdempotencyCompletion::AlreadyCompleted(stored) => {
                    assert_eq!(stored, response);
                    already_completed += 1;
                }
                outcome => panic!("unexpected concurrent completion outcome: {outcome:?}"),
            }
        }
        assert_eq!(completed, 1);
        assert_eq!(already_completed, 1);
        assert_eq!(
            store
                .begin_idempotency_request(
                    &identity,
                    request_hash,
                    IDEMPOTENCY_DEFAULT_TTL_SECONDS,
                )
                .await?,
            IdempotencyBegin::Replay(response)
        );
        Ok(())
    }

    #[tokio::test]
    async fn corrupt_sensitive_client_state_is_skipped_and_soft_deleted() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "corrupt-client-state").await?;
        let user_id = principal.user.id;
        let organization_id = principal.active_organization.id;
        let old_key = "old-client-state-secret";
        let current_key = "current-client-state-secret";

        store
            .set_client_state(
                user_id,
                organization_id,
                "aro-theme".to_string(),
                serde_json::json!("dark"),
                None,
            )
            .await?;

        for (key, payload) in [
            (
                "aro-api-keys",
                serde_json::json!([{ "name": "Old key", "secret": "sk_old" }]),
            ),
            (
                "aro-custom-token",
                serde_json::json!({ "token": "old-token" }),
            ),
        ] {
            sqlx::query(
                r#"
                INSERT INTO client_state
                  (organization_id, user_id, key, value, encrypted_value, sensitive, updated_at, deleted_at)
                VALUES ($1, $2, $3, NULL, pgp_sym_encrypt($4::text, $5), true, now(), NULL)
                "#,
            )
            .bind(organization_id)
            .bind(user_id)
            .bind(key)
            .bind(payload.to_string())
            .bind(old_key)
            .execute(store.pool())
            .await
            .map_err(map_sqlx)?;
        }

        let token = store
            .get_client_state(
                user_id,
                organization_id,
                "aro-custom-token".to_string(),
                Some(current_key),
            )
            .await?;
        assert!(token.is_none());

        let state = store
            .list_client_state(user_id, organization_id, Some(current_key))
            .await?;
        assert_eq!(state.get("aro-theme"), Some(&serde_json::json!("dark")));
        assert!(!state.contains_key("aro-api-keys"));
        assert!(!state.contains_key("aro-custom-token"));

        for key in ["aro-api-keys", "aro-custom-token"] {
            let was_soft_deleted: bool = sqlx::query_scalar(
                r#"
                SELECT deleted_at IS NOT NULL
                FROM client_state
                WHERE organization_id = $1 AND user_id = $2 AND key = $3
                "#,
            )
            .bind(organization_id)
            .bind(user_id)
            .bind(key)
            .fetch_one(store.pool())
            .await
            .map_err(map_sqlx)?;
            assert!(was_soft_deleted, "{key} should be soft deleted");
        }

        Ok(())
    }

    #[tokio::test]
    async fn postgres_round_trips_auth_tenant_state_and_resource_crud() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let unique = Uuid::new_v4();
        let principal = create_test_principal(&store, "test").await?;

        let secret_key = "integration-secret";
        let (second_org, _) = store
            .create_organization_for_user(
                principal.user.id,
                format!("Second Org {unique}"),
                None,
                None,
            )
            .await?;
        let refresh_token = format!("refresh-{unique}");
        store
            .create_refresh_token(
                principal.user.id,
                second_org.id,
                None,
                &refresh_token,
                7,
                90,
            )
            .await?;
        let consumed = store
            .consume_refresh_token(&refresh_token)
            .await?
            .expect("refresh token principal");
        assert_eq!(consumed.user_id, principal.user.id);
        assert_eq!(consumed.organization_id, Some(second_org.id));
        assert!(store.consume_refresh_token(&refresh_token).await?.is_none());
        let scoped_principal = store
            .principal_for_user_in_org(principal.user.id, second_org.id)
            .await?
            .expect("scoped principal");
        assert_eq!(scoped_principal.active_organization.id, second_org.id);

        store
            .set_client_state(
                principal.user.id,
                principal.active_organization.id,
                "aro-api-keys".to_string(),
                serde_json::json!([{ "name": "Test", "secret": "sk_test" }]),
                Some(secret_key),
            )
            .await?;
        let state = store
            .get_client_state(
                principal.user.id,
                principal.active_organization.id,
                "aro-api-keys".to_string(),
                Some(secret_key),
            )
            .await?
            .expect("client state");
        assert_eq!(state[0]["secret"], "sk_test");

        let memory = store
            .create_collection_item(
                principal.user.id,
                principal.active_organization.id,
                PersistedCollection::Memories,
                serde_json::json!({
                    "content": "remember this",
                    "category": "personal",
                    "pinned": true
                }),
                Some(secret_key),
            )
            .await?;
        let memory_id = Uuid::parse_str(memory["id"].as_str().expect("id")).expect("uuid");
        let updated = store
            .update_collection_item(
                principal.user.id,
                principal.active_organization.id,
                PersistedCollection::Memories,
                memory_id,
                serde_json::json!({ "content": "updated memory" }),
                Some(secret_key),
            )
            .await?;
        assert_eq!(updated["content"], "updated memory");

        let api_key_secret = format!("aro_live_test_secret_{unique}");
        let api_key_prefix = api_key_secret.chars().take(14).collect::<String>();
        let api_key = store
            .create_api_key(
                principal.user.id,
                principal.active_organization.id,
                "Integration key".to_string(),
                api_key_prefix,
                hash_secret(&api_key_secret),
            )
            .await?;
        store
            .revoke_api_key(
                principal.user.id,
                principal.active_organization.id,
                api_key.id,
            )
            .await?;

        let other = store
            .create_user_with_org(NewUserWithOrg {
                email: format!("other-{unique}@aro.local"),
                name: "Other User".to_string(),
                role_title: None,
                avatar_color: None,
                password_hash: "argon2-test-hash".to_string(),
                organization_name: format!("Other Org {unique}"),
                organization_domain: None,
                organization_description: None,
            })
            .await?;
        assert!(store
            .ensure_org_access(other.user.id, principal.active_organization.id)
            .await
            .is_err());

        store
            .delete_collection_item(
                principal.user.id,
                principal.active_organization.id,
                PersistedCollection::Memories,
                memory_id,
            )
            .await?;
        assert!(store
            .get_collection_item(
                principal.user.id,
                principal.active_organization.id,
                PersistedCollection::Memories,
                memory_id,
            )
            .await?
            .is_none());

        assert_eq!(
            store
                .count_audit_events_for_org(principal.active_organization.id)
                .await?,
            5
        );
        assert_eq!(
            store
                .count_outbox_events_for_org(principal.active_organization.id)
                .await?,
            5
        );
        let pending_events = store
            .list_pending_outbox_events(principal.user.id, principal.active_organization.id, 10)
            .await?;
        assert_eq!(pending_events.len(), 5);
        let outbox_json = serde_json::to_string(&pending_events)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        assert!(!outbox_json.contains("remember this"));
        assert!(!outbox_json.contains("updated memory"));
        assert!(outbox_json.contains("contentHash"));

        let audit_payloads: Vec<Value> = sqlx::query_scalar(
            "SELECT data FROM audit_events WHERE organization_id = $1 ORDER BY created_at ASC",
        )
        .bind(principal.active_organization.id)
        .fetch_all(store.pool())
        .await
        .map_err(map_sqlx)?;
        let audit_json = serde_json::to_string(&audit_payloads)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        assert!(!audit_json.contains("remember this"));
        assert!(!audit_json.contains("updated memory"));
        assert!(audit_json.contains("contentHash"));

        let processed_event = store
            .mark_outbox_event_processed(
                principal.user.id,
                principal.active_organization.id,
                pending_events[0].id,
            )
            .await?
            .expect("processed outbox event");
        assert!(processed_event.processed_at.is_some());
        let remaining_events = store
            .list_pending_outbox_events(principal.user.id, principal.active_organization.id, 10)
            .await?;
        assert_eq!(remaining_events.len(), 4);
        assert!(!remaining_events
            .iter()
            .any(|event| event.id == processed_event.id));

        Ok(())
    }

    #[tokio::test]
    async fn owner_membership_cannot_be_demoted_through_invite() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "owner-invite").await?;

        let result = store
            .invite_organization_member(
                principal.user.id,
                principal.active_organization.id,
                NewOrganizationMember {
                    email: principal.user.email.clone(),
                    name: "Demoted Owner".to_string(),
                    role: MembershipRole::Member,
                },
                TEST_SECRETS_KEY,
            )
            .await;
        assert_security(result, "owner membership");

        let membership = store
            .ensure_org_access(principal.user.id, principal.active_organization.id)
            .await?;
        assert_eq!(membership.role, MembershipRole::Owner);

        Ok(())
    }

    #[tokio::test]
    async fn soft_deleted_users_and_organizations_are_denied_org_access() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };

        let deleted_org_principal = create_test_principal(&store, "deleted-org").await?;
        sqlx::query("UPDATE organizations SET deleted_at = now() WHERE id = $1")
            .bind(deleted_org_principal.active_organization.id)
            .execute(store.pool())
            .await
            .map_err(map_sqlx)?;
        assert_security(
            store
                .ensure_org_access(
                    deleted_org_principal.user.id,
                    deleted_org_principal.active_organization.id,
                )
                .await,
            "organization access denied",
        );

        let deleted_user_principal = create_test_principal(&store, "deleted-user").await?;
        sqlx::query("UPDATE users SET deleted_at = now() WHERE id = $1")
            .bind(deleted_user_principal.user.id)
            .execute(store.pool())
            .await
            .map_err(map_sqlx)?;
        assert_security(
            store
                .ensure_org_access(
                    deleted_user_principal.user.id,
                    deleted_user_principal.active_organization.id,
                )
                .await,
            "organization access denied",
        );
        assert!(store
            .principal_for_user(deleted_user_principal.user.id)
            .await?
            .is_none());

        Ok(())
    }

    #[tokio::test]
    async fn conversations_and_agent_runs_are_private_within_an_organization() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let unique = Uuid::new_v4();
        let owner = create_test_principal(&store, "private-owner").await?;
        let member = create_test_principal(&store, "private-member").await?;
        let member_role = enum_to_string(&MembershipRole::Member)?;
        sqlx::query(
            r#"
            INSERT INTO memberships (user_id, organization_id, role, status)
            VALUES ($1, $2, $3, 'active')
            ON CONFLICT (user_id, organization_id)
            DO UPDATE SET role = excluded.role, status = excluded.status, deleted_at = NULL
            "#,
        )
        .bind(member.user.id)
        .bind(owner.active_organization.id)
        .bind(member_role)
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;

        let conversation = store
            .create_conversation(
                tenant_context(owner.user.id, owner.active_organization.id),
                format!("private conversation {unique}"),
                AssistantMode::Chat,
            )
            .await?;
        assert!(store
            .list_conversations(tenant_context(member.user.id, owner.active_organization.id,))
            .await?
            .is_empty());
        assert!(store
            .get_conversation(
                tenant_context(member.user.id, owner.active_organization.id),
                conversation.id,
            )
            .await?
            .is_none());
        assert_memory_error(
            store
                .update_conversation_title(
                    tenant_context(member.user.id, owner.active_organization.id),
                    conversation.id,
                    "attempted update".to_string(),
                )
                .await,
            "conversation not found",
        );
        assert_security(
            store
                .add_message(
                    tenant_context(member.user.id, owner.active_organization.id),
                    &ChatMessage::new(conversation.id, MessageRole::User, "attempted read/write"),
                )
                .await,
            "conversation access denied",
        );

        let lane = store
            .ensure_agent_lane(
                owner.user.id,
                owner.active_organization.id,
                Some(conversation.id),
                "Private lane",
            )
            .await?;
        let mut run = AgentRun::new(
            "private run",
            AssistantMode::Chat,
            Some(conversation.id),
            None,
            None,
            None,
        );
        run.lane_id = Some(lane.id);
        let run = store
            .upsert_agent_run(owner.user.id, owner.active_organization.id, &run)
            .await?;
        assert_eq!(
            store
                .list_agent_runs_for_lane(owner.user.id, owner.active_organization.id, lane.id, 10,)
                .await?
                .len(),
            1
        );
        assert_eq!(
            store
                .list_agent_lane_views(owner.user.id, owner.active_organization.id)
                .await?
                .len(),
            1
        );
        assert!(store
            .list_agent_runs_for_lane(member.user.id, owner.active_organization.id, lane.id, 10)
            .await?
            .is_empty());
        assert!(store
            .list_agent_lane_views(member.user.id, owner.active_organization.id)
            .await?
            .is_empty());
        assert!(store
            .get_agent_run(member.user.id, owner.active_organization.id, run.id)
            .await?
            .is_none());
        assert_security(
            store
                .add_agent_step(
                    member.user.id,
                    owner.active_organization.id,
                    &AgentStep::completed(
                        run.id,
                        1,
                        aro_core::AgentStepKind::RunStarted,
                        "attempted step",
                        serde_json::json!({}),
                        serde_json::json!({}),
                    ),
                )
                .await,
            "agent run access denied",
        );
        store
            .add_agent_context_item(
                owner.user.id,
                owner.active_organization.id,
                &AgentContextItem {
                    id: Uuid::new_v4(),
                    run_id: Some(run.id),
                    conversation_id: Some(conversation.id),
                    kind: "test".to_string(),
                    title: "Private context".to_string(),
                    content: "confidential context".to_string(),
                    uri: None,
                    metadata: serde_json::json!({}),
                    created_at: Utc::now(),
                },
            )
            .await?;
        assert_eq!(
            store
                .search_agent_context(
                    owner.user.id,
                    owner.active_organization.id,
                    "confidential",
                    10,
                )
                .await?
                .len(),
            1
        );
        assert!(store
            .search_agent_context(
                member.user.id,
                owner.active_organization.id,
                "confidential",
                10,
            )
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn file_scan_jobs_are_leased_and_gate_file_availability() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "file-scan-job").await?;
        let user_id = principal.user.id;
        let organization_id = principal.active_organization.id;
        let make_upload = |name: &str| NewFileUpload {
            file_id: Uuid::new_v4(),
            original_name: name.to_string(),
            mime_type: "text/plain".to_string(),
            size_bytes: 12,
            sha256: Some(hash_secret("scan-test-bytes")),
            storage_backend: "local".to_string(),
            bucket: None,
            object_key: format!("organizations/{organization_id}/files/{name}"),
            expires_at: Utc::now() + Duration::minutes(5),
        };

        let blocked = store
            .create_file_upload_session(user_id, organization_id, make_upload("blocked"))
            .await?;
        let blocked_file = store
            .complete_file_upload_session(
                user_id,
                organization_id,
                blocked.upload.id,
                12,
                hash_secret("scan-test-bytes"),
                "text/plain".to_string(),
                None,
            )
            .await?;
        assert_eq!(blocked_file.status, FileStatus::Pending);
        let first_lease = Uuid::new_v4();
        let claimed = store
            .claim_pending_file_scan_jobs(10, first_lease, 60, 3)
            .await?;
        assert_eq!(claimed.len(), 1);
        assert!(store
            .claim_pending_file_scan_jobs(10, Uuid::new_v4(), 60, 3)
            .await?
            .is_empty());
        let quarantined = store
            .quarantine_file_scan_job(claimed[0].id, claimed[0].lease_token)
            .await?
            .expect("scanner lease should quarantine the file");
        assert_eq!(quarantined.status, FileStatus::Quarantined);
        assert_eq!(quarantined.scan_status, FileScanStatus::Blocked);

        let clean = store
            .create_file_upload_session(user_id, organization_id, make_upload("clean"))
            .await?;
        let clean_file = store
            .complete_file_upload_session(
                user_id,
                organization_id,
                clean.upload.id,
                12,
                hash_secret("scan-test-bytes"),
                "text/plain".to_string(),
                None,
            )
            .await?;
        let claimed = store
            .claim_pending_file_scan_jobs(10, Uuid::new_v4(), 60, 3)
            .await?;
        assert_eq!(claimed.len(), 1);
        let available = store
            .acknowledge_file_scan_clean(claimed[0].id, claimed[0].lease_token)
            .await?
            .expect("scanner lease should approve the file");
        assert_eq!(available.id, clean_file.id);
        assert_eq!(available.status, FileStatus::Available);
        assert_eq!(available.scan_status, FileScanStatus::Clean);
        assert!(store
            .acknowledge_file_scan_clean(claimed[0].id, claimed[0].lease_token)
            .await?
            .is_none());
        let index_jobs: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM file_index_jobs WHERE organization_id = $1 AND file_id = $2",
        )
        .bind(organization_id)
        .bind(clean_file.id)
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(index_jobs, 1);
        Ok(())
    }

    #[tokio::test]
    async fn file_scan_jobs_can_be_leased_just_in_time_one_by_one() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "file-scan-jit-lease").await?;
        let user_id = principal.user.id;
        let organization_id = principal.active_organization.id;
        let mut file_ids = Vec::new();

        for name in ["first", "second"] {
            let upload = store
                .create_file_upload_session(
                    user_id,
                    organization_id,
                    NewFileUpload {
                        file_id: Uuid::new_v4(),
                        original_name: format!("{name}.txt"),
                        mime_type: "text/plain".to_string(),
                        size_bytes: 12,
                        sha256: Some(hash_secret("scan-test-bytes")),
                        storage_backend: "local".to_string(),
                        bucket: None,
                        object_key: format!(
                            "organizations/{organization_id}/files/scan-jit-{name}"
                        ),
                        expires_at: Utc::now() + Duration::minutes(5),
                    },
                )
                .await?;
            let file = store
                .complete_file_upload_session(
                    user_id,
                    organization_id,
                    upload.upload.id,
                    12,
                    hash_secret("scan-test-bytes"),
                    "text/plain".to_string(),
                    None,
                )
                .await?;
            file_ids.push(file.id);
        }

        let first_token = Uuid::new_v4();
        let first_claim = store
            .claim_pending_file_scan_jobs(1, first_token, 60, 3)
            .await?;
        assert_eq!(first_claim.len(), 1);
        assert_eq!(first_claim[0].lease_token, first_token);

        let waiting = sqlx::query(
            r#"
            SELECT file_id, status, attempts, lease_token
            FROM file_scan_jobs
            WHERE organization_id = $1
              AND file_id = ANY($2)
              AND id <> $3
            "#,
        )
        .bind(organization_id)
        .bind(&file_ids)
        .bind(first_claim[0].id)
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        let waiting_file_id: Uuid = waiting.get("file_id");
        assert_eq!(waiting.get::<String, _>("status"), "queued");
        assert_eq!(waiting.get::<i32, _>("attempts"), 0);
        assert_eq!(waiting.get::<Option<Uuid>, _>("lease_token"), None);

        store
            .quarantine_file_scan_job(first_claim[0].id, first_claim[0].lease_token)
            .await?
            .expect("the first just-in-time lease should remain valid");

        let second_token = Uuid::new_v4();
        let second_claim = store
            .claim_pending_file_scan_jobs(1, second_token, 60, 3)
            .await?;
        assert_eq!(second_claim.len(), 1);
        assert_eq!(second_claim[0].file_id, waiting_file_id);
        assert_eq!(second_claim[0].lease_token, second_token);
        store
            .acknowledge_file_scan_clean(second_claim[0].id, second_claim[0].lease_token)
            .await?
            .expect("the second file should receive its own fresh lease");
        Ok(())
    }

    #[tokio::test]
    async fn issuing_new_invitation_does_not_reserve_global_identity() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let owner = create_test_principal(&store, "invitation-principal").await?;
        let email = format!("invited-{}@aro.local", Uuid::new_v4());
        let invited = store
            .invite_organization_member(
                owner.user.id,
                owner.active_organization.id,
                NewOrganizationMember {
                    email: email.clone(),
                    name: "Invited person".to_string(),
                    role: MembershipRole::Member,
                },
                TEST_SECRETS_KEY,
            )
            .await?;
        assert!(invited.member.is_none());
        assert!(store.find_user_credentials(&email).await?.is_none());
        let stored: (Option<Uuid>, Option<Uuid>, bool) = sqlx::query_as(
            r#"
            SELECT invited_user_id, membership_id, delivery_token_encrypted IS NOT NULL
            FROM organization_invitations
            WHERE id = $1
            "#,
        )
        .bind(invited.id)
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(stored, (None, None, true));
        Ok(())
    }

    #[tokio::test]
    async fn organization_admins_can_page_and_revoke_membershipless_invitations() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let issuer = create_test_principal(&store, "invitation-lifecycle").await?;
        let email = format!("lifecycle-{}@aro.local", Uuid::new_v4());
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let issued = store
            .issue_organization_invitation(
                issuer.user.id,
                issuer.active_organization.id,
                NewOrganizationMember {
                    email: email.clone(),
                    name: "Lifecycle invitee".to_string(),
                    role: MembershipRole::Guest,
                },
                token.clone(),
                Utc::now() + Duration::hours(1),
                TEST_SECRETS_KEY,
            )
            .await?;
        let expired_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let expired = store
            .issue_organization_invitation(
                issuer.user.id,
                issuer.active_organization.id,
                NewOrganizationMember {
                    email: format!("expired-{email}"),
                    name: "Expired invitee".to_string(),
                    role: MembershipRole::Guest,
                },
                expired_token,
                Utc::now() + Duration::hours(1),
                TEST_SECRETS_KEY,
            )
            .await?;
        sqlx::query(
            "UPDATE organization_invitations SET expires_at = now() - interval '1 minute' WHERE id = $1",
        )
        .bind(expired.id)
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;

        let active = store
            .list_organization_invitations(
                issuer.user.id,
                issuer.active_organization.id,
                None,
                false,
                50,
            )
            .await?;
        assert!(active.iter().all(|invitation| invitation.id != expired.id));
        let listed = active
            .iter()
            .find(|invitation| invitation.id == issued.id)
            .expect("active invitation is listed");
        assert_eq!(listed.email, email);
        assert_eq!(listed.status, OrganizationInvitationStatus::Pending);

        store
            .revoke_organization_invitation(
                issuer.user.id,
                issuer.active_organization.id,
                issued.id,
            )
            .await?;
        let active_after = store
            .list_organization_invitations(
                issuer.user.id,
                issuer.active_organization.id,
                None,
                false,
                50,
            )
            .await?;
        assert!(active_after
            .iter()
            .all(|invitation| invitation.id != issued.id));
        let closed = store
            .list_organization_invitations(
                issuer.user.id,
                issuer.active_organization.id,
                None,
                true,
                50,
            )
            .await?;
        assert!(closed.iter().any(|invitation| {
            invitation.id == issued.id && invitation.status == OrganizationInvitationStatus::Revoked
        }));
        assert!(closed.iter().any(|invitation| {
            invitation.id == expired.id
                && invitation.status == OrganizationInvitationStatus::Expired
        }));
        assert_security(
            store
                .accept_new_organization_invitation(&token, "never-created".to_string())
                .await,
            "invitation is invalid",
        );
        assert!(store.find_user_credentials(&email).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn invitation_cannot_overwrite_account_registered_after_issue() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let issuer = create_test_principal(&store, "invite-race-issuer").await?;
        let email = format!("invite-race-{}@aro.local", Uuid::new_v4());
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .issue_organization_invitation(
                issuer.user.id,
                issuer.active_organization.id,
                NewOrganizationMember {
                    email: email.clone(),
                    name: "Mailbox owner".to_string(),
                    role: MembershipRole::Member,
                },
                token.clone(),
                Utc::now() + Duration::hours(1),
                TEST_SECRETS_KEY,
            )
            .await?;

        let registered = store
            .create_user_with_org(NewUserWithOrg {
                email: email.clone(),
                name: "Real mailbox owner".to_string(),
                role_title: None,
                avatar_color: None,
                password_hash: "real-password-hash".to_string(),
                organization_name: format!("Personal org {}", Uuid::new_v4()),
                organization_domain: None,
                organization_description: None,
            })
            .await?;
        assert_security(
            store
                .accept_new_organization_invitation(&token, "attacker-password-hash".to_string())
                .await,
            "account now exists",
        );
        let password_after = store
            .find_user_credentials(&email)
            .await?
            .expect("registered credentials")
            .password_hash;
        assert_eq!(password_after, "real-password-hash");

        let current_refresh = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                registered.user.id,
                registered.active_organization.id,
                None,
                &current_refresh,
                7,
                90,
            )
            .await?;
        let invitation_successor =
            format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let accepted = store
            .accept_existing_organization_invitation(
                registered.user.id,
                &token,
                &current_refresh,
                &invitation_successor,
                7,
            )
            .await?;
        assert_eq!(accepted.organization_id, issuer.active_organization.id);
        let family_state: (Uuid, Uuid, bool) = sqlx::query_as(
            r#"
            SELECT current.family_id, successor.family_id, current.rotated_at IS NOT NULL
            FROM refresh_tokens current
            JOIN refresh_tokens successor ON successor.token_hash = $2
            WHERE current.token_hash = $1
            "#,
        )
        .bind(hash_secret(&current_refresh))
        .bind(hash_secret(&invitation_successor))
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(family_state.0, family_state.1);
        assert!(family_state.2);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_rotation_is_atomic_and_bound_to_authenticated_user() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-rotation").await?;
        let (second_org, _) = store
            .create_organization_for_user(
                principal.user.id,
                format!("Second refresh org {}", Uuid::new_v4()),
                None,
                None,
            )
            .await?;
        let current = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let successor = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &current,
                7,
                90,
            )
            .await?;
        let absolute_cap = Utc::now() + Duration::hours(1);
        sqlx::query(
            r#"
            UPDATE refresh_token_families
            SET absolute_expires_at = $2
            WHERE id = (SELECT family_id FROM refresh_tokens WHERE token_hash = $1)
            "#,
        )
        .bind(hash_secret(&current))
        .bind(absolute_cap)
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;
        let rotated = store
            .rotate_refresh_token(
                &current,
                &successor,
                Some(second_org.id),
                Some(principal.user.id),
                7,
            )
            .await?
            .expect("refresh should rotate");
        assert_eq!(rotated.organization_id, second_org.id);
        assert!(rotated.expires_at <= absolute_cap);
        let current_was_rotated: bool = sqlx::query_scalar(
            "SELECT revoked_at IS NOT NULL AND rotated_at IS NOT NULL FROM refresh_tokens WHERE token_hash = $1",
        )
        .bind(hash_secret(&current))
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert!(current_was_rotated);
        let successor_principal = store
            .consume_refresh_token(&successor)
            .await?
            .expect("successor should be valid");
        assert_eq!(successor_principal.user_id, principal.user.id);
        assert_eq!(successor_principal.organization_id, Some(second_org.id));

        let protected = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &protected,
                7,
                90,
            )
            .await?;
        assert_security(
            store
                .rotate_refresh_token(
                    &protected,
                    &format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
                    None,
                    Some(Uuid::new_v4()),
                    7,
                )
                .await,
            "authenticated user",
        );
        assert!(store.consume_refresh_token(&protected).await?.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn rotated_refresh_token_reuse_revokes_its_entire_family() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-reuse").await?;
        let current = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let successor = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let rejected_successor = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &current,
                7,
                90,
            )
            .await?;
        assert!(store
            .rotate_refresh_token(&current, &successor, None, None, 7)
            .await?
            .is_some());

        assert!(store
            .rotate_refresh_token(&current, &rejected_successor, None, Some(Uuid::new_v4()), 7,)
            .await?
            .is_none());
        assert!(store.consume_refresh_token(&successor).await?.is_none());

        let compromise_state: (bool, bool, bool, bool) = sqlx::query_as(
            r#"
            SELECT reused.reuse_detected_at IS NOT NULL,
                   descendant.revoked_at IS NOT NULL,
                   reused.family_id = descendant.family_id,
                   family.reuse_detected_at IS NOT NULL
            FROM refresh_tokens reused
            JOIN refresh_tokens descendant ON descendant.token_hash = $2
            JOIN refresh_token_families family ON family.id = reused.family_id
            WHERE reused.token_hash = $1
            "#,
        )
        .bind(hash_secret(&current))
        .bind(hash_secret(&successor))
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(compromise_state, (true, true, true, true));
        let rejected_was_not_persisted: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE token_hash = $1")
                .bind(hash_secret(&rejected_successor))
                .fetch_one(store.pool())
                .await
                .map_err(map_sqlx)?;
        assert_eq!(rejected_was_not_persisted, 0);
        Ok(())
    }

    #[tokio::test]
    async fn simple_refresh_token_revocation_is_not_reported_as_reuse() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-logout").await?;
        let current = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &current,
                7,
                90,
            )
            .await?;
        store.revoke_refresh_token(&current).await?;
        assert!(store
            .rotate_refresh_token(
                &current,
                &format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
                None,
                None,
                7,
            )
            .await?
            .is_none());

        let revocation_state: (bool, bool, bool, bool) = sqlx::query_as(
            r#"
            SELECT token.revoked_at IS NOT NULL, token.rotated_at IS NOT NULL,
                   token.reuse_detected_at IS NOT NULL, family.revoked_at IS NOT NULL
            FROM refresh_tokens token
            JOIN refresh_token_families family ON family.id = token.family_id
            WHERE token.token_hash = $1
            "#,
        )
        .bind(hash_secret(&current))
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(revocation_state, (true, false, false, true));
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_logout_revokes_a_refresh_successor_if_rotation_wins() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-logout-race").await?;
        let current = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let successor = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &current,
                7,
                90,
            )
            .await?;

        let (rotation, logout) = tokio::join!(
            store.rotate_refresh_token(&current, &successor, None, None, 7),
            store.revoke_refresh_token(&current),
        );
        rotation?;
        logout?;

        let active_family_tokens: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM refresh_tokens family_token
            WHERE family_token.family_id = (
              SELECT family_id FROM refresh_tokens WHERE token_hash = $1
            )
              AND family_token.revoked_at IS NULL
              AND family_token.expires_at > now()
            "#,
        )
        .bind(hash_secret(&current))
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(active_family_tokens, 0);
        Ok(())
    }

    #[tokio::test]
    async fn refresh_family_maintenance_purges_only_complete_retained_lineages() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-family-purge").await?;
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &token,
                7,
                90,
            )
            .await?;
        let family_id: Uuid =
            sqlx::query_scalar("SELECT family_id FROM refresh_tokens WHERE token_hash = $1")
                .bind(hash_secret(&token))
                .fetch_one(store.pool())
                .await
                .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            UPDATE refresh_token_families
            SET created_at = now() - interval '40 days',
                absolute_expires_at = now() - interval '31 days'
            WHERE id = $1
            "#,
        )
        .bind(family_id)
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;

        assert!(store.purge_refresh_token_families(100, 30).await? >= 1);
        let remaining_tokens: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM refresh_tokens WHERE family_id = $1")
                .bind(family_id)
                .fetch_one(store.pool())
                .await
                .map_err(map_sqlx)?;
        assert_eq!(remaining_tokens, 0);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_refresh_inserts_create_and_respect_absolute_families() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-family-compat").await?;
        let first_expiry = Utc::now() + Duration::days(7);
        let first_hash = hash_secret(&format!("legacy-first-{}", Uuid::new_v4()));

        // This intentionally matches the pre-family application INSERT: family_id is omitted and
        // supplied by the column default. The compatibility trigger must create the parent row.
        let (family_id, persisted_first_expiry): (Uuid, DateTime<Utc>) = sqlx::query_as(
            r#"
            INSERT INTO refresh_tokens
              (user_id, organization_id, device_id, token_hash, expires_at)
            VALUES ($1, $2, NULL, $3, $4)
            RETURNING family_id, expires_at
            "#,
        )
        .bind(principal.user.id)
        .bind(principal.active_organization.id)
        .bind(first_hash)
        .bind(first_expiry)
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert!(
            (persisted_first_expiry - first_expiry)
                .num_microseconds()
                .is_some_and(|delta| delta.abs() <= 1),
            "PostgreSQL may only truncate sub-microsecond precision"
        );

        let family: (Uuid, Option<Uuid>, DateTime<Utc>) = sqlx::query_as(
            "SELECT user_id, device_id, absolute_expires_at FROM refresh_token_families WHERE id = $1",
        )
        .bind(family_id)
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(family.0, principal.user.id);
        assert_eq!(family.1, None);
        assert_eq!(family.2, persisted_first_expiry);

        // A previous-release replica would calculate a new TTL on rotation. The bridge keeps the
        // insert compatible but clamps it to the already-established absolute family expiry.
        let successor_expiry: DateTime<Utc> = sqlx::query_scalar(
            r#"
            INSERT INTO refresh_tokens
              (user_id, organization_id, device_id, token_hash, expires_at, family_id)
            VALUES ($1, $2, NULL, $3, $4, $5)
            RETURNING expires_at
            "#,
        )
        .bind(principal.user.id)
        .bind(principal.active_organization.id)
        .bind(hash_secret(&format!("legacy-successor-{}", Uuid::new_v4())))
        .bind(Utc::now() + Duration::days(30))
        .bind(family_id)
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(successor_expiry, family.2);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_refresh_insert_serializes_with_family_revocation() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-family-revocation-race").await?;
        let current = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &current,
                7,
                90,
            )
            .await?;
        let family_id: Uuid =
            sqlx::query_scalar("SELECT family_id FROM refresh_tokens WHERE token_hash = $1")
                .bind(hash_secret(&current))
                .fetch_one(store.pool())
                .await
                .map_err(map_sqlx)?;

        let mut revocation = store.pool().begin().await.map_err(map_sqlx)?;
        sqlx::query(
            "UPDATE refresh_token_families SET revoked_at = now(), updated_at = now() WHERE id = $1",
        )
        .bind(family_id)
        .execute(&mut *revocation)
        .await
        .map_err(map_sqlx)?;

        let insert_pool = store.pool().clone();
        let organization_id = principal.active_organization.id;
        let user_id = principal.user.id;
        let mut legacy_insert = tokio::spawn(async move {
            sqlx::query(
                r#"
                INSERT INTO refresh_tokens
                  (user_id, organization_id, device_id, token_hash, expires_at, family_id)
                VALUES ($1, $2, NULL, $3, $4, $5)
                "#,
            )
            .bind(user_id)
            .bind(organization_id)
            .bind(hash_secret(&format!("legacy-racing-{}", Uuid::new_v4())))
            .bind(Utc::now() + Duration::days(7))
            .bind(family_id)
            .execute(&insert_pool)
            .await
        });

        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(200), &mut legacy_insert)
                .await
                .is_err(),
            "the compatibility trigger must wait for the family revocation lock"
        );
        revocation.commit().await.map_err(map_sqlx)?;

        let error = legacy_insert
            .await
            .map_err(|error| AroError::Unexpected(format!("legacy insert task failed: {error}")))?
            .expect_err("a legacy insert must be rejected after family revocation commits");
        assert_eq!(
            error
                .as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .as_deref(),
            Some("23514")
        );
        Ok(())
    }

    #[tokio::test]
    async fn legacy_refresh_reuse_revokes_the_family_and_all_descendants() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-family-legacy-reuse").await?;
        let current = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let successor = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &current,
                7,
                90,
            )
            .await?;
        store
            .rotate_refresh_token(&current, &successor, None, None, 7)
            .await?
            .expect("the initial rotation should succeed");

        // This matches the meaningful transition emitted by a previous-release reuse path: only
        // the token table records the detection. The statement trigger must bridge the family.
        sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET reuse_detected_at = now()
            WHERE token_hash = $1
              AND reuse_detected_at IS NULL
            "#,
        )
        .bind(hash_secret(&current))
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;

        let state: (bool, bool, bool) = sqlx::query_as(
            r#"
            SELECT family.revoked_at IS NOT NULL,
                   family.reuse_detected_at IS NOT NULL,
                   successor.revoked_at IS NOT NULL
            FROM refresh_tokens predecessor
            JOIN refresh_token_families family ON family.id = predecessor.family_id
            JOIN refresh_tokens successor ON successor.token_hash = $2
            WHERE predecessor.token_hash = $1
            "#,
        )
        .bind(hash_secret(&current))
        .bind(hash_secret(&successor))
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(state, (true, true, true));
        assert!(store.consume_refresh_token(&successor).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_refresh_rotation_detects_reuse_and_leaves_no_valid_successor(
    ) -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "refresh-concurrent").await?;
        let current = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let left_successor = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let right_successor = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                principal.user.id,
                principal.active_organization.id,
                None,
                &current,
                7,
                90,
            )
            .await?;

        let (left, right) = tokio::join!(
            store.rotate_refresh_token(&current, &left_successor, None, None, 7),
            store.rotate_refresh_token(&current, &right_successor, None, None, 7),
        );
        let left = left?;
        let right = right?;
        assert_ne!(left.is_some(), right.is_some());
        assert!(store
            .consume_refresh_token(&left_successor)
            .await?
            .is_none());
        assert!(store
            .consume_refresh_token(&right_successor)
            .await?
            .is_none());

        let persisted_successors: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM refresh_tokens WHERE token_hash = $1 OR token_hash = $2",
        )
        .bind(hash_secret(&left_successor))
        .bind(hash_secret(&right_successor))
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert_eq!(persisted_successors, 1);
        let reuse_detected: bool = sqlx::query_scalar(
            "SELECT reuse_detected_at IS NOT NULL FROM refresh_tokens WHERE token_hash = $1",
        )
        .bind(hash_secret(&current))
        .fetch_one(store.pool())
        .await
        .map_err(map_sqlx)?;
        assert!(reuse_detected);
        Ok(())
    }

    #[tokio::test]
    async fn invitation_tokens_are_single_use_and_bind_existing_accounts() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let issuer = create_test_principal(&store, "invite-token-issuer").await?;
        let new_email = format!("new-invite-{}@aro.local", Uuid::new_v4());
        let new_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let issued = store
            .issue_organization_invitation(
                issuer.user.id,
                issuer.active_organization.id,
                NewOrganizationMember {
                    email: new_email.clone(),
                    name: "New invitee".to_string(),
                    role: MembershipRole::Member,
                },
                new_token.clone(),
                Utc::now() + Duration::hours(1),
                TEST_SECRETS_KEY,
            )
            .await?;
        let persisted_token_hash: String =
            sqlx::query_scalar("SELECT token_hash FROM organization_invitations WHERE id = $1")
                .bind(issued.id)
                .fetch_one(store.pool())
                .await
                .map_err(map_sqlx)?;
        assert_eq!(persisted_token_hash, hash_secret(&new_token));
        assert_ne!(persisted_token_hash, new_token);

        let (new_user_id, accepted_org_id) = store
            .accept_new_organization_invitation(&new_token, "accepted-password-hash".to_string())
            .await?;
        assert_eq!(accepted_org_id, issuer.active_organization.id);
        assert!(store.principal_for_user(new_user_id).await?.is_some());
        assert_security(
            store
                .accept_new_organization_invitation(&new_token, "another-password-hash".to_string())
                .await,
            "invitation is invalid",
        );

        let existing = create_test_principal(&store, "invite-token-existing").await?;
        let password_before = store
            .find_user_credentials(&existing.user.email)
            .await?
            .expect("existing credentials")
            .password_hash;
        let existing_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .issue_organization_invitation(
                issuer.user.id,
                issuer.active_organization.id,
                NewOrganizationMember {
                    email: existing.user.email.clone(),
                    name: existing.user.name.clone(),
                    role: MembershipRole::Member,
                },
                existing_token.clone(),
                Utc::now() + Duration::hours(1),
                TEST_SECRETS_KEY,
            )
            .await?;
        assert_security(
            store
                .accept_new_organization_invitation(
                    &existing_token,
                    "attacker-password-hash".to_string(),
                )
                .await,
            "invitation is invalid",
        );
        assert_security(
            store
                .accept_existing_organization_invitation(
                    existing.user.id,
                    &existing_token,
                    "invalid-refresh-token-that-is-long-enough",
                    &format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
                    7,
                )
                .await,
            "valid refresh token",
        );

        let victim = create_test_principal(&store, "invite-stolen-refresh-victim").await?;
        let victim_rotated = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let victim_successor = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                victim.user.id,
                victim.active_organization.id,
                None,
                &victim_rotated,
                7,
                90,
            )
            .await?;
        assert!(store
            .rotate_refresh_token(&victim_rotated, &victim_successor, None, None, 7)
            .await?
            .is_some());
        assert_security(
            store
                .accept_existing_organization_invitation(
                    existing.user.id,
                    &existing_token,
                    &victim_rotated,
                    &format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
                    7,
                )
                .await,
            "valid refresh token",
        );
        assert!(store
            .consume_refresh_token(&victim_successor)
            .await?
            .is_none());

        let current_refresh = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        store
            .create_refresh_token(
                existing.user.id,
                existing.active_organization.id,
                None,
                &current_refresh,
                7,
                90,
            )
            .await?;
        let accepted = store
            .accept_existing_organization_invitation(
                existing.user.id,
                &existing_token,
                &current_refresh,
                &format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple()),
                7,
            )
            .await?;
        assert_eq!(accepted.organization_id, issuer.active_organization.id);
        let password_after = store
            .find_user_credentials(&existing.user.email)
            .await?
            .expect("existing credentials after acceptance")
            .password_hash;
        assert_eq!(password_before, password_after);
        Ok(())
    }

    #[tokio::test]
    async fn outbox_events_are_leased_and_retried_without_duplicate_claims() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "outbox-lease").await?;
        store
            .emit_domain_event(
                principal.active_organization.id,
                principal.user.id,
                "test.outbox",
                "test",
                None,
                serde_json::json!({ "safe": true }),
            )
            .await?;

        let first_lease = Uuid::new_v4();
        let first = store
            .claim_pending_outbox_events_for_organization(
                principal.active_organization.id,
                10,
                first_lease,
                60,
            )
            .await?;
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].lease_token, first_lease);
        assert!(store
            .claim_pending_outbox_events_for_organization(
                principal.active_organization.id,
                10,
                Uuid::new_v4(),
                60,
            )
            .await?
            .is_empty());

        assert!(
            store
                .release_outbox_event_for_retry(first[0].event.id, first_lease, "temporary", 1)
                .await?
        );
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let retry_lease = Uuid::new_v4();
        let retry = store
            .claim_pending_outbox_events_for_organization(
                principal.active_organization.id,
                10,
                retry_lease,
                60,
            )
            .await?;
        assert_eq!(retry.len(), 1);
        assert_eq!(retry[0].event.id, first[0].event.id);
        assert!(store
            .acknowledge_outbox_event_by_worker(retry[0].event.id, retry_lease)
            .await?
            .is_some());
        assert!(store
            .claim_pending_outbox_events_for_organization(
                principal.active_organization.id,
                10,
                Uuid::new_v4(),
                60,
            )
            .await?
            .is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn collection_owner_scope_and_rbac_are_enforced() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let unique = Uuid::new_v4();
        let owner = create_test_principal(&store, "collection-owner").await?;
        let member = create_test_principal(&store, "collection-member").await?;
        let member_role = enum_to_string(&MembershipRole::Member)?;

        sqlx::query(
            r#"
            INSERT INTO memberships (user_id, organization_id, role, status)
            VALUES ($1, $2, $3, 'active')
            ON CONFLICT (user_id, organization_id)
            DO UPDATE SET role = excluded.role, status = excluded.status, deleted_at = NULL
            "#,
        )
        .bind(member.user.id)
        .bind(owner.active_organization.id)
        .bind(member_role)
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;

        let owner_memory = store
            .create_collection_item(
                owner.user.id,
                owner.active_organization.id,
                PersistedCollection::Memories,
                serde_json::json!({ "content": format!("owner memory {unique}") }),
                None,
            )
            .await?;
        let owner_memory_id =
            Uuid::parse_str(owner_memory["id"].as_str().expect("owner memory id")).expect("uuid");

        let member_memory = store
            .create_collection_item(
                member.user.id,
                owner.active_organization.id,
                PersistedCollection::Memories,
                serde_json::json!({ "content": format!("member memory {unique}") }),
                None,
            )
            .await?;
        let member_memory_id =
            Uuid::parse_str(member_memory["id"].as_str().expect("member memory id")).expect("uuid");

        let member_memories = store
            .get_collection(
                member.user.id,
                owner.active_organization.id,
                PersistedCollection::Memories,
            )
            .await?;
        let member_memory_ids = member_memories
            .as_array()
            .expect("memory array")
            .iter()
            .filter_map(|item| item["id"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(member_memory_ids, vec![member_memory_id.to_string()]);
        assert!(store
            .get_collection_item(
                member.user.id,
                owner.active_organization.id,
                PersistedCollection::Memories,
                owner_memory_id,
            )
            .await?
            .is_none());
        assert_memory_error(
            store
                .update_collection_item(
                    member.user.id,
                    owner.active_organization.id,
                    PersistedCollection::Memories,
                    owner_memory_id,
                    serde_json::json!({ "content": "stolen update" }),
                    None,
                )
                .await,
            "not found",
        );
        assert_memory_error(
            store
                .delete_collection_item(
                    member.user.id,
                    owner.active_organization.id,
                    PersistedCollection::Memories,
                    owner_memory_id,
                )
                .await,
            "not found",
        );

        assert_security(
            store
                .create_collection_item(
                    member.user.id,
                    owner.active_organization.id,
                    PersistedCollection::PluginConnections,
                    serde_json::json!({ "name": "Unsafe plugin" }),
                    Some("test-secret"),
                )
                .await,
            "organization admin permission required",
        );
        assert_security(
            store
                .create_collection_item(
                    member.user.id,
                    owner.active_organization.id,
                    PersistedCollection::SkillGroups,
                    serde_json::json!({ "name": "Unsafe group" }),
                    None,
                )
                .await,
            "organization manager permission required",
        );

        let task = store
            .create_collection_item(
                owner.user.id,
                owner.active_organization.id,
                PersistedCollection::ScheduledTasks,
                serde_json::json!({
                    "name": "Nightly",
                    "prompt": "summarize",
                    "scheduleType": "daily"
                }),
                None,
            )
            .await?;
        let task_id = Uuid::parse_str(task["id"].as_str().expect("task id")).expect("uuid");
        assert_security(
            store
                .run_scheduled_task(
                    member.user.id,
                    owner.active_organization.id,
                    task_id,
                    serde_json::json!({ "status": "completed" }),
                )
                .await,
            "organization manager permission required",
        );

        Ok(())
    }

    #[tokio::test]
    async fn cross_tenant_conversation_and_local_result_are_refused() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let principal = create_test_principal(&store, "cross-tenant").await?;
        let (second_org, _) = store
            .create_organization_for_user(
                principal.user.id,
                format!("Second Tenant {}", Uuid::new_v4()),
                None,
                None,
            )
            .await?;

        let original = store
            .create_conversation(
                tenant_context(principal.user.id, principal.active_organization.id),
                "Original tenant conversation".to_string(),
                AssistantMode::Chat,
            )
            .await?;
        let conflicting =
            Conversation::with_id(original.id, "Cross-tenant takeover", AssistantMode::Chat);

        assert_security(
            store
                .upsert_conversation(
                    tenant_context(principal.user.id, second_org.id),
                    &conflicting,
                )
                .await,
            "conversation belongs to another organization",
        );

        assert_security(
            store
                .store_local_result(
                    tenant_context(principal.user.id, second_org.id),
                    conflicting,
                    ChatMessage::new(original.id, MessageRole::User, "hello from wrong tenant"),
                    ChatMessage::new(original.id, MessageRole::Assistant, "wrong tenant reply"),
                )
                .await,
            "conversation belongs to another organization",
        );

        let original_after = store
            .get_conversation(
                tenant_context(principal.user.id, principal.active_organization.id),
                original.id,
            )
            .await?
            .expect("original conversation should remain visible");
        assert_eq!(original_after.title, "Original tenant conversation");
        assert_memory_error(
            store
                .list_messages(
                    tenant_context(principal.user.id, second_org.id),
                    original.id,
                )
                .await,
            "conversation not found",
        );

        Ok(())
    }

    #[tokio::test]
    async fn memory_sources_reject_cross_owner_and_cross_tenant() -> AroResult<()> {
        let Some(store) = postgres_store().await? else {
            return Ok(());
        };
        let owner = create_test_principal(&store, "memory-source-owner").await?;
        let member = create_test_principal(&store, "memory-source-member").await?;
        let member_role = enum_to_string(&MembershipRole::Member)?;

        sqlx::query(
            r#"
            INSERT INTO memberships (user_id, organization_id, role, status)
            VALUES ($1, $2, $3, 'active')
            ON CONFLICT (user_id, organization_id)
            DO UPDATE SET role = excluded.role, status = excluded.status, deleted_at = NULL
            "#,
        )
        .bind(member.user.id)
        .bind(owner.active_organization.id)
        .bind(member_role)
        .execute(store.pool())
        .await
        .map_err(map_sqlx)?;

        let owner_conversation = store
            .create_conversation(
                tenant_context(owner.user.id, owner.active_organization.id),
                "Owner source".to_string(),
                AssistantMode::Chat,
            )
            .await?;
        let owner_message =
            ChatMessage::new(owner_conversation.id, MessageRole::User, "owner message");
        store
            .add_message(
                tenant_context(owner.user.id, owner.active_organization.id),
                &owner_message,
            )
            .await?;

        assert_security(
            store
                .create_collection_item(
                    member.user.id,
                    owner.active_organization.id,
                    PersistedCollection::Memories,
                    serde_json::json!({
                        "content": "wrong owner source",
                        "sourceConversationId": owner_conversation.id
                    }),
                    None,
                )
                .await,
            "source conversation",
        );

        assert_security(
            store
                .create_collection_item(
                    member.user.id,
                    owner.active_organization.id,
                    PersistedCollection::Memories,
                    serde_json::json!({
                        "content": "wrong owner source message",
                        "sourceMessageIds": [owner_message.id]
                    }),
                    None,
                )
                .await,
            "source messages",
        );

        let other_conversation = store
            .create_conversation(
                tenant_context(member.user.id, member.active_organization.id),
                "Other tenant source".to_string(),
                AssistantMode::Chat,
            )
            .await?;
        assert_security(
            store
                .create_collection_item(
                    owner.user.id,
                    owner.active_organization.id,
                    PersistedCollection::Memories,
                    serde_json::json!({
                        "content": "wrong tenant source",
                        "sourceConversationId": other_conversation.id
                    }),
                    None,
                )
                .await,
            "source conversation",
        );

        Ok(())
    }
}
