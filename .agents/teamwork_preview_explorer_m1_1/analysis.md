# Architecture Exploration & Implementation Strategy: Cognitive Memory Persistence Schema (Feature 1)

**Working Directory**: `crates/aro-memory`, `crates/aro-core`  
**Milestone**: Milestone 1 (Feature 1)  
**Author**: `teamwork_preview_explorer_m1_1`  
**Date**: 2026-09-24  

---

## 1. Executive Summary

Feature 1 of Milestone 1 establishes the foundational persistence layer for ARO's multi-agent cognitive architecture. In complex collaborative workflows, autonomous sub-agents need persistent working memory—including scratchpads, empirical findings, inter-agent message ledgers, and artifact references—that survive process restarts, UI navigation, and session handoffs.

### Key Discoveries:
1. **Core Models Already Well-Formed**: In `crates/aro-core/src/agent.rs` (lines 506–732), the Rust models (`AgentMemoryContext`, `AgentMemoryFinding`, `AgentMessageEnvelope`, `AgentParticipant`, `AgentMessagePayload`, `AgentArtifactRef`, `AgentMessageType`, `AgentParticipantKind`) are already implemented and have 100% test coverage in `crates/aro-core/tests/cognitive_memory_contract_tests.rs`. Their JSON serialization directly matches the TypeScript contracts in `packages/contracts/src/agent.ts`.
2. **Absence in SQLite Layer**: In `crates/aro-memory/src/lib.rs` (3,924 lines), `SqliteMemoryStore` defines 14 core tables (`projects`, `folders`, `conversations`, `messages`, `memories`, `agent_runs`, `agent_lanes`, `agent_steps`, `agent_artifacts`, `agent_context_items`, `agent_permission_profiles`, `plans`, `notifications`, `episodes`), but **none** of the three multi-agent cognitive memory tables (`agent_memories`, `agent_findings`, `agent_message_envelopes`) exist yet.
3. **Current Desktop State**: `apps/desktop/src/lib/agent-protocol.ts` currently relies on browser `localStorage` as a volatile mock, which causes state loss on restarts. The desktop IPC bindings (`agent_get_memory`, `agent_save_memory`, `agent_dispatch_directive`) and E2E suites (`tests/e2e/tier1-feature-coverage.test.ts` tests T1.4.1–T1.4.5) expect a persistent SQLite-backed cognitive memory engine.
4. **Target Implementation**: We specify the exact SQL DDL, indexes, and Rust CRUD methods to add to `crates/aro-memory` to achieve complete persistence, concurrency safety (`BEGIN IMMEDIATE`), and alignment with `SqliteMemoryStore`.

---

## 2. Examination of `crates/aro-core` Domain Models

In `crates/aro-core/src/agent.rs`, the following models define the cognitive memory contract:

### 2.1 `AgentMemoryFinding`
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemoryFinding {
    pub id: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>, // "fact", "constraint", "decision", "discovery", "insight", "vulnerability"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_tool: Option<String>,
    pub timestamp: DateTime<Utc>,
}
```

### 2.2 `AgentMessageEnvelope`
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageEnvelope {
    pub id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub sender: AgentParticipant,
    pub recipient: AgentParticipant,
    pub message_type: AgentMessageType,
    pub payload: AgentMessagePayload,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<AgentRunPriority>,
    pub timestamp: DateTime<Utc>,
}
```

