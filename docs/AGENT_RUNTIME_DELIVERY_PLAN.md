# Plan canonique de refactorisation du runtime d’agents

Statut : séquence de livraison proposée, à exécuter après validation des documents d’audit et d’architecture.

Ce plan supplante AGENT_OS_REFACTORING_PLAN.md pour le périmètre précis du runtime d’agents. Les travaux tools/skills/plugins/MCP de cet ancien plan deviennent des sous-chantiers de la phase 13.

## 1. Principes de livraison

- Strangler pattern : nouvelle architecture derrière interfaces stables, puis migration par tenant/run.
- Migrations expand/backfill/switch/contract ; aucune suppression avant la fin du double-read.
- Flags serveur, jamais uniquement frontend.
- Un diff de périmètre et un git status avant chaque tranche afin de préserver le travail parallèle.
- Aucun faux fallback : une capacité indisponible échoue explicitement.
- Chaque phase se termine par preuve automatisée, télémétrie et procédure de rollback.
- Les phases peuvent préparer la suivante, mais leurs gates ne sont pas contournées.

## 2. Carte des phases

| # | Phase | Gate principal |
| ---: | --- | --- |
| 1 | Cartographie de l’existant | baseline et inventaire signés |
| 2 | Concepts et contrats | domain v2 compilable et versionné |
| 3 | Modèle de données | migrations additives et RLS testées |
| 4 | Machines à états | reducer exhaustif et property tests |
| 5 | Runtime durable | vraie activation claim/heartbeat/fencing |
| 6 | Persistance des runs | API crée un run v2 durable |
| 7 | Checkpoints et reprise | kill/recovery sans perte |
| 8 | Tâches et DAG | séquentiel/parallèle/fan-in |
| 9 | Isolation du contexte | aucune fuite entre tasks |
| 10 | Mémoire agent | ACL/version/projection asynchrone |
| 11 | Sous-agents | délégation bornée sans escalade |
| 12 | Environnements | lifecycle isolé et cleanup |
| 13 | Modèles et capabilities | gateway unifiée exécutable |
| 14 | Permissions et approvals | action-hash et revalidation |
| 15 | Événements, triggers et scheduler | démarrage durable dédupliqué |
| 16 | Streaming et frontend | UI complète sur projections v2 |
| 17 | Migration des agents actuels | compatibilité et double-read validés |
| 18 | Résilience et tests | matrice crash/chaos/longue durée |
| 19 | Observabilité | SLO, alertes et runbooks |
| 20 | Déploiement progressif | canary et rollback exercés |
| 21 | Durcissement production | gates GA signés |

## Phase 1 — Cartographie de l’existant

**Objectif.** Établir le baseline autoritatif, les trois chemins d’exécution, les divergences, P0/P1/P2, actifs réutilisables et ownership.

**Composants/fichiers.** aro-core agent/tool, aro-agent, aro-runtime, aro-tools, apps/api handlers/main/agent_runner, aro-store agent_jobs et migrations, desktop App/api/features. Livrables : AGENT_RUNTIME_AUDIT_2026-07-16.md et architecture cible.

**Schémas/migrations.** Inventaire seulement : agent_runs/lanes/steps/artifacts/context, agent_run_jobs/events, custom_agent_definitions, memory, integrations, scheduled tasks. Aucune migration.

**Services/endpoints.** Tracer POST /agent/runs, commandes de contrôle, direct tool, worker claim/finalize et UI. Aucune modification.

**Événements.** Inventorier événements historiques dérivés, agent_run_events durables et outbox ; aucune émission ajoutée.

**Risques/dépendances.** Baseline sans commits et modifications parallèles. Capturer timestamp/résultat et revalider avant code.

**Tests.** cargo check workspace, tests ciblés agent/store/tools, npm check/unit ; classifier les échecs préexistants.

**Acceptation.** Tous les livrables demandés sont mappés, les constats obsolètes sont signalés et aucune partie hors périmètre n’est modifiée.

**Rollback.** Documentation additive uniquement ; supprimer les nouveaux documents si rejetés.

## Phase 2 — Concepts et contrats v2

**Objectif.** Introduire les identités et contrats immuables Agent, AgentVersion, Instance, Run, Session, Task, Step, Attempt, PlanVersion, Checkpoint, Result, Environment, Call, Approval et Event.

