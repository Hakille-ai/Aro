# aro-agent-domain

Contrats provider-neutral et machines à états pures du runtime durable ARO.

## Frontière

Ce crate connaît :

- les identifiants typés UUIDv7 pour les nouvelles ressources ;
- les specs immuables d’AgentVersion ;
- les commandes et événements versionnés ;
- les projections minimales Run, Task, Step, StepAttempt, Approval et Environment ;
- les transitions et invariants ;
- la canonicalisation JSON et les digests SHA-256.

Il ne connaît pas PostgreSQL, Axum, Redis, les workers, les fournisseurs de modèles, les exécuteurs de tools ou le frontend. Ces couches doivent adapter leurs données au contrat sans ajouter de transition parallèle.

## Contrat de persistance

Pour chaque commande, le futur CommandHandler doit, dans une transaction :

1. dédupliquer command_id et IdempotencyKey ;
2. fixer tenant, actor, expected_version et TransitionContext ;
3. verrouiller et charger l’aggregate ;
4. appeler le reducer pur ;
5. écrire aggregate, AgentEvent et outbox atomiquement ;
6. mémoriser le résultat de commande ;
7. retourner le même résultat lors d’un replay.

Le reducer ne promet pas l’exactly-once externe. StepAttempt et l’effect ledger fournissent les frontières nécessaires.

## Invariants déjà exécutables

- un tenant différent et une version optimiste périmée sont refusés avant transition ;
- aucun état terminal Run/Task/Step ne peut être ressuscité ;
- pause, annulation ou destruction active exigent une frontière sûre ;
- un résultat final n’est attaché qu’à une transition terminale valide ;
- une compensation ne remplace pas le résultat original de la Task ;
- un effet préparé dont le lease est perdu passe en EffectUnknown ;
- EffectUnknown exige ReconcileEffectCommitted ou ReconcileEffectAbsent ;
- un effet committé ne peut pas être marqué failed ou abandonné puis rejoué ;
- un Approval expire, est single-use et correspond au digest exact de l’action ;
- l’acteur de décision d’un Approval est distinct de son consommateur ;
- un Environment détruit possède un cleanup receipt ;
- chaque transition incrémente exactement AggregateVersion et produit un événement causal ;
- recorded_at ne peut précéder requested_at ;
- les erreurs et raisons persistées sont des codes machine bornés, pas des messages sensibles ;
- la sérialisation JSON publique est stable : enveloppes camelCase, états/types snake_case.

## Intégration progressive

Le crate est membre du workspace mais n’est encore consommé par aucun chemin utilisateur. La première intégration autorisée est un repository/CommandHandler v2 sous feature flag avec exécuteur déterministe de test. Les types historiques aro-core::AgentRun restent inchangés jusqu’au double-read et à la migration explicite.
