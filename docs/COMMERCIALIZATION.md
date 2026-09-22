# ARO — offres, facturation et exploitation

État : implémentation en préparation, paiements désactivés par défaut. Les prix sont des hypothèses de lancement, pas une preuve de rentabilité. Voir [le dossier juridique](../legal/README.md) avant toute publication.

## Ce qui est vendu

| Offre | Prix de lancement | Objet |
| --- | --- | --- |
| Community | Gratuit | Usages permis par PolyForm Noncommercial 1.0.0 ; modèles locaux et BYOK |
| Cloud | 12 € TTC / mois | Espace personnel et synchronisation hébergée |
| Business | 29 € HT / utilisateur / mois | Droits commerciaux, espace partagé et administration |
| Enterprise | À partir de 12 000 € HT / an | Périmètre, déploiement, assistance et engagements définis au contrat |

Le calcul IA est distinct de l'abonnement. BYOK signifie que le client paie son fournisseur directement ; cela n'accorde pas de droits commerciaux sur ARO. L'API ARO Compute utilise un crédit de service prépayé, sans transfert entre utilisateurs ni retrait d'argent. Ne pas proposer de calcul illimité.

Le catalogue partagé se trouve dans `packages/contracts/src/commercial-catalog.json`. Il est lu par Rust et l'interface desktop/web. Les montants de facturation sont des entiers en micro-euros (1 € = 1 000 000 unités). Les achats de crédit sont exprimés en centimes HT ; Stripe calcule les taxes. Un crédit de 10 € HT donne 10 € de consommation, pas le montant TTC payé.

## Configuration

La licence du code est appliquée : voir `LICENSE` et `legal/README.md`. Exécuter `npm run license:check` pour vérifier les textes standards et les périmètres. `npm run license:inventory` reconstruit l'inventaire déclaratif des dépendances ; `node scripts/generate-license-inventory.mjs --fetch-missing` complète les métadonnées depuis les registres publics.

Copier `legal/operator.example.json` vers `legal/operator.json` et renseigner les informations réelles du vendeur et les liens contractuels. Ce fichier est exclu de Git. Vérifier la configuration sans afficher de secrets avec `node --env-file=.env scripts/check-commercial-readiness.mjs --deployment`. Ajouter `--stripe-test` pour contrôler les deux prix dans Stripe en lecture seule ; ce contrôle refuse les clés de production et ne remplace pas un paiement de test avec livraison des webhooks.

Appliquer les migrations avec le compte de migration habituel avant de démarrer le serveur. Les nouvelles tables commencent sans crédit et avec des plafonds à zéro. Ne jamais appliquer les tests à une base client.

1. Valider les droits sur le code, les conditions contractuelles, l'identité du vendeur, la fiscalité et la confidentialité. `ARO_COMMERCIAL_TERMS_APPROVED=true` est une attestation de l'opérateur, pas une validation juridique automatique.
2. Configurer Stripe **en mode test**. Créer deux prix mensuels EUR : Cloud `1200` centimes avec taxe incluse et Business `2900` centimes hors taxe, quantité par utilisateur. Configurer Stripe Tax et les liens publics de conditions/confidentialité dans le tableau de bord. Le checkout exige l'acceptation des conditions et porte la version du catalogue en métadonnées. Archiver la version des conditions associée à chaque version du catalogue. Le serveur vérifie montant, devise, périodicité et base fiscale avant un checkout.
3. Définir `ARO_STRIPE_SECRET_KEY`, `ARO_STRIPE_WEBHOOK_SECRET`, `ARO_STRIPE_PRICE_CLOUD`, `ARO_STRIPE_PRICE_BUSINESS`, `ARO_BILLING_RETURN_URL` (HTTPS) et `ARO_SALES_EMAIL`. Ne jamais fournir ces secrets au frontend.
4. Activer `ARO_BILLING_ENABLED=true` uniquement après ces étapes. Le catalogue reste consultable avant activation ; les boutons de paiement restent désactivés.
5. Configurer le portail Stripe : factures et résiliation ; conserver les prix autorisés. Ne pas autoriser une réduction de quantité au-dessous des membres actifs sans avoir réduit l'équipe. Une réduction reçue de Stripe empêche les nouvelles admissions mais ne supprime pas les membres existants.
6. Pour l'hébergement commercial, activer `ARO_COMMERCIAL_ENFORCEMENT=true`. Les nouvelles invitations nécessitent Business/Enterprise ; l'acceptation vérifie les sièges dans la transaction d'adhésion. Les téléchargements, exports et suppressions doivent rester possibles après expiration. L'auto-hébergement professionnel relève aussi du contrat ; un indicateur logiciel ne rend pas une licence impossible à contourner.

