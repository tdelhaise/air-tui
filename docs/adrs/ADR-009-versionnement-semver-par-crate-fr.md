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
- Le gate `check-versions` **existe** depuis le jalon M6b : `cargo xtask check-versions`
  compare les crates touchées sur la plage `origin/main..HEAD` aux versions déclarées, et
  refuse une crate touchée dont le numéro n'avance pas. L'arbre de travail est **joint** à
  la plage : un contrôle qui ne regarderait que ce qui est committé passerait en silence
  sur un travail en cours, c'est-à-dire au pire moment pour rassurer. Une crate absente de
  la base est neuve, et n'a rien à dépasser.
- Il contrôle aussi la **cohérence des dépendances par chemin** : `version = "0.7.0"` doit
  être la version que porte réellement la crate visée. Cargo ne voit que
  l'incompatible — il refuse `^0.6.0` pour une crate en 0.7.0 — et laisse passer la dérive
  compatible : déclarer `0.7.0` alors que la crate porte `0.7.1` compile sans un mot, et
  c'est la publication qui livrerait une borne fausse, bien plus tard. C'est ce cas-là que
  le gate attrape.
- `CHANGELOG.md` par crate reste à décider ; la question n'est pas tranchée par cet ADR.

## Alternatives écartées

**Une version unique pour l'espace de travail.** Plus simple à tenir, mais elle fait
mentir les numéros : un consommateur qui voit `air-tui-sys` passer de 0.3.0 à 0.4.0 doit
pouvoir en conclure que *cette* crate a rompu quelque chose.
