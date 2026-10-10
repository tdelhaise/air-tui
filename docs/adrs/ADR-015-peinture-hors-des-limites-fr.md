# ADR-015 — Une vue ne peint pas hors de ses limites ; son propriétaire le fait pour elle

**Statut :** Accepté — 2026-10-10

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

- la **vue** déclare qu'elle en projette une, par `AirViewState::casts_shadow`. Faux par
  défaut : une ombre sur chaque vue couvrirait l'écran de gris ;
- le **thème** décide de sa taille, par `AirDecorations` — enfin écrite, et avec une seule
  assiette qui décrit quelque chose que l'on dessine ;
- la **palette** donne sa couleur, par le rôle `AirRole::WindowShadow`.

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

### Ce que l'ombre ne couvre pas encore

Les **boutons** n'en projettent pas. Turbo Vision leur en donne une, et le mécanisme de cet
ADR y suffirait — un bouton est l'enfant d'une fenêtre, qui possède donc la zone où son
ombre tomberait. Ce n'est pas fait, et `AirRole::ButtonShadow` reste **lu par personne**.

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
  n'est à revérifier.

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
