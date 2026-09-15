-- Integration platform v3. This is an expand-only migration; v2 tables remain intact until the
-- explicit contract release.

CREATE TABLE connector_definitions (
  provider_id TEXT NOT NULL,
  version TEXT NOT NULL,
  descriptor JSONB NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT false,
  certification_status TEXT NOT NULL DEFAULT 'uncertified'
    CHECK (certification_status IN ('uncertified', 'testing', 'certified', 'suspended')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  PRIMARY KEY (provider_id, version),
  CHECK (provider_id ~ '^[a-z0-9][a-z0-9_-]{0,127}$'),
  CHECK (jsonb_typeof(descriptor) = 'object')
);

CREATE UNIQUE INDEX idx_connector_definitions_one_enabled
  ON connector_definitions(provider_id)
  WHERE enabled AND deleted_at IS NULL;

CREATE TABLE connector_client_profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL,
  provider_version TEXT NOT NULL,
  environment TEXT NOT NULL,
  realm TEXT NOT NULL DEFAULT 'default',
  client_id TEXT NOT NULL,
  client_secret_ref TEXT,
  callback_uris TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  enabled BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  FOREIGN KEY (provider_id, provider_version)
    REFERENCES connector_definitions(provider_id, version),
  UNIQUE NULLS NOT DISTINCT (organization_id, provider_id, environment, realm)
);

CREATE UNIQUE INDEX idx_teams_org_id_v3 ON teams(organization_id, id);

