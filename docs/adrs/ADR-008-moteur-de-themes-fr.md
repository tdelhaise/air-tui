# ADR-008 — Moteur de thèmes

**Statut :** Accepté — 2026-10-10

## Contexte

Le look and feel doit être un thème, le thème par défaut reproduisant Turbo Vision et un
second reproduisant Midnight Commander.

Le piège est de croire qu'un thème est une palette de couleurs. Ce qui fait reconnaître
Turbo Vision tient autant à ses **glyphes** — cadres doubles `╔═╗` pour les dialogues,
simples pour les fenêtres, case de fermeture `[■]`, flèches `▲▼`, bureau en `░`, ombre
portée de deux colonnes — qu'à ses couleurs.

## Décision

Un thème est un **triplet**, pas une palette :

```rust
pub struct AirTheme {
    palette:     AirPalette,     // rôles sémantiques → couleurs
    glyphs:      AirGlyphSet,    // cadres, cases, flèches, remplissages, curseurs
    decorations: AirDecorations, // ombres, simple ou double trait, style des hotkeys
}
```

**La palette est indexée par rôles sémantiques** — `window.frame.active`,
`menu.item.hotkey`, `dialog.button.default`, `scrollbar.thumb` — et non par la chaîne de
remappage à 63 octets de Borland. La fidélité du thème par défaut est obtenue en
**instanciant** les valeurs exactes de Borland dans ce modèle, pas en reproduisant son
mécanisme.

**Un quantificateur est requis, pas optionnel.** Un thème écrit en RGB 24 bits doit
dégrader proprement vers 256 couleurs puis vers les 16 couleurs ANSI d'une console Linux.
Sans lui, le thème par défaut est illisible sur une VT.

**Thème et coquille sont deux choses distinctes.** Ce qui fait reconnaître Midnight
Commander est sa coquille — deux panneaux, ligne de commande, barre F1-F10 étiquetée — pas
sa palette. Un « thème MC » appliqué au chrome de Turbo Vision ne ressemblerait pas à MC.
D'où : `AirTheme` porte palette, glyphes et décorations ; la composition du chrome relève
de l'`AirScreen` que l'application choisit. Deux applications d'exemple en découlent,
`redit` et `rmc`, et non une application à deux thèmes.

**Les thèmes sont compilés, pas analysés au runtime.** Le Principe 11 d'Air — « quand un
texte doit entrer, il ne passe pas par la porte d'exécution : un outil dédié le compile en
artefact vérifié » — s'applique directement. L'importeur de skins Midnight Commander est
donc un `xtask` qui compile un `.ini` en artefact Rust committé : un skin mal formé est un
refus à la compilation, jamais un comportement dégradé à l'exécution.

## Conséquences

- Le thème Turbo Vision est le défaut et sert de référence de non-régression visuelle : la
  comparaison se fait contre `tvdemo` de `magiblot/tvision`.
- L'importeur de skins MC donne accès à des thèmes existants pour le coût d'un analyseur
  `.ini` — mais côté `xtask`, pas côté runtime.
- Jalons : M5 pour le moteur et le thème Turbo Vision, M7a pour MC et l'importeur, M7b
  pour les rôles de liste et la coquille `rmc`.

## État au jalon M7

**Fait.** `cargo xtask import-mc-skin --skin <fichier.ini>` compile un skin en artefact
Rust committé, `--check` le confronte à son skin, et `AirTheme::midnight_commander()` rend
le thème. `air-tui` n'embarque aucun analyseur de `.ini` et n'ouvre aucun fichier pour
afficher un thème : la décision « compilés, pas analysés au runtime » est tenue par la
structure, pas par une consigne.

La table de correspondance est explicite et annotée : vingt-neuf rôles sont transcrits
d'une seule clef du skin, trois sont composés de deux clefs faute d'être nommés par `mc`,
et quinze correspondances portent écrite la règle qui a demandé un choix. Une dérivation
nommée se discute ; une dérivation silencieuse se découvre des mois plus tard devant un
écran qui ne ressemble pas au skin.

**Mesuré sur les trente-neuf skins livrés avec `mc` 4.8.33 : dix sont importables** au
jalon M7b, contre douze à M7a. Les vingt-neuf autres sont **refusés**, et pour deux raisons
qui sont des limites du modèle d'air-tui, non des défauts de l'analyseur :

