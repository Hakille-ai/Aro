# ARO — Proposition économique : un abonnement de continuité

Document de réflexion du 7 septembre 2026. Aucune facturation ni modification applicative implémentée. Les prix ARO sont des hypothèses à tester, pas des offres disponibles. Base technique : [analyse du projet](./ANALYSE_PROJET_2026-09-06.md), capture examinée du 6 septembre ; le dépôt évolue en parallèle.

## 1. Décision proposée

Construire un espace de travail personnel durable, gratuit dans ses fonctions locales essentielles, puis vendre un service de prise en charge : IA hébergée, simplicité, synchronisation fiable, capacité de travail et assistance.

Promesse envisagée : **« Votre travail reste accessible. ARO vous indique comment continuer, sans dépense surprise. »**

Le mot « illimité » ne doit qualifier que des éléments réellement sans quota commercial, comme les projets locaux dans la limite du disque. Il ne peut pas signifier calcul cloud maximal, agents autonomes permanents et réponses immédiates pour tous à prix fixe. Même une file d'attente ne supprime pas la facture fournisseur.

L'innovation recherchée est un contrat produit cohérent : anticipation du coût, réservation avant exécution, continuité du contexte, choix transparents et respect du travail après résiliation. Ses composants existent ailleurs ; aucune nouveauté mondiale n'est revendiquée.

## 2. Ce que les autres vendent

Tarifs affichés sur les sources officielles consultées le 7 septembre 2026. Dollars affichés par les fournisseurs, pas conversion en euros ni prix final français.

| Produit | Repères | Mécanisme observable | Leçon pour ARO |
|---|---|---|---|
| ChatGPT / Work / Codex | Free, Go 8 $/mois, Plus 20 $, Pro à partir de 100 $, palier 200 $ | Offres groupées, budgets d'usage, modèles de coûts différents et crédits supplémentaires | Une souscription unifie le produit ; elle ne garantit pas tout calcul illimité |
| Claude | Pro 20 $ mensuel ou 200 $ annuels ; Max 100/200 $ mensuels | Plus de capacité, paliers 5×/20×, fenêtres d'usage et priorité | Les utilisateurs intensifs financent une capacité supérieure |
| Cursor | Pro affiché 20 $/mois ; Teams standard 40 $/utilisateur/mois | Usage de modèles inclus puis consommation à la demande facturée | L'intégration au travail peut justifier le prix au-delà du chat |
| Raycast | Pro 10 $ mensuel ou équivalent 8 $ annuel ; Advanced AI +8 $/mois | Produit gratuit utile, confort et IA payants, option pour modèles avancés | Très pertinent pour une application native qui utilise plusieurs modèles |
| Apple One France | Individuel 19,95 €, Famille 25,95 €, Premium 34,95 €/mois | Regroupement de services ; partage avec espaces privés ; stockage borné | Simplifier l'achat sans prétendre que chaque ressource est infinie |

