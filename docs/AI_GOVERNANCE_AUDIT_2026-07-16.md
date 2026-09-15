# Audit du système de gouvernance IA — 16 juillet 2026

## Verdict

L’existant fournit de bonnes fondations d’authentification, d’isolation propriétaire et de
description des tools, mais ne constitue pas encore un système d’autorisation IA de production.
La décision est dispersée entre les handlers HTTP, les repositories, des booléens de profils
d’agents, les paramètres du client et les adaptateurs réseau. Les rôles d’organisation protègent
principalement l’administration du tenant ; ils ne décrivent ni une intention IA complète, ni un
consentement, ni une capability temporaire, ni une délégation atténuée.

La refonte doit donc être additive. Les protections fiables existantes restent actives pendant que
les décisions migrent vers un PDP central, en double évaluation puis en enforcement. Aucun contrôle
frontend ou instruction de prompt ne devient une frontière de sécurité.

## Cartographie de l’état initial

| Domaine | Stockage | Calcul / vérification | Transmission / affichage | État |
| --- | --- | --- | --- | --- |
| Authentification | `users`, `memberships`, familles de refresh tokens, JWT | `apps/api/src/auth.rs`, `aro-store` | session Tauri en mémoire/keyring, navigateur en mémoire | Base robuste : tenant signé, membership revalidé, rotation/replay des refresh |
| Tenant et propriétaire | `organization_id`, `owner_user_id`, FK composites partielles | filtres repository et `TenantContext`; RLS préparée pour conversations/messages/mémoires | organisation active dans la session | Bon socle, mais RLS privée encore en déploiement progressif et non généralisée |
| RBAC organisation | `memberships.role` (`owner/admin/manager/member/guest`) | helpers `ensure_org_admin` et variantes dans `aro-store` | UI réduite à `admin/member` | Règles dispersées et sémantique UI incomplète |
| Profils agents | `agent_permission_profiles` | booléens lecture/écriture/shell/réseau, roots/domaines, approbation | page Permissions | Avant cette tranche, profil non attaché au run créé depuis le composer |
| Runs / tâches | lanes, runs, steps, jobs, leases, budgets partiels | `aro-agent`, `agent_runner`, `agent_jobs` | timeline de contexte | Identité d’agent stable, délégation et grant snapshot incomplets |
| Tools | descripteurs riches dans `aro-core/src/tool.rs` | registry/runtime + policy réseau dans `aro-tools` | résultats/steps | Contrat réutilisable ; PDP, consentement et output guard manquaient |
| Fichiers | tables fichiers/uploads/scans, owner et tenant | handlers + store + quarantaine/ClamAV | UI fichiers | Isolation utile ; classification, ACL de partage, roots/symlinks et autorisation centrale absentes |
| Mémoire | `memories`, owner/tenant, projection vectorielle | store + scope vectoriel | page Mémoire | Privée par défaut ; consentement par champ, désactivation forte et politiques de transfert absents |
| Intégrations | installations/capabilities/scopes/credentials chiffrés | OAuth/API key côté serveur | panneau Intégrations | Bon début de manifest ; scopes réels et consentement différentiel mal exposés |
| Plugins / skills / MCP | collections génériques et payloads nettoyés/chiffrés | CRUD tenant ; peu d’enforcement runtime commun | pages Settings | Manifest de permissions, identité, confiance, sandbox et output guard manquants |
| Modèles | settings provider/model, secrets hors settings | validation local/HTTPS dans `aro-runtime` | page Modèles | Pas de politique liée à la classification de la donnée |
| Settings | préférences user, app settings user+org, collections client | handlers/repositories distincts | nombreuses pages Svelte | Session/device/user/agent/workspace/org/global insuffisamment séparés |
| Confirmations | enum de profil et événements `tool.approval_required` | pas de cycle API/UI complet | aucune modale durable | Non applicable de bout en bout |
| Audit | `audit_events`, traces techniques, steps agents | écritures ponctuelles | stats admin limitées | Pas de preuve uniforme de décision, rétention/immutabilité/redaction incomplètes |
| Révocation | memberships, invitations, refresh, intégrations | immédiate pour certains domaines | UI partielle | Pas de registre transversal ni invalidation en cascade des agents/capabilities |

