# Validation de l'implémentation commerciale

Vérifications locales du 21 septembre 2026. Les résultats ci-dessous concernent le code et des données de test, sans paiement réel ni déploiement public.

| Vérification | Résultat observé |
| --- | --- |
| `cargo test -p aro-core billing --lib` | 5 tests réussis : droits expirés, calcul séparé, plafonds, arrondis et dépassements |
| `cargo test -p aro-api billing::tests --bin aro-api` | 4 tests réussis : signature brute/expiration, prix, identifiants, périmètre des écritures payantes |
| `cargo test -p aro-api compute::tests --bin aro-api` | 1 test réussi : réservation UTF-8 et limites avant inférence |
| `cargo test -p aro-store --test billing -- --ignored` avec `ARO_BILLING_TEST_DATABASE_URL` | Test PostgreSQL réel réussi : concurrence, reprise, règlement unique, isolement, sièges, contrat idempotent et réponse purgée |
| `cargo check -p aro-store --example billing_admin` | Réussi |
| `cargo check -p aro-desktop` | Réussi |
| Tests Vitest modèle de facturation | 3 tests réussis |
| Tests Vitest composant BillingSettings | 3 tests réussis |
| `npm run check --workspace @aro/desktop` | 0 erreur ; 71 avertissements dans 12 fichiers existants hors facturation |
| `npm run build --workspace @aro/desktop` | Réussi ; avertissement sur les gros bundles |
| `flutter analyze` | Aucun problème détecté |
| Playwright, composant réel avec réponses API de test | Contrôle visuel desktop et largeur 390 px ; aucune erreur console observée |

Le test PostgreSQL utilise une base dédiée, sans modification des données de l'application. Pour la vérification RLS, un rôle sans privilège propriétaire est créé dans une transaction puis annulé ; les rôles de production ne sont pas modifiés. Le test est explicitement ignoré par défaut afin de ne pas viser une base par accident. Il doit être lancé avec `--ignored` et une base jetable dont l'opérateur possède les droits de création de rôle.

Les captures locales sont dans `output/playwright/billing-desktop.png` et `output/playwright/billing-mobile-web.png`. Elles utilisent des soldes fictifs exclusivement pour contrôler le rendu. Elles ne constituent pas un test Stripe ou une preuve de paiement.

Restent indispensables avant lancement : parcours Stripe de bout en bout avec clés de test et webhooks, modèle d'inférence réellement configuré et mesures de coût, validation juridique et titularité, configuration d'exploitation/alertes/rétention, et essais mobile sur appareil. Les paiements restent désactivés par défaut ; les migrations ont été exercées uniquement sur la base de test.