CREATE TABLE team_memberships (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  team_id UUID NOT NULL,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role TEXT NOT NULL DEFAULT 'member' CHECK (role IN ('member', 'manager')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  PRIMARY KEY (organization_id, team_id, user_id),
  FOREIGN KEY (organization_id, team_id) REFERENCES teams(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE enterprise_installation_templates (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  provider_id TEXT NOT NULL,
  provider_version TEXT NOT NULL,
  owner_scope TEXT NOT NULL CHECK (owner_scope IN ('enterprise', 'instance')),
  label TEXT NOT NULL,
  public_config JSONB NOT NULL DEFAULT '{}'::jsonb,
  capability_ids TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  credential_external_ref TEXT,
  enabled BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  FOREIGN KEY (provider_id, provider_version)
    REFERENCES connector_definitions(provider_id, version)
);

CREATE TABLE integration_external_accounts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL,
  external_account_id TEXT NOT NULL,
  realm TEXT NOT NULL DEFAULT 'default',
  display_name TEXT,
  avatar_url TEXT,
  details JSONB NOT NULL DEFAULT '{}'::jsonb,
  first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_seen_at TIMESTAMPTZ,
  deleted_at TIMESTAMPTZ,
  UNIQUE (organization_id, id),
  UNIQUE (organization_id, provider_id, realm, external_account_id)
);

CREATE TABLE integration_installations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL,
  provider_version TEXT NOT NULL,
  created_by_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  creation_idempotency_key TEXT,
  creation_request_hash TEXT,
  owner_type TEXT NOT NULL CHECK (owner_type IN ('user', 'team', 'organization')),
  owner_user_id UUID REFERENCES users(id) ON DELETE RESTRICT,
  owner_team_id UUID,
  external_account_id UUID,
  enterprise_template_id UUID REFERENCES enterprise_installation_templates(id) ON DELETE SET NULL,
  label TEXT,
  lifecycle_status TEXT NOT NULL DEFAULT 'pending'
    CHECK (lifecycle_status IN ('pending', 'active', 'reauthorization_required', 'disconnecting', 'revoked', 'deleted')),
  health_status TEXT NOT NULL DEFAULT 'unknown'
    CHECK (health_status IN ('unknown', 'healthy', 'degraded', 'provider_unavailable', 'credentials_expired', 'permissions_insufficient', 'invalid_credentials', 'sync_error')),
  enabled BOOLEAN NOT NULL DEFAULT true,
  public_config JSONB NOT NULL DEFAULT '{}'::jsonb,
  version BIGINT NOT NULL DEFAULT 1,
  connected_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  UNIQUE (organization_id, id),
  UNIQUE (organization_id, created_by_user_id, creation_idempotency_key),
  FOREIGN KEY (provider_id, provider_version)
    REFERENCES connector_definitions(provider_id, version),
  FOREIGN KEY (organization_id, owner_team_id)
    REFERENCES teams(organization_id, id),
  FOREIGN KEY (organization_id, external_account_id)
    REFERENCES integration_external_accounts(organization_id, id),
  CHECK (
    (owner_type = 'user' AND owner_user_id IS NOT NULL AND owner_team_id IS NULL)
    OR (owner_type = 'team' AND owner_user_id IS NULL AND owner_team_id IS NOT NULL)
    OR (owner_type = 'organization' AND owner_user_id IS NULL AND owner_team_id IS NULL)
  )
);

CREATE INDEX idx_integration_installations_org_status
  ON integration_installations(organization_id, lifecycle_status, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_integration_installations_owner_user
  ON integration_installations(organization_id, owner_user_id, updated_at DESC)
  WHERE owner_type = 'user' AND deleted_at IS NULL;
CREATE INDEX idx_integration_installations_owner_team
  ON integration_installations(organization_id, owner_team_id, updated_at DESC)
  WHERE owner_type = 'team' AND deleted_at IS NULL;

CREATE TABLE integration_credential_versions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  installation_id UUID NOT NULL,
  version INTEGER NOT NULL CHECK (version > 0),
  credential_type TEXT NOT NULL,
  algorithm TEXT NOT NULL,
  key_id TEXT NOT NULL,
  key_version TEXT NOT NULL,
  nonce BYTEA NOT NULL,
  encrypted_dek BYTEA NOT NULL,
  ciphertext BYTEA NOT NULL,
  granted_scopes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  expires_at TIMESTAMPTZ,
  current BOOLEAN NOT NULL DEFAULT true,
  last_used_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  UNIQUE (organization_id, id),
  UNIQUE (organization_id, installation_id, version),
  FOREIGN KEY (organization_id, installation_id)
    REFERENCES integration_installations(organization_id, id) ON DELETE CASCADE,
  CHECK (octet_length(nonce) = 12),
  CHECK (octet_length(encrypted_dek) > 0),
  CHECK (octet_length(ciphertext) > 0)
);

CREATE UNIQUE INDEX idx_integration_credential_versions_current
  ON integration_credential_versions(organization_id, installation_id)
  WHERE current AND revoked_at IS NULL AND deleted_at IS NULL;
CREATE INDEX idx_integration_credential_versions_refresh_due
  ON integration_credential_versions(expires_at, organization_id, installation_id)
  WHERE current AND revoked_at IS NULL AND deleted_at IS NULL AND expires_at IS NOT NULL;

CREATE TABLE integration_authorization_attempts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  actor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  installation_id UUID,
  provider_id TEXT NOT NULL,
  provider_version TEXT NOT NULL,
  owner_type TEXT NOT NULL CHECK (owner_type IN ('user', 'team', 'organization')),
  owner_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  owner_team_id UUID,
  auth_method TEXT NOT NULL,
  capability_ids TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  requested_scopes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  state_digest TEXT NOT NULL UNIQUE,
  nonce_digest TEXT,
  pkce_algorithm TEXT,
  pkce_encryption_algorithm TEXT,
  pkce_key_id TEXT,
  pkce_key_version TEXT,
  pkce_nonce BYTEA,
  pkce_encrypted_dek BYTEA,
  pkce_ciphertext BYTEA,
  callback_uri TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'processing', 'authorized', 'denied', 'failed', 'expired', 'cancelled')),
  error_code TEXT,
  error_detail TEXT,
  installation_result_id UUID,
  idempotency_key TEXT,
  request_hash TEXT,
  expires_at TIMESTAMPTZ NOT NULL,
  claimed_at TIMESTAMPTZ,
  claim_token UUID,
  completed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, id),
  UNIQUE (organization_id, actor_user_id, idempotency_key),
  FOREIGN KEY (provider_id, provider_version)
    REFERENCES connector_definitions(provider_id, version),
  FOREIGN KEY (organization_id, installation_id)
    REFERENCES integration_installations(organization_id, id),
  FOREIGN KEY (organization_id, owner_team_id)
    REFERENCES teams(organization_id, id),
  FOREIGN KEY (organization_id, installation_result_id)
    REFERENCES integration_installations(organization_id, id),
  CHECK (
    (owner_type = 'user' AND owner_user_id IS NOT NULL AND owner_team_id IS NULL)
    OR (owner_type = 'team' AND owner_user_id IS NULL AND owner_team_id IS NOT NULL)
    OR (owner_type = 'organization' AND owner_user_id IS NULL AND owner_team_id IS NULL)
  )
);