**Composants/fichiers.** Nouveau module/crate aro-agent-domain ; adapters dans aro-core pour compatibilité ; schémas JSON/OpenAPI et génération TypeScript. Éviter de renommer les types historiques.

**Schémas/migrations.** Aucune table obligatoire ; définir IDs, enums, commands, event envelope, error taxonomy, version fields et canonical serialization.

**Services/endpoints.** Traits AgentCommandService, AgentQueryService, RuntimeRepository, EventSink, Clock, IdGenerator. DTO /v1 expérimentaux derrière flag.

**Événements.** Schemas version 1 pour agent.*, run.*, task.*, checkpoint.*, tool/model.call.*, approval.*.

**Risques/dépendances.** Objets trop génériques, sérialisation instable, dépendance circulaire avec aro-core. Dépend validation de l’architecture.

**Tests.** Snapshot/round-trip JSON, backward compatibility, canonical hash, exhaustivité enums, cargo semver checks si disponible.

**Acceptation.** Contrats compilables sans accès DB/HTTP, documentation de chaque invariant, aucun comportement legacy cassé.

**Rollback.** Crate/module non référencé quand flag off ; retirer les adapters.

## Phase 3 — Modèle de données v2

**Objectif.** Ajouter le schéma tenant-safe et immutable nécessaire au runtime sans modifier les tables historiques.

**Composants/fichiers.** Nouvelles migrations SQLx et aro-agent-store repositories séparés de aro-store/lib.rs ; fixtures DB.

**Schémas/migrations.** Créer par lots agents/versions/instances, runs/sessions, plans/tasks/dependencies/steps/attempts, checkpoints/results, calls/effect ledger, context/memory ACL, workspaces/environments, approvals/triggers/schedules/events/commands/budgets/delegations. Ajouter index partiels, FK composites, CHECK, partitioning initial. Activer/forcer RLS après tests du contexte tenant.

**Services/endpoints.** Repositories seulement ; aucun trafic productif. Endpoint admin interne de diagnostics schema optionnel.

**Événements.** Aucun événement métier avant branchement ; migration auditée.

**Risques/dépendances.** Locks, volume futur, RLS lockout, duplication avec tables historiques. Dépend phase 2.

**Tests.** Migrate base vide et snapshot N-1, rollback applicatif, FK cross-tenant, RLS API/worker, index plans via EXPLAIN, immutability triggers.

**Acceptation.** Migrations additives répétables en staging, aucun downtime, rôles API/worker minimaux et tous tests d’isolation verts.

**Rollback.** Application ignore tables v2 ; tables supprimables seulement en environnement non productif. En prod, laisser le schéma dormant.

## Phase 4 — Reducers et machines à états

**Objectif.** Centraliser toutes les transitions Run/Task/Step/Approval/Environment en fonctions pures avec guards.

**Composants/fichiers.** aro-agent-domain state modules ; AgentCommandHandler transactionnel ; clock injectable.

**Schémas/migrations.** Utiliser agent_commands pour déduplication et expected_version ; event append-only et projection status/version atomique.

**Services/endpoints.** Commandes create/start/pause/resume/cancel/instruct/retry ; non exposées publiquement avant tests.

**Événements.** run.state.changed et événements spécifiques avec correlation/causation/command IDs.

**Risques/dépendances.** Transitions manquantes, terminal state resurrection, double completion. Dépend phases 2-3.

**Tests.** Table-driven toutes paires d’états, property tests, commandes rejouées, version conflicts, terminal immutability, invalid clocks.

**Acceptation.** Aucune écriture de status v2 hors CommandHandler ; couverture exhaustive des guards et mêmes commandes produisant mêmes événements.

**Rollback.** Flag runtime v2 off ; données tests isolées. Reducers restent sans impact legacy.

## Phase 5 — Runtime durable d’activations

**Objectif.** Remplacer le worker simulé par un ActivationRunner borné qui claim, heartbeat, yield, fence et échoue honnêtement.

**Composants/fichiers.** apps/worker ou rôle worker dédié, aro-agent-runtime scheduler/runner, adapter temporaire autour de agent_run_jobs, sémaphore local, cancellation token, reaper/reconciler loops.

**Schémas/migrations.** Étendre progressivement la queue vers agent_task activations ou créer agent_task_activations. Supprimer la contrainte conceptuelle un job/run seulement par expansion compatible ; conserver les colonnes legacy.

