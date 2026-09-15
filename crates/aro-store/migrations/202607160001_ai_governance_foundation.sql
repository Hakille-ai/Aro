-- AI governance foundation, phase 1.
--
-- This is additive: legacy membership roles and agent_permission_profiles remain in place while
-- adapters feed the central evaluator. Policy versions are immutable after publication, bearer
-- capabilities are stored only as hashes, and every tenant table is protected by RLS from day one.

ALTER TABLE agent_permission_profiles
  ADD CONSTRAINT agent_permission_profiles_org_owner_id_unique
  UNIQUE (organization_id, owner_user_id, id);

CREATE TABLE authorization_roles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  description TEXT,
  system_key TEXT,
  created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  archived_at TIMESTAMPTZ,
  UNIQUE (organization_id, name),
  UNIQUE (organization_id, system_key),
  UNIQUE (organization_id, id)
);

CREATE TABLE authorization_permissions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  permission_key TEXT NOT NULL UNIQUE,
  resource_kind TEXT NOT NULL,
  action TEXT NOT NULL,
  description TEXT NOT NULL,
  default_risk TEXT NOT NULL DEFAULT 'moderate'
    CHECK (default_risk IN ('low', 'moderate', 'high', 'critical')),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (permission_key ~ '^[a-z0-9][a-z0-9._:-]{2,199}$')
);

CREATE TABLE authorization_role_permissions (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  role_id UUID NOT NULL,
  permission_id UUID NOT NULL REFERENCES authorization_permissions(id) ON DELETE RESTRICT,
  constraints JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (role_id, permission_id),
  FOREIGN KEY (organization_id, role_id)
    REFERENCES authorization_roles(organization_id, id) ON DELETE CASCADE,
  CHECK (jsonb_typeof(constraints) = 'object')
);

CREATE TABLE policy_sets (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  policy_key TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  scope TEXT NOT NULL DEFAULT 'organization'
    CHECK (scope IN ('global-security', 'organization', 'workspace', 'user', 'agent', 'component')),
  status TEXT NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft', 'active', 'disabled', 'archived')),
  current_version BIGINT NOT NULL DEFAULT 0 CHECK (current_version >= 0),
  created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  archived_at TIMESTAMPTZ,
  UNIQUE (organization_id, policy_key),
  UNIQUE (organization_id, id)
);

CREATE TABLE policy_versions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  policy_set_id UUID NOT NULL,
  version BIGINT NOT NULL CHECK (version > 0),
  status TEXT NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft', 'published', 'superseded', 'withdrawn')),
  source JSONB NOT NULL,
  source_hash TEXT NOT NULL,
  change_summary TEXT,
  created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  published_at TIMESTAMPTZ,
  valid_from TIMESTAMPTZ,
  valid_until TIMESTAMPTZ,
  UNIQUE (policy_set_id, version),
  UNIQUE (organization_id, id),
  FOREIGN KEY (organization_id, policy_set_id)
    REFERENCES policy_sets(organization_id, id) ON DELETE CASCADE,
  CHECK (jsonb_typeof(source) = 'object'),
  CHECK (valid_until IS NULL OR valid_from IS NULL OR valid_until > valid_from),
  CHECK ((status = 'published') = (published_at IS NOT NULL) OR status IN ('superseded', 'withdrawn'))
);

CREATE TABLE policy_rules (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  policy_version_id UUID NOT NULL,
  rule_key TEXT NOT NULL,
  layer TEXT NOT NULL CHECK (layer IN (
    'legacy-adapter', 'capability', 'temporary-grant', 'consent', 'role', 'user',
    'organization', 'global-security'
  )),
  priority INTEGER NOT NULL DEFAULT 0,
  effect TEXT NOT NULL CHECK (effect IN (
    'allow', 'deny', 'require_confirmation', 'require_step_up_authentication',
    'require_admin_approval'
  )),
  definition JSONB NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT true,
  valid_from TIMESTAMPTZ,
  valid_until TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  FOREIGN KEY (organization_id, policy_version_id)
    REFERENCES policy_versions(organization_id, id) ON DELETE CASCADE,
  UNIQUE (policy_version_id, rule_key),
  CHECK (jsonb_typeof(definition) = 'object'),
  CHECK (valid_until IS NULL OR valid_from IS NULL OR valid_until > valid_from)
);

