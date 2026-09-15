# Audit architecture et production — 2026-07

## Verdict

Le dépôt est une base d'ingénierie prometteuse, pas encore un backend production-ready ou une plateforme AGI. Le socle Rust est cohérent et plusieurs protections sont déjà meilleures qu'un prototype habituel : migrations SQLx, contrôles tenant/owner dans les repositories, Argon2, refresh tokens hashés, CORS explicite, outbox avec leases, S3 obligatoire en production et validation SSRF sérieuse pour les fetchs Web.

Mais l'exposition publique doit rester bloquée. Les frontières sont encore largement applicatives, l'API est monolithique, les traitements longs ne sont pas durables et des capacités d'intégration/agent/fichier n'ont pas les garanties nécessaires. La décision de référence est donc l'architecture décrite dans [BACKEND_TARGET_ARCHITECTURE.md](BACKEND_TARGET_ARCHITECTURE.md), déployée par étapes dans [BACKEND_MIGRATION_PLAN.md](BACKEND_MIGRATION_PLAN.md).

## Cartographie actuelle

| Zone | État observé | Appréciation |
| --- | --- | --- |
| `aro-core` | DTO et contrats sérialisables partagés | Base utile, mais pas encore un domaine isolé |
| `aro-store` | PostgreSQL, migrations, auth, collections, fichiers, agents, outbox dans ~7 400 lignes | Trop large ; repository/fonctions de transaction à découper |
| `apps/api` | Axum, auth, Redis, routes et orchestration dans ~4 800 lignes | Monolithe HTTP à extraire en cas d'usage/domaines |
| `aro-agent` / `aro-runtime` | Contexte, modèles et outils ; boucle agent locale bornée | Pas encore de budget tokens/coût, worker durable, lease ou annulation |
| `aro-files` / `aro-vector` | abstraction S3/local et Qdrant/Ollama | Projections asynchrones incomplètes et chargements mémoire complets |
| desktop | Tauri/Svelte avec runtime local | UI très concentrée ; chemins secrets/SaaS directs désormais supprimés et capacités Tauri réduites |
| déploiement | Docker, Compose, Helm, CI, Trivy | Helm durci ; supply chain, services HA et egress cluster restent insuffisants |

La documentation produit est contradictoire : [PRODUCT_SPEC.md](PRODUCT_SPEC.md) annonce un produit local/offline sans comptes cloud, alors que [README.md](../README.md) et l'API décrivent une synchronisation cloud multi-tenant. L'architecture hybride cible tranche cette divergence ; les données cloud et les traitements edge doivent être distingués par contrat et consentement.

## Risques bloquants

### Autorisation et identité

- L'isolation est exprimée dans les requêtes `organization_id`/`owner_user_id`, sans RLS PostgreSQL. Une seule requête future incomplète peut exposer les tenants. Les FK composites sont un bon complément, pas une frontière de sécurité suffisante.
- Authentification incomplète : pas de reset, MFA/SSO, révocation immédiate d'access tokens ni console de sessions par appareil. Le scope tenant est signé dans le JWT ; un switch exige le refresh courant et le fait tourner. Les familles refresh sérialisent rotation/logout, ont une échéance absolue non prolongeable, révoquent tous leurs membres lors de la réutilisation d'un prédécesseur et sont purgées par lignée après rétention. L'acceptation d'invitation est à jeton unique et expirant, mais nécessite encore un canal de livraison audité. Le `kid` JWT est émis sans rotation asymétrique/JWKS complète.
- Les clés API sont créées mais ne sont pas un mécanisme d'authentification du transport.

### Agents, outils et egress

- Les runs agents n'ont ni queue durable, ni lease, ni budget de coût/tokens, ni reprise, DLQ ou annulation fiable. La boucle applique désormais huit étapes par défaut et un plafond absolu de 32, avec échec persisté à la limite.
- La politique d'outil n'était pas réellement imposée lors d'un appel direct. L'API est désormais deny-by-default : l'egress direct exige `ARO_AGENT_DIRECT_TOOL_EXECUTION=true`, l'activation Web et un profil de permissions avec allowlist. Les requêtes assistant restent sans egress implicite.
- Fetch, recherche et SearX valident/revalident DNS et redirects, pinent les adresses, bornent les corps, refusent les IP non globales et ignorent les proxies ambiants. Un egress proxy explicitement approuvé avec audit reste requis pour le contrôle centralisé GA.

