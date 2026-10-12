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
  pour les rôles de liste et la coquille `rmc`, M7d pour les rôles du texte sélectionné,
  M7f pour ceux des cases à cocher et des boutons radio, M7g pour les attributs par rôle
  et la couleur du terminal.

## État au jalon M7

**Fait.** `cargo xtask import-mc-skin --skin <fichier.ini>` compile un skin en artefact
Rust committé, `--check` le confronte à son skin, et `AirTheme::midnight_commander()` rend
le thème. `air-tui` n'embarque aucun analyseur de `.ini` et n'ouvre aucun fichier pour
afficher un thème : la décision « compilés, pas analysés au runtime » est tenue par la
structure, pas par une consigne.

La table de correspondance est explicite et annotée : trente-cinq rôles sont transcrits
d'une seule clef du skin, trois sont composés de deux clefs faute d'être nommés par `mc`,
et seize correspondances portent écrite la règle qui a demandé un choix. Une dérivation
nommée se discute ; une dérivation silencieuse se découvre des mois plus tard devant un
écran qui ne ressemble pas au skin.

**Mesuré sur les trente-neuf skins livrés avec `mc` 4.8.33 : les trente-neuf sont
importables** depuis le jalon M7g. Ils étaient neuf à M7d et M7f, dix à M7b, douze à M7a.

### Ce que la mesure a démenti

Ce paragraphe portait, de M7a à M7f, un décompte par **cause de refus** : vingt skins pour
un attribut sur un rôle employé, neuf pour la couleur `default`, un — `gotar.ini` — pour
une clef absente. Il en concluait que « lever les deux premières limites rendrait
vingt-neuf des trente refusés importables ».

C'était **faux**, et le jalon M7g l'a montré en levant les deux limites : le compte est
passé de neuf à **seize**, pas à trente-huit. Le raisonnement avait un défaut qui ne se
voit qu'à la mesure : l'importeur s'arrête à la **première** cause qu'il rencontre, donc le
décompte comptait une cause par skin, alors que la plupart en avaient deux. Vingt-trois
skins restaient refusés, et pour une raison que l'ADR chiffrait à **un seul** skin :

- dix-huit n'écrivent pas `[editor] editframeactive` ;
- quatre n'ont pas de `[editor] _default_` ;
- `gotar.ini` n'a pas de `[core] inputmark`.

Une mesure qui compte des causes exclusives là où elles se cumulent ne se corrige pas en
raffinant le décompte : elle se corrige en levant une limite et en recomptant.

### La troisième cause n'était pas une limite du modèle

Et c'est le second démenti, plus coûteux que le premier : l'ADR rangeait la clef absente
parmi « les limites du modèle d'air-tui ou les lacunes du skin », et écrivait que « `mc` en
applique un [défaut] ; air-tui préférerait qu'on décide lequel ». Il n'y avait rien à
décider. Le manuel de `mc` décrit la cascade en deux endroits :

- `[core] _default_` — « Default color pair. Used in all other sections if they not contain
  color definitions » ;
- le `_default_` de chaque section — « Default color for this section. Used
  [core]._default_ if not specified ».

Une clef absente hérite donc du `_default_` de sa section, puis de celui de `[core]`.
Aucun des vingt-trois skins n'est mal formé, et `mc` les affiche tous. C'était un **défaut
de l'analyseur**, qui traitait l'absence comme une erreur là où le format la traite comme
une valeur vide — et une valeur vide, il savait déjà l'hériter depuis M7a.

La leçon est celle du jalon : **une limite attribuée au modèle est une hypothèse** tant
qu'elle n'a pas été levée. Les deux premières l'étaient vraiment ; la troisième était une
lacune qu'on avait rangée du mauvais côté, et il a suffi de lire le manuel pour s'en
apercevoir. Vingt-trois skins refusés depuis M7a par une ligne de code, et non par une
décision de conception.

### Ce qui reste vrai

**Ce qui fait baisser le compte est une clef nouvelle, pas un rôle nouveau.** Le jalon M7f
l'avait montré en démentant une formulation antérieure : il ajoute trois rôles, et le
compte n'avait pas bougé, parce que les trois se branchent sur `[dialog] _default_`,
`dfocus` et `dhotnormal`, déjà lues. Cette observation tient toujours — mais elle a perdu
son mordant depuis que la cascade garantit qu'une clef nouvelle est au pire héritée.

Les deux skins perdus entre M7a et M7b — `gray-green-purple256` et `gray-orange-blue256` —
posaient `bold` sur `[core] marked`, une clef que la table n'employait pas avant les rôles
de liste. Ils sont importables depuis M7g.

