# Licences et contrats ARO

Depuis la modification du dépôt du 22 septembre 2026, le code original du cœur ARO est placé sous **PolyForm Noncommercial 1.0.0**. Le [texte intégral](../LICENSE) s'applique par défaut aux éléments dont les titulaires autorisent cette licence. Les licences de tiers et les droits déjà accordés restent inchangés.

| Périmètre | Licence |
| --- | --- |
| Applications desktop/mobile, serveur, crates Rust, outils, documentation et UI tokens | PolyForm Noncommercial 1.0.0, sauf avis explicite propre à un élément |
| `packages/api-client` | Apache-2.0, texte dans le répertoire |
| `packages/contracts` | Apache-2.0, texte dans le répertoire |
| Modèles, dépendances et ressources tierces | Leurs licences propres |
| Usages du cœur non permis par PolyForm | Accord commercial distinct du titulaire autorisé |

Les SDK Apache peuvent être intégrés commercialement sans acheter une licence du SDK. Cela ne donne pas de droits supplémentaires sur le serveur ou l'application ARO. Les programmes d'exemple situés hors de ces deux répertoires suivent la licence racine ; aucune exception générale implicite.

Le cœur est **source disponible**, pas open source au sens de l'[OSI, §6](https://opensource.org/osd). La [licence PolyForm officielle](https://polyformproject.org/licenses/noncommercial/1.0.0) prévoit des usages et catégories d'organisations permis ; ne pas la résumer par « toute organisation doit payer ». Les services Cloud et Compute sont facturés indépendamment de la licence du code.

## Commercialisation

Le changement de licence du code est appliqué. Les [conditions commerciales](COMMERCIAL_TERMS.draft.md), [conditions Cloud](CLOUD_TERMS.draft.md) et [annexes données](PRIVACY_AND_DPA.draft.md) nécessitent encore l'identité réelle du titulaire/vendeur, ses coordonnées et les caractéristiques réelles du service. Ces trames ne constituent pas un contrat conclu ; aucun nom, numéro d'immatriculation ou engagement d'hébergement n'est inventé.

Avant de vendre : renseigner ces informations, vérifier la chaîne des droits (employeur, prestataires, coauteurs), finaliser les conditions applicables et configurer les liens publics dans Stripe. `ARO_COMMERCIAL_TERMS_APPROVED` reste faux tant que cette étape n'est pas réalisée. La mise en licence du dépôt ne valide ni les droits d'exploitation d'un modèle tiers ni la conformité d'une vente.

Voir [CONTRIBUTING.md](../CONTRIBUTING.md) pour les contributions, [NOTICE](../NOTICE) pour les exceptions et la marque, et [l'inventaire](THIRD_PARTY_INVENTORY.md) pour les dépendances. Ne pas remplacer les notices originales par la licence ARO.

## Provenance des textes

PolyForm : copie intégrale non modifiée du texte SPDX `PolyForm-Noncommercial-1.0.0`, [source](https://github.com/spdx/license-list-data/blob/main/text/PolyForm-Noncommercial-1.0.0.txt), SHA-256 `FFCCA38841ADB694B6F380647E15F17C446A4D1656FED51A1E2041D064C94CC8`.

Apache : copie intégrale non modifiée du [texte Apache officiel](https://www.apache.org/licenses/LICENSE-2.0.txt), SHA-256 `CFC7749B96F63BD31C3C42B5C471BF756814053E847C10F3EB003417BC523D30`.