CREATE TABLE permission_assignments (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  subject_kind TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  role_id UUID,
  policy_set_id UUID,
  permission_id UUID REFERENCES authorization_permissions(id) ON DELETE RESTRICT,
  constraints JSONB NOT NULL DEFAULT '{}'::jsonb,
  granted_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  valid_from TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (num_nonnulls(role_id, policy_set_id, permission_id) = 1),
  FOREIGN KEY (organization_id, role_id)
    REFERENCES authorization_roles(organization_id, id) ON DELETE CASCADE,
  FOREIGN KEY (organization_id, policy_set_id)
    REFERENCES policy_sets(organization_id, id) ON DELETE CASCADE,
  CHECK (expires_at IS NULL OR expires_at > valid_from),
  CHECK (jsonb_typeof(constraints) = 'object')
);

CREATE TABLE governed_resources (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  resource_kind TEXT NOT NULL,
  resource_key TEXT NOT NULL,
  owner_kind TEXT,
  owner_id TEXT,
  workspace_id UUID,
  environment TEXT,
  attributes JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  UNIQUE (organization_id, resource_kind, resource_key),
  UNIQUE (organization_id, id),
  CHECK (jsonb_typeof(attributes) = 'object')
);

CREATE TABLE resource_classifications (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  resource_id UUID NOT NULL,
  classification TEXT NOT NULL CHECK (classification IN (
    'public', 'internal', 'personal', 'confidential', 'sensitive',
    'highly-sensitive', 'secret', 'regulated'
  )),
  field_classifications JSONB NOT NULL DEFAULT '{}'::jsonb,
  classified_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  policy_version_id UUID,
  valid_from TIMESTAMPTZ NOT NULL DEFAULT now(),
  valid_until TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  FOREIGN KEY (organization_id, resource_id)
    REFERENCES governed_resources(organization_id, id) ON DELETE CASCADE,
  FOREIGN KEY (organization_id, policy_version_id)
    REFERENCES policy_versions(organization_id, id) ON DELETE SET NULL,
  CHECK (jsonb_typeof(field_classifications) = 'object'),
  CHECK (valid_until IS NULL OR valid_until > valid_from)
);

CREATE TABLE agent_permission_bindings (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  agent_id TEXT NOT NULL,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  permission_profile_id UUID,
  policy_set_id UUID,
  autonomy_level TEXT NOT NULL DEFAULT 'supervised',
  allowed_components JSONB NOT NULL DEFAULT '{}'::jsonb,
  limits JSONB NOT NULL DEFAULT '{}'::jsonb,
  delegable_permissions JSONB NOT NULL DEFAULT '[]'::jsonb,
  non_delegable_permissions JSONB NOT NULL DEFAULT '[]'::jsonb,
  expires_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, agent_id),
  FOREIGN KEY (organization_id, owner_user_id, permission_profile_id)
    REFERENCES agent_permission_profiles(organization_id, owner_user_id, id) ON DELETE RESTRICT,
  FOREIGN KEY (organization_id, policy_set_id)
    REFERENCES policy_sets(organization_id, id) ON DELETE SET NULL,
  CHECK (jsonb_typeof(allowed_components) = 'object'),
  CHECK (jsonb_typeof(limits) = 'object'),
  CHECK (jsonb_typeof(delegable_permissions) = 'array'),
  CHECK (jsonb_typeof(non_delegable_permissions) = 'array')
);

CREATE TABLE component_permission_manifests (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  component_kind TEXT NOT NULL CHECK (component_kind IN ('tool', 'plugin', 'skill', 'mcp-server')),
  component_id TEXT NOT NULL,
  component_version TEXT NOT NULL,
  owner_id TEXT,
  trust_level TEXT NOT NULL DEFAULT 'untrusted'
    CHECK (trust_level IN ('untrusted', 'restricted', 'trusted', 'platform')),
  manifest JSONB NOT NULL,
  manifest_hash TEXT NOT NULL,
  signature TEXT,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'active', 'disabled', 'revoked')),
  valid_from TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, component_kind, component_id, component_version),
  CHECK (jsonb_typeof(manifest) = 'object')
);