**Services/endpoints.** Worker health/readiness, drain admin authentifié, aucune requête HTTP longue.

**Événements.** activation.claimed/started/heartbeat_lost/yielded/completed/abandoned.

**Risques/dépendances.** Double claim, starvation, stampede après restart, worker role grants. Dépend phases 3-4.

**Tests.** Deux workers concurrents, lease expiry, stale token writes, bounded concurrency, graceful drain, perte Redis, reaper et fairness.

**Acceptation.** Aucun texte simulé, pas de spawn non borné, usage réel monotone, récupération après kill et progrès sans Redis.

**Rollback.** Stop claims v2, drain workers, flag off ; jobs v2 restent queued pour reprise ultérieure.

## Phase 6 — Persistance et API des runs

**Objectif.** Faire de POST /v1/agents/{id}/runs une transaction durable complète et fournir les commandes de contrôle v2.

**Composants/fichiers.** Module agent API Axum séparé de handlers.rs, AgentCommandService, projectors, adapters DTO legacy.

**Schémas/migrations.** Transaction AgentRun + Session + Plan/Task racine + budgets + command + event + outbox. Aucun modèle/contexte brut obligatoire dans la table run.

**Services/endpoints.** Create/list/get run, pause/resume/cancel/instructions/retry, progress/results/costs. Idempotency-Key, If-Match, pagination cursor.

**Événements.** run.created/queued/paused/resumed/cancel.requested/instruction.received.

**Risques/dépendances.** Double création, divergence projection/event, auth stale. Dépend phase 5.

**Tests.** Rejeu idempotent, transaction rollback, tenant access, terminal resume refusé, commands concurrentes, OpenAPI contract.

**Acceptation.** Chaque run v2 est visible après commit et réclamable sans appel synchrone au modèle ; contrôles passent tous par reducer.

**Rollback.** Router nouvelles créations vers legacy ; conserver lecture v2 et drainer les runs déjà créés.

## Phase 7 — Checkpoints et reprise

**Objectif.** Créer des snapshots restaurables, intègres et migrables à toutes les frontières critiques.

**Composants/fichiers.** CheckpointService, SnapshotCodec, keyring/KMS adapter, migrators de schema, RecoveryService, effect reconciliation hook.

**Schémas/migrations.** agent_checkpoints, links artifacts/effects, current_checkpoint FK, digest/MAC/key version, safe_restore et parent checkpoint. Payload chiffré par référence si volumineux.

**Services/endpoints.** List/get checkpoint redacted, request restore, internal create/validate. Aucun payload secret au frontend.

**Événements.** run.checkpoint.created/validated/corrupt/restored, run.recovery.started/completed/blocked.

**Risques/dépendances.** Snapshot incomplet, incompatibilité de runtime, corruption, rollback après effet. Dépend phases 5-6 et effect stubs.

**Tests.** Golden snapshots, tamper/MAC, key rotation, migration v1→v2, kill avant/après commit, artifact manquant, checkpoint ancien non sûr.

**Acceptation.** Une activation tuée reprend depuis le dernier checkpoint sans répéter une Step validée ; corruption mène à quarantine explicite.

**Rollback.** Lecture des checkpoints reste optionnelle ; revenir au démarrage de Task seulement pour runs sans effet externe et flag ciblé.

## Phase 8 — Tâches, DAG et orchestration

**Objectif.** Supporter tâches séquentielles/parallèles/conditionnelles/récurrentes, dépendances, fan-out/fan-in, retry et compensation.

**Composants/fichiers.** PlanValidator, TaskScheduler, DependencyResolver, FanOutGuard, CompensationPlanner.

**Schémas/migrations.** agent_tasks/dependencies/plans déjà créées ; ajouter conditions versionnées, dependency groups, ready_at, retry policy, compensation_task_id et index claims.

**Services/endpoints.** List/create/patch/cancel/retry tasks, get plan versions. Les modifications de plan créent PlanVersion.

**Événements.** plan.version.created, task.created/blocked/ready/started/completed/failed/retried/skipped/compensated.

**Risques/dépendances.** Cycles, explosion fan-out, fan-in perdu, priorité injuste. Dépend phases 4-7.

**Tests.** DAG aléatoires, cycle rejection, all/any/quorum, concurrence, cancellation cascade, retry budgets, compensation, virtual time.

**Acceptation.** Deux tasks indépendantes s’exécutent en parallèle ; fan-in ne démarre qu’une fois ; aucune tâche au-delà des quotas.

