# Confidentialité et DPA — dossier à compléter

Identifier les responsables de traitement et sous-traitants selon chaque service. Renseigner identité et contact, finalités, bases légales, catégories de données, destinataires, durées, droits et réclamations. Décrire séparément compte, facturation, contenu IA, diagnostics et assistance ; ne pas collecter les prompts à des fins d'entraînement sans décision et cadre explicites.

Cartographier les lieux réels de stockage et de traitement pour PostgreSQL, objets, vecteurs, inférence, voix, outils externes, paiement, journaux et sauvegardes. Pour chaque sous-traitant : entité, pays, fonction, garanties contractuelles et modalités de transfert. Un serveur installé chez le client n'empêche pas automatiquement les sorties réseau.

Annexes du DPA professionnel : objet/durée, instructions documentées, catégories de personnes/données, confidentialité des personnes autorisées, mesures techniques/organisationnelles, gestion des sous-traitants ultérieurs, assistance aux droits et incidents, restitution/effacement et audit. Renseigner les engagements opérationnels réels et les obligations légales applicables.

Attention spécifique à Compute : `billing_reservations.result` contient la réponse modèle pour la reprise idempotente. Choisir une rétention courte et explicite, implémenter la purge sans permettre une nouvelle facturation de la même requête, vérifier les sauvegardes et l'effacement. Le registre financier suit une rétention distincte. Les empreintes des clés ne remplacent pas la gestion de leur révocation et du départ des utilisateurs.
