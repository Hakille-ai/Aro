# Architecture cible du runtime d’agents ARO

Statut : contrat d’architecture préalable à l’implémentation.

## 1. Décisions structurantes

1. PostgreSQL est la source de vérité des définitions, états, transitions, checkpoints, effets, budgets et événements.
2. Un moteur interne de machine à états durable exécute des activations courtes de tâches ; aucun run multi-jour ne monopolise un process.
3. Redis peut réveiller les workers, limiter la concurrence et mettre en cache. La perte totale de Redis ne doit perdre ni travail ni état.
4. Le control plane reste un monolithe modulaire Rust/Axum. Les workers, scheduler, projecteurs et edge executors sont des rôles/processus séparés déployables indépendamment.
5. Toutes les mutations passent par des commandes idempotentes et un reducer de transitions unique.
6. Un event log append-only décrit les faits ; des projections servent l’API et l’UI. L’event log n’est pas utilisé comme event sourcing intégral du contenu sensible.
7. Chaque action de modèle, tool, skill, plugin ou MCP est un StepAttempt rattaché à agent, version, run, task, step, tenant, actor, policy et environnement.
8. Les checkpoints sont immuables, versionnés, chiffrés, vérifiables et séparés des logs.
9. Les permissions sont évaluées hors du modèle. Un approval autorise le hash exact d’une action, pas une intention vague.
10. SaaS et on-prem utilisent les mêmes contrats. Les adapters de secrets, stockage, modèles et exécuteurs changent par déploiement.

## 2. Modèle conceptuel canonique

| Concept | Responsabilité | Ne contient pas |
| --- | --- | --- |
| AgentTemplate | recette publiable et réutilisable avec paramètres | état utilisateur, exécution |
| Agent | identité durable, ownership, lifecycle et pointeur de version active | historique mutable du prompt, état de run |
| AgentVersion | snapshot immuable des instructions, capabilities, policies, routage, mémoire, limites et schémas | progression |
| AgentInstance | binding durable optionnel d’un AgentVersion à un workspace, environnement, conversation ou intégration | exécution précise |
| Run | mission précise d’une version, avec objectif, budget, état global et résultat | détails de chaque tentative |
| Session | fenêtre logique d’interaction et de continuité dans un run ; peut survivre à plusieurs activations | identité de l’agent |
| Task | unité durable de travail dans un DAG, isolée en contexte et budget | sous-étapes techniques |
| Subtask | Task ayant parent_task_id ; même contrat et mêmes invariants | modèle parallèle spécial |
| Step | intention atomique planifiée : appel modèle, tool, attente, vérification, checkpoint ou synthèse | tentative mutable |
| StepAttempt | essai d’un Step avec lease, input snapshot, résultat/erreur et usage | définition de l’étape |
| Plan | identité du plan d’un run | contenu mutable |
| PlanVersion | snapshot immuable du DAG, contraintes et justification ; une replanification crée une version | exécution |
| Checkpoint | snapshot restaurable du run ou d’une task à une frontière sûre | secrets/logs bruts |
| Memory | connaissance durable typée, versionnée, avec scope, provenance, ACL, confiance et TTL | contexte de travail implicite |
| Workspace | ressource durable contenant références de dépôts/fichiers et politique d’accès | process actif |
| Environment | instance isolée d’exécution avec type, lifecycle, limites, secret refs et état | secrets en clair |
| ToolCall | invocation normalisée d’une capability et ledger de son effet | raisonnement libre |
| ModelCall | invocation provider-neutral avec route choisie, usage, coût et résultat normalisé | clé fournisseur |
| Event | fait immuable, versionné, causal et redacted | source de secrets |
| Artifact | sortie adressable, typée, hashée, avec provenance et retention | blob forcément inline |
| Approval | décision durable sur un action_hash, bornée, expirante et single-use | élévation permanente implicite |
| Trigger | règle événementielle versionnée qui propose un run | état d’exécution |
| Schedule | calendrier/fuseau/fenêtre/misfire policy d’un Trigger | worker |
| Result | sortie finale ou intermédiaire structurée et versionnée d’un run/task | message UI seulement |
| Budget | limites réservées/consommées coût, tokens, temps, steps, appels et ressources | simple compteur local |
| Quota | politique partagée user/org/agent/capability/environnement | budget d’un run |
| Delegation | relation parent/enfant avec envelope de permissions, budget et profondeur | droit d’escalade |

### Identités et immutabilité

- AgentVersion, PlanVersion, Checkpoint, Event, StepAttempt input, Approval decision et Result publié sont immuables.
- Agent, AgentInstance, Run, Task et Environment ont une colonne version pour optimistic concurrency.
- Les identifiants sont UUIDv7/ULID ordonnables si la bibliothèque retenue le permet ; l’ordre métier vient toujours de sequence/occurred_at, jamais du seul UUID.
- Toutes les ressources privées portent organization_id ; les ressources personnelles portent owner_user_id. Les clés étrangères tenant-scopées sont composites.

## 3. Architecture logique