**Fait au jalon M7i : plusieurs skins compilés à la fois, et le choix par un nom.** Le
dépôt ne portait qu'**un** artefact : importer le second effaçait le premier, de sorte que
les trente-neuf importables ne l'étaient jamais ensemble. `theme/skins/mod.rs` est devenu
un **registre généré** qui déclare les modules et les nomme, et
`AirTheme::from_command_line_name` résout trois formes — `tv`, `mc`, et `mc:<nom>` pour un
skin compilé.

L'ADR n'est pas touché, et c'est le point : **rien n'est lu à l'exécution**. Le choix se
fait par une clef dans une table construite à la compilation, pas par un fichier ouvert au
démarrage. L'espace de noms `mc:` dit d'où vient un thème, ce qu'un nom nu ne dirait pas.

Le registre est généré plutôt qu'écrit parce que la liste des modules et la table des noms
disent la même chose deux fois : tenues à la main, elles divergeraient. Les deux sortent du
même balayage du répertoire, à chaque import.

Deux conséquences qu'il faut écrire :

- le nom d'un thème est le nom du fichier de skin **transposé en identifiant Rust** —
  minuscules, tirets en soulignés —, parce qu'un module Rust n'admet ni tiret ni majuscule
  et que `clippy -D warnings` refuse `non_snake_case`. `--list-themes` donne les noms
  exacts à taper, et c'est lui qui fait foi ;
- `xtask` dépend d'`air-tui`, donc un registre désaccordé de ses fichiers **empêche
  l'outil de se bâtir**, et c'est l'outil qui écrit le registre. Retirer un artefact à la
  main demande donc de corriger `mod.rs` une fois à la main avant de relancer un import.
  C'est une arête du dépôt à un seul espace de travail (ADR-005), payée deux fois en
  écrivant ce jalon.

**Trois skins sont repris**, chacun pour ce qu'il démontre : `default` (celui que `mc`
emploie sans configuration), `modarcon16-defbg` (la couleur du terminal, sur quinze rôles)
et `seasons-autumn16M` (les vraies couleurs et les attributs). Le registre en accepte
autant qu'on lui en donne ; n'en reprendre que trois est une décision de l'auteur du dépôt
— les trente-neuf coûteraient 395 Kio de source générée et deux dixièmes de seconde de
compilation, mesurés.

**Fait au jalon M7g : les deux limites du thème, et une troisième qui n'en était pas
une.** `AirColorPair` porte des attributs en plus de ses deux couleurs, et `AirColor`
compte une variante `TerminalDefault` que l'encodeur SGR émet par `39` et `49` — les deux
paramètres qui *annulent* une couleur au lieu d'en poser une. Elle traverse les trois
profondeurs du quantificateur inchangée, et c'est la seule dans ce cas : la quantification
choisit la plus proche voisine d'une couleur connue, et celle-ci est celle que
l'utilisateur a réglée dans son terminal.

`AirAttributes` a gagné `blink`, cinquième et dernier attribut que `mc` sait écrire.
**Aucun** des trente-neuf skins ne l'emploie — mesuré —, et il est là pour que la
correspondance soit totale : un test confronte les cinq noms de l'analyseur aux cinq champs
de la structure, de sorte que le refus « cet attribut n'a pas d'expression » soit
démontrablement inatteignable au lieu d'être supposé tel.

Le défaut que ce jalon rend possible est **silencieux** : un widget qui construit sa
cellule à partir des deux champs de couleur du couple laisse l'attribut au bord du chemin,
et ni l'un ni l'autre des deux thèmes livrés n'en pose, donc rien ne le montrerait. Deux
chemins le portent désormais — `AirColorPair::cell(character)` et
`AirPainter::draw_text_with_colors` —, et un test peint les douze widgets avec une palette
dont les trente-huit rôles sont gras en refusant la moindre cellule qui ne l'est pas. Le
test a été vérifié par son contrôle : rétabli sur un seul site, il échoue en nommant le
widget, la cellule et sa position.

**Fait au jalon M7j : la troisième assiette, et `AirTheme` est enfin le triplet que cet
ADR décrit.** L'écart a été assumé pendant quatre jalons, et pour une raison qui n'était pas
de la paresse : la seule décoration que l'assiette aurait portée est l'**ombre portée**, qui
tombe hors des limites de la vue qui la projette — ce que le découpage du peintre refusait
sans exception (M4, M5b). Une assiette qui aurait décrit une ombre que rien ne dessine
aurait annoncé une capacité absente.

L'**ADR-015** a tranché la question sans toucher à l'invariant : ce qui dépasse d'une vue
est peint par le propriétaire de la surface où cela dépasse. `AirDecorations` porte donc la
taille de l'ombre, la palette sa couleur par le rôle `AirRole::WindowShadow` — trente-neuf
rôles —, et `AirViewState::shadow_style` dit quelles vues en projettent une, et de quel
genre.

