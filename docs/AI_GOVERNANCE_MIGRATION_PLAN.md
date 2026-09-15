# Plan de migration et de refactorisation de la gouvernance IA

## Stratégie

La migration suit le modèle expand → shadow → enforce → contract. Chaque PEP possède un feature
flag et trois modes : `legacy`, `shadow`, `enforce`. En shadow, l’ancienne décision reste
exécutoire, la nouvelle est auditée et les divergences sont métrées. Un deny de sécurité invariant
peut toutefois bloquer immédiatement un chemin explicitement classé critique.

Rollback : revenir au mode précédent et à la dernière policy version publiée. Les migrations de
schéma restent additives jusqu’à la phase de suppression finale ; aucun rollback applicatif ne
nécessite de supprimer une table ou une colonne contenant une preuve.

## Phases

### 1. Audit et cartographie — réalisée pour le périmètre initial

- **Objectif :** inventaire auth/tenant/roles/resources/agents/tools/components/settings/audit.
- **Fichiers :** `apps/api`, `crates/*`, `apps/desktop`, migrations et documentation.
- **Sorties :** audit daté, flux, risques P0/P1/P2, éléments conservables.
- **Tests :** aucune mutation ; validation croisée backend/runtime/frontend.
- **Acceptation :** chaque point d’effet a owner, entrée, ressource et contrôle actuel.
- **Rollback :** sans objet.

### 2. Inventaire machine des permissions

- **Objectif :** registre de toutes les actions et ressources, sans wildcard implicite.
- **Composants :** routes Axum, repositories, descriptors tools, connecteurs, commandes Tauri,
  workers, voix, fichiers, mémoire et modèles.
- **Interfaces :** `PermissionCatalogEntry`, `EffectPoint`, `ResourceResolver`.
- **Tables :** seed versionné de `authorization_permissions`.
- **Tests :** CI refuse une route/tool/effect sans catalog entry et owner.
- **Acceptation :** couverture 100 %, pas d’action libre non normalisée.
- **Rollback :** registre non exécutoire.

### 3. Modèle central — fondation réalisée

- **Objectif :** requête/décision/constraints/policies déterministes.
- **Fichiers :** `crates/aro-policy`, workspace Cargo.
- **Interfaces :** `AuthorizationRequest`, `PolicyRule`, `AuthorizationDecision`,
  `DecisionConstraints`, `PolicyEngine`.
- **Tests :** default deny, tenant, deny precedence, gates, digest, intersections disjointes,
  read-only, quotas/budgets et délégation.
- **Acceptation :** évaluateur pur, sans I/O, résultats reproductibles.
- **Rollback :** retirer les PEP ; crate sans effet seul.

### 4. Persistance/versioning — fondation réalisée

- **Objectif :** modèle de données complet, RLS, immutabilité et audit de décision.
- **Fichiers :** `202607160001_ai_governance_foundation.sql`, `governance.rs`.
- **Tables :** rôles, permissions, policies/versions/rules, assignments, resources,
  classifications, bindings, manifests, consents, approvals, grants, capabilities, delegations,
  revocations, restrictions, handling policies, evaluations et security audit.
- **Tests :** migration sous owner puis accès `aro_app/aro_worker`, FK cross-tenant, RLS,
  immutabilité, séquence/hash anti-fork.
- **Acceptation :** aucune policy publiée modifiable ; aucun accès cross-tenant.
- **Rollback :** ne pas activer les readers/writers ; conserver les tables.

### 5. Policy repository et compilation

- **Objectif :** résoudre RBAC/ABAC/ReBAC/ACL/consent/grants/revocations en bundle.
- **Services :** `PolicyRepository`, `PolicyCompiler`, `PolicyBundleVerifier`.
- **Fichiers :** nouveau module store/service, pas de logique dans les handlers.
- **Événements :** `policy.published`, `policy.invalidated`.
- **Tests :** ordre de priorité, héritage, conflits, rules invalides fail-closed.
- **Acceptation :** bundle hashé et explain plan stable.
- **Rollback :** loader rules-only de la fondation.