CREATE INDEX idx_integration_authorization_attempts_pending
  ON integration_authorization_attempts(expires_at)
  WHERE status IN ('pending', 'processing');

-- Provider callbacks are unauthenticated and only carry a high-entropy state value. This narrow
-- security-definer function resolves and claims that opaque value without granting the API role
-- cross-tenant table access. A stale claim may be fenced and recovered after five minutes.
CREATE OR REPLACE FUNCTION public.aro_claim_integration_authorization_attempt(
  requested_state_digest TEXT,
  requested_provider_id TEXT,
  requested_claim_token UUID
) RETURNS SETOF integration_authorization_attempts
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $function$
BEGIN
  RETURN QUERY
  UPDATE public.integration_authorization_attempts attempt
  SET status = CASE WHEN attempt.expires_at <= now() THEN 'expired' ELSE 'processing' END,
      error_code = CASE WHEN attempt.expires_at <= now() THEN 'authorization_expired' ELSE NULL END,
      error_detail = CASE WHEN attempt.expires_at <= now() THEN 'Authorization attempt expired' ELSE NULL END,
      claimed_at = CASE WHEN attempt.expires_at <= now() THEN attempt.claimed_at ELSE now() END,
      claim_token = CASE WHEN attempt.expires_at <= now() THEN NULL ELSE requested_claim_token END,
      completed_at = CASE WHEN attempt.expires_at <= now() THEN now() ELSE NULL END
  WHERE attempt.state_digest = requested_state_digest
    AND attempt.provider_id = requested_provider_id
    AND (
      attempt.status = 'pending'
      OR (
        attempt.status = 'processing'
        AND attempt.claimed_at < now() - interval '5 minutes'
      )
    )
  RETURNING attempt.*;
  IF FOUND THEN
    RETURN;
  END IF;

  RETURN QUERY
  SELECT attempt.*
  FROM public.integration_authorization_attempts attempt
  WHERE attempt.state_digest = requested_state_digest
    AND attempt.provider_id = requested_provider_id;
END
$function$;

REVOKE ALL ON FUNCTION public.aro_claim_integration_authorization_attempt(TEXT, TEXT, UUID) FROM PUBLIC;

CREATE TABLE integration_capability_grants (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  installation_id UUID NOT NULL,
  capability_id TEXT NOT NULL,
  granted_scopes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  granted_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ,
  PRIMARY KEY (organization_id, installation_id, capability_id),
  FOREIGN KEY (organization_id, installation_id)
    REFERENCES integration_installations(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE integration_health_snapshots (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  installation_id UUID NOT NULL,
  status TEXT NOT NULL,
  error_code TEXT,
  retry_at TIMESTAMPTZ,
  checked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  details JSONB NOT NULL DEFAULT '{}'::jsonb,
  PRIMARY KEY (organization_id, installation_id),
  FOREIGN KEY (organization_id, installation_id)
    REFERENCES integration_installations(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE integration_health_history (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  installation_id UUID NOT NULL,
  status TEXT NOT NULL,
  error_code TEXT,
  checked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  FOREIGN KEY (organization_id, installation_id)
    REFERENCES integration_installations(organization_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_integration_health_history_retention
  ON integration_health_history(checked_at, organization_id);

CREATE TABLE integration_jobs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  installation_id UUID,
  job_type TEXT NOT NULL CHECK (job_type IN ('refresh', 'health', 'sync', 'revoke', 'webhook', 'compensate')),
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'leased', 'retry', 'completed', 'dead_letter', 'cancelled')),
  payload JSONB NOT NULL DEFAULT '{}'::jsonb,
  attempt_count INTEGER NOT NULL DEFAULT 0,
  max_attempts INTEGER NOT NULL DEFAULT 8,
  available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  lease_owner TEXT,
  lease_token UUID,
  lease_expires_at TIMESTAMPTZ,
  last_error_code TEXT,
  last_error_detail TEXT,
  completed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  FOREIGN KEY (organization_id, installation_id)
    REFERENCES integration_installations(organization_id, id) ON DELETE CASCADE
);

CREATE INDEX idx_integration_jobs_claim
  ON integration_jobs(job_type, available_at, created_at)
  WHERE status IN ('pending', 'retry');
CREATE INDEX idx_integration_jobs_lease_expiry
  ON integration_jobs(lease_expires_at)
  WHERE status = 'leased';

CREATE TABLE integration_webhook_subscriptions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  installation_id UUID NOT NULL,
  provider_subscription_id TEXT,
  event_types TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  secret_credential_version_id UUID,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'active', 'degraded', 'revoked')),
  expires_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  UNIQUE (organization_id, id),
  FOREIGN KEY (organization_id, installation_id)
    REFERENCES integration_installations(organization_id, id) ON DELETE CASCADE,
  FOREIGN KEY (organization_id, secret_credential_version_id)
    REFERENCES integration_credential_versions(organization_id, id)
);

