# ADR-005 — Journal d’effets et réconciliation

Statut : accepté le 16 juillet 2026.

## Contexte

Un worker peut réaliser un email, paiement, publication ou suppression puis tomber avant de persister le résultat. At-least-once peut alors répéter l’effet.

## Décision

Chaque effet possède input canonique, effect_key stable, idempotency key fournisseur si disponible et journal prepare/commit/verify/compensate. Un checkpoint PRE_EFFECT précède l’appel et POST_EFFECT suit la preuve.

StepAttempt distingue EffectPrepared, EffectUnknown, EffectCommitted et ResultRecorded. La perte de lease après préparation produit EffectUnknown. Seule une réconciliation explicite peut prouver committed ou absent. Un état inconnu non vérifiable attend une intervention ; il ne retry jamais automatiquement.

## Garanties

- exactly-once interne pour transition/résultat via transaction et fencing ;
- effectively-once externe seulement si fournisseur et clé d’idempotence le permettent ;
- sinon vérification, approval ou compensation explicite.

## Rollback

Une capability dépourvue de stratégie d’effet est désactivée pour les runs v2. Revenir au runtime legacy n’autorise pas le replay d’un effet déjà prepared/unknown/committed.