### 6. PEP tools — premier chemin Web réalisé, généralisation restante

- **Objectif :** gateway unique avant/après tout tool.
- **Composants :** API directe, runtime local, worker durable, `aro-tools`.
- **Interfaces :** `ToolIntent`, `AuthorizedInvocation`, `ExecutionCapability`, `OutputGuard`.
- **API :** aucune exécution depuis un simple `ToolExecutionRequest` historique.
- **Tests :** descriptor hash, schema input/output, confirmation, revocation race, SSRF, DLP.
- **Acceptation :** 100 % tools derrière gateway ; legacy adapter seulement restrictif.
- **Rollback :** tools non migrés désactivés, jamais exécution directe permissive.

### 7. Agents et runs — liaison profil réalisée, identité durable restante

- **Objectif :** agent/owner/tenant/policy version/epoch/budget figés au run.
- **Fichiers :** `aro-core/agent.rs`, API handlers, runtime, store, clients desktop.
- **Tables :** `agent_permission_bindings`, colonnes snapshot/epoch sur runs/jobs.
- **Tests :** profil tenant validé, absence = moindre privilège, profil révoqué pendant run.
- **Acceptation :** chaque décision référence un agent stable et un run.
- **Rollback :** profil historique lu via adaptateur, aucun widening.

### 8. Queue durable et effect ledger

- **Objectif :** relier création de run → job → worker réel.
- **Composants :** `agent_jobs`, `agent_runner`, modèle, tools, checkpoints.
- **Interfaces :** command handler serveur, `EffectPrepared/Committed/Compensated`.
- **Contraintes :** horloge/acteur/safe-boundary construits serveur, sémaphore et fencing.
- **Tests :** replay, crash, lease, pause/cancel, faux timestamp/boundary, consommation monotone.
- **Acceptation :** aucun faux succès worker, reprise/idempotence prouvées.
- **Rollback :** feature flag durable off, chemin synchrone deny-by-default.

### 9. Délégation et sous-agents

- **Objectif :** activation contrôlée de `agent.delegate`.
- **Interfaces :** `DelegationRequest/Grant`, atténuation, cascade graph.
- **Tables :** `permission_delegations`, parent composite, descendants/revocations.
- **Policies :** profondeur, nombre, durée, environments, budget, non-delegable.
- **Tests :** expansion chaque dimension, cross-tenant, expiry et cascade pendant exécution.
- **Acceptation :** preuve automatique `child ⊆ parent`.
- **Rollback :** capacité cachée/désactivée et grants révoqués.

### 10. Plugins, skills et MCP

- **Objectif :** manifest commun, identité, trust et sandbox.
- **Composants :** collections historiques, tool registry, installers, sessions MCP.
- **Tables :** `component_permission_manifests`, installations et session capabilities.
- **Policies :** publisher/signature, domains/files/secrets, environment/session TTL.
- **Tests :** plugin/MCP malveillant, collision ID, output hostile, supply chain.
- **Acceptation :** aucun composant non signé/non approuvé n’exécute un effet.
- **Rollback :** composants en lecture/configuration seulement.

### 11. Contexte utilisateur, mémoire et fichiers

- **Objectif :** tools typés par champ, finalité et classification.
- **Interfaces :** `user.profile.*`, `user.preferences.*`, `user.memory.*`, `files.*`.
- **Composants :** context builder, vector resolver, file resolver/root canonicalizer.
- **Tests :** owner/tenant, consent memory off, symlink/path escape, provenance non fiable.
- **Acceptation :** pas d’objet utilisateur/mémoire complète injecté au modèle.
- **Rollback :** contexte minimal, mémoire/réseau désactivés.

### 12. Classification et providers

