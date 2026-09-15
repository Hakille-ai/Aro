-- Phase 1 of the private-data RLS rollout.
--
-- This migration installs and validates the policy shape, but deliberately does not run
-- `ALTER TABLE ... ENABLE ROW LEVEL SECURITY`. The current release must first move every
-- conversation/message/memory query into a transaction that sets both context values with
-- `set_config(..., true)`. A later, explicit activation migration is the phase-2 gate. Keeping
-- activation out of this migration makes rolling deployments deterministic and prevents an old
-- API replica from suddenly seeing an empty database midway through a rollout.
--
-- Table owners remain exempt because FORCE ROW LEVEL SECURITY is intentionally not used. Runtime
-- roles are required to be non-owning NOSUPERUSER NOBYPASSRLS roles by deployment and startup
-- checks.

CREATE INDEX IF NOT EXISTS idx_conversations_rls_tenant_owner
  ON conversations (organization_id, owner_user_id, id);

CREATE INDEX IF NOT EXISTS idx_messages_rls_tenant_conversation
  ON messages (organization_id, conversation_id, created_at, id);

CREATE INDEX IF NOT EXISTS idx_memories_rls_tenant_owner
  ON memories (organization_id, owner_user_id, id);

-- Treat an absent, cleared, or malformed session setting as no context instead of raising an
-- invalid UUID exception from inside a policy. This helper is deliberately SECURITY INVOKER: it
-- can only inspect the calling session's own settings and receives no table privileges.
CREATE OR REPLACE FUNCTION public.aro_private_context_uuid(setting_name TEXT)
RETURNS UUID
LANGUAGE SQL
STABLE
PARALLEL SAFE
SECURITY INVOKER
SET search_path = pg_catalog
AS $aro_private_context_uuid$
  SELECT CASE
    WHEN pg_input_is_valid(NULLIF(current_setting(setting_name, true), ''), 'uuid')
      THEN NULLIF(current_setting(setting_name, true), '')::UUID
    ELSE NULL::UUID
  END
$aro_private_context_uuid$;

REVOKE ALL ON FUNCTION public.aro_private_context_uuid(TEXT) FROM PUBLIC;

DO $aro_private_context_uuid_grant$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    EXECUTE 'GRANT EXECUTE ON FUNCTION public.aro_private_context_uuid(TEXT) TO aro_app';
  END IF;
END
$aro_private_context_uuid_grant$;

-- PostgreSQL ORs permissive policies and then ANDs restrictive policies. Each table therefore has
-- one permissive policy requiring an explicit transaction-local context and one restrictive
-- boundary policy. A future permissive policy cannot widen access past the tenant/owner boundary.

DROP POLICY IF EXISTS conversations_context_present ON conversations;
CREATE POLICY conversations_context_present
  ON conversations
  AS PERMISSIVE
  FOR ALL
  USING (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  )
  WITH CHECK (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  );

DROP POLICY IF EXISTS conversations_tenant_owner_boundary ON conversations;
CREATE POLICY conversations_tenant_owner_boundary
  ON conversations
  AS RESTRICTIVE
  FOR ALL
  USING (
    organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND owner_user_id = public.aro_private_context_uuid('aro.actor_id')
    AND EXISTS (
      SELECT 1
      FROM memberships AS active_membership
      JOIN users AS active_user
        ON active_user.id = active_membership.user_id
       AND active_user.deleted_at IS NULL
      JOIN organizations AS active_organization
        ON active_organization.id = active_membership.organization_id
       AND active_organization.deleted_at IS NULL
      WHERE active_membership.user_id =
              public.aro_private_context_uuid('aro.actor_id')
        AND active_membership.organization_id =
              public.aro_private_context_uuid('aro.organization_id')
        AND active_membership.status = 'active'
        AND active_membership.deleted_at IS NULL
    )
  )
  WITH CHECK (
    organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND owner_user_id = public.aro_private_context_uuid('aro.actor_id')
    AND EXISTS (
      SELECT 1
      FROM memberships AS active_membership
      JOIN users AS active_user
        ON active_user.id = active_membership.user_id
       AND active_user.deleted_at IS NULL
      JOIN organizations AS active_organization
        ON active_organization.id = active_membership.organization_id
       AND active_organization.deleted_at IS NULL
      WHERE active_membership.user_id =
              public.aro_private_context_uuid('aro.actor_id')
        AND active_membership.organization_id =
              public.aro_private_context_uuid('aro.organization_id')
        AND active_membership.status = 'active'
        AND active_membership.deleted_at IS NULL
    )
  );

DROP POLICY IF EXISTS messages_context_present ON messages;
CREATE POLICY messages_context_present
  ON messages
  AS PERMISSIVE
  FOR ALL
  USING (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  )
  WITH CHECK (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  );

