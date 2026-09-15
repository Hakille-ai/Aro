# ADR-004 — Checkpoints immuables, chiffrés et migrables

Statut : accepté le 16 juillet 2026.

## Contexte

checkpoint_summary est une chaîne de présentation et ne permet pas de reprendre après crash ou autour d’un effet externe.

## Décision

Checkpoint est un snapshot immuable contenant refs d’agent/run/task/step, PlanVersion, états reducer, work memory résumée, résultats/artifacts, effets, événements consommés, approvals, budgets, environnement et versions runtime/schema. Le payload est chiffré, digesté et lié au checkpoint précédent.

La création est atomique avec current_checkpoint_id, transition et event. La restauration vérifie tenant, digest/MAC, migrations et effets avant de créer une nouvelle tentative.

## Conséquences

- reprise déterministe de l’état interne ;
- les sorties modèle/tool validées sont référencées, jamais rejouées ;
- besoin de codecs/migrators et rotation de clés ;
- aucun secret ou log brut dans le snapshot.

## Rollback

Pour un run sans effet, v2 peut reprendre au début de la Task si le codec est indisponible. Un checkpoint invalide est quarantined ; il n’est jamais ignoré silencieusement.
