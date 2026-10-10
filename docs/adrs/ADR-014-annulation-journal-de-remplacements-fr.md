# ADR-014 — L'annulation : un journal de remplacements, pas des instantanés

**Statut :** Accepté — 2026-10-10

## Contexte

`redit` édite un fichier et ne sait pas revenir en arrière. Une frappe de trop se répare à
la main, un collage malheureux ne se répare pas du tout.

Le moment de trancher est celui-ci, et pas plus tard : les jalons M7d et M7e ont ajouté au
tampon des opérations dont **l'inverse n'est pas évident**. Remplacer une sélection par un
collage est deux gestes en un ; supprimer une sélection déplace le curseur *et* abandonne
l'ancre ; `SelectLine` pose l'ancre au début d'une ligne et le curseur au début de la
suivante, de sorte qu'une suppression retire la ligne entière. Écrire l'annulation après
coup sur ces opérations serait reconstituer leur effet depuis l'extérieur ; l'écrire
maintenant permet de les regarder.

Deux choses très différentes s'appellent « annulation », et le choix entre elles est la
décision de cet ADR.

## Décision

### Un journal de remplacements

Un pas d'annulation est un **remplacement** : « entre ces deux positions, il y avait ce
texte, il y a maintenant celui-là ». Son inverse est le même remplacement, les deux textes
échangés. C'est tout.

```rust
struct AirTextEdit {
    start:         AirTextPosition,      // là où le remplacement commence
    removed:       String,               // ce qui y était
    inserted:      String,               // ce qui y est
    cursor_before: AirTextPosition,      // où était le curseur avant
    anchor_before: Option<AirTextPosition>,
}
```

**Pas d'instantanés du tampon.** Un instantané coûte la taille du document à **chaque pas**
— un fichier d'un mégaoctet coûterait un mégaoctet par caractère tapé. Une entrée de
journal coûte la longueur du texte qui change, c'est-à-dire un caractère pour une frappe.
Le rapport entre les deux n'est pas un facteur constant : il est la taille du document.

L'instantané a pourtant un avantage réel, et c'est lui qu'on paie en le refusant : il est
**bête**. Il ne peut pas se tromper sur l'inverse, parce qu'il n'en calcule pas. Le journal,
lui, n'est juste que si chaque mutation produit son inverse exact — et c'est la contrainte
que la décision suivante satisfait.

### Une seule mutation primitive, par laquelle tout passe

Avant cet ADR, le tampon portait **sept** chirurgies de lignes écrites à la main :
`insert_character`, `insert_text`, `insert_line_break`, `delete_backward`,
`delete_forward`, `delete_line`, `delete_selection`. Chacune découpait, joignait et
déplaçait les lignes à sa façon.

Sept chemins de mutation, c'est sept inverses à écrire et à tenir d'accord. Il n'y en a
plus qu'un :

```rust
fn splice(&mut self, start, end, inserted: &str, changes) -> AirTextPosition
```

Les sept opérations se ramènent chacune à un triplet `(start, end, inserted)`, et c'est
`splice` qui fait la chirurgie, avance les générations, consigne
`AirTextChange::LineCountChanged` s'il le faut, et **enregistre l'entrée du journal**.

L'annulation n'est donc pas une couche posée à côté du tampon : c'est une propriété de son
unique point de mutation. Un huitième chemin d'édition qui contournerait `splice`
n'existerait pas longtemps — il ne compilerait pas sans toucher aux champs privés, et la
revue le verrait.

C'est le vrai gain de ce jalon, et il se mesure : la chirurgie de lignes passe de sept
écritures à une.

### Le geste, et non la frappe, est l'unité d'annulation

Taper `abc` puis annuler doit rendre la ligne telle qu'elle était, pas `ab`. Une entrée
nouvelle se **fond** dans la précédente quand les deux forment un geste continu :

- une insertion dans une insertion, si elle commence exactement là où la précédente
  finissait ;
- une suppression arrière dans une suppression arrière, si elle finit exactement là où la
  précédente commençait.

Tout le reste **coupe** le geste : un déplacement du curseur, un changement de sélection,
un saut de ligne, un collage, une suppression de sélection ou de ligne, un enregistrement,
un chargement. Un collage est donc un seul pas d'annulation, et c'est ce qu'on attend de
lui ; une touche `Entrée` est une frontière, parce qu'un paragraphe en est une.

La règle est écrite ici parce qu'elle est arbitraire dans ses bords : Midnight Commander
annule caractère par caractère, Turbo Vision ne garde qu'un seul pas. Ni l'un ni l'autre
n'est un argument ; ce qui en est un, c'est qu'un collage de dix lignes annulé en dix fois
serait un défaut, et qu'une ligne tapée annulée d'un coup en serait un autre.

