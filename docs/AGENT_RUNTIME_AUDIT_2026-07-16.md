# Audit autoritatif du runtime d’agents — 16 juillet 2026

Statut : livrable d’architecture préalable à l’implémentation.

Ce document remplace les constats temporels de AGENT_OS_AUDIT_2026-07.md pour le périmètre des agents. Il ne remplace pas les audits backend généraux. Le dépôt ne possède encore aucun commit et tous les fichiers sont non suivis ; les numéros de ligne sont donc des repères du baseline observé, pas des références immuables.

## 1. Verdict

ARO ne possède pas encore un runtime d’agents autonome complet. Il possède trois chemins partiellement indépendants :

1. un runtime desktop local, séquentiel, attaché au processus Tauri et limité aux capacités Web réellement exécutables ;
2. une API cloud qui crée et projette des runs dans le modèle historique ;
3. une queue PostgreSQL durable très avancée sur les leases, l’idempotence de soumission, les budgets et le fencing, mais non raccordée au point d’entrée principal et consommée par un worker simulé.

Le principal problème n’est donc pas l’absence de primitives. C’est l’absence d’un modèle de domaine et d’un chemin d’exécution uniques reliant définition durable, version, run, session, tâches, plan, checkpoints, appels de modèle et de capacités, approvals, environnements, événements et résultats.

La recommandation est de conserver PostgreSQL comme source de vérité et de transformer progressivement la queue actuelle en moteur interne de machine à états durable. Redis reste un accélérateur de réveil, de cache et de sémaphore ; il ne détient aucun état indispensable. Le control plane reste un monolithe modulaire Rust/Axum ; les workers sont des processus séparés ; les capacités locales passent par un exécuteur edge explicitement appairé.

L’activation production doit rester impossible tant que les P0 sont ouverts : entrée API non durable, worker simulé, commandes de contrôle contournant la queue, droits SQL worker incomplets, isolation tenant agent non renforcée et absence de checkpoints restaurables.

## 2. Méthode et baseline dynamique

L’audit a couvert les contrats aro-core, l’orchestrateur aro-agent, le runtime local aro-runtime, les exécuteurs aro-tools, les handlers et le worker Axum, les migrations et services aro-store, le modèle de mémoire, les modèles, skills, plugins, MCP, intégrations, le frontend Svelte et la passerelle Tauri.

Le code a changé pendant l’audit. Les constats suivants sont donc classés :

- confirmé : encore présent dans le dernier état lu ;
- résolu pendant l’audit : observé initialement puis corrigé par un changement concurrent ;
- hors périmètre observé : défaut réel du baseline, non causé et non corrigé par cet audit.

### Vérifications au dernier passage

| Vérification | Résultat | Classement |
| --- | --- | --- |
| cargo check --workspace | succès | baseline compilable |
| npm run check | succès, zéro erreur et zéro warning | baseline frontend typé |
| tests Rust ciblant aro-agent/runtime | échec de compilation : quatre appels de tests aro-runtime n’ont pas suivi les nouvelles signatures SearchSettings/callback | changement concurrent non synchronisé |
| npm run test:unit | 75 succès, 1 échec | façade API exporte cinq fonctions integration absentes de la liste des modules du test |
| état Git | aucun commit, tous les fichiers non suivis | attribution fine impossible |

Ces deux échecs ciblés ne bloquent pas la conception, mais doivent être rendus verts avant la première mutation du runtime agent afin de disposer d’un oracle de non-régression.

### Changements concurrents déjà intégrés

- ToolDescriptor v1 est désormais présent dans aro-core et deux descriptors Web canoniques sont enregistrés.
- Les anciens identifiants Web sont traités comme alias.
- Les tools workspace, shell et artifact non exécutables ne sont plus annoncés par le registre courant.
- Le contrôle Web applique maintenant l’intersection entre politique et domaines demandés, avec protections SSRF, DNS pinning et redirects contrôlés.
- L’enrichissement Web implicite de l’API est désactivé.

Ces avancées sont conservées. Elles ne résolvent pas le runtime durable.

## 3. Cartographie de l’existant

~~~mermaid
flowchart LR
  C[Conversation et UI Svelte]
  F[Façade TypeScript]
  T[Tauri]
  L[aro-runtime local]
  A[API Axum]
  H[handlers agent historiques]
  O[aro-agent contexte et registre]
  X[aro-tools exécuteurs Web]
  P[(PostgreSQL)]
  Q[(agent_run_jobs)]
  W[worker aro-api]
  R[(Redis)]
  V[(Qdrant)]

  C --> F
  F --> T
  F --> A
  T --> L
  L --> O
  L --> X
  L -->|projections locales| C
  A --> H
  H --> O
  H --> X
  H --> P
  H --> R
  H --> V
  P --> Q
  W -->|claim et lease| Q
  W -. résultat simulé .-> P
  H -. ne soumet pas .-> Q