**Rollback.** Limiter planner au DAG linéaire par flag ; terminer les plans déjà matérialisés.

## Phase 9 — Isolation du contexte

**Objectif.** Construire des prompts minimaux et prouver l’absence de fuite entre scopes et tâches.

**Composants/fichiers.** aro-agent-context ScopeResolver, ContextAssembler, Redactor, PromptBudgeter, provider-neutral PromptPackage.

**Schémas/migrations.** context_items/links, scope type/id, sensitivity, prompt_policy, provenance, expiry et ACL ; index tenant/scope/type.

**Services/endpoints.** Internal context preview redacted pour debug autorisé ; aucun endpoint de dump brut.

**Événements.** context.item.published/expired, context.package.built avec refs/digest uniquement.

**Risques/dépendances.** Prompt injection, ACL contournée par vector search, sur-résumé, coût. Dépend phases 2-3 et Task model.

**Tests.** Matrice de scopes, sibling isolation, subtask inheritance, forbidden data, provider residency, injection corpus, token budgets.

**Acceptation.** Un test canari secret dans Task A n’apparaît jamais dans Task B ; provenance de chaque item sélectionné disponible.

**Rollback.** ContextAssembler v2 par flag ; fallback legacy seulement pour runs legacy.

## Phase 10 — Mémoire structurée

**Objectif.** Ajouter mémoire typée, versionnée, ACL, confiance, TTL, contradiction et projection vectorielle asynchrone.

**Composants/fichiers.** MemoryService, MemoryPolicy, Deduplicator, ContradictionResolver, ProjectionWorker ; adapter vers aro-memory/aro-vector.

**Schémas/migrations.** memory_versions, sources, relations, ACL, consents, projection jobs et tombstones ; backfill des mémoires legacy avec provenance unknown.

**Services/endpoints.** Search/create/update/delete/merge, propose lesson, summarize run, export/forget.

**Événements.** memory.proposed/created/versioned/merged/expired/forgotten/projected.

**Risques/dépendances.** Contamination cross-agent, suppression incomplète, Qdrant lag, vérités modèle. Dépend phase 9.

**Tests.** ACL/RLS, dedupe, contradictions, TTL virtual clock, projection outage/rebuild, export/delete, consent.

**Acceptation.** PostgreSQL répond correctement sans Qdrant ; partage refusé par défaut ; oubli supprime contenu et projection avec reçu.

**Rollback.** Dual-read avec préférence legacy ; arrêter writes v2 et conserver backfill journalisé.

## Phase 11 — Sous-agents

**Objectif.** Permettre une délégation durable, bornée, observable et sans escalade.

**Composants/fichiers.** DelegationService, SubagentSpawner, mailbox/inbox, result validator et loop detector.

**Schémas/migrations.** delegations, parent_run/task, permission/budget/context digests, depth/count, status et reservation. Remplacer parent_job_id orphelin par FK/adaptation.

**Services/endpoints.** Spawn/list/message/stop subagent ; agents.subagent.spawn utilise le même service.

**Événements.** subagent.spawn.requested/spawned/message.sent/completed/failed/stopped.

**Risques/dépendances.** Escalade, récursion infinie, fan-out et coûts, contexte excessif. Dépend phases 8-10 et policy de base.

**Tests.** Permission intersection, profondeur/count atomiques, budget child, loop goals, parent cancel, result schema, tenant isolation.

**Acceptation.** Un enfant ne peut obtenir aucune capability, donnée, environnement ou budget absent du parent ; limites tiennent sous concurrence.

**Rollback.** Désactiver spawn ; laisser les enfants existants terminer ou cascade cancel selon policy persistée.

## Phase 12 — Environnements et workspaces

**Objectif.** Exécuter les tasks dans des environnements isolés avec lifecycle, secret refs, resource limits et cleanup prouvés.

**Composants/fichiers.** aro-environments profiles/manager/adapters ; premier adapter local edge et container sandbox ; protocole Tauri signé.

**Schémas/migrations.** workspaces/bindings, environment_profiles/instances, secret_bindings, resource_usage, snapshots et cleanup receipts.

**Services/endpoints.** Create/get/suspend/destroy environment, bind workspace, edge pair/heartbeat/command result.

**Événements.** environment.requested/provisioning/ready/busy/quarantined/destroyed/cleanup.failed.

