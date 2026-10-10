# État d'avancement d'air-tui

Document de suivi **vivant** : où en est-on, quelle est la prochaine tâche. Il se met à
jour à chaque session. L'historique figé est dans [`JOURNAL.md`](JOURNAL.md).

**Dernière mise à jour : 2026-10-10.**

## Où en est-on

Jalon **M0 en cours**. Le dépôt est initialisé avec ses conventions, ses ADR fondateurs et
le socle technique minimal. Rien n'est utilisable comme framework à ce stade.

### Fait

- Conventions reprises d'Air et consignées (`docs/CONVENTIONS-fr.md`), dont la règle de
  nommage sans abréviation et son glossaire.
- ADR-001 à ADR-010 rédigés (français seulement, traduction anglaise à venir).
- Espace de travail à deux crates, lints d'arithmétique défensive en `deny`.
- `air-tui-sys` : les six symboles C, les enveloppes typées `terminal_window_size`,
  `enter_raw_mode` / `restore_mode`, `wait_for_input`, le type `Errno`.
- `air-tui` : géométrie (`AirColumn`, `AirRow`, `AirPoint`, `AirSize`, `AirRectangle`),
  `AirCell`, `AirSurface` avec différentiel, `AirTerminal` en RAII, un thème minimal aux
  glyphes Borland, et l'exemple `frame`.
- Vérifié sur macOS `x86_64` : `cargo fmt --check` propre,
  `cargo clippy --workspace --all-targets -- -D warnings` propre, **41 tests** au vert, et
  l'exemple `frame` exécuté dans un pty rend bien le bureau hachuré, la barre de menu, la
  fenêtre à case `[■]` et la barre de statut, puis restaure le terminal (code de sortie 0).
- Divergence de plateforme **constatée et consignée** : le `poll` de macOS rend `POLLNVAL`
  sur `/dev/null` là où Linux rend `POLLIN`. C'est le contrôle du champ `revents` — et non
  le compte rendu par `poll` — qui l'a révélée au premier lancement. Les deux
  comportements sont couverts par un test sous `cfg`.

### Pas fait — et à ne pas annoncer comme fait

- Parseur ANSI, souris, protocole clavier Kitty, `modifyOtherKeys`.
- Tables Unicode (largeur d'affichage, segmentation en graphèmes).
- Arbre de vues, routage en trois phases, focus, ordre de profondeur, découpage.
- Tout widget au-delà du cadre dessiné par l'exemple.
- Moteur de thèmes complet, quantificateur de couleurs, thème Midnight Commander.
- Couche MVC.
- `redit`, `airmc`.
- Tout gate `cargo xtask`, dont `check-naming` (ADR-004) et `check-versions` (ADR-009).
  Les contrôles sont manuels.
- Le découpage par vue : `draw_text` découpe sur le bord de la **surface**, pas sur celui
  d'une fenêtre. Un texte trop long écrase donc le cadre — constaté sur l'exemple, à
  traiter avec l'arbre de vues (M4).
- La bordure par défaut des vues (ADR-010) : décidée, pas implémentée.
- Couverture de tests : loin de l'objectif de 100 %. Quelques tests unitaires seulement.
- **Vérification sur Linux** : le socle n'a été construit et exécuté que sur macOS x86_64.

## Prochaine tâche

Terminer M0 : vérifier le socle sur Linux (noyau 6.1+, x86_64 et aarch64), puis écrire les
premiers tests de couverture sur `AirSurface` et la géométrie.

Ensuite M1 : le parseur ANSI incrémental, avec sa cible `cargo-fuzz` dès le premier commit
du module (Principe 1 d'Air : fuzzing systématique sur toute API qui accepte des données
externes).

## Jalons

| Jalon | Contenu | État |
|---|---|---|
| M0 | socle : couche 0, surface, différentiel, rendu ANSI, exemple | en cours |
| M1 | parseur ANSI, souris SGR, collage encadré, redimensionnement | à faire |
| M2 | protocole clavier Kitty, repli `modifyOtherKeys` | à faire |
| M3 | tables Unicode : largeur et graphèmes | à faire |
| M4 | arbre de vues, trois phases, focus, découpage, widgets de base | à faire |
| M5 | moteur de thèmes, thème Turbo Vision, quantificateur | à faire |
| M6 | `redit` — équivalent de `tvedit` | à faire |
| M7 | thème et coquille Midnight Commander, importeur de skins | à faire |
| M8 | étage natif Linux : evdev, `TIOCLINUX`, gpm | à faire |
