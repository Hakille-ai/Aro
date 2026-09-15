CREATE TABLE integration_providers (
  provider_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  kind TEXT NOT NULL,
  display_name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  category TEXT NOT NULL DEFAULT 'custom',
  icon TEXT,
  auth JSONB NOT NULL DEFAULT '{}'::jsonb,
  permissions JSONB NOT NULL DEFAULT '[]'::jsonb,
  capabilities JSONB NOT NULL DEFAULT '[]'::jsonb,
  enabled BOOLEAN NOT NULL DEFAULT true,
  source TEXT NOT NULL DEFAULT 'seed',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  PRIMARY KEY (provider_id, manifest_version)
);

CREATE TABLE integration_accounts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL,
  external_account_id TEXT,
  display_name TEXT,
  email TEXT,
  avatar_url TEXT,
  details JSONB NOT NULL DEFAULT '{}'::jsonb,
  first_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_seen_at TIMESTAMPTZ,
  deleted_at TIMESTAMPTZ,
  UNIQUE (organization_id, id),
  UNIQUE (organization_id, provider_id, external_account_id)
);

CREATE TABLE integration_connections (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  provider_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  account_id UUID,
  status TEXT NOT NULL DEFAULT 'needs-auth',
  enabled BOOLEAN NOT NULL DEFAULT true,
  granted_scopes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  public_config JSONB NOT NULL DEFAULT '{}'::jsonb,
  last_checked_at TIMESTAMPTZ,
  connected_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  legacy_plugin_connection_id UUID REFERENCES plugin_connections(id) ON DELETE SET NULL,
  client_id TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  UNIQUE (organization_id, id),
  FOREIGN KEY (provider_id, manifest_version)
    REFERENCES integration_providers(provider_id, manifest_version),
  FOREIGN KEY (organization_id, account_id)
    REFERENCES integration_accounts(organization_id, id)
);

CREATE TABLE integration_credentials (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  connection_id UUID NOT NULL,
  account_id UUID,
  provider_id TEXT NOT NULL,
  credential_type TEXT NOT NULL,
  store TEXT NOT NULL,
  key_id TEXT,
  key_version TEXT,
  encrypted_payload BYTEA,
  external_ref TEXT,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  scopes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  expires_at TIMESTAMPTZ,
  last_used_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  FOREIGN KEY (organization_id, connection_id)
    REFERENCES integration_connections(organization_id, id),
  FOREIGN KEY (organization_id, account_id)
    REFERENCES integration_accounts(organization_id, id)
);

CREATE TABLE integration_oauth_states (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL,
  manifest_version TEXT NOT NULL,
  connection_id UUID,
  state_hash TEXT NOT NULL UNIQUE,
  nonce_hash TEXT,
  pkce_verifier_encrypted BYTEA,
  redirect_uri TEXT NOT NULL,
  requested_scopes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  status TEXT NOT NULL DEFAULT 'pending',
  error TEXT,
  expires_at TIMESTAMPTZ NOT NULL,
  consumed_at TIMESTAMPTZ,
  completed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  FOREIGN KEY (provider_id, manifest_version)
    REFERENCES integration_providers(provider_id, manifest_version),
  FOREIGN KEY (organization_id, connection_id)
    REFERENCES integration_connections(organization_id, id)
);

CREATE INDEX idx_integration_providers_catalog
  ON integration_providers(category, display_name)
  WHERE deleted_at IS NULL AND enabled;