**Risques/dépendances.** Secret/file leakage, orphan resources, edge replay, image drift. Dépend checkpoints, policy et artifacts.

**Tests.** Filesystem/network isolation, secret audience/TTL, signed nonce, worker loss, cleanup retry, image digest, cross-environment transfer.

**Acceptation.** Aucun transfert implicite ; toute instance terminale a cleanup receipt ou alerte/quarantine ; limites CPU/mémoire/réseau appliquées.

**Rollback.** Autoriser uniquement environment local legacy pour runs legacy ; stop provisioning des nouveaux types et drain.

## Phase 13 — Modèles, tools, skills, plugins et MCP

**Objectif.** Relier le runtime à une couche de capacités unifiée et à un routage modèles provider-neutral.

**Composants/fichiers.** aro-model-gateway, CapabilityRegistry/Gateway, ToolDescriptor v1 étendu, SkillCompiler, plugin adapter integrations v3, MCP client durable. Migrer les deux tools Web en premier.

**Schémas/migrations.** capability definitions/versions/installations/health, model_calls, tool_calls/effect ledger, skill/plugin/MCP snapshots ; réutiliser integration_*.

**Services/endpoints.** Capability discovery/health, agent control tools, model routes, MCP sync/quarantine, plugin install/rollback.

**Événements.** model.call.*, tool.call.*, capability.health.*, skill.compiled, plugin.installed, mcp.synchronized.

**Risques/dépendances.** Tool non exécutable annoncé, schema injection, provider data leak, plugin/MCP malveillant. Dépend phases 9, 12 et policy.

**Tests.** Descriptor/schema contracts, provider fallback privacy, output validation, malicious MCP/plugin, health revocation, web SSRF, parallel tools.

**Acceptation.** 100 % des capacités exposées ont executor sain, version et policy ; chaque call est rattaché aux identités requises ; changement de modèle conserve PromptPackage.

**Rollback.** Alias legacy et adapters ; désactiver capability/version individuellement ; fallback provider seulement s’il était déjà autorisé.

## Phase 14 — Permissions, approvals, budgets et quotas

**Objectif.** Mettre RBAC/ABAC, délégation, confirmations exact-hash et comptabilité au centre de chaque action.

**Composants/fichiers.** aro-agent-policy, PolicyDecision store, ApprovalService, BudgetService, secret broker.

**Schémas/migrations.** policy decisions, approvals, delegations, budget accounts/reservations/ledger, quota windows et secret bindings. RLS/immutability.

**Services/endpoints.** Effective permissions, approvals list/grant/reject/revoke, budget change request, quota admin.

**Événements.** policy.denied, approval.requested/granted/rejected/consumed, budget.reserved/consumed/exhausted.

**Risques/dépendances.** TOCTOU, approval payload modifié, budget overspend concurrent, self-escalation. Dépend toutes capabilities/environments.

**Tests.** Action hash tamper, expiry/single-use, revalidation, concurrent reservations, subagent lattice, tenant/admin boundaries, redaction.

**Acceptation.** Aucun effet sans PolicyDecision fraîche ; overspend impossible sous concurrence ; UI ne peut contourner une confirmation.

**Rollback.** Mode deny-by-default pour v2 ; ne jamais revenir à allow implicit. Désactiver seulement les nouvelles actions.

## Phase 15 — Événements, triggers et scheduler

**Objectif.** Démarrer/reprendre des runs depuis user, conversation, horaire, webhook, intégration, fichier ou fin d’un autre run.

**Composants/fichiers.** EventInbox, TriggerEngine, Scheduler, outbox dispatcher et adapters d’intégrations.

**Schémas/migrations.** trigger/versions, schedules, inbox events, subscriptions, delivery attempts, unique source/external ID et next_fire indexes.

**Services/endpoints.** CRUD triggers/schedules, webhook intake signé, simulate trigger, delivery status.

**Événements.** trigger.matched/rejected, schedule.fired/misfired, event.received/deduplicated, run.triggered.

**Risques/dépendances.** Storm, replay attack, timezone/DST, catch-up infini, principal expiré. Dépend CommandService et integrations.

**Tests.** Duplicate webhooks, signatures, DST, misfire policies, bounded catch-up, revoked permissions, burst limits, virtual time days.

**Acceptation.** Un événement externe unique produit au plus une commande logique ; aucun trigger ne dépasse permission/quota.

