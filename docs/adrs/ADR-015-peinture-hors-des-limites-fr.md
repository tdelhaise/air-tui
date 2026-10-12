# ADR-015 — Une vue ne peint pas hors de ses limites ; son propriétaire le fait pour elle

**Statut :** Accepté — 2026-10-10, amendé le 2026-10-12 (jalon M7k, l'ombre des boutons)

## Contexte

Le peintre découpe toute écriture aux limites de la vue, intersectées avec le découpage
hérité. `AirPainter::set_cell` rend `false` au lieu d'écrire, et « le refus n'est pas une
erreur : c'est le découpage qui fonctionne » (M4, M5b). C'est cet invariant qui fait qu'une
étiquette trop large ne mange plus la bordure qui l'entoure, ce qu'elle faisait au jalon
M0.

Trois décorations de Turbo Vision tombent **hors** de la vue qui les porte, et l'ADR-008
les nomme dans son contexte : l'**ombre portée de deux colonnes**, la case de fermeture, et
le cadre. Les deux dernières sont dans la vue ; la première n'y est pas.

Tant que cette question n'était pas tranchée, `AirDecorations` — la troisième assiette du
triplet de l'ADR-008 — ne pouvait pas être écrite : une assiette qui décrirait une ombre
que rien ne dessine annoncerait une capacité absente. Le rôle `AirRole::ButtonShadow` était
dans la palette depuis le jalon M5, transcrit de Borland comme de `mc`, et **aucun widget
ne le lisait**.

## Décision

**L'invariant ne bouge pas.** Une vue ne peint pas hors de ses limites, sans exception.

Ce qui dépasse d'une vue est peint par **le propriétaire de la surface où cela dépasse**.
Pour l'ombre d'une fenêtre posée sur un bureau, c'est le bureau ; plus généralement, c'est
le parent, dans la boucle où il dessine ses enfants.

### Trois stratégies, et le dépôt en employait déjà deux

Le problème n'est pas nouveau, et il avait déjà été résolu une fois — ce qui est la
meilleure indication que la troisième stratégie n'était pas nécessaire :

1. **Agrandir la vue.** La barre de menu le fait depuis le jalon M5b : à l'ouverture d'un
   menu, `open_at` étend ses propres limites pour contenir le déroulant. Le découpage est
   respecté parce qu'il n'y a plus rien dehors. Cela vaut quand la vue **possède** la zone
   où elle grandit, et quand l'agrandissement ne gêne pas ses sœurs — la barre de menu est
   dans un emplacement d'écran, dessinée à part.
2. **Faire peindre le propriétaire.** C'est ce que cet ADR ajoute, pour l'ombre portée. La
   zone d'ombre appartient au bureau, pas à la fenêtre : c'est donc le bureau qui l'écrit,
   dans ses propres limites, juste avant de dessiner la fenêtre.
3. **Affaiblir le découpage** — une marge de débordement déclarée par la vue. **Écartée**,
   et les raisons sont dans les alternatives.

Le choix entre 1 et 2 n'est pas affaire de goût. Agrandir une fenêtre de deux colonnes et
d'une ligne pour y loger son ombre ferait entrer la zone d'ombre dans ses limites — donc
dans sa **détection de clic** et dans le calcul de son intérieur. Cliquer sur l'ombre
activerait la fenêtre, et son contenu serait disposé dans un rectangle plus grand que ce que
l'on voit. L'ombre n'appartient pas à la fenêtre : elle appartient à ce qui est derrière.

### Ce que l'ombre fait aux cellules

Elle ne pose pas un bloc de couleur : elle **recolore ce qui est dessous** en gardant le
caractère. C'est ce qui lui donne l'aspect d'une ombre sur le motif du bureau plutôt que
d'un rectangle collé à côté de la fenêtre. Le peintre sait lire une cellule — `cell()` —,
et c'est tout ce qu'il faut.

### Ce qui déclare une ombre, et ce qui la dessine

- la **vue** déclare le **genre** d'ombre qu'elle projette, par
  `AirViewState::shadow_style` — `None` par défaut : une ombre sur chaque vue couvrirait
  l'écran de gris. Un genre et non un booléen depuis le jalon M7k, parce qu'un bouton et
  une fenêtre n'ont pas la même ombre, et que la vue est seule à savoir lequel des deux
  elle est ;
- le **thème** décide de son apparence, par `AirDecorations` — enfin écrite, et avec une
  seule assiette qui décrit quelque chose que l'on dessine ;
- la **palette** donne sa couleur, par les rôles `AirRole::WindowShadow` et
  `AirRole::ButtonShadow`.

Les trois sont nécessaires, et séparés pour la raison habituelle : un thème sans ombre les
désactive toutes d'un coup, et une vue qui n'en projette pas n'en projette dans aucun thème.

### La couleur, et ce qu'on ne peut pas prétendre transcrire

Pour Midnight Commander, elle vient de `[core] shadow`, que **trente et un** des
trente-neuf skins livrés définissent — `gray;black` dans le skin par défaut. Le manuel
confirme l'usage : « Shadows. If this option is enabled, all dialogs and drop down menus
will have a shadow. »

Pour Turbo Vision, **ce n'est pas une transcription de chaîne de palette**, et le dire
vaut mieux qu'inventer une provenance : l'ombre d'une fenêtre n'a pas d'entrée dans
`cpAppColor` — `cpButton[8]` est l'ombre d'un **bouton**, déjà transcrite en
`AirRole::ButtonShadow`. La valeur retenue est l'attribut VGA `0x08`, gris sombre sur noir,
c'est-à-dire exactement ce que `mc` nomme de son côté. Deux apparences qui concordent ne
font pas une source, mais elles font un repère, et c'est ce qui est écrit à côté de la
valeur.

### L'ombre des boutons — l'amendement du jalon M7k

Les boutons en projettent une depuis ce jalon, et `AirRole::ButtonShadow` a enfin un
lecteur. Le mécanisme n'a pas changé d'un mot : un bouton est l'enfant d'une fenêtre ou
d'un dialogue, qui possède donc la zone où son ombre tombe. C'est **l'apparence** qui
diffère, et elle diffère par trois choses à la fois :

| | fenêtre | bouton |
| --- | --- | --- |
| forme | le rectangle **décalé**, moins la vue | un trait qui **épouse** la vue |
| taille | deux colonnes, une ligne | une colonne, une ligne |
| remplissage | recolore, garde le caractère | demi-blocs `▄` et `▀` |
| rôle | `WindowShadow` | `ButtonShadow` |

Trois différences simultanées, c'est la raison d'une **énumération** — `AirShadowStyle`,
deux variantes — plutôt que de trois champs sur `AirShadow`. Une ombre paramétrée par
trois boutons décrirait des combinaisons qu'aucun thème ne remplit, et un réglage sans
lecteur est exactement ce que cet ADR refuse d'écrire.

Les demi-blocs sont la transcription de Turbo Vision, qui emploie `0xDC` et `0xDF` de la
page de code 437 pour qu'une ombre de bouton paraisse **deux fois plus fine** que celle
d'une fenêtre : la cellule à droite du bouton n'est dans l'ombre que par sa moitié basse,
celle du dessous par sa moitié haute. La ligne du dessous est décalée d'une colonne, parce
que l'ombre est portée en bas **et** à droite — sans ce décalage, le trait dépasserait sous
le bord gauche du bouton, où rien ne le projette.

### Le fond que le demi-bloc laisse voir, et une convergence mesurée

Un demi-bloc ne couvre que la moitié de sa cellule : l'autre moitié doit montrer ce qu'il y
a derrière. L'ombre d'un bouton ne prend donc du rôle que son **avant-plan**, et garde le
fond de la cellule qu'elle assombrit. Turbo Vision écrit les deux couleurs de l'attribut
`0x70` — noir sur gris.

Les deux règles donnent le **même octet** dans un dialogue, et c'est mesuré : le fond d'un
dialogue de Turbo Vision *est* le gris de `cpButton[8]`, donc l'ombre des boutons de
`redit` sort en `38;5;0;48;5;7`, exactement l'attribut `0x70`. Elles ne diffèrent que là
où ce dépôt pose aussi des boutons et où Borland n'en posait pas : sur la fenêtre **bleue**
de l'exemple `frame`, où garder le fond donne `38;5;0;48;5;4` au lieu d'un rectangle gris
collé sur du bleu.

### Le dialogue de saisie est un propriétaire comme un autre

`AirPromptDialog` compose ses trois contrôles lui-même : ils ne paraissent pas dans ses
enfants, donc la traversée de l'arbre ne voit pas leurs ombres. C'est lui le propriétaire
de la surface où elles tombent, et c'est donc lui qui les peint — par la **même** fonction
que la traversée, `view::draw_view_shadow`, rendue publique pour cela. Un calcul écrit une
seconde fois dans le dialogue aurait dédoublé la règle, et une règle écrite deux fois finit
par ne plus l'être qu'une (leçon du jalon M7c).

Conséquence qu'il faut nommer : la zone de contenu du dialogue a gagné une **cinquième
ligne**, celle où l'ombre des boutons tombe. Ce n'est pas un espacement — sans elle, le
découpage écarterait l'ombre, ce qui serait juste et invisible. Un dialogue de saisie
demande donc sept lignes au lieu de six.

## Conséquences

- `AirTheme` devient le **triplet** que l'ADR-008 décrit depuis le jalon M5 : palette,
  glyphes, décorations. L'écart assumé pendant quatre jalons est comblé par une assiette
  qui décrit quelque chose de dessiné, et non par une structure vide ;
- la palette passe à **trente-neuf** rôles ;
- le dessin d'un enfant se fait désormais en deux temps dans `draw_view` — l'ombre, puis
  l'enfant —, et l'ordre compte : les enfants sont dessinés de l'arrière vers l'avant, donc
  l'ombre d'une fenêtre de premier plan tombe sur celles d'en dessous, et le corps d'une
  fenêtre de premier plan recouvre l'ombre qu'une autre y aurait projetée ;
- l'ombre est **découpée** comme tout le reste : une fenêtre contre le bord droit du bureau
  n'en projette qu'un morceau, ce qui est le comportement juste et non un cas à traiter ;
- le découpage du peintre reste sans exception, donc rien de ce que M4 et M5b garantissent
  n'est à revérifier ;
- **jalon M7k** : `AirDecorations` porte **deux** champs, et c'est le premier que les deux
  thèmes du dépôt remplissent différemment — Turbo Vision dessine un trait sous ses
  boutons, Midnight Commander n'en dessine aucun, ses boutons étant `[ OK ]`, plats. La
  règle « une assiette ne gagne un champ que le jour où deux thèmes le remplissent
  autrement » est respectée plutôt que contournée ;
- la palette garde **trente-neuf** rôles : aucun n'a été ajouté, un l'a été **lu**. Le
  quarantième est venu au jalon M7m, pour le séparateur de colonnes d'une liste, et il n'a
  rien à voir avec les ombres.

## Alternatives écartées

**Une marge de débordement déclarée par la vue.** `AirPainter` aurait découpé aux limites
élargies d'une marge que la vue annonce. C'est la solution la plus courte à écrire, et
c'est celle qui coûte le plus cher : elle rend l'invariant **conditionnel**, donc à
vérifier à chaque lecture de code — « cette vue a-t-elle une marge ? » devient une question
qu'on se pose partout. Elle ouvre aussi la porte à une vue qui peindrait sur ses sœurs sans
que l'ordre de dessin le dise, ce qui est précisément ce que le découpage empêchait.

**Une passe de dessin séparée pour les ombres.** Dessiner toutes les ombres de l'arbre
avant tout le reste aurait évité l'ordre à deux temps. Mais une ombre doit tomber **sur ce
qui est derrière et sous ce qui est devant** : une passe globale ne sait pas distinguer les
deux, et l'ombre d'une fenêtre d'arrière-plan passerait au-dessus d'une fenêtre de premier
plan.

**Agrandir la fenêtre de la taille de son ombre.** Ferait entrer l'ombre dans la détection
de clic et dans le calcul de l'intérieur. L'ombre n'appartient pas à la fenêtre.
