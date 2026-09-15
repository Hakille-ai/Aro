# Plan de migration backend

Ce plan est additif et réversible. Une migration n'est considérée terminée qu'après métriques, tests de charge, restauration et plan de rollback vérifiés.

## Phase 0 — Containment immédiat

1. Masquer ou répondre explicitement « indisponible » pour les actions MCP, hooks, scheduler, indexation, agent cloud et intégrations qui n'ont pas de worker durable.
2. Garder l'egress agent direct désactivé ; une exécution explicite exige un profil de permission persisté et une allowlist. Les requêtes assistant ne doivent jamais appeler le réseau par implicite.
3. Laisser tous les fichiers non scannés en quarantaine. N'autoriser ni téléchargement, ni pièce jointe, ni indexation avant verdict propre.
4. Réduire les capacités Tauri et supprimer tout appel SaaS direct depuis le renderer. Les secrets sont envoyés une seule fois à un endpoint vault dédié, jamais synchronisés via des collections JSON.

Critère de sortie : tests négatifs prouvant qu'un fichier pending, un outil sans profil, un tenant étranger et un secret renderer ne peuvent atteindre la ressource sensible.

## Phase 1 — Contrats et identité

1. Publier `/v1` et OpenAPI ; générer le client desktop au lieu de maintenir des DTO duplicés.
2. Étendre Problem Details, request-id et l'idempotence déjà actifs sur les résultats locaux à toutes les commandes ; ajouter pagination cursor/ETag et tests de compatibilité.
3. Les familles refresh durables, leur échéance absolue, la rotation sérialisée, la détection de réutilisation, le logout familial et la purge par lignée après rétention sont en place. Le scope tenant est lié au JWT et tout switch exige une rotation. Remplacer maintenant le JWT HMAC mono-clé par un keyring asymétrique/JWKS ; ajouter `jti`, gestion/révocation des sessions par appareil et événements d'alerte de compromission.
4. Ajouter vérification e-mail, reset, MFA et SSO/OIDC avec validation JWKS/nonce. Les invitations à jeton expirant sont en place; raccorder un canal de livraison audité.

## Phase 2 — Autorisation et données

1. Les identités PostgreSQL API/worker/migration sont désormais séparées et vérifiées au démarrage. Ajouter maintenant les variables transactionnelles acteur/tenant.
2. Activer RLS domaine par domaine en commençant par conversations, messages, fichiers, mémoires et agents ; ne jamais utiliser `BYPASSRLS` dans l'application.
3. Ajouter une ACL typée pour le partage explicite et les tests de non-divulgation cross-tenant.
4. Déplacer les mutations + audit + outbox dans les mêmes transactions et introduire des événements versionnés.

## Phase 3 — Jobs et données asynchrones

1. Construire la queue PostgreSQL, leases, heartbeat, annulation, idempotence, backoff, DLQ et console de rejeu.
2. Étendre le worker ClamAV existant avec CDR, puis migrer extraction, indexation, Qdrant, webhooks et scheduler en jobs. Ajouter la réconciliation S3/Qdrant.
3. Introduire S3 multipart présigné borné par taille/MIME/checksum ; le worker vérifie l'objet réel après upload.
4. Migrer les agents vers une machine d'état durable et bornée (temps, tokens, coût, étapes), avec approbations persistées et egress proxy.

## Phase 4 — Intégrations et model gateway

1. Remplacer `ARO_SECRETS_KEY` par chiffrement enveloppe KMS et rotation ; exposer seulement des credential references.
2. Finaliser OAuth/OIDC (PKCE, state, nonce, JWKS, refresh, révocation) et la health-check asynchrone des comptes.
3. Ajouter le model gateway, ses politiques de résidence/rétention, budgets et audit ; mettre les appels cloud dans les workers.
4. Versionner les index vectoriels, imposer le filtre tenant et rendre les projections rejouables.

## Phase 5 — Exploitation et GA

1. Définir SLI/SLO, alertes, runbooks incident et exercices de restauration/chaos.
2. Pinner et signer images/actions, produire SBOM/provenance, scanner secrets/SAST/IaC et appliquer admission policies.
3. Ajouter resources, PDB, HPA, NetworkPolicies, compte de service, seccomp, egress policies et Redis authentifié/TLS.
4. Exécuter les tests E2E desktop/API, compatibilité contractuelle, multi-réplique, charge, failover, migration et restauration avant toute ouverture publique.