~~~mermaid
flowchart TB
  subgraph Edge[Edge plane]
    UI[Desktop et Web]
    EE[Edge Executor appairé]
    LE[Environnement local]
  end

  subgraph Control[Control plane Rust]
    API[Agent API et SSE]
    CMD[Command Service]
    DEF[Agent Definition Service]
    POL[Policy et Approval]
    TRG[Trigger et Scheduler]
    CTX[Context et Memory]
    CAP[Capability Registry]
    PRJ[Projectors]
  end

  subgraph Data[Data plane]
    DB[(PostgreSQL)]
    OUT[(Outbox)]
    CACHE[(Redis optionnel)]
    OBJ[(Object storage)]
    VEC[(Vector index)]
  end

  subgraph Execution[Execution plane]
    W[Runtime Workers]
    ENV[Environment Manager]
    MOD[Model Gateway]
    TOOL[Tool Skill Plugin MCP Gateway]
    REC[Reaper et Recovery]
  end

  UI --> API
  API --> CMD
  CMD --> DB
  DEF --> DB
  POL --> DB
  TRG --> DB
  CTX --> DB
  CAP --> DB
  DB --> OUT
  OUT --> CACHE
  CACHE -. réveil .-> W
  W -->|claim avec fencing| DB
  W --> CTX
  W --> POL
  W --> MOD
  W --> TOOL
  W --> ENV
  ENV --> EE
  EE --> LE
  W --> OBJ
  CTX --> VEC
  PRJ --> DB
  REC --> DB
  API -->|SSE depuis curseur| DB
~~~

### Modules Rust cibles

| Module/crate | Rôle |
| --- | --- |
| aro-agent-domain | types, invariants, états, commandes, événements, errors |
| aro-agent-store | repositories et transactions PostgreSQL, sans logique HTTP |
| aro-agent-runtime | reducer, planner, activation runner, recovery et budgets |
| aro-agent-context | scope resolver, retrieval, redaction et prompt assembly |
| aro-agent-policy | RBAC/ABAC, approvals, delegation envelope et data routing |
| aro-capabilities | catalogue unifié tools/skills/plugins/MCP |
| aro-environments | lifecycle et adapters local/container/remote/browser |
| aro-model-gateway | routage provider-neutral, fallback et cost ledger |
| apps/api agent module | DTO, auth, commandes et SSE uniquement |
| apps/worker | claim, heartbeat, activation, reaper et projecteurs |
| desktop edge module | exécution locale appairée, consentement et attestations |

Les crates peuvent être introduites progressivement ; les interfaces sont la contrainte, pas le découpage physique initial.

## 4. Cycle de vie d’un agent

~~~mermaid
flowchart LR
  T[Template optionnel] --> D[Agent draft]
  D --> V[AgentVersion immuable]
  V --> A[Agent actif]
  A --> I[AgentInstance optionnelle]
  A --> R[Run créé]
  I --> R
  R --> P[PlanVersion]
  P --> Q[Tasks prêtes]
  Q --> X[Activations workers]
  X --> C[Checkpoints et événements]
  C --> X
  X --> W[Attente user event resource]
  W --> X
  X --> S[Résultat final]
  S --> M[Résumé et mémoires proposées]
  M --> Z[Run archivé]
~~~

Création depuis une conversation ou un tool :

1. le modèle appelle agents.create avec un draft structuré ;
2. le backend valide schémas, ownership, capabilities, routage, budgets et permissions ;
3. toute capacité sensible produit un aperçu et éventuellement un Approval ;
4. la création publie une AgentVersion immuable ;
5. agents.start crée Run, Session, PlanVersion initiale, Task racine, budget reservations et événement dans une transaction ;
6. l’outbox réveille les workers après commit.

## 5. Machines à états

### 5.1 Run

États :

created, queued, starting, running, waiting_for_tool, waiting_for_event, waiting_for_user, waiting_for_resource, paused, retrying, recovering, completing, completed, failed, cancelled, expired.

~~~mermaid
stateDiagram-v2
  [*] --> created
  created --> queued: start
  queued --> starting: claim
  starting --> running: activation_started
  running --> waiting_for_tool
  running --> waiting_for_event
  running --> waiting_for_user
  running --> waiting_for_resource
  waiting_for_tool --> running
  waiting_for_event --> queued: matching_event
  waiting_for_user --> queued: instruction_or_approval
  waiting_for_resource --> queued: resource_available
  running --> retrying: transient_failure
  retrying --> queued: backoff_elapsed
  running --> recovering: lease_lost_or_restore
  recovering --> queued: checkpoint_valid
  created --> cancelled
  queued --> paused
  running --> paused: safe_boundary
  paused --> queued: resume
  running --> completing
  completing --> completed
  created --> expired
  queued --> expired
  running --> failed
  recovering --> failed
  created --> failed
  created --> cancelled
  queued --> cancelled
  running --> cancelled: cooperative_cancel
  paused --> cancelled
  completed --> [*]
  failed --> [*]
  cancelled --> [*]
  expired --> [*]
~~~

waiting_for_tool est une projection utile pour l’UI ; le ToolCall possède sa propre tentative et lease. Un run ne reprend jamais directement de completed, failed, cancelled ou expired. Retry manuel d’un run terminal crée un nouveau Run avec resumed_from_run_id.

### 5.2 Task

draft, blocked, ready, claimed, running, waiting, paused, retry_wait, succeeded, failed, cancelled, skipped, compensated.