Envoyer à `POST /v1/billing/webhook` les événements `customer.subscription.created`, `customer.subscription.updated`, `customer.subscription.deleted`, `checkout.session.completed`, `checkout.session.async_payment_succeeded`, `charge.refunded`, `charge.dispute.created`, `charge.dispute.closed`. La signature porte sur le corps brut avec une tolérance de 300 secondes. Synchroniser les horloges. Le serveur relit l'objet Stripe actuel sous verrou d'organisation ; le contenu d'un événement ancien n'est pas appliqué aveuglément.

Un abonnement n'est pas activé par la page de retour de paiement. Les droits viennent de l'état Stripe vérifié, de la date de validité et du plan stockés au serveur. Les crédits sont ajoutés une seule fois par intention de paiement. Un remboursement partiel retire la proportion de crédit hors taxe ; un litige bloque l'intégralité du crédit concerné, même déjà consommé, et peut rendre le solde négatif. Une décision favorable après litige nécessite une réconciliation opérateur documentée.

## Modèles hébergés et API de calcul

Configurer un serveur compatible Chat Completions, son éventuel secret et un catalogue de **modèles dont les droits d'hébergement et de commercialisation ont été vérifiés**. Les modèles ne sont pas fournis par cette implémentation.

```dotenv
ARO_COMPUTE_ENABLED=true
ARO_COMPUTE_BASE_URL=https://inference.example.com/v1
ARO_COMPUTE_UPSTREAM_KEY=secret-fourni-par-le-gestionnaire-de-secrets
ARO_COMPUTE_MODELS_JSON=[{"id":"aro-text","name":"ARO Text","upstreamModel":"modele-verifie","inputMicrosPerMillion":500000,"outputMicrosPerMillion":1500000,"maxOutputTokens":4096}]
```

Les tarifs ci-dessus sont des exemples de format, pas des prix recommandés. Mesurer GPU, utilisation effective, mémoire, stockage, réseau, paiement, support et fraude avant de fixer les tarifs. Les taux sont en micro-euros **par million de tokens** : `500000` correspond à 0,50 € / million.

L'administrateur achète du crédit, fixe ses plafonds mensuel et par requête, puis crée une clé dans **Paramètres → Offre & consommation**. La clé `aro_compute_…` n'est montrée qu'une fois ; le serveur conserve son empreinte. Elle ne donne pas accès aux données ou à l'administration du compte. Ajouter un fournisseur OpenAI-compatible dans ARO avec l'adresse `/v1` du serveur et cette clé. Révocation immédiate pour les nouvelles requêtes ; les appels déjà partis sont réglés normalement.

Endpoints : `GET /v1/models`, `POST /v1/chat/completions`. Envoyer un `Idempotency-Key` UUID stable lors d'une reprise de la même requête. Même clé et contenu différent : refus ; requête encore en cours : conflit ; réponse déjà réglée : restitution sans nouvel appel modèle. Le client doit conserver cette clé avant l'envoi. Les clients génériques qui ne l'envoient pas n'ont pas de garantie de reprise sans nouvel appel.

