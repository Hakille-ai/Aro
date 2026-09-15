# ARO : décision produit, marque et modèle économique

Analyse du dépôt présent le 14 septembre 2026 et consultation de sources officielles concurrentes. Ce document propose une direction ; il ne décrit pas une offre déjà commercialisée. Aucun changement de licence, de marque ou de facturation n'a été effectué.

## Orientation actualisée après échange : plateforme interne pour entreprises

L'utilisateur a écarté les trois noms proposés et précisé une autre hypothèse : les entreprises installent la plateforme chez elles, connectent leurs données, configurent les modèles et les équipes, et paient une licence. Les noms ci-dessous ne sont donc plus des propositions retenues. La stratégie destinée aux indépendants, conservée plus bas pour référence, n'est plus la recommandation prioritaire.

**Nouvelle priorité proposée : une plateforme IA auto-hébergée pour entreprises, avec une édition Community et une édition Enterprise commerciale.** Cette direction utilise mieux l'investissement existant dans le backend, les organisations, la gouvernance et le déploiement. Sa demande commerciale reste à valider.

Le dépôt comprend un chemin d'auto-hébergement (`docs/SELF_HOST.md`, `deploy/compose.prod.yml`, `deploy/helm/aro`), la gestion d'organisations/équipes et des fournisseurs configurables. Le moteur de politiques contient des restrictions de modèles et fournisseurs (`crates/aro-policy/src/lib.rs:274`). En revanche, les réglages applicatifs sont actuellement stockés par utilisateur et organisation (`crates/aro-store/src/lib.rs:4203`) : cela ne démontre pas une console d'administration imposant une politique de modèles à chaque équipe sur tous les chemins d'exécution. Ce parcours reste à vérifier et compléter.

Connecter une base métier n'est pas simplement utiliser le PostgreSQL interne d'ARO. Il faut un connecteur ou outil explicitement autorisé, idéalement en lecture seule pour le premier pilote, qui conserve les droits de la source jusque dans les recherches, extraits, caches et réponses. L'existence de MCP ou d'un catalogue ne démontre pas ces propriétés.

Le contrôle des données exige aussi le contrôle des sorties : une installation interne utilisant un modèle externe peut lui transmettre du contenu. Une offre entièrement interne doit vérifier les modèles, embeddings, voix, outils, journaux et flux réseau. La conservation de l'historique chez le client ne suffit pas à elle seule.

### Ce qui peut être vendu

- Community : un socle réellement utilisable, auto-hébergeable et extensible sous licence open source.
- Enterprise : modules commerciaux d'administration centralisée, politiques par équipe, identité d'entreprise, exploitation, audit exportable, sauvegarde/restauration, mises à jour supportées et assistance contractuelle. La sécurité fondamentale reste commune ; ces capacités ne sont pas toutes démontrées comme livrées aujourd'hui.
- Intégration : prestation initiale bornée pour installation et connexion d'une première source métier.
- OEM/intégrateurs : contrat distinct lorsqu'un éditeur veut redistribuer ou incorporer le logiciel dans une offre propriétaire. Cet usage diffère d'une personnalisation interne.

La licence Enterprise peut être annuelle, avec une capacité d'organisation définie et un volume d'assistance explicite. Le client finance son infrastructure et ses modèles ; le revenu ne dépend pas d'une commission sur les tokens. Aucun tarif B2B n'est encore validé par des clients ni par le coût du support.

### Distinction essentielle sur les licences

