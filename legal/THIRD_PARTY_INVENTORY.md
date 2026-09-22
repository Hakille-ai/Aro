# Inventaire de droits — registre à remplir avant publication

Cet inventaire est une liste de contrôles, pas une certification de compatibilité.

| Ensemble | Source à examiner | Preuve attendue | État |
| --- | --- | --- | --- |
| Code ARO | Historique Git, contrats auteurs/employeurs/prestataires | Autorisation de publication et double licence | À vérifier |
| Rust | Cargo.lock et manifeste de chaque crate, dépendances transitives | Texte de licence, attribution, compatibilité, versions distribuées | À vérifier |
| Web/desktop | package-lock.json, dépendances et ressources embarquées | Licences et notices, code réellement livré | À vérifier |
| Mobile | pubspec.lock, SDK/plugins et ressources | Licences, notices et droits de distribution | À vérifier |
| Modèles et tokenizers | Identifiant exact, révision, fournisseur des poids | Hébergement payant autorisé, limitations, attribution et conditions d'accès | Aucun modèle certifié ici |
| Images, polices, sons et marque | Origine de chaque ressource | Droits de redistribution et d'usage commercial | À vérifier |
| SDK/exemples Apache-2.0 envisagés | Répertoires et contributeurs précis | Séparation du cœur, titularité et notices | Périmètre à définir |

Conserver les textes et les preuves correspondant à chaque release, idéalement avec un SBOM. Aucun document ARO ne remplace les droits tiers.
