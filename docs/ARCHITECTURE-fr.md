# Architecture d'air-tui

## Les trois crates, et pourquoi la frontière est là

```
             air-tui                  ← le cœur : ne connaît que le contrat
                │
       air-tui-platform               ← le contrat : des traits, zéro unsafe, zéro C
                ▲
      ┌─────────┴──────────┐
air-tui-platform-posix     air-tui-platform-air     ← à écrire
(Linux glibc, macOS)       (Rust intégral, linux-air)
```

La règle vient d'Air : « au-dessus de la couche 0, Rust pur et idiomatique — pas
d'assembleur, pas d'appel à du C, pas d'`unsafe` ». Mais la raison du découpage en trois
plutôt qu'en deux est le **portage à venir sur `linux-air`** (ADR-011), cible intégralement
en Rust dont la couche 0 n'est pas appelable par un framework. Sur cette cible, air-tui
sera en couche 2 ou 3.

D'où la conséquence qui structure tout : **`air-tui` n'a aucune dépendance vers une
implémentation de plateforme.** Elle ne peut pas atteindre le système, même par accident —
ce n'est pas une discipline, c'est le graphe de dépendances. C'est le binaire final qui
choisit une implémentation et l'injecte :

```rust
let device = air_tui_platform_posix::PosixTerminal::open()?;
let terminal = AirTerminal::new(Box::new(device))?;
```

Bénéfice immédiat, qui n'était pas la motivation : le rendu se teste **sans terminal**. Un
périphérique en mémoire implémente le contrat en trente lignes, et la couverture du module
`terminal.rs` du cœur est passée de 0 % à 93,8 % le jour où la couture a été posée.

### La surface C, exhaustive

Six symboles, tous dans la bibliothèque C que `std` lie déjà — glibc sur `*-linux-gnu`,
libSystem sur `*-apple-darwin`. Aucune entrée dans `Cargo.toml`. Ils vivent uniquement
dans `air-tui-platform-posix`.

| Symbole | Emploi |
|---|---|
| `tcgetattr` | lire l'état du terminal avant de le modifier |
| `tcsetattr` | passer en mode brut, et le restaurer |
| `cfmakeraw` | composer le mode brut sans réécrire les masques de drapeaux à la main |
| `ioctl` | `TIOCGWINSZ` (taille), plus tard `TIOCLINUX` (état des modificateurs sur VT) |
| `poll` | attendre une entrée avec délai, sans occuper le processeur |
| `isatty` | refuser de passer en mode brut sur ce qui n'est pas un terminal |

Trois choses sont volontairement **absentes** de cette liste, parce que `std` les couvre :
l'ouverture de `/dev/tty` (`std::fs::File`), la lecture et l'écriture
(`std::io::Read`/`Write`), et la récupération d'`errno`
(`std::io::Error::last_os_error()`).

### Trois pièges de plateforme, traités explicitement

