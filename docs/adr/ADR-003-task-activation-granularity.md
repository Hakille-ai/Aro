# ADR-003 — Activations bornées au niveau Task

Statut : accepté le 16 juillet 2026.

## Contexte

Un job unique plafonné à un jour ne représente ni mission multi-jour, ni DAG, ni fan-out/fan-in. Garder un process vivant empêche une reprise et un déploiement sûrs.

## Décision

Run est la mission durable ; Task est l’unité d’ordonnancement ; Step est une intention ; StepAttempt est un essai sous lease. Un worker exécute une activation courte d’une Task, checkpoint puis yield avant son quantum.

Les dépendances rendent les Tasks ready. Une activation est au moins une fois ; sa finalisation interne est protégée par transaction, expected_version et fencing.

## Conséquences

- parallélisme et quotas par task/environnement ;
- reprise fine sans process long ;
- modèle de données et scheduler plus riches ;
- les opérations longues doivent exposer polling/callback ou produire des checkpoints périodiques.

## Rollback

Un flag limite temporairement les nouveaux PlanVersion à une seule Task linéaire. Le schéma multi-task reste additif.