~~~

### 3.1 Chemin desktop local

La conversation appelle la façade TypeScript, puis les commandes Tauri et AssistantEngine dans aro-runtime. La boucle :

1. construit un ContextPack ;
2. appelle un ModelProvider ;
3. interprète une action JSON final/tool/pause ;
4. exécute un tool à la fois ;
5. enregistre steps, artifacts et sources de contexte ;
6. recommence jusqu’à réponse finale ou plafond d’étapes.

Le plafond est borné à 32 étapes. La boucle est séquentielle, sans DAG, sans tâches indépendantes et sans checkpoint durable. Un redémarrage peut reconstruire certaines projections SQLite, mais pas reprendre une intention exacte autour d’un effet externe. Ce chemin est adapté à une assistance locale courte, pas à une mission multi-jour.

### 3.2 Chemin API historique

POST /agent/runs valide l’objectif, choisit une lane, crée AgentRun et un premier AgentStep, assemble de la mémoire et du contexte, puis renvoie une vue. Il n’appelle pas submit_agent_run_job. Le feature flag agent_durable_execution_enabled existe dans ApiState mais ne route aucun trafic.

Pause, resume et cancel mutent directement les tables historiques. Elles ne passent pas par la machine de transitions de la queue, ne portent pas d’idempotency key et ne garantissent pas le fencing face à un worker actif.

Les événements retournés par l’API historique sont dérivés des projections du run. Ils ne constituent ni un journal causal canonique, ni une source permettant un replay.

### 3.3 Queue durable PostgreSQL

La migration 202607020012_agent_execution_queue.sql et aro-store/src/agent_jobs.rs constituent le meilleur actif existant :

- snapshot de soumission immuable et chiffré ;
- idempotence de soumission et advisory locks ;
- quotas par utilisateur et organisation ;
- priorité, aging, lanes et sélection FOR UPDATE SKIP LOCKED ;
- lease token et génération pour interdire les écritures d’un worker périmé ;
- heartbeat, budgets et compteurs monotones ;
- pause, reprise, annulation, retry et réconciliation ;
- événements append-only sans contenu ;
- finalisation atomique du message et du step final ;
- reaper des leases expirés et purge.

Limites structurantes :

- un seul job par run empêche de représenter naturellement plusieurs tâches ou tentatives ;
- max_wall_time_seconds est plafonné à 86 400 secondes ; une mission multi-jour doit être découpée en activations courtes ;
- le snapshot est une entrée de job, pas un checkpoint de runtime ;
- parent_job_id a été ajouté sans contrainte ni comportement ;
- la queue ne modélise pas le graphe de tâches, les approvals, les environnements ou le journal d’effets ;
- les événements ne suivent pas encore l’enveloppe canonique complète.

### 3.4 Worker

Le worker réclame les jobs puis lance un tokio::spawn par job sans sémaphore process. Il renouvelle le lease avec un usage nul, construit ModelRouter sans l’utiliser, écrit un step fictif et termine avec « Agent completed goal ». Il peut donc produire un faux succès durable.

Le processus principal n’exécute pas périodiquement le reaper, la réconciliation des jobs inéligibles ni la purge agent. Les rôles SQL de runtime antérieurs ne reçoivent pas les grants des nouvelles tables de queue ; la migration de queue indique explicitement qu’elle n’accorde aucun droit. Un worker déployé avec le rôle restreint peut donc ne pas pouvoir réclamer de job.

### 3.5 Modèle agent actuel

AgentRunStatus ne couvre que queued, running, waiting, paused, completed, failed et cancelled. CustomAgentDefinition mélange identité et configuration mutable : nom, description, prompt système, un fournisseur/modèle, profil d’autonomie et liste de tools.

Il n’existe pas de concepts persistés séparés pour AgentTemplate, AgentVersion, AgentInstance, Session, Task, Plan, Checkpoint, ModelCall, Environment, Approval, Trigger ou Schedule. checkpoint_summary est une chaîne libre. AgentStep.sequence impose une timeline linéaire.

### 3.6 Contexte et mémoire

ContextBuilder sélectionne le but courant, un environnement sommaire, huit messages récents, quatre souvenirs et les capacités disponibles. Il n’existe ni matrice de portée, ni héritage explicite, ni politique d’exclusion du prompt, ni budget de contexte par tâche.

La mémoire cloud possède contenu, catégorie, scope, status, salience et sources, avec recherche lexicale/vectorielle. Le chemin agent charge néanmoins une collection large avant le retrieval. La projection vectorielle se produit encore dans certains chemins HTTP après le commit PostgreSQL. Il manque versions, confiance, provenance normalisée, TTL, sensibilité, ACL de partage, contradiction/fusion et workflow d’oubli.

