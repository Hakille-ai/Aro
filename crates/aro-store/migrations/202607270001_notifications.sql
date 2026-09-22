-- Migration: Notifications System for ARO Cloud
-- Target: PostgreSQL / ARO Cloud Store

CREATE TABLE IF NOT EXISTS notifications (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  user_id UUID REFERENCES users(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'system',
  priority TEXT NOT NULL DEFAULT 'normal',
  status TEXT NOT NULL DEFAULT 'unread',
  source TEXT NOT NULL DEFAULT 'system',
  action_url TEXT,
  metadata JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  read_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_notifications_org_user_status
  ON notifications(organization_id, user_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_notifications_org_status
  ON notifications(organization_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_notifications_created
  ON notifications(created_at DESC);