- draft vers blocked/ready lors de la validation du PlanVersion ;
- ready vers claimed exige dépendances satisfaites, budget réservé, policy valide et slot de concurrence ;
- claimed vers running exige lease valide ;
- running vers succeeded/failed/waiting se fait atomiquement avec résultat, événements et prochain scheduling ;
- failed vers retry_wait seulement si classification retryable et attempts restantes ;
- une task terminale est immutable ; retry manuel crée une nouvelle tentative ou task de reprise selon la sémantique ;
- fan-in devient ready lorsque la condition all/any/quorum/expression est satisfaite.

### 5.3 Step et StepAttempt

Step : planned, ready, executing, waiting, succeeded, failed, cancelled, skipped.

StepAttempt : leased, started, effect_prepared, effect_committed, result_recorded, failed, abandoned.

Une nouvelle tentative n’écrase jamais l’ancienne. Le lease_generation et le lease_token doivent correspondre pour écrire. effect_committed sans result_recorded déclenche une réconciliation, pas une répétition aveugle.

### 5.4 Approval

requested, granted, rejected, expired, revoked, consumed.

granted vers consumed exige même action_hash, actor autorisé, non-expiration et revalidation des permissions. Un payload modifié crée un nouvel Approval.

### 5.5 Environment

requested, provisioning, ready, busy, suspended, draining, destroying, destroyed, failed, quarantined.

Chaque transition est auditée. destroyed est terminal. Une réutilisation crée une nouvelle Environment instance, même si l’image/workspace est identique.

### 5.6 Contrat de transition

Chaque commande contient command_id, idempotency_key, expected_version, tenant, actor, subject, requested_at et payload versionné. Le transaction handler :

1. verrouille la projection ;
2. charge l’état et vérifie tenant/version ;
3. réévalue policy/quota ;
4. applique le reducer pur ;
5. écrit projection, événements, outbox et réservations dans la même transaction ;
6. renvoie le résultat déjà produit si la commande est rejouée.

## 6. Choix du moteur durable

| Option | Atouts | Limites pour ARO | Décision |
| --- | --- | --- | --- |
| Temporal | historique/replay, timers, signals, activités | SDK Rust encore Public Preview ; nouvelle plateforme opérationnelle ; frontières avec transactions PostgreSQL ARO | réévaluer à la GA Rust ou avec orchestrateur séparé accepté |
| Celery | mature et vaste écosystème | runtime Python, broker/backend supplémentaire, pas un modèle agent complet, cohérence applicative séparée | rejeté |
| Dramatiq | modèle simple, retries, Redis/RabbitMQ | runtime Python et livraison au moins une fois ; workflow/checkpoints à construire | rejeté |
| arq | simple asyncio/Redis | Python, Redis source critique et projet en mode maintenance | rejeté |
| Redis Streams | consumer groups, pending entries, ack | transport durable selon config, pas machine à états ni transaction atomique avec données ARO | réveil optionnel seulement |
| NATS JetStream | durable consumers, replay, faible latence | broker supplémentaire ; ne résout pas état métier/checkpoints/transactions | futur bus wakeup/event si besoin |
| Kafka | log durable, scale, replay | forte charge opérationnelle ; exactly-once limité au contexte Kafka ; externe à la transaction métier | futur analytics/event backbone, pas runtime initial |
| moteur PostgreSQL interne | réutilise queue, leases, transactions, équipe Rust, SaaS/on-prem simple | il faut construire reducer, checkpoints, timers, DAG et outils ops | retenu |

