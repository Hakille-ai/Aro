# Dossier de refonte du runtime d’agents

Date du baseline : 16 juillet 2026.

L’implémentation n’a pas commencé. Ces documents constituent le point de contrôle demandé avant toute réécriture :

1. AGENT_RUNTIME_AUDIT_2026-07-16.md — cartographie, baseline dynamique, problèmes, risques et composants réutilisables.
2. AGENT_RUNTIME_TARGET_ARCHITECTURE.md — concepts, architecture, lifecycle, machines à états, moteur durable, données, API, reprise, sécurité et production.
3. AGENT_RUNTIME_DELIVERY_PLAN.md — migration additive en 21 phases avec objectif, composants, fichiers, schémas, migrations, services, endpoints, événements, risques, dépendances, tests, acceptation et rollback.

Les fichiers AGENT_OS_AUDIT_2026-07.md, AGENT_OS_TARGET_ARCHITECTURE.md et AGENT_OS_REFACTORING_PLAN.md restent des documents historiques plus larges. Le présent dossier est autoritatif pour le runtime d’agents ; il tient compte des changements concurrents observés pendant l’audit.

## Matrice des livrables

| Livrable demandé | Document et section |
| --- | --- |
| Cartographie actuelle | Audit §3 |
| Liste des problèmes/risques | Audit §5 à §7 |
| Composants réutilisables/à remplacer | Audit §8 |
| Architecture cible | Architecture §1 à §3 |
| Concepts et responsabilités | Architecture §2 |
| Cycle de vie | Architecture §4 |
| Machine à états et transitions | Architecture §5 |
| Choix du runtime durable | Architecture §6 à §7 |
| Modèle de données | Architecture §8 |
| Checkpoints et reprise/rollback | Architecture §9 |
| Multi-tâches/DAG | Architecture §10 |
| Isolation du contexte | Architecture §11 |
| Mémoire | Architecture §12 |
| Sous-agents | Architecture §13 |
| Multi-environnements/workspaces | Architecture §14 |
| Routage modèles | Architecture §15 |
| Tools, skills, plugins et MCP | Architecture §16 |
| Permissions/approvals/secrets | Architecture §17 |
| Événements/triggers/schedules | Architecture §18 |
| Idempotence/erreurs/récupération | Architecture §19 |
| Contrats API et streaming | Architecture §20 |
| Frontend | Architecture §21 |
| Observabilité | Architecture §22 |
| Plan de tests | Architecture §23 et Plan phase 18 |
| Critères production | Architecture §24 |
| Plan de migration/déploiement | Plan phases 1 à 21 |
| Première tranche | Plan §3 |

## Décision attendue

Avant le code, valider ou amender :

1. PostgreSQL comme moteur durable initial, avec Redis seulement optionnel pour les réveils ;
2. activations courtes par Task au lieu d’un job multi-jour ;
3. distinction Agent/AgentVersion/Instance/Run/Session/Task/StepAttempt ;
4. reducer unique et commandes idempotentes ;
5. checkpoint chiffré immuable et effect ledger ;
6. migration progressive derrière flags, sans suppression legacy avant canary complet.