CREATE TABLE user_consents (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  agent_id TEXT,
  purpose TEXT NOT NULL,
  action TEXT NOT NULL,
  resource_selector JSONB NOT NULL,
  service_id TEXT,
  data_shared BOOLEAN NOT NULL DEFAULT false,
  reversible BOOLEAN NOT NULL DEFAULT true,
  duration_kind TEXT NOT NULL CHECK (duration_kind IN (
    'once', 'session', 'conversation', 'agent', 'organization', 'fixed-duration', 'until-revoked'
  )),
  scope_id TEXT,
  valid_from TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  revocation_reason TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (jsonb_typeof(resource_selector) = 'object'),
  CHECK (expires_at IS NULL OR expires_at > valid_from)
);

CREATE TABLE approval_requests (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  requester_subject_kind TEXT NOT NULL,
  requester_subject_id TEXT NOT NULL,
  actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  agent_id TEXT,
  action TEXT NOT NULL,
  resource_kind TEXT NOT NULL,
  resource_id TEXT NOT NULL,
  amount_minor BIGINT,
  binding_hash TEXT NOT NULL UNIQUE,
  requirement TEXT NOT NULL CHECK (requirement IN (
    'confirmation', 'strong-confirmation', 'step-up-authentication', 'admin-approval'
  )),
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'approved', 'denied', 'expired', 'consumed', 'revoked')),
  requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL,
  decided_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  decided_at TIMESTAMPTZ,
  consumed_at TIMESTAMPTZ,
  decision_reason TEXT,
  CHECK (expires_at > requested_at)
);

CREATE TABLE temporary_grants (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  subject_kind TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  grant_constraints JSONB NOT NULL,
  max_uses BIGINT CHECK (max_uses IS NULL OR max_uses > 0),
  used_count BIGINT NOT NULL DEFAULT 0 CHECK (used_count >= 0),
  valid_from TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL,
  granted_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (jsonb_typeof(grant_constraints) = 'object'),
  CHECK (expires_at > valid_from),
  CHECK (max_uses IS NULL OR used_count <= max_uses)
);

CREATE TABLE authorization_capabilities (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  subject_kind TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  nonce TEXT NOT NULL UNIQUE,
  action TEXT NOT NULL,
  resource_kind TEXT NOT NULL,
  resource_id TEXT NOT NULL,
  agent_id TEXT,
  run_id UUID,
  task_id TEXT,
  environment TEXT,
  constraints JSONB NOT NULL DEFAULT '{}'::jsonb,
  max_uses BIGINT NOT NULL DEFAULT 1 CHECK (max_uses > 0),
  used_count BIGINT NOT NULL DEFAULT 0 CHECK (used_count >= 0 AND used_count <= max_uses),
  issued_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  issued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL,
  revoked_at TIMESTAMPTZ,
  CHECK (jsonb_typeof(constraints) = 'object'),
  CHECK (expires_at > issued_at)
);

CREATE TABLE permission_delegations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  parent_delegation_id UUID,
  parent_subject_id TEXT NOT NULL,
  child_subject_id TEXT NOT NULL,
  depth INTEGER NOT NULL CHECK (depth >= 1),
  max_depth INTEGER NOT NULL CHECK (max_depth >= depth),
  objective TEXT NOT NULL,
  grant_constraints JSONB NOT NULL,
  max_calls BIGINT CHECK (max_calls IS NULL OR max_calls > 0),
  max_budget_minor BIGINT CHECK (max_budget_minor IS NULL OR max_budget_minor >= 0),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ NOT NULL,
  revoked_at TIMESTAMPTZ,
  UNIQUE (organization_id, id),
  FOREIGN KEY (organization_id, parent_delegation_id)
    REFERENCES permission_delegations(organization_id, id) ON DELETE CASCADE,
  CHECK (jsonb_typeof(grant_constraints) = 'object'),
  CHECK (parent_subject_id <> child_subject_id),
  CHECK (expires_at > created_at)
);

