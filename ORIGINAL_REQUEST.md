# Original User Request

## Initial Request — 2026-09-04T17:33:12Z

Implémentation complète de niveau professionnel (standard Apple / Google) des fonctionnalités clés de l'espace de travail IA ARO, en déployant en priorité le Visual Diff interactif pour la revue et l'application de code ainsi que la Feuille de route / Plan de travail autonome connectée au panneau latéral droit.

Working directory: c:/Users/Stagiaire/Documents/ARO
Integrity mode: development

## Requirements

### R1. Visual Diff & Gestionnaire d'Artefacts Interactif (Panneau Droit & Chat)
- Intégrer un visualiseur de différences interactif (Diff split & unified) avec coloration syntaxique des ajouts, suppressions et modifications de lignes.
- Fournir des actions directes et sécurisées : « Appliquer au projet », « Rejeter », et « Copier le code ».
- Rendre la carte « Sorties & Artefacts » de l'espace de travail (`RightPanel.svelte`) vivante : affichage des fichiers générés/modifiés dans la conversation active avec prévisualisation immédiate.

### R2. Feuille de Route & Plan de Travail Autonome (Checklist & Tâches Dynamiques)
- Permettre à l'IA d'instancier et de structurer des plans d'action (Plan -> Tâches -> Statuts) lors de requêtes de développement ou de refactorisation.
- Connecter l'onglet « Plan de Travail » (`RightPanel.svelte`) aux API de plans (`listPlans`, `createPlan`, `updatePlan`, `deletePlan`) avec mise à jour en direct (état en cours, coche terminée, gestion des erreurs).
- Permettre à l'utilisateur d'ajouter manuellement des tâches ou de demander à l'assistant d'élaborer une feuille de route pour le projet.

### R3. Dispatcher Multi-Agents & Vue d'Activité
- Afficher les voies d'exécution et les sous-agents d'arrière-plan dans l'onglet « Sous-Agents » avec compteurs d'activité synchronisés (Actifs, En file, Terminés).
- Connecter le visualiseur de logs en direct (`AgentLiveLogViewer`) pour inspecter la réflexion et les actions des agents.

### R4. Mentions `@` dans le Composer
- Permettre l'autocomplétion des fichiers et dossiers du projet lors de la saisie de `@` dans la barre de chat pour faciliter l'injection de contexte ciblé.

## Acceptance Criteria

### Rendu Visuel & Expérience Utilisateur
- [ ] L'onglet « Sorties & Artefacts » liste les fichiers modifiés et propose une vue Diff claire avec prévisualisation avant application.
- [ ] Le bouton « Appliquer » écrit les modifications dans les fichiers réels du projet sur le disque et met à jour l'arborescence.
- [ ] L'onglet « Plan de Travail » permet de visualiser et cocher les étapes franchies avec indicateurs de statut visuels.
- [ ] L'onglet « Sous-Agents » reflète l'activité réelle des agents sans écran vide statique.
- [ ] `npm --workspace @aro/desktop run check` rapporte 0 erreur.
- [ ] `npm --workspace @aro/desktop run test:unit` et `test:components` passent à 100%.

## Follow-up — 2026-09-11T21:44:50Z

Implémenter pour le backend ARO un système de mémoire cognitive multi-niveaux (court terme, moyen terme épisodique, long terme sémantique) garantissant une rétention sans faille des informations, une compression de contexte sans perte et des capacités de rappel avancées pour les agents.

L'utilisateur demande explicitement une coordination en équipe d'agents : "lance des agents pour coordonnez-vous ensemble pour voilà travailler en équipe quoi pour voilà réaliser ça extrêmement bien, réaliser ça bien avec du bon code bien fait, bien structuré".

Working directory: c:/Users/Stagiaire/Documents/ARO
Integrity mode: development

## Requirements

### R1. Architecture mémoire hiérarchique unifiée (Court, Moyen et Long terme)
Fournir un cycle de vie complet et multi-niveaux de la mémoire dans le backend :
- Mémoire de travail immédiate (tampon de messages récents, variables de session actives).
- Mémoire épisodique à moyen terme (synthèse de sessions, jalons et contexte des tâches intermédiaires).
- Mémoire sémantique à long terme (faits extraits, préférences et connaissances persistées avec scoring dynamique de saillance, récence et consolidation).

### R2. Gestion de contexte adaptatif et compression sans perte
Mettre en place un mécanisme de consolidation continue en arrière-plan et de compaction incrémentale de contexte empêchant la saturation de la fenêtre de tokens lors de conversations longues (100+ tours), tout en préservant systématiquement les décisions, entités critiques et contraintes de l'utilisateur.

### R3. Recherche hybride et outillage mémoire pour les agents
Intégrer une recherche hybride haute performance couplant la recherche plein texte SQLite FTS5 et la recherche sémantique vectorielle (`aro-vector`). Exposer au runtime d'agents des primitives complètes d'auto-mémorisation et d'introspection (`memory_save`, `memory_search`, `memory_recall`, `memory_update`, `memory_forget`).

## Acceptance Criteria

### Rétention et rappel sur conversations longues
- [ ] Une suite de tests d'intégration simulant une conversation d'au moins 100 tours valide que des faits spécifiques introduits aux premiers tours (ex: tours 5 et 20) sont rappelés avec exactitude lors d'une requête au tour 95+.
- [ ] Le volume de tokens injecté dans le contexte ne dépasse jamais la limite configurée tout en conservant les faits critiques nécessaires aux réponses.

### Intégrité et robustesse du stockage
- [ ] `cargo test -p aro-memory -p aro-vector -p aro-runtime` s'exécute avec 100% de succès et 0 régression sur l'ensemble des suites de tests existantes.
- [ ] Les opérations de lecture/écriture, consolidation et recherche hybride sont thread-safe, transactionnelles et résilientes aux redémarrages.

### Outillage et cycle de vie agent
- [ ] Le runtime de l'agent peut invoquer et exploiter de manière autonome les outils mémoire pour stocker et rechercher des informations pendant son raisonnement.
- [ ] La reprise après redémarrage ou reprise d'une conversation retrouve fidèlement le contexte épisodique et les faits consolidés.

