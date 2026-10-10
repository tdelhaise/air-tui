# ADR-009 — Versionnement semver, par crate et par PR

**Statut :** Accepté — 2026-10-10

## Contexte

air-tui est un espace de travail à plusieurs crates publié sur crates.io. Deux questions
se posent : quelle discipline de version, et à quelle granularité.

## Décision

**Semver strict**, et **chaque crate porte sa propre version**. Toute PR incrémente la
version des crates qu'elle touche, et seulement celles-là.

Conséquence immédiate sur le manifeste : `version` **n'est pas** déclarée dans
`[workspace.package]`. Une version partagée par héritage rendrait la règle inapplicable —
toucher `air-tui` ferait avancer `air-tui-sys` qui n'a pas changé, et le numéro cesserait
de dire quelque chose.

| Crate | Version d'amorçage |
|---|---|
| `air-tui-sys` | 0.1.0 |
| `air-tui` | 0.1.0 |

## Ce que semver signifie tant qu'on est en `0.x`

C'est le point qui se perd le plus souvent, donc il est écrit : en `0.y.z`, cargo traite
**`y` comme la case de rupture**. `0.1.0 → 0.2.0` est une version incompatible,
`0.1.0 → 0.1.1` est compatible. Il n'y a pas de place pour une « nouvelle fonctionnalité
sans rupture » distincte d'un correctif : les deux vont en `z`.

Donc, pour une PR :

| Nature du changement | Incrément |
|---|---|
| rupture d'API publique | `0.1.x` → `0.2.0` |
| ajout d'API, correctif, documentation d'API | `0.1.0` → `0.1.1` |
| changement purement interne, sans effet sur l'API publique | `0.1.0` → `0.1.1` |

Le passage en `1.0.0` n'arrive pas avant que l'API du modèle d'objets soit stabilisée —
au plus tôt après le jalon M6, quand `redit` aura prouvé que les widgets tiennent.

## Conséquences

- La dépendance interne porte une version explicite :
  `air-tui-sys = { path = "../air-tui-sys", version = "0.1.0" }`. Le `path` sert au
  développement local, la `version` à la publication.
- Une PR qui ne touche que `docs/` n'incrémente rien.
- Un gate `check-versions` est **à écrire** : comparer les crates modifiées sur la plage
  `origin/main..HEAD` aux versions incrémentées, et refuser une PR qui touche du code sans
  avancer le numéro. Tant qu'il n'existe pas, la règle est tenue à la main.
- `CHANGELOG.md` par crate reste à décider ; la question n'est pas tranchée par cet ADR.

## Alternatives écartées

**Une version unique pour l'espace de travail.** Plus simple à tenir, mais elle fait
mentir les numéros : un consommateur qui voit `air-tui-sys` passer de 0.3.0 à 0.4.0 doit
pouvoir en conclure que *cette* crate a rompu quelque chose.