Sources : [OpenAI](https://learn.chatgpt.com/fr-FR/docs/pricing), [Claude](https://claude.com/pricing), [limites Max](https://support.claude.com/en/articles/11049741-what-is-the-max-plan), [Cursor](https://cursor.com/pricing), [Raycast](https://www.raycast.com/pricing), [Apple One](https://www.apple.com/fr/apple-one/).

Ces pages décrivent les offres, pas leurs marges ou leur rentabilité. Nous ne connaissons ni leurs coûts négociés ni leur distribution réelle de consommation. Copier leurs tarifs ne prouve donc rien sur la viabilité d'ARO.

Lecture stratégique : ARO affronte aussi leurs fonctions de projets, mémoire, outils et agents. « Plusieurs IA dans une application » ne suffit pas. Il faut gagner sur des tâches concrètes avec les fichiers de l'utilisateur, la reprise du travail et la qualité de l'expérience.

## 3. Pourquoi ARO peut avoir une économie différente

L'analyse du code identifie une application Tauri/Svelte, un moteur Rust, des modèles locaux et distants, une mémoire SQLite, des outils de fichiers/documents, et une synchronisation cloud distincte.

Cela donne trois ressources à distinguer :

1. Le logiciel et le travail local : coût marginal fournisseur souvent faible, mais maintenance, support, énergie et matériel existent.
2. L'intelligence achetée auprès de fournisseurs : coût variable selon modèle, contexte, sorties, outils et répétitions.
3. Les services hébergés : synchronisation, stockage, sauvegarde, exécution distante et assistance.

Il faut mesurer ce qui marche sur des ordinateurs modestes. Le local n'est pas un cadeau universel : il exige du matériel, de la mémoire, de l'énergie et des modèles adaptés. Une clé API personnelle simplifie l'économie d'ARO mais transfère la facture au client ; elle n'est pas une offre d'IA gratuite.

Les abonnements grand public des fournisseurs ne doivent pas être supposés réutilisables comme crédits API ARO. Toute connexion par abonnement nécessiterait un mécanisme officiellement autorisé et ses propres conditions. Notre scénario de base utilise des API facturées ou des modèles locaux.

## 4. L'expérience proposée

### Un socle qui reste utile

- Projets locaux sans quota artificiel, lecture, édition et export.
- Historique local conservé après changement d'offre, sous contrôle de l'utilisateur.
- Modèles locaux compatibles sans compteur ARO ; clés personnelles possibles avec facture fournisseur distincte.
- Même attention à la lisibilité, à l'accessibilité et à la protection des fichiers pour tous.
- Une découverte cloud financée et bornée, annoncée comme telle.

Le gratuit ne comprend pas du stockage cloud éternel et illimité. Après résiliation, une période d'export du cloud et des règles de conservation devront être explicites ; le travail déjà enregistré localement reste utilisable. Cette expérience est une cible à construire : l'interface actuelle exige encore une authentification cloud dans son parcours normal.

### Un abonnement qui prend en charge les tâches

Le service choisit une méthode adaptée à la demande et aux préférences : calcul local, modèle économique ou modèle puissant. L'utilisateur peut imposer un modèle et voir l'effet sur sa capacité. Un changement susceptible de modifier la qualité ou d'envoyer des données hors du poste est visible et contrôlable.

Avant une tâche coûteuse, ARO estime et réserve une enveloppe maximale. Les appels parallèles partagent cette réservation. Une fois un travail admis, son budget ne disparaît pas parce qu'une autre conversation consomme le solde.

Si le travail dépasse le périmètre réservé, ARO conserve un point de reprise, explique ce qui reste et propose une extension. Cette protection ne garantit ni résultat parfait ni exécution infinie. Une réserve financée par ARO couvre certains incidents de service et leurs reprises, avec un plafond ; elle ne finance pas toutes les demandes insatisfaisantes sans fin.

### Continuer sans surprise

Quand la capacité hébergée disponible ne permet pas une nouvelle tâche :

- Continuer localement si le matériel et la qualité attendue le permettent.
- Choisir une méthode hébergée moins coûteuse si une capacité correspondante reste disponible.
- Programmer le travail après renouvellement de la capacité ou sur une capacité différée réellement financée, avec une estimation de délai.
- Utiliser sa clé personnelle, en connaissant son coût externe.
- Autoriser un supplément pour une tâche précise, avec prix maximal accepté avant lancement.

Si aucune de ces ressources n'existe, une nouvelle génération cloud doit attendre ou être financée. Le logiciel, les résultats et les outils locaux restent accessibles. Nous ne devons pas appeler cela « IA sans aucune limite ».

La mise en attente ne suffit pas à économiser. Les API Batch, lorsqu'elles conviennent, peuvent réduire les prix ; une tâche agentique interactive à dépendances successives n'est pas automatiquement éligible. Le provisionnement propre demande aussi une capacité finie et une étude de coût total.

### Une capacité souple, pas une monnaie fictive

Éviter le nombre de messages : un message peut coûter cent fois plus qu'un autre. Éviter aussi des crédits incompréhensibles dont le taux change silencieusement.

Piste à tester : une capacité incluse qui se reconstitue progressivement avec accumulation bornée, plutôt qu'une rupture brutale à la fin du mois. Afficher la prochaine disponibilité, une explication claire et le détail de consommation consultable. Ce mécanisme reste une limite de ressources ; il ne faut pas le masquer.

Ne pas fixer encore le taux de renouvellement, le plafond d'accumulation ou une équivalence en tâches : les distributions réelles de coût manquent. Les moyennes financières de la section suivante ne constituent pas une allocation promise à chaque client.

## 5. Une gamme simple à tester

| Offre envisagée | Prix mensuel de test | Valeur principale |
|---|---:|---|
| ARO | 0 € | Espace personnel local, outils essentiels, export, modèles locaux/clés personnelles, découverte cloud bornée |
| ARO Plus | 19 € TTC | IA hébergée intégrée, continuité des tâches dans leur budget, synchronisation fiable, sauvegarde bornée et assistance |
| ARO Pro | 49 € TTC, ultérieurement | Davantage de capacité puissante, priorité et exécutions parallèles encadrées pour utilisateurs intensifs |

Lancer le gratuit et un seul abonnement Plus d'abord. Pro devient pertinent quand les usages intensifs sont mesurés. Ne pas annoncer des agents cloud permanents dans le prix tant que leur exécution et leur coût ne sont pas validés.

Tester 15/19/24 € pour Plus sur des groupes comparables. Le prix doit être accepté pour une valeur observée. 19 € est un point de départ, pas un résultat de recherche utilisateur.

Accessibilité supplémentaire : places financées par écoles/employeurs, puis tarif solidaire autour de 9 € avec le même produit et une subvention identifiée. Toute réduction doit être financée, limitée par une enveloppe claire et testée. Éviter de vendre six consommations lourdes au prix d'une seule sous prétexte d'offre familiale.

Un compte « financé par mon organisation » peut être particulièrement intéressant : espace personnel privé, enveloppe professionnelle distincte, facturation à l'organisation des seuls usages autorisés. Cela exige une isolation et des règles de propriété des données solides, pas encore démontrées par l'analyse précédente.

## 6. Économie chiffrée : hypothèses et limites

### Pourquoi compter les messages est trompeur

Exemple texte, tarifs API standard à contexte court, sans cache, outils, image, audio ni taxes. Chaque appel utilise 4 000 tokens d'entrée et 1 000 de sortie facturée. Les tarifs consultés donnent :

| Modèle | Entrée / sortie par million de tokens | Coût calculé par appel | Pour 1 000 appels |
|---|---:|---:|---:|
| GPT-5.6 Luna | 0,20 $ / 1,20 $ | 0,002 $ | 2 $ |
| GPT-5.6 Sol | 4 $ / 20 $ | 0,036 $ | 36 $ |
| GPT-6 Astra | 10 $ / 50 $ | 0,090 $ | 90 $ |

Source : [tarifs API OpenAI](https://developers.openai.com/api/docs/pricing). Prix Sol promotionnel annoncé au moins jusqu'au 21 novembre 2026. Ce tableau ne compare pas la qualité et n'affirme pas que ces modèles sont interchangeables.

Exemple agent : 20 appels Sol, chacun de 50 000 tokens entrants et 2 000 sortants facturés, coûtent 20 × (0,05 × 4 + 0,002 × 20) = 4,80 $, avant outils et environnement. Trente tâches de ce profil coûtent 144 $. Le raisonnement facturé, les reprises et le contexte croissant peuvent accroître la consommation.

### Contribution par abonnement

Simulation en euros, hypothèse de taxe de 20 % et vente web directe. Ce n'est pas une détermination fiscale. Les frais de paiement sont des hypothèses, pas un devis fournisseur. Les coûts IA en euros ci-dessous sont des budgets de scénario indépendants des exemples API en dollars ; aucun taux de change implicite.

| Poste par client et par mois | Plus | Pro |
|---|---:|---:|
| Prix payé, taxe comprise selon hypothèse | 19,00 € | 49,00 € |
| Revenu hors taxe | 15,83 € | 40,83 € |
| Paiement | −0,55 € | −1,05 € |
| Infrastructure variable | −1,30 € | −2,50 € |
| Support variable / incidents | −1,00 € | −2,00 € |
| IA moyenne hypothétique | −4,00 € | −14,00 € |
| Contribution avant gratuit et frais fixes | **8,98 €** | **21,28 €** |

Ce n'est pas le bénéfice net. Salaires, acquisition, développement, frais fixes, remboursements et subvention des gratuits restent à financer. Les commissions de boutiques ne sont pas incluses. À 12 € d'IA moyenne pour Plus, sa contribution descend à 0,98 € ; à 15 €, elle devient négative, −2,02 €.

Le risque est aussi dans les extrêmes : p90/p99, utilisateurs automatisant en boucle, contextes gigantesques, appels simultanés, fraude aux essais. Ne pas attendre la facture mensuelle pour découvrir ces usages. L'admission des tâches doit rester bornée par des réservations effectives, même si la moyenne paraît confortable.

### Qui finance le gratuit ?

Simulation : 10 000 utilisateurs actifs mensuels, tous les payants sur Plus, contribution de 8,9833 € par payant, gratuit à 0,30 €/actif/mois, frais fixes de scénario de 4 000 €/mois. Ce dernier montant n'est pas une estimation des besoins d'une équipe complète.

| Conversion payante | Payants | Gratuits | Solde après gratuit et frais fixes du scénario |
|---|---:|---:|---:|
| 5 % | 500 | 9 500 | −2 358 € |
| 10 % | 1 000 | 9 000 | +2 283 € |
| 20 % | 2 000 | 8 000 | +11 567 € |

Formule : payants × contribution − gratuits × coût gratuit − frais fixes. Seuil d'équilibre de ce scénario : environ 7,54 % de conversion. Ce n'est pas une prévision de conversion ni un bénéfice net comptable. Avec seulement 1 € de coût moyen gratuit, le scénario à 10 % passe à environ −4 017 €.

Conséquence : un gratuit majoritairement local, ou financé par un tiers, est bien plus crédible qu'une promesse universelle de cloud gratuit intensif. On peut affecter une part explicite de la contribution à un fonds d'accès, mais l'enveloppe ne crée pas du calcul illimité.

## 7. Modèles écartés ou différés

| Modèle | Décision et raison |
|---|---|
| Tout illimité cloud à petit prix | Écarter : dépenses non bornées et sélection des utilisateurs les plus coûteux |
| Crédits seuls | Écarter comme expérience principale : l'utilisateur hésite à explorer et doit comprendre une monnaie technique |
| Facturation de chaque résultat | Différer aux tâches objectivement vérifiables : succès contestable, mauvais encouragement à déclarer le travail terminé |
| Achat unique à vie avec IA incluse | Écarter : revenu ponctuel contre dépenses récurrentes |
| Logiciel payant avec clés personnelles uniquement | Viable pour experts, insuffisant comme parcours grand public universel |
| Publicité / recommandations sponsorisées dans les réponses | Ne pas retenir : détourne la relation de confiance recherchée |
| Revente des données ou calcul sur les machines des utilisateurs | Ne pas utiliser comme financement implicite de la gratuité |
| Marketplace avec commission | Option ultérieure si des créateurs apportent une valeur réelle ; ne pas la compter dans la viabilité initiale |

## 8. Ce que « premium » doit signifier

L'inspiration Apple retenue ici est une interprétation de design : un achat compréhensible, des choix par défaut soignés, un produit cohérent et des espaces privés. Ce n'est pas une connaissance de sa stratégie interne.

Pour ARO : installation sans terminal, choix automatique compréhensible, temps de réponse lisible, fichiers protégés, arrêt effectif des agents, reprise fiable, export simple et facture prévisible. La sécurité de base doit être commune aux offres ; vendre une gouvernance d'équipe supplémentaire ne justifie pas d'affaiblir le gratuit.

La rétention doit venir du travail utile et de la confiance. Mesurer aussi les tâches réussies, pas seulement le temps passé. Permettre de partir proprement peut rendre la décision de confier son travail à ARO plus facile ; c'est une hypothèse à tester, pas une causalité prouvée.

## 9. Validation avant décision et implémentation

1. Sélectionner trois parcours réels : travailler sur un dossier de documents, modifier un petit projet avec revue des changements, retrouver puis réutiliser du contexte.
2. Observer environ 30 utilisateurs pendant quatre semaines, incluant machines modestes, débutants, professionnels et utilisateurs intensifs. Échantillon exploratoire, pas preuve statistique de marché.
3. Mesurer le coût complet par tâche acceptée : tous les appels, raisonnement facturé, cache, outils, reprises, durée et infrastructure. L'analyse précédente jugeait le comptage actuel insuffisant pour facturer.
4. Comparer plusieurs routages avec évaluation humaine de la qualité, du délai et des erreurs. Une baisse de coût qui double les reprises n'est pas une économie.
5. Tester des offres et une intention d'achat réelle sans fausse disponibilité ; comparer 15/19/24 €, puis observer les renouvellements effectifs lors d'un pilote payant autorisé.
6. Mesurer réussite, rétention à quatre semaines, activation, conversion, coût moyen/p90/p99, fréquence des changements de mode, temps d'attente et abandons associés. Suivre séparément local, cloud, clés personnelles et petites machines.
7. Décider après observation : prix, enveloppes réellement promises, capacité de secours financée, stockage, renouvellement, limites de parallélisme et conditions de supplément.

Prérequis techniques issus de l'analyse précédente : isolation des comptes locaux et cloud, protection contre l'application de modifications périmées, annulation réelle, mesure exacte de consommation et reprise d'exécution. Un moteur de réservation devrait ensuite comptabiliser admission, appels, coût effectif, libération du solde, échecs et remboursements de manière idempotente.

La priorité commerciale proposée est donc un abonnement individuel à l'expérience complète et prévisible. L'ambition grand public demeure ; l'acquisition initiale doit partir d'un usage où ARO démontre une différence que les utilisateurs acceptent de payer.