Sources primaires : [Temporal Rust SDK](https://github.com/temporalio/sdk-rust), [Celery brokers/backends](https://docs.celeryq.dev/en/stable/getting-started/backends-and-brokers/index.html), [Dramatiq guide](https://dramatiq.io/guide.html), [arq](https://github.com/python-arq/arq), [Redis Streams](https://redis.io/docs/latest/develop/data-types/streams/), [NATS JetStream consumers](https://docs.nats.io/nats-concepts/jetstream/consumers), [Kafka design](https://kafka.apache.org/41/design/design/).

### Critères de sortie de la décision

Un ADR réévalue Temporal/NATS/Kafka si deux des conditions suivantes apparaissent : plus de 10 000 activations/s soutenues, besoins multi-région actifs-actifs, timers supérieurs à 10 millions, équipe plateforme dédiée, workflows externes polyglottes, ou SDK Rust Temporal GA avec support opérationnel accepté.

## 7. Modèle d’exécution durable

Un Run multi-jour est une suite d’activations bornées, jamais un job multi-jour :

- une activation traite une Task ou une frontière d’attente ;
- durée cible inférieure à 5 minutes, limite dure configurable ;
- heartbeat toutes les 10 à 30 secondes ;
- checkpoint obligatoire avant expiration du quantum ;
- yield volontaire lorsque budget/temps approche ;
- le scheduler rend une Task ready après événement, timer, approval ou dépendance ;
- le reaper rend récupérable toute activation dont le lease expire ;
- le worker charge uniquement des références et snapshots nécessaires.

PostgreSQL utilise SELECT FOR UPDATE SKIP LOCKED, lease_until, lease_generation et compare-and-swap. Redis Pub/Sub ou Streams peut réduire la latence de poll ; un poll PostgreSQL périodique garantit le progrès.

## 8. Modèle de données

### 8.1 Définitions

| Table | Colonnes essentielles | Contraintes/index |
| --- | --- | --- |
| agent_templates | id, org, owner, slug, visibility, status | unique tenant/slug, RLS |
| agent_template_versions | template_id, version, spec_json, schema_version, digest | unique template/version, immuable |
| agents | id, org, owner, name, status, active_version_id, version | FK composite tenant, optimistic version |
| agent_versions | agent_id, version, instructions_ref, capability_policy, model_policy, memory_policy, limits, digest | unique agent/version, immuable |
| agent_instances | agent_id/version, conversation_id, workspace_id, default_environment_profile_id, status | index conversation/workspace |

### 8.2 Exécution

| Table | Colonnes essentielles | Contraintes/index |
| --- | --- | --- |
| agent_runs | id, agent/version, instance, session, goal_ref, status, priority, current_plan_version, checkpoint, result, budgets, command_version | indexes tenant/status/priority, terminal_at |
| agent_sessions | run, conversation, status, started/ended, context_policy | une session active selon policy |
| agent_tasks | run, parent_task, kind, objective_ref, status, priority, ready_at, environment, budgets, timeout, retry_policy, lease fields, result | indexes ready claims, parent, run/status |
| agent_task_dependencies | run, task, depends_on, condition, group | unique edge, no self-edge, cycle validé |
| agent_steps | task, plan_version, ordinal, kind, status, input_ref, policy_ref | unique task/plan/ordinal |
| agent_step_attempts | step, attempt, worker, lease generation, input snapshot, status, timestamps, error/result/usage | unique step/attempt |
| agent_plans | id, run | unique run/plan |
| agent_plan_versions | plan, version, graph_json, rationale_ref, digest, created_by | immutable |
| agent_results | subject_type/id, schema, payload_ref, digest, finality | unique final par sujet/version |

### 8.3 Reprise et effets

| Table | Rôle |
| --- | --- |
| agent_checkpoints | snapshot chiffré, digest, runtime/schema versions, parent checkpoint, safe_restore flag |
| agent_checkpoint_artifacts | références hashées vers artifacts |
| agent_tool_calls | capability version, canonical input digest, idempotency key, policy, environment, effect class, status |
| agent_effect_ledger | prepare/commit/verify/compensate, provider key, external ref, reconciliation status |
| agent_model_calls | route policy, provider/model, prompt package digest, response ref, tokens, coût, latency, error |
| agent_errors | taxonomy, retryable, source, sanitized details, causal refs |
| agent_retry_schedules | subject, attempt, due_at, backoff, reason |

### 8.4 Contexte, mémoire et ressources

| Table | Rôle |
| --- | --- |
| context_items | scope_type/id, type, content_ref, sensitivity, prompt_policy, expiry, provenance |
| context_links | héritage/partage explicite entre scopes |
| memories et memory_versions | connaissance versionnée, confiance, salience, TTL, contradiction |
| memory_acl | principal, permissions search/read/write/share/delete |
| memory_sources/relations | provenance et liens supersedes/contradicts/derives |
| workspaces/workspace_bindings | ressource, repo/file refs, isolation et ownership |
| environment_profiles | définition immutable de type/image/limits/policy |
| environments | instance de lifecycle, worker/edge binding et state |
| environment_secret_bindings | références opaques, audience et expiry |
| artifacts/artifact_links | metadata, blob ref, digest, provenance, retention |

### 8.5 Contrôle, événements et gouvernance

| Table | Rôle |
| --- | --- |
| approvals | action hash, preview redacted, scope, expiry, decision/consumer |
| agent_events | enveloppe canonique append-only, payload redacted/ref |
| agent_commands | déduplication et résultat de commande |
| agent_triggers/trigger_versions | règle versionnée, filtre, principal et template de run |
| agent_schedules | cron/interval, timezone, misfire, jitter, next_fire_at |
| agent_inbox_events | événement entrant dédupliqué et statut de dispatch |
| budget_accounts/reservations/ledger | limites, réservation, consommation et release |
| quota_policies/quota_windows | limites hiérarchiques |
| delegations | parent/child, permission digest, budgets, profondeur |
| audit_events/outbox_events | preuves utilisateur/admin et publication fiable |

### 8.6 Règles SQL

- toutes les tables tenant-scopées activent puis forcent RLS ; rôles runtime NOBYPASSRLS ;
- contraintes composites interdisent les FK cross-tenant ;
- CHECK sur tous les états, montants non négatifs et bornes ;
- index partiels sur tasks ready, approvals requested, environments busy et retry due ;
- BRIN/partition mensuelle pour agent_events, model/tool calls et ledger à volume élevé ;
- payloads volumineux dans object storage chiffré, PostgreSQL garde ref/digest/metadata ;
- événements/audit non modifiables par rôles runtime ;
- effacement RGPD via tombstone et crypto-shredding des payloads personnels, sans falsifier les métadonnées d’audit ;
- export par workflow durable produisant un artifact expirant ;
- migrations expand/backfill/switch/contract avec compatibilité N/N-1.

## 9. Checkpoints

### 9.1 Enveloppe minimale

CheckpointEnvelope v1 contient :

- checkpoint_id, organization_id, agent_id, agent_version_id, run_id, session_id ;
- task_id et step_id courants, attempt et lease_generation observée ;
- plan_id, plan_version, plan_digest ;
- tâches terminées, actives, restantes et dépendances satisfaites ;
- état reducer du run/task ;
- mémoire de travail résumée par scope, avec références et digests ;
- artifacts et résultats utiles ;
- erreurs récupérables et retry schedule ;
- effets prepared/committed/unverified et external references ;
- événements consommés et curseurs d’inbox ;
- approvals en attente ;
- réservations et consommation budget/coût/tokens/temps ;
- environment profile/instance et workspace snapshot refs ;
- model route policy et prompt package digest ;
- runtime_version, schema_version, created_at, previous_checkpoint_id ;
- encryption_key_version, payload_digest et MAC/signature d’intégrité.

Le checkpoint n’inclut jamais credential, secret, token, prompt source sensible brut ou blob d’artifact.

### 9.2 Fréquences

- après chaque Step critique ;
- avant un effet externe : checkpoint PREPARED ;
- après effet : ledger COMMITTED puis checkpoint ;
- après replanification ;
- avant pause, deploy drain et changement d’environnement ;
- avant expiration du quantum ;
- périodiquement selon temps ou volume pour une tâche longue ;
- avant attente user/event/resource.

### 9.3 Création atomique

Dans une transaction :

1. vérifier lease et version ;
2. écrire snapshot chiffré et digest ;
3. mettre à jour current_checkpoint_id ;
4. enregistrer event run.checkpoint.created ;
5. mettre à jour task/run et outbox.

Les blobs déjà écrits sont content-addressed et garbage-collected après délai s’ils ne sont jamais référencés.

### 9.4 Reprise

1. le reaper marque l’activation abandoned ;
2. le recovery service choisit le dernier checkpoint safe_restore et vérifie tenant, digest, MAC, versions et références ;
3. il migre le snapshot avec des migrators purs versionnés ;
4. il réconcilie chaque effet non terminé :
   - prepared sans preuve : vérifier fournisseur avant retry ;
   - committed sans résultat : récupérer par external_ref/idempotency key ;
   - effet inconnu non idempotent : waiting_for_user ;
5. il revalide permissions, quotas, agent version disponible et environnement ;
6. il crée une nouvelle StepAttempt avec generation supérieure ;
7. il émet run.recovery.started puis completed/blocked/failed.

La reprise est déterministe sur l’état interne. Les modèles et systèmes externes ne sont pas rejoués ; leurs sorties validées sont réutilisées par référence.

### 9.5 Retour arrière

Un rollback vers un checkpoint antérieur crée une branche de reprise :

- autorisé uniquement si aucun effet ultérieur irréversible non compensé ;
- conserve toute l’histoire ;
- crée un nouveau PlanVersion et un événement checkpoint.restored ;
- ne décrémente jamais le cost ledger ;
- exige approval pour supprimer/masquer des résultats déjà publiés.

## 10. Multi-tâches et orchestration

Le PlanVersion est un DAG validé : pas de cycle, limites de taille/profondeur, types d’edges success/failure/always/condition/quorum. Chaque Task possède son scope de contexte, workspace binding, environment, capability envelope, budget et retry policy.

Ordonnancement :

- priorité effective = priorité de base + aging borné ;
- quotas globaux, tenant, user, agent, run, capability et environnement ;
- fairness par weighted round-robin au-dessus de SKIP LOCKED ;
- fan-out crée des Tasks bornées par max_fanout ;
- fan-in attend all/any/quorum/expression déterministe ;
- tasks conditionnelles évaluent une expression sûre sur Results typés ;
- annulation coopérative puis fencing ; les tâches non démarrées passent cancelled/skipped ;
- compensation est une Task explicite, jamais un callback caché.

Une Task indépendante ne voit pas la mémoire de travail d’une autre. Le partage exige un Result/Artifact publié dans un context scope partagé et un ContextLink autorisé.

## 11. Isolation du contexte

### 11.1 Scopes

| Scope | Héritage par défaut | Partage | Prompt |
| --- | --- | --- | --- |
| organisation | non | grants explicites | seulement policy/knowledge autorisée |
| utilisateur | non cross-user | consentement | profil minimal pertinent |
| agent | vers ses runs | version figée | instructions et mémoire autorisée |
| conversation | vers session liée | participants autorisés | extraits/résumé pertinent |
| run | vers sessions/tasks | interne au run | objectif, plan, contraintes |
| session | vers ses tasks | interne | dernières interactions/résumé |
| task | vers subtasks par whitelist | aucun sibling implicite | objectif et dépendances utiles |
| subtask | depuis parent filtré | résultat remonté explicitement | strict minimum |
| environment | seulement task liée | jamais entre env par défaut | metadata non secrète |
| model ephemeral | aucun | jamais | supprimé après appel sauf refs |

### 11.2 ContextItem

Chaque item porte scope, type, provenance, sensitivity, confidence, created/expiry, prompt_policy, retention, ACL et content_ref. prompt_policy vaut allowed, summarize, redact, reference_only ou forbidden.

### 11.3 Construction

Le ContextAssembler reçoit TaskContextRequest avec objectif, modèle ciblé, budget tokens, capabilities et data residency. Il :

1. résout les scopes autorisés ;
2. sélectionne état du plan et dépendances ;
3. fait un retrieval lexical/vectoriel filtré en SQL par tenant/scope/ACL ;
4. déduplique et traite contradictions ;
5. applique sensibilité, redaction et routing provider ;
6. réserve un budget tokens par section ;
7. produit PromptPackage provider-neutral avec provenance et digest ;
8. enregistre seulement refs, sélection et digest.

## 12. Mémoire

Types : working, episodic, semantic, project, authorized_user, organization, decision, action_history, run_summary, lesson.

Une écriture passe par propose, validate, deduplicate/merge, approve si sensible, persist, project. Les sorties modèle ne deviennent jamais automatiquement une vérité organisationnelle.

Les ACL accordent search/read/write/share/delete. Le partage entre agents est refusé par défaut. TTL, confidence et validity interval permettent l’expiration. Les relations supports, contradicts, supersedes et derives conservent la provenance. Qdrant est une projection reconstructible alimentée par outbox ; PostgreSQL reste autoritatif.

## 13. Sous-agents

Un sous-agent est un nouveau Run relié par Delegation et parent_run_id/parent_task_id, pas un thread caché dans le parent.

DelegationEnvelope contient :

- agent/version enfant autorisé ou draft validé ;
- objectif et Result schema ;
- intersection des permissions parent, user, org et trigger ;
- capabilities et environnements autorisés ;
- budget réservé, timeout, profondeur et max descendants ;
- context manifest explicitement transmis ;
- can_spawn_subagents, communication policy et termination criteria.

Garde-fous :

- profondeur et nombre total atomiquement réservés ;
- budget enfant prélevé sur le parent ;
- aucune permission implicite ou wildcard ajoutée ;
- signature de délégation et digest ;
- détection de boucle sur chaîne agent/version/goal digest ;
- limites de fan-out, durée et absence de progrès ;
- messages parent/enfant via inbox événementielle persistée ;
- résultat enfant validé contre schema avant publication au parent ;
- annulation parent configurable cascade/detach, jamais ambiguë.

## 14. Environnements et workspaces

EnvironmentProfile décrit type local_edge, container, sandbox, browser, remote, cloud ou enterprise, image/digest, resources, filesystem policy, network policy, secret audiences, max lifetime et cleanup.

EnvironmentManager :

1. provisionne de façon idempotente ;
2. atteste image/agent edge ;
3. monte un Workspace en lecture/écriture selon policy ;
4. injecte des secret refs à courte durée via broker ;
5. collecte logs redacted et resource usage ;
6. snapshotte si policy l’autorise ;
7. détruit et produit une cleanup receipt.

Le passage entre environnements crée une nouvelle Step et un checkpoint. Aucun fichier, variable ou secret ne traverse sans Artifact/ContextItem explicitement publié, scanné et autorisé.

L’exécuteur desktop edge utilise pairing, device identity, commandes signées, nonce anti-replay, confirmation locale si nécessaire et attestations de résultat. Le backend ne suppose jamais qu’un workspace local est encore disponible.

## 15. Modèles

ModelPolicy versionnée contient routes candidates, capacités requises, coût/latence/qualité, résidence, sensibilité maximale, contexte, fallback, retry et plafond d’usage.

ModelRouter choisit par Task, pas uniquement par Agent. Il réserve le budget, vérifie la data policy, sélectionne provider/model, construit son adapter et persiste ModelCall. Un fallback :

- ne traverse jamais une frontière local/cloud ou de région sans autorisation ;
- réutilise PromptPackage provider-neutral ;
- crée une nouvelle tentative ;
- n’écrase pas la réponse précédente ;
- limite provider/model loops ;
- peut réduire le contexte ou demander intervention.

Les messages provider-neutral séparent system instructions, user content, tool schemas, tool results et provenance. Les formats propriétaires sont des projections éphémères.

## 16. Tools, skills, plugins et MCP

CapabilityDescriptor unifie identity, version, schemas, risk, permissions, execution kind, environments, idempotency, side effects, retry, limits, health, provenance et lifecycle.

- Tool : opération atomique exécutable.
- Skill : stratégie/version de prompt ou workflow compilé en Plan fragment.
- Plugin : package signé fournissant capabilities, resources et migrations contrôlées.
- MCP : capabilities distantes découvertes et figées dans un snapshot de version.

Le pipeline discover → validate → authorize → rank → expose n’envoie au modèle que les capabilities utiles. Chaque invocation passe par CapabilityGateway, PolicyEngine, Environment binding, ToolCall, EffectLedger, output schema validation et Artifact scanner.

Les tools de contrôle agents.create/read/update/clone/delete/start/pause/resume/stop/task.create/task.assign/run.status/run.checkpoint/subagent.spawn utilisent exactement le Command Service public et les mêmes permissions que l’API. Un modèle ne bénéficie d’aucun chemin privilégié.

## 17. Permissions, approvals et secrets

La décision effective est l’intersection de :

- rôle et ownership user/org ;
- AgentVersion capability policy ;
- Trigger principal ;
- DelegationEnvelope ;
- Task et Environment policies ;
- classification des données et destination ;
- risk/side effects du descriptor ;
- budget/quota et autonomie.

PolicyDecision est persistée avec policy_version, facts_digest, allow/deny/approval, obligations et expiry. Elle est réévaluée juste avant l’effet.

Approval action_hash couvre capability/version, input canonique, effect preview, tenant, actor, environment, destinations et policy digest. Il est single-use, expire et ne se délègue pas pour critical.

Les secrets sont des SecretRef opaques avec audience capability/environment/provider, TTL et version. Ils ne sont jamais renvoyés au modèle, au checkpoint, au journal ou au frontend.

## 18. Événements, triggers et schedules

EventEnvelope v1 :

- event_id, event_type, event_version ;
- organization_id, actor_ref, subject_type/id ;
- correlation_id, causation_id, command_id ;
- occurred_at, recorded_at ;
- sensitivity, payload_schema, payload ou payload_ref, digest ;
- source et trace_context.

Familles minimales :

- agent.created/updated/version.published/archived ;
- run.created/queued/started/state.changed/checkpoint.created/recovery.started/completed/failed/cancelled ;
- task.created/ready/claimed/started/waiting/completed/failed/retried/compensated ;
- model.call.started/completed/failed ;
- tool.call.prepared/started/committed/verified/failed/compensated ;
- approval.requested/granted/rejected/expired/consumed ;
- subagent.spawned/message.sent/completed/stopped ;
- environment.requested/ready/quarantined/destroyed ;
- budget.reserved/consumed/exhausted/released.

Les événements entrants ont source, external_event_id et digest uniques. TriggerEngine filtre avec une DSL bornée, vérifie principal/policy, déduplique puis émet une commande StartRun. Les Schedule utilisent timezone IANA, misfire policy skip/fire_once/catch_up_bounded, jitter et next_fire_at verrouillé. Email, calendrier, GitHub et webhooks passent par les intégrations versionnées, jamais par un handler agent spécial.

## 19. Idempotence, erreurs et récupération

### Effets

Pour toute capability à effet :

1. canonicaliser input et calculer effect_key ;
2. réserver l’effet avec unique tenant/capability/effect_key ;
3. checkpoint PRE_EFFECT ;
4. appeler le fournisseur avec sa clé d’idempotence si disponible ;
5. enregistrer external_ref et COMMITTED ;
6. vérifier le résultat ;
7. checkpoint POST_EFFECT ;
8. publier Result.

### Taxonomie

| Classe | Retry | Réponse |
| --- | --- | --- |
| transient | oui avec jitter et plafond | même route puis fallback |
| provider/model | borné | autre route si policy |
| tool/environment | borné après health/reprovision | fallback capability/env |
| quota/budget | non automatique | waiting_for_user/admin |
| permission/validation | non | failed ou approval |
| user_required | non | waiting_for_user |
| conflict/fencing | recharger, ne pas répéter effet | nouvelle activation |
| permanent | non | failed + résultat partiel |
| unknown effect | jamais retry aveugle | reconcile ou intervention |
| system corruption | non | quarantine + incident |

Backoff exponentiel full jitter, retry budgets par Task/Run et circuit breakers par provider/capability. Tout état bloqué porte blocked_reason_code, required_action et resume_condition.

## 20. API v1

Toutes les écritures acceptent Idempotency-Key. Les mutations de ressources acceptent If-Match/version. Les listes utilisent curseurs opaques, filtres et limites bornées. Problem Details porte un code stable.

### Agents

- POST /v1/agents
- GET /v1/agents
- GET/PATCH/DELETE /v1/agents/{agent_id}
- POST /v1/agents/{agent_id}/versions
- GET /v1/agents/{agent_id}/versions
- POST /v1/agents/{agent_id}/clone
- POST /v1/agents/{agent_id}/instances

### Runs et contrôle

- POST /v1/agents/{agent_id}/runs
- GET /v1/runs et GET /v1/runs/{run_id}
- POST /v1/runs/{run_id}/pause
- POST /v1/runs/{run_id}/resume
- POST /v1/runs/{run_id}/cancel
- POST /v1/runs/{run_id}/instructions
- POST /v1/runs/{run_id}/retry créant un nouveau run
- GET /v1/runs/{run_id}/progress
- GET /v1/runs/{run_id}/results
- GET /v1/runs/{run_id}/costs

### Plans, tâches et sous-agents

- GET /v1/runs/{run_id}/plans et /plans/{version}
- GET/POST /v1/runs/{run_id}/tasks
- GET/PATCH /v1/tasks/{task_id}
- POST /v1/tasks/{task_id}/cancel
- POST /v1/tasks/{task_id}/retry
- GET /v1/runs/{run_id}/subagents
- POST /v1/runs/{run_id}/subagents
- POST /v1/subagent-runs/{child_run_id}/message

### Checkpoints, événements et artifacts

- GET /v1/runs/{run_id}/checkpoints
- GET /v1/checkpoints/{checkpoint_id}
- POST /v1/runs/{run_id}/restore avec approval si nécessaire
- GET /v1/runs/{run_id}/events?after={cursor}
- GET /v1/runs/{run_id}/artifacts
- GET /v1/artifacts/{artifact_id}/download-url

### Approvals, environnements, triggers

- GET /v1/approvals et GET /v1/approvals/{id}
- POST /v1/approvals/{id}/grant ou /reject
- GET/POST /v1/environments ; GET /v1/environments/{id}
- POST /v1/environments/{id}/suspend ou /destroy
- GET/POST/PATCH/DELETE /v1/agent-triggers
- GET/POST/PATCH/DELETE /v1/agent-schedules

### Streaming et webhooks

SSE est le premier choix : GET /v1/runs/{id}/events/stream avec Last-Event-ID, heartbeat, reconnexion, autorisation à chaque connexion et rattrapage depuis PostgreSQL. WebSocket n’est ajouté que si la bidirectionnalité temps réel apporte une valeur prouvée ; les instructions restent des commandes HTTP idempotentes.

Les webhooks sortants sont signés, retryés via outbox et dédupliqués. Les payloads sont minimaux et versionnés.

## 21. Frontend

Routes/features dédiées :

- catalogue : statut, owner, version active, triggers et santé ;
- création : wizard instructions, modèles, capabilities, mémoire, permissions, environnement, budget et aperçu des approvals ;
- détail agent : versions, instances, historique et clone ;
- détail run : objectif, progression, état bloqué et actions sûres ;
- graphe de Tasks avec dépendances, fan-out/fan-in et sous-agents ;
- timeline causale filtrable events/model/tool/approval/checkpoint ;
- checkpoints : raison, sécurité de restauration et différences ;
- approvals inbox avec preview exacte et expiry ;
- environnements et workspaces sans révéler de secrets ;
- coûts/tokens/budgets/quota ;
- artifacts et Results ;
- intégration conversationnelle sous forme de carte de run, persistée par run_id.

L’UI utilise un store normalisé et les événements SSE pour invalider/mettre à jour les projections. Aucun fallback de démonstration silencieux en production. Les actions terminales ou sensibles présentent la conséquence, pas les détails d’infrastructure.

## 22. Observabilité

Traces OpenTelemetry W3C de la commande au worker, modèle, capability et environnement. Logs JSON redacted avec trace_id, run/task/step/attempt/call IDs, statut, error_code et latence ; jamais de contenu brut.

Métriques :

- queue depth/age, claim latency, lease loss, recoveries ;
- runs/tasks par état et temps bloqué ;
- step attempts, retries, duplicate suppression, unknown effects ;
- model/tool latency, errors, tokens, cost ;
- budgets/quota, fan-out, profondeur sous-agents ;
- checkpoint size/latency/restore failures ;
- environment provision/cleanup/leaks ;
- SSE lag et projector lag.

Détecteurs : absence de progrès, boucle plan/action, retry storm, coût anormal, duplication, worker clock skew, checkpoint corruption, environment leak et approval expiré.

SLO initiaux à valider par mesure :

- 99,9 % des commandes acceptées durablement en moins de 500 ms hors upload ;
- p95 ready-to-claim inférieur à 5 s sous charge nominale ;
- aucune perte de transition après crash ;
- reprise p95 inférieure à 60 s après lease expiry ;
- zéro effet irréversible répété dans les scénarios certifiés ;
- SSE rattrapable sans trou depuis un curseur retenu.

## 23. Stratégie de tests

- unitaires : reducers, guards, DAG, context scopes, policy, budgets, backoff ;
- property-based : transitions, absence de cycles, invariants budget/permission ;
- migrations : upgrade N-1/N, restart backfill, rollback applicatif ;
- repositories : transactions, RLS et FK cross-tenant ;
- contrat : OpenAPI, événements, capabilities, model providers, edge protocol ;
- concurrence : double command, double claim, lease loss, fencing, fan-in ;
- reprise : kill à chaque frontière avant/après effet/checkpoint ;
- chaos : perte Redis, DB failover, réseau, provider, object store, Qdrant ;
- sécurité : tenant escape, SSRF, prompt injection, approval tamper, secret exfiltration, subagent escalation ;
- longue durée : horloge virtuelle et scheduler injectable pour simuler des jours ;
- charge/soak : millions d’événements, partitioning, backlog et fairness ;
- E2E : conversation → création → approval → run → sous-agent → artifact → reprise ;
- déploiement : worker N avec API N-1 et inverse, drain/canary/rollback.

## 24. Critères de préparation production

1. Tous les P0 de l’audit sont fermés avec tests.
2. Un seul Command Service et un seul reducer contrôlent chaque transition.
3. Le worker exécute un vrai modèle/capability ou échoue explicitement ; aucun faux succès.
4. Kill tests avant/après chaque effet critique prouvent la récupération.
5. RLS active/forcée et tests cross-tenant sur toutes les tables privées.
6. Sous-agent incapable d’augmenter permission, budget ou profondeur.
7. Secrets absents des logs, events, checkpoints, prompts non autorisés et UI.
8. Backups/restores, migrations N/N-1, reaper, reconciliation et DR exercés.
9. SLO, dashboards, alertes, runbooks, DLQ/quarantine et ownership opérationnels.
10. Charge nominale et pic respectent fairness, quotas, coût et latence.
11. Export, retention et suppression RGPD vérifiés.
12. Canary et rollback démontrés sans double effet ni perte de run.
13. SaaS et on-prem passent la même suite de contrats.
14. Revue sécurité indépendante sans high/critical ouvert.

## 25. Principaux ADR à figer avant code

- ADR-001 : moteur PostgreSQL interne et critères de réévaluation ;
- ADR-002 : transitions par command/reducer et projections ;
- ADR-003 : granularité Task/Step/Attempt et activations bornées ;
- ADR-004 : checkpoint envelope et stratégie de migration ;
- ADR-005 : garantie d’effet et effect ledger ;
- ADR-006 : scopes de contexte et mémoire ;
- ADR-007 : permission/delegation lattice ;
- ADR-008 : protocole edge/environnement ;
- ADR-009 : provider-neutral model transcript ;
- ADR-010 : SSE, rétention des événements et curseurs.
