# Architecture d'air-tui

## Les deux crates, et pourquoi la frontière est là

```
air-tui-sys  ── couche 0 ── le SEUL `unsafe`, le SEUL appel C. Six symboles de la libc.
     ▲
air-tui      ── couche 1 ── #![forbid(unsafe_code)]. Tout le reste.
```

La règle vient d'Air : « au-dessus de la couche 0, Rust pur et idiomatique — pas
d'assembleur, pas d'appel à du C, pas d'`unsafe` ». La frontière n'est pas une discipline,
c'est une crate : `air-tui` ne *peut pas* appeler de C, son attribut le lui interdit à la
compilation.

### La surface C, exhaustive

Six symboles, tous présents dans la bibliothèque C que `std` lie déjà — glibc sur
`*-linux-gnu`, libSystem sur `*-apple-darwin`. Aucune entrée dans `Cargo.toml`.

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

### Deux pièges d'ABI, traités explicitement

**`ioctl` est variadique.** Sur l'ABI arm64 d'Apple, les arguments variadiques passent par
la pile, là où les arguments fixes passent par registre. Déclarer `ioctl` en non-variadique
« marche » sur x86_64 et casse silencieusement sur Apple Silicon. La déclaration est donc
`fn ioctl(fd: i32, request: c_ulong, ...) -> i32`, et chaque opération reçoit son
enveloppe dédiée typée (ADR-021 §3).

**`struct termios` n'a pas la même forme sur Linux et macOS** : `tcflag_t` est 32 bits et
`NCCS` vaut 32 sur Linux ; 64 bits et 20 sur macOS. Plutôt que de recopier deux
définitions de champs — et deux jeux de constantes de drapeaux — depuis la mémoire,
`TerminalMode` est un **tampon opaque sur-dimensionné** que seul le noyau remplit. Le code
ne lit aucun champ : il enchaîne `tcgetattr` → `cfmakeraw` → `tcsetattr`, et conserve une
copie intacte pour la restauration. Aucune constante de drapeau n'apparaît dans le projet.

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
`airmc` en style Midnight Commander — sur le même moteur.

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

Un thème est un triplet, pas une palette :

```rust
pub struct AirTheme {
    palette:     AirPalette,     // rôles sémantiques → couleurs
    glyphs:      AirGlyphSet,    // cadres, cases, flèches, remplissages
    decorations: AirDecorations, // ombres, double ou simple trait, style des hotkeys
}
```

Ce qui fait reconnaître Turbo Vision tient autant aux glyphes (`╔═╗`, `[■]`, `▲▼`, `░`)
qu'aux couleurs. Et ce qui fait reconnaître Midnight Commander tient à sa **coquille** —
deux panneaux, ligne de commande, barre F1-F10 — pas à sa palette : d'où une seconde
application d'exemple plutôt qu'un simple jeu de couleurs.
