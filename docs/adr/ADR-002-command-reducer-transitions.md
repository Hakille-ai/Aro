# ADR-002 — Commandes idempotentes et reducer unique

Statut : accepté le 16 juillet 2026.

## Contexte

Les chemins historiques mutent directement les statuts et peuvent diverger entre API, queue et UI. Les transitions doivent être persistées, validées, auditées, idempotentes et observables.

## Décision

Toute mutation v2 passe par CommandEnvelope et un reducer pur de aro-agent-domain. Le handler vérifie tenant, actor, expected_version, policy et déduplication, puis persiste projection, AgentEvent et outbox dans une transaction.

Les reducers ne lisent ni base, ni horloge, ni réseau. EventId et temps sont fournis par TransitionContext. Un terminal ne reprend jamais ; retry manuel crée une nouvelle tentative ou un nouveau run selon le sujet.

## Conséquences

- tests exhaustifs et déterministes ;
- un seul endroit pour les guards ;
- optimistic concurrency explicite ;
- la compatibilité legacy exige un adapter, pas une seconde machine v2.

## Rollback

Le RuntimeRouter peut cesser d’envoyer de nouvelles commandes vers v2. Les événements déjà écrits restent vrais ; les runs v2 sont drainés par une version compatible.
