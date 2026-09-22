-- Operator-only audit trail. No public HTTP route grants contracts or adjustments.
CREATE TABLE billing_operator_actions (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id),
  reference TEXT NOT NULL,
  action TEXT NOT NULL,
  details JSONB NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, reference)
);
ALTER TABLE billing_operator_actions ENABLE ROW LEVEL SECURITY;
CREATE POLICY billing_operator_tenant ON billing_operator_actions
  USING (organization_id = public.aro_private_context_uuid('aro.organization_id'))
  WITH CHECK (organization_id = public.aro_private_context_uuid('aro.organization_id'));
-- Deliberately no aro_app grant: run the offline operator command with a dedicated
-- privileged database role, never with a customer key or an application account.