Deux champs n'y sont **pas**, et c'est délibéré :

- le **style de trait** vit déjà dans le jeu de glyphes, qui porte un cadre de fenêtre et
  un cadre de dialogue distincts — et c'est par là que le skin par défaut de `mc` se
  distingue de Turbo Vision, ses dialogues étant en trait simple ;
- le **style des touches rapides** est le marqueur `~X~`, en dur dans `widgets/hotkey.rs`.
  Les deux thèmes emploient le même, donc une assiette qui le porterait ne prouverait rien.

Une assiette ne gagne un champ que le jour où deux thèmes le remplissent différemment.

**Fait au jalon M7k : l'assiette a un second champ, et c'est le premier que les deux
thèmes remplissent différemment.** L'ombre des **boutons** — `cpButton[8]`, transcrit au
jalon M5 et lu par personne jusque-là — est dessinée par Turbo Vision et **absente** de
Midnight Commander, dont les boutons sont `[ OK ]`, plats, et dont le format de skin ne
décrit rien de tel. La règle du paragraphe précédent est donc respectée plutôt que
contournée : l'assiette a gagné `button_shadow` le jour où deux thèmes le remplissaient
autrement, et pas avant. Le détail de l'apparence — un trait fin en demi-blocs, une
colonne au lieu de deux — est dans l'ADR-015.

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

**Fait au jalon M7d : trois rôles pour le texte sélectionné.** `EditorSelected`,
`InputLineText` et `InputLineSelected` portent la palette à trente-cinq rôles. Les valeurs
de Turbo Vision viennent de la chaîne de palettes, et chacune est confirmée par une source
**indépendante du calcul** — la méthode de M7b, reprise parce qu'elle avait servi :
`cpEditor "\x06\x07"` donne `cpBlueWindow[6]` et `[7]`, soit `cpAppColor[13]` et `[14]`,
que `tvdemo2.cpp` nomme « Normal text » et « Highlighted text » dans son groupe « Puzzle »,
une fenêtre bleue comme celle de l'éditeur ; `cpInputLine` donne `cpAppColor[50]` et `[51]`,
que le même fichier nomme « Input normal » et « Input selected ».

Un résultat de cette transcription mérite d'être écrit, parce qu'il valide l'existant :
le texte **non** sélectionné de l'éditeur n'a pas besoin d'un rôle propre. `cpEditor[1]`
vaut `cpAppColor[13]`, qui est déjà `AirRole::WindowText`, transcrit au jalon M5. La vue
d'édition peignait donc juste depuis le début, et sans qu'on sache pourquoi.

**Fait au jalon M7f : trois rôles de cluster, et six glyphes qui ne viennent pas du skin.**
`ClusterNormal`, `ClusterSelected` et `ClusterHotkey` portent la palette à trente-huit
rôles. Les valeurs de Turbo Vision suivent la méthode des jalons précédents et sont
confirmées par une source indépendante du calcul : `cpCluster "\x10\x11\x12…"` donne
`cpGrayDialog[16..18]`, soit `cpAppColor[47..49]`, que `tvdemo2.cpp` nomme « Cluster
normal », « Cluster selected » et « Cluster shortcut ».

Les **six caractères** des cases à cocher et des boutons radio — `[`, `X`, `]`, `(`, `•`,
`)` — ne sont **pas** transcrits, et c'est écrit comme tel : aucun des trente-neuf skins ne
porte de clef pour eux, ce qui a été vérifié par un balayage et non supposé. Ils restent
donc ceux du thème par défaut. Le décompte des glyphes hors skin passe à douze, et il était
**faux** depuis M7b : il annonçait cinq glyphes et en énumérait six, le curseur d'ascenseur
ayant été ajouté sans que la phrase suive. Corrigé aux trois endroits qui le portaient.

Le jeu de glyphes a gagné `scroll_thumb`, pour une raison que seule la coquille pouvait
faire apparaître : le curseur d'ascenseur empruntait `close_box_mark`. Les deux glyphes
coïncident dans Turbo Vision — c'est le même `■` —, et le skin de `mc` ferme ses fenêtres
avec un `X` : la glissière portait donc un `X` en plein milieu. Deux usages sans rapport ne
peuvent pas partager un glyphe de thème, et un thème unique ne permettait pas de le voir.

## Alternatives écartées

**Thème = palette seule.** Ne reproduit ni l'un ni l'autre des deux looks visés.

**Analyse des fichiers de thème à l'exécution.** Contraire au Principe 11, et introduit un
analyseur tolérant sur un chemin d'entrée de décision.