**`ioctl` est variadique.** Sur l'ABI arm64 d'Apple, les arguments variadiques passent par
la pile, là où les arguments fixes passent par registre. Déclarer `ioctl` en non-variadique
« marche » sur `x86_64` et casse silencieusement sur Apple Silicon. La déclaration est donc
variadique, et chaque opération reçoit son enveloppe dédiée typée (ADR-021 §3 d'Air).

**`struct termios` n'a pas la même forme sur les deux plateformes** : `tcflag_t` est 32 bits
et `NCCS` vaut 32 sur Linux ; 64 bits et 20 sur macOS. Plutôt que de recopier deux
définitions de champs — et deux jeux de constantes de drapeaux — depuis une mémoire
faillible, le type est un **tampon opaque sur-dimensionné** que seul le noyau remplit. Le
code ne lit aucun champ : il enchaîne `tcgetattr` → `cfmakeraw` → `tcsetattr`, et garde une
copie intacte pour la restauration. Aucune constante de drapeau n'apparaît dans le projet,
donc aucune ne peut être fausse.

**`nfds_t` diffère** : `unsigned long` dans la glibc, `unsigned int` chez Apple. Et
`TIOCGWINSZ` n'a pas la même valeur, Linux employant un numéro plat là où les BSD encodent
la direction et la taille. C'est exactement ce que la crate d'implémentation existe pour
absorber.

### Les codecs

Toute conversion d'un flux passe par un codec — `Decoder`, `Encoder`, ou les deux — et tout
codec est tenu à **100 % de couverture** (ADR-012). Deux existent : l'encodeur SGR, et
l'écriture décimale qu'il emploie. Les décodeurs à venir — séquences ANSI d'entrée,
protocole Kitty, graphèmes Unicode — porteront chacun une cible `cargo-fuzz`.

## Unicode

Écrit au jalon M3 (`air-tui/src/unicode/`), avec des tables **générées depuis le
référentiel et committées** (ADR-006), édition 18.0.0 — `cargo xtask
generate-unicode-tables`.

Deux propriétés que `std` ne donne pas, et dont une interface texte ne peut pas se passer :

- la **largeur d'affichage**. Sans elle, un `漢` compté pour une colonne décale la bordure
  droite de sa fenêtre. `AirSurface::draw_text` écrit donc une **cellule de continuation**
  après une grappe large, et le rendu la saute : l'émettre écraserait la moitié droite du
  glyphe que le terminal vient de dessiner ;
- la **segmentation en graphèmes** (annexe 29). Sans elle, `Backspace` démembre un
  `👨‍👩‍👧‍👦` en quatre personnages. Les règles `GB3` à `GB13` sont implémentées, **y
  compris `GB9c`** — la coupure conjointe indienne, sans laquelle le curseur coupe au
  milieu d'un `क्ष` devanagari.

Un choix à connaître : la largeur d'une grappe est le **maximum** des largeurs de ses
caractères, pas leur somme. Une famille d'emoji soudée par des jointures occupe deux
colonnes, pas huit — c'est ce que font les terminaux modernes, et la seule définition qui
garde le curseur et le rendu d'accord entre eux.

## La hiérarchie d'objets

```
AirApplication                 boucle, terminal, thème, jeu de commandes
 └─ AirScreen  (pile)          surface plein écran : chrome + contenu, commutable
     ├─ AirMenuBar
     ├─ AirDesktop             zone des fenêtres : ordre de profondeur, cascade, mosaïque
     │   └─ AirWindow          cadre, titre, déplaçable, redimensionnable
     │       └─ AirView        tout ce qui se dessine et reçoit des événements
     └─ AirStatusLine
```

Et la couche basse, nommée pour ne pas entrer en collision avec `AirScreen` :

| Type | Rôle |
|---|---|
| `AirTerminal` | le périphérique : `/dev/tty`, mode brut, source d'entrée, puits de sortie |
| `AirSurface` | la grille de cellules, le différentiel, le suivi des zones salies |
| `AirCell` | un caractère, ses couleurs, ses attributs |

`AirScreen` est un **ajout** par rapport à Turbo Vision, qui n'a qu'un seul chrome
d'application figé. En faire un objet empilable et commutable donne ce que Turbo Vision ne
savait pas faire : un écran d'accueil, un assistant multi-étapes, et surtout la
cohabitation de deux chromes radicalement différents — `redit` en style Turbo Vision,
`rmc` en style Midnight Commander — sur le même moteur.

## L'arbre de vues

Écrit au jalon M4 (`air-tui/src/view/`). Trois pièces, et c'est la deuxième qui fait tout
tenir :

**Le modèle.** `AirView` est un **trait** et `AirViewState` une structure que chaque vue
**contient** — composition, pas héritage (ADR-011). Une vue expose son état et dit comment
se dessiner ; elle n'hérite de rien.

**Le peintre.** `AirPainter` est la pièce maîtresse. Une vue dessine à partir de `(0, 0)`,
**le sien** : elle ne sait pas où elle se trouve à l'écran, et c'est voulu — un bouton dans
une fenêtre déplacée n'a aucune raison d'être réécrit. C'est le modèle relatif au
propriétaire de Turbo Vision (`TView::origin`), et il apporte ce qui manquait depuis M0 :
**un découpage réel**. Toute écriture hors du rectangle de découpage est abandonnée, et un
peintre d'enfant intersecte son découpage avec celui de son parent. Une vue ne peut pas
déborder de son propriétaire, même en essayant. Le texte trop long qui mangeait le cadre
au jalon M0 est devenu impossible, et l'exemple `frame` en fait la démonstration en
insérant volontairement une étiquette plus large que sa fenêtre.

**Le routage.** Les trois phases de Turbo Vision, conservées telles quelles :
`PreProcess`, `Focused`, `PostProcess`. C'est ce qui permet à une barre de menu ouverte de
voir `Échap` avant le champ de saisie qui a le focus, et à un raccourci global de ne pas
voler une frappe qui lui était destinée. Les événements de souris, eux, se routent par la
**position** : du premier plan vers l'arrière, et les coordonnées sont traduites à la
descente puis restaurées à la remontée — une vue reçoit toujours des coordonnées qui sont
les siennes.

Le focus est **exclusif entre sœurs**, tenu par le groupe : le garder à un seul endroit est
la façon la plus sûre de ne pas le voir se dédoubler.

## Les commandes

`AirCommandSet` est le mécanisme 3 de l'ADR-007, et il mérite d'être signalé parce qu'il
résout en *pull* ce pour quoi on écrirait un observateur. Griser « Enregistrer » grise d'un
coup l'item de menu, le bouton et l'item de barre de statut — **sans que personne soit
notifié** : chaque vue interroge le jeu au moment où elle se dessine. Il n'y a aucune arête
à tenir à jour, donc aucune à oublier, et les widgets créés après la désactivation sont
grisés comme les autres.

## La bordure des vues

Contrairement à Turbo Vision, où le cadre est un `TFrame` qu'un `TWindow` insère comme
sous-vue, **toute `AirView` a une bordure par défaut** et la retire par `border = false`
(ADR-010). La zone de contenu est donc distincte des limites de la vue :
`content_bounds()` rend les limites moins une cellule de chaque côté quand la bordure est
présente, et c'est elle que le dessin d'un widget doit viser.

Les petits contrôles — bouton, étiquette, groupe de cases — posent `border = false` dans
leur constructeur : le défaut profite aux conteneurs.

## L'entrée, à deux étages

Détaillé dans [`adrs/ADR-003-entree-clavier-deux-etages-fr.md`](adrs/ADR-003-entree-clavier-deux-etages-fr.md).
Le point essentiel : **un pty ne transporte pas de scancode**. En SSH, l'émulateur côté
client a déjà traduit la frappe en séquence d'échappement. Le modèle de touche le dit :

```rust
pub struct AirKeyEvent {
    pub logical:   AirKey,              // toujours présent
    pub modifiers: AirModifiers,
    pub state:     AirKeyState,         // Press | Repeat | Release
    pub physical:  Option<AirScanCode>, // None sur un pty
}
```

Règle d'API : **aucun widget d'air-tui ne doit dépendre de `physical`**. Un widget qui le
ferait casserait en SSH.

### Ce qui est écrit

L'étage **terminal** l'est : `air-tui/src/input/` décode les caractères UTF-8, les
commandes C0, les combinaisons avec la touche alternative, les séquences CSI et SS3 avec
leurs modificateurs, la souris au format SGR 1006, les collages encadrés et les événements
de focus. Il est à 100 % de couverture et porte une cible `cargo-fuzz`, comme l'ADR-012
l'exige d'un décodeur.

Trois propriétés y sont tenues, et c'est ce qui le rend utilisable sur un flux réel :

- un **préfixe incomplet** ne consomme rien, donc une séquence coupée entre deux lectures
  n'est pas perdue — le fuzzing vérifie qu'un découpage arbitraire de l'entrée produit la
  même suite d'événements qu'un morceau unique ;
- une séquence **malformée** est une erreur, jamais une devinette ;
- une séquence **bien formée mais non reconnue** est consommée sans produire d'événement.
  Ce n'est pas de la tolérance : un terminal émet des réponses qu'on ne lui a pas
  demandées, et les refuser bloquerait le flux sur la première.

L'ambiguïté de l'échappement est traitée là où elle peut l'être : un `ESC` isolé est
indiscernable du début d'une séquence, seul le temps les sépare. Le décodeur, qui est pur,
rend « incomplet » ; c'est la boucle d'événements, qui sait qu'un délai s'est écoulé, qui
appelle `AirAnsiDecoder::isolated_escape_event`.

### Le protocole Kitty, et pourquoi il était indispensable

Écrit au jalon M2 (`air-tui/src/input/kitty.rs`). C'est le seul mécanisme qui livre les
modificateurs complets à travers un pty : sans lui, `Ctrl+Shift+S` et `Ctrl+S` produisent
tous deux l'octet `0x13` et sont indiscernables. Il apporte aussi deux choses qu'aucun
autre étage terminal ne donne :

- les **relâchements de touche**, et la distinction enfoncement / répétition ;
- la **touche de la disposition de base**, exposée par `AirKeyEvent::base_layout` et
  `shortcut_key()`. C'est ce qui fait fonctionner `Alt+F` pour « Fichier » sur un clavier
  cyrillique comme sur un QWERTY — exactement le problème qu'un framework à menus doit
  résoudre, et que le codage historique ne permet pas de résoudre du tout.

**La négociation ne repose pas sur un délai.** On envoie l'interrogation `CSI ? u`
*suivie* d'une demande d'attributs de périphérique. Les terminaux répondent dans l'ordre :
voir les attributs sans avoir vu de réponse Kitty avant eux est donc une conclusion
négative **certaine**. Le délai ne sert qu'au terminal qui ne répond à rien, pour ne jamais
geler le lancement. Et ce qui est tapé pendant l'interrogation est transmis à l'appelant,
pas jeté.

Le repli est `modifyOtherKeys` niveau 2, demandé **sans interrogation** : il n'existe pas
de sondage universel pour lui, mais la demande est inoffensive — un terminal qui ne la
comprend pas l'ignore.

Le protocole demandé est défait à la destruction du terminal. Laisser un terminal en
protocole Kitty après la sortie rendrait le shell inutilisable.

Deux limites consignées : la table des touches fonctionnelles ne couvre que les numéros
**certains** — tabulation, entrée, échappement, retour arrière, caractères imprimables,
`F13` à `F35` —, le pavé numérique et les touches multimédia étant hors table et donc
consommés sans événement ; et cette table a été écrite d'après la spécification, **pas
vérifiée contre un terminal réel**.

L'étage **natif** (M8) reste à écrire.

## Le rendu

`AirSurface` tient deux grilles — la courante et la précédente — et produit un
différentiel. `AirTerminal` traduit ce différentiel en séquences ANSI, encadrées par les
séquences de mise à jour synchronisée (`2026h` / `2026l`) quand le terminal les annonce,
ce qui supprime le scintillement sur les redessins larges.

Aucune allocation sur le chemin chaud : les deux grilles et le tampon de sortie sont
alloués à la construction et réutilisés (ADR-021 §4).

## MVC

Détaillé dans [`adrs/ADR-007-mvc-journal-de-changements-fr.md`](adrs/ADR-007-mvc-journal-de-changements-fr.md).
L'arête Observer du MVC canonique est remplacée par une **donnée rendue à l'appelant** :
le modèle consigne ce qui a changé et ne notifie personne. Conséquence directe : aucun
`Rc`, aucun `RefCell`, et la réentrance est impossible par construction.

## Les thèmes

Écrit au jalon M5. Un thème est un **triplet** — palette, glyphes, décorations — parce que
ce qui fait reconnaître un look tient autant aux caractères qu'aux couleurs.

### La palette est transcrite, pas approchée

Les valeurs viennent de `cpAppColor` et `cpGrayDialog` de `magiblot/tvision`, en suivant
les chaînes de `cpBackground`, `cpMenuView`, `cpStatusLine`, `cpFrame` et `cpButton`.
Chaque entrée porte en commentaire l'octet d'origine et sa chaîne : la transcription se
vérifie ligne à ligne contre le source C++.

Deux pièges s'y cachaient, et ils ont chacun produit une correction :

- **les indices VGA ne sont pas les indices ANSI.** Dans un attribut VGA, 1 est le bleu et
  4 le rouge ; en ANSI c'est l'inverse. Transcrire sans conversion donnait un bureau rouge
  et des lettres d'accès rapide bleues. La conversion est faite par `ansi_from_vga`, et un
  test vérifie qu'elle est une bijection qui échange bien ces paires ;
- **la barre de statut n'est pas cyan.** `cpStatusLine` et `cpMenuView` partagent la même
  chaîne : elle est noir sur gris clair, comme la barre de menu. Les premières valeurs de
  ce projet la donnaient sur cyan, par souvenir plutôt que par lecture.

### Le quantificateur

Requis et non optionnel (ADR-008), pour une raison concrète : un thème s'écrit en RGB,
mais une console Linux n'a que seize couleurs. Sans lui, un thème soigné s'affiche en
couleurs arbitraires sur la moitié des terminaux visés.

Trois profondeurs — vraies couleurs, deux cent cinquante-six, seize —, déduites de
`COLORTERM` et de `TERM` et forçables par l'application. La quantification a lieu **au
dernier moment**, dans le rendu : un thème est écrit une fois, et chaque terminal reçoit ce
qu'il sait afficher. L'encodeur SGR, lui, reste pur.

Une couleur déjà représentable traverse sans être touchée, et la quantification est
**idempotente** — un test le vérifie, parce que sans cela un rendu partiel et un rendu
complet donneraient deux images différentes.

La métrique est euclidienne dans l'espace RGB. Ce n'est pas perceptuel, et c'est dit : une
distance CIE choisirait mieux sur les teintes voisines, au prix de tables et d'une
arithmétique en flottant qu'ADR-005 ne permet pas.

### Thème et coquille restent deux choses

Ce qui fait reconnaître Midnight Commander est sa **coquille** — deux panneaux, ligne de
commande, barre F1-F10 — pas sa palette. `AirTheme` porte palette, glyphes et décorations ;
la composition du chrome relève de l'`AirScreen` que l'application choisit. D'où deux
applications d'exemple à venir, `redit` et `rmc`, et non une application à deux thèmes.

Les thèmes sont **compilés, pas analysés au runtime** : l'importeur de skins Midnight
Commander sera un `xtask` qui compile un `.ini` en artefact Rust committé (Principe 11
d'Air).
