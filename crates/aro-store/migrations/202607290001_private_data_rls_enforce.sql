-- Phase 2 of the private-data RLS rollout: enforce the tenant/owner
-- boundary policies installed (but deliberately not enabled) by
-- 202607020011_private_data_rls_policies.sql.
--
-- Preconditions, all implemented before this migration lands:
-- - every conversation/message/memory query runs inside a transaction that
--   sets aro.actor_id + aro.organization_id (AroStore::begin_tenant_tx), or
--   under an explicit per-unit-of-work worker context
--   (set_trusted_worker_tenant_context);
-- - cross-tenant maintenance (reconcile_ineligible_agent_run_jobs)
--   evaluates the RLS-gated conversations leg per job under that job
--   owner's session context instead of one context-free sweep.
--
-- Table owners stay exempt (no FORCE ROW LEVEL SECURITY): migrations and
-- owner tooling keep working. Runtime roles must stay non-owning
-- NOSUPERUSER NOBYPASSRLS (enforced at production startup by
-- ensure_restricted_database_role), otherwise RLS would be silently inert.
ALTER TABLE conversations ENABLE ROW LEVEL SECURITY;
ALTER TABLE messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE memories ENABLE ROW LEVEL SECURITY;