CREATE TABLE permission_revocations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  target_kind TEXT NOT NULL,
  target_id TEXT NOT NULL,
  cascade BOOLEAN NOT NULL DEFAULT true,
  reason TEXT NOT NULL,
  revoked_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  revoked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  effective_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  expires_at TIMESTAMPTZ,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  CHECK (jsonb_typeof(metadata) = 'object'),
  CHECK (expires_at IS NULL OR expires_at > effective_at)
);

CREATE TABLE organization_restrictions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  restriction_key TEXT NOT NULL,
  definition JSONB NOT NULL,
  priority INTEGER NOT NULL DEFAULT 1000,
  enabled BOOLEAN NOT NULL DEFAULT true,
  valid_from TIMESTAMPTZ NOT NULL DEFAULT now(),
  valid_until TIMESTAMPTZ,
  created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, restriction_key),
  CHECK (jsonb_typeof(definition) = 'object'),
  CHECK (valid_until IS NULL OR valid_until > valid_from)
);

CREATE TABLE data_handling_policies (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  classification TEXT NOT NULL CHECK (classification IN (
    'public', 'internal', 'personal', 'confidential', 'sensitive',
    'highly-sensitive', 'secret', 'regulated'
  )),
  allowed_model_ids TEXT[] NOT NULL DEFAULT '{}',
  allowed_provider_ids TEXT[] NOT NULL DEFAULT '{}',
  allowed_environments TEXT[] NOT NULL DEFAULT '{}',
  external_transfer_allowed BOOLEAN NOT NULL DEFAULT false,
  export_allowed BOOLEAN NOT NULL DEFAULT false,
  retention_seconds BIGINT CHECK (retention_seconds IS NULL OR retention_seconds >= 0),
  encryption_required BOOLEAN NOT NULL DEFAULT true,
  confirmation_required BOOLEAN NOT NULL DEFAULT false,
  redaction_rules JSONB NOT NULL DEFAULT '[]'::jsonb,
  policy_version_id UUID,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, classification),
  FOREIGN KEY (organization_id, policy_version_id)
    REFERENCES policy_versions(organization_id, id) ON DELETE SET NULL,
  CHECK (jsonb_typeof(redaction_rules) = 'array')
);

CREATE TABLE permission_evaluations (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  subject_kind TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  agent_id TEXT,
  run_id UUID,
  task_id TEXT,
  tool_id TEXT,
  action TEXT NOT NULL,
  resource_kind TEXT NOT NULL,
  resource_id TEXT NOT NULL,
  environment TEXT NOT NULL,
  risk_level TEXT NOT NULL,
  data_classification TEXT NOT NULL,
  decision TEXT NOT NULL CHECK (decision IN (
    'allow', 'deny', 'allow_with_constraints', 'require_confirmation',
    'require_step_up_authentication', 'require_admin_approval'
  )),
  reason TEXT NOT NULL,
  error_code TEXT,
  applied_policies TEXT[] NOT NULL DEFAULT '{}',
  constraints JSONB NOT NULL DEFAULT '{}'::jsonb,
  request_snapshot JSONB NOT NULL,
  decision_snapshot JSONB NOT NULL,
  policy_cache_version BIGINT,
  engine_version TEXT NOT NULL,
  evaluated_at TIMESTAMPTZ NOT NULL,
  duration_micros BIGINT CHECK (duration_micros IS NULL OR duration_micros >= 0),
  execution_result TEXT,
  revocation_checked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (jsonb_typeof(constraints) = 'object'),
  CHECK (jsonb_typeof(request_snapshot) = 'object'),
  CHECK (jsonb_typeof(decision_snapshot) = 'object')
);

CREATE TABLE security_audit_events (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
  subject_kind TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  action TEXT,
  resource_kind TEXT,
  resource_id TEXT,
  evaluation_id UUID REFERENCES permission_evaluations(id) ON DELETE SET NULL,
  outcome TEXT NOT NULL,
  tenant_sequence BIGINT NOT NULL CHECK (tenant_sequence > 0),
  payload JSONB NOT NULL DEFAULT '{}'::jsonb,
  previous_hash TEXT,
  event_hash TEXT NOT NULL,
  occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  retention_until TIMESTAMPTZ,
  UNIQUE (organization_id, tenant_sequence),
  CHECK (jsonb_typeof(payload) = 'object')
);