### Fichiers et données

- La réception HTTP et les adapters de fichiers chargent les objets en mémoire ; la cible est un upload S3 multipart/presigné et du streaming borné.
- Les objets restent pending/quarantined jusqu'au verdict ClamAV du worker avec leases et retries, ce qui est désormais une barrière réelle. Il reste à ajouter CDR, extraction/indexation durables, quotas, rétention et tests multi-réplique.
- Qdrant est une projection qui doit imposer un filtre tenant, TLS/auth, version d'index et réconciliation. Les suppressions et reindex ne satisfont pas encore ces garanties.

### Secrets et intégrations

- `ARO_SECRET_PROVIDER` n'est pas réellement utilisé pour un KMS et la clé symétrique actuelle ne fournit pas une rotation enveloppe complète.
- Les flux desktop ne valident plus les secrets directement auprès des SaaS : les blocs directs ont été supprimés physiquement, les payloads plugin/MCP/hook sont nettoyés à l'écriture et à l'hydratation, et les simulations d'intégration sont fail-closed. KMS et OAuth validé de bout en bout restent des prérequis GA.

### Exploitation

- L'API a maintenant un arrêt propre et une limite de concurrence HTTP ; il manque toujours des deadlines par route, les métriques RED/USE, traces distribuées, SLO et alertes.
- Helm impose désormais secrets externes par défaut, ressources, probes, PDB/HPA, NetworkPolicies, contextes de sécurité, PVC ClamAV et support des digests. Les services embarqués restent mono-instance, Redis n'a ni TLS/auth ni persistance, l'egress cluster n'est pas restreint et Compose/CI doivent encore généraliser SBOM, signatures et provenance.
- Les tests actuels couvrent correctement les unités et un smoke PostgreSQL, mais pas RLS, compatibilité, multi-réplique, pannes S3/Redis/DB, charge, restauration ni E2E Tauri.

## Changements réalisés avec cet audit

- L'API et le worker réagissent désormais proprement à Ctrl+C/SIGTERM, ce qui permet le drain orchestré.
- Une limite de concurrence (`ARO_HTTP_MAX_CONCURRENCY`, 256 par défaut) et une deadline de requête (`ARO_HTTP_REQUEST_TIMEOUT_SECONDS`, 30 s par défaut) protègent les ressources du processus.
- Le rate limit non authentifié utilise l'adresse TCP du pair lorsque les headers proxy ne sont pas approuvés, au lieu de placer tous les clients dans le seau `ip:unknown`.
- Les erreurs HTTP suivent Problem Details avec un code stable tout en conservant le champ `error` pour les clients existants.
- Les appels directs d'outils sont désactivés par défaut et les enrichissements Web automatiques sont supprimés du chemin assistant. Toute réactivation reste limitée par un profil persisted et une allowlist.
- `/v1` est le contrat canonique, les clients desktop/web l'utilisent, et les routes historiques exposent leur dépréciation.
- La persistance des résultats locaux est idempotente : hash de requête, lease d'exécution, replay exact et conflit sur réutilisation incohérente de clé.
- Les refresh tokens tournent dans une famille durable et sérialisée ; la réutilisation d'un ancien token révoque atomiquement la famille, et le logout familial ne peut pas laisser survivre un successeur concurrent. Tauri garde le refresh durable dans le keyring et les clients web coalescent les rafraîchissements concurrents en mémoire.
- Le renderer ne contient plus de fetch SaaS ni de succès MCP/plugin simulé, et son rôle organisationnel affiché provient de la membership réelle.
- Le chart Helm est fail-closed sur les secrets, sépare les variables par workload et fournit les contrôles de base Kubernetes ; les limites HA sont explicitement documentées.

Ces changements sont des contrôles de confinement. Ils ne transforment pas les composants listés ci-dessus en capacités GA ; le plan de migration reste obligatoire.
