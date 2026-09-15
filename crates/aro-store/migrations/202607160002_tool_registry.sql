-- Versioned, tenant-scoped tool descriptor registry.
--
-- Core descriptors continue to ship in aro-agent as a read-only fallback. Tenant entries can add
-- or override a canonical tool id, but activation in the execution path is a later migration.

CREATE TABLE tool_registry_entries (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  tool_id TEXT NOT NULL,
  tool_version TEXT NOT NULL,
  descriptor_schema_version INTEGER NOT NULL,
  descriptor JSONB NOT NULL,
  descriptor_hash TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'disabled', 'deprecated', 'revoked')),
  provenance_kind TEXT NOT NULL CHECK (provenance_kind IN (
    'core', 'plugin', 'mcp', 'skill', 'user', 'organization', 'generated'
  )),
  owner_kind TEXT NOT NULL CHECK (owner_kind IN (
    'platform', 'team', 'organization', 'user', 'publisher'
  )),
  is_current BOOLEAN NOT NULL DEFAULT true,
  created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ,
  UNIQUE (organization_id, id),
  UNIQUE (organization_id, tool_id, tool_version),
  CHECK (tool_id ~ '^[a-z0-9][a-z0-9_-]*(\.[a-z0-9][a-z0-9_-]*)+$'),
  CHECK (char_length(tool_id) <= 200),
  CHECK (char_length(tool_version) <= 64),
  CHECK (descriptor_schema_version > 0),
  CHECK (jsonb_typeof(descriptor) = 'object'),
  CHECK (descriptor_hash ~ '^[0-9a-f]{64}$'),
  CHECK ((status = 'revoked') = (revoked_at IS NOT NULL))
);

CREATE UNIQUE INDEX uq_tool_registry_current
  ON tool_registry_entries (organization_id, tool_id)
  WHERE is_current;

CREATE INDEX idx_tool_registry_status
  ON tool_registry_entries (organization_id, status, tool_id)
  WHERE is_current;

CREATE TABLE tool_registry_aliases (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  alias TEXT NOT NULL,
  entry_id UUID NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (organization_id, alias),
  FOREIGN KEY (organization_id, entry_id)
    REFERENCES tool_registry_entries(organization_id, id) ON DELETE CASCADE,
  CHECK (alias ~ '^[a-z0-9][a-z0-9_-]*(\.[a-z0-9][a-z0-9_-]*)+$'),
  CHECK (char_length(alias) <= 200)
);

CREATE INDEX idx_tool_registry_alias_entry
  ON tool_registry_aliases (organization_id, entry_id);

CREATE TRIGGER tool_registry_entries_touch_updated_at
BEFORE UPDATE ON tool_registry_entries
FOR EACH ROW EXECUTE FUNCTION aro_touch_updated_at();

ALTER TABLE tool_registry_entries ENABLE ROW LEVEL SECURITY;
ALTER TABLE tool_registry_aliases ENABLE ROW LEVEL SECURITY;

CREATE POLICY tool_registry_entries_context_present
  ON tool_registry_entries AS PERMISSIVE FOR ALL
  USING (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  )
  WITH CHECK (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  );

CREATE POLICY tool_registry_entries_tenant_boundary
  ON tool_registry_entries AS RESTRICTIVE FOR ALL
  USING (
    organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND EXISTS (
      SELECT 1 FROM memberships membership
      WHERE membership.organization_id = public.aro_private_context_uuid('aro.organization_id')
        AND membership.user_id = public.aro_private_context_uuid('aro.actor_id')
        AND membership.status = 'active'
        AND membership.deleted_at IS NULL
    )
  )
  WITH CHECK (
    organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND EXISTS (
      SELECT 1 FROM memberships membership
      WHERE membership.organization_id = public.aro_private_context_uuid('aro.organization_id')
        AND membership.user_id = public.aro_private_context_uuid('aro.actor_id')
        AND membership.status = 'active'
        AND membership.deleted_at IS NULL
    )
  );

CREATE POLICY tool_registry_aliases_context_present
  ON tool_registry_aliases AS PERMISSIVE FOR ALL
  USING (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  )
  WITH CHECK (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  );

CREATE POLICY tool_registry_aliases_tenant_boundary
  ON tool_registry_aliases AS RESTRICTIVE FOR ALL
  USING (
    organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND EXISTS (
      SELECT 1 FROM memberships membership
      WHERE membership.organization_id = public.aro_private_context_uuid('aro.organization_id')
        AND membership.user_id = public.aro_private_context_uuid('aro.actor_id')
        AND membership.status = 'active'
        AND membership.deleted_at IS NULL
    )
  )
  WITH CHECK (
    organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND EXISTS (
      SELECT 1 FROM memberships membership
      WHERE membership.organization_id = public.aro_private_context_uuid('aro.organization_id')
        AND membership.user_id = public.aro_private_context_uuid('aro.actor_id')
        AND membership.status = 'active'
        AND membership.deleted_at IS NULL
    )
  );

DO $aro_tool_registry_grants$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON tool_registry_entries TO aro_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON tool_registry_aliases TO aro_app;
  END IF;
END
$aro_tool_registry_grants$;
