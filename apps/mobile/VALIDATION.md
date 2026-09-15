# Validation locale — 13 septembre 2026

- Flutter 3.47.4 / Dart 3.13.3 sous Windows.
- `flutter analyze` : aucune anomalie.
- `flutter test` : 13 tests réussis.
- `flutter build web --debug --no-wasm-dry-run --dart-define=ARO_API_URL=http://127.0.0.1:1440` : réussi.
- `cargo check -p aro-store` : réussi.
- Test `plans_require_an_accessible_conversation_and_persist_it` avec DATABASE_URL local chargé : 1 test réussi, PostgreSQL réellement utilisé.
- Navigateur Chromium, 390 × 844 : inscription réelle, restauration de session après rechargement, chat d’accueil, création de projet et dossier, navigation/réglages, recherche de réglage et thème sombre vérifiés. Zéro erreur console après rechargement du dernier build.
- Captures : `output/playwright/mobile-auth.png`, `mobile-home.png`, `mobile-sidebar.png`, `mobile-settings.png`, `mobile-dark.png`, à la racine du dépôt.

Le premier contrôle API avait 12 résultats réussis et une erreur de création de plan. Le correctif de plan a ensuite été validé par le test PostgreSQL ci-dessus. La tentative de recompilation du serveur local a échoué car `target/debug/aro-api.exe` est verrouillé par le processus déjà lancé ; ce processus n’a pas été interrompu. La nouvelle route de création de plan n’a donc pas été revalidée par HTTP sur ce serveur.

Le build APK, le simulateur iOS, les appareils physiques, la dictée/TTS natifs, une réponse avec un vrai fournisseur IA et le workflow CI ajouté n’ont pas été exécutés/validés ici. Le build web utilise JavaScript ; WebAssembly n’est pas validé avec la dépendance TTS actuelle.

Le compte de test visuel et ses deux objets restent uniquement dans l’API locale. Aucun déploiement, envoi d’e-mail externe ni publication sur un store n’a été réalisé.