- vingt posent un **attribut** (`bold`, `underline`…) sur un rôle employé, et
  `AirColorPair` ne porte que deux couleurs. L'ignorer donnerait un thème qui ne ressemble
  pas au skin ;
- neuf emploient la couleur `default`, le fond transparent de `mc`, qu'`AirColor` ne sait
  pas exprimer.

Les deux skins perdus entre M7a et M7b — `gray-green-purple256` et `gray-orange-blue256` —
posent `bold` sur `[core] marked`, une clef que la table n'employait pas avant les rôles de
liste. Le compte baisse donc **parce que le modèle s'est enrichi** : un rôle de plus est
une clef de plus à savoir exprimer. C'est le prix de l'exigence, et il est chiffré.

Lever ces deux limites — un attribut par rôle, une couleur « défaut du terminal » — rendrait
les vingt-neuf importables. C'est chiffré, donc décidable ; ce n'est pas fait.

**Pas fait : la troisième assiette.** Cet ADR décrit un triplet, et `AirTheme` est un
**couple** `palette` + `glyphs`. `AirDecorations` n'existe pas, et l'écart est assumé
plutôt que comblé par une structure vide :

- le **style de trait** vit déjà dans le jeu de glyphes, qui porte un cadre de fenêtre et
  un cadre de dialogue distincts — et c'est par là que le skin par défaut de `mc` se
  distingue de Turbo Vision, ses dialogues étant en trait simple ;
- l'**ombre portée** ne peut pas être dessinée aujourd'hui : elle tombe hors des limites de
  la vue qui la projette, et le découpage du peintre ne souffre aucune exception (M4, M5b).
  Le rôle `AirRole::ButtonShadow` est dans la palette, transcrit de Borland comme de `mc`,
  et aucun widget ne le lit ;
- le **style des touches rapides** est le marqueur `~X~`, en dur dans `widgets/hotkey.rs`.
  Les deux thèmes emploient le même, donc une assiette qui le porterait ne prouverait rien.

Une assiette qui décrirait une ombre que rien ne dessine annoncerait une capacité absente.
La trancher demande de décider si une vue peut peindre hors de ses limites — ce qui est une
décision d'architecture, pas de thème.

**Fait au jalon M7b : `rmc`.** Cet ADR prévient que « ce qui fait reconnaître Midnight
Commander est sa coquille, pas sa palette ». Les deux moitiés existent désormais
séparément, et c'est ce qui rend l'avertissement vérifiable : `redit --theme mc` donne un
éditeur aux couleurs de `mc`, `rmc --theme tv` donne la coquille de `mc` aux couleurs de
Turbo Vision, et ni l'un ni l'autre n'est `mc`. La séparation n'est plus une affirmation de
l'ADR : c'est une ligne de commande.

La palette a gagné à cette occasion ses **quatre rôles de liste** — `ListNormal`,
`ListSelected`, `ListMarked`, `ListMarkedSelected` —, première extension depuis qu'un
second thème existe. C'est ce qui a éprouvé le moteur plutôt que de le décrire : ajouter un
rôle demande une valeur dans chacun des deux thèmes, et celle du thème compilé vient d'une
clef du skin que la table désigne. `mc` nomme les quatre ; Turbo Vision n'en nomme que
trois, et le quatrième est composé.

Le jeu de glyphes a gagné `scroll_thumb`, pour une raison que seule la coquille pouvait
faire apparaître : le curseur d'ascenseur empruntait `close_box_mark`. Les deux glyphes
coïncident dans Turbo Vision — c'est le même `■` —, et le skin de `mc` ferme ses fenêtres
avec un `X` : la glissière portait donc un `X` en plein milieu. Deux usages sans rapport ne
peuvent pas partager un glyphe de thème, et un thème unique ne permettait pas de le voir.

## Alternatives écartées

**Thème = palette seule.** Ne reproduit ni l'un ni l'autre des deux looks visés.

**Analyse des fichiers de thème à l'exécution.** Contraire au Principe 11, et introduit un
analyseur tolérant sur un chemin d'entrée de décision.