## Flux d’autorisation initial

1. `AuthContext` dérive l’utilisateur et l’organisation du JWT signé.
2. Le store revalide le membership et filtre souvent par `organization_id` et `owner_user_id`.
3. Le handler effectue éventuellement un test RBAC ou un test booléen du profil agent.
4. Le runtime/tool applique ses propres contraintes (allowlist réseau, SSRF, taille, redirects).
5. Le résultat est persisté en step/artifact/context, sans décision d’autorisation uniforme.

Ce flux protège plusieurs accès directs, mais ne permet pas de répondre de façon homogène à
« quelle politique, quel consentement, quelle durée et quelles contraintes ont autorisé l’action ».

## Éléments à conserver

- Le tenant signé dans le JWT et le switch d’organisation comme transition de session.
- `TenantContext` non mutable, `begin_tenant_tx` et la revalidation du membership dans la même
  transaction.
- Les filtres propriétaire/tenant, FK composites et le déploiement RLS progressif.
- La rotation sérialisée des refresh tokens, la détection de replay et la révocation familiale.
- Les descripteurs immuables de tools : permissions requises, risque, effets, environnement,
  idempotence, limites, provenance, owner et observabilité.
- Les protections SSRF/DNS/redirect/body limits de `aro-tools`.
- Les leases, barrières d’idempotence, budgets et événements durables déjà présents dans les jobs.
- Le chiffrement des credentials et la séparation renderer/backend.
- La quarantaine des fichiers et les scopes owner/tenant de la mémoire.
- La séparation des clients API frontend en modules et les composants Settings existants.

## Failles et limitations

### P0 — Blocantes

1. **Pas de PDP unique.** Une nouvelle route peut oublier un contrôle ou appliquer une règle
   différente. Le frontend, le store et les runtimes ne disposent pas d’un contrat de décision
   partagé.
2. **Profils affichés mais non appliqués aux runs.** Le composer ne transmettait pas le profil et le
   backend appelait `start_run(..., None)`. L’utilisateur pouvait croire un run restreint alors que
   le snapshot ne portait aucun profil.
3. **Sémantique réseau inversée dans l’UI.** `allowedDomains` était présenté comme une blocklist ;
   ajouter un domaine « bloqué » l’autorisait en réalité. Une liste vide affichée comme illimitée
   était refusée par le backend.
4. **Droits des plugins/skills/MCP non imposés par une frontière commune.** Leur contenu est traité
   comme une collection de configuration et non comme une identité non fiable avec manifest signé,
   scopes, egress, filesystem, secrets, durée et output guard.
5. **Aucune classification de donnée exécutable.** Le choix model/provider/environnement n’est pas
   automatiquement restreint par la sensibilité de la ressource ou du champ.
6. **Pas de révocation universelle immédiate.** Les mécanismes existent par domaine, sans registre
   transversal, génération de cache, fermeture des sessions/composants et cascade de délégation.

### P1 — Élevées

- Les confirmations `always/safe-auto/never` n’ont ni approval durable complet, ni modale, ni
  consommation atomique liée au digest exact de l’intention.
- Un sous-agent n’a pas de grant atténué explicite ; objectif, profondeur, durée, tools, ressources,
  budget, non-transférabilité et révocation en cascade ne forment pas un contrat commun.
- L’UI réduit `owner/admin/manager/member/guest` à `admin/member`, confond certains statuts et ne
  reçoit pas de matrice de capacités du serveur.
- Plusieurs lectures frontend authentifiées masquent 401/403/5xx par une liste vide ou des données
  de démonstration. Certaines écritures offline simulent un succès.
- Les mutations sensibles (rôle, membre, clé, intégration) manquent de confirmation forte et
  plusieurs mises à jour optimistes ne font pas de rollback.