- **Objectif :** data handling imposé à l’input/output et au choix du modèle.
- **Services :** classifier déterministe/manual, field labels, DLP/redaction.
- **Tables :** `resource_classifications`, `data_handling_policies`.
- **Tests :** sensitive→external deny, local-only, regulated, mixed classification=max.
- **Acceptation :** chaque transfert possède une classification résolue.
- **Rollback :** classification unknown = restrictive/local-only.

### 13. Consentements

- **Objectif :** consentement compréhensible, scoped, expirant et révocable.
- **API :** `consent.create/list/revoke` ; aucun consentement via prompt.
- **UI :** donnée, agent, service, finalité, durée, partage, réversibilité.
- **Tests :** once/session/conversation/agent/org/duration, revoke immédiat, changement de scope.
- **Acceptation :** delta exact visible et evidence serveur.
- **Rollback :** demandes restent pending, effet bloqué.

### 14. Confirmations, step-up et approvals

- **Objectif :** workflow durable complet.
- **API :** request/approve/deny/consume ; auth forte/device binding.
- **UI :** carte/modale exacte, montant/ressource/agent/durée et explication.
- **Tests :** input modifié, replay, expiry, approver démis, consommation concurrente.
- **Acceptation :** preuve one-shot consommée atomiquement avec l’effet.
- **Rollback :** action reste en attente ou est annulée.

### 15. Settings et matrice frontend

- **Objectif :** séparer session/device/user/agent/workspace/org/global.
- **API :** capabilities effectives renvoyées par serveur ; UI jamais source d’autorité.
- **UI :** autonomie, memory/files/services/models/data sharing/history/plugins/MCP/delegation,
  budgets/quotas/notifications ; restrictions org verrouillées et expliquées.
- **Tests :** owner/admin/manager/member/guest, tenant switch, 403 explicite, rollback optimiste.
- **Acceptation :** aucune fausse réussite ni état cross-tenant.
- **Rollback :** pages read-only avec policy effective.

### 16. Révocation et cache

- **Objectif :** SLO de révocation immédiate sans stale allow.
- **Services :** policy/subject epoch, invalidation outbox/pub-sub, L1/L2 versionné.
- **Actions :** requeue re-evaluation, close component sessions, cascade agents/capabilities.
- **Tests :** cache stale, partition Redis, concurrence, revoke pendant model/tool.
- **Acceptation :** stale entry jamais utilisée après epoch change ; fail closed critique.
- **Rollback :** désactiver cache et évaluer DB.

### 17. Audit, expurgation et observabilité

- **Objectif :** preuve complète distincte des logs techniques.
- **Services :** redaction avant persistance, hash chain/anchor, export/rétention.
- **Migration :** audit historique metadata-only ; arrêt du payload complet dans Redis/outbox.
- **Tests :** secrets/prompts absents, mutation audit rejetée, fork détecté, export tenant.
- **Acceptation :** qui/pour qui/agent/tool/resource/env/permission/durée/policy/contraintes/
  confirmation/pourquoi reconstructibles.
- **Rollback :** audit local DB ; ne jamais republier contenu brut.

### 18. Shadow mode et migration des domaines

- **Objectif :** migrer route par route et worker par worker.
- **Ordre :** tools → models → files/memory → integrations → scheduled tasks → admin → components.
- **Métriques :** allow/deny divergence, missing resolver, latency, cache, false positive.
- **Acceptation :** fenêtre sans divergence inexpliquée et P95 conforme.
- **Rollback :** flag domaine vers legacy ; security invariants restent actives.

### 19. Tests système et durcissement

- **Matrice :** RBAC/ABAC/ReBAC, tenants, ownership, conflicts, expiry, delegation, consent,
  confirmation, revocation, injection, confused deputy, plugins/MCP, cache, load, migration.
- **Fuzz/property :** intersections jamais élargies, enfant jamais supérieur au parent, reducer sans
  résurrection, canonical digest stable.
