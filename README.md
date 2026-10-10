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
  défaut reproduit celui de Turbo Vision ; un second thème reproduit celui de Midnight
  Commander. Un thème est un triplet palette + glyphes + décorations.
- **Screens, Windows, Views.** Une hiérarchie explicite : l'application empile des
  `AirScreen` (surfaces plein écran commutables), chacune organise un `AirDesktop` de
  `AirWindow`, composés d'`AirView`.
- **MVC.** Le modèle ne connaît aucune vue et n'appelle personne : il consigne ce qui a
  changé et rend la main. Les vues n'émettent que des intentions.
- **Zéro dépendance.** La couche 0 (`air-tui-platform-posix`) isole les appels système
  derrière un contrat en traits ; tout le reste est du Rust sûr, en
  `#![forbid(unsafe_code)]`, et le cœur n'a aucune dépendance vers une implémentation de
  plateforme.

## État

**En construction, et déjà exécutable.** `cargo run --example frame` ouvre une interface
en style Turbo Vision qui répond : des menus déroulants, un champ de saisie qui édite par
grappes de graphème, une barre de statut à raccourcis, et le curseur du terminal à la
bonne colonne.

Ce qui manque encore : les applications `redit` et `rmc`, le thème Midnight Commander,
l'éditeur de texte et les listes. L'avancement réel, jalon par jalon, est tenu dans
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
