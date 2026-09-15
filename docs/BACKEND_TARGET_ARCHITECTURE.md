# Architecture backend cible

## Décision de produit

ARO adopte un modèle **hybride** : un control plane cloud multi-tenant est la source de vérité pour l'identité, les organisations, les politiques, les métadonnées, les audits et les travaux durables ; les modèles, la voix et les fichiers explicitement locaux s'exécutent à l'edge, sur le poste de l'utilisateur. Cette décision remplace l'ambiguïté entre le produit « local-only » et une plateforme cloud synchronisée.

Un monolithe modulaire est le bon point de départ. Il permet des transactions PostgreSQL atomiques, une exploitation simple et des frontières de domaine testables. Il ne sera découpé en services qu'après des mesures démontrant un besoin d'isolation de déploiement ou de charge. Les workers sont des processus séparés dès le départ.

## Bounded contexts et dépendances

```text
HTTP / gRPC / events
        |
        v
application (cas d'usage, autorisation, idempotence, transactions)
        |
        +-- identity-access      sessions, MFA, organisations, RBAC/ACL
        +-- knowledge            conversations, mémoire, RAG, fichiers
        +-- agent-orchestration  runs, budgets, approbations, annulation
        +-- integrations         catalogues, comptes, credentials, webhooks
        +-- execution            jobs, leases, retries, DLQ, planification
        +-- audit-billing        audit immuable, usage, outbox
        |
        v
ports (repositories, queue, object store, model gateway, vault, egress)
        |
        v
adapters (PostgreSQL/RLS, S3, Redis, Qdrant, KMS, providers IA)
```

`aro-core` devient progressivement le noyau de domaine sans dépendance d'infrastructure. Les interfaces de cas d'usage vivent dans une couche application. `aro-store`, `aro-files`, `aro-vector`, les fournisseurs de runtime et Redis deviennent des adapters ; `apps/api` ne contient plus que le transport, l'assemblage et les politiques HTTP. Aucun runtime desktop concret ne peut être une dépendance de la couche application serveur.

## Invariants non négociables

- Toute commande porte `tenant_id`, `actor_id`, `request_id`, une deadline et, pour toute écriture externe, une idempotency key.
- PostgreSQL est la vérité durable. S3, Qdrant, Redis et les fournisseurs sont des projections ou des dépendances compensables, jamais la seule source d'un état métier.
- L'autorisation est vérifiée dans le cas d'usage et par RLS PostgreSQL. Le rôle d'organisation n'accorde jamais implicitement l'accès aux ressources privées d'un membre.
- Une mutation et son événement outbox sont écrits dans une seule transaction. Les consommateurs sont idempotents et dédupliquent par `event_id`.
- Une action agent ou intégration ne peut sortir du système qu'après politique persistée, budget, approbation si requise et audit. L'egress passe par une passerelle/allowlist contrôlée.
- Les fichiers restent en quarantaine après réception jusqu'au verdict d'un scanner ; seulement ensuite ils deviennent téléchargeables, indexables ou utilisables dans un prompt.
- Secrets et jetons sont des références opaques hors du renderer. Le stockage s'appuie sur chiffrement enveloppe KMS avec version de clé et rotation, jamais sur une clé symétrique de long terme transmise au processus SQL.

## Données et sécurité

PostgreSQL reçoit un rôle propriétaire de migration et un rôle applicatif sans `BYPASSRLS`. Chaque transaction applicative fixe `SET LOCAL aro.actor_id`, `aro.organization_id` et, si utile, `aro.service_role`. Les politiques RLS limitent toutes les tables privées à cet acteur et tenant ; une suite d'intégration prouve les refus cross-tenant. Les contraintes de domaine remplacent les colonnes d'état libres (`CHECK`/enums versionnés, FK composites, indices adaptés aux requêtes).

Les accès paginés utilisent un curseur opaque stable (`created_at`, `id`) plutôt qu'un offset. Les ressources modifiables exposent un `version`/ETag ; les écritures utilisent `If-Match` pour éviter les pertes de mise à jour. `/v1` est désormais le chemin canonique et les routes historiques renvoient un header de dépréciation ; la prochaine étape est de générer ces schémas depuis un contrat OpenAPI versionné et testé pour compatibilité.

## Exécution durable

La table de jobs est la source de vérité : type, payload validé, tenant, priorité, idempotency key, état, tentative, `available_at`, lease token, lease expiry, cancellation requested, dernière erreur et résultat de référence. Les workers réclament avec `FOR UPDATE SKIP LOCKED`, renouvellent leur lease, contrôlent l'annulation et appliquent un backoff exponentiel borné. Après le maximum de tentatives, le job passe en DLQ avec alerte et outil de rejeu contrôlé.

Les projections S3/Qdrant, scans antivirus, extraction, indexation, webhooks, synchronisations, agents et appels modèles sont tous des jobs. Un worker n'effectue jamais un appel réseau dans une transaction métier ouverte. Chaque dépendance externe a un timeout, un budget, une limite de concurrence par tenant et un circuit breaker.

## IA, outils et mémoire

Le model gateway est un port avec un contrat uniforme : capacités, contexte maximal, streaming, coût estimé/réel, confidentialité, région et politique de rétention. Les providers locaux et cloud sont des adapters. Une demande agent reçoit un budget de temps, tokens, coût et étapes ; aucune boucle ne peut être non bornée.

La mémoire/RAG est une projection asynchrone : contenu approuvé et dédoublonné dans PostgreSQL, embedding/index versionné dans Qdrant, filtre tenant obligatoire et réconciliation périodique. Les résultats sont cités avec leur provenance et ne doivent jamais contourner une ACL.

## Opérations

L'API et les workers répondent à SIGTERM avec un drain borné. L'API a une limite de concurrence, de taille et de durée par route, sans timeout appliqué aux streams SSE actifs. Les métriques fournissent débit, erreurs, latence, saturation, jobs, DLQ et dépendances ; traces et logs structurés propagent le `request_id` sans contenu utilisateur ni secret.

Les déploiements utilisent des images digestées et signées, SBOM/attestation, comptes non privilégiés, système de fichiers en lecture seule quand possible, resources/PDB/HPA, NetworkPolicies et sauvegardes PostgreSQL chiffrées testées par restauration. Les secrets proviennent d'un gestionnaire de secrets, pas d'un fichier `.env` de production.
