-- Commercial subscriptions and prepaid inference are separate ledgers.
CREATE TABLE billing_accounts (
  organization_id UUID PRIMARY KEY REFERENCES organizations(id),
  plan TEXT NOT NULL DEFAULT 'community' CHECK (plan IN ('community','cloud','business','enterprise')),
  status TEXT NOT NULL DEFAULT 'inactive',
  seats INTEGER NOT NULL DEFAULT 1 CHECK (seats BETWEEN 1 AND 100000),
  valid_until TIMESTAMPTZ,
  cancel_at_period_end BOOLEAN NOT NULL DEFAULT FALSE,
  stripe_customer_id TEXT UNIQUE,
  stripe_subscription_id TEXT UNIQUE,
  balance_micros BIGINT NOT NULL DEFAULT 0 CHECK (balance_micros BETWEEN -1000000000000 AND 1000000000000),
  reserved_micros BIGINT NOT NULL DEFAULT 0 CHECK (reserved_micros >= 0),
  monthly_limit_micros BIGINT NOT NULL DEFAULT 0 CHECK (monthly_limit_micros BETWEEN 0 AND 1000000000000),
  per_request_limit_micros BIGINT NOT NULL DEFAULT 0 CHECK (per_request_limit_micros BETWEEN 0 AND 1000000000000),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE billing_checkout_intents (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id),
  actor_id UUID NOT NULL REFERENCES users(id),
  kind TEXT NOT NULL CHECK (kind IN ('cloud','business','credit')),
  quantity INTEGER NOT NULL CHECK (quantity BETWEEN 1 AND 100000),
  amount_cents BIGINT NOT NULL DEFAULT 0,
  stripe_session_id TEXT UNIQUE,
  stripe_payment_intent_id TEXT UNIQUE,
  credited_micros BIGINT NOT NULL DEFAULT 0,
  reversed_micros BIGINT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK (reversed_micros >= 0 AND reversed_micros <= credited_micros)
);
CREATE TABLE billing_ledger (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id),
  reference TEXT NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('credit','charge','refund','adjustment')),
  amount_micros BIGINT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, reference)
);
CREATE INDEX billing_ledger_month ON billing_ledger (organization_id, created_at);
CREATE TABLE billing_reservations (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id),
  actor_id UUID NOT NULL REFERENCES users(id),
  request_key UUID NOT NULL,
  request_hash TEXT NOT NULL,
  reserved_micros BIGINT NOT NULL CHECK (reserved_micros > 0),
  charged_micros BIGINT CHECK (charged_micros >= 0 AND charged_micros <= reserved_micros),
  status TEXT NOT NULL CHECK (status IN ('reserved','completed','released','uncertain')),
  result JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (organization_id, actor_id, request_key)
);
CREATE TABLE billing_compute_keys (
  id UUID PRIMARY KEY,
  organization_id UUID NOT NULL REFERENCES organizations(id),
  actor_id UUID NOT NULL REFERENCES users(id),
  name TEXT NOT NULL,
  key_hash TEXT NOT NULL UNIQUE,
  prefix TEXT NOT NULL,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- No application endpoint exposes checkout intents or key hashes. They are looked up by
-- authenticated provider IDs / hashed bearer credentials before a tenant is established.
-- Financial state and generated outputs enforce tenant scope at database level as well.
DO $$ DECLARE t TEXT; BEGIN
  FOREACH t IN ARRAY ARRAY['billing_accounts','billing_ledger','billing_reservations'] LOOP
    EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
    EXECUTE format('CREATE POLICY billing_tenant ON %I USING (organization_id = public.aro_private_context_uuid(''aro.organization_id'')) WITH CHECK (organization_id = public.aro_private_context_uuid(''aro.organization_id''))', t);
  END LOOP;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    GRANT SELECT, INSERT, UPDATE ON billing_accounts, billing_checkout_intents, billing_reservations, billing_compute_keys TO aro_app;
    GRANT SELECT, INSERT ON billing_ledger TO aro_app;
  END IF;
END $$;
