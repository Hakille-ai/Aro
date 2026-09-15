# Runtime d’agents v2 — état d’implémentation

Date de référence : 16 juillet 2026.

## Tranche 1A — domaine et machines à états

Statut : implémenté et isolé ; aucun trafic utilisateur routé.

### Livré

- nouveau crate aro-agent-domain sans dépendance DB/HTTP/worker/provider ;
- identifiants typés pour les concepts du runtime ;
- AgentVersionSpec versionné avec politiques modèles/capabilities/mémoire/environnements, limites et budgets ;
- CommandEnvelope avec tenant, actor, idempotency key, expected version et causalité ;
- AgentEvent v1 provider-neutral ;
- états et reducers purs Run, Task, Step, StepAttempt, Approval et Environment ;
- canonicalisation JSON récursive et ContentDigest SHA-256 ;
- protection explicite des effets inconnus et approvals exact-hash ;
- ADR-001 à ADR-005.

### Validation ciblée

| Vérification | Résultat |
| --- | --- |
| cargo test -p aro-agent-domain | 12 tests réussis |
| cargo clippy -p aro-agent-domain --all-targets -- -D warnings | réussi |
| cargo fmt -p aro-agent-domain | réussi |

Les tests couvrent lifecycle complet, terminaux, safe boundaries, tenant/version, causalité, canonical hash, spec de routage, Task/Step/Attempt, effet inconnu, approval single-use, cleanup environment, contrat JSON et property test de non-résurrection.

### Baseline concurrent observé

Avant cette tranche, les corrections parallèles avaient déjà rendu verts aro-agent 11/11, aro-runtime 16/16, frontend 76/76 et svelte-check. Aucun de ces fichiers n’a été modifié par la tranche domaine.

### Limites explicites

- aucun schéma SQL v2 ;
- aucun CommandHandler transactionnel ;
- aucun worker d’activation v2 ;
- aucun checkpoint codec ;
- aucune API/UI v2 ;
- aucun adapter depuis les types historiques ;
- aucune promesse de production tant que les P0 de l’audit restent ouverts.

## Prochaine tranche

Tranche 1B : migration additive minimale agent_commands/events/runs/tasks/attempts, repositories RLS-aware, CommandHandler transactionnel et worker déterministe sous feature flag. Aucun raccordement de POST /agent/runs avant les tests double claim, fencing, kill/recovery et perte Redis.