CREATE INDEX idx_policy_versions_active
  ON policy_versions (organization_id, policy_set_id, version DESC)
  WHERE status = 'published';
CREATE INDEX idx_policy_rules_evaluation
  ON policy_rules (organization_id, enabled, layer, priority DESC)
  WHERE enabled;
CREATE INDEX idx_permission_assignments_subject
  ON permission_assignments (organization_id, subject_kind, subject_id, expires_at)
  WHERE revoked_at IS NULL;
CREATE INDEX idx_governed_resources_lookup
  ON governed_resources (organization_id, resource_kind, resource_key)
  WHERE deleted_at IS NULL;
CREATE INDEX idx_resource_classifications_current
  ON resource_classifications (organization_id, resource_id, valid_from DESC)
  WHERE valid_until IS NULL;
CREATE INDEX idx_agent_permissions_active
  ON agent_permission_bindings (organization_id, agent_id)
  WHERE revoked_at IS NULL;
CREATE INDEX idx_component_manifests_active
  ON component_permission_manifests (organization_id, component_kind, component_id, status);
CREATE INDEX idx_consents_effective
  ON user_consents (organization_id, user_id, action, expires_at)
  WHERE revoked_at IS NULL;
CREATE INDEX idx_approvals_binding
  ON approval_requests (organization_id, binding_hash, status, expires_at);
CREATE INDEX idx_temporary_grants_effective
  ON temporary_grants (organization_id, subject_kind, subject_id, expires_at)
  WHERE revoked_at IS NULL;
CREATE INDEX idx_capabilities_effective
  ON authorization_capabilities (organization_id, subject_id, action, resource_id, expires_at)
  WHERE revoked_at IS NULL;
CREATE INDEX idx_delegations_parent
  ON permission_delegations (organization_id, parent_delegation_id, expires_at)
  WHERE revoked_at IS NULL;
CREATE INDEX idx_revocations_target
  ON permission_revocations (organization_id, target_kind, target_id, effective_at DESC);
CREATE INDEX idx_permission_evaluations_subject_time
  ON permission_evaluations (organization_id, subject_id, evaluated_at DESC);
CREATE INDEX idx_permission_evaluations_resource_time
  ON permission_evaluations (organization_id, resource_kind, resource_id, evaluated_at DESC);
CREATE INDEX idx_permission_evaluations_denials
  ON permission_evaluations (organization_id, evaluated_at DESC)
  WHERE decision = 'deny';
CREATE INDEX idx_security_audit_events_time
  ON security_audit_events (organization_id, tenant_sequence DESC);

-- Published policy source is append-only. Changes require a new version.
CREATE OR REPLACE FUNCTION public.aro_protect_published_policy_version()
RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY INVOKER
SET search_path = pg_catalog, public
AS $aro_protect_published_policy_version$
BEGIN
  IF OLD.status IN ('published', 'superseded', 'withdrawn') THEN
    RAISE EXCEPTION 'published policy versions are immutable';
  END IF;
  RETURN NEW;
END
$aro_protect_published_policy_version$;

CREATE TRIGGER protect_published_policy_version
BEFORE UPDATE OR DELETE ON policy_versions
FOR EACH ROW EXECUTE FUNCTION public.aro_protect_published_policy_version();

CREATE OR REPLACE FUNCTION public.aro_protect_published_policy_rule()
RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY INVOKER
SET search_path = pg_catalog, public
AS $aro_protect_published_policy_rule$
DECLARE
  version_status TEXT;
  protected_version_id UUID;
BEGIN
  protected_version_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.policy_version_id ELSE NEW.policy_version_id END;
  SELECT status INTO version_status FROM policy_versions WHERE id = protected_version_id;
  IF version_status IS DISTINCT FROM 'draft' THEN
    RAISE EXCEPTION 'rules of non-draft policy versions are immutable';
  END IF;
  RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END
$aro_protect_published_policy_rule$;

CREATE TRIGGER protect_published_policy_rule
BEFORE INSERT OR UPDATE OR DELETE ON policy_rules
FOR EACH ROW EXECUTE FUNCTION public.aro_protect_published_policy_rule();

