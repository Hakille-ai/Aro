# ARO — analyse technique et produit

Analyse du code local le 10 septembre 2026. Avis : conserver les technologies principales, concentrer le travail sur la fiabilité des parcours et sur les frontières entre exécution locale, serveur et mobile.

## Périmètre et niveau de preuve

Inspection des manifests, transports, sessions, état mobile, commandes Tauri, runtime IA, worker, stockage, migrations, CI et déploiement. Vérifications exécutées ci-dessous. Aucun changement au code applicatif. Deux reproductions isolées sont conservées dans `output/audit-2026-09-10/`.

Cette analyse ne certifie ni la sécurité globale, ni la production, ni les performances. Pas de test sur téléphone physique, pas de build signé, pas de restauration réelle ni de charge, pas de vérification du schéma d'une base déployée. Les constats SQL concernent les migrations présentes. Le dossier contient un `.git`, mais `git ls-files` ne retourne aucun fichier et HEAD ne résout aucun commit : impossible d'attacher cette observation à une révision immuable. Cela ne prouve pas qu'aucune sauvegarde n'existe ailleurs.

Les notes d'une analyse antérieure ont seulement orienté les vérifications ; les conclusions ci-dessous reposent sur les fichiers actuels. Plusieurs documents décrivent une cible et ne constituent pas la preuve de son intégration.

## 1. Les choix technologiques

| Couche | Choix observé | Décision conseillée | Compromis principal |
| --- | --- | --- | --- |
| Desktop | Tauri 2, Svelte 5, TypeScript | Conserver | Intégration système et Rust adaptés ; différences de WebView à tester |
| Mobile | Expo 57, React Native 0.86.3, React 19.2.3 | Conserver | Bon client mobile ; deuxième système d'interface à maintenir |
| API | Rust, Axum 0.8, Tokio | Conserver, organiser en monolithe modulaire | Code exigeant, compilation et intégration plus coûteuses |
| Données serveur | PostgreSQL, SQLx | Conserver comme autorité des données partagées | Transactions et migrations à traiter rigoureusement |
| Données desktop | SQLite, rusqlite | Conserver avec espaces locaux isolés | Cohérence avec le cloud et cloisonnement des comptes |
| Coordination | Redis | Garder un rôle limité et explicite | Ne doit pas devenir une seconde source de vérité durable |
| Fichiers | API S3, scan ClamAV | Conserver selon besoins réels | Coût RAM, files de scan, reprise et configuration réseau |
| Recherche mémoire | Qdrant, embeddings Ollama, recherche lexicale | Garder optionnelle ; décider par mesures | Service et index supplémentaires à exploiter |
| Contrats clients | packages TS partagés et types desktop | Unifier réellement | La génération annoncée n'est pas démontrée |

### Desktop

Tauri est cohérent avec un outil qui manipule des projets locaux et utilise des moteurs Rust. Svelte convient à l'interface. Je ne financerais pas une migration vers Electron ou React pour résoudre les problèmes observés : ils viennent surtout du découpage et de la sémantique des opérations.

