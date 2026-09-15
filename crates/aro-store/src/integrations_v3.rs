use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use super::{hash_secret, map_sqlx, AroError, AroResult, AroStore, TenantContext};

pub struct NewCredentialInstallation {
    pub installation_id: Uuid,
    pub provider_id: String,
    pub owner_type: String,
    pub owner_user_id: Option<Uuid>,
    pub owner_team_id: Option<Uuid>,
    pub label: Option<String>,
    pub public_config: Value,
    pub capability_ids: Vec<String>,
    pub credential_type: String,
    pub algorithm: String,
    pub key_id: String,
    pub key_version: String,
    pub nonce: Vec<u8>,
    pub encrypted_dek: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub granted_scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub idempotency_key: String,
    pub request_hash: String,
}

#[derive(Debug, Clone)]
pub struct NewAuthorizationAttempt {
    pub attempt_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub provider_id: String,
    pub owner_type: String,
    pub owner_user_id: Option<Uuid>,
    pub owner_team_id: Option<Uuid>,
    pub auth_method: String,
    pub capability_ids: Vec<String>,
    pub state_digest: String,
    pub nonce_digest: String,
    pub algorithm: String,
    pub key_id: String,
    pub key_version: String,
    pub nonce: Vec<u8>,
    pub encrypted_dek: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub callback_uri: String,
    pub environment: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AuthorizationAttemptSetup {
    pub attempt_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub organization_id: Uuid,
    pub actor_user_id: Uuid,
    pub provider_id: String,
    pub provider_version: String,
    pub auth_method: String,
    pub owner_type: String,
    pub owner_user_id: Option<Uuid>,
    pub owner_team_id: Option<Uuid>,
    pub capability_ids: Vec<String>,
    pub requested_scopes: Vec<String>,
    pub state_digest: String,
    pub nonce_digest: String,
    pub descriptor: Value,
    pub client_id: String,
    pub client_secret_ref: Option<String>,
    pub callback_uri: String,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub credential_version: i32,
    pub algorithm: String,
    pub key_id: String,
    pub key_version: String,
    pub nonce: Vec<u8>,
    pub encrypted_dek: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ClaimedAuthorizationAttempt {
    pub setup: AuthorizationAttemptSetup,
    pub claim_token: Option<Uuid>,
    pub installation_result_id: Option<Uuid>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewAuthorizedInstallation {
    pub attempt_id: Uuid,
    pub claim_token: Uuid,
    pub installation_id: Uuid,
    pub external_identity: Option<Value>,
    pub credential_type: String,
    pub credential_version: i32,
    pub algorithm: String,
    pub key_id: String,
    pub key_version: String,
    pub nonce: Vec<u8>,
    pub encrypted_dek: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub granted_scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ConnectorClientProfileUpsert {
    pub provider_id: String,
    pub provider_version: String,
    pub environment: String,
    pub realm: String,
    pub client_id: String,
    pub client_secret_ref: Option<String>,
    pub callback_uris: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct InstallationPatchV3 {
    pub enabled: Option<bool>,
    pub label: Option<Option<String>>,
    pub public_config: Option<Value>,
    pub capability_ids: Option<Vec<String>>,
    pub expected_version: i64,
}

#[derive(Debug, Clone)]
pub struct ClaimedIntegrationJob {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub installation_id: Option<Uuid>,
    pub job_type: String,
    pub payload: Value,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub lease_token: Uuid,
    pub created_at: DateTime<Utc>,
}

impl AroStore {
    pub async fn upsert_connector_client_profile_v3(
        &self,
        context: TenantContext,
        profile: ConnectorClientProfileUpsert,
    ) -> AroResult<Value> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let is_admin = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM memberships
              WHERE organization_id = $1 AND user_id = $2
                AND status = 'active' AND deleted_at IS NULL
                AND role IN ('owner', 'admin')
            )
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if !is_admin {
            return Err(AroError::Security(
                "organization administrator access required".into(),
            ));
        }
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM connector_definitions
              WHERE provider_id = $1 AND version = $2 AND deleted_at IS NULL
            )
            "#,
        )
        .bind(&profile.provider_id)
        .bind(&profile.provider_version)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if !exists {
            return Err(AroError::Configuration(
                "connector definition version does not exist".into(),
            ));
        }
        let value = sqlx::query_scalar::<_, Value>(
            r#"
            INSERT INTO connector_client_profiles(
              organization_id, provider_id, provider_version, environment, realm,
              client_id, client_secret_ref, callback_uris, enabled
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (organization_id, provider_id, environment, realm)
            DO UPDATE SET provider_version = EXCLUDED.provider_version,
                          client_id = EXCLUDED.client_id,
                          client_secret_ref = EXCLUDED.client_secret_ref,
                          callback_uris = EXCLUDED.callback_uris,
                          enabled = EXCLUDED.enabled,
                          deleted_at = NULL, updated_at = now()
            RETURNING jsonb_build_object(
              'id', id, 'organizationId', organization_id, 'providerId', provider_id,
              'providerVersion', provider_version, 'environment', environment, 'realm', realm,
              'clientId', client_id, 'clientSecretConfigured', client_secret_ref IS NOT NULL,
              'callbackUris', callback_uris, 'enabled', enabled, 'updatedAt', updated_at
            )
            "#,
        )
        .bind(context.organization_id())
        .bind(&profile.provider_id)
        .bind(&profile.provider_version)
        .bind(&profile.environment)
        .bind(&profile.realm)
        .bind(&profile.client_id)
        .bind(&profile.client_secret_ref)
        .bind(&profile.callback_uris)
        .bind(profile.enabled)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO audit_events(organization_id, actor_user_id, action, target_type, data)
            VALUES ($1, $2, 'integration.client_profile.updated', 'connector_client_profile',
                    jsonb_build_object('providerId', $3, 'environment', $4, 'realm', $5,
                                       'enabled', $6))
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&profile.provider_id)
        .bind(&profile.environment)
        .bind(&profile.realm)
        .bind(profile.enabled)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn integration_catalog_v3(&self, context: TenantContext) -> AroResult<Value> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let value = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT COALESCE(jsonb_agg(
              descriptor || jsonb_build_object(
                'enabled', enabled,
                'certificationStatus', certification_status
              ) ORDER BY descriptor->>'category', descriptor->>'displayName'
            ), '[]'::jsonb)
            FROM connector_definitions
            WHERE deleted_at IS NULL
            "#,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn integration_catalog_entry_v3(
        &self,
        context: TenantContext,
        provider_id: &str,
    ) -> AroResult<Option<Value>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let value = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT descriptor || jsonb_build_object(
              'enabled', enabled,
              'certificationStatus', certification_status
            )
            FROM connector_definitions
            WHERE provider_id = $1 AND deleted_at IS NULL
            ORDER BY enabled DESC, created_at DESC
            LIMIT 1
            "#,
        )
        .bind(provider_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn create_authorization_attempt_v3(
        &self,
        context: TenantContext,
        request: NewAuthorizationAttempt,
    ) -> AroResult<AuthorizationAttemptSetup> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let membership_role: String = sqlx::query_scalar(
            r#"
            SELECT role FROM memberships
            WHERE organization_id = $1 AND user_id = $2
              AND status = 'active' AND deleted_at IS NULL
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if membership_role == "guest" {
            return Err(AroError::Security(
                "guests cannot authorize integration installations".into(),
            ));
        }
        authorize_owner(
            &mut tx,
            context,
            &membership_role,
            &request.owner_type,
            request.owner_user_id,
            request.owner_team_id,
        )
        .await?;
        if let Some(installation_id) = request.installation_id {
            ensure_installation_manage_access(&mut tx, context, installation_id).await?;
        }

        if let Some(row) = sqlx::query(
            r#"
            SELECT attempt.*, definition.descriptor,
                   profile.client_id, profile.client_secret_ref,
                   COALESCE((
                     SELECT MAX(credential.version) + 1
                     FROM integration_credential_versions credential
                     WHERE credential.organization_id = attempt.organization_id
                       AND credential.installation_id = attempt.installation_id
                   ), 1)::integer AS credential_version
            FROM integration_authorization_attempts attempt
            JOIN connector_definitions definition
              ON definition.provider_id = attempt.provider_id
             AND definition.version = attempt.provider_version
            LEFT JOIN LATERAL (
              SELECT client_id, client_secret_ref
              FROM connector_client_profiles profile
              WHERE profile.provider_id = attempt.provider_id
                AND profile.provider_version = attempt.provider_version
                AND profile.environment = $4
                AND profile.deleted_at IS NULL
                AND (profile.organization_id = $1 OR profile.organization_id IS NULL)
                AND attempt.callback_uri = ANY(profile.callback_uris)
              ORDER BY (profile.organization_id IS NOT NULL) DESC, profile.updated_at DESC
              LIMIT 1
            ) profile ON true
            WHERE attempt.organization_id = $1
              AND attempt.actor_user_id = $2
              AND attempt.idempotency_key = $3
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&request.idempotency_key)
        .bind(&request.environment)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        {
            let stored_hash: Option<String> = row.get("request_hash");
            if stored_hash.as_deref() != Some(request.request_hash.as_str()) {
                return Err(AroError::Security(
                    "idempotency key was reused with a different authorization request".into(),
                ));
            }
            let setup = authorization_attempt_setup_from_row(&row)?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(setup);
        }

        let provider = sqlx::query(
            r#"
            SELECT version, descriptor
            FROM connector_definitions
            WHERE provider_id = $1 AND enabled
              AND certification_status = 'certified' AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(&request.provider_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Configuration("connector is not enabled and certified".into()))?;
        let provider_version: String = provider.get("version");
        let descriptor: Value = provider.get("descriptor");
        validate_capabilities(&descriptor, &request.capability_ids)?;
        if !descriptor_has_auth_method(&descriptor, &request.auth_method) {
            return Err(AroError::Security(
                "authorization method is not supported by this connector".into(),
            ));
        }
        let requested_scopes = scopes_for_capabilities(&descriptor, &request.capability_ids)?;
        let profile = sqlx::query(
            r#"
            SELECT client_id, client_secret_ref
            FROM connector_client_profiles
            WHERE provider_id = $1 AND provider_version = $2
              AND environment = $3 AND enabled AND deleted_at IS NULL
              AND (organization_id = $4 OR organization_id IS NULL)
              AND $5 = ANY(callback_uris)
            ORDER BY (organization_id IS NOT NULL) DESC, updated_at DESC
            LIMIT 1
            "#,
        )
        .bind(&request.provider_id)
        .bind(&provider_version)
        .bind(&request.environment)
        .bind(context.organization_id())
        .bind(&request.callback_uri)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Configuration("OAuth client profile is not configured".into()))?;
        let client_id: String = profile.get("client_id");
        let client_secret_ref: Option<String> = profile.get("client_secret_ref");
        let credential_version = if let Some(installation_id) = request.installation_id {
            sqlx::query_scalar::<_, i32>(
                r#"
                SELECT (COALESCE(MAX(version), 0) + 1)::integer
                FROM integration_credential_versions
                WHERE organization_id = $1 AND installation_id = $2
                "#,
            )
            .bind(context.organization_id())
            .bind(installation_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?
        } else {
            1
        };

        sqlx::query(
            r#"
            INSERT INTO integration_authorization_attempts(
              id, organization_id, actor_user_id, provider_id, provider_version,
              owner_type, owner_user_id, owner_team_id, auth_method, capability_ids,
              requested_scopes, state_digest, nonce_digest, pkce_algorithm, pkce_key_id,
              pkce_encryption_algorithm, pkce_key_version, pkce_nonce, pkce_encrypted_dek, pkce_ciphertext,
              callback_uri, status, idempotency_key, request_hash, expires_at, installation_id
            ) VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
              'S256', $14, $15, $16, $17, $18, $19, $20, 'pending', $21, $22, $23, $24
            )
            "#,
        )
        .bind(request.attempt_id)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&request.provider_id)
        .bind(&provider_version)
        .bind(&request.owner_type)
        .bind(request.owner_user_id)
        .bind(request.owner_team_id)
        .bind(&request.auth_method)
        .bind(&request.capability_ids)
        .bind(&requested_scopes)
        .bind(&request.state_digest)
        .bind(&request.nonce_digest)
        .bind(&request.key_id)
        .bind(&request.algorithm)
        .bind(&request.key_version)
        .bind(&request.nonce)
        .bind(&request.encrypted_dek)
        .bind(&request.ciphertext)
        .bind(&request.callback_uri)
        .bind(&request.idempotency_key)
        .bind(&request.request_hash)
        .bind(request.expires_at)
        .bind(request.installation_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        sqlx::query(
            r#"
            INSERT INTO audit_events(organization_id, actor_user_id, action, target_type, target_id, data)
            VALUES ($1, $2, 'integration.authorization.started', 'integration_authorization_attempt',
                    $3, jsonb_build_object('providerId', $4, 'authMethod', $5))
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(request.attempt_id)
        .bind(&request.provider_id)
        .bind(&request.auth_method)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;

        Ok(AuthorizationAttemptSetup {
            attempt_id: request.attempt_id,
            installation_id: request.installation_id,
            organization_id: context.organization_id(),
            actor_user_id: context.actor_id(),
            provider_id: request.provider_id,
            provider_version,
            auth_method: request.auth_method,
            owner_type: request.owner_type,
            owner_user_id: request.owner_user_id,
            owner_team_id: request.owner_team_id,
            capability_ids: request.capability_ids,
            requested_scopes,
            state_digest: request.state_digest,
            nonce_digest: request.nonce_digest,
            descriptor,
            client_id,
            client_secret_ref,
            callback_uri: request.callback_uri,
            status: "pending".into(),
            expires_at: request.expires_at,
            credential_version,
            algorithm: request.algorithm,
            key_id: request.key_id,
            key_version: request.key_version,
            nonce: request.nonce,
            encrypted_dek: request.encrypted_dek,
            ciphertext: request.ciphertext,
        })
    }

    pub async fn get_authorization_attempt_v3(
        &self,
        context: TenantContext,
        attempt_id: Uuid,
    ) -> AroResult<Option<Value>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        sqlx::query(
            r#"
            UPDATE integration_authorization_attempts
            SET status = 'expired', error_code = 'authorization_expired',
                error_detail = 'Authorization attempt expired', completed_at = now(),
                claim_token = NULL
            WHERE organization_id = $1 AND actor_user_id = $2 AND id = $3
              AND status IN ('pending', 'processing') AND expires_at <= now()
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(attempt_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let value = sqlx::query_scalar::<_, Value>(authorization_attempt_public_sql())
            .bind(context.organization_id())
            .bind(context.actor_id())
            .bind(attempt_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn cancel_authorization_attempt_v3(
        &self,
        context: TenantContext,
        attempt_id: Uuid,
    ) -> AroResult<Value> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let affected = sqlx::query(
            r#"
            UPDATE integration_authorization_attempts
            SET status = 'cancelled', error_code = 'cancelled',
                error_detail = 'Authorization was cancelled', completed_at = now(),
                claim_token = NULL
            WHERE organization_id = $1 AND actor_user_id = $2 AND id = $3
              AND status = 'pending'
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(attempt_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        if affected != 1 {
            return Err(AroError::Security(
                "authorization attempt is not cancellable".into(),
            ));
        }
        let value = sqlx::query_scalar::<_, Value>(authorization_attempt_public_sql())
            .bind(context.organization_id())
            .bind(context.actor_id())
            .bind(attempt_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn claim_authorization_attempt_v3(
        &self,
        state_digest: &str,
        provider_id: &str,
        claim_token: Uuid,
        environment: &str,
    ) -> AroResult<Option<ClaimedAuthorizationAttempt>> {
        let claimed = sqlx::query(
            r#"
            SELECT * FROM public.aro_claim_integration_authorization_attempt($1, $2, $3)
            "#,
        )
        .bind(state_digest)
        .bind(provider_id)
        .bind(claim_token)
        .fetch_optional(self.pool())
        .await
        .map_err(map_sqlx)?;
        let Some(claimed) = claimed else {
            return Ok(None);
        };
        let organization_id: Uuid = claimed.get("organization_id");
        let actor_user_id: Uuid = claimed.get("actor_user_id");
        let attempt_id: Uuid = claimed.get("id");
        let context = TenantContext::new(actor_user_id, organization_id)?;
        let mut tx = self.begin_tenant_tx(context).await?;
        let row = sqlx::query(
            r#"
            SELECT attempt.*, definition.descriptor,
                   profile.client_id, profile.client_secret_ref,
                   COALESCE((
                     SELECT MAX(credential.version) + 1
                     FROM integration_credential_versions credential
                     WHERE credential.organization_id = attempt.organization_id
                       AND credential.installation_id = attempt.installation_id
                   ), 1)::integer AS credential_version
            FROM integration_authorization_attempts attempt
            JOIN connector_definitions definition
              ON definition.provider_id = attempt.provider_id
             AND definition.version = attempt.provider_version
            LEFT JOIN LATERAL (
              SELECT client_id, client_secret_ref
              FROM connector_client_profiles profile
              WHERE profile.provider_id = attempt.provider_id
                AND profile.provider_version = attempt.provider_version
                AND profile.environment = $4
                AND profile.deleted_at IS NULL
                AND (profile.organization_id = $1 OR profile.organization_id IS NULL)
                AND attempt.callback_uri = ANY(profile.callback_uris)
              ORDER BY (profile.organization_id IS NOT NULL) DESC, profile.updated_at DESC
              LIMIT 1
            ) profile ON true
            WHERE attempt.organization_id = $1 AND attempt.actor_user_id = $2
              AND attempt.id = $3
            "#,
        )
        .bind(organization_id)
        .bind(actor_user_id)
        .bind(attempt_id)
        .bind(environment)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let setup = authorization_attempt_setup_from_row(&row)?;
        let result = ClaimedAuthorizationAttempt {
            setup,
            claim_token: row.get("claim_token"),
            installation_result_id: row.get("installation_result_id"),
            error_code: row.get("error_code"),
        };
        tx.commit().await.map_err(map_sqlx)?;
        Ok(Some(result))
    }

    pub async fn fail_authorization_attempt_v3(
        &self,
        context: TenantContext,
        attempt_id: Uuid,
        claim_token: Uuid,
        status: &str,
        error_code: &str,
        error_detail: &str,
    ) -> AroResult<()> {
        if !matches!(status, "denied" | "failed") {
            return Err(AroError::Security(
                "authorization failure status is invalid".into(),
            ));
        }
        let mut tx = self.begin_tenant_tx(context).await?;
        let affected = sqlx::query(
            r#"
            UPDATE integration_authorization_attempts
            SET status = $4, error_code = $5, error_detail = $6,
                completed_at = now(), claim_token = NULL
            WHERE organization_id = $1 AND actor_user_id = $2 AND id = $3
              AND status = 'processing' AND claim_token = $7
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(attempt_id)
        .bind(status)
        .bind(error_code)
        .bind(truncate_error(error_detail))
        .bind(claim_token)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        if affected == 1 {
            sqlx::query(
                r#"
                INSERT INTO audit_events(organization_id, actor_user_id, action, target_type, target_id, data)
                VALUES ($1, $2, 'integration.authorization.failed',
                        'integration_authorization_attempt', $3,
                        jsonb_build_object('status', $4, 'errorCode', $5))
                "#,
            )
            .bind(context.organization_id())
            .bind(context.actor_id())
            .bind(attempt_id)
            .bind(status)
            .bind(error_code)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        }
        tx.commit().await.map_err(map_sqlx)?;
        Ok(())
    }

    pub async fn finalize_authorization_attempt_v3(
        &self,
        context: TenantContext,
        request: NewAuthorizedInstallation,
    ) -> AroResult<Value> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let attempt = sqlx::query(
            r#"
            SELECT actor_user_id, provider_id, provider_version, owner_type, owner_user_id,
                   owner_team_id, capability_ids, installation_id, status, claim_token,
                   installation_result_id
            FROM integration_authorization_attempts
            WHERE organization_id = $1 AND id = $2
            FOR UPDATE
            "#,
        )
        .bind(context.organization_id())
        .bind(request.attempt_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Security("authorization attempt not found".into()))?;
        let actor_user_id: Uuid = attempt.get("actor_user_id");
        if actor_user_id != context.actor_id() {
            return Err(AroError::Security("authorization actor mismatch".into()));
        }
        let status: String = attempt.get("status");
        if status == "authorized" {
            let existing_id: Option<Uuid> = attempt.get("installation_result_id");
            let existing_id = existing_id.ok_or_else(|| {
                AroError::Unexpected("authorized attempt has no installation result".into())
            })?;
            let value = public_installation_in_tx(&mut tx, context, existing_id)
                .await?
                .ok_or_else(|| AroError::Security("installation result is not visible".into()))?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(value);
        }
        let stored_claim: Option<Uuid> = attempt.get("claim_token");
        if status != "processing" || stored_claim != Some(request.claim_token) {
            return Err(AroError::Security(
                "authorization attempt is not owned by this callback".into(),
            ));
        }

        let provider_id: String = attempt.get("provider_id");
        let provider_version: String = attempt.get("provider_version");
        let owner_type: String = attempt.get("owner_type");
        let owner_user_id: Option<Uuid> = attempt.get("owner_user_id");
        let owner_team_id: Option<Uuid> = attempt.get("owner_team_id");
        let capability_ids: Vec<String> = attempt.get("capability_ids");
        let existing_installation_id: Option<Uuid> = attempt.get("installation_id");

        let external_account_id = if let Some(identity) = request.external_identity.as_ref() {
            let subject = identity
                .get("subject")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty() && value.len() <= 512)
                .ok_or_else(|| AroError::Security("external identity subject is invalid".into()))?;
            let realm = identity
                .get("realm")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty() && value.len() <= 255)
                .unwrap_or("default");
            let display_name = identity
                .get("displayName")
                .and_then(Value::as_str)
                .map(|value| {
                    value
                        .chars()
                        .filter(|ch| !ch.is_control())
                        .take(255)
                        .collect::<String>()
                });
            let avatar_url = identity
                .get("avatarUrl")
                .and_then(Value::as_str)
                .filter(|value| value.starts_with("https://") && value.len() <= 2048);
            let details = identity
                .get("attributes")
                .filter(|value| value.is_object())
                .cloned()
                .unwrap_or_else(|| json!({}));
            Some(
                sqlx::query_scalar::<_, Uuid>(
                    r#"
                    INSERT INTO integration_external_accounts(
                      organization_id, provider_id, external_account_id, realm,
                      display_name, avatar_url, details, last_seen_at
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, now())
                    ON CONFLICT (organization_id, provider_id, realm, external_account_id)
                    DO UPDATE SET display_name = EXCLUDED.display_name,
                                  avatar_url = EXCLUDED.avatar_url,
                                  details = EXCLUDED.details,
                                  last_seen_at = now(), deleted_at = NULL
                    RETURNING id
                    "#,
                )
                .bind(context.organization_id())
                .bind(&provider_id)
                .bind(subject)
                .bind(realm)
                .bind(display_name)
                .bind(avatar_url)
                .bind(details)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?,
            )
        } else {
            None
        };

        let installation_id = if let Some(installation_id) = existing_installation_id {
            ensure_installation_manage_access(&mut tx, context, installation_id).await?;
            let affected = sqlx::query(
                r#"
                UPDATE integration_installations
                SET external_account_id = COALESCE($4, external_account_id),
                    lifecycle_status = 'active', health_status = 'unknown', enabled = true,
                    connected_at = now(), revoked_at = NULL, version = version + 1,
                    updated_at = now()
                WHERE organization_id = $1 AND id = $2 AND provider_id = $3
                  AND deleted_at IS NULL
                "#,
            )
            .bind(context.organization_id())
            .bind(installation_id)
            .bind(&provider_id)
            .bind(external_account_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?
            .rows_affected();
            if affected != 1 {
                return Err(AroError::Security(
                    "reauthorization provider or installation mismatch".into(),
                ));
            }
            installation_id
        } else {
            sqlx::query(
                r#"
                INSERT INTO integration_installations(
                  id, organization_id, provider_id, provider_version, created_by_user_id,
                  creation_idempotency_key, creation_request_hash, owner_type, owner_user_id,
                  owner_team_id, external_account_id, label, lifecycle_status, health_status,
                  enabled, public_config, connected_at
                ) VALUES (
                  $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                  'active', 'unknown', true, '{}'::jsonb, now()
                )
                "#,
            )
            .bind(request.installation_id)
            .bind(context.organization_id())
            .bind(&provider_id)
            .bind(&provider_version)
            .bind(context.actor_id())
            .bind(format!("oauth:{}", request.attempt_id))
            .bind(hash_secret(&format!(
                "{}:{provider_id}",
                request.attempt_id
            )))
            .bind(&owner_type)
            .bind(owner_user_id)
            .bind(owner_team_id)
            .bind(external_account_id)
            .bind(
                request
                    .external_identity
                    .as_ref()
                    .and_then(|identity| identity.get("displayName"))
                    .and_then(Value::as_str)
                    .map(|value| {
                        value
                            .chars()
                            .filter(|ch| !ch.is_control())
                            .take(120)
                            .collect::<String>()
                    }),
            )
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            request.installation_id
        };

        let next_version: i32 = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(version), 0)::integer + 1
            FROM integration_credential_versions
            WHERE organization_id = $1 AND installation_id = $2
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if next_version != request.credential_version {
            return Err(AroError::RetryableTransaction(
                "credential version changed during authorization".into(),
            ));
        }
        sqlx::query(
            r#"
            UPDATE integration_credential_versions
            SET current = false, revoked_at = COALESCE(revoked_at, now())
            WHERE organization_id = $1 AND installation_id = $2
              AND current AND deleted_at IS NULL
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO integration_credential_versions(
              organization_id, installation_id, version, credential_type, algorithm,
              key_id, key_version, nonce, encrypted_dek, ciphertext, granted_scopes,
              expires_at, current
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, true)
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(next_version)
        .bind(&request.credential_type)
        .bind(&request.algorithm)
        .bind(&request.key_id)
        .bind(&request.key_version)
        .bind(&request.nonce)
        .bind(&request.encrypted_dek)
        .bind(&request.ciphertext)
        .bind(&request.granted_scopes)
        .bind(request.expires_at)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        sqlx::query(
            r#"
            UPDATE integration_capability_grants
            SET revoked_at = now()
            WHERE organization_id = $1 AND installation_id = $2
              AND NOT (capability_id = ANY($3)) AND revoked_at IS NULL
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(&capability_ids)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        // Insertion groupée via unnest (était : 1 INSERT par capability).
        sqlx::query(
            r#"
            INSERT INTO integration_capability_grants(
              organization_id, installation_id, capability_id, granted_scopes,
              granted_by_user_id, revoked_at
            )
            SELECT $1, $2, unnest($3::text[]), $4, $5, NULL
            ON CONFLICT (organization_id, installation_id, capability_id)
            DO UPDATE SET granted_scopes = EXCLUDED.granted_scopes,
                          granted_by_user_id = EXCLUDED.granted_by_user_id,
                          granted_at = now(), revoked_at = NULL
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(&capability_ids)
        .bind(&request.granted_scopes)
        .bind(context.actor_id())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        sqlx::query(
            r#"
            UPDATE integration_authorization_attempts
            SET status = 'authorized', installation_result_id = $4,
                completed_at = now(), claim_token = NULL, error_code = NULL, error_detail = NULL
            WHERE organization_id = $1 AND actor_user_id = $2 AND id = $3
              AND status = 'processing' AND claim_token = $5
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(request.attempt_id)
        .bind(installation_id)
        .bind(request.claim_token)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO audit_events(organization_id, actor_user_id, action, target_type, target_id, data)
            VALUES ($1, $2, 'integration.authorization.authorized', 'integration_installation',
                    $3, jsonb_build_object('providerId', $4, 'attemptId', $5))
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(installation_id)
        .bind(&provider_id)
        .bind(request.attempt_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO outbox_events(organization_id, event_type, payload)
            VALUES ($1, 'integration.installation.authorized',
                    jsonb_build_object('installationId', $2, 'providerId', $3))
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(&provider_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        let value = public_installation_in_tx(&mut tx, context, installation_id)
            .await?
            .ok_or_else(|| AroError::Unexpected("authorized installation is missing".into()))?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn list_integration_installations_v3(
        &self,
        context: TenantContext,
    ) -> AroResult<Value> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let value = sqlx::query_scalar::<_, Value>(&format!(
            r#"
                SELECT COALESCE(jsonb_agg(data ORDER BY data->>'updatedAt' DESC), '[]'::jsonb)
                FROM (
                  SELECT {public_json} AS data
                  FROM integration_installations installation
                  LEFT JOIN integration_external_accounts account
                    ON account.organization_id = installation.organization_id
                   AND account.id = installation.external_account_id
                  LEFT JOIN integration_health_snapshots health
                    ON health.organization_id = installation.organization_id
                   AND health.installation_id = installation.id
                  LEFT JOIN LATERAL (
                    SELECT credential_type, granted_scopes, expires_at, last_used_at, created_at
                    FROM integration_credential_versions credential
                    WHERE credential.organization_id = installation.organization_id
                      AND credential.installation_id = installation.id
                      AND credential.current
                      AND credential.revoked_at IS NULL
                      AND credential.deleted_at IS NULL
                    LIMIT 1
                  ) credential ON true
                  WHERE installation.organization_id = $1
                    AND installation.deleted_at IS NULL
                    AND {access_predicate}
                ) visible
                "#,
            public_json = installation_public_json_sql(),
            access_predicate = installation_access_predicate_sql(),
        ))
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn get_integration_installation_v3(
        &self,
        context: TenantContext,
        installation_id: Uuid,
    ) -> AroResult<Option<Value>> {
        let mut tx = self.begin_tenant_tx(context).await?;
        let value = public_installation_in_tx(&mut tx, context, installation_id).await?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn create_credential_installation_v3(
        &self,
        context: TenantContext,
        request: NewCredentialInstallation,
    ) -> AroResult<Value> {
        let mut tx = self.begin_tenant_tx(context).await?;

        if let Some(row) = sqlx::query(
            r#"
            SELECT id, creation_request_hash
            FROM integration_installations
            WHERE organization_id = $1
              AND created_by_user_id = $2
              AND creation_idempotency_key = $3
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(&request.idempotency_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        {
            let stored_hash: Option<String> = row.get("creation_request_hash");
            if stored_hash.as_deref() != Some(request.request_hash.as_str()) {
                return Err(AroError::Security(
                    "idempotency key was reused with a different request".to_string(),
                ));
            }
            let id: Uuid = row.get("id");
            let value = public_installation_in_tx(&mut tx, context, id)
                .await?
                .ok_or_else(|| AroError::Unexpected("idempotent installation is missing".into()))?;
            tx.commit().await.map_err(map_sqlx)?;
            return Ok(value);
        }

        let membership_role: String = sqlx::query_scalar(
            r#"
            SELECT role FROM memberships
            WHERE organization_id = $1 AND user_id = $2
              AND status = 'active' AND deleted_at IS NULL
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if membership_role == "guest" {
            return Err(AroError::Security(
                "guests cannot create integration installations".to_string(),
            ));
        }
        authorize_owner(
            &mut tx,
            context,
            &membership_role,
            &request.owner_type,
            request.owner_user_id,
            request.owner_team_id,
        )
        .await?;

        let provider = sqlx::query(
            r#"
            SELECT version, descriptor
            FROM connector_definitions
            WHERE provider_id = $1
              AND enabled
              AND certification_status = 'certified'
              AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(&request.provider_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::Configuration("connector is not enabled and certified".into()))?;
        let provider_version: String = provider.get("version");
        let descriptor: Value = provider.get("descriptor");
        validate_capabilities(&descriptor, &request.capability_ids)?;
        if !descriptor_auth_method(&descriptor, &request.credential_type) {
            return Err(AroError::Security(
                "credential type is not supported by this connector".to_string(),
            ));
        }

        let installation_id = request.installation_id;
        sqlx::query(
            r#"
            INSERT INTO integration_installations(
              id, organization_id, provider_id, provider_version, created_by_user_id,
              creation_idempotency_key, creation_request_hash, owner_type, owner_user_id,
              owner_team_id, label, lifecycle_status, health_status, enabled, public_config,
              connected_at
            ) VALUES (
              $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11,
              'active', 'unknown', true, $12, now()
            )
            "#,
        )
        .bind(installation_id)
        .bind(context.organization_id())
        .bind(&request.provider_id)
        .bind(&provider_version)
        .bind(context.actor_id())
        .bind(&request.idempotency_key)
        .bind(&request.request_hash)
        .bind(&request.owner_type)
        .bind(request.owner_user_id)
        .bind(request.owner_team_id)
        .bind(request.label.as_deref())
        .bind(&request.public_config)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        sqlx::query(
            r#"
            INSERT INTO integration_credential_versions(
              organization_id, installation_id, version, credential_type, algorithm,
              key_id, key_version, nonce, encrypted_dek, ciphertext, granted_scopes,
              expires_at, current
            ) VALUES ($1, $2, 1, $3, $4, $5, $6, $7, $8, $9, $10, $11, true)
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(&request.credential_type)
        .bind(&request.algorithm)
        .bind(&request.key_id)
        .bind(&request.key_version)
        .bind(request.nonce)
        .bind(request.encrypted_dek)
        .bind(request.ciphertext)
        .bind(&request.granted_scopes)
        .bind(request.expires_at)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        // Insertion groupée via unnest (était : 1 INSERT par capability).
        sqlx::query(
            r#"
            INSERT INTO integration_capability_grants(
              organization_id, installation_id, capability_id, granted_scopes,
              granted_by_user_id
            )
            SELECT $1, $2, unnest($3::text[]), $4, $5
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(&request.capability_ids)
        .bind(&request.granted_scopes)
        .bind(context.actor_id())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        sqlx::query(
            r#"
            INSERT INTO audit_events(organization_id, actor_user_id, action, target_type, target_id, data)
            VALUES ($1, $2, 'integration.installation.created', 'integration_installation', $3,
              jsonb_build_object('providerId', $4, 'ownerType', $5, 'credentialType', $6))
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(installation_id)
        .bind(&request.provider_id)
        .bind(&request.owner_type)
        .bind(&request.credential_type)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        sqlx::query(
            r#"
            INSERT INTO outbox_events(organization_id, event_type, payload)
            VALUES ($1, 'integration.installation.created',
              jsonb_build_object('installationId', $2, 'providerId', $3))
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(&request.provider_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;

        let value = public_installation_in_tx(&mut tx, context, installation_id)
            .await?
            .ok_or_else(|| AroError::Unexpected("created installation is missing".into()))?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn patch_integration_installation_v3(
        &self,
        context: TenantContext,
        installation_id: Uuid,
        patch: InstallationPatchV3,
    ) -> AroResult<Value> {
        let mut tx = self.begin_tenant_tx(context).await?;
        ensure_installation_manage_access(&mut tx, context, installation_id).await?;
        if let Some(config) = patch.public_config.as_ref() {
            if !config.is_object() {
                return Err(AroError::Configuration(
                    "public config must be an object".into(),
                ));
            }
        }
        let updated = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE integration_installations
            SET enabled = COALESCE($4, enabled),
                label = CASE WHEN $5 THEN $6 ELSE label END,
                public_config = COALESCE($7, public_config),
                version = version + 1,
                updated_at = now()
            WHERE organization_id = $1 AND id = $2 AND version = $3 AND deleted_at IS NULL
            RETURNING id
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(patch.expected_version)
        .bind(patch.enabled)
        .bind(patch.label.is_some())
        .bind(patch.label.flatten())
        .bind(patch.public_config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .ok_or_else(|| AroError::RetryableTransaction("installation version changed".into()))?;

        if let Some(capabilities) = patch.capability_ids {
            let descriptor: Value = sqlx::query_scalar(
                r#"
                SELECT definition.descriptor
                FROM integration_installations installation
                JOIN connector_definitions definition
                  ON definition.provider_id = installation.provider_id
                 AND definition.version = installation.provider_version
                WHERE installation.organization_id = $1 AND installation.id = $2
                "#,
            )
            .bind(context.organization_id())
            .bind(installation_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            validate_capabilities(&descriptor, &capabilities)?;
            sqlx::query(
                "UPDATE integration_capability_grants SET revoked_at = now() WHERE organization_id = $1 AND installation_id = $2 AND revoked_at IS NULL",
            )
            .bind(context.organization_id())
            .bind(installation_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
            for capability in capabilities {
                sqlx::query(
                    r#"
                    INSERT INTO integration_capability_grants(
                      organization_id, installation_id, capability_id, granted_by_user_id, revoked_at
                    ) VALUES ($1, $2, $3, $4, NULL)
                    ON CONFLICT (organization_id, installation_id, capability_id)
                    DO UPDATE SET granted_by_user_id = excluded.granted_by_user_id,
                                  granted_at = now(), revoked_at = NULL
                    "#,
                )
                .bind(context.organization_id())
                .bind(installation_id)
                .bind(capability)
                .bind(context.actor_id())
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
            }
        }

        let value = public_installation_in_tx(&mut tx, context, updated)
            .await?
            .ok_or_else(|| AroError::Unexpected("updated installation is missing".into()))?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn enqueue_integration_action_v3(
        &self,
        context: TenantContext,
        installation_id: Uuid,
        job_type: &str,
    ) -> AroResult<Value> {
        if !matches!(job_type, "health" | "revoke") {
            return Err(AroError::Configuration(
                "unsupported integration action".into(),
            ));
        }
        let mut tx = self.begin_tenant_tx(context).await?;
        ensure_installation_manage_access(&mut tx, context, installation_id).await?;
        if job_type == "revoke" {
            sqlx::query(
                r#"
                UPDATE integration_installations
                SET lifecycle_status = 'disconnecting', enabled = false,
                    version = version + 1, updated_at = now()
                WHERE organization_id = $1 AND id = $2 AND deleted_at IS NULL
                "#,
            )
            .bind(context.organization_id())
            .bind(installation_id)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx)?;
        }
        let job_id = if let Some(id) = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id FROM integration_jobs
            WHERE organization_id = $1 AND installation_id = $2 AND job_type = $3
              AND status IN ('pending', 'leased', 'retry')
            ORDER BY created_at DESC LIMIT 1
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .bind(job_type)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx)?
        {
            id
        } else {
            sqlx::query_scalar::<_, Uuid>(
                r#"
                INSERT INTO integration_jobs(organization_id, installation_id, job_type, max_attempts)
                VALUES ($1, $2, $3, CASE WHEN $3 = 'revoke' THEN 200 ELSE 8 END) RETURNING id
                "#,
            )
            .bind(context.organization_id())
            .bind(installation_id)
            .bind(job_type)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx)?
        };
        sqlx::query(
            r#"
            INSERT INTO audit_events(organization_id, actor_user_id, action, target_type, target_id, data)
            VALUES ($1, $2, $3, 'integration_installation', $4, jsonb_build_object('jobId', $5))
            "#,
        )
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(format!("integration.installation.{job_type}_requested"))
        .bind(installation_id)
        .bind(job_id)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(json!({ "jobId": job_id, "status": "pending" }))
    }

    pub async fn integration_installation_audit_v3(
        &self,
        context: TenantContext,
        installation_id: Uuid,
    ) -> AroResult<Value> {
        let mut tx = self.begin_tenant_tx(context).await?;
        ensure_installation_view_access(&mut tx, context, installation_id).await?;
        let value = sqlx::query_scalar::<_, Value>(
            r#"
            SELECT COALESCE(jsonb_agg(jsonb_build_object(
              'id', id, 'action', action, 'actorUserId', actor_user_id,
              'data', data, 'createdAt', created_at
            ) ORDER BY created_at DESC), '[]'::jsonb)
            FROM audit_events
            WHERE organization_id = $1 AND target_type = 'integration_installation'
              AND target_id = $2
            "#,
        )
        .bind(context.organization_id())
        .bind(installation_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        tx.commit().await.map_err(map_sqlx)?;
        Ok(value)
    }

    pub async fn claim_integration_jobs(
        &self,
        worker_id: &str,
        limit: i64,
        lease_seconds: i64,
    ) -> AroResult<Vec<ClaimedIntegrationJob>> {
        let lease_token = Uuid::new_v4();
        let rows = sqlx::query(
            r#"
            WITH candidates AS (
              SELECT id FROM integration_jobs
              WHERE (
                status IN ('pending', 'retry') AND available_at <= now()
              ) OR (
                status = 'leased' AND lease_expires_at < now()
              )
              ORDER BY available_at, created_at
              FOR UPDATE SKIP LOCKED
              LIMIT $1
            )
            UPDATE integration_jobs job
            SET status = 'leased', lease_owner = $2, lease_token = $3,
                lease_expires_at = now() + make_interval(secs => $4),
                attempt_count = attempt_count + 1, updated_at = now()
            FROM candidates
            WHERE job.id = candidates.id
            RETURNING job.id, job.organization_id, job.installation_id, job.job_type,
                      job.payload, job.attempt_count, job.max_attempts, job.lease_token,
                      job.created_at
            "#,
        )
        .bind(limit.clamp(1, 100))
        .bind(worker_id)
        .bind(lease_token)
        .bind(lease_seconds.clamp(5, 3600))
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(rows
            .into_iter()
            .map(|row| ClaimedIntegrationJob {
                id: row.get("id"),
                organization_id: row.get("organization_id"),
                installation_id: row.get("installation_id"),
                job_type: row.get("job_type"),
                payload: row.get("payload"),
                attempt_count: row.get("attempt_count"),
                max_attempts: row.get("max_attempts"),
                lease_token: row.get("lease_token"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn complete_integration_job(&self, job: &ClaimedIntegrationJob) -> AroResult<bool> {
        let affected = sqlx::query(
            r#"
            UPDATE integration_jobs SET status = 'completed', completed_at = now(),
              lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = now()
            WHERE id = $1 AND lease_token = $2 AND status = 'leased'
            "#,
        )
        .bind(job.id)
        .bind(job.lease_token)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        Ok(affected == 1)
    }

    pub async fn fail_integration_job(
        &self,
        job: &ClaimedIntegrationJob,
        error_code: &str,
        error_detail: &str,
    ) -> AroResult<bool> {
        let dead_letter = job.attempt_count >= job.max_attempts;
        let retry_seconds = 2_i64.pow(job.attempt_count.clamp(1, 10) as u32).min(900);
        let affected = sqlx::query(
            r#"
            UPDATE integration_jobs
            SET status = CASE WHEN $3 THEN 'dead_letter' ELSE 'retry' END,
                available_at = CASE WHEN $3 THEN available_at ELSE now() + make_interval(secs => $4) END,
                last_error_code = $5, last_error_detail = $6,
                lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL,
                updated_at = now()
            WHERE id = $1 AND lease_token = $2 AND status = 'leased'
            "#,
        )
        .bind(job.id)
        .bind(job.lease_token)
        .bind(dead_letter)
        .bind(retry_seconds)
        .bind(error_code)
        .bind(truncate_error(error_detail))
        .execute(&self.pool)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        Ok(affected == 1)
    }

    /// Completes the provider-independent part of health and revocation jobs. Connectors with a
    /// remote health/revocation API perform that call before invoking this fenced transition.
    pub async fn finalize_local_integration_job(
        &self,
        job: &ClaimedIntegrationJob,
    ) -> AroResult<bool> {
        let Some(installation_id) = job.installation_id else {
            return Err(AroError::Configuration(
                "integration job has no installation".into(),
            ));
        };
        let mut tx = self.pool.begin().await.map_err(map_sqlx)?;
        let owns_lease = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
              SELECT 1 FROM integration_jobs
              WHERE id = $1 AND lease_token = $2 AND status = 'leased'
            )
            "#,
        )
        .bind(job.id)
        .bind(job.lease_token)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_sqlx)?;
        if !owns_lease {
            tx.rollback().await.map_err(map_sqlx)?;
            return Ok(false);
        }

        match job.job_type.as_str() {
            "health" => {
                let expired = sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT COALESCE(bool_or(expires_at <= now()), false)
                    FROM integration_credential_versions
                    WHERE organization_id = $1 AND installation_id = $2
                      AND current AND revoked_at IS NULL AND deleted_at IS NULL
                    "#,
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                let status = if expired {
                    "credentials_expired"
                } else {
                    // Expiry can be established locally; provider health cannot. Certified
                    // connectors replace this snapshot after their remote check succeeds.
                    "unknown"
                };
                sqlx::query(
                    r#"
                    INSERT INTO integration_health_snapshots(
                      organization_id, installation_id, status, checked_at
                    ) VALUES ($1, $2, $3, now())
                    ON CONFLICT (organization_id, installation_id)
                    DO UPDATE SET status = excluded.status, error_code = NULL,
                                  retry_at = NULL, checked_at = now(), details = '{}'::jsonb
                    "#,
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .bind(status)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                sqlx::query(
                    "INSERT INTO integration_health_history(organization_id, installation_id, status) VALUES ($1, $2, $3)",
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .bind(status)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                sqlx::query(
                    "UPDATE integration_installations SET health_status = $3, lifecycle_status = CASE WHEN $3 = 'credentials_expired' THEN 'reauthorization_required' ELSE lifecycle_status END, updated_at = now() WHERE organization_id = $1 AND id = $2",
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .bind(status)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
            }
            "revoke" => {
                let remote_required = sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT COALESCE(bool_or(credential_type IN (
                      'oauth2', 'oidc', 'device_code', 'app_installation'
                    )), false)
                    FROM integration_credential_versions
                    WHERE organization_id = $1 AND installation_id = $2
                      AND current AND revoked_at IS NULL AND deleted_at IS NULL
                    "#,
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                let remote_confirmed = job
                    .payload
                    .get("remoteRevocationConfirmed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if remote_required
                    && !remote_confirmed
                    && job.created_at > Utc::now() - chrono::Duration::hours(24)
                {
                    return Err(AroError::Configuration(
                        "remote provider revocation is still pending".into(),
                    ));
                }
                sqlx::query(
                    "UPDATE integration_webhook_subscriptions SET status = 'revoked', secret_credential_version_id = NULL, deleted_at = now(), updated_at = now() WHERE organization_id = $1 AND installation_id = $2 AND deleted_at IS NULL",
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                // Removing the envelope (especially the wrapped DEK) is the cryptographic erase.
                sqlx::query(
                    "DELETE FROM integration_credential_versions WHERE organization_id = $1 AND installation_id = $2",
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                sqlx::query(
                    r#"
                    UPDATE integration_installations
                    SET lifecycle_status = 'revoked', health_status = 'unknown', enabled = false,
                        revoked_at = now(), updated_at = now(), version = version + 1
                    WHERE organization_id = $1 AND id = $2
                    "#,
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
                sqlx::query(
                    r#"
                    INSERT INTO audit_events(organization_id, action, target_type, target_id, data)
                    VALUES ($1, 'integration.installation.revoked', 'integration_installation', $2,
                      jsonb_build_object(
                        'remoteRevocation', CASE
                          WHEN $3 THEN 'confirmed'
                          WHEN $4 THEN 'not_confirmed_after_24h'
                          ELSE 'not_applicable'
                        END,
                        'credentialsErased', true
                      ))
                    "#,
                )
                .bind(job.organization_id)
                .bind(installation_id)
                .bind(remote_confirmed)
                .bind(remote_required)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx)?;
            }
            _ => {
                return Err(AroError::Configuration(
                    "job requires a provider connector".into(),
                ));
            }
        }

        let affected = sqlx::query(
            r#"
            UPDATE integration_jobs SET status = 'completed', completed_at = now(),
              lease_owner = NULL, lease_token = NULL, lease_expires_at = NULL, updated_at = now()
            WHERE id = $1 AND lease_token = $2 AND status = 'leased'
            "#,
        )
        .bind(job.id)
        .bind(job.lease_token)
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?
        .rows_affected();
        tx.commit().await.map_err(map_sqlx)?;
        Ok(affected == 1)
    }
}

fn authorization_attempt_setup_from_row(
    row: &sqlx::postgres::PgRow,
) -> AroResult<AuthorizationAttemptSetup> {
    let status: String = row.get("status");
    let client_id: Option<String> = row.get("client_id");
    if matches!(status.as_str(), "pending" | "processing") && client_id.is_none() {
        return Err(AroError::Configuration(
            "OAuth client profile is no longer available".into(),
        ));
    }
    let algorithm: Option<String> = row.get("pkce_encryption_algorithm");
    let key_id: Option<String> = row.get("pkce_key_id");
    let key_version: Option<String> = row.get("pkce_key_version");
    let nonce: Option<Vec<u8>> = row.get("pkce_nonce");
    let encrypted_dek: Option<Vec<u8>> = row.get("pkce_encrypted_dek");
    let ciphertext: Option<Vec<u8>> = row.get("pkce_ciphertext");
    let require_transaction = matches!(status.as_str(), "pending" | "processing");
    if require_transaction
        && (algorithm.is_none()
            || key_id.is_none()
            || key_version.is_none()
            || nonce.is_none()
            || encrypted_dek.is_none()
            || ciphertext.is_none())
    {
        return Err(AroError::Unexpected(
            "authorization transaction envelope is incomplete".into(),
        ));
    }
    Ok(AuthorizationAttemptSetup {
        attempt_id: row.get("id"),
        installation_id: row.get("installation_id"),
        organization_id: row.get("organization_id"),
        actor_user_id: row.get("actor_user_id"),
        provider_id: row.get("provider_id"),
        provider_version: row.get("provider_version"),
        auth_method: row.get("auth_method"),
        owner_type: row.get("owner_type"),
        owner_user_id: row.get("owner_user_id"),
        owner_team_id: row.get("owner_team_id"),
        capability_ids: row.get("capability_ids"),
        requested_scopes: row.get("requested_scopes"),
        state_digest: row.get("state_digest"),
        nonce_digest: row.get("nonce_digest"),
        descriptor: row.get("descriptor"),
        client_id: client_id.unwrap_or_default(),
        client_secret_ref: row.get("client_secret_ref"),
        callback_uri: row.get("callback_uri"),
        status,
        expires_at: row.get("expires_at"),
        credential_version: row.get("credential_version"),
        algorithm: algorithm.unwrap_or_default(),
        key_id: key_id.unwrap_or_default(),
        key_version: key_version.unwrap_or_default(),
        nonce: nonce.unwrap_or_default(),
        encrypted_dek: encrypted_dek.unwrap_or_default(),
        ciphertext: ciphertext.unwrap_or_default(),
    })
}

fn authorization_attempt_public_sql() -> &'static str {
    r#"
    SELECT jsonb_strip_nulls(jsonb_build_object(
      'id', id,
      'providerId', provider_id,
      'authMethod', auth_method,
      'owner', jsonb_strip_nulls(jsonb_build_object(
        'type', owner_type, 'userId', owner_user_id, 'teamId', owner_team_id
      )),
      'capabilityIds', capability_ids,
      'requestedScopes', requested_scopes,
      'status', status,
      'errorCode', error_code,
      'installationId', installation_result_id,
      'expiresAt', expires_at,
      'completedAt', completed_at,
      'createdAt', created_at
    ))
    FROM integration_authorization_attempts
    WHERE organization_id = $1 AND actor_user_id = $2 AND id = $3
    "#
}

async fn authorize_owner(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: TenantContext,
    membership_role: &str,
    owner_type: &str,
    owner_user_id: Option<Uuid>,
    owner_team_id: Option<Uuid>,
) -> AroResult<()> {
    let elevated = matches!(membership_role, "owner" | "admin");
    match owner_type {
        "user" if owner_user_id == Some(context.actor_id()) && owner_team_id.is_none() => Ok(()),
        "team" if owner_user_id.is_none() && owner_team_id.is_some() => {
            if elevated {
                return Ok(());
            }
            let manager = sqlx::query_scalar::<_, bool>(
                r#"
                SELECT EXISTS(
                  SELECT 1 FROM team_memberships
                  WHERE organization_id = $1 AND team_id = $2 AND user_id = $3
                    AND role = 'manager' AND deleted_at IS NULL
                )
                "#,
            )
            .bind(context.organization_id())
            .bind(owner_team_id)
            .bind(context.actor_id())
            .fetch_one(&mut **tx)
            .await
            .map_err(map_sqlx)?;
            if manager {
                Ok(())
            } else {
                Err(AroError::Security("team manager access required".into()))
            }
        }
        "organization" if owner_user_id.is_none() && owner_team_id.is_none() && elevated => Ok(()),
        _ => Err(AroError::Security(
            "invalid or unauthorized integration owner".into(),
        )),
    }
}

async fn ensure_installation_view_access(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: TenantContext,
    installation_id: Uuid,
) -> AroResult<()> {
    let visible = sqlx::query_scalar::<_, bool>(&format!(
        r#"
        SELECT EXISTS(
          SELECT 1 FROM integration_installations installation
          WHERE installation.organization_id = $1 AND installation.id = $3
            AND installation.deleted_at IS NULL AND {}
        )
        "#,
        installation_access_predicate_sql()
    ))
    .bind(context.organization_id())
    .bind(context.actor_id())
    .bind(installation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if visible {
        Ok(())
    } else {
        Err(AroError::Security("integration access denied".into()))
    }
}

async fn ensure_installation_manage_access(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: TenantContext,
    installation_id: Uuid,
) -> AroResult<()> {
    let allowed = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM integration_installations installation
          JOIN memberships membership
            ON membership.organization_id = installation.organization_id
           AND membership.user_id = $2 AND membership.status = 'active'
           AND membership.deleted_at IS NULL
          WHERE installation.organization_id = $1 AND installation.id = $3
            AND installation.deleted_at IS NULL
            AND (
              membership.role IN ('owner', 'admin')
              OR (installation.owner_type = 'user' AND installation.owner_user_id = $2)
              OR (installation.owner_type = 'team' AND EXISTS(
                SELECT 1 FROM team_memberships team_member
                WHERE team_member.organization_id = installation.organization_id
                  AND team_member.team_id = installation.owner_team_id
                  AND team_member.user_id = $2 AND team_member.role = 'manager'
                  AND team_member.deleted_at IS NULL
              ))
            )
        )
        "#,
    )
    .bind(context.organization_id())
    .bind(context.actor_id())
    .bind(installation_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_sqlx)?;
    if allowed {
        Ok(())
    } else {
        Err(AroError::Security(
            "integration management access denied".into(),
        ))
    }
}

async fn public_installation_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: TenantContext,
    installation_id: Uuid,
) -> AroResult<Option<Value>> {
    let query = format!(
        r#"
        SELECT {public_json}
        FROM integration_installations installation
        LEFT JOIN integration_external_accounts account
          ON account.organization_id = installation.organization_id
         AND account.id = installation.external_account_id
        LEFT JOIN integration_health_snapshots health
          ON health.organization_id = installation.organization_id
         AND health.installation_id = installation.id
        LEFT JOIN LATERAL (
          SELECT credential_type, granted_scopes, expires_at, last_used_at, created_at
          FROM integration_credential_versions credential
          WHERE credential.organization_id = installation.organization_id
            AND credential.installation_id = installation.id
            AND credential.current AND credential.revoked_at IS NULL
            AND credential.deleted_at IS NULL
          LIMIT 1
        ) credential ON true
        WHERE installation.organization_id = $1 AND installation.id = $3
          AND installation.deleted_at IS NULL AND {access_predicate}
        "#,
        public_json = installation_public_json_sql(),
        access_predicate = installation_access_predicate_sql(),
    );
    sqlx::query_scalar::<_, Value>(&query)
        .bind(context.organization_id())
        .bind(context.actor_id())
        .bind(installation_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(map_sqlx)
}

fn installation_public_json_sql() -> &'static str {
    r#"jsonb_strip_nulls(jsonb_build_object(
      'id', installation.id,
      'organizationId', installation.organization_id,
      'providerId', installation.provider_id,
      'providerVersion', installation.provider_version,
      'owner', jsonb_strip_nulls(jsonb_build_object(
        'type', installation.owner_type,
        'userId', installation.owner_user_id,
        'teamId', installation.owner_team_id
      )),
      'label', installation.label,
      'lifecycleStatus', installation.lifecycle_status,
      'healthStatus', COALESCE(health.status, installation.health_status),
      'enabled', installation.enabled,
      'publicConfig', installation.public_config,
      'account', CASE WHEN account.id IS NULL THEN NULL ELSE jsonb_strip_nulls(jsonb_build_object(
        'externalAccountId', account.external_account_id,
        'displayName', account.display_name,
        'realm', account.realm,
        'avatarUrl', account.avatar_url,
        'details', account.details
      )) END,
      'credential', CASE WHEN credential.credential_type IS NULL THEN NULL ELSE jsonb_strip_nulls(jsonb_build_object(
        'type', credential.credential_type,
        'grantedScopes', credential.granted_scopes,
        'expiresAt', credential.expires_at,
        'lastUsedAt', credential.last_used_at,
        'createdAt', credential.created_at
      )) END,
      'version', installation.version,
      'connectedAt', installation.connected_at,
      'revokedAt', installation.revoked_at,
      'createdAt', installation.created_at,
      'updatedAt', installation.updated_at
    ))"#
}

fn installation_access_predicate_sql() -> &'static str {
    r#"(
      EXISTS (
        SELECT 1 FROM memberships actor_membership
        WHERE actor_membership.organization_id = installation.organization_id
          AND actor_membership.user_id = $2
          AND actor_membership.status = 'active'
          AND actor_membership.deleted_at IS NULL
          AND actor_membership.role IN ('owner', 'admin')
      )
      OR (installation.owner_type = 'user' AND installation.owner_user_id = $2)
      OR (installation.owner_type = 'team' AND EXISTS (
        SELECT 1 FROM team_memberships team_member
        WHERE team_member.organization_id = installation.organization_id
          AND team_member.team_id = installation.owner_team_id
          AND team_member.user_id = $2 AND team_member.deleted_at IS NULL
      ))
      OR installation.owner_type = 'organization'
    )"#
}

fn validate_capabilities(descriptor: &Value, requested: &[String]) -> AroResult<()> {
    let available = descriptor
        .get("capabilities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if requested.iter().any(|requested_id| {
        !available.iter().any(|capability| {
            capability.get("id").and_then(Value::as_str) == Some(requested_id.as_str())
        })
    }) {
        return Err(AroError::Security("unknown connector capability".into()));
    }
    Ok(())
}

fn descriptor_auth_method(descriptor: &Value, credential_type: &str) -> bool {
    let expected = match credential_type {
        "api_key" => ["api_key", "personal_access_token"].as_slice(),
        "personal_access_token" => ["personal_access_token", "api_key"].as_slice(),
        "service_account" => ["service_account", "client_credentials"].as_slice(),
        "external_vault" => ["external_vault"].as_slice(),
        _ => return false,
    };
    descriptor
        .get("authMethods")
        .and_then(Value::as_array)
        .is_some_and(|methods| {
            methods
                .iter()
                .filter_map(Value::as_str)
                .any(|method| expected.contains(&method))
        })
}

fn descriptor_has_auth_method(descriptor: &Value, auth_method: &str) -> bool {
    matches!(
        auth_method,
        "authorization_code_pkce" | "open_id_connect" | "app_installation"
    ) && descriptor
        .get("authMethods")
        .and_then(Value::as_array)
        .is_some_and(|methods| {
            methods
                .iter()
                .filter_map(Value::as_str)
                .any(|method| method == auth_method)
        })
}

fn scopes_for_capabilities(descriptor: &Value, requested: &[String]) -> AroResult<Vec<String>> {
    let mut scopes = descriptor
        .get("oauth")
        .and_then(|oauth| oauth.get("defaultScopes"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let capabilities = descriptor
        .get("capabilities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for capability_id in requested {
        let capability = capabilities
            .iter()
            .find(|capability| capability.get("id").and_then(Value::as_str) == Some(capability_id))
            .ok_or_else(|| AroError::Security("unknown connector capability".into()))?;
        for scope in capability
            .get("scopes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !scopes.iter().any(|existing| existing == scope) {
                scopes.push(scope.to_string());
            }
        }
    }
    if scopes
        .iter()
        .any(|scope| scope.trim().is_empty() || scope.len() > 512)
    {
        return Err(AroError::Configuration(
            "connector manifest contains an invalid OAuth scope".into(),
        ));
    }
    Ok(scopes)
}

fn truncate_error(value: &str) -> String {
    value.chars().take(500).collect()
}
