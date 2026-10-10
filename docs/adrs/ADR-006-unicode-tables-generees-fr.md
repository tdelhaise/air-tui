# ADR-006 — Unicode : tables UCD générées

**Statut :** Proposé — 2026-10-10

## Contexte

Le cahier des charges demande Unicode et UTF-8 par défaut. `std` fournit `char`, `str` et
la validation UTF-8, mais **pas** deux propriétés dont une interface texte ne peut pas se
passer :

- la **largeur d'affichage** d'un caractère — East Asian Wide et Fullwidth, combinants de
  largeur nulle, emoji de largeur deux. Sans elle, tout alignement casse dès le premier
  idéogramme ou emoji ;
- la **segmentation en graphèmes** (UAX #29). Sans elle, le curseur d'un éditeur coupe les
  séquences à jointure de largeur nulle et les combinants : `Backspace` démembre un
  `👨‍👩‍👧‍👦` au lieu de le supprimer.

L'écosystème résout cela avec `unicode-width` et `unicode-segmentation`. ADR-005 les
exclut.

## Décision

Les tables sont **générées depuis l'UCD et committées**. Un `xtask` écrit en `std` pur
analyse les fichiers de référence du Consortium Unicode et émet `src/unicode/tables.rs` :

| Fichier UCD | Propriété produite |
|---|---|
| `EastAsianWidth.txt` | largeur d'affichage |
| `emoji-data.txt` | largeur des emoji, `Extended_Pictographic` |
| `GraphemeBreakProperty.txt` | segmentation en graphèmes (UAX #29) |
| `WordBreakProperty.txt` | segmentation en mots — déplacement par mot dans l'éditeur |

Représentation : tables de plages à deux niveaux, recherche binaire, ordre de grandeur de
quelques dizaines de kilo-octets de source. La version de l'UCD employée est inscrite en
tête du fichier généré, et le `xtask` sait vérifier que le fichier committé correspond à
ses entrées.

Licence : `Unicode-3.0`, permissive, déjà admise dans `deny.toml` — même précédent que
l'exception icu4x d'Air (ADR-016).

## Conséquences

- Coût réel, à ne pas sous-estimer : c'est le poste où les interfaces texte artisanales se
  cassent les dents. Jalon M3 à lui seul.
- Le fichier généré n'est **jamais** édité à la main. Même régime que le registre d'ADR
  d'Air.
- La mise à jour d'une version d'Unicode est une campagne identifiable : relancer le
  `xtask`, committer le diff, vérifier les tests de largeur sur les cas connus.

## Statut

Proposé, pas accepté : la forme exacte des tables sera arrêtée au moment de M3, après
mesure de l'empreinte et du temps de recherche.