CREATE TABLE integration_webhook_deliveries (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  subscription_id UUID NOT NULL,
  provider_event_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  signature_valid BOOLEAN NOT NULL,
  received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  processed_at TIMESTAMPTZ,
  payload JSONB NOT NULL,
  UNIQUE (organization_id, subscription_id, provider_event_id),
  FOREIGN KEY (organization_id, subscription_id)
    REFERENCES integration_webhook_subscriptions(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE integration_access_grants (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  installation_id UUID NOT NULL,
  grantee_type TEXT NOT NULL CHECK (grantee_type IN ('user', 'team', 'organization')),
  grantee_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  grantee_team_id UUID,
  capability_ids TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ,
  FOREIGN KEY (organization_id, installation_id)
    REFERENCES integration_installations(organization_id, id) ON DELETE CASCADE,
  FOREIGN KEY (organization_id, grantee_team_id)
    REFERENCES teams(organization_id, id),
  CHECK (
    (grantee_type = 'user' AND grantee_user_id IS NOT NULL AND grantee_team_id IS NULL)
    OR (grantee_type = 'team' AND grantee_user_id IS NULL AND grantee_team_id IS NOT NULL)
    OR (grantee_type = 'organization' AND grantee_user_id IS NULL AND grantee_team_id IS NULL)
  )
);

CREATE UNIQUE INDEX idx_integration_access_grants_unique
  ON integration_access_grants(
    organization_id,
    installation_id,
    grantee_type,
    COALESCE(grantee_user_id, '00000000-0000-0000-0000-000000000000'::uuid),
    COALESCE(grantee_team_id, '00000000-0000-0000-0000-000000000000'::uuid)
  ) WHERE revoked_at IS NULL;

-- Seed manifests remain disabled until their client profile and certification suite are present.
INSERT INTO connector_definitions(provider_id, version, descriptor, enabled, certification_status)
VALUES
  ('github', '2026-07-v3', '{"id":"github","version":"2026-07-v3","displayName":"GitHub","description":"GitHub App installation","category":"Development","authMethods":["app_installation","authorization_code_pkce"],"permissions":[],"capabilities":[],"allowedHosts":["github.com","api.github.com"]}'::jsonb, false, 'testing'),
  ('google-workspace', '2026-07-v3', '{"id":"google-workspace","version":"2026-07-v3","displayName":"Google Workspace","description":"Google Workspace OIDC","category":"Productivity","authMethods":["open_id_connect"],"permissions":[],"capabilities":[],"allowedHosts":["accounts.google.com","oauth2.googleapis.com","openidconnect.googleapis.com"]}'::jsonb, false, 'testing'),
  ('microsoft-365', '2026-07-v3', '{"id":"microsoft-365","version":"2026-07-v3","displayName":"Microsoft 365","description":"Microsoft identity platform","category":"Productivity","authMethods":["open_id_connect","device_code"],"permissions":[],"capabilities":[],"allowedHosts":["login.microsoftonline.com","graph.microsoft.com"]}'::jsonb, false, 'testing'),
  ('slack', '2026-07-v3', '{"id":"slack","version":"2026-07-v3","displayName":"Slack","description":"Slack workspace installation","category":"Communication","authMethods":["app_installation"],"permissions":[],"capabilities":[],"allowedHosts":["slack.com","api.slack.com"]}'::jsonb, false, 'testing'),
  ('notion', '2026-07-v3', '{"id":"notion","version":"2026-07-v3","displayName":"Notion","description":"Notion public integration","category":"Productivity","authMethods":["authorization_code_pkce"],"permissions":[],"capabilities":[],"allowedHosts":["api.notion.com"]}'::jsonb, false, 'testing'),
  ('atlassian-jira', '2026-07-v3', '{"id":"atlassian-jira","version":"2026-07-v3","displayName":"Atlassian Jira","description":"Atlassian OAuth 2.0 3LO","category":"Development","authMethods":["authorization_code_pkce"],"permissions":[],"capabilities":[],"allowedHosts":["auth.atlassian.com","api.atlassian.com"]}'::jsonb, false, 'testing')
ON CONFLICT (provider_id, version) DO NOTHING;

-- Tenant tables enforce the organization boundary at the database layer. API and worker
-- transactions must set aro.organization_id; application RBAC further narrows owner access.
DO $integration_v3_rls$
DECLARE
  table_name TEXT;
BEGIN
  ALTER TABLE connector_client_profiles ENABLE ROW LEVEL SECURITY;
  FOREACH table_name IN ARRAY ARRAY[
    'team_memberships', 'integration_external_accounts', 'integration_installations',
    'integration_credential_versions', 'integration_authorization_attempts',
    'integration_capability_grants', 'integration_health_snapshots',
    'integration_health_history', 'integration_jobs', 'integration_webhook_subscriptions',
    'integration_webhook_deliveries', 'integration_access_grants'
  ] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
      EXECUTE format('CREATE POLICY %I ON %I TO aro_app USING (organization_id = public.aro_private_context_uuid(''aro.organization_id'')) WITH CHECK (organization_id = public.aro_private_context_uuid(''aro.organization_id''))', table_name || '_tenant_app', table_name);
    END IF;
  END LOOP;

  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    CREATE POLICY connector_client_profiles_tenant_app ON connector_client_profiles TO aro_app
      USING (
        organization_id IS NULL
        OR organization_id = public.aro_private_context_uuid('aro.organization_id')
      )
      WITH CHECK (
        organization_id = public.aro_private_context_uuid('aro.organization_id')
      );
    GRANT EXECUTE ON FUNCTION public.aro_claim_integration_authorization_attempt(TEXT, TEXT, UUID) TO aro_app;
  END IF;

  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker') THEN
    CREATE POLICY connector_client_profiles_worker ON connector_client_profiles TO aro_worker
      USING (true) WITH CHECK (true);
    FOREACH table_name IN ARRAY ARRAY[
      'integration_installations', 'integration_credential_versions', 'integration_health_snapshots',
      'integration_health_history', 'integration_jobs', 'integration_webhook_subscriptions',
      'integration_webhook_deliveries'
    ] LOOP
      EXECUTE format('CREATE POLICY %I ON %I TO aro_worker USING (true) WITH CHECK (true)', table_name || '_worker', table_name);
    END LOOP;
  END IF;
END
$integration_v3_rls$;

DO $integration_v3_grants$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    EXECUTE 'GRANT SELECT ON connector_definitions TO aro_app';
    EXECUTE 'GRANT SELECT, INSERT, UPDATE ON connector_client_profiles TO aro_app';
    EXECUTE 'GRANT SELECT ON enterprise_installation_templates TO aro_app';
    EXECUTE 'GRANT SELECT, INSERT, UPDATE, DELETE ON team_memberships, integration_external_accounts,
      integration_installations, integration_credential_versions, integration_authorization_attempts,
      integration_capability_grants, integration_health_snapshots, integration_health_history,
      integration_jobs, integration_webhook_subscriptions, integration_webhook_deliveries,
      integration_access_grants TO aro_app';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker') THEN
    EXECUTE 'GRANT SELECT ON connector_definitions, connector_client_profiles TO aro_worker';
    EXECUTE 'GRANT SELECT, INSERT, UPDATE ON integration_installations,
      integration_credential_versions, integration_health_snapshots, integration_health_history,
      integration_jobs, integration_webhook_subscriptions, integration_webhook_deliveries TO aro_worker';
  END IF;
END
$integration_v3_grants$;
