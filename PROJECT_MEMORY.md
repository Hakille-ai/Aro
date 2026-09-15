# Project: ARO Cognitive Memory Backend Implementation

## Architecture
The ARO Cognitive Memory Backend provides a unified multi-level cognitive memory lifecycle for the ARO AI agent system:
- **Tier 1 — Immediate Working Memory**: Bounded high-fidelity message buffer ($N \le 8$ turns, $B \le 2,400$ tokens) tracking active conversational turns, session variables, and scratchpad.
- **Tier 2 — Medium-Term Episodic Memory**: Structured episodic summaries (`episodes` + `episodes_fts`) recording conversational intervals (10-turn chunks), task milestones, key decisions, and critical entities.
- **Tier 3 — Long-Term Semantic Memory**: Persistent knowledge base in SQLite (`memories` + `memories_fts`) and Qdrant/`aro-vector` with dynamic scoring:
  - Initial Salience $S_0 \in [0.1, 1.0]$ based on user preference/rule indicators and entity density.
  - Ebbinghaus Recency Decay $R(t) = \exp(-\ln(2) \cdot \Delta t / 168.0)$ (7-day half-life, pinned memories exempt).
  - Recall Utility $U(m, q) = 0.55 \cdot \text{Score}_{\text{hybrid}} + 0.30 \cdot (S_0 \cdot R(t)) + 0.15 \cdot \ln(1 + \text{recall\_count})$.
- **Adaptive Context Compaction**: Automated background compaction preventing token window saturation across 100+ turns while guaranteeing lossless preservation of critical entities, constraints, and decisions within an 8,192 token ceiling.
- **Hybrid Search Engine**: Reciprocal Rank Fusion (RRF, $k=60, w_{\text{fts}}=0.40, w_{\text{vec}}=0.60$) combining SQLite FTS5 BM25 and `aro-vector` cosine similarity, modulated by salience and pinned flags.
- **Agent Memory Primitives**: Full agent tooling exposing `memory_save`, `memory_search`, `memory_recall`, `memory_update`, and `memory_forget`.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | F1.1 | SQLite WAL Mode & Concurrency Configuration (`PRAGMA journal_mode = WAL`, `synchronous = NORMAL`) | M1 | Survey / AC2 |
| 2 | F1.2 | Episodic Memory Relational & FTS5 Schema (`episodes`, `episodes_fts`, indexes) | M1 | ORIGINAL_REQUEST §R1 |
| 3 | F1.3 | Unified Cognitive Memory Types in `aro-core` (`WorkingMemoryBuffer`, `Episode`, `EpisodeSummary`, `ConsolidationCandidate`) | M1 | ORIGINAL_REQUEST §R1 |
| 4 | F1.4 | Dynamic Scoring Engine: Salience ($S_0$), Ebbinghaus Decay ($R(t)$), Utility ($U(m, q)$) | M1 | ORIGINAL_REQUEST §R1 |
| 5 | F1.5 | Thread-Safe & Transactional Episodic Storage in `SqliteMemoryStore` | M1 | ORIGINAL_REQUEST §R1, AC2 |
| 6 | F2.1 | Adaptive Context Window Manager with 8,192 token ceiling partitioning | M2 | ORIGINAL_REQUEST §R2, AC1 |
| 7 | F2.2 | Continuous Background Consolidation Trigger (10-turn intervals, token thresholds) | M2 | ORIGINAL_REQUEST §R2 |
| 8 | F2.3 | Lossless Entity, Constraint & Decision Preservation Extractor | M2 | ORIGINAL_REQUEST §R2 |
| 9 | F2.4 | Bounded Prompt & History Assembly in `aro-runtime` and `aro-agent` | M2 | ORIGINAL_REQUEST §R2, AC1 |
| 10 | F3.1 | Reciprocal Rank Fusion (RRF) Hybrid Search coupling FTS5 BM25 + `aro-vector` | M3 | ORIGINAL_REQUEST §R3 |
| 11 | F3.2 | Top-K ID Retrieval replacing full-table RAM dumps (`store.list_memories()`) | M3 | Survey / R3 |
| 12 | F3.3 | Canonical Agent Memory Tools (`memory_save`, `memory_search`, `memory_recall`, `memory_update`, `memory_forget`) | M3 | ORIGINAL_REQUEST §R3, AC3 |
| 13 | F3.4 | Tool Normalization & Alias Routing in `AgentRuntime` and `ToolRegistry` | M3 | Survey / R3 |
| 14 | F4.1 | 100+ Turn Long-Conversation Benchmark (facts planted at turns 5 & 20, recalled at turn 95+) | M4 | ORIGINAL_REQUEST §AC1 |
| 15 | F4.2 | Token Budget Ceiling Enforcement across all 100+ turns ($B \le 8,192$) | M4 | ORIGINAL_REQUEST §AC1 |
| 16 | F4.3 | Cold Restart & Resumption Resilience Verification (episodic context + semantic facts restored) | M4 | ORIGINAL_REQUEST §AC2, AC3 |
| 17 | F4.4 | Zero Regression Guarantee: `cargo test -p aro-memory -p aro-vector -p aro-runtime` 100% pass | M4 | ORIGINAL_REQUEST §AC2 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| 1 | M1: Core Storage, Schema & Dynamic Scoring | WAL mode, `episodes` + `episodes_fts` schema in `aro-memory`, domain models in `aro-core`, dynamic scoring formulas ($S_0$, $R(t)$, $U(m, q)$) | none | DONE |
| 2 | M2: Context Manager & Lossless Compactor | Token budgeting manager (8k ceiling), 10-turn background compaction daemon, lossless entity extractor, bounded prompt assembly in `aro-runtime`/`aro-agent` | M1 | DONE |
| 3 | M3: Hybrid Search (RRF) & Agent Tooling | FTS5 + `aro-vector` Reciprocal Rank Fusion, Top-K ID retrieval, 5 agent memory primitives (`save`, `search`, `recall`, `update`, `forget`), alias routing | M1, M2 | DONE |
| 4 | M4: 100+ Turn Benchmark, Resumption & Hardening | 100+ turn integration test suite (turns 5/20 -> 95+ recall), restart resilience verification, 100% test pass on memory, vector, runtime crates | M1, M2, M3 | DONE |
| E2E | E2E Testing Track | Requirement-driven test suite for cognitive memory across Tiers 1-4 | none | DONE |