- **E2E :** UI → API → queue → PDP → capability → executor → output guard → audit.
- **Acceptation :** tests négatifs et rôle PostgreSQL production verts.
- **Rollback :** enforcement non activé si gate rouge.

### 20. Enforcement global et retrait legacy

- **Objectif :** `enforce` partout, suppression des doubles décisions et booléens redondants.
- **Préconditions :** couverture PEP, RLS, revocation SLO, audit/DLP, runbooks, charge et recovery.
- **Migration :** archive des profils/règles legacy après période de compatibilité.
- **Tests :** rollback release, restore, key/epoch rotation, chaos DB/Redis/executor.
- **Acceptation :** checklist production signée sécurité/plateforme/produit.
- **Rollback :** dernière version app/policy compatible ; données legacy conservées pendant la
  fenêtre contractuelle.

## Contrats internes à stabiliser

- `permissions.check`, `permissions.explain`, `permissions.list_effective`.
- `permissions.grant/revoke/request/approve/deny`.
- `consent.create/revoke`.
- `policy.evaluate/simulate/publish`.
- `capability.issue/consume/revoke`.
- `agent.permissions.inspect`, `tool.permissions.inspect`.
- `audit.permission_decision`, `audit.effect_result`.

Chaque commande de mutation prend une idempotency key, un expected version/epoch et un principal
authentifié. Aucun handler ne construit sa propre matrice de rôles.

## Stratégie de tests

| Niveau | Contenu |
| --- | --- |
| Unitaire | matcher, priorité, contraintes, digest, classification, atténuation, explication |
| Policy fixtures | cas allow/deny/gates par rôle, ressource, tenant, env, provider et risk |
| Repository | RLS, FK composites, immutabilité, consommation atomique, revocation epoch |
| Intégration | PEP avec tools/models/files/memory/connectors et output guards |
| Sécurité | injection, confused deputy, replay, escalation, exfiltration, plugin/MCP hostile |
| Concurrence | approval/capability one-shot, leases, revoke/effect, audit sequence, cache |
| Charge | batch/P95/P99, cache invalidation storm, policy publish et audit throughput |
| E2E | matrices UI/API/worker, switch tenant, explication et révocation live |
| Migration | old/new dual read, shadow divergence, rollback release/policy |

La CI génère une matrice `(subject, role, agent, component, action, resource, context) → expected`
et vérifie surtout les refus. Toute nouvelle permission exige au moins un test allow contraint et
plusieurs tests négatifs (autre tenant, owner, env, expiry, revoke et scope supplémentaire).

## Déploiement

1. Déployer le schéma additif et vérifier grants/RLS sous les rôles réels.
2. Déployer le PDP sans PEP exécutoire, compiler les policies et observer.
3. Activer audit metadata-only et corriger les fuites historiques outbox/Redis.
4. Activer shadow par domaine, du risque le plus faible au plus élevé.
5. Activer enforce sur lectures bornées, puis writes réversibles, enfin effets externes/critiques.
6. Activer cache seulement après invalidation/epoch et chaos tests.
7. Activer plugins/MCP/délégation en dernier.
8. Contracter le legacy après la période de preuve et un exercice de rollback.

## Gate de préparation production

- [ ] Inventaire des effets à 100 %.
- [ ] PEP obligatoire et impossible à contourner local/API/worker.
- [ ] RLS/FK tenant sous rôles réels.
- [ ] Policies/rules publiées immuables.
- [ ] Evidence liée au digest exact et consommée atomiquement.
- [ ] Révocation live/cascade/cache sous SLO.
- [ ] Classification et transfert externe imposés.
- [ ] Plugin/MCP sandbox, manifest et output guard.
- [ ] Audit expurgé, séquencé, hashé, exportable et rétentionné.
- [ ] Matrice négative, concurrence, charge, migration et E2E vertes.
- [ ] Runbooks incident, cache/PDP, clés, révocation, rollback et restore exercés.

