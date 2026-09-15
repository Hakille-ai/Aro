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
- Chat, historique, Markdown, copie, modification du texte pour renvoi, modes, recherche web, pièces jointes cloud, choix de personnalité et destination. Dictée et lecture utilisent les services du téléphone.
- Création, renommage, suppression et déplacement des conversations, projets et dossiers. Ordre de la barre latérale et brouillons conservés localement par compte/espace.
- Plans liés à la conversation, étapes cochables ; consultation des fichiers et des exécutions d’agents, commandes de pause/reprise/annulation proposées par l’API.
- Vingt destinations de réglages : modèles, prompt système, voix, chemins, système, préférences, monitoring, profil, organisation, personnalités, mémoire, skills, plugins, MCP, hooks, planificateur, agents, recherche, permissions et raccourcis.
- Éditeurs reliés aux collections existantes pour les personnalités, souvenirs, prompts, skills, MCP, hooks, tâches planifiées et définitions d’agents. Profil, modèle, recherche et apparence modifiables ; changement d’organisation avec remise à zéro de la navigation et des données affichées.

## Périmètre et limites

La présence des pages ne signifie pas encore une parité intégrale avec chaque option desktop. Plugins, permissions, appareils/chemins et une partie du monitoring sont consultatifs. L’administration complète des membres, la facturation, les secrets fournisseurs, MFA, le terminal, l’éditeur de code, les diffs et le contrôle du PC ne sont pas portés ici. Le navigateur intégré et le contrôle natif du téléphone demandent des modules et parcours de consentement dédiés ; ils ne sont pas implémentés.

Les brouillons utilisent SharedPreferences ; ce n’est pas une base hors ligne chiffrée. Les opérations métier exigent le réseau. Le serveur actuel peut terminer la génération avant de transmettre ses événements SSE. Arrêter la réception mobile ne garantit donc pas l’arrêt du travail serveur. Une réponse IA réelle exige un fournisseur configuré. La dictée, le TTS et les permissions doivent encore être validés sur appareils.

Le correctif de création des plans se trouve dans `crates/aro-store/src/lib.rs` : il renseigne et vérifie `conversation_id`. Un serveur lancé avant cette correction doit être recompilé et redémarré.

## Validation

- Tests Flutter : décodage SSE fragmenté/Unicode, renouvellement et isolation de session, absence de rejeu des écritures dans une autre organisation, formulaires, navigation et vingt réglages sur écran étroit.
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
