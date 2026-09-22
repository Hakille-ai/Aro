# ARO mobile

Application Flutter Android/iOS reliée à l’API Rust ARO. L’accueil est le chat ; la barre latérale organise conversations, projets et dossiers. Les couleurs et logos viennent du desktop. L’interface s’adapte au téléphone et à la tablette, avec thèmes clair, sombre, système et OLED.

## Démarrer

Depuis la racine du dépôt, sous Windows :

```powershell
./scripts/mobile.ps1 check
./scripts/mobile.ps1 test
./scripts/mobile.ps1 preview
```

L’aperçu est à http://127.0.0.1:1440 et transmet les requêtes à l’API locale sur 8710. Il utilise de vraies données et ne fournit pas de réponse IA fictive. L’API doit être démarrée séparément. `ARO_PREVIEW_API` permet de choisir un autre port local.

Sur un appareil avec Flutter installé :

```sh
cd apps/mobile
flutter pub get
flutter run --dart-define=ARO_API_URL=https://votre-serveur-aro.example
```

L’adresse peut aussi être renseignée sur l’écran de connexion. Les versions de production exigent HTTPS. Android debug autorise HTTP pour l’émulateur (`http://10.0.2.2:8710`). Une adresse localhost désigne le téléphone lui-même, pas le PC.

## Fonctions raccordées

- Connexion, inscription, récupération/réinitialisation du mot de passe et acceptation d’invitation ; sessions stockées avec flutter_secure_storage, renouvellement partagé entre requêtes concurrentes.
- Chat, historique, Markdown, copie, modification du texte pour renvoi, « Retenir » vers la mémoire serveur (repli local honnête hors-ligne), recherche web, pièces jointes cloud, choix de personnalité et destination. Dictée et lecture utilisent les services du téléphone.
- Mentions `@` dans le composer (fichiers cloud, skills, agents, MCP, plugins, profils, modèles) avec bottom-sheet de recherche et injection de contexte `Contexte du projet (références @)` comme le desktop.
- Palette de commande mobile (Ctrl/⌘ K ou loupe) : conversations, projets, dossiers, actions, réglages.
- Cloche de notifications avec compteur non-lus et centre in-app : liste serveur, marquer-lu au tap, swipe-pour-supprimer, tout-lire, pull-to-refresh.
- Second facteur TOTP dans Profil › Sécurité : configuration (secret + `otpauthUrl`), activation/désactivation avec preuve de possession RFC 6238.
- Administration organisation : renommer/créer l’espace, inviter (nom/e-mail/rôle), changer les rôles, retirer les membres, lister et révoquer les invitations en attente.
- Bouton Régénérer sur la dernière réponse (réenvoi honnête du dernier prompt, le cloud n’exposant pas de route `/regenerate`) et bannière hors-ligne ambre dédiée.
- Monitoring : totaux tokens, top modèles, barres des 20 derniers événements et jauge vs budget 8 192 tokens.
- Streaming throttlé (~8 Hz) pour éviter de reconstruire l’app à chaque token.
- Création, renommage, suppression et déplacement des conversations, projets et dossiers. Ordre de la barre latérale et brouillons conservés localement par compte/espace.
- Plans liés à la conversation aux 4 états desktop (`pending → in_progress → completed → error`), progression, statuts `active/completed/archived`, ajout/suppression d’étapes et génération IA via le chat.
- Sorties & artefacts : extraction des blocs de code/diff des réponses, recherche, filtres, viewer unifié avec numéros de ligne, `+`/`−`, copie et enregistrement comme fichier cloud, rejet.
- Fichiers cloud : recherche rapide (quick open), aperçu texte/code (tronqué à 200 000 caractères comme desktop) et images, suppression confirmée.
- Exécutions d’agents : compteurs Actifs/En file/Finis, pause/reprise/annulation, actualisation.
- Vingt-trois destinations de réglages (parité desktop hors alias `general`) : modèles, prompt système, voix, chemins, système, préférences, notifications, monitoring, profil, organisation, personnalités, mémoire, skills, plugins, MCP, hooks, planificateur, agents, navigateur, appareil, recherche, permissions et raccourcis.
- Éditeurs reliés aux collections existantes pour les personnalités, souvenirs, prompts, skills, MCP, hooks, tâches planifiées et définitions d’agents. Profil, modèle, recherche, notifications in-app et apparence modifiables ; changement d’organisation avec remise à zéro de la navigation et des données affichées.

## Périmètre et limites