DROP POLICY IF EXISTS messages_tenant_owner_boundary ON messages;
CREATE POLICY messages_tenant_owner_boundary
  ON messages
  AS RESTRICTIVE
  FOR ALL
  USING (
    messages.organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND EXISTS (
      SELECT 1
      FROM conversations AS owned_conversation
      WHERE owned_conversation.organization_id = messages.organization_id
        AND owned_conversation.id = messages.conversation_id
        AND owned_conversation.owner_user_id =
          public.aro_private_context_uuid('aro.actor_id')
        AND owned_conversation.deleted_at IS NULL
    )
    AND EXISTS (
      SELECT 1
      FROM memberships AS active_membership
      JOIN users AS active_user
        ON active_user.id = active_membership.user_id
       AND active_user.deleted_at IS NULL
      JOIN organizations AS active_organization
        ON active_organization.id = active_membership.organization_id
       AND active_organization.deleted_at IS NULL
      WHERE active_membership.user_id =
              public.aro_private_context_uuid('aro.actor_id')
        AND active_membership.organization_id =
              public.aro_private_context_uuid('aro.organization_id')
        AND active_membership.status = 'active'
        AND active_membership.deleted_at IS NULL
    )
  )
  WITH CHECK (
    messages.organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND EXISTS (
      SELECT 1
      FROM conversations AS owned_conversation
      WHERE owned_conversation.organization_id = messages.organization_id
        AND owned_conversation.id = messages.conversation_id
        AND owned_conversation.owner_user_id =
          public.aro_private_context_uuid('aro.actor_id')
        AND owned_conversation.deleted_at IS NULL
    )
    AND EXISTS (
      SELECT 1
      FROM memberships AS active_membership
      JOIN users AS active_user
        ON active_user.id = active_membership.user_id
       AND active_user.deleted_at IS NULL
      JOIN organizations AS active_organization
        ON active_organization.id = active_membership.organization_id
       AND active_organization.deleted_at IS NULL
      WHERE active_membership.user_id =
              public.aro_private_context_uuid('aro.actor_id')
        AND active_membership.organization_id =
              public.aro_private_context_uuid('aro.organization_id')
        AND active_membership.status = 'active'
        AND active_membership.deleted_at IS NULL
    )
  );

DROP POLICY IF EXISTS memories_context_present ON memories;
CREATE POLICY memories_context_present
  ON memories
  AS PERMISSIVE
  FOR ALL
  USING (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  )
  WITH CHECK (
    public.aro_private_context_uuid('aro.organization_id') IS NOT NULL
    AND public.aro_private_context_uuid('aro.actor_id') IS NOT NULL
  );

DROP POLICY IF EXISTS memories_tenant_owner_boundary ON memories;
CREATE POLICY memories_tenant_owner_boundary
  ON memories
  AS RESTRICTIVE
  FOR ALL
  USING (
    organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND owner_user_id = public.aro_private_context_uuid('aro.actor_id')
    AND EXISTS (
      SELECT 1
      FROM memberships AS active_membership
      JOIN users AS active_user
        ON active_user.id = active_membership.user_id
       AND active_user.deleted_at IS NULL
      JOIN organizations AS active_organization
        ON active_organization.id = active_membership.organization_id
       AND active_organization.deleted_at IS NULL
      WHERE active_membership.user_id =
              public.aro_private_context_uuid('aro.actor_id')
        AND active_membership.organization_id =
              public.aro_private_context_uuid('aro.organization_id')
        AND active_membership.status = 'active'
        AND active_membership.deleted_at IS NULL
    )
  )
  WITH CHECK (
    organization_id = public.aro_private_context_uuid('aro.organization_id')
    AND owner_user_id = public.aro_private_context_uuid('aro.actor_id')
    AND EXISTS (
      SELECT 1
      FROM memberships AS active_membership
      JOIN users AS active_user
        ON active_user.id = active_membership.user_id
       AND active_user.deleted_at IS NULL
      JOIN organizations AS active_organization
        ON active_organization.id = active_membership.organization_id
       AND active_organization.deleted_at IS NULL
      WHERE active_membership.user_id =
              public.aro_private_context_uuid('aro.actor_id')
        AND active_membership.organization_id =
              public.aro_private_context_uuid('aro.organization_id')
        AND active_membership.status = 'active'
        AND active_membership.deleted_at IS NULL
    )
    AND (
      source_conversation_id IS NULL
      OR EXISTS (
        SELECT 1
        FROM conversations AS source_conversation
        WHERE source_conversation.organization_id = memories.organization_id
          AND source_conversation.id = memories.source_conversation_id
          AND source_conversation.owner_user_id =
                public.aro_private_context_uuid('aro.actor_id')
          AND source_conversation.deleted_at IS NULL
      )
    )
    AND cardinality(source_message_ids) = (
      SELECT COUNT(DISTINCT source_message.id)::integer
      FROM messages AS source_message
      JOIN conversations AS message_conversation
        ON message_conversation.organization_id = source_message.organization_id
       AND message_conversation.id = source_message.conversation_id
       AND message_conversation.owner_user_id =
             public.aro_private_context_uuid('aro.actor_id')
       AND message_conversation.deleted_at IS NULL
      WHERE source_message.organization_id = memories.organization_id
        AND source_message.id = ANY(memories.source_message_ids)
        AND source_message.deleted_at IS NULL
        AND (
          memories.source_conversation_id IS NULL
          OR source_message.conversation_id = memories.source_conversation_id
        )
    )
  );
