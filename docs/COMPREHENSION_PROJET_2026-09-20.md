# Compréhension du projet ARO — 20 septembre 2026

Analyse du checkout courant, y compris ses modifications non committées. Lecture du code, des manifests, migrations et workflows ; ce document ne constitue pas une validation de production ni un audit de sécurité exhaustif.

## Intention produit

ARO est un espace de travail personnel assisté par IA : conversations organisées en projets et dossiers, contexte de fichiers, modèles interchangeables, mémoire persistante, outils et suivi des agents. La voix, les extensions, MCP, la recherche web et les livrables enrichissent ce parcours. Le desktop est le point d'exécution local ; le mobile donne accès aux fonctions disponibles par l'API.

## Architecture effectivement observée

| Couche | Technologies | Responsabilité |
| --- | --- | --- |
| Desktop | Tauri 2, Svelte 5, TypeScript, Rust | Interface, fichiers locaux, moteur IA, événements de progression |
| Runtime | aro-runtime, aro-agent, aro-tools | Contexte, fournisseurs, boucle modèle/outils, exécution locale |
| Mémoire locale | aro-memory, SQLite, aro-vector | Conversations et mémoire locales, épisodes, recherche lexicale et vectorielle |
| API | Axum, Tokio, aro-store, PostgreSQL | Authentification, organisations, données synchronisées, jobs et configuration |
| Coordination | Redis | Verrous, limitation de débit, diffusion d'événements |
| Fichiers et vecteurs | MinIO/S3, Qdrant | Services de stockage et recherche selon configuration |
| Mobile | Flutter / Dart | Client Android/iOS utilisant les routes HTTP et SSE |
| Distribution | Docker, Compose, Helm, GitHub Actions | Développement, déploiement et vérifications automatisées |

Le mobile actuel n'est plus l'ancien client Expo. Le README racine décrit PostgreSQL comme source de vérité et SQLite comme mémoire historique, mais le moteur desktop utilise activement SQLite pour son fonctionnement. La distinction entre état local, données synchronisées et état serveur doit être explicitée.

## Parcours d'un message

### Desktop natif

1. `App.svelte::submitMessage` vérifie l'état de synchronisation, prépare la conversation, la personnalité, le modèle, les mentions de fichiers et les pièces jointes.
2. `transport.ts::sendMessageStream` appelle la commande Tauri `message_send_stream`.
3. La commande choisit le fournisseur et appelle `AssistantEngine::send_message_stream`.
4. Le runtime assemble le contexte, consulte la mémoire, appelle le modèle, valide les actions et exécute les outils demandés.
5. Les événements Tauri transmettent texte et étapes à l'interface.
6. Le résultat local est ensuite envoyé au cloud par `persist_local_result`. Un échec de cette synchronisation est journalisé ; le résultat local est conservé.

Sources : `apps/desktop/src/App.svelte:8025`, `apps/desktop/src/lib/api/transport.ts:2133`, `apps/desktop/src-tauri/src/main.rs:3505`, `crates/aro-runtime/src/lib.rs:1472`.

### Web et mobile

Les clients utilisent `/assistant/stream`. Le serveur vérifie l'accès à la conversation, prend un verrou, persiste le message, prépare éventuellement du contexte web et appelle le fournisseur. Le code actuel utilise `generate_stream` et transmet les fragments par SSE : la mention d'une génération nécessairement terminée avant émission, encore présente dans la documentation mobile, est dépassée pour ce chemin.

Ce parcours n'utilise pas la totalité du moteur local desktop. Il faut donc tester séparément les capacités des trois clients.

Source : `apps/api/src/handlers.rs:2922`.

## Mémoire et extensibilité

Le runtime raccorde mémoire de travail bornée, consolidation épisodique et souvenirs durables. La recherche fusionne résultats lexicaux et vectoriels. Les budgets peuvent dépendre des réglages et du modèle ; les commentaires parlant uniquement d'un plafond universel de 8 192 tokens ne résument pas tous les appels actuels.

Le code contient des outils pour les fichiers, le shell, l'exécution de code, les documents, le web et certaines commandes du système. Les crates plugins, skills et MCP séparent une partie de l'extensibilité. La présence du code ne prouve pas que chaque capacité fonctionne sur chaque plateforme.

`aro-agent-domain` définit une autre fondation de machines à états et contrats durables. La recherche des dépendances et usages n'a pas trouvé de raccordement à un parcours utilisateur ; son README le présente lui-même comme une intégration future.

## Écarts et points prioritaires

1. **Worker cloud incomplet pour les outils.** Dans `apps/api/src/agent_runner.rs`, la branche `AgentActionType::Tool` incrémente le compteur, stocke une étape et ajoute la demande à l'historique, sans appeler l'exécuteur ni fournir de résultat d'outil. La boucle existe désormais, mais elle n'a pas la parité du runtime desktop.
2. **Parseur SSE web fragile.** Dans `transport.ts::sendMessageStream`, `currentEvent` est réinitialisé à chaque lecture réseau. Si `event:` et `data:` arrivent dans deux lectures différentes, le type d'événement est perdu. Constat par lecture du code ; pas de reproduction réseau réalisée pendant cette analyse.
3. **Synchronisation non durable à cet endroit.** `cloud-sync.ts::syncKey` retire la mise à jour de la file avant l'appel réseau et ne la remet en attente que pour une erreur 429. Les autres échecs sont seulement journalisés. La file est en mémoire et ce gestionnaire ne comporte pas de mécanisme explicite de changement d'identité.
4. **Isolation des données à démontrer.** Le desktop ouvre un chemin SQLite commun `aro.sqlite3`. Le cloisonnement par utilisateur/organisation et les transitions de session demandent une vérification dédiée. Côté PostgreSQL, la migration des données privées diffère explicitement l'activation RLS ; les autres activations trouvées concernent notamment gouvernance, outils et intégrations. `tests/rls.rs` ne teste qu'une connexion, malgré le nom plus ambitieux de l'étape CI.
5. **Boucle locale sans plafond d'étapes.** `aro-runtime` ignore explicitement l'ancien `max_steps` pour arrêter la boucle ; celle-ci attend une action finale/pause ou une erreur. Un bornage indépendant du modèle mérite d'être vérifié pour la durée, les ressources et les coûts.
6. **Concentration du code.** Au moment de la lecture : `App.svelte` compte 10 907 lignes, le point d'entrée Tauri 4 973, les handlers API 7 317 et le store PostgreSQL 13 765. Les crates et composants séparent déjà des responsabilités, mais les fichiers centraux conservent beaucoup de coordination.

## Validation de cette analyse

- `npm run test:unit` : **28 fichiers, 267 tests réussis**. Le premier lancement était bloqué par `spawn EPERM` ; la relance avec les permissions adaptées a réussi.
- `npm run check` : **0 erreur et 69 avertissements dans 11 fichiers**, principalement des sélecteurs CSS inutilisés ; relance avec les permissions adaptées réussie.
- Les tests de composants Svelte sont une suite distincte des 267 tests unitaires.
- Pas de compilation Rust complète, de tests Flutter, de validation sur téléphone, de test utilisateur natif ni de preuve de production effectués pendant cette analyse.
- Aucun code applicatif modifié. Les modifications déjà présentes dans le checkout ont été conservées.

## Orientation

Le socle produit est étendu et plusieurs parcours sont réellement raccordés. L'effort prioritaire devrait porter sur la cohérence des frontières : qui exécute l'action, où les données font autorité, comment une opération interrompue reprend et quelles capacités chaque client peut réellement annoncer. Ajouter des écrans avant de stabiliser ces contrats augmenterait le nombre de comportements à maintenir.
