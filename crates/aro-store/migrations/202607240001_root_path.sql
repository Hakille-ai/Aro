-- Migration: Root Path Support for Projects, Folders and Conversations
-- Target: PostgreSQL / ARO Cloud Store

ALTER TABLE projects ADD COLUMN IF NOT EXISTS root_path TEXT;
ALTER TABLE folders ADD COLUMN IF NOT EXISTS root_path TEXT;
ALTER TABLE conversations ADD COLUMN IF NOT EXISTS root_path TEXT;