La présence des pages ne signifie pas encore une parité intégrale avec chaque option desktop. Plugins, permissions, appareil/chemins et secrets fournisseurs restent consultatifs. La facturation, le terminal, l’éditeur de code avec application de diffs sur PC et le contrôle du PC ne sont pas portés ici (l’enregistrement d’artefacts se fait comme fichier cloud). Le navigateur intégré et le contrôle natif du téléphone demandent des modules et parcours de consentement dédiés ; les liens s’ouvrent dans le navigateur de l’appareil avec historique local.

Notes d’honnêteté : la route `/auth/login` du serveur ne demande pas encore le code TOTP à la connexion — l’enrôlement MFA reste préparatoire. Il n’existe pas de route `DELETE /users/me` côté serveur, donc la suppression de compte n’est pas proposée. Le cloud n’expose pas de route `/regenerate` : Régénérer renvoie le dernier prompt comme nouveau tour, sans faux endpoint.

Les brouillons sont chiffrés dans le trousseau OS (`flutter_secure_storage`, migration automatique depuis l’ancien emplacement, repli prefs uniquement si le trousseau est indisponible). Les opérations métier exigent le réseau. Le pré-contrôle `GET /assistant/status` bloque l’envoi quand le serveur ne peut pas générer (erreur éphémère, historique jamais pollué) ; la page Modèles indique par provider ce qui s’exécute réellement sur le téléphone. Le sélecteur du composer applique une surcharge par message (`modelId` + `provider`, cloud opt-in inclus) : le serveur sert exactement le modèle choisi ou échoue honnêtement, sans substitution — comme sur desktop. Les envois portent `promptScope: "personal"` pour que le serveur complète les consignes du mode et les souvenirs, comme le harnais desktop. Le serveur actuel peut terminer la génération avant de transmettre ses événements SSE. Arrêter la réception mobile ne garantit donc pas l’arrêt du travail serveur. Une réponse IA réelle exige un fournisseur configuré. La dictée, le TTS et les permissions doivent encore être validés sur appareils.

Le correctif de création des plans se trouve dans `crates/aro-store/src/lib.rs` : il renseigne et vérifie `conversation_id`. Un serveur lancé avant cette correction doit être recompilé et redémarré.

## Validation

- Tests Flutter (36) : décodage SSE fragmenté/Unicode, renouvellement et isolation de session, absence de rejeu des écritures dans une autre organisation, formulaires, navigation et vingt-trois réglages sur écran étroit, plus mentions (`detect/filter/apply/extract`), plans (cycle 4 états, progression), artefacts (extraction, diffs), monitoring (totaux, fenêtrage), brouillons chiffrés (round-trip trousseau, migration prefs, repli, purge par compte), surcharge modèle par message (payload, sans mutation org) et clavier ouvert sans overflow (dont bannière IA affichée).
- Test PostgreSQL `plans_require_an_accessible_conversation_and_persist_it` : création, persistance du rattachement, refus des conversations d’un autre espace et des plans sans conversation.
- `scripts/verify-flutter-api.ps1` : contrôle des écritures avec un compte local isolé ; nettoie les objets créés, conserve son identité de test.
- Captures de navigateur dans `output/playwright/mobile-*.png` à la racine du dépôt.

La compilation web valide le frontend. Elle ne valide ni un APK signé ni iOS. Sur le poste initial de développement, le SDK Android et Xcode ne sont pas disponibles.

## Distribution native

Android : installer le SDK Android/JDK puis créer `android/key.properties` (ignoré par Git) avec `storeFile`, `storePassword`, `keyAlias`, `keyPassword`. `storeFile` est relatif au dossier android ou absolu. La configuration release ne réutilise pas la clé debug.

```powershell
./scripts/mobile.ps1 android -ApiUrl https://votre-serveur-aro.example
```

iOS : utiliser un Mac avec Xcode, configurer l’équipe de signature et le bundle identifier, puis `flutter build ipa --dart-define=ARO_API_URL=https://votre-serveur-aro.example`. Les icônes sont dérivées du logo desktop ; `tool/icons.ps1` les régénère sous Windows.

## Structure

`lib/core` contient transport et état métier ; `lib/ui` les styles ; `lib/features` les écrans et le catalogue de réglages. Aucun secret de serveur ni clé de fournisseur n’est intégré à l’application. Les modules natifs futurs doivent passer par des interfaces explicites de capacités pour exposer correctement les différences Android/iOS.