Une vraie licence open source autorise les usages commerciaux : une entreprise respectant ses conditions n'a pas automatiquement à acheter une seconde licence parce qu'elle utilise le logiciel en interne. Voir la [définition OSI](https://opensource.org/osd).

Une double licence AGPL/commerciale peut proposer des conditions différentes aux clients qui en ont besoin, sous réserve de détenir les droits nécessaires sur le code. L'AGPL n'est pas une taxe sur les entreprises ; sa section 13 porte sur l'accès au code correspondant d'une version modifiée pour ses utilisateurs réseau. Elle n'impose pas par elle-même de publier les bases de données clients. Voir [AGPL, section 13](https://opensource.org/license/agpl-3.0) et [FSF sur la vente d'exceptions](https://www.gnu.org/philosophy/selling-exceptions.en.html).

Si l'objectif est d'imposer un paiement pour tout usage interne professionnel du produit concerné, il faut envisager une licence commerciale à code accessible, avec les conditions adaptées ; il ne faut pas la présenter comme une licence open source. La rédaction définitive et les droits sur les contributions/dépendances devront être examinés avant publication.

### Première expérience commerciale proposée

Chercher deux ou trois entreprises disposant d'un responsable informatique et d'une équipe pilote de 10–30 personnes. Choisir une seule source de documents ou base métier et un seul cas d'usage. Vérifier l'installation chez le client, le respect des permissions, le choix imposé des modèles et un résultat utilisé réellement. Vendre un pilote à périmètre fixe puis un abonnement Enterprise si le bénéfice et les coûts d'accompagnement le justifient.

Les écarts techniques constatés plus bas restent valables, notamment moteur distant, isolation, comptage et distribution. L'auto-hébergement déplace les responsabilités d'exploitation ; il ne supprime pas ces exigences.

## Analyse initiale conservée pour référence

## Décision proposée

Construire un espace de travail IA pour les indépendants qui produisent des dossiers et des livrables clients. Commencer avec une bêta desktop accompagnée, une offre payante simple à tester et une seule plateforme de distribution. Préparer un socle local ouvert et des services hébergés payants, mais garder le dépôt privé pendant la clarification du périmètre et la validation initiale.

Première piste de marque : **Ormevo**, prononcé « or-mé-vo ». C'est une proposition créative cohérente avec l'arbre du logo, pas un nom dont les droits, domaines et performances vocales sont validés.

Promesse à éprouver : **« Votre espace de travail IA, du dossier au livrable. »**

La décision structurante est le résultat à vendre. Le nom et l'ouverture du code doivent soutenir ce choix.

## Ce que le produit contient réellement

| Élément | Preuve dans le dépôt | Implication produit |
|---|---|---|
| Desktop Tauri/Svelte et moteur Rust | `apps/desktop/src-tauri/src/main.rs`, `crates/aro-runtime/src/lib.rs` | Le poste de travail est une surface principale pertinente. |
| Projets avec dossier de travail et instructions | `apps/desktop/src/features/projects/CreateProjectModal.svelte:19`, `:124`, `:132` | Organiser un dossier client avec son contexte est cohérent avec le produit. |
| Boucle modèle/outils locale | `crates/aro-runtime/src/lib.rs:901`, `:989`, `:1118` | Le produit possède de vrais chemins d'action et de retour d'outils. Leur qualité finale reste à éprouver sur des tâches réelles. |
| Lecture, recherche, écriture, commandes et documents | `crates/aro-tools/src/lib.rs:202`, `:346`, `:398`, `:739`, `:1183` | Un résultat peut être un fichier Word, Excel, CSV ou autre livrable, au-delà du texte de conversation. |
| Mémoire, plans, outils MCP et connecteurs | `crates/aro-runtime/src/lib.rs:688`, `:1656`, `:1856`, `:2435`, `:2624` | Une continuité de travail est envisageable. Un catalogue de connecteurs n'est pas une preuve de disponibilité de chaque intégration. |
| Fournisseurs locaux et distants | `crates/aro-core/src/settings.rs:6`, configuration et routage desktop | Le choix du moteur peut servir la confidentialité, le coût et la qualité. Ce n'est pas une différenciation suffisante seul. |
| Identité et synchronisation serveur | `apps/desktop/src-tauri/src/main.rs:193`, `apps/api/src/handlers.rs`, `crates/aro-store` | L'architecture actuelle est hybride. « Tout reste sur votre ordinateur » serait une promesse incorrecte. |
| Mobile Flutter/Dart | `apps/mobile/lib/main.dart`, `apps/mobile/lib/core`, `apps/mobile/lib/features` | Le checkout actuel a remplacé l'ancienne architecture mobile évoquée dans des analyses précédentes. |

Les logos `apps/desktop/public/logo.png` et `aro-core-logo.png` ont été examinés. Ils représentent un arbre à feuilles colorées et ramifications de circuit. Les captures déjà enregistrées dans `output/playwright/desktop-reference.png` et `mobile-refined-home.png` ont aussi été examinées ; ce sont des captures existantes, pas une session interactive testée pendant cet audit.

La documentation produit historique est incohérente avec le parcours actuel : `docs/PRODUCT_SPEC.md` décrit encore un assistant sans compte cloud et entièrement local, alors que `App.svelte:9459` affiche la connexion sans authentification. Il faut réécrire cette promesse avant de préparer le site commercial.

## Les écarts qui changent les décisions commerciales

### 1. Le gratuit local autonome reste à construire

`App.svelte:7706` contrôle l'autorisation d'écriture cloud avant un envoi ; l'écran principal dépend de l'authentification. La présence de SQLite et de modèles locaux ne suffit pas à fournir un parcours sans compte ni serveur.

Il faut permettre un espace personnel utilisable sans abonnement, conserver les résultats locaux et rendre l'export simple. La synchronisation devient alors un service facultatif. L'auto-hébergement du backend est une autre option, plus technique ; il ne remplace pas cette expérience sans compte.

### 2. Le moteur cloud n'est pas équivalent au moteur desktop

Dans `apps/api/src/agent_runner.rs:260`, une action Tool est enregistrée et ajoutée à l'historique ; elle n'est pas envoyée à un exécuteur dans cette branche. Le moteur local exécute les outils et réintroduit leurs résultats (`crates/aro-runtime/src/lib.rs:989`).

Il ne faut donc pas vendre « l'agent continue toutes vos tâches quand votre PC est éteint ». Ce serait une capacité à livrer et à vérifier, avec un environnement distant distinct des fichiers et sessions du PC.

Le chat serveur accumule également les fragments avant de construire le flux SSE (`apps/api/src/handlers.rs:3075`). Le suivi mobile et l'arrêt du travail doivent être vérifiés de bout en bout.

### 3. La consommation technique n'est pas encore un système commercial

La recherche des parcours de paiement, abonnements et droits d'offre n'a pas identifié de facturation ARO raccordée. La mention de Stripe dans un catalogue de plugins n'est pas un paiement de l'application.

Le backend possède des budgets de jobs et des compteurs. Cependant, le worker affecte la même estimation aux tokens entrants et sortants (`apps/api/src/agent_runner.rs:280`). La boucle locale ignore `requested_max_steps` (`crates/aro-runtime/src/lib.rs:918`) et conserve l'estimation de la génération courante, sans constituer à elle seule un relevé complet des coûts de la tâche.

Avant de financer l'IA d'un client, il faut une réservation et un plafond monétaire effectifs, la mesure de chaque appel, la réconciliation finale et la gestion des appels simultanés, erreurs et reprises. Une limite de contexte par appel ne limite pas le coût cumulé d'une tâche.

### 4. Les preuves de sécurité et de livraison doivent correspondre à l'offre

La migration `202607020011_private_data_rls_policies.sql` prépare des politiques de séparation pour conversations/messages/mémoires mais diffère leur activation. D'autres tables ont leur RLS : il ne faut pas généraliser l'absence à toute la base. `crates/aro-store/tests/rls.rs` reste un test de connexion, pas une preuve d'isolation intercomptes. Cela ne démontre pas une fuite ; cela laisse une condition de lancement à vérifier.

Le workflow `.github/workflows/release.yml` publie le conteneur API. La configuration Tauri prévoit des paquets, mais la chaîne examinée ne démontre pas la livraison de clients desktop signés et mis à jour. Le workflow mobile construit un APK debug et un simulateur iOS, ce qui ne prouve pas une distribution native publique.

## Premier public et première vente

Public proposé : indépendants en conseil, communication et gestion de projet, déjà utilisateurs d'IA, qui produisent chaque semaine des documents pour plusieurs clients. C'est une hypothèse tirée des fonctions du produit, pas une demande de marché démontrée.

Ce public permet de tester la valeur des dossiers, instructions, mémoire, documents et reprise du contexte sur un travail récurrent. Le secteur précis sera choisi selon les premiers utilisateurs réellement accessibles et leurs tâches.

Trois scénarios de validation :

1. Un brief et des documents sources deviennent un dossier de synthèse structuré avec ses références, sauvegardé dans le projet.
2. Des données et consignes deviennent un tableau et un document client que la personne vérifie puis utilise réellement.
3. Une modification arrive une semaine plus tard ; la personne retrouve le contexte et met à jour le livrable sans reconstruire sa demande.

Ces scénarios sont des cibles de test. Cet audit n'a pas démontré leur réussite complète.

La démonstration doit montrer les fichiers d'entrée, le résultat final, les vérifications et la reprise. Les réglages de modèles, plugins et agents deviennent des moyens au service de cette tâche.

Je différerais la promesse grand public universelle, l'offre entreprise complète, le contrôle général du téléphone et la multiplication des nouveaux connecteurs. L'ambition peut rester large ; les engagements de la première version doivent être précis.

## Concurrence et valeur défendable

Les pages officielles consultées le 14 septembre 2026 donnent deux repères particulièrement utiles :

- [Claude Cowork](https://claude.com/product/cowork) propose déjà du travail sur fichiers et des tâches à plusieurs étapes, avec Cowork inclus dans Pro à 20 $ mensuels ; web et mobile y sont décrits en bêta. La page signale des limites d'usage. ARO devra démontrer un avantage concret face à cette expérience.
- [AnythingLLM](https://anythingllm.com/cloud) propose du Docker gratuit et un cloud Basic à 50 $ mensuels, en demandant une clé LLM personnelle. Son [application desktop est gratuite](https://anythingllm.com/download). Cela montre qu'un service peut être vendu avec BYOK ; cela ne prouve pas sa rentabilité ni celle d'ARO.
- [LibreChat](https://www.librechat.ai/) fournit déjà une base multi-modèles extensible. Le simple accès à plusieurs fournisseurs ne peut pas porter toute la proposition payante.

L'avantage à construire serait l'association d'un espace durable, de fichiers portables, d'un contrôle clair du lieu de traitement et de parcours très soignés pour un métier. Aucun de ces éléments ne constitue à lui seul une exclusivité durable. La qualité d'exécution et la proximité avec les premiers clients seront décisives.

## Monétisation proposée

Deux offres à terme, avec un seul abonnement payant au départ :

| Offre proposée | Prix de départ à tester | Contenu |
|---|---:|---|
| Personnel local | Gratuit | Projets, consultation, édition/export et utilisation de moteurs locaux ou de clés personnelles, après livraison du vrai parcours autonome. |
| Plus | 19 €/mois, hypothèse | Synchronisation gérée, sauvegarde/restauration avec volume explicite, continuité entre appareils, assistance et fonctions de travail réellement validées. |

La clé personnelle signifie une facture IA payée au fournisseur par le client. Le service ARO conserve sa valeur propre. Les pilotes peuvent commencer avec des utilisateurs qui acceptent ce fonctionnement ; ce n'est pas un onboarding grand public final.

Pour les utilisateurs qui veulent un paiement unique et aucun paramétrage, ajouter ensuite une IA intégrée avec capacité incluse et coût maximal explicite. Ne fixer cette capacité qu'après mesure. Les tâches distantes payantes arrivent lorsque le moteur distant sait les exécuter.

Le prix de 19 € n'est ni validé par des clients ni calculé à partir des coûts d'exploitation du produit : ces données manquent. La réflexion du 7 septembre envisageait déjà ce niveau avec IA incluse ; la recommandation actuelle sépare plus nettement le revenu du service et le financement de l'IA pour éviter de promettre une capacité non mesurée.

La contribution à mesurer est : revenu hors taxes et frais de vente, moins infrastructure, support variable et IA financée. Elle doit ensuite payer développement, acquisition et frais fixes. Étudier les utilisateurs coûteux, les longues tâches, les appels parallèles et les reprises, pas seulement la moyenne.

Éviter l'IA cloud illimitée à petit prix, l'achat à vie incluant des dépenses récurrentes, la publicité dans les réponses et une marketplace supposée financer le lancement. Une offre équipe ou du déploiement accompagné peut venir après les premiers usages payants récurrents.

## Open source : choix et calendrier

Direction retenue : **socle local ouvert et services hébergés payants**. Première étape : **bêta privée**, tant que le périmètre autonome et le bénéfice payé ne sont pas validés.

Ouvrir en priorité les contrats d'intégration, exemples d'extensions et composants nécessaires à l'autonomie du client ; stabiliser ensuite une édition locale utile. Facturer l'exploitation gérée, les sauvegardes, la synchronisation, les capacités distantes et l'accompagnement. Un utilisateur technique qui exploite sa propre installation n'achètera pas nécessairement ces services : c'est un effet accepté du modèle.

L'ouverture peut faciliter l'inspection du logiciel, les extensions et la confiance. Elle apporte aussi maintenance publique, documentation, support et possibilités de forks. Une communauté n'apparaît pas automatiquement à la publication.

`Cargo.toml` déclare aujourd'hui `license = "Proprietary"`. Aucun changement n'est proposé dans cet audit. Avant publication, déterminer les droits sur les dépendances, les modèles redistribués, les contributions et la marque, puis choisir une licence cohérente avec le périmètre. Une restriction interdisant toute utilisation commerciale ne correspond pas à la [définition OSI de l'open source](https://opensource.org/osd).

## Distribution et acquisition

Commencer par Windows si les premiers pilotes utilisent Windows : c'est le chemin le plus directement aligné sur le poste de développement observé. Le choix doit suivre les appareils des vrais pilotes ; ajouter macOS ensuite si la demande le justifie.

Le site doit contenir une promesse, une courte démonstration sur un dossier réel, les conditions de traitement des données, un prix compréhensible et un téléchargement. Prévoir un installateur signé, des mises à jour fiables, une désinstallation propre et une première tâche guidée.

Le parcours initial proposé : installer, choisir ou créer un projet, ajouter quelques fichiers, obtenir un résultat sauvegardé. Pour la bêta, la configuration du modèle et du compte peut être accompagnée ; pour une diffusion large, elle doit devenir simple et explicite.

Recruter d'abord 10 à 15 personnes du même métier par réseau personnel et communautés professionnelles, avec leur accord. Observer leur travail, puis proposer le même abonnement pilote avec des engagements précis. Les vidéos de cas concrets et les recommandations des utilisateurs viennent avant la publicité à grande échelle.

GitHub peut soutenir une distribution technique. Docker convient à l'auto-hébergement. Le mobile peut soutenir l'adoption en permettant de retrouver les projets et les résultats, mais ne doit pas être annoncé comme un opérateur universel du PC.

## Nom et logo

Le logo associe croissance, branches de projets et connexions. Un nom court, humain et facile à dicter convient mieux à cette identité qu'un assemblage de mots techniques. L'interprétation ci-dessous est créative : ce ne sont pas des étymologies établies.

| Piste | Prononciation proposée | Intérêt | Réserve |
|---|---|---|---|
| **Ormevo** | or-mé-vo | Évoque l'orme et l'évolution ; lien simple avec l'arbre ; six lettres. Premier choix à tester. | Vérifier mémorisation, perception et transcription réelle. |
| Arviane | ar-vi-ane | Sonorité plus humaine ; peut soutenir une identité d'assistant. | Orthographe moins évidente à la dictée. |
| Ramelys | ra-mé-liss | Évoque les ramifications et une identité organique. | Ton plus poétique, pertinence professionnelle à tester. |

Les recherches web initiales ont écarté des pistes telles que Sylora, déjà employé par un [service IA](https://sylora.ai/terms-of-service), et Arvenio, déjà utilisé par une [entreprise de conseil et technologie](https://marketing.arvenio.de/). Pour les trois pistes retenues, aucun statut de disponibilité commerciale, de marque ou de domaine n'est établi. Ce sont des pistes de travail, pas une autorisation d'utilisation.

Tester les noms en aveugle avec le même logo : prononciation, écriture après écoute, souvenir le lendemain et association au produit. Pour un réveil vocal, ajouter de vrais essais de reconnaissance et de faux déclenchements. Garder ARO comme nom interne jusqu'à la décision ; ne pas renommer tout le code pour un simple favori.

## Plan de décision sur six semaines

Ce calendrier organise des expériences ; il ne garantit pas que les écarts techniques seront corrigés dans ce délai.

1. Semaine 1 : sélectionner un métier accessible, observer cinq tâches et choisir une démonstration reproductible. Tester les trois noms avec le logo existant.
2. Semaines 2 et 3 : terminer le parcours retenu et ses conditions de sécurité, mesurer coûts et erreurs, vérifier installation, fichiers et reprise. Si ces conditions échouent, prolonger cette étape.
3. Semaine 4 : faire utiliser le produit à 10–15 pilotes accompagnés. Mesurer premier résultat utilisé, fréquence, temps gagné et demandes de support.
4. Semaines 5 et 6 : proposer un paiement réel pour le service livré et observer le retour spontané au produit. Décider alors du périmètre à ouvrir et de l'élargissement de la distribution.

Critères exploratoires proposés : sur dix pilotes, au moins six réalisent un livrable qu'ils utilisent, quatre reviennent sans relance et trois acceptent de payer. Ces seuils sont des règles de décision choisies pour ce petit essai, pas des références sectorielles ni une preuve statistique de marché.

Si l'utilisation existe sans paiement, revoir la frontière payante et le public. Si les utilisateurs ne reviennent pas, revoir le cas d'usage avant d'ajouter des fonctions. Si les tâches sont utiles mais le support absorbe le prix, simplifier le parcours avant l'acquisition.

## Limites de l'analyse

La vérification `npm run check` a été exécutée. Une première tentative a été perturbée par le refus de lancement d'esbuild dans le bac à sable (`spawn EPERM`). La seconde, exécutée hors de cette restriction, se termine avec **35 erreurs et 2 avertissements dans 2 fichiers**. Les diagnostics incluent des noms de propriétés d'authentification de plugins incompatibles avec les types et des signatures de fonctions incompatibles. Ces erreurs constituent une condition de stabilisation supplémentaire ; aucun correctif applicatif n'a été réalisé dans cette analyse stratégique.

Inspection statique des chemins cités, des fichiers de configuration et des captures existantes. Aucune nouvelle preuve de paiement, de succès complet d'un agent sur une tâche métier, de signature publique, de disponibilité juridique d'un nom ou de rentabilité n'est produite. Les prix proposés et le public initial restent des hypothèses à tester. Les comparaisons externes décrivent les pages officielles consultées, pas des essais comparatifs des produits.
