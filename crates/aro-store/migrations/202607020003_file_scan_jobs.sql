CREATE TABLE file_scan_jobs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  file_id UUID NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued',
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  lease_token UUID,
  lease_until TIMESTAMPTZ,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at TIMESTAMPTZ,
  deleted_at TIMESTAMPTZ,
  CONSTRAINT file_scan_jobs_file_fk
    FOREIGN KEY (organization_id, file_id)
    REFERENCES file_objects(organization_id, id)
    ON DELETE CASCADE,
  CONSTRAINT file_scan_jobs_org_file_unique UNIQUE (organization_id, file_id)
);

CREATE INDEX idx_file_scan_jobs_claimable
  ON file_scan_jobs(status, lease_until, created_at)
  WHERE deleted_at IS NULL;

CREATE TRIGGER trg_file_scan_jobs_updated_at
  BEFORE UPDATE ON file_scan_jobs
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();
