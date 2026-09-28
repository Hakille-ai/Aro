# Plan — ARO Architecture & Multi-Agent Orchestration

## Phase 0: Survey & Scope Mapping
- [ ] Dispatch 3 parallel Explorers:
  - Explorer 1: Rust backend orchestration, multi-agent coordination, session persistence, and CLI/API boundaries.
  - Explorer 2: Advanced agent tooling, security boundaries, permission policies (Standard, Read-Only, Autonomous Developer, Sandbox).
  - Explorer 3: Desktop UI & Observability (breadcrumbs, subagent navigation, real-time indicators, security badges, inspection view) and TypeScript/contracts/testing suites.
- [ ] Synthesize findings into `PROJECT.md` (Architecture, Feature Inventory, Milestones, Interface Contracts, Code Layout).
- [ ] Synthesize E2E testing strategy into `TEST_INFRA.md`.

## Phase 1: Milestones Implementation & E2E Testing (Dual Track)
- [ ] Implementation Track: Execute Milestones (Rust Core & Collaboration, Tooling & Sandboxing, Desktop UI & Observability).
- [ ] E2E Testing Track: Build comprehensive test suite covering all features, boundaries, combinations, and application scenarios.
- [ ] Publish `TEST_READY.md`.

## Phase 2: Final Verification & Zero-Regression Invariants
- [ ] Phase 1 E2E Test Suite Pass (100% of Tiers 1-4).
- [ ] Ensure all invariants pass:
  - `npm run contracts:check` (100% pass)
  - `npm run api-client:test` (100% pass)
  - `npm run test:unit` & `npm run test:components` (100% pass)
  - `npm run lint:rust` (0 warnings, clean audit)
- [ ] Phase 2 Adversarial Coverage Hardening (Tier 5).
- [ ] Independent Forensic Audit (`teamwork_preview_auditor`).
- [ ] Report Completion to Sentinel.
