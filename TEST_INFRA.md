# E2E Test Infra: ARO Autonomous Multi-Agent & Tooling

## Test Philosophy
- Opaque-box, requirement-driven. Derived from `ORIGINAL_REQUEST.md` and user specifications.
- Methodology: Category-Partition + Boundary Value Analysis (BVA) + Pairwise Combinatorial Testing + Real-World Workload Testing.

## Feature Inventory & Test Mapping
| # | Feature | Requirement | Tier 1 (Coverage) | Tier 2 (Boundaries) | Tier 3 (Interactions) | Tier 4 (Workload) |
|---|---------|-------------|:-----------------:|:-------------------:|:---------------------:|:-----------------:|
| 1 | Cognitive Memory Persistence | R1 | 5 | 5 | ✓ | ✓ |
| 2 | Sub-Agent Async Execution | R1 | 5 | 5 | ✓ | ✓ |
| 3 | Cloud Worker Delegation | R1 | 5 | 5 | ✓ | ✓ |
| 4 | Memory IPC & Sync | R1 | 5 | 5 | ✓ | ✓ |
| 5 | Tool Authorization Guard | R2 | 5 | 5 | ✓ | ✓ |
| 6 | Workspace Confinement | R2 | 5 | 5 | ✓ | ✓ |
| 7 | Shell & Code Sandboxing | R2 | 5 | 5 | ✓ | ✓ |
| 8 | Tool Registry Parity | R2 | 5 | 5 | ✓ | ✓ |
| 9 | Svelte Typecheck Integrity | R3/R4 | 5 | 5 | ✓ | ✓ |
| 10 | Persistent Sub-Agent UI Thread | R3 | 5 | 5 | ✓ | ✓ |
| 11 | Observability & Breadcrumbs | R3 | 5 | 5 | ✓ | ✓ |

## Test Architecture
- **Location**: `tests/e2e/`
- **Execution Command**: `npm run test:e2e:opaque` or Node/Vitest opaque-box test runner
- **Pass/Fail Semantics**: 100% assertions pass, exit code 0.
- **Tiers**:
  - **Tier 1 (Feature Coverage)**: >=5 isolated tests per feature (55+ tests).
  - **Tier 2 (Boundary & Corner Cases)**: >=5 boundary tests per feature (empty inputs, overflows, unauthorized access, root escaping) (55+ tests).
  - **Tier 3 (Cross-Feature Combinations)**: Multi-agent coordination with restricted permission presets (e.g. Subagent running under Read-Only preset attempting workspace write) (11+ pairwise tests).
  - **Tier 4 (Real-World Scenarios)**: Full user workflows (e.g. Principal agent decomposes a complex software engineering problem, delegates to sub-agents, shares findings across restarts, validates security boundaries, reflects live updates in UI) (6+ scenarios).

## Real-World Application Scenarios (Tier 4)
| # | Scenario | Features Exercised | Complexity |
|---|----------|--------------------|------------|
| 1 | Autonomous Multi-Agent Code Audit | F1, F2, F5, F6, F10, F11 | High |
| 2 | Read-Only Sandbox Exploration | F5, F6, F7, F8, F11 | Medium |
| 3 | State Recovery Across Restart | F1, F4, F10 | High |
| 4 | Sandboxed Code Execution with Secret Scrubbing | F5, F7 | Medium |
| 5 | Complex Collaborative Task with Artifact Handover | F1, F2, F3, F4, F10 | High |
| 6 | Full Permission Escalation Prevention | F5, F6, F7, F8 | High |

## Coverage Thresholds
- Tier 1: >=55 tests (>=5 per feature)
- Tier 2: >=55 tests (>=5 per feature)
- Tier 3: >=11 cross-feature tests
- Tier 4: >=6 real-world scenarios
- **Total Minimum**: >=127 comprehensive E2E tests