-- Security audit rows are append-only for the application role. Retention/archival must use a
-- separate privileged maintenance identity and a recorded procedure.
CREATE OR REPLACE FUNCTION public.aro_reject_security_audit_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY INVOKER
SET search_path = pg_catalog
AS $aro_reject_security_audit_mutation$
BEGIN
  RAISE EXCEPTION 'security audit events are append-only';
END
$aro_reject_security_audit_mutation$;

CREATE TRIGGER reject_security_audit_mutation
BEFORE UPDATE OR DELETE ON security_audit_events
FOR EACH ROW EXECUTE FUNCTION public.aro_reject_security_audit_mutation();

-- All tenant governance tables use the trusted transaction-local identity established by
-- AroStore::begin_tenant_tx. No caller-controlled header participates in these policies.
DO $aro_governance_rls$
DECLARE
  table_name TEXT;
BEGIN
  FOREACH table_name IN ARRAY ARRAY[
    'authorization_roles', 'authorization_role_permissions', 'policy_sets', 'policy_versions', 'policy_rules',
    'permission_assignments', 'governed_resources', 'resource_classifications',
    'agent_permission_bindings', 'component_permission_manifests', 'user_consents',
    'approval_requests', 'temporary_grants', 'authorization_capabilities',
    'permission_delegations', 'permission_revocations', 'organization_restrictions',
    'data_handling_policies', 'permission_evaluations', 'security_audit_events'
  ]
  LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', table_name);
    EXECUTE format(
      'CREATE POLICY %I ON %I AS PERMISSIVE FOR ALL USING (
         public.aro_private_context_uuid(''aro.organization_id'') IS NOT NULL
         AND public.aro_private_context_uuid(''aro.actor_id'') IS NOT NULL
       ) WITH CHECK (
         public.aro_private_context_uuid(''aro.organization_id'') IS NOT NULL
         AND public.aro_private_context_uuid(''aro.actor_id'') IS NOT NULL
       )',
      table_name || '_context_present', table_name
    );
    EXECUTE format(
      'CREATE POLICY %I ON %I AS RESTRICTIVE FOR ALL USING (
         organization_id = public.aro_private_context_uuid(''aro.organization_id'')
         AND EXISTS (
           SELECT 1 FROM memberships m
           WHERE m.organization_id = public.aro_private_context_uuid(''aro.organization_id'')
             AND m.user_id = public.aro_private_context_uuid(''aro.actor_id'')
             AND m.status = ''active'' AND m.deleted_at IS NULL
         )
       ) WITH CHECK (
         organization_id = public.aro_private_context_uuid(''aro.organization_id'')
         AND EXISTS (
           SELECT 1 FROM memberships m
           WHERE m.organization_id = public.aro_private_context_uuid(''aro.organization_id'')
             AND m.user_id = public.aro_private_context_uuid(''aro.actor_id'')
             AND m.status = ''active'' AND m.deleted_at IS NULL
         )
       )',
      table_name || '_tenant_boundary', table_name
    );
  END LOOP;
END
$aro_governance_rls$;

DO $aro_governance_runtime_grants$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    EXECUTE 'GRANT SELECT, INSERT, UPDATE, DELETE ON
      authorization_roles, authorization_role_permissions, policy_sets, policy_versions,
      policy_rules, permission_assignments, governed_resources, resource_classifications,
      agent_permission_bindings, component_permission_manifests, user_consents,
      approval_requests, temporary_grants, authorization_capabilities,
      permission_delegations, permission_revocations, organization_restrictions,
      data_handling_policies, permission_evaluations, security_audit_events
    TO aro_app';
    EXECUTE 'GRANT SELECT ON authorization_permissions TO aro_app';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker') THEN
    EXECUTE 'GRANT SELECT ON
      authorization_permissions, authorization_roles, authorization_role_permissions,
      policy_sets, policy_versions, policy_rules, permission_assignments, governed_resources,
      resource_classifications, agent_permission_bindings, component_permission_manifests,
      user_consents, approval_requests, temporary_grants, authorization_capabilities,
      permission_delegations, permission_revocations, organization_restrictions,
      data_handling_policies
    TO aro_worker';
    EXECUTE 'GRANT SELECT, INSERT ON permission_evaluations, security_audit_events TO aro_worker';
  END IF;
END
$aro_governance_runtime_grants$;