## Interface Contracts

### 1. Episodic Storage (`SqliteMemoryStore`)
```rust
impl SqliteMemoryStore {
    pub fn store_episode(&self, episode: &Episode) -> AroResult<()>;
    pub fn list_episodes(&self, conversation_id: Uuid) -> AroResult<Vec<Episode>>;
    pub fn search_episodes(&self, conversation_id: Uuid, query: &str, limit: usize) -> AroResult<Vec<Episode>>;
    pub fn get_latest_episode(&self, conversation_id: Uuid) -> AroResult<Option<Episode>>;
}
```

### 2. Dynamic Scoring (`aro-core::memory`)
```rust
pub fn compute_initial_salience(content: &str, pinned: bool, category: MemoryCategory) -> f32;
pub fn compute_recency_decay(last_used_at: DateTime<Utc>, now: DateTime<Utc>, pinned: bool) -> f32;
pub fn compute_memory_utility(hybrid_score: f32, salience: f32, recency_factor: f32, recall_count: u32) -> f32;
```

### 3. Context Management & Compaction (`aro-runtime`)
```rust
pub struct ContextBudget {
    pub system_budget: usize,   // 800
    pub semantic_budget: usize, // 1600
    pub episodic_budget: usize, // 2000
    pub working_budget: usize,  // 2400
    pub reserve_budget: usize,  // 1392
}

pub trait ContextCompactor: Send + Sync {
    fn should_compact(&self, turn_count: usize, estimated_tokens: usize) -> bool;
    fn compact_turns(&self, conversation_id: Uuid, turns: &[ChatMessage]) -> AroResult<CompactionResult>;
}
```

### 4. Hybrid Search Fusion (`aro-vector`)
```rust
pub fn reciprocal_rank_fusion(
    fts_hits: &[ScoredMemoryId],
    vec_hits: &[VectorMemoryHit],
    k: f32, // 60.0
    w_fts: f32, // 0.40
    w_vec: f32, // 0.60
) -> Vec<FusedMemoryScore>;
```

### 5. Agent Memory Tools JSON Contract
- `memory_save`: `{ content: String, category?: String, scope?: String, pinned?: bool, salience?: f32 } -> { success: bool, id: Uuid, content: String }`
- `memory_search`: `{ query: String, limit?: usize, category?: String, scope?: String } -> { count: usize, memories: Vec<ScoredMemory> }`
- `memory_recall`: `{ id?: Uuid, entity_key?: String, include_episodes?: bool } -> { found: bool, memory?: Memory, related_episodes?: Vec<Episode> }`
- `memory_update`: `{ id: Uuid, content?: String, salience?: f32, pinned?: bool, category?: String } -> { success: bool, id: Uuid, updated_at: String }`
- `memory_forget`: `{ id: Uuid, reason?: String } -> { success: bool, id: Uuid, status: "archived" }`

## Code Layout
- `crates/aro-core/src/memory.rs`: Memory structs, `Episode`, `EpisodeSummary`, dynamic scoring equations.
- `crates/aro-core/src/tool.rs`: Canonical memory tool IDs and aliases.
- `crates/aro-memory/src/lib.rs`: SQLite store, WAL mode, migrations (`episodes`, `episodes_fts`), triggers, transactional methods.
- `crates/aro-vector/src/lib.rs`: Reciprocal Rank Fusion, Top-K ID hybrid retriever, vector indexing.
- `crates/aro-agent/src/lib.rs`: Tool registry descriptors with aliases, context pack assembly with episodic/semantic tiers.
- `crates/aro-runtime/src/lib.rs`: Agent execution loop, tool dispatch, context window manager, background compaction daemon.
- `crates/aro-runtime/tests/long_conversation_100_turns_test.rs`: 100+ turn integration test suite and restart resilience benchmarks.