CREATE INDEX idx_integration_connections_org_updated
  ON integration_connections(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_integration_connections_provider_status
  ON integration_connections(organization_id, provider_id, status)
  WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX idx_integration_connections_client_id
  ON integration_connections(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_integration_connections_legacy_plugin
  ON integration_connections(organization_id, legacy_plugin_connection_id)
  WHERE deleted_at IS NULL AND legacy_plugin_connection_id IS NOT NULL;

CREATE INDEX idx_integration_credentials_connection_active
  ON integration_credentials(organization_id, connection_id)
  WHERE deleted_at IS NULL AND revoked_at IS NULL;

CREATE INDEX idx_integration_credentials_expiry
  ON integration_credentials(expires_at)
  WHERE deleted_at IS NULL AND revoked_at IS NULL AND expires_at IS NOT NULL;

CREATE INDEX idx_integration_oauth_states_pending_expiry
  ON integration_oauth_states(expires_at)
  WHERE status = 'pending';

CREATE OR REPLACE FUNCTION integration_connection_public_json(
  c integration_connections,
  a integration_accounts,
  cred integration_credentials
) RETURNS jsonb AS $$
  SELECT jsonb_strip_nulls(jsonb_build_object(
    'id', c.id,
    'organizationId', c.organization_id,
    'ownerUserId', c.owner_user_id,
    'providerId', c.provider_id,
    'manifestVersion', c.manifest_version,
    'status', c.status,
    'enabled', c.enabled,
    'grantedScopes', c.granted_scopes,
    'account', jsonb_strip_nulls(jsonb_build_object(
      'externalAccountId', a.external_account_id,
      'displayName', a.display_name,
      'email', a.email,
      'avatarUrl', a.avatar_url,
      'details', NULLIF(a.details::text, '{}'::text)
    )),
    'credential', CASE
      WHEN cred.id IS NULL THEN NULL
      ELSE jsonb_strip_nulls(jsonb_build_object(
        'id', cred.id,
        'type', cred.credential_type,
        'store', cred.store,
        'keyId', cred.key_id,
        'keyVersion', cred.key_version,
        'scopes', cred.scopes,
        'expiresAt', cred.expires_at,
        'lastUsedAt', cred.last_used_at,
        'revokedAt', cred.revoked_at,
        'createdAt', cred.created_at
      ))
    END,
    'publicConfig', c.public_config,
    'lastCheckedAt', c.last_checked_at,
    'connectedAt', c.connected_at,
    'revokedAt', c.revoked_at,
    'createdAt', c.created_at,
    'updatedAt', c.updated_at
  ));
$$ LANGUAGE sql STABLE;

DROP TRIGGER IF EXISTS trg_integration_providers_updated_at ON integration_providers;
CREATE TRIGGER trg_integration_providers_updated_at
  BEFORE UPDATE ON integration_providers
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_integration_credentials_updated_at ON integration_credentials;
CREATE TRIGGER trg_integration_credentials_updated_at
  BEFORE UPDATE ON integration_credentials
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_integration_connections_updated_at ON integration_connections;
CREATE TRIGGER trg_integration_connections_updated_at
  BEFORE UPDATE ON integration_connections
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

INSERT INTO integration_providers (
  provider_id, manifest_version, kind, display_name, description, category, icon,
  auth, permissions, capabilities, source
) VALUES
(
  'github',
  '2026-07-01',
  'app',
  'GitHub',
  'Connect repositories, issues, pull requests, and code search.',
  'Development',
  'github',
  '{
    "mode": "oauth2",
    "credentialStore": "server-vault",
    "oauth": {
      "authorizationUrl": "https://github.com/login/oauth/authorize",
      "tokenUrl": "https://github.com/login/oauth/access_token",
      "userinfoUrl": "https://api.github.com/user",
      "revocationUrl": null,
      "redirectPath": "/integrations/github/oauth/callback",
      "defaultScopes": ["read:user", "repo"],
      "pkceRequired": true,
      "oidc": false
    },
    "configFields": []
  }'::jsonb,
  '[
    {"id":"github.profile.read","label":"Read profile","description":"Show the connected GitHub account.","required":true,"sensitive":false},
    {"id":"github.repo.read","label":"Read repositories","description":"Search repositories, issues, and pull requests.","required":true,"sensitive":true}
  ]'::jsonb,
  '[
    {"id":"github.repo.search","label":"Search repositories","description":"Find repositories, issues, pull requests, and code references.","readOnly":true,"permissionIds":["github.repo.read"]}
  ]'::jsonb,
  'seed'
),
(
  'google-workspace',
  '2026-07-01',
  'app',
  'Google Workspace',
  'Connect Drive, Docs, Sheets, Slides, Calendar, and Keep.',
  'Productivity',
  'google',
  '{
    "mode": "oidc",
    "credentialStore": "server-vault",
    "oauth": {
      "authorizationUrl": "https://accounts.google.com/o/oauth2/v2/auth",
      "tokenUrl": "https://oauth2.googleapis.com/token",
      "userinfoUrl": "https://openidconnect.googleapis.com/v1/userinfo",
      "revocationUrl": "https://oauth2.googleapis.com/revoke",
      "redirectPath": "/integrations/google-workspace/oauth/callback",
      "defaultScopes": ["openid", "email", "profile", "https://www.googleapis.com/auth/drive.metadata.readonly"],
      "pkceRequired": true,
      "oidc": true
    },
    "configFields": []
  }'::jsonb,
  '[
    {"id":"google.profile.read","label":"Read profile","description":"Show the connected Google account.","required":true,"sensitive":false},
    {"id":"google.drive.metadata.read","label":"Read Drive metadata","description":"List accessible Drive documents without downloading file content by default.","required":true,"sensitive":true}
  ]'::jsonb,
  '[
    {"id":"google.drive.search","label":"Search Drive","description":"Find Drive files the user authorized.","readOnly":true,"permissionIds":["google.drive.metadata.read"]}
  ]'::jsonb,
  'seed'
)
ON CONFLICT (provider_id, manifest_version) DO NOTHING;