Tauri utilise les WebViews des plateformes ; il faut donc valider les moteurs de rendu réels, les dialogues, le clavier, la voix, les chemins et les permissions sur chaque OS. Cela ne permet pas d'affirmer qu'ARO consomme peu de mémoire sans mesure : les modèles locaux peuvent dominer le coût. [Architecture Tauri](https://v2.tauri.app/concept/architecture/), [modèle de processus](https://v2.tauri.app/concept/process-model/).

### Mobile

Expo/React Native est pertinent pour consulter les résultats, discuter, importer un document, approuver une action et recevoir une notification. Je privilégierais ce rôle de compagnon. Reproduire immédiatement l'ensemble du bureau et son moteur local multiplie les difficultés sans bénéfice utilisateur démontré.

Deux interfaces Svelte/React ne sont pas une erreur : partager les contrats, les règles métier pures et les tokens visuels suffit souvent. Le transport Tauri doit garder sa frontière native et ses secrets. Éviter une abstraction universelle qui obligerait les composants mobiles à imiter le desktop.

Le README mobile annonce encore Expo 52/Router 4 alors que le manifest est passé à Expo 57 : rétablir une configuration reproductible et documentée avant toute nouvelle montée de versions. Les échecs TypeScript/Vitest observés ne prouvent pas un défaut intrinsèque d'Expo.

### Backend

Rust/Axum permet de réutiliser le domaine et les adaptateurs, avec des contrats explicites. C'est un bon investissement si l'équipe peut maintenir ce code. Changer de langage ne corrigerait ni une annulation ignorée, ni un mauvais calcul d'usage, ni un conflit de synchronisation.

Rester sur un monolithe modulaire avec API et workers déployables séparément. Ne pas créer des microservices avant qu'un besoin d'isolation, de déploiement ou de charge soit mesuré. PostgreSQL peut porter l'état durable et les files avec baux ; Redis sert la coordination et la diffusion. Le code contient déjà des briques de baux, d'idempotence et de transactions à exploiter.

## 2. Forces réelles

- Un moteur local et des adaptateurs de modèles existent réellement, au-delà d'une simple interface de chat.
- Rust est partagé entre plusieurs couches, avec des crates dédiées aux outils, secrets, intégrations, fichiers, mémoire et politiques.
- Le desktop conserve les credentials de session dans le trousseau OS ; le mobile utilise SecureStore. La rotation des refresh tokens est explicitement traitée, notamment par sérialisation sur desktop et single-flight dans le client TS.
- Le backend prévoit des identités PostgreSQL séparées pour API, worker et migrations, des contrôles de configuration, l'audit, l'outbox et les migrations.
- Le Compose de production impose des images référencées par digest, des limites de ressources, des systèmes de fichiers applicatifs en lecture seule et des environnements distincts.
- La recherche vectorielle possède un mode dégradé : selon sa configuration, une panne vectorielle ne bloque pas nécessairement la restitution lexicale.
- `aro-agent-domain` expose des invariants pertinents : versions optimistes, états terminaux, approbations liées au digest exact de l'action, effets externes inconnus à réconcilier. Ses tests passent.
- Il existe une base de tests exploitable. L'amélioration doit porter sur les scénarios, pas seulement sur leur nombre.

## 3. Problèmes concrets, par priorité

### P0 — Les patchs peuvent écraser un travail récent

Preuve : `apps/desktop/src-tauri/src/main.rs`, `apply_unified_patch` et `workspace_diff_apply`. Les suppressions retirent la ligne à sa position sans vérifier son contenu ; les lignes de contexte ne sont pas comparées. La commande ne reçoit pas l'empreinte du fichier sur lequel la proposition a été construite et écrit directement le résultat.

Reproduction avec la fonction réelle extraite : contenu actuel `user-new-value`, patch supposant `old-value`, résultat accepté `ai-value`. Aucun fichier utilisateur n'a été modifié par le test.

Correction : transporter l'empreinte de base, refuser le conflit, vérifier strictement le patch, préparer une écriture atomique et conserver une restauration. Pour plusieurs fichiers, définir aussi le comportement en cas d'échec partiel. Critère : une proposition ancienne ne doit jamais remplacer silencieusement une édition récente.

### P0 — Cloisonnement local des comptes à finir

Desktop : `state.rs` initialise un même `aro.sqlite3`, indépendamment de l'utilisateur et de l'organisation. Le schéma local de `aro-memory` ne possède pas les dimensions d'identité du serveur pour ces données. Le chargement de messages cloud les recopie localement ; un fallback local existe. Une déconnexion ne recrée pas ce magasin.

Mobile : `useChat` conserve conversations et messages ; `logout` et `switchOrganization` ne le réinitialisent pas. Le QueryClient survit à la navigation et les clés `['files']`, `['agent-runs']`, `['settings']` ne contiennent ni compte ni organisation. Une transition peut donc réutiliser un état précédent. Ce n'est pas une démonstration de contournement des droits serveur ; le problème constaté est l'état conservé côté client.

Correction : namespace explicite par serveur/utilisateur/organisation, invalidation de génération des requêtes, annulation des opérations en vol et purge des vues lors du changement d'identité. Sur desktop, choisir une base par identité ou un schéma intégralement partitionné. Tester A → déconnexion → B, et A/org1 → A/org2, y compris hors ligne.

### P0 — La preuve RLS annoncée par la CI n'existe pas dans le test

La migration `202607020011_private_data_rls_policies.sql` réserve expressément l'activation de la RLS des conversations/messages/mémoires à une phase ultérieure. D'autres migrations activent bien la RLS sur les tables d'intégrations et de gouvernance : il serait faux de dire que toute la base est dépourvue de RLS.

En revanche, `crates/aro-store/tests/rls.rs` contient seulement un test qui ouvre une connexion, ou retourne un succès si DATABASE_URL est absent. La CI intitule son étape comme une preuve sous rôle non propriétaire, sans que ce fichier utilise `ARO_RLS_TEST_DATABASE_URL` ni exerce un accès inter-tenant.

Correction : activer les tables concernées après migration des chemins de requête et ajouter de vrais tests négatifs, avec deux utilisateurs de la même organisation et deux organisations, sous le rôle runtime. Exiger une base configurée dans le job dédié. Une politique déclarée ne suffit pas : [documentation PostgreSQL](https://www.postgresql.org/docs/current/ddl-rowsecurity.html).

### P1 — Streaming serveur différé et parseur client fragile

Dans `assistant_stream`, le serveur attend `generate_server_assistant_text`, persiste la réponse, puis découpe le texte par espaces avant de créer le flux SSE. L'utilisateur attend donc la génération complète avant les premiers événements. Le timeout HTTP ordinaire enveloppe aussi ce handler : une génération longue peut échouer avant l'établissement du flux.

Dans `packages/api-client/src/stream.ts`, `currentEvent` est réinitialisé à chaque lecture réseau. Si `event:` et `data:` arrivent dans deux morceaux distincts, l'événement est perdu. La reproduction isolée exécute le parseur du projet : flux groupé réussi, même flux fragmenté en échec avec `Stream fermé sans événement done`.

Le helper `fetchWithTimeout` enlève par ailleurs le listener d'annulation dès le retour des en-têtes. Le lecteur vérifie le signal après `reader.read()` ; si la lecture reste suspendue, l'arrêt peut tarder. L'annulation de l'UI, du transport et de la génération doivent être reliées explicitement.

Correction : flux provider → serveur → client réellement incrémental, parseur conservant son état jusqu'à la fin de l'événement, délais distincts de connexion/inactivité/durée totale, et message final durable. Valider fragmentation, Unicode, déconnexion et stop pendant une lecture. Expo fournit un fetch compatible streaming ; le défaut reproduit appartient ici au parseur ARO. [Documentation Expo](https://docs.expo.dev/versions/latest/sdk/expo/).

### P1 — Le worker cloud n'est pas équivalent au moteur desktop

`apps/api/src/agent_runner.rs` configure un modèle, effectue une génération avec historique vide et termine un job avec zéro appel d'outil. Le desktop possède une vraie boucle modèle/outils. Les deux ne rendent donc pas le même service.

Le worker lance des tâches avec `tokio::spawn` en vidant la file sans plafond de concurrence visible dans ce dispatcher. La boucle de renouvellement voit pause/annulation, mais n'interrompt pas directement `router.generate`. La persistance peut refuser une complétion périmée ; cela ne stoppe pas l'appel au modèle déjà en cours.

Le runner transmet la même estimation à `input_tokens` et `output_tokens`, et des usages nuls lors du renouvellement. Des contrôles de budget existent dans le store ; leur présence ne corrige pas cette comptabilité. Le runtime local ignore explicitement `requested_max_steps`, avec des tests qui valident ce choix de boucle sans limite.

Correction : concurrence bornée et équité par tenant, annulation propagée, limites de durée/tokens/coût, comptage fournisseur séparé entrée/sortie, et reprise depuis un checkpoint utile. Une limite configurable peut rester généreuse : elle protège contre les répétitions accidentelles. Ne pas vendre une sortie de texte comme une tâche externe effectivement réalisée.

### P1 — Le lieu d'exécution et les clés ne sont pas unifiés

`generate_server_assistant_text` ne peut pas utiliser une clé restée dans le trousseau du desktop. Il privilégie des modèles locaux au serveur, puis cherche un modèle Ollama. Un choix explicite non exploitable peut retomber sur la résolution par défaut.

Un endpoint localhost désigne la machine du processus qui l'utilise. Le mobile ne peut pas retrouver automatiquement le moteur du PC à travers le serveur. Il faut distinguer : exécution sur ce PC, sur une machine jumelée, sur le serveur, ou chez un fournisseur.

Correction : figer dans chaque run un environnement, un modèle et une politique de repli. Afficher la disponibilité avant lancement, rendre les replis consentis et traçables. Un agent desktop jumelé serait une fonctionnalité nouvelle à concevoir, pas une propriété déjà acquise du mobile.

### P1 — Le mobile n'est pas encore validé comme produit distribué

Le typecheck échoue sur `baseUrl` avec TypeScript 6 ; Vitest échoue en important la syntaxe Flow de React Native. La CI inspectée ne lance pas les commandes de contrôle mobile ou du package API client. Les tests de composants et E2E desktop ne sont pas non plus exécutés dans le job frontend montré.

Le cache Query est en mémoire ; aucune persistance des conversations ni reprise des mutations hors ligne n'est visible. Un échec de restauration réseau aboutit à une session UI nulle ; le refresh efface la session sur échec ambigu. La prudence sur le replay du token est compréhensible, mais nécessite une expérience hors ligne séparée.

`usePush` s'exécute une fois au montage racine, sans dépendre de la connexion ; un enregistrement raté avant authentification est avalé et n'est pas automatiquement retenté au login. Les capabilities de secours annoncent plusieurs fonctions disponibles même quand leur lecture a échoué.

Enfin, le défaut d'URL est localhost et Android autorise le trafic HTTP en configuration commune. Cela ne prouve pas qu'une release utilise HTTP, mais le profil production devrait imposer un endpoint HTTPS connu. Tester un vrai build Android/iOS, le retour d'arrière-plan et les notifications après login.

### P1 — Une frontière hors ligne à définir explicitement

SQLite et l'inférence locale sont utiles, mais ne garantissent pas un parcours indépendant du cloud. Certaines commandes tentent le refresh avant leur fallback local et propagent son erreur. Un produit peut être cloud-synchronisé tout en préservant l'accès aux résultats déjà présents.

Écrire un contrat par opération : lire localement, éditer localement, attendre une synchronisation, ou exiger le serveur. Ajouter une outbox locale durable pour les écritures acceptées hors ligne, avec idempotence, version et gestion des conflits. Ne jamais laisser croire que sauvegarde locale et sauvegarde cloud sont équivalentes.

### P2 — Trop de responsabilités dans quelques fichiers

Mesure indicative de lignes non vides au moment de la lecture : environ 12 730 dans `aro-store/src/lib.rs`, 9 741 dans `App.svelte`, 6 393 dans `handlers.rs`, 4 086 dans le `main.rs` Tauri. Certaines incluent des tests ; la taille seule ne prouve pas un défaut, mais les responsabilités observées y sont réellement nombreuses.

Découper par parcours : session, conversations, exécution, fichiers, synchronisation et settings. Les handlers traduisent HTTP ; les services orchestrent ; les repositories portent SQL ; les composants composent la vue. Garder des tests de comportement avant extraction, sans réécriture générale.

`packages/contracts` se dit généré mais décrit une copie des types desktop ; aucune chaîne de génération n'a été trouvée dans les scripts inspectés. Introduire une source de schéma autoritative, génération vérifiée en CI et tests de compatibilité entre versions. Les casts TS ne valident pas les réponses à l'exécution.

### P2 — Exploitation et livraison encore incomplètes

Le workflow release observé publie l'image API ; il ne produit pas d'installateurs desktop signés. Ajouter installation, signature, mise à jour et rollback par OS. [Distribution Tauri](https://v2.tauri.app/distribute/).

Le réseau Compose `aro-internal` est `internal: true` et API/worker/ClamAV n'ont que ce réseau. C'est une isolation forte, mais une configuration sortante explicite manque dans ce fichier pour SMTP, stockage S3 externe, fournisseurs ou mises à jour antivirus. C'est une incompatibilité de configuration à tester, pas une panne constatée sur un serveur déployé. Prévoir un egress contrôlé. [Réseaux Docker Compose](https://docs.docker.com/reference/compose-file/networks/#internal).

Les services de données sont mono-instance dans cette configuration ; les manifests Helm ne prouvent pas une haute disponibilité. Démarrer avec un déploiement simple, des sauvegardes restaurées en test, des alertes et des capacités mesurées. Les plafonds mémoire Compose ne sont pas des mesures de consommation.

## 4. Direction produit et architecture cible

La proposition la plus défendable est un espace de travail où l'IA produit des résultats vérifiables : demande → contexte → exécution → résultat → relecture → application → reprise. La voix facilite l'accès ; la confiance dans les fichiers, l'historique et la reprise fait la valeur durable.

Le desktop est le lieu du travail sur les fichiers locaux. Le mobile sert à capturer une demande, suivre une tâche, consulter et approuver. Le serveur synchronise, conserve les autorisations et orchestre les exécuteurs autorisés. Chaque interface peut rester spécifique à sa plateforme.

Le domaine pur `aro-agent-domain` va dans une bonne direction, mais il n'est pas consommé par les chemins utilisateurs inspectés. Le brancher progressivement sur une tranche verticale sous feature flag ; éviter de maintenir indéfiniment deux moteurs de transitions indépendants.

Le succès d'une tâche doit dépendre de son livrable : fichier présent, commande vérifiée, diff valide ou réponse explicitement produite. Une génération terminée n'est pas nécessairement un objectif atteint. Séparer état d'exécution, preuve du résultat et validation utilisateur.

La mémoire doit également être évaluée : récupération utile, source, date, portée d'accès, correction et suppression propagée. Comparer recherche lexicale seule et ajout vectoriel sur un jeu de demandes ; décider du maintien de Qdrant par gain mesuré, pas par préférence de stack. Mesurer aussi l'assemblage du contexte en tokens, pas uniquement en nombre de messages.

Sur les coûts, suivre par tâche les tokens d'entrée/sortie, les outils, les reprises et l'environnement. Le code annonce `billing: false` ; une facturation exploitable n'est pas prouvée ici. Facturer la continuité du travail, la synchronisation et les workflows est une piste produit ; les prix demandent une validation séparée. Conserver l'accès aux résultats lorsque l'inférence payante est indisponible.

## 5. Ordre d'exécution recommandé

| Lot | Travail | Preuve de sortie |
| --- | --- | --- |
| 1 — intégrité | Snapshot Git propre après exclusion des secrets, patchs avec conflit, isolation des états | Édition concurrente préservée ; changement de compte sans données précédentes |
| 2 — parcours mobile | TypeScript/Vitest, SSE, arrêt, refresh, endpoint de release, CI mobile | Conversation réelle sur Android/iOS, réseau coupé puis rétabli |
| 3 — backend fiable | RLS réellement exercée, worker borné, annulation et usage | Tests négatifs tenant ; double claim ; kill/recovery ; budget atteint |
| 4 — continuité | Exécution/localité explicites, outbox locale, lecture hors ligne | Même tâche et artefacts retrouvés après redémarrage et reconnexion |
| 5 — livraison | Installateurs signés, egress, restauration, métriques, alertes | Installation et mise à jour sur machine vierge ; restauration chronométrée |
| 6 — développement produit | Intégrations prioritaires, voix et recherche améliorées | Gains mesurés sur des tâches utiles, adoption et taux de réussite |

Ces lots sont une séquence de décision, pas un devis de durée. Le nombre de développeurs, l'état du déploiement et les essais natifs restent inconnus.

Mesures proposées : taux de tâches réellement réussies, délai jusqu'au premier contenu, taux de conflits correctement bloqués, réussite de reprise, coût au percentile 95, fraîcheur de synchronisation, crashs par session et utilité de la mémoire. Fixer les objectifs après une baseline.

## 6. Vérifications exécutées

| Commande ou contrôle | Résultat |
| --- | --- |
| `npm run check` | 0 erreur, 0 avertissement Svelte |
| `npm run test:unit` | 207 tests, 22 fichiers, réussis |
| `npm run mobile:check` | Échec TS5101, `baseUrl` déprécié |
| `npm run mobile:test` | Échec de collecte React Native `import typeof`, aucun test exécuté |
| `npm run api-client:check` | Réussi |
| `npm run api-client:test` | 16 tests, 3 fichiers, réussis |
| `npm run contracts:check` | Réussi |
| `cargo test --locked -p aro-agent-domain -p aro-runtime -p aro-agent` | 39 tests réussis : 12 + 15 + 12 |
| `node output/audit-2026-09-10/probe-stream.cjs` | Perte d'événements reproduite sur flux fragmenté |
| Fonction de patch extraite, compilée avec rustc | Écrasement d'une valeur récente reproduit |

Les tests ciblés n'incluent pas toute l'API ni les intégrations PostgreSQL/S3, les composants, le navigateur E2E, les appareils natifs ou les performances. Ils donnent des preuves circonscrites, pas un feu vert de lancement.
