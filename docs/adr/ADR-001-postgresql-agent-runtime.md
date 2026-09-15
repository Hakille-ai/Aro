# ADR-001 — PostgreSQL comme moteur durable initial

Statut : accepté le 16 juillet 2026.

## Contexte

ARO est un système Rust/Axum dont l’autorité transactionnelle, le multi-tenant, l’outbox et une queue à leases résident déjà dans PostgreSQL. Le runtime doit fonctionner en SaaS et on-prem sans imposer une seconde plateforme.

## Décision

PostgreSQL conserve définitions, commandes, projections, events, tasks, attempts, checkpoints, effets, budgets et timers. Les workers réclament des activations bornées avec SKIP LOCKED, lease_generation et fencing. Redis reste optionnel pour réveils, cache et sémaphores ; sa perte ne perd aucun état.

## Conséquences

- cohérence atomique avec permissions et données ARO ;
- exploitation initiale plus simple ;
- obligation de construire reducer, DAG, scheduler, checkpoints, reaper et outils ops ;
- aucun run multi-jour ne reste un job/process continu.

## Réévaluation

Réouvrir si au moins deux critères sont atteints : 10 000 activations/s soutenues, plus de 10 millions de timers, multi-région actif-actif, workflows polyglottes, équipe plateforme dédiée, ou SDK Rust Temporal GA accepté.

## Rollback

Le journal/outbox et les contrats de domaine restent transport-agnostic. Un futur orchestrateur consomme les mêmes commandes/événements ; PostgreSQL demeure autorité des données métier pendant la migration.
