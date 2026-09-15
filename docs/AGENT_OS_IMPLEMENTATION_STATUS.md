# Agent OS — état d’implémentation

Date de référence : 2026-07-16.

Ce document accompagne l’audit, l’architecture cible et le plan en 20 phases. Il décrit ce qui est effectivement livré dans le premier jalon de refactorisation. Il ne signifie pas que les 20 phases sont terminées.

## Jalon 1 — fondations du registre d’outils et sécurité de l’egress

Statut : implémenté et validé par les tests ciblés et la suite Rust complète disponible au moment du jalon.

### Contrat `ToolDescriptor` v1

`aro-core` expose désormais un contrat sérialisable et validable couvrant :

- identité canonique, version de schéma et version sémantique ;
- catégorie, descriptions, capacités et aliases ;
- schémas JSON d’entrée et de sortie ;
- permissions, niveau de risque et confirmation ;
- type d’exécution, environnement, timeout, idempotence et effets de bord ;
- politique de retry et limites d’usage ;
- dépendances, capture de données et observabilité ;
- statut, provenance et propriétaire.

La validation refuse notamment les identifiants non namespacés, les versions non sémantiques, les schémas vides, les timeouts ou retries non bornés et les valeurs dupliquées.

### Catalogue compatible

Le registre embarqué de `aro-agent` contient maintenant des descripteurs complets pour les deux outils réellement exécutables :

- `core.search.web`, alias historique `web.search` ;
- `core.web.page.read`, alias historique `web.fetch`.

La façade `ToolRef` reste disponible pour ne pas casser les consommateurs existants. Les outils fantômes précédemment annoncés au modèle (`workspace.read`, `workspace.write`, `shell.run`, `artifact.create`) ne sont plus exposés tant qu’aucun exécuteur réel ne leur correspond.

### Compatibilité d’exécution

`aro-tools` accepte les identifiants canoniques et leurs aliases historiques. Les providers de test, le runtime et l’API utilisent désormais les identifiants canoniques, sans rupture du format persistant actuel.

### Correctifs de sécurité

Deux élargissements involontaires de l’accès réseau ont été supprimés :

1. la vérification de domaine autorisé appliquait la négation du résultat et inversait donc l’allowlist ;
2. l’API activait un accès réseau sans restriction dès qu’un profil de l’organisation autorisait le réseau, même si le run courant ne l’avait pas demandé.

Le mode `Auto` est maintenant fermé par défaut pour l’enrichissement web implicite. Seul le mode `On` ouvre explicitement l’accès, sous réserve de la politique et de l’allowlist de domaines.

## Compatibilité et migrations

Aucune migration SQL ni bascule de protocole agent n’est introduite dans ce jalon. Le contrat riche est ajouté derrière une façade compatible. Cette stratégie permet de mesurer et stabiliser les descripteurs avant la création du registre persistant prévue à la phase suivante.

## Validation

Résultats obtenus :

| Vérification | Résultat |
| --- | --- |
| `cargo test -p aro-core -p aro-agent` | vert, 30 tests ciblés |
| `cargo test -p aro-runtime -p aro-tools` | vert, 28 tests ciblés |
| `cargo test --workspace` | vert, 160 tests réussis et 1 test Qdrant ignoré faute de service local |
| `npm run check` | vert, 0 erreur et 0 avertissement |
| `npm run test:unit` | vert, 76 tests |
| `npm run test:components` | vert, 18 tests |
| `npm run build` | vert ; avertissements de découpage du bundle documentés |
| `cargo fmt --all --check` | vert avant l’apparition concurrente d’un crate incomplet `aro-agent-domain` |
| `cargo clippy --workspace --all-targets -- -D warnings` | les écarts trouvés ont été corrigés ; relance globale bloquée par les modules encore absents du crate concurrent `aro-agent-domain` |

Les tests ajoutés couvrent la sérialisation du descripteur, les erreurs de validation, les bornes de retry/timeout, la validité du catalogue, l’absence d’outils fantômes, la compatibilité alias/canonique et la politique réseau fermée par défaut.

Le build Vite signale un chunk principal d’environ 763 kB et le chargement simultanément statique et dynamique de l’API Tauri. Ce sont des optimisations P1 de découpage frontend, pas des erreurs de production du jalon.

## Limites connues du jalon

- Le registre reste embarqué en mémoire ; il n’est pas encore persistant ni administrable par API.
- Le modèle reçoit toujours la façade textuelle héritée, pas encore un appel d’outil natif typé.
- Le moteur reste séquentiel ; le DAG, le scheduler, la reprise et la compensation appartiennent à des phases ultérieures.
- La file durable `agent_run_jobs` n’est pas encore raccordée au chemin principal `/agent/runs`.
- Skills, plugins et MCP ne consomment pas encore le registre unifié.
- La fédération de recherche, la mémoire hybride, les permissions fines, la voix complète et l’observabilité de production restent à implémenter selon le plan.

## Prochain jalon recommandé

Phase 3 du plan : introduire le registre persistant et son service de résolution, avec migration additive, API de lecture, validation à l’écriture, aliases, révocation, cache et fallback vers le catalogue embarqué. La bascule de l’exécution ne doit intervenir qu’après validation de parité et tests de rollback.
