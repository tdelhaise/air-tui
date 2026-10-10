# État d'avancement d'air-tui

Document de suivi **vivant** : où en est-on, quelle est la prochaine tâche. Il se met à
jour à chaque session. L'historique figé est dans [`JOURNAL.md`](JOURNAL.md).

**Dernière mise à jour : 2026-10-10** (jalon M1 — décodeur ANSI d'entrée).

## Dépôt

<https://github.com/tdelhaise/air-tui> — public, MPL-2.0, branche `main` protégée : toute
modification passe par une pull request, y compris pour les administrateurs. Les commits
sont signés par la clef `5C6F8A2E20E58061` (`thierry.delhaise@air-desktop.org`).

**Aucune CI n'existe à ce jour**, donc aucun contrôle n'est requis à la fusion : les
vérifications de `CLAUDE.md` sont à exécuter en local avant d'ouvrir une PR.

## Où en est-on

Jalon **M0 terminé sur macOS**, jalon **M1 terminé**. Le dépôt porte ses conventions, ses
ADR fondateurs, le socle de rendu et le décodeur d'entrée. Ce n'est pas encore utilisable
comme framework : il n'y a ni arbre de vues, ni widget.

### Fait

- Conventions reprises d'Air et consignées (`docs/CONVENTIONS-fr.md`), dont la règle de
  nommage sans abréviation et son glossaire.
- ADR-001 à ADR-012 rédigés (français seulement, traduction anglaise à venir).
- Espace de travail à **trois** crates (ADR-011), lints d'arithmétique défensive en `deny`.
- `air-tui-platform` : le contrat — `TerminalDevice` (object-safe), `Decoder`, `Encoder`,
  `PlatformError` portable, `WindowSize`, `InputReadiness`. Zéro `unsafe`, zéro C.
- `air-tui-platform-posix` : les six symboles C derrière des enveloppes typées, et
  `PosixTerminal` qui implémente le contrat.
- Codecs : `AirSgrEncoder`, l'écriture décimale et le **décodeur ANSI d'entrée**, tous à
  **100 % de couverture** (ADR-012).
- **M1 — décodeur ANSI d'entrée** (`air-tui/src/input/`) : caractères UTF-8, commandes C0,
  combinaisons avec la touche alternative, séquences CSI et SS3 avec modificateurs, souris
  SGR 1006 avec molette et déplacement, collages encadrés recollés à travers les lectures,
  événements de focus. Modèle d'événements conforme à l'ADR-003, `physical` optionnel et
  employé par aucun widget.
- `air-tui` : géométrie (`AirColumn`, `AirRow`, `AirPoint`, `AirSize`, `AirRectangle`),
  `AirCell`, `AirSurface` avec différentiel, `AirTerminal` en RAII, un thème minimal aux
  glyphes Borland, et l'exemple `frame`.
- Vérifié sur macOS `x86_64` : `cargo fmt --check` propre,
  `cargo clippy --workspace --all-targets -- -D warnings` propre, **94 tests** au vert,
  `cargo build --locked` OK, et l'exemple `frame` exécuté dans un pty rend bien le bureau
  hachuré, la barre de menu, la fenêtre à case `[■]` et la barre de statut, puis restaure
  le terminal (code de sortie 0).
- Couverture mesurée au 2026-10-10 : les cinq modules d'entrée et de codec sont à
  **100 %** de régions, de fonctions et de lignes (`input/ansi.rs`, `input/event.rs`,
  `input/scanning.rs`, `codec/sgr.rs`, `codec/decimal.rs`). Le cœur `terminal.rs` est à
  93,8 % — il était à 0 % avant la couture par traits.
- **Fuzzing du décodeur** : `cargo +nightly fuzz run ansi_decoder` — 494 565 exécutions en
  61 secondes, **aucun plantage**, 673 entrées retenues au corpus. Trois invariants
  vérifiés : les octets consommés ne dépassent jamais l'entrée, aucun reste décodable
  n'est laissé derrière, et **découper l'entrée en deux lectures produit la même suite
  d'événements** — la propriété qui compte sur un terminal, où une séquence arrive coupée
  sans prévenir.
- Divergence de plateforme **constatée et consignée** : le `poll` de macOS rend `POLLNVAL`
  sur `/dev/null` là où Linux rend `POLLIN`. C'est le contrôle du champ `revents` — et non
  le compte rendu par `poll` — qui l'a révélée au premier lancement. Les deux
  comportements sont couverts par un test sous `cfg`.

### Pas fait — et à ne pas annoncer comme fait

- Protocole clavier Kitty et repli `modifyOtherKeys` (M2). Sans eux, `Ctrl+Shift+S` reste
  indiscernable de `Ctrl+S` sur un pty — limite du codage historique, pas du code.
- Le double clic : il demande une horloge, donc il n'a pas sa place dans un décodeur pur.
- Tables Unicode (largeur d'affichage, segmentation en graphèmes).
- Arbre de vues, routage en trois phases, focus, ordre de profondeur, découpage.
- Tout widget au-delà du cadre dessiné par l'exemple.
- Moteur de thèmes complet, quantificateur de couleurs, thème Midnight Commander.
- Couche MVC.
- `redit`, `rmc`.
- Tout gate `cargo xtask` : `check-naming` (ADR-004), `check-versions` (ADR-009),
  `check-codec-coverage` (ADR-012). Les contrôles sont manuels.
- Toute CI. `main` n'exige donc aucun contrôle à la fusion, faute d'en avoir un à exiger.
- `air-tui-platform-air`, l'implémentation pour la cible `linux-air`.
- Les cibles `cargo-fuzz` des décodeurs à venir (Kitty, graphèmes Unicode).
- Le découpage par vue : `draw_text` découpe sur le bord de la **surface**, pas sur celui
  d'une fenêtre. Un texte trop long écrase donc le cadre — constaté sur l'exemple, à
  traiter avec l'arbre de vues (M4).
- La bordure par défaut des vues (ADR-010) : décidée, pas implémentée.
- Couverture de tests : loin de l'objectif de 100 %. Quelques tests unitaires seulement.
- **Vérification sur Linux** : le socle n'a été construit et exécuté que sur macOS x86_64.

## Prochaine tâche

**M2 — le protocole clavier Kitty**, tier-1 par ADR-003 : c'est le seul mécanisme qui livre
les modificateurs complets à travers un pty, donc en SSH et sur macOS. Négociation par
`CSI > 1 u`, décodage de `CSI … u`, et repli sur `modifyOtherKeys` niveau 2 pour xterm et
VTE. Ce sera un décodeur, donc 100 % de couverture et une cible de fuzzing.

Deux dettes à solder en parallèle : **vérifier le socle sur Linux** (noyau 6.1+, `x86_64`
et `aarch64`), seul trou de vérification du projet, et remonter la couverture de
`air-tui-platform-posix` par des tests sous pty.

Ensuite M1 : le parseur ANSI incrémental, avec sa cible `cargo-fuzz` dès le premier commit
du module (Principe 1 d'Air : fuzzing systématique sur toute API qui accepte des données
externes).

## Jalons

| Jalon | Contenu | État |
|---|---|---|
| M0 | socle : couche 0, surface, différentiel, rendu ANSI, exemple | fait (hors Linux) |
| M1 | décodeur ANSI, souris SGR, collage encadré, focus | **fait** |
| M2 | protocole clavier Kitty, repli `modifyOtherKeys` | à faire |
| M3 | tables Unicode : largeur et graphèmes | à faire |
| M4 | arbre de vues, trois phases, focus, découpage, widgets de base | à faire |
| M5 | moteur de thèmes, thème Turbo Vision, quantificateur | à faire |
| M6 | `redit` — équivalent de `tvedit` | à faire |
| M7 | thème et coquille Midnight Commander, importeur de skins | à faire |
| M8 | étage natif Linux : evdev, `TIOCLINUX`, gpm | à faire |