### 3.7 Modèles

Les abstractions ModelProvider et ModelRouter sont réutilisables. La définition d’agent choisit cependant un seul couple provider/model ; le worker durable n’utilise pas le routeur. Il n’existe ni politique de routage par tâche, ni fallback gouverné par confidentialité/coût/latence, ni transcript provider-neutral versionné, ni ledger ModelCall fiable.

### 3.8 Tools, skills, plugins et MCP

ToolDescriptor v1 est une bonne base : catégorie, risque, permissions, environnement, idempotence, effets, retries, limites, dépendances, observabilité, provenance et aliases. ToolExecutionRequest/Result restent trop pauvres pour l’exécution agent : ils ne portent pas agent/version/task/step/tenant/actor/policy/environment/idempotency/effect/model/cost de manière canonique.

Les skills sont du contenu CRUD avec triggers ; aucun compilateur ne produit de contrat ou plan exécutable. MCP conserve configuration et snapshots mais aucun client durable ne réalise discovery, auth, heartbeat et invocation. Les plugins historiques sont génériques ; les intégrations v3 apportent en revanche catalogue, installations, credentials versionnés, grants, health, jobs et audit, à réutiliser.

### 3.9 Permissions et isolation

PermissionProfile combine quatre booléens et des listes de racines/domaines. Le direct tool endpoint impose un profil explicite ; l’accès Web implicite est désormais désactivé. Il n’existe pas encore de décision ABAC persistée liée au hash exact d’une action.

Des politiques RLS existent pour certaines données privées mais ne sont volontairement pas activées. Les tables agent ne disposent pas d’une couverture RLS complète. L’isolation dépend donc des filtres applicatifs et des contraintes composites, insuffisants seuls pour un runtime autonome.

### 3.10 Frontend

L’UI montre une inbox de runs dans la conversation, des lanes, steps, artifacts et actions pause/reprise/annulation. AgentsSettings propose un CRUD minimal de CustomAgentDefinition. Il n’existe pas de catalogue d’agents versionnés, détail de run, graphe de tâches, vue de sous-agents, checkpoints, approvals, environnements, consommation ou diagnostic de reprise.

Les fallbacks de démonstration en mémoire peuvent masquer une indisponibilité backend. Les tâches planifiées actuelles ne déclenchent pas un run durable.

## 4. Matrice des concepts actuels

| Concept cible | Représentation actuelle | Écart |
| --- | --- | --- |
| AgentTemplate | aucune | absent |
| Agent | CustomAgentDefinition | non versionné, identité/configuration mélangées |
| AgentVersion | aucune | absent |
| AgentInstance | aucune | absent |
| Run | AgentRun + agent_run_jobs | deux sources concurrentes |
| Session | conversation implicite | pas de cycle de vie agent |
| Task/Subtask | lane/step | pas d’unité durable ni dépendances |
| Step/Attempt | AgentStep/job attempt | identités et sémantiques séparées |
| Plan | prompt et séquence | non persisté/versionné |
| Checkpoint | checkpoint_summary | non restaurable |
| Memory | memories | scopes/ACL/versioning incomplets |
| Workspace | chemin local implicite | pas de ressource durable |
| Environment | snapshot en mémoire | pas de lifecycle ni isolation |
| ToolCall | AgentStep JSON | pas de ledger d’effet |
| ModelCall | aucune table canonique | coûts et fallback non fiables |
| Event | deux modèles d’événements | pas d’enveloppe unique |
| Artifact | AgentArtifact/fichiers | liens de provenance incomplets |
| Approval | confirm UI ponctuel | non durable, non hashé |
| Trigger/Schedule | CRUD générique | non relié aux agents |
| Result | message/step final | pas de résultat versionné |

## 5. Responsabilités mélangées

- handlers.rs combine transport HTTP, autorisation, assemblage de contexte, orchestration et persistance.
- aro-runtime combine conversation, boucle modèle, exécution de capabilities et projection UI locale.
- AgentRun mélange demande, état courant, progression, conversation et résumé de checkpoint.
- AgentStep sert à la fois de trace, appel de tool, résultat, raisonnement et projection d’interface.
- CustomAgentDefinition mélange définition durable et préférences d’exécution.
- App.svelte combine orchestration, cache, vue et fallbacks.
- aro-store/src/lib.rs concentre trop de domaines malgré l’extraction récente de agent_jobs.rs.

## 6. Risques classés

### P0 — intégrité, sécurité ou faux succès

1. L’API principale ne soumet aucun job durable.
2. Le worker durable simule l’exécution et marque le job réussi.
3. Pause/reprise/annulation contournent le service de queue et son fencing.
4. Les grants SQL du rôle worker ne couvrent pas explicitement la queue agent.
5. Aucun checkpoint restaurable ne protège les effets et décisions.
6. L’isolation tenant agent n’est pas défendue par RLS active et forcée.
7. Les payloads de steps peuvent conserver entrées/résultats bruts de tools, avec risque de secrets et données personnelles.

