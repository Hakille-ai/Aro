-- Migration: Projects and Folders for Conversations
-- Target: PostgreSQL / ARO Cloud Store

CREATE TABLE IF NOT EXISTS projects (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  description TEXT,
  color TEXT NOT NULL DEFAULT '#3b82f6',
  icon TEXT NOT NULL DEFAULT 'folder-tree',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS folders (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  project_id UUID REFERENCES projects(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  color TEXT,
  icon TEXT NOT NULL DEFAULT 'folder',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE conversations
  ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES projects(id) ON DELETE SET NULL,
  ADD COLUMN IF NOT EXISTS folder_id UUID REFERENCES folders(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_projects_org_user ON projects(organization_id, owner_user_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_folders_org_user ON folders(organization_id, owner_user_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversations_project ON conversations(project_id);
CREATE INDEX IF NOT EXISTS idx_conversations_folder ON conversations(folder_id);