### Le journal est borné, et ce qu'il perd est écrit

Deux cent cinquante-six pas. Au-delà, la plus ancienne entrée est oubliée. Un journal non
borné est une fuite lente dans une session longue ; une borne est une capacité qu'on annonce
plutôt qu'une mémoire qu'on promet.

### Le journal vit dans le modèle

Il ne peut pas vivre ailleurs : seul le tampon connaît le texte qu'une opération retire,
donc seul lui peut produire l'inverse. C'est l'inverse du presse-papiers, que le jalon M7d
a placé dans le **contrôleur** parce qu'il est partagé entre deux widgets. Les deux
décisions suivent la même règle — une chose appartient à qui la connaît.

Le contrôleur n'a donc rien à tenir. Il demande, par deux actions de plus :
`AirTextAction::Undo` et `AirTextAction::Redo`.

### Taper sur une sélection est **un** geste

Les trois insertions — caractère, texte, saut de ligne — remplacent la sélection **dans le
même remplacement**. Supprimer puis insérer en ferait deux entrées, et l'utilisateur devrait
annuler deux fois un geste qu'il n'a fait qu'une.

Ce n'est pas une décision prise d'avance : la première écriture faisait les deux, et ce sont
les essais de l'annulation qui l'ont rendu visible. C'est aussi ce qui a simplifié le code —
les trois insertions ne passent plus par la suppression de sélection.

### Le grisage reste en *pull*

`can_undo()` et `can_redo()` sont lues **au dessin**, comme `COPY` et `CUT` le sont depuis
M7d : c'est le mécanisme 3 de l'ADR-007. Aucun changement structurel n'est consigné quand
le journal se remplit ou se vide, et il n'en faut pas — une vue qui demande au moment où
elle peint ne peut pas afficher un état périmé.

## Conséquences

- la refonte des sept mutations en une est un changement **interne** : l'énumération
  `AirTextAction` garde ses variantes, et la suite d'essais du tampon — plus de cent — est
  le filet qui dit si la refonte a changé un comportement ;
- une entrée de journal garde le curseur et l'ancre d'**avant**. Annuler replace donc le
  curseur là où il était, ce qui est la moitié de ce qu'on attend d'une annulation : le
  texte revient, et le regard aussi ;
- la borne de deux cent cinquante-six pas est une constante nommée, pas un nombre répandu
  dans le code ;
- la vue d'édition gagne `Ctrl`+`Z` et `Ctrl`+`R`, et le menu Édition deux items. Le choix
  de la seconde touche s'est fait **après vérification**, pas de mémoire : `Ctrl`+`Y` est
  déjà liée, à la suppression de ligne de Turbo Vision, et la reprendre pour le
  rétablissement casserait un geste transcrit. `Ctrl`+`Maj`+`Z` a été écarté pour une
  raison mesurable : un terminal qui ne négocie pas le protocole Kitty ne le distingue pas
  de `Ctrl`+`Z` (ADR-003), donc le rétablissement marcherait sur certains terminaux et pas
  sur d'autres — pire que de ne pas l'offrir ;
- les deux touches sortent par le canal des **intentions** et non des commandes, puisque
  l'historique est dans le modèle que la vue adresse déjà. Les commandes `UNDO` et `REDO`
  existent pour la barre de menu, qui ne sait émettre que des commandes ;
- `Ctrl`+`Z` ne suspend pas le processus : `cfmakeraw` efface `ISIG`, et l'octet arrive à
  l'application. Vérifié dans `air-tui-platform-posix`, pas supposé.

## Alternatives écartées

**Les instantanés du tampon.** Simples et incapables de se tromper, mais au prix de la
taille du document par pas. Le refus se paie par la contrainte « une seule mutation
primitive », qui est exactement ce qui rend le journal sûr.

**Un journal d'actions, et non de remplacements.** Ré-appliquer `DeleteBackward` à l'envers
demanderait de savoir ce qu'il a supprimé, donc de reconstituer l'état d'avant. Les actions
sont des **intentions** dont l'effet dépend de l'état : `DeleteBackward` supprime une
sélection, ou une grappe, ou un saut de ligne. Une intention ne s'inverse pas ; un effet,
si.

**L'annulation dans le contrôleur de `redit`.** Il aurait fallu lui donner accès au texte
retiré, donc lui faire refaire le travail du tampon. Et l'annulation n'aurait existé que
pour `redit`, alors que tout tampon de texte la demande.