### P1 — fiabilité et architecture

1. Trois chemins d’exécution et deux sources d’état divergent.
2. Un job unique par run ne permet ni DAG, ni tentatives par tâche.
3. Le worker n’a pas de concurrence locale bornée ni comptage d’usage réel.
4. Reaper, réconciliation et purge agent ne sont pas opérés.
5. Les transitions historiques sont permissives et non centralisées.
6. Absence d’idempotency/effect ledger pour les actions externes.
7. Contexte de tâches non isolé et mémoire trop largement chargée.
8. Un seul modèle par définition et pas de fallback gouverné.
9. Skills, MCP et plusieurs plugins sont déclaratifs, pas exécutables.
10. parent_job_id et agent.delegate donnent l’apparence d’une délégation non implémentée.
11. Les tâches planifiées et événements externes ne produisent pas de runs durables.
12. Les sessions longues dépendent d’un plafond de job inférieur ou égal à un jour.

### P2 — produit, exploitation et maintenabilité

1. Pas de vues produit dédiées au graphe, checkpoints, approvals, sous-agents et coûts.
2. Pas de contrat OpenAPI/event généré vers TypeScript.
3. Observabilité sans traces causales et métriques par task/step/call.
4. Gros modules à fort risque de conflits parallèles.
5. Fallbacks UI susceptibles de masquer un backend indisponible.
6. Pas de partitionnement, archivage et rétention dédiés aux événements agents à fort volume.

### Résolu pendant l’audit

1. Allowlist Web inversée.
2. Élargissement implicite de l’egress.
3. Tools annoncés sans exécuteur dans le registre courant.
4. Absence complète de descriptor tool canonique.

## 7. Concurrence et double exécution

Le lease fencing de la queue protège les écritures après perte de lease, mais ne rend pas une action externe exactement une fois. Un worker peut effectuer l’action, tomber avant de l’enregistrer puis provoquer un retry. Sans effect ledger et idempotency key stable au niveau ToolCall, un email, paiement, publication ou suppression peut être répété.

La garantie réaliste est :

- au moins une fois pour l’activation d’une task ;
- transition et résultat exactement une fois dans PostgreSQL via compare-and-swap/fencing ;
- effet externe effectivement une fois seulement si le fournisseur accepte une clé d’idempotence ;
- sinon journal before/after, vérification, approval ou compensation explicite.

## 8. Composants à conserver et à remplacer

| Décision | Composants |
| --- | --- |
| Conserver | PostgreSQL, migrations SQLx additives, outbox, audit append-only, lease fencing, quotas/budgets, ModelProvider/ModelRouter, stockage fichier, Qdrant comme projection, keyring Tauri, intégrations v3, ToolDescriptor v1, protections SSRF |
| Étendre | agent_run_jobs en activations de task, événements en enveloppe canonique, aro-core en contrats agent v2, aro-agent en planner/context/policy services, Tauri en edge executor, mémoire en domaine ACL/versionné |
| Encapsuler | API et DTO historiques derrière AgentRuntimeFacade, façade TypeScript, CustomAgentDefinition, AgentRun/Step/Artifact |
| Déprécier après migration | exécution immédiate HTTP, mutation directe des runs, checkpoint_summary, registre ToolRef, fallbacks agent de démonstration, agent.delegate sans runtime |
| Ne pas réutiliser comme source de vérité | Redis, Qdrant, état worker en RAM, timeline UI, réponses modèle brutes |

## 9. Contraintes non négociables

- aucune donnée nécessaire à la reprise ne vit seulement en RAM ;
- aucune action externe irréversible sans ledger, stratégie d’idempotence et policy ;
- aucune permission de sous-agent supérieure à l’intersection parent/utilisateur/organisation ;
- aucune donnée cross-tenant sans grant explicite et vérifié ;
- aucune reprise à partir d’un prompt brut non versionné ;
- aucun événement métier mutable ;
- aucun succès worker sans résultat validé et finalisation transactionnelle ;
- aucune migration destructive pendant le double-run ;
- aucune activation durable tenue par une requête HTTP ;
- aucun secret brut dans événements, checkpoints, logs, prompts ou artifacts.

## 10. Conclusion

ARO doit évoluer par strangler pattern. Le socle durable n’est pas à réécrire ; il faut lui donner un modèle de domaine complet, un reducer de transitions unique, des activations courtes par tâche, des checkpoints immuables et un journal d’effets. Le runtime local et le runtime cloud doivent implémenter les mêmes contrats, avec des exécuteurs différents. La migration ne peut commencer qu’après gel des contrats v2, des invariants et des tests de caractérisation.