**Rollback.** Désactiver chaque trigger/scheduler shard ; inbox conservée pour replay contrôlé.

## Phase 16 — Streaming et frontend agent

**Objectif.** Donner une vue compréhensible, reprise après reconnexion et contrôles sûrs sans exposer l’infrastructure.

**Composants/fichiers.** API SSE, projectors/read models, features/agents Svelte, store normalisé et composants conversationnels. Réduire App.svelte par extraction ciblée.

**Schémas/migrations.** Read models optionnels run_progress/task_graph/approval_inbox ; curseurs event stables.

**Services/endpoints.** Catalogue/création/détail, graph, timeline, checkpoints, costs, approvals, environments, artifacts et SSE Last-Event-ID.

**Événements.** Pas de famille UI ; consommer l’enveloppe canonique.

**Risques/dépendances.** Event gaps, PII dans timeline, optimistic UI trompeuse, conflits App.svelte. Dépend API v2 et projections.

**Tests.** Reconnexion/rattrapage, auth expiry, pagination, a11y, action confirmations, large DAG, frontend contract/E2E.

**Acceptation.** L’utilisateur comprend état, blocage et action nécessaire ; aucune démo silencieuse en production ; refresh ne perd rien.

**Rollback.** Route UI par flag vers inbox legacy ; SSE v2 reste sans impact.

## Phase 17 — Migration des agents actuels

**Objectif.** Migrer CustomAgentDefinition et runs compatibles sans big bang.

**Composants/fichiers.** LegacyAgentAdapter, backfill CLI/job, dual-read comparator, routing par tenant/agent, alias tools.

**Schémas/migrations.** Mapping custom agent → Agent + AgentVersion ; liens legacy IDs ; backfill progress/errors ; ne pas convertir les runs actifs non reprenables en faux checkpoints.

**Services/endpoints.** Endpoints legacy appellent la façade v2 pour nouveaux tenants ; réponses adaptées aux DTO historiques.

**Événements.** agent.migration.started/completed/failed, legacy.run.linked.

**Risques/dépendances.** Sémantique incompatible, double run, données partielles, tools renommés. Dépend phases 6-16.

**Tests.** Golden fixtures, dry-run, restart backfill, dual-read diffs, legacy clients, rollback routing, active run handling.

**Acceptation.** 100 % des définitions migrables avec digest ; écarts explicitement quarantined ; aucun run exécuté deux fois.

**Rollback.** Router tenant vers legacy, conserver IDs/liens et arrêter backfill ; aucune contract migration.

## Phase 18 — Résilience et suite de tests complète

**Objectif.** Prouver crash recovery, idempotence, isolation et longue durée avant canary.

**Composants/fichiers.** Harness chaos, fake clock, deterministic providers/tools, fault injection DB/Redis/network/object/Qdrant, load generator.

**Schémas/migrations.** Fixtures partitionnées et datasets multi-tenant ; vérifier chaque migration N/N-1.

**Services/endpoints.** Test-only fault controls exclus des builds production.

**Événements.** Assertions de séquence/causalité et absence de trous.

**Risques/dépendances.** Tests lents/flaky, couverture d’effets insuffisante. Dépend toutes les phases fonctionnelles.

**Tests.** Unit, integration, contract, state, crash à chaque frontière, duplication, concurrency, subagents, environments, permissions, quotas, days via virtual time, load, deploy during run, outages et E2E.

**Acceptation.** Matrice exigences/tests complète, zéro flaky toléré, mutation tests sur reducers/policy, seuils de charge respectés.

**Rollback.** Les harness sont isolés ; quarantiner seulement avec issue, owner et délai court.

## Phase 19 — Observabilité et opérations

**Objectif.** Rendre chaque run/task/step/call causalement traçable et détecter blocage, boucle, duplication et coût anormal.

**Composants/fichiers.** OpenTelemetry, métriques Prometheus, dashboards, alerts, redaction library, runbooks, admin diagnostics.

**Schémas/migrations.** Metric rollups et retention si nécessaire ; aucun contenu brut en télémétrie.

**Services/endpoints.** Health/readiness worker/scheduler/projector, run diagnostics autorisé, queue/quarantine ops.

**Événements.** operational alerts distincts des événements métier.

**Risques/dépendances.** Cardinalité, coût, PII/secrets, alert fatigue. Dépend IDs et événements stables.