- Les scopes OAuth effectifs, leur delta, le propriétaire et l’expiration sont insuffisamment
  présentés ; des capacités sont cochées par défaut.
- Les paramètres user/device/session/agent/workspace/org/global sont mélangés, ce qui rend
  héritage, priorité et verrou administrateur difficiles à expliquer.
- Les logs techniques, steps agents et audit métier ne constituent pas une chaîne de preuve
  immuable et uniformément expurgée.

### P2 — Structurelles

- `aro-store` et `handlers.rs` restent très larges ; la logique d’autorisation s’y duplique.
- Les ressources partagées n’ont pas encore d’ACL/ReBAC explicite ; la règle actuelle est privée
  par défaut.
- Le cache de politiques, sa version, son invalidation et les métriques de divergence n’existent
  pas encore.
- La matrice de tests ne couvre pas systématiquement tenant, héritage, conflit, expiration,
  révocation, confused deputy, injection, plugins/MCP malveillants et concurrence.

## Risques d’attaque

| Menace | Exposition initiale | Contrôle cible |
| --- | --- | --- |
| Élévation de privilèges | modification de profil, règle manquante, rôle simplifié | mutations admin via PDP, deny explicite, version immuable, step-up |
| Confused deputy | tool/intégration agit avec le credential du service | capability liée acteur-agent-action-resource-intent |
| Prompt injection | contenu externe peut influencer l’intention | contenu non fiable, PEP hors modèle, aucun changement de permission par prompt |
| Exfiltration | réseau/modèle cloud sans politique de donnée | classification, output/input guard, egress proxy, provider policy |
| Mouvement latéral tenant/env | ID connu ou cache obsolète | tenant cryptographique + RLS + FK composite + version de révocation |
| Replay / double exécution | approbation ou capability réutilisée | nonce/hash, consommation atomique, idempotency key, max uses |
| Délégation excessive | enfant hérite implicitement | atténuation ensembliste, non-delegable, profondeur et cascade |
| Plugin/MCP hostile | manifest/configuration déclarative non imposée | identité, signature, sandbox, trust, allowlists, output non fiable |

## Correctifs réalisés dans la tranche fondation

- Ajout du crate `aro-policy`, évaluateur pur et déterministe, deny-by-default.
- Décisions structurées, explications, règles versionnées, ABAC/ReBAC simple, contraintes
  intersectées, gates de confirmation/step-up/admin et invariants tenant/secret.
- Refus explicite des intersections de contraintes disjointes ; une intersection vide ne devient
  jamais « illimitée ».
- Confirmations/approbations liées à l’acteur, l’agent, l’action, la ressource, le montant, la durée
  et au digest canonique de l’intention.
- Modèle d’atténuation de délégation avec sous-ensembles, profondeur, expiration, appels et budget.
- Migration additive de gouvernance : policies/versions/rules, assignments, ressources et
  classifications, agents/components, consentements, approvals, grants, capabilities,
  délégations, révocations, évaluations, restrictions org, data handling et audit.
- RLS immédiate sur les nouvelles tables tenant, FK tenant composites, policy versions publiées
  immuables et audit sécurité append-only chaîné par hash.
- Branchement du PDP avant l’exécution des tools Web ; chargement des policies publiées, adaptateur
  legacy restrictif, décision et audit avant effet.
- Liaison explicite `autonomyProfileId` au run, validée côté serveur dans le tenant.
- Correction de la sémantique allowlist dans l’UI, purge des profils au switch de tenant et fin du
  fallback silencieux pour la lecture authentifiée des profils.

## Limites restantes de cette tranche

Cette fondation ne signifie pas que toute la plateforme est déjà migrée. Restent notamment : API
d’administration complète des policies/consents/approvals/revocations ; PEP sur chaque handler,
worker et connecteur ; identité durable des agents ; manifests imposés pour plugins/skills/MCP ;
classification par ressource/champ ; output guards ; UI de décision/audit ; cache versionné ; RLS
généralisée ; comparaison shadow et matrice E2E. Le déploiement public reste conditionné aux gates
définies dans le plan de migration.