Le gateway prend en charge les messages texte `system/user/assistant`, `temperature`, `max_tokens`, `response_format` text/json_object et `stream`. Le SSE est actuellement envoyé **après génération complète**, pas token par token. Images, audio, outils natifs et JSON Schema ne sont pas pris en charge. Une réservation conservatrice précède l'appel ; le débit réel repose sur l'usage retourné. Aucun usage fiable, délai expiré ou résultat ambigu : conserver la réserve pour vérification, jamais rembourser automatiquement un calcul potentiellement exécuté. La limite actuelle est de huit appels simultanés par processus ; un déploiement multi-réplicas nécessite un dimensionnement global.

## Réconciliation et protection des données

Surveiller les réservations `reserved` trop anciennes et toutes les `uncertain`. Recouper l'identifiant, les journaux du serveur d'inférence et les quantités avant toute régularisation. La méthode interne `billing_settle` effectue la libération ou le règlement sous verrou et refuse un débit supérieur à la réserve. Ne pas éditer seulement le solde SQL : le registre et la réserve deviendraient incohérents.

Un outil hors ligne est fourni. Définir `ARO_BILLING_ADMIN_DATABASE_URL` dans l'environnement sécurisé de l'opérateur, avec un rôle dédié autorisé sur `billing_operator_actions` et les tables de facturation. Le rôle HTTP `aro_app` n'a pas ce privilège. Remplacer les arguments ci-dessous par les valeurs du contrat ou du ticket ; ces commandes écrivent réellement en base.

```text
cargo run -p aro-store --example billing_admin -- pending ORG_UUID
cargo run -p aro-store --example billing_admin -- grant-enterprise ORG_UUID 25 2027-09-21T00:00:00Z CONTRAT-2026-001
cargo run -p aro-store --example billing_admin -- reconcile ORG_UUID RESERVATION_UUID release TICKET-123
cargo run -p aro-store --example billing_admin -- reconcile ORG_UUID RESERVATION_UUID 4500 TICKET-124
cargo run -p aro-store --example billing_admin -- purge-responses ORG_UUID 7
```

Le contrat Enterprise est traçable et idempotent par référence ; il refuse d'écraser un abonnement Stripe encore actif. Les factures Enterprise restent hors de cet outil. La réconciliation exige une référence de preuve et dix minutes sans mise à jour, et conserve cette référence dans la réservation. La purge efface les réponses terminées après la rétention choisie, conserve les traces de régularisation et les identifiants déjà débités : une réponse expirée n'est jamais régénérée avec la même clé. Définir une procédure et une rétention distinctes pour les références de support et les sauvegardes.

Les réponses de calcul sont conservées dans `billing_reservations.result` pour permettre la reprise idempotente. Elles peuvent contenir des données personnelles. Définir contractuellement la durée de reprise, programmer l'outil de purge avec cette durée et organiser les demandes d'effacement avant production. Ne jamais journaliser les clés, prompts, réponses ou objets Stripe complets. Sauvegarder les tables financières et tester la restauration.

## Validation avant lancement

- Exécuter les tests unitaires API/core/frontend et le test PostgreSQL isolé : `ARO_BILLING_TEST_DATABASE_URL=… cargo test -p aro-store --test billing -- --ignored`.
- Tester Stripe en mode test : achat, paiement asynchrone, webhook répété/désordonné, résiliation, remboursement partiel/complet et litige. Un test unitaire de signature ne prouve pas ce parcours.
- Tester un véritable modèle sur l'endpoint configuré : usage fiable, timeout, interruption du client, même clé de reprise, refus de budget et révocation de clé.
- Vérifier isolation avec le rôle SQL d'application non propriétaire, secrets, métriques, support, documents contractuels et purge des réponses.
- Valider les parcours mobile et la politique de distribution des stores avant d'ajouter des achats dans les applications mobiles.

Ne pas activer de paiements réels tant que ces vérifications et les parcours opérateur manquants ne sont pas terminés.