**Tests.** Trace propagation, redaction canaries, cardinality budgets, alerts synthetic, stuck/loop/cost detectors.

**Acceptation.** Dashboards/SLO/runbooks ont owner ; un incident simulé est diagnostiqué sans accès DB manuel ni contenu sensible.

**Rollback.** Sampling et exporters désactivables ; métriques critiques locales conservées.

## Phase 20 — Déploiement progressif

**Objectif.** Activer v2 par environnement, tenant, agent et capability avec shadow read, canary et ramp-up.

**Composants/fichiers.** RuntimeRouter, rollout policies, canary workers, compatibility matrix N/N-1, deployment/runbook.

**Schémas/migrations.** Expand achevé ; backfills reprenables ; aucune contract migration pendant ramp-up.

**Services/endpoints.** Admin rollout/status/pause/rollback avec audit et double contrôle.

**Événements.** rollout.started/advanced/paused/aborted/completed.

**Risques/dépendances.** Double effet en shadow, schema skew, métriques non comparables. Dépend phases 17-19.

**Tests.** 1/5/25/50/100 %, auto-abort, worker N/API N-1, rolling deploy, drain, restore, rollback.

**Acceptation.** Aucun shadow write ; chaque palier tient les SLO/coûts/sécurité pendant la fenêtre définie ; abort automatique prouvé.

**Rollback.** Stop claims, drain, router legacy pour nouveaux runs, continuer/reprendre v2 existants avec version compatible ; image N-1 supportée.

## Phase 21 — Durcissement et préparation production

**Objectif.** Fermer les exigences sécurité, conformité, capacité, DR, support et on-prem avant GA.

**Composants/fichiers.** KMS/secret rotation, SBOM/signatures, sandbox/egress, HA/capacity, backups, incident response, privacy workflows et documentation opérateur.

**Schémas/migrations.** RLS forcée partout, partition/retention/archive mesurés, deletion receipts, key versions ; contract migrations seulement après fin du rollback applicatif.

**Services/endpoints.** Ops minimales authentifiées, export/delete, sessions/revocations nécessaires, support bundle redacted.

**Événements.** security/privacy/retention/backup events avec accès restreint.

**Risques/dépendances.** Lockout RLS, rotation, supply chain, capacity/provider, on-prem drift. Dépend toutes phases.

**Tests.** Pen test, tenant escape, SSRF/sandbox, prompt injection, secret exfiltration, soak/peak, DR, backup restore, provider/region outage, tabletop incident et install on-prem air-gapped selon cible.

**Acceptation.** Les 14 gates de production de l’architecture sont signés engineering, security, product et operations ; aucun high/critical ouvert ; go/no-go documenté.

**Rollback.** Version N-1, restauration et compatibilité données exercées ; dual-key decrypt ; break-glass audité ; packages/capabilities révocables.

## 3. Première tranche d’implémentation après validation

La première tranche doit rester étroite et additive :

1. rendre verts les tests aro-runtime désynchronisés et le test de façade frontend, en distinguant clairement ces corrections de baseline ;
2. figer ADR-001 à ADR-005 et créer aro-agent-domain avec IDs, états, commands/events et reducers purs ;
3. ajouter les tests exhaustifs de transitions et de canonical hash ;
4. créer une première migration v2 limitée à agent_commands, agent_events v2, agent_runs v2, agent_tasks et task_attempts, avec RLS et grants ;
5. construire un worker d’activation réel mais utilisant un DeterministicTestExecutor, sans router le trafic utilisateur ;
6. prouver double claim, fencing, kill/recovery au début d’une task et perte Redis ;
7. exposer uniquement des diagnostics internes sous flag ;
8. présenter les résultats avant de raccorder POST /v1/agents/{id}/runs.

Cette tranche ne modifie ni l’UI principale, ni le modèle mémoire, ni les intégrations. Elle établit le noyau fiable dont toutes les phases suivantes dépendent.

## 4. Définition de done commune

Une phase n’est terminée que si :

- code, migration, API/event schemas et documentation sont alignés ;
- tests unitaires/intégration/contrat/sécurité proportionnés passent ;
- permissions, tenant, idempotence, rétention et redaction ont été revus ;
- métriques et logs nécessaires existent ;
- feature flag et rollback ont été exercés ;
- aucun échec hors périmètre n’est masqué ou attribué au changement ;
- le diff reste limité au périmètre annoncé.
