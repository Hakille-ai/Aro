use std::path::{Path, PathBuf};
use std::time::Duration;

use aro_core::{
    AgentArtifact, AgentContextItem, AgentLane, AgentLaneStatus, AgentLaneView, AgentRun,
    AgentRunPriority, AgentRunStatus, AgentStep, AgentStepKind, AgentStepStatus, AroError,
    AroResult, AssistantMode, ChatMessage, ContextSource, Conversation, Episode, Folder,
    LongTermMemory, MemoryEntry, MessageRole, NotificationFilter, NotificationItem,
    NotificationKind, NotificationPriority, NotificationSource, NotificationStatus,
    PermissionProfile, Plan, Project, MEMORY_STATUS_APPROVED,
};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SqliteMemoryStore {
    db_path: PathBuf,
}

impl SqliteMemoryStore {
    pub fn new(db_path: impl AsRef<Path>) -> AroResult<Self> {
        let store = Self {
            db_path: db_path.as_ref().to_path_buf(),
        };
        store.migrate()?;
        Ok(store)
    }

    fn connect(&self) -> AroResult<Connection> {
        let conn =
            Connection::open(&self.db_path).map_err(|err| AroError::Memory(err.to_string()))?;
        conn.busy_timeout(Duration::from_millis(5000))
            .map_err(|err| AroError::Memory(err.to_string()))?;
        conn.execute_batch(
            r#"
            PRAGMA busy_timeout = 5000;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            "#,
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(conn)
    }

    pub fn migrate(&self) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS projects (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              description TEXT,
              instructions TEXT,
              root_path TEXT,
              color TEXT NOT NULL DEFAULT '#3b82f6',
              icon TEXT NOT NULL DEFAULT 'folder-tree',
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              organization_id TEXT
            );

            CREATE TABLE IF NOT EXISTS folders (
              id TEXT PRIMARY KEY,
              project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
              name TEXT NOT NULL,
              root_path TEXT,
              color TEXT,
              icon TEXT NOT NULL DEFAULT 'folder',
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              organization_id TEXT
            );

            CREATE TABLE IF NOT EXISTS conversations (
              id TEXT PRIMARY KEY,
              title TEXT NOT NULL,
              mode TEXT NOT NULL,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
              folder_id TEXT REFERENCES folders(id) ON DELETE SET NULL,
              root_path TEXT
            );

            CREATE TABLE IF NOT EXISTS messages (
              id TEXT PRIMARY KEY,
              conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
              role TEXT NOT NULL,
              content TEXT NOT NULL,
              created_at TEXT NOT NULL,
              token_estimate INTEGER,
              model_id TEXT
            );

            CREATE TABLE IF NOT EXISTS memories (
              id TEXT PRIMARY KEY,
              client_id TEXT,
              content TEXT NOT NULL,
              category TEXT NOT NULL DEFAULT 'personal',
              scope TEXT NOT NULL DEFAULT 'user',
              status TEXT NOT NULL DEFAULT 'approved',
              source_conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
              source_message_ids TEXT NOT NULL DEFAULT '[]',
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              pinned INTEGER NOT NULL DEFAULT 0,
              salience REAL NOT NULL DEFAULT 0.5,
              recall_count INTEGER NOT NULL DEFAULT 0,
              last_used_at TEXT,
              deleted_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_messages_conversation_created
              ON messages(conversation_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_conversations_updated
              ON conversations(updated_at DESC);
            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts
              USING fts5(id UNINDEXED, content, category, scope);

            CREATE TABLE IF NOT EXISTS agent_runs (
              id TEXT PRIMARY KEY,
              lane_id TEXT,
              conversation_id TEXT,
              goal TEXT NOT NULL,
              mode TEXT NOT NULL,
              status TEXT NOT NULL,
              priority TEXT NOT NULL DEFAULT '"normal"',
              model_provider_id TEXT,
              model_id TEXT,
              autonomy_profile_id TEXT,
              checkpoint_summary TEXT,
              last_error TEXT,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              heartbeat_at TEXT,
              completed_at TEXT,
              sync_status TEXT NOT NULL DEFAULT 'local'
            );

            CREATE TABLE IF NOT EXISTS agent_lanes (
              id TEXT PRIMARY KEY,
              conversation_id TEXT,
              title TEXT NOT NULL,
              status TEXT NOT NULL,
              priority TEXT NOT NULL,
              max_concurrent_runs INTEGER NOT NULL DEFAULT 1,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              sync_status TEXT NOT NULL DEFAULT 'local'
            );

            CREATE TABLE IF NOT EXISTS agent_steps (
              id TEXT PRIMARY KEY,
              run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
              sequence INTEGER NOT NULL,
              kind TEXT NOT NULL,
              status TEXT NOT NULL,
              title TEXT NOT NULL,
              input_json TEXT NOT NULL,
              output_json TEXT NOT NULL,
              error TEXT,
              started_at TEXT NOT NULL,
              finished_at TEXT
            );

            CREATE TABLE IF NOT EXISTS agent_artifacts (
              id TEXT PRIMARY KEY,
              run_id TEXT NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
              kind TEXT NOT NULL,
              title TEXT NOT NULL,
              uri TEXT,
              content TEXT,
              metadata_json TEXT NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_context_items (
              id TEXT PRIMARY KEY,
              run_id TEXT,
              conversation_id TEXT,
              kind TEXT NOT NULL,
              title TEXT NOT NULL,
              content TEXT NOT NULL,
              uri TEXT,
              metadata_json TEXT NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS agent_context_fts
              USING fts5(id UNINDEXED, title, content);

            CREATE TABLE IF NOT EXISTS agent_permission_profiles (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              profile_json TEXT NOT NULL,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS plans (
              id TEXT PRIMARY KEY,
              conversation_id TEXT NOT NULL,
              title TEXT NOT NULL,
              description TEXT,
              tasks TEXT NOT NULL DEFAULT '[]',
              status TEXT NOT NULL DEFAULT 'active',
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              deleted_at TEXT
            );

            CREATE TABLE IF NOT EXISTS notifications (
              id TEXT PRIMARY KEY,
              organization_id TEXT,
              user_id TEXT,
              title TEXT NOT NULL,
              body TEXT NOT NULL,
              kind TEXT NOT NULL DEFAULT 'info',
              priority TEXT NOT NULL DEFAULT 'normal',
              status TEXT NOT NULL DEFAULT 'unread',
              source TEXT NOT NULL DEFAULT 'system',
              action_url TEXT,
              metadata TEXT,
              created_at TEXT NOT NULL,
              read_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_notifications_status
              ON notifications(status);

            CREATE INDEX IF NOT EXISTS idx_notifications_created
              ON notifications(created_at DESC);

            CREATE INDEX IF NOT EXISTS idx_notifications_org
              ON notifications(organization_id, created_at DESC);

            CREATE TABLE IF NOT EXISTS episodes (
              id TEXT PRIMARY KEY,
              conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,

              turn_start INTEGER NOT NULL,
              turn_end INTEGER NOT NULL,
              summary TEXT NOT NULL,
              key_decisions TEXT NOT NULL DEFAULT '[]',
              entities TEXT NOT NULL DEFAULT '[]',
              token_count INTEGER NOT NULL DEFAULT 0,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_episodes_conversation
              ON episodes(conversation_id, turn_start ASC);

            CREATE INDEX IF NOT EXISTS idx_episodes_updated
              ON episodes(updated_at DESC);

            CREATE VIRTUAL TABLE IF NOT EXISTS episodes_fts USING fts5(
              id UNINDEXED,
              conversation_id UNINDEXED,
              summary,
              key_decisions,
              entities
            );

            CREATE TRIGGER IF NOT EXISTS episodes_ai AFTER INSERT ON episodes BEGIN
              INSERT INTO episodes_fts (id, conversation_id, summary, key_decisions, entities)
              VALUES (new.id, new.conversation_id, new.summary, new.key_decisions, new.entities);
            END;

            CREATE TRIGGER IF NOT EXISTS episodes_ad AFTER DELETE ON episodes BEGIN
              DELETE FROM episodes_fts WHERE id = old.id;
            END;

            CREATE TRIGGER IF NOT EXISTS episodes_au AFTER UPDATE ON episodes BEGIN
              DELETE FROM episodes_fts WHERE id = old.id;
              INSERT INTO episodes_fts (id, conversation_id, summary, key_decisions, entities)
              VALUES (new.id, new.conversation_id, new.summary, new.key_decisions, new.entities);
            END;
            "#,
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;

        // Non-destructive migrations for local SQLite databases created by older desktop builds.
        let _ = conn.execute_batch("ALTER TABLE plans ADD COLUMN conversation_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE messages ADD COLUMN model_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE messages ADD COLUMN agent_run_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE memories ADD COLUMN client_id TEXT;");
        let _ = conn.execute_batch(
            "ALTER TABLE memories ADD COLUMN category TEXT NOT NULL DEFAULT 'personal';",
        );
        let _ = conn
            .execute_batch("ALTER TABLE memories ADD COLUMN scope TEXT NOT NULL DEFAULT 'user';");
        let _ = conn.execute_batch(
            "ALTER TABLE memories ADD COLUMN status TEXT NOT NULL DEFAULT 'approved';",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE memories ADD COLUMN source_message_ids TEXT NOT NULL DEFAULT '[]';",
        );
        let _ = conn.execute_batch("ALTER TABLE memories ADD COLUMN updated_at TEXT;");
        let _ = conn
            .execute_batch("UPDATE memories SET updated_at = created_at WHERE updated_at IS NULL;");
        let _ = conn
            .execute_batch("ALTER TABLE memories ADD COLUMN salience REAL NOT NULL DEFAULT 0.5;");
        let _ = conn.execute_batch("ALTER TABLE memories ADD COLUMN last_used_at TEXT;");
        let _ = conn.execute_batch("ALTER TABLE memories ADD COLUMN deleted_at TEXT;");
        let _ = conn.execute_batch(
            "ALTER TABLE memories ADD COLUMN recall_count INTEGER NOT NULL DEFAULT 0;",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_memories_pinned_salience ON memories(pinned DESC, salience DESC, updated_at DESC);",
        );
        let _ = conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_memories_recall ON memories(recall_count DESC);",
        );
        let _ = conn.execute_batch("ALTER TABLE agent_runs ADD COLUMN lane_id TEXT;");
        let _ = conn.execute_batch(
            "ALTER TABLE agent_runs ADD COLUMN priority TEXT NOT NULL DEFAULT '\"normal\"';",
        );
        let _ = conn.execute_batch("ALTER TABLE agent_runs ADD COLUMN model_provider_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE agent_runs ADD COLUMN model_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE agent_runs ADD COLUMN autonomy_profile_id TEXT;");
        let _ = conn.execute_batch("ALTER TABLE agent_runs ADD COLUMN checkpoint_summary TEXT;");
        let _ = conn.execute_batch("ALTER TABLE agent_runs ADD COLUMN last_error TEXT;");
        let _ = conn.execute_batch("ALTER TABLE agent_runs ADD COLUMN heartbeat_at TEXT;");
        let _ = conn.execute_batch("ALTER TABLE agent_runs ADD COLUMN completed_at TEXT;");
        let _ = conn.execute_batch(
            "ALTER TABLE agent_runs ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'local';",
        );
        let _ = conn.execute_batch(
            "ALTER TABLE agent_lanes ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'local';",
        );
        let _ = conn.execute("ALTER TABLE conversations ADD COLUMN project_id TEXT;", []);
        let _ = conn.execute("ALTER TABLE conversations ADD COLUMN folder_id TEXT;", []);
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN instructions TEXT;", []);
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN root_path TEXT;", []);
        let _ = conn.execute("ALTER TABLE folders ADD COLUMN root_path TEXT;", []);
        let _ = conn.execute("ALTER TABLE conversations ADD COLUMN root_path TEXT;", []);
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN organization_id TEXT;", []);
        let _ = conn.execute("ALTER TABLE folders ADD COLUMN organization_id TEXT;", []);

        let _ = conn.execute_batch(
            r#"
            DELETE FROM agent_runs WHERE conversation_id IS NOT NULL AND autonomy_profile_id IS NULL;
            DELETE FROM agent_lanes WHERE conversation_id IS NOT NULL AND id NOT IN (SELECT DISTINCT lane_id FROM agent_runs WHERE lane_id IS NOT NULL);
            "#,
        );

        conn.execute_batch(
            r#"
            INSERT OR REPLACE INTO memories_fts (id, content, category, scope)
              SELECT id, content, category, scope
              FROM memories
              WHERE deleted_at IS NULL;

            CREATE INDEX IF NOT EXISTS idx_agent_runs_updated
              ON agent_runs(updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_agent_runs_conversation_updated
              ON agent_runs(conversation_id, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_agent_runs_lane_status
              ON agent_runs(lane_id, status, priority);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_lanes_conversation
              ON agent_lanes(conversation_id)
              WHERE conversation_id IS NOT NULL;
            CREATE INDEX IF NOT EXISTS idx_agent_steps_run_sequence
              ON agent_steps(run_id, sequence);
            CREATE INDEX IF NOT EXISTS idx_memories_status_updated
              ON memories(status, updated_at DESC)
              WHERE deleted_at IS NULL;
            CREATE INDEX IF NOT EXISTS idx_memories_client_id
              ON memories(client_id)
              WHERE deleted_at IS NULL AND client_id IS NOT NULL;

            INSERT INTO episodes_fts (id, conversation_id, summary, key_decisions, entities)
              SELECT id, conversation_id, summary, key_decisions, entities
              FROM episodes
              WHERE id NOT IN (SELECT id FROM episodes_fts);
            "#,
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;

        Ok(())
    }

    pub fn upsert_conversation(&self, conversation: &Conversation) -> AroResult<()> {
        // Le placement (projet/dossier/root) n'est JAMAIS effacé par un
        // upsert : une valeur entrante NULL conserve la valeur stockée.
        // Seuls move_conversation / set_conversation_root_path (UPDATE
        // directs) peuvent le modifier ou le réinitialiser. Sans esto,
        // chaque sync cloud ou renommage ramenait les conversations
        // classées vers "non classées".
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO conversations (id, title, mode, created_at, updated_at, project_id, folder_id, root_path)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              mode = excluded.mode,
              updated_at = excluded.updated_at,
              project_id = COALESCE(excluded.project_id, conversations.project_id),
              folder_id = COALESCE(excluded.folder_id, conversations.folder_id),
              root_path = COALESCE(excluded.root_path, conversations.root_path)
            "#,
            params![
                conversation.id.to_string(),
                conversation.title,
                serde_json::to_string(&conversation.mode)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                conversation.created_at.to_rfc3339(),
                conversation.updated_at.to_rfc3339(),
                conversation.project_id.map(|id| id.to_string()),
                conversation.folder_id.map(|id| id.to_string()),
                conversation.root_path,
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn create_conversation(
        &self,
        title: impl Into<String>,
        mode: AssistantMode,
        project_id: Option<Uuid>,
        folder_id: Option<Uuid>,
    ) -> AroResult<Conversation> {
        let mut conversation = Conversation::new(title, mode);
        conversation.project_id = project_id;
        conversation.folder_id = folder_id;
        self.upsert_conversation(&conversation)?;
        Ok(conversation)
    }

    pub fn get_conversation(&self, id: Uuid) -> AroResult<Option<Conversation>> {
        let conn = self.connect()?;
        conn.query_row(
            "SELECT id, title, mode, created_at, updated_at, project_id, folder_id, root_path FROM conversations WHERE id = ?1",
            params![id.to_string()],
            map_conversation,
        )
        .optional()
        .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn list_conversations(&self) -> AroResult<Vec<Conversation>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, title, mode, created_at, updated_at, project_id, folder_id, root_path FROM conversations ORDER BY updated_at DESC",
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map([], map_conversation)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn move_conversation(
        &self,
        conversation_id: Uuid,
        project_id: Option<Uuid>,
        folder_id: Option<Uuid>,
    ) -> AroResult<Conversation> {
        // UPDATE direct : avec l'upsert préservant le placement, passer par
        // upsert empêcherait de déclasser (None explicite).
        let conn = self.connect()?;
        let changed = conn
            .execute(
                "UPDATE conversations SET project_id = ?1, folder_id = ?2, updated_at = ?3 WHERE id = ?4",
                params![
                    project_id.map(|id| id.to_string()),
                    folder_id.map(|id| id.to_string()),
                    Utc::now().to_rfc3339(),
                    conversation_id.to_string()
                ],
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        if changed == 0 {
            return Err(AroError::Memory("Conversation not found".into()));
        }
        self.get_conversation(conversation_id)?
            .ok_or_else(|| AroError::Memory("Conversation not found".into()))
    }

    pub fn set_conversation_root_path(
        &self,
        conversation_id: Uuid,
        root_path: Option<String>,
    ) -> AroResult<Conversation> {
        let conn = self.connect()?;
        let changed = conn
            .execute(
                "UPDATE conversations SET root_path = ?1, updated_at = ?2 WHERE id = ?3",
                params![
                    root_path,
                    Utc::now().to_rfc3339(),
                    conversation_id.to_string()
                ],
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        if changed == 0 {
            return Err(AroError::Memory("Conversation not found".into()));
        }
        self.get_conversation(conversation_id)?
            .ok_or_else(|| AroError::Memory("Conversation not found".into()))
    }

    pub fn list_projects(&self) -> AroResult<Vec<Project>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare("SELECT id, name, description, instructions, root_path, color, icon, created_at, updated_at, organization_id FROM projects ORDER BY updated_at DESC")
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map([], map_project)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn save_project(&self, project: &Project) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO projects (id, name, description, instructions, root_path, color, icon, created_at, updated_at, organization_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
              name = excluded.name,
              description = excluded.description,
              instructions = excluded.instructions,
              root_path = excluded.root_path,
              color = excluded.color,
              icon = excluded.icon,
              updated_at = excluded.updated_at,
              organization_id = excluded.organization_id
            "#,
            params![
                project.id.to_string(),
                project.name,
                project.description,
                project.instructions,
                project.root_path,
                project.color,
                project.icon,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339(),
                project.organization_id,
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn search_messages(&self, query: &str, limit: usize) -> AroResult<Vec<ChatMessage>> {
        let conn = self.connect()?;
        let pattern = format!("%{}%", query.trim());
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, role, content, created_at, token_estimate, model_id, agent_run_id
                FROM messages
                WHERE content LIKE ?1
                ORDER BY created_at DESC
                LIMIT ?2
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![pattern, limit as i64], map_message)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn delete_project(&self, id: Uuid) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            "DELETE FROM projects WHERE id = ?1",
            params![id.to_string()],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn list_folders(&self) -> AroResult<Vec<Folder>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare("SELECT id, project_id, name, root_path, color, icon, created_at, updated_at, organization_id FROM folders ORDER BY updated_at DESC")
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map([], map_folder)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn save_folder(&self, folder: &Folder) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO folders (id, project_id, name, root_path, color, icon, created_at, updated_at, organization_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
              project_id = excluded.project_id,
              name = excluded.name,
              root_path = excluded.root_path,
              color = excluded.color,
              icon = excluded.icon,
              updated_at = excluded.updated_at,
              organization_id = excluded.organization_id
            "#,
            params![
                folder.id.to_string(),
                folder.project_id.map(|id| id.to_string()),
                folder.name,
                folder.root_path,
                folder.color,
                folder.icon,
                folder.created_at.to_rfc3339(),
                folder.updated_at.to_rfc3339(),
                folder.organization_id,
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn resolve_effective_root_path(&self, conversation_id: Uuid) -> AroResult<Option<String>> {
        let conn = self.connect()?;
        let conv: Option<(Option<String>, Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT root_path, project_id, folder_id FROM conversations WHERE id = ?1",
                params![conversation_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|err| AroError::Memory(err.to_string()))?;

        let (conv_root, project_id_str, folder_id_str) = match conv {
            Some(data) => data,
            None => return Ok(None),
        };

        if let Some(path) = conv_root {
            if !path.trim().is_empty() {
                return Ok(Some(path));
            }
        }

        let mut folder_parent_project: Option<String> = None;
        if let Some(folder_id) = folder_id_str {
            let folder_row: Option<(Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT root_path, project_id FROM folders WHERE id = ?1",
                    params![folder_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|err| AroError::Memory(err.to_string()))?;

            if let Some((folder_root, folder_project)) = folder_row {
                if let Some(path) = folder_root {
                    if !path.trim().is_empty() {
                        return Ok(Some(path));
                    }
                }
                folder_parent_project = folder_project;
            }
        }

        // Héritage dossier vide -> projet parent du dossier.
        if let Some(folder_project_id) = folder_parent_project {
            let project_root: Option<String> = conn
                .query_row(
                    "SELECT root_path FROM projects WHERE id = ?1",
                    params![folder_project_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|err| AroError::Memory(err.to_string()))?
                .flatten();

            if let Some(path) = project_root {
                if !path.trim().is_empty() {
                    return Ok(Some(path));
                }
            }
        }

        if let Some(project_id) = project_id_str {
            let project_root: Option<String> = conn
                .query_row(
                    "SELECT root_path FROM projects WHERE id = ?1",
                    params![project_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|err| AroError::Memory(err.to_string()))?
                .flatten();

            if let Some(path) = project_root {
                if !path.trim().is_empty() {
                    return Ok(Some(path));
                }
            }
        }

        Ok(None)
    }

    /// Fusionne les projets strictement identiques (même nom + mêmes
    /// métadonnées, ids différents) créés par les anciennes boucles de sync
    /// cloud. Les conversations et dossiers des doublons sont remappés vers
    /// le projet gardé. Retourne les ids supprimés (à purger côté cloud).
    pub fn deduplicate_projects(&self) -> AroResult<Vec<Uuid>> {
        let projects = self.list_projects()?;
        let conversations = self.list_conversations()?;
        let mut groups: std::collections::HashMap<String, Vec<Project>> =
            std::collections::HashMap::new();
        for p in projects {
            let key = format!(
                "{}\n{}\n{}\n{}\n{:?}",
                p.name.trim().to_lowercase(),
                p.root_path.clone().unwrap_or_default(),
                p.description.clone().unwrap_or_default(),
                p.instructions.clone().unwrap_or_default(),
                p.organization_id
            );
            groups.entry(key).or_default().push(p);
        }
        let mut removed = Vec::new();
        for members in groups.values().filter(|m| m.len() > 1) {
            let mut ranked = members.clone();
            ranked.sort_by(|a, b| {
                let ca = conversations
                    .iter()
                    .filter(|c| c.project_id == Some(a.id))
                    .count();
                let cb = conversations
                    .iter()
                    .filter(|c| c.project_id == Some(b.id))
                    .count();
                cb.cmp(&ca).then_with(|| b.updated_at.cmp(&a.updated_at))
            });
            let keeper = ranked[0].id;
            for dup in &ranked[1..] {
                let mut conn = self.connect()?;
                let tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|err| AroError::Memory(err.to_string()))?;
                tx.execute(
                    "UPDATE conversations SET project_id = ?1 WHERE project_id = ?2",
                    params![keeper.to_string(), dup.id.to_string()],
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;
                tx.execute(
                    "UPDATE folders SET project_id = ?1 WHERE project_id = ?2",
                    params![keeper.to_string(), dup.id.to_string()],
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;
                tx.execute(
                    "DELETE FROM projects WHERE id = ?1",
                    params![dup.id.to_string()],
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;
                tx.commit()
                    .map_err(|err| AroError::Memory(err.to_string()))?;
                removed.push(dup.id);
            }
        }
        Ok(removed)
    }

    /// Idem pour les dossiers : même nom (insensible à la casse) sous le
    /// même projet et même root. Retourne les ids supprimés.
    pub fn deduplicate_folders(&self) -> AroResult<Vec<Uuid>> {
        let folders = self.list_folders()?;
        let conversations = self.list_conversations()?;
        let mut groups: std::collections::HashMap<String, Vec<Folder>> =
            std::collections::HashMap::new();
        for f in folders {
            let key = format!(
                "{}\n{}\n{}\n{:?}",
                f.project_id.map(|id| id.to_string()).unwrap_or_default(),
                f.name.trim().to_lowercase(),
                f.root_path.clone().unwrap_or_default(),
                f.organization_id
            );
            groups.entry(key).or_default().push(f);
        }
        let mut removed = Vec::new();
        for members in groups.values().filter(|m| m.len() > 1) {
            let mut ranked = members.clone();
            ranked.sort_by(|a, b| {
                let ca = conversations
                    .iter()
                    .filter(|c| c.folder_id == Some(a.id))
                    .count();
                let cb = conversations
                    .iter()
                    .filter(|c| c.folder_id == Some(b.id))
                    .count();
                cb.cmp(&ca).then_with(|| b.updated_at.cmp(&a.updated_at))
            });
            let keeper = ranked[0].id;
            for dup in &ranked[1..] {
                let mut conn = self.connect()?;
                let tx = conn
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(|err| AroError::Memory(err.to_string()))?;
                tx.execute(
                    "UPDATE conversations SET folder_id = ?1 WHERE folder_id = ?2",
                    params![keeper.to_string(), dup.id.to_string()],
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;
                tx.execute(
                    "DELETE FROM folders WHERE id = ?1",
                    params![dup.id.to_string()],
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;
                tx.commit()
                    .map_err(|err| AroError::Memory(err.to_string()))?;
                removed.push(dup.id);
            }
        }
        Ok(removed)
    }

    pub fn delete_folder(&self, id: Uuid) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM folders WHERE id = ?1", params![id.to_string()])
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn add_message(&self, message: &ChatMessage) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO messages (id, conversation_id, role, content, created_at, token_estimate, model_id, agent_run_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                message.id.to_string(),
                message.conversation_id.to_string(),
                serde_json::to_string(&message.role)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                message.content,
                message.created_at.to_rfc3339(),
                message.token_estimate,
                message.model_id,
                message.agent_run_id.map(|id| id.to_string()),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn upsert_message(&self, message: &ChatMessage) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO messages (id, conversation_id, role, content, created_at, token_estimate, model_id, agent_run_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
              content = excluded.content,
              token_estimate = excluded.token_estimate,
              model_id = excluded.model_id,
              agent_run_id = excluded.agent_run_id
            "#,
            params![
                message.id.to_string(),
                message.conversation_id.to_string(),
                serde_json::to_string(&message.role)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                message.content,
                message.created_at.to_rfc3339(),
                message.token_estimate,
                message.model_id,
                message.agent_run_id.map(|id| id.to_string()),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn list_messages(&self, conversation_id: Uuid) -> AroResult<Vec<ChatMessage>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, role, content, created_at, token_estimate, model_id, agent_run_id
                FROM messages
                WHERE conversation_id = ?1
                ORDER BY created_at ASC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![conversation_id.to_string()], map_message)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn delete_conversation(&self, conversation_id: Uuid) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            params![conversation_id.to_string()],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    /// Supprime les conversations sans aucun message (brouillons créés par
    /// les captures vocales, sessions interrompues…). Ne touche jamais aux
    /// conversations avec du contenu. Retourne les ids supprimés.
    pub fn delete_empty_conversations(&self) -> AroResult<Vec<Uuid>> {
        let ids: Vec<String> = {
            let conn = self.connect()?;
            let mut stmt = conn
                .prepare(
                    "SELECT c.id FROM conversations c WHERE NOT EXISTS (SELECT 1 FROM messages m WHERE m.conversation_id = c.id)",
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;
            let ids = stmt
                .query_map([], |row| row.get(0))
                .map_err(|err| AroError::Memory(err.to_string()))?
                .collect::<Result<Vec<String>, _>>()
                .map_err(|err| AroError::Memory(err.to_string()))?;
            ids
        };
        let mut deleted = Vec::new();
        for id in ids {
            let id = match Uuid::parse_str(&id) {
                Ok(id) => id,
                Err(_) => continue,
            };
            // Ligne par ligne : une conversation retenue par une contrainte
            // (runs, lanes…) est conservée au lieu de tout annuler.
            if self.delete_conversation(id).is_ok() {
                deleted.push(id);
            }
        }
        Ok(deleted)
    }

    pub fn get_message(&self, id: Uuid) -> AroResult<Option<ChatMessage>> {
        let conn = self.connect()?;
        conn.query_row(
            "SELECT id, conversation_id, role, content, created_at, token_estimate, model_id, agent_run_id FROM messages WHERE id = ?1",
            params![id.to_string()],
            map_message,
        )
        .optional()
        .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn update_message(&self, id: Uuid, content: String) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            "UPDATE messages SET content = ?2 WHERE id = ?1",
            params![id.to_string(), content],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn delete_messages_after(
        &self,
        conversation_id: Uuid,
        created_at: DateTime<Utc>,
    ) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1 AND created_at > ?2",
            params![conversation_id.to_string(), created_at.to_rfc3339()],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn reset(&self) -> AroResult<()> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.execute_batch(
            r#"
            DELETE FROM messages;
            DELETE FROM memories;
            DELETE FROM memories_fts;
            DELETE FROM conversations;
            DELETE FROM episodes;
            DELETE FROM episodes_fts;
            "#,
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.commit()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn add_memory(&self, memory: &MemoryEntry) -> AroResult<()> {
        self.upsert_memory(&memory.clone().into()).map(|_| ())
    }

    pub fn upsert_memory(&self, memory: &LongTermMemory) -> AroResult<LongTermMemory> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.execute(
            r#"
            INSERT INTO memories
              (id, client_id, content, category, scope, status, source_conversation_id,
               source_message_ids, created_at, updated_at, pinned, salience, recall_count, last_used_at, deleted_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, NULL)
            ON CONFLICT(id) DO UPDATE SET
              client_id = excluded.client_id,
              content = excluded.content,
              category = excluded.category,
              scope = excluded.scope,
              status = excluded.status,
              source_conversation_id = excluded.source_conversation_id,
              source_message_ids = excluded.source_message_ids,
              updated_at = excluded.updated_at,
              pinned = excluded.pinned,
              salience = excluded.salience,
              recall_count = CASE WHEN excluded.recall_count > 0 THEN excluded.recall_count ELSE memories.recall_count END,
              last_used_at = excluded.last_used_at,
              deleted_at = NULL
            "#,
            params![
                memory.id.to_string(),
                memory.client_id,
                memory.content,
                memory.category,
                memory.scope,
                memory.status,
                memory.source_conversation_id.map(|id| id.to_string()),
                serde_json::to_string(&memory.source_message_ids)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                memory.created_at.to_rfc3339(),
                memory.updated_at.to_rfc3339(),
                memory.pinned as i32,
                memory.salience,
                memory.recall_count as i64,
                memory.last_used_at.map(|value| value.to_rfc3339()),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.execute(
            r#"
            INSERT OR REPLACE INTO memories_fts (id, content, category, scope)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                memory.id.to_string(),
                memory.content,
                memory.category,
                memory.scope,
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.commit()
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(memory.clone())
    }

    pub fn list_memories(&self) -> AroResult<Vec<LongTermMemory>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, client_id, content, category, scope, status, source_conversation_id,
                       source_message_ids, created_at, updated_at, pinned, salience, last_used_at, recall_count
                FROM memories
                WHERE deleted_at IS NULL
                ORDER BY pinned DESC, updated_at DESC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map([], map_memory)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn search_memories(&self, query: &str, limit: usize) -> AroResult<Vec<LongTermMemory>> {
        let conn = self.connect()?;
        let limit = limit.clamp(1, 50) as i64;
        let trimmed = query.trim();
        if trimmed.is_empty() {
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT id, client_id, content, category, scope, status, source_conversation_id,
                           source_message_ids, created_at, updated_at, pinned, salience, last_used_at, recall_count
                    FROM memories
                    WHERE deleted_at IS NULL AND status = ?1
                    ORDER BY pinned DESC, salience DESC, updated_at DESC
                    LIMIT ?2
                    "#,
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;
            let rows = stmt
                .query_map(params![MEMORY_STATUS_APPROVED, limit], map_memory)
                .map_err(|err| AroError::Memory(err.to_string()))?;
            return rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| AroError::Memory(err.to_string()));
        }

        let fts_query = memory_fts_query(trimmed);
        if let Some(fts_query) = fts_query {
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT m.id, m.client_id, m.content, m.category, m.scope, m.status,
                           m.source_conversation_id, m.source_message_ids, m.created_at,
                           m.updated_at, m.pinned, m.salience, m.last_used_at, m.recall_count
                    FROM memories_fts f
                    JOIN memories m ON m.id = f.id
                    WHERE memories_fts MATCH ?1
                      AND m.deleted_at IS NULL
                      AND m.status = ?2
                    ORDER BY m.pinned DESC, bm25(memories_fts), m.salience DESC, m.updated_at DESC
                    LIMIT ?3
                    "#,
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;
            let rows = stmt.query_map(
                params![fts_query, MEMORY_STATUS_APPROVED, limit],
                map_memory,
            );
            if let Ok(rows) = rows {
                return rows
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|err| AroError::Memory(err.to_string()));
            }
        }

        let pattern = format!("%{trimmed}%");
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, client_id, content, category, scope, status, source_conversation_id,
                       source_message_ids, created_at, updated_at, pinned, salience, last_used_at, recall_count
                FROM memories
                WHERE deleted_at IS NULL
                AND status = ?1
                AND (content LIKE ?2 OR category LIKE ?2 OR scope LIKE ?2)
                ORDER BY pinned DESC, salience DESC, updated_at DESC
                LIMIT ?3
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![MEMORY_STATUS_APPROVED, pattern, limit], map_memory)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn get_memories_by_ids(&self, memory_ids: &[Uuid]) -> AroResult<Vec<LongTermMemory>> {
        if memory_ids.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.connect()?;
        let mut retrieved_map: std::collections::HashMap<Uuid, LongTermMemory> =
            std::collections::HashMap::with_capacity(memory_ids.len());

        for chunk in memory_ids.chunks(500) {
            let placeholders = (1..=chunk.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                r#"
                SELECT id, client_id, content, category, scope, status, source_conversation_id,
                       source_message_ids, created_at, updated_at, pinned, salience, last_used_at, recall_count
                FROM memories
                WHERE id IN ({placeholders}) AND deleted_at IS NULL
                "#
            );

            let mut stmt = conn
                .prepare(&sql)
                .map_err(|err| AroError::Memory(err.to_string()))?;

            let id_strings: Vec<String> = chunk.iter().map(|id| id.to_string()).collect();
            let rows = stmt
                .query_map(rusqlite::params_from_iter(id_strings), map_memory)
                .map_err(|err| AroError::Memory(err.to_string()))?;

            for row in rows {
                let memory = row.map_err(|err| AroError::Memory(err.to_string()))?;
                retrieved_map.insert(memory.id, memory);
            }
        }

        let mut ordered_memories = Vec::with_capacity(retrieved_map.len());
        for id in memory_ids {
            if let Some(memory) = retrieved_map.remove(id) {
                if memory.approved_for_recall() {
                    ordered_memories.push(memory);
                }
            }
        }

        Ok(ordered_memories)
    }

    pub fn get_memory(&self, id: Uuid) -> AroResult<Option<LongTermMemory>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, client_id, content, category, scope, status, source_conversation_id,
                       source_message_ids, created_at, updated_at, pinned, salience, last_used_at, recall_count
                FROM memories
                WHERE id = ?1 AND deleted_at IS NULL
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let mut rows = stmt
            .query_map(params![id.to_string()], map_memory)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        match rows.next() {
            Some(Ok(memory)) => Ok(Some(memory)),
            Some(Err(err)) => Err(AroError::Memory(err.to_string())),
            None => Ok(None),
        }
    }

    pub fn list_pinned_memories(&self) -> AroResult<Vec<LongTermMemory>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, client_id, content, category, scope, status, source_conversation_id,
                       source_message_ids, created_at, updated_at, pinned, salience, last_used_at, recall_count
                FROM memories
                WHERE pinned = 1 AND deleted_at IS NULL AND status = ?1
                ORDER BY salience DESC, updated_at DESC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![MEMORY_STATUS_APPROVED], map_memory)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let memories = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(memories
            .into_iter()
            .filter(LongTermMemory::approved_for_recall)
            .collect())
    }

    pub fn memory_context_sources(
        &self,
        query: &str,
        limit: usize,
    ) -> AroResult<Vec<ContextSource>> {
        let limit = limit.clamp(1, 50);
        let mut memories = self.list_pinned_memories()?;

        let search_query = query.trim();
        let searched = if search_query.is_empty() {
            self.search_memories("", limit * 2)?
        } else {
            self.search_memories(search_query, limit * 2)?
        };
        for memory in searched {
            if memory.approved_for_recall()
                && !memories.iter().any(|existing| existing.id == memory.id)
            {
                memories.push(memory);
            }
        }

        Ok(diversify_memories(memories, limit)
            .into_iter()
            .take(limit)
            .map(memory_to_context_source)
            .collect())
    }

    pub fn delete_memory(&self, memory_id: Uuid) -> AroResult<()> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.execute(
            "UPDATE memories SET status = ?2, deleted_at = ?3, updated_at = ?3 WHERE id = ?1",
            params![
                memory_id.to_string(),
                aro_core::MEMORY_STATUS_ARCHIVED,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.execute(
            "DELETE FROM memories_fts WHERE id = ?1",
            params![memory_id.to_string()],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.commit().map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn touch_memories_used(&self, memory_ids: &[Uuid]) -> AroResult<()> {
        if memory_ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let now = Utc::now().to_rfc3339();
        for id in memory_ids {
            tx.execute(
                "UPDATE memories SET recall_count = recall_count + 1, last_used_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
                params![id.to_string(), now],
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        }
        tx.commit()
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    // ========================================================================
    // Episodic Memory Operations
    // ========================================================================

    /// Stores or updates an episode in an atomic transaction that automatically
    /// synchronizes both the relational `episodes` table and the `episodes_fts` virtual table
    /// via SQLite triggers.
    pub fn store_episode(&self, episode: &Episode) -> AroResult<()> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| AroError::Memory(err.to_string()))?;

        tx.execute(
            r#"
            INSERT INTO episodes (
                id, conversation_id, turn_start, turn_end,
                summary, key_decisions, entities, token_count,
                created_at, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                conversation_id = excluded.conversation_id,
                turn_start = excluded.turn_start,
                turn_end = excluded.turn_end,
                summary = excluded.summary,
                key_decisions = excluded.key_decisions,
                entities = excluded.entities,
                token_count = excluded.token_count,
                updated_at = excluded.updated_at
            "#,
            params![
                episode.id.to_string(),
                episode.conversation_id.to_string(),
                episode.turn_start as i64,
                episode.turn_end as i64,
                episode.summary,
                serde_json::to_string(&episode.key_decisions)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                serde_json::to_string(&episode.entities)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                episode.token_count as i64,
                episode.created_at.to_rfc3339(),
                episode.updated_at.to_rfc3339(),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;

        tx.commit().map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    /// Lists all episodes for a conversation ordered chronologically by `turn_start ASC`.
    pub fn list_episodes(&self, conversation_id: Uuid) -> AroResult<Vec<Episode>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, turn_start, turn_end,
                       summary, key_decisions, entities, token_count,
                       created_at, updated_at
                FROM episodes
                WHERE conversation_id = ?1
                ORDER BY turn_start ASC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;

        let rows = stmt
            .query_map(params![conversation_id.to_string()], map_episode)
            .map_err(|err| AroError::Memory(err.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    /// Lists episodes globally ordered chronologically descending (most recent first).
    pub fn list_all_episodes(&self, limit: usize) -> AroResult<Vec<Episode>> {
        let conn = self.connect()?;
        let limit = limit.clamp(1, 500) as i64;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, turn_start, turn_end,
                       summary, key_decisions, entities, token_count,
                       created_at, updated_at
                FROM episodes
                ORDER BY created_at DESC, turn_start DESC
                LIMIT ?1
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;

        let rows = stmt
            .query_map(params![limit], map_episode)
            .map_err(|err| AroError::Memory(err.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    /// Returns the most recent episode for a conversation based on `turn_end DESC`.
    pub fn get_latest_episode(&self, conversation_id: Uuid) -> AroResult<Option<Episode>> {
        let conn = self.connect()?;
        conn.query_row(
            r#"
            SELECT id, conversation_id, turn_start, turn_end,
                   summary, key_decisions, entities, token_count,
                   created_at, updated_at
            FROM episodes
            WHERE conversation_id = ?1
            ORDER BY turn_end DESC, turn_start DESC, created_at DESC
            LIMIT 1
            "#,
            params![conversation_id.to_string()],
            map_episode,
        )
        .optional()
        .map_err(|err| AroError::Memory(err.to_string()))
    }

    /// Searches episodes within a conversation using SQLite FTS5 with BM25 ranking,
    /// matching across summary, key_decisions, and entities.
    pub fn search_episodes(
        &self,
        conversation_id: Uuid,
        query: &str,
        limit: usize,
    ) -> AroResult<Vec<Episode>> {
        let conn = self.connect()?;
        let limit = limit.clamp(1, 100) as i64;
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        let fts_query = episode_fts_query(trimmed);
        if let Some(fts_query) = fts_query {
            let mut stmt = conn
                .prepare(
                    r#"
                    SELECT e.id, e.conversation_id, e.turn_start, e.turn_end,
                           e.summary, e.key_decisions, e.entities, e.token_count,
                           e.created_at, e.updated_at
                    FROM episodes_fts f
                    JOIN episodes e ON e.id = f.id
                    WHERE episodes_fts MATCH ?1
                      AND e.conversation_id = ?2
                    ORDER BY bm25(episodes_fts) ASC, e.turn_start DESC
                    LIMIT ?3
                    "#,
                )
                .map_err(|err| AroError::Memory(err.to_string()))?;

            let rows = stmt.query_map(
                params![fts_query, conversation_id.to_string(), limit],
                map_episode,
            );
            if let Ok(rows) = rows {
                let collected: Result<Vec<_>, _> = rows.collect();
                if let Ok(results) = collected {
                    if !results.is_empty() {
                        return Ok(results);
                    }
                }
            }
        }

        // Fallback search with LIKE if FTS produced no matches or tokenization yielded none
        let pattern = format!("%{trimmed}%");
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, turn_start, turn_end,
                       summary, key_decisions, entities, token_count,
                       created_at, updated_at
                FROM episodes
                WHERE conversation_id = ?1
                  AND (summary LIKE ?2 OR key_decisions LIKE ?2 OR entities LIKE ?2)
                ORDER BY turn_start DESC
                LIMIT ?3
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;

        let rows = stmt
            .query_map(
                params![conversation_id.to_string(), pattern, limit],
                map_episode,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn upsert_agent_lane(&self, lane: &AgentLane) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO agent_lanes
              (id, conversation_id, title, status, priority, max_concurrent_runs,
               created_at, updated_at, sync_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'local')
            ON CONFLICT(id) DO UPDATE SET
              conversation_id = excluded.conversation_id,
              title = excluded.title,
              status = excluded.status,
              priority = excluded.priority,
              max_concurrent_runs = excluded.max_concurrent_runs,
              updated_at = excluded.updated_at,
              sync_status = 'local'
            "#,
            params![
                lane.id.to_string(),
                lane.conversation_id.map(|id| id.to_string()),
                lane.title,
                serde_json::to_string(&lane.status)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                serde_json::to_string(&lane.priority)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                lane.max_concurrent_runs,
                lane.created_at.to_rfc3339(),
                lane.updated_at.to_rfc3339(),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn ensure_agent_lane(
        &self,
        conversation_id: Option<Uuid>,
        title: impl Into<String>,
    ) -> AroResult<AgentLane> {
        if let Some(conversation_id) = conversation_id {
            if let Some(lane) = self.agent_lane_for_conversation(conversation_id)? {
                return Ok(lane);
            }
        }
        let lane = AgentLane::new(conversation_id, title);
        self.upsert_agent_lane(&lane)?;
        Ok(lane)
    }

    pub fn agent_lane_for_conversation(
        &self,
        conversation_id: Uuid,
    ) -> AroResult<Option<AgentLane>> {
        let conn = self.connect()?;
        conn.query_row(
            r#"
            SELECT id, conversation_id, title, status, priority, max_concurrent_runs,
                   created_at, updated_at
            FROM agent_lanes
            WHERE conversation_id = ?1
            "#,
            params![conversation_id.to_string()],
            map_agent_lane,
        )
        .optional()
        .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn get_agent_lane(&self, lane_id: Uuid) -> AroResult<Option<AgentLane>> {
        let conn = self.connect()?;
        conn.query_row(
            r#"
            SELECT id, conversation_id, title, status, priority, max_concurrent_runs,
                   created_at, updated_at
            FROM agent_lanes
            WHERE id = ?1
            "#,
            params![lane_id.to_string()],
            map_agent_lane,
        )
        .optional()
        .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn list_agent_lanes(&self) -> AroResult<Vec<AgentLane>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, title, status, priority, max_concurrent_runs,
                       created_at, updated_at
                FROM agent_lanes
                ORDER BY updated_at DESC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map([], map_agent_lane)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn list_agent_lane_views(&self) -> AroResult<Vec<AgentLaneView>> {
        self.list_agent_lanes()?
            .into_iter()
            .map(|lane| {
                Ok(AgentLaneView {
                    queued_count: self
                        .count_agent_runs_for_lane(lane.id, AgentRunStatus::Queued)?,
                    running_count: self
                        .count_agent_runs_for_lane(lane.id, AgentRunStatus::Running)?,
                    waiting_count: self
                        .count_agent_runs_for_lane(lane.id, AgentRunStatus::Waiting)?
                        + self.count_agent_runs_for_lane(lane.id, AgentRunStatus::Paused)?,
                    latest_runs: self.list_agent_runs_for_lane(lane.id, 4)?,
                    lane,
                })
            })
            .collect()
    }

    pub fn update_agent_lane_status(
        &self,
        lane_id: Uuid,
        status: AgentLaneStatus,
    ) -> AroResult<Option<AgentLane>> {
        let mut lane = match self.get_agent_lane(lane_id)? {
            Some(lane) => lane,
            None => return Ok(None),
        };
        lane.status = status;
        lane.updated_at = Utc::now();
        self.upsert_agent_lane(&lane)?;
        Ok(Some(lane))
    }

    pub fn update_agent_lane_priority(
        &self,
        lane_id: Uuid,
        priority: AgentRunPriority,
    ) -> AroResult<Option<AgentLane>> {
        let mut lane = match self.get_agent_lane(lane_id)? {
            Some(lane) => lane,
            None => return Ok(None),
        };
        lane.priority = priority;
        lane.updated_at = Utc::now();
        self.upsert_agent_lane(&lane)?;
        Ok(Some(lane))
    }

    pub fn count_running_agent_runs(&self) -> AroResult<u32> {
        self.count_agent_runs_by_status(AgentRunStatus::Running)
    }

    pub fn count_running_agent_runs_for_lane(&self, lane_id: Uuid) -> AroResult<u32> {
        self.count_agent_runs_for_lane(lane_id, AgentRunStatus::Running)
    }

    pub fn upsert_agent_run(&self, run: &AgentRun) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO agent_runs
              (id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id,
               autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at,
               heartbeat_at, completed_at, sync_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 'local')
            ON CONFLICT(id) DO UPDATE SET
              lane_id = excluded.lane_id,
              conversation_id = excluded.conversation_id,
              goal = excluded.goal,
              mode = excluded.mode,
              status = excluded.status,
              priority = excluded.priority,
              model_provider_id = excluded.model_provider_id,
              model_id = excluded.model_id,
              autonomy_profile_id = excluded.autonomy_profile_id,
              checkpoint_summary = excluded.checkpoint_summary,
              last_error = excluded.last_error,
              updated_at = excluded.updated_at,
              heartbeat_at = excluded.heartbeat_at,
              completed_at = excluded.completed_at,
              sync_status = 'local'
            "#,
            params![
                run.id.to_string(),
                run.lane_id.map(|id| id.to_string()),
                run.conversation_id.map(|id| id.to_string()),
                run.goal,
                serde_json::to_string(&run.mode)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                serde_json::to_string(&run.status)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                serde_json::to_string(&run.priority)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                run.model_provider_id,
                run.model_id,
                run.autonomy_profile_id.map(|id| id.to_string()),
                run.checkpoint_summary,
                run.last_error,
                run.created_at.to_rfc3339(),
                run.updated_at.to_rfc3339(),
                run.heartbeat_at.map(|value| value.to_rfc3339()),
                run.completed_at.map(|value| value.to_rfc3339()),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn list_agent_runs(&self) -> AroResult<Vec<AgentRun>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id,
                       autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at,
                       heartbeat_at, completed_at
                FROM agent_runs
                WHERE conversation_id IS NULL OR autonomy_profile_id IS NOT NULL
                ORDER BY updated_at DESC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map([], map_agent_run)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn list_agent_runs_for_lane(
        &self,
        lane_id: Uuid,
        limit: usize,
    ) -> AroResult<Vec<AgentRun>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id,
                       autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at,
                       heartbeat_at, completed_at
                FROM agent_runs
                WHERE lane_id = ?1 AND (conversation_id IS NULL OR autonomy_profile_id IS NOT NULL)
                ORDER BY updated_at DESC
                LIMIT ?2
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![lane_id.to_string(), limit as i64], map_agent_run)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    fn count_agent_runs_by_status(&self, status: AgentRunStatus) -> AroResult<u32> {
        let conn = self.connect()?;
        let status_json =
            serde_json::to_string(&status).map_err(|err| AroError::Memory(err.to_string()))?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_runs WHERE status = ?1 AND (conversation_id IS NULL OR autonomy_profile_id IS NOT NULL)",
                params![status_json],
                |row| row.get(0),
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(count.max(0) as u32)
    }

    fn count_agent_runs_for_lane(&self, lane_id: Uuid, status: AgentRunStatus) -> AroResult<u32> {
        let conn = self.connect()?;
        let status_json =
            serde_json::to_string(&status).map_err(|err| AroError::Memory(err.to_string()))?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_runs WHERE lane_id = ?1 AND status = ?2 AND (conversation_id IS NULL OR autonomy_profile_id IS NOT NULL)",
                params![lane_id.to_string(), status_json],
                |row| row.get(0),
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(count.max(0) as u32)
    }

    pub fn get_agent_run(&self, run_id: Uuid) -> AroResult<Option<AgentRun>> {
        let conn = self.connect()?;
        conn.query_row(
            r#"
            SELECT id, lane_id, conversation_id, goal, mode, status, priority, model_provider_id, model_id,
                   autonomy_profile_id, checkpoint_summary, last_error, created_at, updated_at,
                   heartbeat_at, completed_at
            FROM agent_runs
            WHERE id = ?1
            "#,
            params![run_id.to_string()],
            map_agent_run,
        )
        .optional()
        .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn update_agent_run_status(
        &self,
        run_id: Uuid,
        status: AgentRunStatus,
        last_error: Option<String>,
    ) -> AroResult<Option<AgentRun>> {
        let mut run = match self.get_agent_run(run_id)? {
            Some(run) => run,
            None => return Ok(None),
        };
        run.status = status;
        run.last_error = last_error;
        run.updated_at = Utc::now();
        run.heartbeat_at = Some(run.updated_at);
        if matches!(
            run.status,
            AgentRunStatus::Completed | AgentRunStatus::Failed | AgentRunStatus::Cancelled
        ) {
            run.completed_at = Some(run.updated_at);
        }
        self.upsert_agent_run(&run)?;
        Ok(Some(run))
    }

    pub fn delete_agent_run(&self, run_id: Uuid) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute("PRAGMA foreign_keys = ON;", [])
            .map_err(|err| AroError::Memory(err.to_string()))?;
        conn.execute(
            "DELETE FROM agent_steps WHERE run_id = ?1",
            params![run_id.to_string()],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        conn.execute(
            "DELETE FROM agent_artifacts WHERE run_id = ?1",
            params![run_id.to_string()],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        let _ = conn.execute(
            "DELETE FROM agent_context_items WHERE run_id = ?1",
            params![run_id.to_string()],
        );
        conn.execute(
            "DELETE FROM agent_runs WHERE id = ?1",
            params![run_id.to_string()],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn add_agent_step(&self, step: &AgentStep) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO agent_steps
              (id, run_id, sequence, kind, status, title, input_json, output_json,
               error, started_at, finished_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
              status = excluded.status,
              output_json = excluded.output_json,
              error = excluded.error,
              finished_at = excluded.finished_at
            "#,
            params![
                step.id.to_string(),
                step.run_id.to_string(),
                step.sequence,
                serde_json::to_string(&step.kind)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                serde_json::to_string(&step.status)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                step.title,
                serde_json::to_string(&step.input)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                serde_json::to_string(&step.output)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                step.error,
                step.started_at.to_rfc3339(),
                step.finished_at.map(|value| value.to_rfc3339()),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn list_agent_steps(&self, run_id: Uuid) -> AroResult<Vec<AgentStep>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, run_id, sequence, kind, status, title, input_json, output_json,
                       error, started_at, finished_at
                FROM agent_steps
                WHERE run_id = ?1
                ORDER BY sequence ASC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![run_id.to_string()], map_agent_step)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn add_agent_artifact(&self, artifact: &AgentArtifact) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO agent_artifacts
              (id, run_id, kind, title, uri, content, metadata_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              uri = excluded.uri,
              content = excluded.content,
              metadata_json = excluded.metadata_json
            "#,
            params![
                artifact.id.to_string(),
                artifact.run_id.to_string(),
                artifact.kind,
                artifact.title,
                artifact.uri,
                artifact.content,
                serde_json::to_string(&artifact.metadata)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                artifact.created_at.to_rfc3339(),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn list_agent_artifacts(&self, run_id: Uuid) -> AroResult<Vec<AgentArtifact>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, run_id, kind, title, uri, content, metadata_json, created_at
                FROM agent_artifacts
                WHERE run_id = ?1
                ORDER BY created_at ASC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![run_id.to_string()], map_agent_artifact)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn add_agent_context_item(&self, item: &AgentContextItem) -> AroResult<()> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.execute(
            r#"
            INSERT INTO agent_context_items
              (id, run_id, conversation_id, kind, title, content, uri, metadata_json, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
              title = excluded.title,
              content = excluded.content,
              uri = excluded.uri,
              metadata_json = excluded.metadata_json
            "#,
            params![
                item.id.to_string(),
                item.run_id.map(|id| id.to_string()),
                item.conversation_id.map(|id| id.to_string()),
                item.kind,
                item.title,
                item.content,
                item.uri,
                serde_json::to_string(&item.metadata)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                item.created_at.to_rfc3339(),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        tx.execute(
            r#"
            INSERT INTO agent_context_fts (id, title, content)
            VALUES (?1, ?2, ?3)
            "#,
            params![item.id.to_string(), item.title, item.content],
        )
        .ok();
        tx.commit()
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn search_agent_context(
        &self,
        query: &str,
        limit: usize,
    ) -> AroResult<Vec<AgentContextItem>> {
        let conn = self.connect()?;
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = conn
            .prepare(
                r#"
                SELECT c.id, c.run_id, c.conversation_id, c.kind, c.title, c.content,
                       c.uri, c.metadata_json, c.created_at
                FROM agent_context_fts f
                JOIN agent_context_items c ON c.id = f.id
                WHERE agent_context_fts MATCH ?1
                LIMIT ?2
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![trimmed, limit as i64], map_agent_context_item)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn upsert_permission_profile(&self, profile: &PermissionProfile) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO agent_permission_profiles (id, name, profile_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
              name = excluded.name,
              profile_json = excluded.profile_json,
              updated_at = excluded.updated_at
            "#,
            params![
                profile.id.to_string(),
                profile.name,
                serde_json::to_string(profile).map_err(|err| AroError::Memory(err.to_string()))?,
                profile.created_at.to_rfc3339(),
                profile.updated_at.to_rfc3339(),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn create_plan(&self, plan: &Plan) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            INSERT INTO plans (id, conversation_id, title, description, tasks, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                plan.id.to_string(),
                plan.conversation_id.to_string(),
                plan.title,
                plan.description,
                serde_json::to_string(&plan.tasks).map_err(|err| AroError::Memory(err.to_string()))?,
                plan.status,
                plan.created_at,
                plan.updated_at,
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn update_plan(&self, plan: &Plan) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            r#"
            UPDATE plans
            SET title = ?2, description = ?3, tasks = ?4, status = ?5, updated_at = ?6
            WHERE id = ?1
            "#,
            params![
                plan.id.to_string(),
                plan.title,
                plan.description,
                serde_json::to_string(&plan.tasks)
                    .map_err(|err| AroError::Memory(err.to_string()))?,
                plan.status,
                plan.updated_at,
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn delete_plan(&self, id: Uuid) -> AroResult<()> {
        let conn = self.connect()?;
        conn.execute(
            "UPDATE plans SET deleted_at = ?2 WHERE id = ?1",
            params![id.to_string(), Utc::now().to_rfc3339()],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(())
    }

    pub fn list_plans(&self, conversation_id: Uuid) -> AroResult<Vec<Plan>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, title, description, tasks, status, created_at, updated_at
                FROM plans
                WHERE conversation_id = ?1 AND deleted_at IS NULL
                ORDER BY updated_at DESC
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let rows = stmt
            .query_map(params![conversation_id.to_string()], map_plan)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn get_plan(&self, id: Uuid) -> AroResult<Option<Plan>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT id, conversation_id, title, description, tasks, status, created_at, updated_at
                FROM plans
                WHERE id = ?1 AND deleted_at IS NULL
                "#,
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let mut rows = stmt
            .query_map(params![id.to_string()], map_plan)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        if let Some(row) = rows.next() {
            Ok(Some(row.map_err(|err| AroError::Memory(err.to_string()))?))
        } else {
            Ok(None)
        }
    }

    pub fn create_notification(&self, item: &NotificationItem) -> AroResult<NotificationItem> {
        let conn = self.connect()?;
        let metadata_str = item.metadata.as_ref().map(|v| v.to_string());
        conn.execute(
            r#"
            INSERT INTO notifications (
                id, organization_id, user_id, title, body, kind, priority, status,
                source, action_url, metadata, created_at, read_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                item.id,
                item.organization_id.map(|id| id.to_string()),
                item.user_id.map(|id| id.to_string()),
                item.title,
                item.body,
                item.kind.as_str(),
                item.priority.as_str(),
                item.status.as_str(),
                item.source.as_str(),
                item.action_url,
                metadata_str,
                item.created_at.to_rfc3339(),
                item.read_at.map(|d| d.to_rfc3339()),
            ],
        )
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(item.clone())
    }

    pub fn list_notifications(
        &self,
        filter: &NotificationFilter,
    ) -> AroResult<Vec<NotificationItem>> {
        let conn = self.connect()?;
        let mut query = String::from(
            r#"
            SELECT id, organization_id, user_id, title, body, kind, priority, status,
                   source, action_url, metadata, created_at, read_at
            FROM notifications
            WHERE 1=1
            "#,
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(status) = filter.status {
            query.push_str(" AND status = ?");
            params_vec.push(Box::new(status.as_str().to_string()));
        }
        if let Some(kind) = filter.kind {
            query.push_str(" AND kind = ?");
            params_vec.push(Box::new(kind.as_str().to_string()));
        }
        if let Some(source) = filter.source {
            query.push_str(" AND source = ?");
            params_vec.push(Box::new(source.as_str().to_string()));
        }
        if let Some(org_id) = filter.organization_id {
            query.push_str(" AND organization_id = ?");
            params_vec.push(Box::new(org_id.to_string()));
        } else if filter.personal_only == Some(true) {
            query.push_str(" AND organization_id IS NULL");
        }
        if let Some(search) = &filter.search {
            let pattern = format!("%{}%", search.trim());
            query.push_str(" AND (title LIKE ? OR body LIKE ?)");
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {limit}"));
            if let Some(offset) = filter.offset {
                query.push_str(&format!(" OFFSET {offset}"));
            }
        }

        let mut stmt = conn
            .prepare(&query)
            .map_err(|err| AroError::Memory(err.to_string()))?;

        let params_slice: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let rows = stmt
            .query_map(params_slice.as_slice(), map_notification)
            .map_err(|err| AroError::Memory(err.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| AroError::Memory(err.to_string()))
    }

    pub fn get_unread_notification_count(
        &self,
        organization_id: Option<Uuid>,
    ) -> AroResult<u64> {
        let conn = self.connect()?;
        let count: i64 = if let Some(org_id) = organization_id {
            conn.query_row(
                "SELECT COUNT(*) FROM notifications WHERE status = 'unread' AND organization_id = ?1",
                params![org_id.to_string()],
                |row| row.get(0),
            )
        } else {
            conn.query_row(
                "SELECT COUNT(*) FROM notifications WHERE status = 'unread' AND organization_id IS NULL",
                [],
                |row| row.get(0),
            )
        }
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(count.max(0) as u64)
    }

    pub fn mark_notification_as_read(&self, id: &str) -> AroResult<bool> {
        let conn = self.connect()?;
        let now = Utc::now().to_rfc3339();
        let affected = conn
            .execute(
                "UPDATE notifications SET status = 'read', read_at = ?1 WHERE id = ?2",
                params![now, id],
            )
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(affected > 0)
    }

    pub fn mark_all_notifications_as_read(
        &self,
        organization_id: Option<Uuid>,
    ) -> AroResult<u64> {
        let conn = self.connect()?;
        let now = Utc::now().to_rfc3339();
        let affected = if let Some(org_id) = organization_id {
            conn.execute(
                "UPDATE notifications SET status = 'read', read_at = ?1 WHERE status = 'unread' AND organization_id = ?2",
                params![now, org_id.to_string()],
            )
        } else {
            conn.execute(
                "UPDATE notifications SET status = 'read', read_at = ?1 WHERE status = 'unread' AND organization_id IS NULL",
                params![now],
            )
        }
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(affected as u64)
    }

    pub fn delete_notification(&self, id: &str) -> AroResult<bool> {
        let conn = self.connect()?;
        let affected = conn
            .execute("DELETE FROM notifications WHERE id = ?1", params![id])
            .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(affected > 0)
    }

    pub fn clear_all_notifications(
        &self,
        organization_id: Option<Uuid>,
    ) -> AroResult<u64> {
        let conn = self.connect()?;
        let affected = if let Some(org_id) = organization_id {
            conn.execute(
                "DELETE FROM notifications WHERE organization_id = ?1",
                params![org_id.to_string()],
            )
        } else {
            conn.execute("DELETE FROM notifications WHERE organization_id IS NULL", [])
        }
        .map_err(|err| AroError::Memory(err.to_string()))?;
        Ok(affected as u64)
    }
}


fn map_plan(row: &rusqlite::Row<'_>) -> rusqlite::Result<Plan> {
    let id_str: String = row.get(0)?;
    let id = Uuid::parse_str(&id_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let conv_id_str: String = row.get(1)?;
    let conversation_id = Uuid::parse_str(&conv_id_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let title: String = row.get(2)?;
    let description: Option<String> = row.get(3)?;
    let tasks_str: String = row.get(4)?;
    let tasks = serde_json::from_str(&tasks_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let status: String = row.get(5)?;
    let created_at: String = row.get(6)?;
    let updated_at: String = row.get(7)?;

    Ok(Plan {
        id,
        conversation_id,
        title,
        description,
        tasks,
        status,
        created_at,
        updated_at,
    })
}

fn map_notification(row: &rusqlite::Row<'_>) -> rusqlite::Result<NotificationItem> {
    let id: String = row.get(0)?;
    let org_id_str: Option<String> = row.get(1)?;
    let user_id_str: Option<String> = row.get(2)?;
    let title: String = row.get(3)?;
    let body: String = row.get(4)?;
    let kind_str: String = row.get(5)?;
    let priority_str: String = row.get(6)?;
    let status_str: String = row.get(7)?;
    let source_str: String = row.get(8)?;
    let action_url: Option<String> = row.get(9)?;
    let metadata_str: Option<String> = row.get(10)?;
    let created_at = parse_datetime(row.get(11)?)?;
    let read_at_str: Option<String> = row.get(12)?;
    let read_at = read_at_str.and_then(|s| {
        DateTime::parse_from_rfc3339(&s)
            .ok()
            .map(|d| d.with_timezone(&Utc))
    });

    let kind = match kind_str.as_str() {
        "success" => NotificationKind::Success,
        "warning" => NotificationKind::Warning,
        "error" => NotificationKind::Error,
        "agent-completion" | "agent_completion" => NotificationKind::AgentCompletion,
        "routine" => NotificationKind::Routine,
        "security" => NotificationKind::Security,
        _ => NotificationKind::Info,
    };

    let priority = match priority_str.as_str() {
        "low" => NotificationPriority::Low,
        "high" => NotificationPriority::High,
        "urgent" => NotificationPriority::Urgent,
        _ => NotificationPriority::Normal,
    };

    let status = match status_str.as_str() {
        "read" => NotificationStatus::Read,
        "archived" => NotificationStatus::Archived,
        _ => NotificationStatus::Unread,
    };

    let source = match source_str.as_str() {
        "agent" => NotificationSource::Agent,
        "routine" => NotificationSource::Routine,
        "cloud" => NotificationSource::Cloud,
        _ => NotificationSource::System,
    };

    let metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

    Ok(NotificationItem {
        id,
        organization_id: org_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        user_id: user_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        title,
        body,
        kind,
        priority,
        status,
        source,
        action_url,
        metadata,
        created_at,
        read_at,
    })
}


fn parse_datetime(value: String) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })
}

fn map_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    let mode_json: String = row.get(2)?;
    let project_id_str: Option<String> = row.get(5).ok();
    let folder_id_str: Option<String> = row.get(6).ok();
    let root_path: Option<String> = row.get(7).ok();
    Ok(Conversation {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        title: row.get(1)?,
        mode: serde_json::from_str::<AssistantMode>(&mode_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(err))
        })?,
        created_at: parse_datetime(row.get(3)?)?,
        updated_at: parse_datetime(row.get(4)?)?,
        project_id: project_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        folder_id: folder_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        root_path,
    })
}

fn map_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        name: row.get(1)?,
        description: row.get(2)?,
        instructions: row.get(3)?,
        root_path: row.get(4).ok(),
        color: row.get(5)?,
        icon: row.get(6)?,
        created_at: parse_datetime(row.get(7)?)?,
        updated_at: parse_datetime(row.get(8)?)?,
        organization_id: row.get(9).ok(),
    })
}

fn map_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    let proj_id_str: Option<String> = row.get(1)?;
    Ok(Folder {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        project_id: proj_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        name: row.get(2)?,
        root_path: row.get(3).ok(),
        color: row.get(4)?,
        icon: row.get(5)?,
        created_at: parse_datetime(row.get(6)?)?,
        updated_at: parse_datetime(row.get(7)?)?,
        organization_id: row.get(8).ok(),
    })
}

fn map_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChatMessage> {
    let role_json: String = row.get(2)?;
    let run_id_str: Option<String> = row.get(7).ok();
    let agent_run_id = run_id_str.and_then(|s| Uuid::parse_str(&s).ok());
    Ok(ChatMessage {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        conversation_id: Uuid::parse_str(&row.get::<_, String>(1)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
        })?,
        role: serde_json::from_str::<MessageRole>(&role_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(err))
        })?,
        content: row.get(3)?,
        created_at: parse_datetime(row.get(4)?)?,
        token_estimate: row.get(5)?,
        model_id: row.get(6).ok(),
        attachments: Vec::new(),
        agent_run_id,
        steps: None,
    })
}

#[allow(dead_code)]
fn row_to_memory(row: &rusqlite::Row<'_>) -> rusqlite::Result<LongTermMemory> {
    map_memory(row)
}

fn map_memory(row: &rusqlite::Row<'_>) -> rusqlite::Result<LongTermMemory> {
    let id = Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let source_message_ids_json: String = row.get(7)?;
    let source_message_ids =
        serde_json::from_str::<Vec<Uuid>>(&source_message_ids_json).unwrap_or_default();
    let created_at = parse_datetime(row.get(8)?)?;
    let updated_at = row
        .get::<_, Option<String>>(9)?
        .map(parse_datetime)
        .transpose()?
        .unwrap_or(created_at);
    let last_used_at = row
        .get::<_, Option<String>>(12)?
        .map(parse_datetime)
        .transpose()?;
    let recall_count = row
        .get::<_, u32>("recall_count")
        .or_else(|_| row.get::<_, u32>(13))
        .unwrap_or(0);
    Ok(LongTermMemory {
        id,
        client_id: row.get(1)?,
        content: row.get(2)?,
        category: row.get(3)?,
        scope: row.get(4)?,
        status: row.get(5)?,
        source_conversation_id: parse_optional_uuid(row.get(6)?)?,
        source_message_ids,
        pinned: row.get::<_, i64>(10)? != 0,
        salience: row.get::<_, f64>(11)? as f32,
        recall_count,
        last_used_at,
        created_at,
        updated_at,
    })
}

fn memory_to_context_source(memory: LongTermMemory) -> ContextSource {
    let score = if memory.pinned {
        1.0
    } else {
        memory.salience.clamp(0.0, 1.0)
    };
    ContextSource {
        id: format!("memory:{}", memory.id),
        kind: "memory".to_string(),
        title: format!("Memory / {}", memory.category),
        excerpt: compact_excerpt(&memory.content, 500),
        uri: Some(format!("memory://{}", memory.id)),
        score,
        created_at: Some(memory.created_at),
    }
}

fn diversify_memories(memories: Vec<LongTermMemory>, limit: usize) -> Vec<LongTermMemory> {
    let mut selected = Vec::new();
    let mut selected_ids = Vec::new();
    let mut selected_categories = Vec::new();
    let mut remaining = Vec::new();

    for memory in memories {
        if memory.pinned && selected.len() < limit {
            selected_categories.push(memory.category.clone());
            selected_ids.push(memory.id);
            selected.push(memory);
        } else {
            remaining.push(memory);
        }
    }

    for memory in &remaining {
        if selected.len() >= limit {
            break;
        }
        if selected_ids.contains(&memory.id) || selected_categories.contains(&memory.category) {
            continue;
        }
        selected_categories.push(memory.category.clone());
        selected_ids.push(memory.id);
        selected.push(memory.clone());
    }

    for memory in remaining {
        if selected.len() >= limit {
            break;
        }
        if selected_ids.contains(&memory.id) {
            continue;
        }
        selected_ids.push(memory.id);
        selected.push(memory);
    }

    selected
}

fn memory_fts_query(query: &str) -> Option<String> {
    let tokens = query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| token.chars().count() >= 2)
        .take(8)
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" OR "))
    }
}

fn map_episode(row: &rusqlite::Row<'_>) -> rusqlite::Result<Episode> {
    let id_str: String = row.get(0)?;
    let id = Uuid::parse_str(&id_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })?;

    let conv_id_str: String = row.get(1)?;
    let conversation_id = Uuid::parse_str(&conv_id_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
    })?;

    let turn_start_i64: i64 = row.get(2)?;
    let turn_start = turn_start_i64.max(0) as usize;

    let turn_end_i64: i64 = row.get(3)?;
    let turn_end = turn_end_i64.max(0) as usize;

    let summary: String = row.get(4)?;

    let key_decisions_json: String = row.get(5)?;
    let key_decisions: Vec<String> =
        serde_json::from_str(&key_decisions_json).unwrap_or_default();

    let entities_json: String = row.get(6)?;
    let entities: Vec<String> = serde_json::from_str(&entities_json).unwrap_or_default();

    let token_count_i64: i64 = row.get(7)?;
    let token_count = token_count_i64.max(0) as usize;

    let created_at = parse_datetime(row.get(8)?)?;
    let updated_at = row
        .get::<_, Option<String>>(9)?
        .map(parse_datetime)
        .transpose()?
        .unwrap_or(created_at);

    Ok(Episode {
        id,
        conversation_id,
        turn_start,
        turn_end,
        summary,
        key_decisions,
        entities,
        token_count,
        created_at,
        updated_at,
    })
}

fn episode_fts_query(query: &str) -> Option<String> {
    let tokens = query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .take(10)
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" OR "))
    }
}

fn compact_excerpt(content: &str, max_chars: usize) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        format!(
            "{}...",
            normalized
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn parse_optional_uuid(value: Option<String>) -> rusqlite::Result<Option<Uuid>> {
    value
        .map(|value| {
            Uuid::parse_str(&value).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })
        })
        .transpose()
}

fn map_agent_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentRun> {
    let mode_json: String = row.get(4)?;
    let status_json: String = row.get(5)?;
    let priority_json: String = row.get(6)?;
    Ok(AgentRun {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        lane_id: parse_optional_uuid(row.get(1)?)?,
        conversation_id: parse_optional_uuid(row.get(2)?)?,
        goal: row.get(3)?,
        mode: serde_json::from_str::<AssistantMode>(&mode_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
        })?,
        status: serde_json::from_str::<AgentRunStatus>(&status_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(err))
        })?,
        priority: serde_json::from_str::<AgentRunPriority>(&priority_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(err))
        })?,
        model_provider_id: row.get(7)?,
        model_id: row.get(8)?,
        autonomy_profile_id: parse_optional_uuid(row.get(9)?)?,
        checkpoint_summary: row.get(10)?,
        last_error: row.get(11)?,
        created_at: parse_datetime(row.get(12)?)?,
        updated_at: parse_datetime(row.get(13)?)?,
        heartbeat_at: row
            .get::<_, Option<String>>(14)?
            .map(parse_datetime)
            .transpose()?,
        completed_at: row
            .get::<_, Option<String>>(15)?
            .map(parse_datetime)
            .transpose()?,
    })
}

fn map_agent_lane(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentLane> {
    let status_json: String = row.get(3)?;
    let priority_json: String = row.get(4)?;
    Ok(AgentLane {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        conversation_id: parse_optional_uuid(row.get(1)?)?,
        title: row.get(2)?,
        status: serde_json::from_str::<AgentLaneStatus>(&status_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(err))
        })?,
        priority: serde_json::from_str::<AgentRunPriority>(&priority_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
        })?,
        max_concurrent_runs: row.get::<_, i64>(5)?.max(1) as u32,
        created_at: parse_datetime(row.get(6)?)?,
        updated_at: parse_datetime(row.get(7)?)?,
    })
}

fn map_agent_step(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentStep> {
    let kind_json: String = row.get(3)?;
    let status_json: String = row.get(4)?;
    let input_json: String = row.get(6)?;
    let output_json: String = row.get(7)?;
    Ok(AgentStep {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        run_id: Uuid::parse_str(&row.get::<_, String>(1)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
        })?,
        sequence: row.get(2)?,
        kind: serde_json::from_str::<AgentStepKind>(&kind_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(err))
        })?,
        status: serde_json::from_str::<AgentStepStatus>(&status_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
        })?,
        title: row.get(5)?,
        input: serde_json::from_str(&input_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(err))
        })?,
        output: serde_json::from_str(&output_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
        })?,
        error: row.get(8)?,
        started_at: parse_datetime(row.get(9)?)?,
        finished_at: row
            .get::<_, Option<String>>(10)?
            .map(parse_datetime)
            .transpose()?,
    })
}

fn map_agent_artifact(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentArtifact> {
    let metadata_json: String = row.get(6)?;
    Ok(AgentArtifact {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        run_id: Uuid::parse_str(&row.get::<_, String>(1)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
        })?,
        kind: row.get(2)?,
        title: row.get(3)?,
        uri: row.get(4)?,
        content: row.get(5)?,
        metadata: serde_json::from_str(&metadata_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(err))
        })?,
        created_at: parse_datetime(row.get(7)?)?,
    })
}

fn map_agent_context_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentContextItem> {
    let metadata_json: String = row.get(7)?;
    Ok(AgentContextItem {
        id: Uuid::parse_str(&row.get::<_, String>(0)?).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })?,
        run_id: parse_optional_uuid(row.get(1)?)?,
        conversation_id: parse_optional_uuid(row.get(2)?)?,
        kind: row.get(3)?,
        title: row.get(4)?,
        content: row.get(5)?,
        uri: row.get(6)?,
        metadata: serde_json::from_str(&metadata_json).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
        })?,
        created_at: parse_datetime(row.get(8)?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_conversations_and_messages() {
        let path = std::env::temp_dir().join(format!("aro-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");
        let conversation = store
            .create_conversation("Test", AssistantMode::Chat, None, None)
            .expect("conversation");
        let message = ChatMessage::new(conversation.id, MessageRole::User, "hello");
        store.add_message(&message).expect("message");

        assert_eq!(store.list_conversations().expect("list").len(), 1);
        assert_eq!(
            store
                .list_messages(conversation.id)
                .expect("messages")
                .len(),
            1
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stores_searches_and_soft_deletes_long_term_memories() {
        let path = std::env::temp_dir().join(format!("aro-memory-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");
        let mut memory = LongTermMemory::new("ARO utilise Svelte et Vite.", None);
        memory.client_id = Some("client-memory-1".to_string());
        memory.category = "technical".to_string();
        memory.salience = 0.8;

        store.upsert_memory(&memory).expect("upsert memory");

        let listed = store.list_memories().expect("list memories");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].client_id.as_deref(), Some("client-memory-1"));

        let found = store.search_memories("Svelte", 5).expect("search memories");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, memory.id);

        store
            .touch_memories_used(&[memory.id])
            .expect("touch memory");
        let touched = store.list_memories().expect("list touched memories");
        assert!(touched[0].last_used_at.is_some());

        store.delete_memory(memory.id).expect("delete memory");
        assert!(store.list_memories().expect("list after delete").is_empty());
        assert!(store
            .search_memories("Svelte", 5)
            .expect("search after delete")
            .is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_top_k_get_memories_by_ids_and_atomic_recall_count() {
        let path = std::env::temp_dir().join(format!("aro-top-k-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");

        let mut mem1 = LongTermMemory::new("Memory One", None);
        mem1.pinned = false;
        let mut mem2 = LongTermMemory::new("Memory Two (pinned)", None);
        mem2.pinned = true;
        let mut mem3 = LongTermMemory::new("Memory Three", None);
        mem3.pinned = false;

        store.upsert_memory(&mem1).expect("upsert 1");
        store.upsert_memory(&mem2).expect("upsert 2");
        store.upsert_memory(&mem3).expect("upsert 3");

        // Test get_memory
        let fetched1 = store.get_memory(mem1.id).expect("get_memory").expect("exists");
        assert_eq!(fetched1.content, "Memory One");
        assert_eq!(fetched1.recall_count, 0);

        // Test list_pinned_memories
        let pinned = store.list_pinned_memories().expect("pinned");
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].id, mem2.id);

        // Test get_memories_by_ids preserves input rank order
        let ordered = store
            .get_memories_by_ids(&[mem3.id, mem1.id, mem2.id])
            .expect("ordered");
        assert_eq!(ordered.len(), 3);
        assert_eq!(ordered[0].id, mem3.id);
        assert_eq!(ordered[1].id, mem1.id);
        assert_eq!(ordered[2].id, mem2.id);

        // Test touch_memories_used increments recall_count atomically
        store
            .touch_memories_used(&[mem1.id])
            .expect("touch 1st");
        store
            .touch_memories_used(&[mem1.id, mem3.id])
            .expect("touch 2nd");

        let updated1 = store.get_memory(mem1.id).expect("get").unwrap();
        let updated2 = store.get_memory(mem2.id).expect("get").unwrap();
        let updated3 = store.get_memory(mem3.id).expect("get").unwrap();

        assert_eq!(updated1.recall_count, 2);
        assert_eq!(updated2.recall_count, 0);
        assert_eq!(updated3.recall_count, 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_context_sources_filter_archived_and_sensitive_memories() {
        let path =
            std::env::temp_dir().join(format!("aro-memory-context-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");

        let mut pinned = LongTermMemory::new("Toujours repondre en francais.", None);
        pinned.category = "preference".to_string();
        pinned.pinned = true;
        store.upsert_memory(&pinned).expect("upsert pinned");

        let mut relevant = LongTermMemory::new("Le projet utilise Ollama en local.", None);
        relevant.category = "technical".to_string();
        relevant.salience = 0.9;
        store.upsert_memory(&relevant).expect("upsert relevant");

        let mut archived = LongTermMemory::new("Ollama archived memory should stay hidden.", None);
        archived.status = aro_core::MEMORY_STATUS_ARCHIVED.to_string();
        store.upsert_memory(&archived).expect("upsert archived");

        let sensitive = LongTermMemory::new("Ollama password is super-secret.", None);
        store.upsert_memory(&sensitive).expect("upsert sensitive");

        let sources = store
            .memory_context_sources("Ollama", 4)
            .expect("memory context sources");
        let uris = sources
            .iter()
            .filter_map(|source| source.uri.clone())
            .collect::<Vec<_>>();

        assert!(uris.contains(&format!("memory://{}", pinned.id)));
        assert!(uris.contains(&format!("memory://{}", relevant.id)));
        assert!(!uris.contains(&format!("memory://{}", archived.id)));
        assert!(!uris.contains(&format!("memory://{}", sensitive.id)));
        assert!(sources.iter().all(|source| source.kind == "memory"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stores_agent_run_related_records() {
        let path = std::env::temp_dir().join(format!("aro-agent-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");
        let lane = store
            .ensure_agent_lane(None, "Agent test lane")
            .expect("lane");
        let mut run = AgentRun::new(
            "Ship multi-provider models",
            AssistantMode::Chat,
            None,
            Some("mock-local".to_string()),
            Some("gemma3:1b".to_string()),
            None,
        );
        run.lane_id = Some(lane.id);
        run.priority = AgentRunPriority::High;
        store.upsert_agent_run(&run).expect("run");

        let step = AgentStep::completed(
            run.id,
            1,
            AgentStepKind::Model,
            "Draft response",
            serde_json::json!({ "prompt": "hello" }),
            serde_json::json!({ "ok": true }),
        );
        store.add_agent_step(&step).expect("step");

        let artifact = AgentArtifact {
            id: Uuid::new_v4(),
            run_id: run.id,
            kind: "markdown".to_string(),
            title: "Plan".to_string(),
            uri: None,
            content: Some("needle artifact".to_string()),
            metadata: serde_json::json!({ "source": "test" }),
            created_at: Utc::now(),
        };
        store.add_agent_artifact(&artifact).expect("artifact");

        let context = AgentContextItem {
            id: Uuid::new_v4(),
            run_id: Some(run.id),
            conversation_id: None,
            kind: "note".to_string(),
            title: "Local-first note".to_string(),
            content: "needle context".to_string(),
            uri: None,
            metadata: serde_json::json!({ "rank": 1 }),
            created_at: Utc::now(),
        };
        store.add_agent_context_item(&context).expect("context");

        assert_eq!(store.list_agent_runs().expect("runs").len(), 1);
        let lane_views = store.list_agent_lane_views().expect("lanes");
        assert_eq!(lane_views.len(), 1);
        assert_eq!(
            lane_views[0].latest_runs[0].priority,
            AgentRunPriority::High
        );
        assert_eq!(store.list_agent_steps(run.id).expect("steps").len(), 1);
        assert_eq!(
            store.list_agent_artifacts(run.id).expect("artifacts").len(),
            1
        );
        assert_eq!(
            store
                .search_agent_context("needle", 10)
                .expect("context")
                .len(),
            1
        );

        store.delete_agent_run(run.id).expect("delete run");
        assert_eq!(store.list_agent_runs().expect("runs").len(), 0);
        assert_eq!(store.list_agent_steps(run.id).expect("steps").len(), 0);
        assert_eq!(store.list_agent_artifacts(run.id).expect("artifacts").len(), 0);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrates_agent_runs_created_before_lanes() {
        let path =
            std::env::temp_dir().join(format!("aro-old-agent-test-{}.sqlite", Uuid::new_v4()));
        let conn = Connection::open(&path).expect("open old db");
        let now = Utc::now().to_rfc3339();
        conn.execute_batch(
            r#"
            CREATE TABLE agent_runs (
              id TEXT PRIMARY KEY,
              conversation_id TEXT,
              goal TEXT NOT NULL,
              mode TEXT NOT NULL,
              status TEXT NOT NULL,
              model_provider_id TEXT,
              model_id TEXT,
              autonomy_profile_id TEXT,
              checkpoint_summary TEXT,
              last_error TEXT,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL,
              heartbeat_at TEXT,
              completed_at TEXT,
              sync_status TEXT NOT NULL DEFAULT 'local'
            );
            "#,
        )
        .expect("old schema");
        let run_id = Uuid::new_v4();
        conn.execute(
            r#"
            INSERT INTO agent_runs
              (id, goal, mode, status, created_at, updated_at, sync_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'local')
            "#,
            params![
                run_id.to_string(),
                "Old run",
                serde_json::to_string(&AssistantMode::Chat).expect("mode"),
                serde_json::to_string(&AgentRunStatus::Running).expect("status"),
                now,
                now,
            ],
        )
        .expect("old run");
        drop(conn);

        let store = SqliteMemoryStore::new(&path).expect("migrate old db");
        let runs = store.list_agent_runs().expect("runs after migration");

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run_id);
        assert_eq!(runs[0].lane_id, None);
        assert_eq!(runs[0].priority, AgentRunPriority::Normal);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stores_projects_folders_and_moves_conversations() {
        let path =
            std::env::temp_dir().join(format!("aro-projects-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");

        let project = Project {
            id: Uuid::new_v4(),
            name: "Test Project".to_string(),
            description: Some("Project description".to_string()),
            instructions: Some("Project instructions".to_string()),
            root_path: None,
            color: "#3b82f6".to_string(),
            icon: "folder-tree".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            organization_id: Some("org-alpha".to_string()),
        };
        store.save_project(&project).expect("save project");
        let projects = store.list_projects().expect("list projects");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "Test Project");
        assert_eq!(projects[0].organization_id.as_deref(), Some("org-alpha"));

        let folder = Folder {
            id: Uuid::new_v4(),
            project_id: Some(project.id),
            name: "Subfolder".to_string(),
            root_path: None,
            color: Some("#10b981".to_string()),
            icon: "folder".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            organization_id: Some("org-alpha".to_string()),
        };
        store.save_folder(&folder).expect("save folder");
        let folders = store.list_folders().expect("list folders");
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].name, "Subfolder");
        assert_eq!(folders[0].organization_id.as_deref(), Some("org-alpha"));

        let conv = store
            .create_conversation("Project Chat", AssistantMode::Chat, None, None)
            .expect("create conv");
        let moved = store
            .move_conversation(conv.id, Some(project.id), Some(folder.id))
            .expect("move conv");
        assert_eq!(moved.project_id, Some(project.id));
        assert_eq!(moved.folder_id, Some(folder.id));

        let _ = std::fs::remove_file(path);
    }

    fn test_project(name: &str, root: Option<&str>) -> Project {
        Project {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: None,
            instructions: None,
            root_path: root.map(|s| s.to_string()),
            color: "#3b82f6".to_string(),
            icon: "folder-tree".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            organization_id: None,
        }
    }

    #[test]
    fn upsert_never_wipes_placement_but_move_can_unassign() {
        let path = std::env::temp_dir().join(format!("aro-test-keep-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");
        let project = test_project("Keep Project", None);
        store.save_project(&project).expect("save project");
        let project_id = project.id;
        // Simulation sync cloud : upsert d'une version sans placement.
        let mut placed = Conversation::new("Keep me", AssistantMode::Chat);
        placed.project_id = Some(project_id);
        store.upsert_conversation(&placed).expect("upsert placed");
        let mut bare = placed.clone();
        bare.project_id = None;
        bare.folder_id = None;
        bare.root_path = None;
        store.upsert_conversation(&bare).expect("upsert bare");
        let kept = store
            .get_conversation(placed.id)
            .expect("get")
            .expect("exists");
        assert_eq!(kept.project_id, Some(project_id));

        // Déclassement explicite via move : lui seul peut mettre None.
        let unassigned = store
            .move_conversation(placed.id, None, None)
            .expect("move");
        assert_eq!(unassigned.project_id, None);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn deduplicates_identical_projects_and_remaps_conversations() {
        let path = std::env::temp_dir().join(format!("aro-test-dedup-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");
        let keeper = test_project("Controlix", Some("C:\\work"));
        let dup = test_project("controlix", Some("C:\\work"));
        // Un homonyme volontairement différent ne doit pas fusionner.
        let other = test_project("Controlix", Some("D:\\ailleurs"));
        store.save_project(&keeper).expect("save keeper");
        store.save_project(&dup).expect("save dup");
        store.save_project(&other).expect("save other");
        let conv = store
            .create_conversation("Hi", AssistantMode::Chat, Some(dup.id), None)
            .expect("create conv");

        let removed = store.deduplicate_projects().expect("dedup");
        // Le doublon qui porte la conversation est gardé (priorité au contenu).
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], keeper.id);
        let projects = store.list_projects().expect("list");
        assert_eq!(projects.len(), 2);
        assert!(projects.iter().any(|p| p.id == dup.id));
        assert!(projects.iter().any(|p| p.id == other.id));
        let moved = store
            .get_conversation(conv.id)
            .expect("get")
            .expect("exists");
        assert_eq!(moved.project_id, Some(dup.id));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn creates_conversation_directly_with_destination() {
        let path = std::env::temp_dir().join(format!("aro-test-dest-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");
        let project = Project {
            id: Uuid::new_v4(),
            name: "Dest Project".to_string(),
            description: None,
            instructions: None,
            root_path: Some("C:\\dest".to_string()),
            color: "#fff".to_string(),
            icon: "folder".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            organization_id: None,
        };
        store.save_project(&project).expect("save project");
        let folder = Folder {
            id: Uuid::new_v4(),
            project_id: Some(project.id),
            name: "Dest Folder".to_string(),
            root_path: None,
            color: None,
            icon: "folder".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            organization_id: None,
        };
        store.save_folder(&folder).expect("save folder");
        let conv = store
            .create_conversation(
                "Placed Chat",
                AssistantMode::Chat,
                Some(project.id),
                Some(folder.id),
            )
            .expect("create conv");
        assert_eq!(conv.project_id, Some(project.id));
        assert_eq!(conv.folder_id, Some(folder.id));
        let listed = store.list_conversations().expect("list");
        assert_eq!(listed[0].project_id, Some(project.id));
        assert_eq!(listed[0].folder_id, Some(folder.id));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn verifies_sqlite_wal_mode_and_pragma_concurrency() {
        let path = std::env::temp_dir().join(format!("aro-wal-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");
        let conn = store.connect().expect("connect");
        let mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .expect("journal mode");
        assert_eq!(mode.to_lowercase(), "wal");

        let sync_mode: i64 = conn
            .query_row("PRAGMA synchronous;", [], |row| row.get(0))
            .expect("synchronous");
        assert_eq!(sync_mode, 1); // 1 = NORMAL in SQLite

        let busy_timeout: i64 = conn
            .query_row("PRAGMA busy_timeout;", [], |row| row.get(0))
            .expect("busy_timeout");
        assert_eq!(busy_timeout, 5000);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stores_and_orders_episodes() {
        let path = std::env::temp_dir().join(format!("aro-episode-order-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");

        let conv = store
            .create_conversation("Episodic Test", AssistantMode::Chat, None, None)
            .expect("conversation");

        // Create 3 episodes out of order
        let ep2 = Episode::new(
            conv.id,
            11,
            20,
            "Middle episode covering turn 11 to 20",
            vec!["Use PostgreSQL for cloud sync".to_string()],
            vec!["PostgreSQL".to_string()],
            1500,
        );

        let ep1 = Episode::new(
            conv.id,
            1,
            10,
            "First episode covering turn 1 to 10",
            vec!["Setup project layout".to_string()],
            vec!["Cargo.toml".to_string()],
            1200,
        );

        let ep3 = Episode::new(
            conv.id,
            21,
            30,
            "Latest episode covering turn 21 to 30",
            vec!["Ship cognitive memory".to_string()],
            vec!["SqliteMemoryStore".to_string()],
            1800,
        );

        // Store in jumbled order: ep2, ep1, ep3
        store.store_episode(&ep2).expect("store ep2");
        store.store_episode(&ep1).expect("store ep1");
        store.store_episode(&ep3).expect("store ep3");

        // list_episodes must return all 3 strictly sorted by turn_start ASC
        let listed = store.list_episodes(conv.id).expect("list");
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].turn_start, 1);
        assert_eq!(listed[0].turn_end, 10);
        assert_eq!(listed[0].summary, "First episode covering turn 1 to 10");
        assert_eq!(listed[0].key_decisions, vec!["Setup project layout".to_string()]);
        assert_eq!(listed[0].entities, vec!["Cargo.toml".to_string()]);

        assert_eq!(listed[1].turn_start, 11);
        assert_eq!(listed[1].turn_end, 20);

        assert_eq!(listed[2].turn_start, 21);
        assert_eq!(listed[2].turn_end, 30);

        // get_latest_episode must return ep3
        let latest = store.get_latest_episode(conv.id).expect("latest");
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.id, ep3.id);
        assert_eq!(latest.turn_start, 21);
        assert_eq!(latest.turn_end, 30);
        assert_eq!(latest.summary, "Latest episode covering turn 21 to 30");

        // get_latest_episode for non-existent conversation returns None
        let non_existent = store.get_latest_episode(Uuid::new_v4()).expect("non existent");
        assert!(non_existent.is_none());

        // Update episode 1 and verify update idempotency
        let mut updated_ep1 = ep1.clone();
        updated_ep1.summary = "Updated first episode summary".to_string();
        updated_ep1.token_count = 1250;
        store.store_episode(&updated_ep1).expect("update ep1");

        let listed_after_update = store.list_episodes(conv.id).expect("list after update");
        assert_eq!(listed_after_update.len(), 3, "Upsert must not duplicate records");
        assert_eq!(listed_after_update[0].summary, "Updated first episode summary");
        assert_eq!(listed_after_update[0].token_count, 1250);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn searches_episodes_via_fts5_across_all_fields() {
        let path = std::env::temp_dir().join(format!("aro-episode-fts-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");

        let conv_a = store
            .create_conversation("Conversation A", AssistantMode::Chat, None, None)
            .expect("conv A");
        let conv_b = store
            .create_conversation("Conversation B", AssistantMode::Chat, None, None)
            .expect("conv B");

        // Episode 1: keyword in summary
        let ep1 = Episode::new(
            conv_a.id,
            1,
            10,
            "Architectural review of WebSocket streaming protocols",
            vec!["Decided against gRPC".to_string()],
            vec!["Tauri".to_string()],
            1000,
        );
        store.store_episode(&ep1).expect("store ep1");

        // Episode 2: keyword in key_decisions
        let ep2 = Episode::new(
            conv_a.id,
            11,
            20,
            "Configured persistence layer",
            vec!["Enable WAL mode in SQLite for concurrency".to_string()],
            vec!["Database".to_string()],
            1100,
        );
        store.store_episode(&ep2).expect("store ep2");

        // Episode 3: keyword in entities
        let ep3 = Episode::new(
            conv_a.id,
            21,
            30,
            "Security hardening audit",
            vec!["Rotate API secrets regularly".to_string()],
            vec!["KeyringSecretStore".to_string(), "Argon2".to_string()],
            1200,
        );
        store.store_episode(&ep3).expect("store ep3");

        // Episode in conversation B with same keyword to test conversation isolation
        let ep_b = Episode::new(
            conv_b.id,
            1,
            10,
            "Other conversation discussing WebSocket",
            vec![],
            vec![],
            500,
        );
        store.store_episode(&ep_b).expect("store ep_b");

        // 1. Search by summary keyword
        let found_summary = store.search_episodes(conv_a.id, "WebSocket", 5).expect("search summary");
        assert_eq!(found_summary.len(), 1);
        assert_eq!(found_summary[0].id, ep1.id);

        // 2. Search by key_decisions keyword
        let found_decision = store.search_episodes(conv_a.id, "concurrency", 5).expect("search decision");
        assert_eq!(found_decision.len(), 1);
        assert_eq!(found_decision[0].id, ep2.id);

        // 3. Search by entities keyword
        let found_entity = store.search_episodes(conv_a.id, "KeyringSecretStore", 5).expect("search entity");
        assert_eq!(found_entity.len(), 1);
        assert_eq!(found_entity[0].id, ep3.id);

        // 4. Conversation isolation: searching in conv_b returns only conv_b's episode
        let found_b = store.search_episodes(conv_b.id, "WebSocket", 5).expect("search b");
        assert_eq!(found_b.len(), 1);
        assert_eq!(found_b[0].id, ep_b.id);

        // 5. Empty query returns empty Vec
        let empty_search = store.search_episodes(conv_a.id, "", 5).expect("empty search");
        assert!(empty_search.is_empty());

        // 6. Limit enforcement
        let ep4 = Episode::new(
            conv_a.id,
            31,
            40,
            "Further WebSocket optimizations",
            vec![],
            vec![],
            800,
        );
        store.store_episode(&ep4).expect("store ep4");
        let limited = store.search_episodes(conv_a.id, "WebSocket", 1).expect("limit 1");
        assert_eq!(limited.len(), 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn transaction_rolls_back_on_failure() {
        let path = std::env::temp_dir().join(format!("aro-episode-rollback-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");

        let conv = store
            .create_conversation("Rollback Test", AssistantMode::Chat, None, None)
            .expect("conv");

        // 1. Foreign key constraint rollback test
        let non_existent_conv = Uuid::new_v4();
        let invalid_ep = Episode::new(
            non_existent_conv,
            1,
            10,
            "Orphan episode",
            vec![],
            vec![],
            100,
        );
        let result = store.store_episode(&invalid_ep);
        assert!(result.is_err(), "Store episode with invalid foreign key must fail");

        // Verify nothing was stored in episodes table
        let listed = store.list_episodes(non_existent_conv).expect("list");
        assert!(listed.is_empty());

        // 2. Simulated mid-transaction failure rollback test
        let mut conn = store.connect().expect("connect");
        let ep_id = Uuid::new_v4();
        {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("tx");
            tx.execute(
                r#"
                INSERT INTO episodes (id, conversation_id, turn_start, turn_end, summary, key_decisions, entities, token_count, created_at, updated_at)
                VALUES (?1, ?2, 1, 10, 'Mid-tx episode', '[]', '[]', 100, ?3, ?3)
                "#,
                params![ep_id.to_string(), conv.id.to_string(), Utc::now().to_rfc3339()],
            )
            .expect("insert inside tx");

            // Deliberate syntax/constraint error before commit
            let failed_exec = tx.execute("INSERT INTO episodes (invalid_column) VALUES (123)", []);
            assert!(failed_exec.is_err());
            // Transaction dropped without commit -> automatic rollback!
        }

        // Verify row was rolled back and is NOT present in the database
        let listed_conv = store.list_episodes(conv.id).expect("list conv");
        assert!(
            listed_conv.iter().all(|e| e.id != ep_id),
            "Rolled-back episode must not exist in database"
        );

        // Verify episodes_fts has no ghost entry
        let search_res = store.search_episodes(conv.id, "Mid-tx", 5).expect("search");
        assert!(search_res.is_empty(), "FTS5 table must not contain rolled-back data");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn wal_concurrency_multiple_readers() {
        let path = std::env::temp_dir().join(format!("aro-wal-concurrency-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&path).expect("store");

        let conv = store
            .create_conversation("WAL Concurrency Test", AssistantMode::Chat, None, None)
            .expect("conv");

        // Populate initial seed episode
        let init_ep = Episode::new(
            conv.id,
            1,
            10,
            "Initial seed episode for concurrency",
            vec![],
            vec![],
            100,
        );
        store.store_episode(&init_ep).expect("seed episode");

        // Spawn 4 concurrent reader threads that repeatedly query while a writer thread stores new episodes
        let num_readers = 4;
        let iterations = 25;
        let mut handles = Vec::new();

        // Writer thread: writes episodes sequentially
        let store_writer = store.clone();
        let conv_id = conv.id;
        let writer_handle = std::thread::spawn(move || {
            for i in 1..=iterations {
                let start = (i * 10) + 1;
                let end = start + 9;
                let ep = Episode::new(
                    conv_id,
                    start,
                    end,
                    format!("Episode batch {} discussing performance and concurrency", i),
                    vec![format!("Decision_{}", i)],
                    vec![format!("Entity_{}", i)],
                    150,
                );
                store_writer.store_episode(&ep).expect("writer store");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        });

        // Reader threads: concurrently list and search episodes
        for reader_idx in 0..num_readers {
            let store_reader = store.clone();
            let conv_id = conv.id;
            let handle = std::thread::spawn(move || {
                for _ in 0..iterations {
                    let episodes = store_reader.list_episodes(conv_id).expect("reader list");
                    assert!(!episodes.is_empty());

                    let _ = store_reader
                        .search_episodes(conv_id, "concurrency", 10)
                        .expect("reader search");

                    let latest = store_reader
                        .get_latest_episode(conv_id)
                        .expect("reader latest");
                    assert!(latest.is_some());

                    std::thread::sleep(std::time::Duration::from_millis(3));
                }
                reader_idx
            });
            handles.push(handle);
        }

        writer_handle.join().expect("writer finished");
        for handle in handles {
            handle.join().expect("reader finished");
        }

        // Verify final state after all concurrent writes and reads
        let final_episodes = store.list_episodes(conv.id).expect("final list");
        assert_eq!(final_episodes.len(), iterations + 1);

        // Verify latest episode has the highest turn
        let latest = store.get_latest_episode(conv.id).expect("final latest").unwrap();
        assert_eq!(latest.turn_start, (iterations * 10) + 1);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_notification_sqlite_lifecycle() {
        let db_path = std::env::temp_dir().join(format!("aro-notif-test-{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&db_path).expect("create store");


        let org_id = Uuid::new_v4();

        // 1. Initially empty
        let initial_count = store
            .get_unread_notification_count(Some(org_id))
            .expect("count");
        assert_eq!(initial_count, 0);

        // 2. Create 2 notifications
        let mut notif1 = NotificationItem::new(
            "Agent Finished",
            "Data analysis completed",
            NotificationKind::AgentCompletion,
            NotificationSource::Agent,
        );
        notif1.organization_id = Some(org_id);
        store.create_notification(&notif1).expect("create 1");

        let mut notif2 = NotificationItem::new(
            "Routine Triggered",
            "Morning health check passed",
            NotificationKind::Routine,
            NotificationSource::Routine,
        );
        notif2.organization_id = Some(org_id);
        store.create_notification(&notif2).expect("create 2");

        // 3. Count unread
        let unread = store
            .get_unread_notification_count(Some(org_id))
            .expect("unread count");
        assert_eq!(unread, 2);

        // 4. List with filter
        let list_all = store
            .list_notifications(&NotificationFilter {
                organization_id: Some(org_id),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(list_all.len(), 2);

        let list_agent = store
            .list_notifications(&NotificationFilter {
                kind: Some(NotificationKind::AgentCompletion),
                organization_id: Some(org_id),
                ..Default::default()
            })
            .expect("list agent");
        assert_eq!(list_agent.len(), 1);
        assert_eq!(list_agent[0].title, "Agent Finished");

        // 5. Mark 1 as read
        assert!(store.mark_notification_as_read(&notif1.id).expect("mark read"));
        let unread_after = store
            .get_unread_notification_count(Some(org_id))
            .expect("count after 1 read");
        assert_eq!(unread_after, 1);

        // 6. Mark all as read
        let marked = store
            .mark_all_notifications_as_read(Some(org_id))
            .expect("mark all read");
        assert_eq!(marked, 1);
        assert_eq!(
            store
                .get_unread_notification_count(Some(org_id))
                .expect("count"),
            0
        );

        // 7. Clear all
        let cleared = store
            .clear_all_notifications(Some(org_id))
            .expect("clear all");
        assert_eq!(cleared, 2);

        let list_empty = store
            .list_notifications(&NotificationFilter {
                organization_id: Some(org_id),
                ..Default::default()
            })
            .expect("list empty");
        assert_eq!(list_empty.len(), 0);
        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_notification_personal_vs_org_scoping() {
        let db_path = std::env::temp_dir().join(format!("test_notif_scoping_{}.sqlite", Uuid::new_v4()));
        let store = SqliteMemoryStore::new(&db_path).expect("store init");
        let org_id = Uuid::new_v4();

        // 1. Personal notification (organization_id is None)
        let mut personal_notif = NotificationItem::new(
            "Personal Task Done",
            "Local indexing complete",
            NotificationKind::Success,
            NotificationSource::System,
        );
        personal_notif.organization_id = None;
        store.create_notification(&personal_notif).expect("create personal");

        // 2. Org notification (organization_id is Some)
        let mut org_notif = NotificationItem::new(
            "Team Sprint Review",
            "Sprint plan updated",
            NotificationKind::Info,
            NotificationSource::Agent,
        );
        org_notif.organization_id = Some(org_id);
        store.create_notification(&org_notif).expect("create org");

        // 3. Counts must be strictly isolated
        let personal_count = store.get_unread_notification_count(None).expect("personal unread");
        let org_count = store.get_unread_notification_count(Some(org_id)).expect("org unread");
        assert_eq!(personal_count, 1);
        assert_eq!(org_count, 1);

        // 4. Listing with personal_only must only return personal notification
        let personal_list = store
            .list_notifications(&NotificationFilter {
                personal_only: Some(true),
                ..Default::default()
            })
            .expect("list personal");
        assert_eq!(personal_list.len(), 1);
        assert_eq!(personal_list[0].id, personal_notif.id);

        // 5. Clearing personal must not delete organization notification
        let cleared_personal = store.clear_all_notifications(None).expect("clear personal");
        assert_eq!(cleared_personal, 1);
        assert_eq!(store.get_unread_notification_count(None).expect("personal empty"), 0);
        assert_eq!(store.get_unread_notification_count(Some(org_id)).expect("org still intact"), 1);

        let _ = std::fs::remove_file(db_path);
    }
}