WITH legacy_providers AS (
  SELECT DISTINCT
    CASE
      WHEN client_id = 'plg-github' THEN 'github'
      WHEN client_id = 'plg-google' THEN 'google-workspace'
      WHEN client_id LIKE 'plg-%' THEN regexp_replace(client_id, '^plg-', '')
      ELSE COALESCE(client_id, regexp_replace(lower(name), '[^a-z0-9]+', '-', 'g'))
    END AS provider_id,
    name,
    description,
    category,
    auth_type,
    config
  FROM plugin_connections
  WHERE deleted_at IS NULL
)
INSERT INTO integration_providers (
  provider_id, manifest_version, kind, display_name, description, category, icon,
  auth, permissions, capabilities, source
)
SELECT
  provider_id,
  'legacy',
  'app',
  COALESCE(NULLIF(name, ''), provider_id),
  COALESCE(description, ''),
  COALESCE(category, 'custom'),
  NULL,
  jsonb_build_object(
    'mode', CASE
      WHEN auth_type = 'oauth' THEN 'oauth2'
      WHEN auth_type = 'api_key' THEN 'api-key'
      ELSE 'none'
    END,
    'credentialStore', CASE WHEN auth_type = 'none' THEN 'none' ELSE 'server-vault' END,
    'oauth', NULL,
    'configFields', COALESCE(config->'configFields', '[]'::jsonb)
  ),
  COALESCE(config->'subServices', '[]'::jsonb),
  COALESCE(config->'subServices', '[]'::jsonb),
  'legacy'
FROM legacy_providers
ON CONFLICT (provider_id, manifest_version) DO NOTHING;

WITH legacy_plugins AS (
  SELECT
    plugin.*,
    CASE
      WHEN plugin.client_id = 'plg-github' THEN 'github'
      WHEN plugin.client_id = 'plg-google' THEN 'google-workspace'
      WHEN plugin.client_id LIKE 'plg-%' THEN regexp_replace(plugin.client_id, '^plg-', '')
      ELSE COALESCE(plugin.client_id, regexp_replace(lower(plugin.name), '[^a-z0-9]+', '-', 'g'))
    END AS provider_id
  FROM plugin_connections plugin
  WHERE plugin.deleted_at IS NULL
),
legacy_accounts AS (
  INSERT INTO integration_accounts (
    organization_id, provider_id, external_account_id, display_name, avatar_url, details, last_seen_at
  )
  SELECT
    organization_id,
    provider_id,
    COALESCE(client_id, id::text),
    config->>'profileName',
    config->>'profileAvatar',
    jsonb_strip_nulls(jsonb_build_object('details', config->>'profileDetails')),
    updated_at
  FROM legacy_plugins
  WHERE config ? 'profileName' OR config ? 'profileAvatar' OR config ? 'profileDetails'
  ON CONFLICT (organization_id, provider_id, external_account_id) DO UPDATE SET
    display_name = COALESCE(excluded.display_name, integration_accounts.display_name),
    avatar_url = COALESCE(excluded.avatar_url, integration_accounts.avatar_url),
    details = excluded.details,
    last_seen_at = excluded.last_seen_at
  RETURNING id, organization_id, provider_id, external_account_id
)
INSERT INTO integration_connections (
  id, organization_id, provider_id, manifest_version, account_id, status, enabled,
  granted_scopes, public_config, last_checked_at, connected_at, legacy_plugin_connection_id,
  client_id, created_at, updated_at
)
SELECT
  legacy.id,
  legacy.organization_id,
  legacy.provider_id,
  CASE WHEN provider.provider_id IS NULL THEN 'legacy' ELSE provider.manifest_version END,
  account.id,
  CASE
    WHEN legacy.enabled = false THEN 'disabled'
    WHEN legacy.status = 'connected' THEN 'connected'
    WHEN legacy.auth_type = 'none' THEN 'needs-config'
    ELSE 'needs-auth'
  END,
  legacy.enabled,
  ARRAY[]::TEXT[],
  legacy.config,
  legacy.updated_at,
  CASE WHEN legacy.status = 'connected' THEN legacy.updated_at ELSE NULL END,
  legacy.id,
  legacy.client_id,
  legacy.created_at,
  legacy.updated_at
FROM legacy_plugins legacy
LEFT JOIN legacy_accounts account
  ON account.organization_id = legacy.organization_id
  AND account.provider_id = legacy.provider_id
  AND account.external_account_id = COALESCE(legacy.client_id, legacy.id::text)
LEFT JOIN LATERAL (
  SELECT provider_id, manifest_version
  FROM integration_providers provider
  WHERE provider.provider_id = legacy.provider_id
  ORDER BY CASE WHEN provider.manifest_version = '2026-07-01' THEN 0 ELSE 1 END
  LIMIT 1
) provider ON true
ON CONFLICT (id) DO NOTHING;

INSERT INTO integration_credentials (
  organization_id, connection_id, account_id, provider_id, credential_type, store,
  key_id, key_version, encrypted_payload, metadata, created_at, updated_at
)
SELECT
  plugin.organization_id,
  connection.id,
  connection.account_id,
  connection.provider_id,
  'legacy-secret',
  'server-vault',
  'aro-secrets-key',
  'legacy-pgp',
  plugin.encrypted_secret,
  jsonb_build_object('source', 'plugin_connections.encrypted_secret'),
  plugin.created_at,
  plugin.updated_at
FROM plugin_connections plugin
JOIN integration_connections connection
  ON connection.organization_id = plugin.organization_id
  AND connection.legacy_plugin_connection_id = plugin.id
WHERE plugin.encrypted_secret IS NOT NULL
  AND plugin.deleted_at IS NULL;