### 2.3 `AgentMemoryContext`
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentMemoryContext {
    pub agent_id: String,
    pub agent_name: String,
    pub role: String,
    pub conversation_id: String,
    #[serde(default)]
    pub scratchpad: String,
    #[serde(default)]
    pub findings: Vec<AgentMemoryFinding>,
    #[serde(default)]
    pub ledger: Vec<AgentMessageEnvelope>,
    #[serde(default)]
    pub artifacts: Vec<AgentArtifactRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile_id: Option<String>,
    pub updated_at: DateTime<Utc>,
}
```

### 2.4 Supporting Types
- `AgentMessageType`: `TaskDelegation`, `TaskProgress`, `TaskResult`, `ClarificationRequest`, `ClarificationResponse`, `PeerCollaboration`, `ContextQuery`, `ContextShare`, `ErrorEscalation` (serialized as `snake_case`).
- `AgentParticipant`: `{ id, name, role, icon, type: AgentParticipantKind }` where `kind` is `Orchestrator`, `Subagent`, `User`, `Tool`, `Broadcast`.
- `AgentArtifactRef`: `{ id, title, kind, uri }`.
- `AgentMessagePayload`: `{ content, structured_data, artifacts, suggested_actions }`.

All of these are re-exported by `aro_core::*` via `crates/aro-core/src/lib.rs`.

---

## 3. Examination of `crates/aro-memory` Architecture

`crates/aro-memory/src/lib.rs` implements `SqliteMemoryStore`. Key architectural invariants:
1. **Connection Pragma**:
   ```sql
   PRAGMA busy_timeout = 5000;
   PRAGMA journal_mode = WAL;
   PRAGMA synchronous = NORMAL;
   PRAGMA foreign_keys = ON;
   ```
2. **Transaction Mode**: Write transactions that mutate multiple tables or require strict lock ordering must use `TransactionBehavior::Immediate` (i.e., `conn.transaction_with_behavior(TransactionBehavior::Immediate)`). Using default `BEGIN DEFERRED` causes writer lock contention under high concurrency, as caught by `m1_adversarial_stress_tests.rs`.
3. **Migration Pattern**: The `migrate(&self)` method in `crates/aro-memory/src/lib.rs` executes `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` in an idempotent batch, followed by non-destructive `ALTER TABLE` statements and index additions.
4. **Foreign Key Policy for Conversations**:
   - `episodes` and `messages` have `REFERENCES conversations(id) ON DELETE CASCADE`.
   - In contrast, `agent_runs`, `agent_lanes`, and `plans` use `conversation_id TEXT` **without** a foreign key constraint. This allows testing with virtual or mock IDs (`conv-104`, `conv-1`) and avoids cascade conflicts during detached sub-agent tasks.
   - For cognitive memory (`agent_memories`, `agent_findings`, `agent_message_envelopes`), `conversation_id TEXT NOT NULL` without a strict foreign key to `conversations(id)` is recommended, coupled with explicit cascading in `delete_conversation(&self, conversation_id: Uuid)` and `reset(&self)`.

---

## 4. Target SQLite Persistence Schema

To fully satisfy Feature 1, three dedicated tables and their associated indexes must be added to `SqliteMemoryStore::migrate(&self)`.

### 4.1 Table 1: `agent_memories`
Stores the agent's core working memory context per conversation.

```sql
CREATE TABLE IF NOT EXISTS agent_memories (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  agent_name TEXT NOT NULL,
  role TEXT NOT NULL,
  scratchpad TEXT NOT NULL DEFAULT '',
  artifacts_json TEXT NOT NULL DEFAULT '[]',
  permission_profile_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(conversation_id, agent_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_memories_conv_agent
  ON agent_memories(conversation_id, agent_id);

CREATE INDEX IF NOT EXISTS idx_agent_memories_conversation
  ON agent_memories(conversation_id);

CREATE INDEX IF NOT EXISTS idx_agent_memories_updated
  ON agent_memories(updated_at DESC);
```

### 4.2 Table 2: `agent_findings`
Stores individual empirical findings, discoveries, decisions, and constraints recorded by agents during execution.

```sql
CREATE TABLE IF NOT EXISTS agent_findings (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  summary TEXT NOT NULL,
  category TEXT,
  source_tool TEXT,
  timestamp TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_findings_conv_agent_time
  ON agent_findings(conversation_id, agent_id, timestamp ASC);

CREATE INDEX IF NOT EXISTS idx_agent_findings_conversation
  ON agent_findings(conversation_id);

CREATE INDEX IF NOT EXISTS idx_agent_findings_category
  ON agent_findings(category)
  WHERE category IS NOT NULL;
```

### 4.3 Table 3: `agent_message_envelopes`
Stores the inter-agent message ledger (delegations, status updates, clarifications, peer collaborations).

```sql
CREATE TABLE IF NOT EXISTS agent_message_envelopes (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  parent_message_id TEXT,
  correlation_id TEXT,
  sender_id TEXT NOT NULL,
  sender_name TEXT NOT NULL,
  sender_role TEXT,
  sender_kind TEXT NOT NULL,
  sender_json TEXT NOT NULL,
  recipient_id TEXT NOT NULL,
  recipient_name TEXT NOT NULL,
  recipient_role TEXT,
  recipient_kind TEXT NOT NULL,
  recipient_json TEXT NOT NULL,
  message_type TEXT NOT NULL,
  content TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  permission_profile_id TEXT,
  priority TEXT,
  timestamp TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_envelopes_conv_time
  ON agent_message_envelopes(conversation_id, timestamp ASC);

CREATE INDEX IF NOT EXISTS idx_agent_envelopes_conv_sender
  ON agent_message_envelopes(conversation_id, sender_id);

CREATE INDEX IF NOT EXISTS idx_agent_envelopes_conv_recipient
  ON agent_message_envelopes(conversation_id, recipient_id);

CREATE INDEX IF NOT EXISTS idx_agent_envelopes_conv_msgtype
  ON agent_message_envelopes(conversation_id, message_type);

CREATE INDEX IF NOT EXISTS idx_agent_envelopes_correlation
  ON agent_message_envelopes(correlation_id)
  WHERE correlation_id IS NOT NULL;
```

---

## 5. Required Rust Methods on `SqliteMemoryStore`

The following public methods should be added to `impl SqliteMemoryStore` in `crates/aro-memory/src/lib.rs`:

### 5.1 Retrieval: `get_agent_memory`
Retrieves and reconstitutes the full `AgentMemoryContext` for an `(conversation_id, agent_id)` pair, including its findings and ledger messages.

```rust
pub fn get_agent_memory(
    &self,
    conversation_id: &str,
    agent_id: &str,
) -> AroResult<Option<AgentMemoryContext>> {
    let conn = self.connect()?;
    
    // 1. Fetch base memory context
    let mut stmt = conn.prepare(
        r#"
        SELECT id, agent_name, role, scratchpad, artifacts_json, permission_profile_id, updated_at
        FROM agent_memories
        WHERE conversation_id = ?1 AND agent_id = ?2
        "#,
    ).map_err(|err| AroError::Memory(err.to_string()))?;
    
    let base_row = stmt.query_row(params![conversation_id, agent_id], |row| {
        let agent_name: String = row.get(1)?;
        let role: String = row.get(2)?;
        let scratchpad: String = row.get(3)?;
        let artifacts_raw: String = row.get(4)?;
        let permission_profile_id: Option<String> = row.get(5)?;
        let updated_at_str: String = row.get(6)?;
        Ok((agent_name, role, scratchpad, artifacts_raw, permission_profile_id, updated_at_str))
    }).optional().map_err(|err| AroError::Memory(err.to_string()))?;

    let (agent_name, role, scratchpad, artifacts_raw, permission_profile_id, updated_at_str) = match base_row {
        Some(row) => row,
        None => return Ok(None),
    };

    let artifacts: Vec<AgentArtifactRef> = serde_json::from_str(&artifacts_raw).unwrap_or_default();
    let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    // 2. Fetch findings
    let mut finding_stmt = conn.prepare(
        r#"
        SELECT id, summary, category, source_tool, timestamp
        FROM agent_findings
        WHERE conversation_id = ?1 AND agent_id = ?2
        ORDER BY timestamp ASC
        "#,
    ).map_err(|err| AroError::Memory(err.to_string()))?;
    
    let finding_rows = finding_stmt.query_map(params![conversation_id, agent_id], |row| {
        let id: String = row.get(0)?;
        let summary: String = row.get(1)?;
        let category: Option<String> = row.get(2)?;
        let source_tool: Option<String> = row.get(3)?;
        let ts_str: String = row.get(4)?;
        let ts = DateTime::parse_from_rfc3339(&ts_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        Ok(AgentMemoryFinding {
            id,
            summary,
            category,
            source_tool,
            timestamp: ts,
        })
    }).map_err(|err| AroError::Memory(err.to_string()))?;
    
    let findings = finding_rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| AroError::Memory(err.to_string()))?;

    // 3. Fetch ledger (messages where agent is sender, recipient, or recipient is broadcast)
    let mut ledger_stmt = conn.prepare(
        r#"
        SELECT id, parent_message_id, correlation_id, sender_json, recipient_json,
               message_type, payload_json, permission_profile_id, priority, timestamp
        FROM agent_message_envelopes
        WHERE conversation_id = ?1 AND (sender_id = ?2 OR recipient_id = ?2 OR recipient_kind = 'broadcast')
        ORDER BY timestamp ASC
        "#,
    ).map_err(|err| AroError::Memory(err.to_string()))?;

    let ledger_rows = ledger_stmt.query_map(params![conversation_id, agent_id], |row| {
        let id: String = row.get(0)?;
        let parent_message_id: Option<String> = row.get(1)?;
        let correlation_id: Option<String> = row.get(2)?;
        let sender_json: String = row.get(3)?;
        let recipient_json: String = row.get(4)?;
        let msg_type_str: String = row.get(5)?;
        let payload_json: String = row.get(6)?;
        let perm_id: Option<String> = row.get(7)?;
        let priority_str: Option<String> = row.get(8)?;
        let ts_str: String = row.get(9)?;

        let sender: AgentParticipant = serde_json::from_str(&sender_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?;
        let recipient: AgentParticipant = serde_json::from_str(&recipient_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e)))?;
        let message_type: AgentMessageType = serde_json::from_value(serde_json::Value::String(msg_type_str))
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?;
        let payload: AgentMessagePayload = serde_json::from_str(&payload_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e)))?;
        let priority: Option<AgentRunPriority> = priority_str.and_then(|p| serde_json::from_value(serde_json::Value::String(p)).ok());
        let timestamp = DateTime::parse_from_rfc3339(&ts_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(AgentMessageEnvelope {
            id,
            conversation_id: conversation_id.to_string(),
            parent_message_id,
            correlation_id,
            sender,
            recipient,
            message_type,
            payload,
            permission_profile_id: perm_id,
            priority,
            timestamp,
        })
    }).map_err(|err| AroError::Memory(err.to_string()))?;

    let ledger = ledger_rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| AroError::Memory(err.to_string()))?;

    Ok(Some(AgentMemoryContext {
        agent_id: agent_id.to_string(),
        agent_name,
        role,
        conversation_id: conversation_id.to_string(),
        scratchpad,
        findings,
        ledger,
        artifacts,
        permission_profile_id,
        updated_at,
    }))
}
```

### 5.2 Retrieval or Initialization: `get_or_create_agent_memory`
```rust
pub fn get_or_create_agent_memory(
    &self,
    conversation_id: &str,
    agent_id: &str,
    default_name: &str,
    default_role: &str,
) -> AroResult<AgentMemoryContext> {
    if let Some(existing) = self.get_agent_memory(conversation_id, agent_id)? {
        return Ok(existing);
    }
    let initial = AgentMemoryContext::new(conversation_id, agent_id, default_name, default_role);
    self.save_agent_memory(&initial)?;
    Ok(initial)
}
```

### 5.3 Full Context Save: `save_agent_memory`
Atomic upsert of base memory, findings, and ledger envelopes inside a `BEGIN IMMEDIATE` transaction.

```rust
pub fn save_agent_memory(&self, memory: &AgentMemoryContext) -> AroResult<()> {
    let mut conn = self.connect()?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| AroError::Memory(err.to_string()))?;

    let memory_id = format!("{}:{}", memory.conversation_id, memory.agent_id);
    let artifacts_json = serde_json::to_string(&memory.artifacts)
        .map_err(|err| AroError::Memory(err.to_string()))?;
    let updated_at = memory.updated_at.to_rfc3339();

    // 1. Upsert agent_memories
    tx.execute(
        r#"
        INSERT INTO agent_memories
          (id, conversation_id, agent_id, agent_name, role, scratchpad,
           artifacts_json, permission_profile_id, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(conversation_id, agent_id) DO UPDATE SET
          agent_name = excluded.agent_name,
          role = excluded.role,
          scratchpad = excluded.scratchpad,
          artifacts_json = excluded.artifacts_json,
          permission_profile_id = excluded.permission_profile_id,
          updated_at = excluded.updated_at
        "#,
        params![
            memory_id,
            memory.conversation_id,
            memory.agent_id,
            memory.agent_name,
            memory.role,
            memory.scratchpad,
            artifacts_json,
            memory.permission_profile_id,
            updated_at,
            updated_at,
        ],
    ).map_err(|err| AroError::Memory(err.to_string()))?;

    // 2. Upsert findings
    for finding in &memory.findings {
        tx.execute(
            r#"
            INSERT INTO agent_findings
              (id, conversation_id, agent_id, summary, category, source_tool, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
              summary = excluded.summary,
              category = excluded.category,
              source_tool = excluded.source_tool,
              timestamp = excluded.timestamp
            "#,
            params![
                finding.id,
                memory.conversation_id,
                memory.agent_id,
                finding.summary,
                finding.category,
                finding.source_tool,
                finding.timestamp.to_rfc3339(),
            ],
        ).map_err(|err| AroError::Memory(err.to_string()))?;
    }

    // 3. Upsert ledger envelopes
    for env in &memory.ledger {
        let sender_json = serde_json::to_string(&env.sender)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let recipient_json = serde_json::to_string(&env.recipient)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let msg_type_val = serde_json::to_value(&env.message_type)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let msg_type_str = msg_type_val.as_str().unwrap_or("task_delegation");
        let payload_json = serde_json::to_string(&env.payload)
            .map_err(|err| AroError::Memory(err.to_string()))?;
        let sender_kind_str = format!("{:?}", env.sender.kind).to_lowercase();
        let recipient_kind_str = format!("{:?}", env.recipient.kind).to_lowercase();
        let priority_str = env.priority.as_ref().and_then(|p| serde_json::to_value(p).ok()).and_then(|v| v.as_str().map(|s| s.to_string()));

        tx.execute(
            r#"
            INSERT INTO agent_message_envelopes
              (id, conversation_id, parent_message_id, correlation_id,
               sender_id, sender_name, sender_role, sender_kind, sender_json,
               recipient_id, recipient_name, recipient_role, recipient_kind, recipient_json,
               message_type, content, payload_json, permission_profile_id, priority, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
            ON CONFLICT(id) DO UPDATE SET
              content = excluded.content,
              payload_json = excluded.payload_json,
              permission_profile_id = excluded.permission_profile_id,
              priority = excluded.priority,
              timestamp = excluded.timestamp
            "#,
            params![
                env.id,
                env.conversation_id,
                env.parent_message_id,
                env.correlation_id,
                env.sender.id,
                env.sender.name,
                env.sender.role,
                sender_kind_str,
                sender_json,
                env.recipient.id,
                env.recipient.name,
                env.recipient.role,
                recipient_kind_str,
                recipient_json,
                msg_type_str,
                env.payload.content,
                payload_json,
                env.permission_profile_id,
                priority_str,
                env.timestamp.to_rfc3339(),
            ],
        ).map_err(|err| AroError::Memory(err.to_string()))?;
    }

    tx.commit().map_err(|err| AroError::Memory(err.to_string()))
}
```

### 5.4 Granular Mutations
- **`update_agent_scratchpad(&self, conversation_id: &str, agent_id: &str, scratchpad: &str) -> AroResult<()>`**: Updates only the scratchpad text and `updated_at` timestamp.
- **`record_agent_finding(&self, conversation_id: &str, agent_id: &str, finding: &AgentMemoryFinding) -> AroResult<()>`**: Inserts or updates a finding and bumps `agent_memories.updated_at`.
- **`record_agent_envelope(&self, envelope: &AgentMessageEnvelope) -> AroResult<()>`**: Inserts an envelope into the global ledger for the conversation.
- **`list_agent_envelopes(&self, conversation_id: &str, filter_agent_id: Option<&str>, filter_msg_type: Option<&AgentMessageType>) -> AroResult<Vec<AgentMessageEnvelope>>`**: Queries the ledger with optional filters.
- **`list_agent_findings(&self, conversation_id: &str, agent_id: Option<&str>) -> AroResult<Vec<AgentMemoryFinding>>`**: Lists findings for an agent or across the whole conversation.

### 5.5 Cleaning and Eviction
- **`clear_agent_memory(&self, conversation_id: &str, agent_id: Option<&str>) -> AroResult<()>`**:
  - If `agent_id` is given: deletes only that agent's context, findings, and messages.
  - If `agent_id` is `None`: deletes all memories, findings, and ledger envelopes for that conversation.
- **`delete_conversation(&self, conversation_id: Uuid)` integration**:
  Add statements to purge cognitive memory when a conversation is deleted:
  ```sql
  DELETE FROM agent_findings WHERE conversation_id = ?1;
  DELETE FROM agent_message_envelopes WHERE conversation_id = ?1;
  DELETE FROM agent_memories WHERE conversation_id = ?1;
  ```
- **`reset(&self)` integration**:
  Add `agent_memories`, `agent_findings`, and `agent_message_envelopes` to the table reset batch.

### 5.6 Inspection Helper
```rust
pub fn inspect_cognitive_database_tables(&self) -> AroResult<(usize, usize, usize)> {
    let conn = self.connect()?;
    let mem_count: i64 = conn.query_row("SELECT COUNT(*) FROM agent_memories", [], |r| r.get(0))
        .map_err(|err| AroError::Memory(err.to_string()))?;
    let find_count: i64 = conn.query_row("SELECT COUNT(*) FROM agent_findings", [], |r| r.get(0))
        .map_err(|err| AroError::Memory(err.to_string()))?;
    let env_count: i64 = conn.query_row("SELECT COUNT(*) FROM agent_message_envelopes", [], |r| r.get(0))
        .map_err(|err| AroError::Memory(err.to_string()))?;
    Ok((mem_count as usize, find_count as usize, env_count as usize))
}
```
This directly matches the stats inspected by `inspectDatabaseTables()` in `tests/e2e/helpers/cognitive-memory-engine.ts` (`agent_memories`, `agent_findings`, `agent_message_envelopes`).

---

## 6. Verification and Integration Plan

### 6.1 Rust Verification
1. Create a dedicated test file `crates/aro-memory/tests/cognitive_memory_persistence_tests.rs`:
   - `test_agent_memory_roundtrip`: Verify saving, querying, and reconstituting an `AgentMemoryContext` with findings and ledger messages.
   - `test_scratchpad_partial_update`: Verify fast scratchpad update without overwriting existing findings or ledger.
   - `test_finding_deduplication_and_upsert`: Verify idempotent insertion and category queries.
   - `test_ledger_filtering`: Verify querying ledger by sender, recipient, and message type.
   - `test_clear_conversation_and_cascade`: Verify `clear_agent_memory` cleans up all related records.
   - `test_concurrent_writes_immediate_isolation`: Verify multiple threads writing to different agents' scratchpads concurrently without encountering `SQLITE_BUSY`.
2. Run test suite: `cargo test -p aro-memory` and `cargo test -p aro-core`.

### 6.2 IPC & Desktop Integration
In Milestone 1 Feature 4, expose these methods through Tauri commands in `apps/desktop/src-tauri/src/main.rs`:
- `agent_get_memory(agent_id: String, conversation_id: String) -> Result<AgentMemoryContext, String>`
- `agent_save_memory(memory: AgentMemoryContext) -> Result<(), String>`
- `agent_dispatch_directive(agent_id: String, directive: String, conversation_id: String) -> Result<AgentRunView, String>`

In `apps/desktop/src/lib/agent-protocol.ts`:
- Replace the volatile `localStorage` backing with calls to Tauri IPC (falling back gracefully to cache during browser-only mock mode).

### 6.3 Downstream Impacts on Milestone 1 Features
- **Feature 2 (Sub-Agent Execution Loop)**: When sub-agents run in `aro-runtime`, they will update their working scratchpad and record findings directly into `SqliteMemoryStore`, allowing the UI and parent orchestrator to monitor them in real time.
- **Feature 3 (Cloud Worker Delegation)**: Message envelopes delegated to cloud workers are logged in `agent_message_envelopes` with type `task_delegation`.

---

## 7. Implementation Checklist for Builder Agent

| Step | File | Action |
|------|------|--------|
| 1 | `crates/aro-memory/src/lib.rs` | Add SQL DDL for `agent_memories`, `agent_findings`, `agent_message_envelopes` and indexes into `migrate()` |
| 2 | `crates/aro-memory/src/lib.rs` | Implement `get_agent_memory`, `get_or_create_agent_memory`, `save_agent_memory` |
| 3 | `crates/aro-memory/src/lib.rs` | Implement `update_agent_scratchpad`, `record_agent_finding`, `record_agent_envelope` |
| 4 | `crates/aro-memory/src/lib.rs` | Implement `list_agent_envelopes`, `list_agent_findings`, `clear_agent_memory`, `inspect_cognitive_database_tables` |
| 5 | `crates/aro-memory/src/lib.rs` | Update `delete_conversation` and `reset` to purge cognitive memory tables |
| 6 | `crates/aro-memory/tests/cognitive_memory_persistence_tests.rs` | Add comprehensive unit, roundtrip, and concurrency tests |
| 7 | Shell | Run `cargo test -p aro-memory` and `npm run lint:rust` |
