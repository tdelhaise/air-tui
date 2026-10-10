# air-tui

> **Note de langue** — la documentation du projet est rédigée en français d'abord ;
> la traduction anglaise suivra. Les identifiants du code, eux, sont en anglais.

**air-tui** est un framework d'interface en mode texte écrit en Rust, sans aucune
dépendance : seules la bibliothèque standard et les symboles de la bibliothèque C déjà
liée par `std` sont employés. Ni ncurses, ni terminfo, ni crate tierce.

Il s'adresse à ceux qui veulent le confort des interfaces texte à fenêtres de la grande
époque — menus déroulants, fenêtres superposables, dialogues modaux, barre de statut —
avec un moteur moderne : Unicode et UTF-8 par défaut, couleur 24 bits quand le terminal
la gère, souris, et une gestion du clavier qui va chercher les modificateurs là où le
terminal veut bien les donner.

## Ce qui le distingue

- **Thèmes graphiques.** Le look and feel est une donnée, pas du code. Le thème par
  défaut reproduit celui de Turbo Vision ; un second est **compilé** depuis le skin par
  défaut de Midnight Commander par un outil du dépôt, et non recopié à la main. Un thème est un triplet palette + glyphes + décorations.
- **Screens, Windows, Views.** Une hiérarchie explicite : l'application empile des
  `AirScreen` (surfaces plein écran commutables), chacune organise un `AirDesktop` de
  `AirWindow`, composés d'`AirView`.
- **MVC.** Le modèle ne connaît aucune vue et n'appelle personne : il consigne ce qui a
  changé et rend la main. Les vues n'émettent que des intentions. Pas un `Rc`, pas un
  `RefCell` : la réentrance est impossible par construction.
- **Zéro dépendance.** La couche 0 (`air-tui-platform-posix`) isole les appels système
  derrière un contrat en traits ; tout le reste est du Rust sûr, en
  `#![forbid(unsafe_code)]`, et le cœur n'a aucune dépendance vers une implémentation de
  plateforme.

## État

**En construction, et déjà utilisable.** `cargo run -p redit -- fichier.txt` ouvre
l'éditeur : menus déroulants, cadre à case de fermeture, ascenseurs, barre de statut à
raccourcis grisables, édition par grappes de graphème et déplacement par mot au sens de
l'annexe 29 d'Unicode.

```sh
cargo run --example frame                    # la démonstration des widgets
cargo run --example frame -- mc              # la même, au thème Midnight Commander
cargo run -p redit -- fichier.txt            # l'éditeur
cargo run -p redit -- --theme mc fichier.txt # l'éditeur, aux couleurs de mc
cargo run -p rmc                             # la coquille à deux panneaux
cargo run -p rmc -- --theme tv /usr /etc     # la même, aux couleurs de Turbo Vision
```

Les deux lancements d'un même programme dessinent **exactement** les mêmes vues : seules
la palette et les glyphes changent. C'est la démonstration la plus courte de ce que le
moteur de thèmes promet — et les deux moitiés que l'ADR-008 sépare se vérifient en deux
commandes : `redit --theme mc` donne un éditeur aux couleurs de `mc`, `rmc --theme tv`
donne la coquille de `mc` aux couleurs de Turbo Vision, et ni l'un ni l'autre n'est `mc`.

`rmc` parcourt, marque et relit ; il **n'exécute aucun programme et ne modifie aucun
fichier**.

`redit` sélectionne au clavier comme à la souris (`Maj`+flèches, `Maj`+clic, glisser,
`Ctrl`+`A`, **double clic** sur un mot, **triple clic** sur une ligne) et porte un
**presse-papiers interne** : copier, couper, coller, dans l'application. L'échange avec le
presse-papiers du système est un autre sujet, et il n'est pas traité.

Ce qui manque encore : l'étage d'entrée natif Linux, une liste à plusieurs colonnes, et le
défilement d'un groupe d'items plus long que sa place.
L'avancement réel, jalon par jalon, est tenu dans
[`docs/etat-avancement.md`](docs/etat-avancement.md) — et ce README décrit l'intention du
projet, qui va plus loin que ce qui est livré.

## Cibles

Linux et macOS en tier-1 (voir
[`docs/adrs/ADR-001-cibles-tier-1-fr.md`](docs/adrs/ADR-001-cibles-tier-1-fr.md)) :

| | Plancher | Architectures |
|---|---|---|
| Linux | noyau 6.1 | `x86_64`, `aarch64` |
| macOS | Sequoia 15 | `x86_64`, `aarch64` |

Une limite à connaître d'emblée : les **scancodes matériels** ne sont lisibles que sur la
console Linux locale. À travers un pty — donc en SSH, et sur macOS — il n'existe pas de
scancode, et la détection des combinaisons comme `Ctrl+Shift+Alt+X` dépend du protocole
clavier que le terminal annonce. Le détail est dans
[`docs/adrs/ADR-003-entree-clavier-deux-etages-fr.md`](docs/adrs/ADR-003-entree-clavier-deux-etages-fr.md).

## Documentation

- [`docs/INDEX.md`](docs/INDEX.md) — la carte du dépôt
- [`docs/ARCHITECTURE-fr.md`](docs/ARCHITECTURE-fr.md) — l'architecture d'ensemble
- [`docs/CONVENTIONS-fr.md`](docs/CONVENTIONS-fr.md) — les conventions de code
- [`docs/adrs/`](docs/adrs/) — les décisions techniques

## Licence

Mozilla Public License 2.0, par fichier. Voir [`LICENSE`](LICENSE).

Les tables Unicode de `crates/air-tui/src/unicode/tables.rs` sont générées depuis le
référentiel de caractères Unicode, sous licence `Unicode-3.0` — voir [`NOTICE`](NOTICE).
