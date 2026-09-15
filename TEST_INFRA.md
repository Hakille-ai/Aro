# E2E Test Infra: ARO Cognitive Memory Backend

## Test Philosophy
- Opaque-box, requirement-driven. Derives strictly from `ORIGINAL_REQUEST.md` (Follow-up 2026-09-11T21:44:50Z).
- Methodology: Category-Partition + Boundary Value Analysis (BVA) + Pairwise Combinatorial Testing + 100+ Turn Long-Conversation Workload Testing.

## Feature Inventory & Test Mapping
| # | Feature | Source (Requirement) | Tier 1 (Feature) | Tier 2 (Boundary) | Tier 3 (Cross-Feature) | Tier 4 (Workload) |
|---|---------|----------------------|:----------------:|:-----------------:|:----------------------:|:-----------------:|
| 1 | Working Memory Buffer | ORIGINAL_REQUEST §R1 | 5 tests | 5 tests | ✓ | ✓ |
| 2 | Episodic Memory & Schema | ORIGINAL_REQUEST §R1 | 5 tests | 5 tests | ✓ | ✓ |
| 3 | Long-Term Semantic Store | ORIGINAL_REQUEST §R1 | 5 tests | 5 tests | ✓ | ✓ |
| 4 | Dynamic Scoring Engine | ORIGINAL_REQUEST §R1 | 5 tests | 5 tests | ✓ | ✓ |
| 5 | WAL Concurrency & ACID | ORIGINAL_REQUEST §AC2 | 5 tests | 5 tests | ✓ | ✓ |
| 6 | Adaptive Context Manager | ORIGINAL_REQUEST §R2 | 5 tests | 5 tests | ✓ | ✓ |
| 7 | Lossless Entity Preservation | ORIGINAL_REQUEST §R2 | 5 tests | 5 tests | ✓ | ✓ |
| 8 | Hybrid Search (RRF) | ORIGINAL_REQUEST §R3 | 5 tests | 5 tests | ✓ | ✓ |
| 9 | Agent Tool: memory_save | ORIGINAL_REQUEST §R3 | 5 tests | 5 tests | ✓ | ✓ |
| 10 | Agent Tool: memory_search | ORIGINAL_REQUEST §R3 | 5 tests | 5 tests | ✓ | ✓ |
| 11 | Agent Tool: memory_recall | ORIGINAL_REQUEST §R3 | 5 tests | 5 tests | ✓ | ✓ |
| 12 | Agent Tool: memory_update | ORIGINAL_REQUEST §R3 | 5 tests | 5 tests | ✓ | ✓ |
| 13 | Agent Tool: memory_forget | ORIGINAL_REQUEST §R3 | 5 tests | 5 tests | ✓ | ✓ |
| 14 | 100+ Turn Long Session | ORIGINAL_REQUEST §AC1 | 5 tests | 5 tests | ✓ | ✓ |
| 15 | Cold Restart Resumption | ORIGINAL_REQUEST §AC3 | 5 tests | 5 tests | ✓ | ✓ |

## Test Architecture
- Test runner: `cargo test -p aro-memory -p aro-vector -p aro-runtime`
- Integration harness: `crates/aro-runtime/tests/long_conversation_100_turns_test.rs`
- Pass/fail semantics: 100% pass, 0 regressions on baseline 27 unit tests.

## Coverage Thresholds
- Tier 1: $\ge 5$ tests per feature (happy path isolation).
- Tier 2: $\ge 5$ tests per feature (empty strings, token overflows, corrupted inputs, boundary limits).
- Tier 3: Pairwise interaction (e.g., compaction + restart, save + hybrid search, decay + pinned override).
- Tier 4: $\ge 5$ real-world application scenarios (100+ turns simulation, facts at turns 5/20 recalled at turn 95+, multi-agent concurrent writes).
