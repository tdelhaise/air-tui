# État d'avancement d'air-tui

Document de suivi **vivant** : où en est-on, quelle est la prochaine tâche. Il se met à
jour à chaque session. L'historique figé est dans [`JOURNAL.md`](JOURNAL.md).

**Dernière mise à jour : 2026-10-10** (jalon M4 — arbre de vues).

## Dépôt

<https://github.com/tdelhaise/air-tui> — public, MPL-2.0, branche `main` protégée : toute
modification passe par une pull request, y compris pour les administrateurs. Les commits
sont signés par la clef `5C6F8A2E20E58061` (`thierry.delhaise@air-desktop.org`).

**Aucune CI n'existe à ce jour**, donc aucun contrôle n'est requis à la fusion : les
vérifications de `CLAUDE.md` sont à exécuter en local avant d'ouvrir une PR.

## Où en est-on

Jalons **M0 à M4 terminés**. Le dépôt porte ses conventions, ses
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
- **M4 — arbre de vues** (`air-tui/src/view/`) : trait `AirView` et composition
  `AirViewState`, `AirPainter` à coordonnées relatives au propriétaire et **découpage
  réel**, routage en trois phases plus routage positionnel de la souris avec traduction
  des coordonnées, focus exclusif entre sœurs, ordre de profondeur, bordure par défaut
  d'ADR-010 avec `content_bounds`, et titre de cadre dessiné par la traversée.
- **Le défaut du jalon M0 est devenu impossible** : un contenu trop long ne peut plus
  écraser le cadre qui l'entoure, et l'exemple `frame` en fait la démonstration en
  insérant une étiquette volontairement plus large que sa fenêtre. Les bordures restent
  alignées aux mêmes colonnes sur toutes les lignes, texte accentué compris.
- **`AirCommandSet`** : le mécanisme 3 de l'ADR-007, en *pull*. Griser une commande grise
  d'un coup tout ce qui la porte, sans que personne soit notifié.
- **Deux widgets, pour prouver l'arbre** : `AirPanel`, conteneur bordé à titre, et
  `AirStaticText`, qui retire la bordure dans son constructeur comme le veut le corollaire
  d'ADR-010. Le jeu complet vient avec M5.
- **Une duplication supprimée** : le dessin de texte existait en deux endroits, sur la
  surface et dans le peintre. `AirSurface::draw_text` délègue désormais au peintre — deux
  implémentations de la même règle garantissaient qu'elles divergent.
- **M3 — Unicode** (`air-tui/src/unicode/`, tables générées en Unicode **18.0.0**) :
  largeur d'affichage et segmentation en graphèmes, règles `GB3` à `GB13` de l'annexe 29
  **y compris `GB9c`**, la coupure conjointe indienne. Sept tables, 2546 plages, 113 Ko de
  source générée par `cargo xtask generate-unicode-tables`, régénérable et vérifiable par
  `--check`.
- **Le rendu s'en sert** : `AirSurface::draw_text` parcourt les grappes et pose une
  **cellule de continuation** après une grappe large, que le rendu saute. Un texte
  idéographique n'écrase plus la bordure qui le suit — c'est le défaut que ces tables
  existent pour empêcher, et il a son test.
- **M2 — protocole clavier Kitty** (`air-tui/src/input/kitty.rs`) : décodage des frappes
  `CSI … u` avec modificateurs complets, types d'événement — enfoncement, répétition,
  **relâchement** — et **touche de la disposition de base**, exposée par
  `AirKeyEvent::base_layout` et `shortcut_key()`. Repli `modifyOtherKeys` niveau 2.
  Négociation par interrogation `CSI ? u` suivie d'un jalon d'attributs de périphérique,
  donc **sans dépendre d'un délai** ; protocole défait à la destruction du terminal.
  Paramètres élargis en `u32`, sans quoi un emoji en numéro de touche était refusé à tort.
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
- **Génération reproductible sur les trois plateformes** : macOS `x86_64`, hydrogen et
  neon ont chacune retéléchargé le référentiel et régénéré les tables ; le fichier
  committé correspond **à l'octet** dans les trois cas
  (`cargo xtask generate-unicode-tables --check`). C'est la vérification concrète de ce
  qu'ADR-025 promet.
- **Vérifié sur `aarch64`** (neon — Ubuntu 26.04, noyau 7.0.0-1020-raspi, Raspberry Pi) :
  249 tests au vert et l'exemple `frame` rendant une grille identique à celle des deux
  autres plateformes. C'était le contrôle décisif : la déclaration **variadique**
  d'`ioctl` est juste sur l'ABI aarch64, où les arguments variadiques passent par la pile
  alors que les arguments fixes passent par registre. Un `ioctl` déclaré non-variadique
  aurait fonctionné sur `x86_64` et cassé silencieusement ici.
- **Vérifié sur Linux** (hydrogen — Ubuntu 26.04.1 LTS, noyau 7.0.0-38, `x86_64`) :
  `fmt`, `clippy -D warnings`, **188 tests**, `build --locked` et `deny check` propres ;
  couverture globale **95,0 %** de régions, et les cinq modules d'entrée et de codec
  toujours à 100 % ; l'exemple `frame` rend une grille identique au pixel à celle de
  macOS, en 80×25, avec les accents intacts. Les trois valeurs propres à Linux sont donc
  justes : `TIOCGWINSZ = 0x5413`, `nfds_t` sur 64 bits, et `poll` qui rend `POLLIN` sur
  `/dev/null`.
- **Un défaut trouvé par cette vérification, et invisible depuis macOS** : un import ne
  servant qu'à un test gardé par `cfg(target_os = "macos")` faisait échouer
  `clippy -D warnings` sur Linux. Corrigé ; `air-tui-platform-posix` passe en 0.1.1.
- **Fuzzing du décodeur** : `cargo +nightly fuzz run ansi_decoder` — 494 565 exécutions en
  61 secondes, **aucun plantage**, 673 entrées retenues au corpus. Trois invariants
  vérifiés : les octets consommés ne dépassent jamais l'entrée, aucun reste décodable
  n'est laissé derrière, et **découper l'entrée en deux lectures produit la même suite
  d'événements** — la propriété qui compte sur un terminal, où une séquence arrive coupée
  sans prévenir. Rejoué sur Linux : 371 229 exécutions de plus, aucun plantage non plus,
  soit près de 866 000 exécutions cumulées sur les deux plateformes.
- Divergence de plateforme **constatée et consignée** : le `poll` de macOS rend `POLLNVAL`
  sur `/dev/null` là où Linux rend `POLLIN`. C'est le contrôle du champ `revents` — et non
  le compte rendu par `poll` — qui l'a révélée au premier lancement. Les deux
  comportements sont couverts par un test sous `cfg`.

### Pas fait — et à ne pas annoncer comme fait

- Le double clic : il demande une horloge, donc il n'a pas sa place dans un décodeur pur.
- La table des touches fonctionnelles de Kitty au-delà des numéros certains : pavé
  numérique, touches multimédia, touches modificatrices seules. Hors table, donc consommées
  sans événement — et la table a été écrite d'après la spécification, **pas vérifiée contre
  un terminal réel**.
- Tables Unicode (largeur d'affichage, segmentation en graphèmes).
- Arbre de vues, routage en trois phases, focus, ordre de profondeur, découpage.
- Tout widget au-delà du cadre dessiné par l'exemple.
- Moteur de thèmes complet, quantificateur de couleurs, thème Midnight Commander.
- Couche MVC.
- `redit`, `rmc`.
- La couche MVC d'ADR-007 au-delà du mécanisme 3 : le journal de changements et les
  compteurs de génération attendent l'éditeur du jalon M6.
- Les gates `check-naming` (ADR-004), `check-versions` (ADR-009) et
  `check-codec-coverage` (ADR-012). `cargo xtask` **existe** depuis M3, mais ne porte que
  la génération des tables Unicode ; ces trois contrôles restent manuels.
- La **segmentation en mots** (annexe 29) : reportée au jalon M6, quand l'éditeur en aura
  besoin pour le déplacement par mot.
- La couverture de `xtask` : 34 % à 81 % selon le module. C'est de l'outillage hors
  exécution du framework, qu'ADR-012 ne couvre pas — mais ce n'est pas une raison de le
  laisser là.
- Toute CI. `main` n'exige donc aucun contrôle à la fusion, faute d'en avoir un à exiger.
- `air-tui-platform-air`, l'implémentation pour la cible `linux-air`.
- Les cibles `cargo-fuzz` des décodeurs à venir (Kitty, graphèmes Unicode).
- Le découpage par vue : `draw_text` découpe sur le bord de la **surface**, pas sur celui
  d'une fenêtre. Un texte trop long écrase donc le cadre — constaté sur l'exemple, à
  traiter avec l'arbre de vues (M4).
- La bordure par défaut des vues (ADR-010) : décidée, pas implémentée.
- Couverture de tests : loin de l'objectif de 100 %. Quelques tests unitaires seulement.
- *(plus rien ici : la dette de vérification Linux est soldée, voir ci-dessous)*

## Prochaine tâche

**M5 — les widgets et le moteur de thèmes**, recalés ensemble parce qu'ils se tiennent :
un bouton, un champ de saisie, une barre de menu et une barre de statut ont besoin de rôles
de couleur que la palette actuelle n'a pas, et le quantificateur qui ramène un thème en RGB
vers les seize couleurs d'une console Linux conditionne leur lisibilité. S'y ajoutent
`AirWindow` et sa case de fermeture `[■]`, absente de l'exemple depuis que celui-ci est
bâti sur l'arbre.

Une dette restante : remonter la couverture de `air-tui-platform-posix` par des tests sous
pty. Et deux vérifications encore absentes : la **console Linux native**, inaccessible par
SSH et dont dépend tout l'étage natif du jalon M8 ; et le protocole Kitty **contre un vrai
terminal** — il est décodé et négocié d'après la spécification, et les tests le pilotent
avec un périphérique en mémoire, ce qui prouve le code mais pas la table des numéros de
touches fonctionnelles.

Ensuite M1 : le parseur ANSI incrémental, avec sa cible `cargo-fuzz` dès le premier commit
du module (Principe 1 d'Air : fuzzing systématique sur toute API qui accepte des données
externes).

## Jalons

| Jalon | Contenu | État |
|---|---|---|
| M0 | socle : couche 0, surface, différentiel, rendu ANSI, exemple | **fait** |
| M1 | décodeur ANSI, souris SGR, collage encadré, focus | **fait** |
| M2 | protocole clavier Kitty, repli `modifyOtherKeys` | **fait** |
| M3 | tables Unicode : largeur et graphèmes | **fait** |
| M4 | arbre de vues, trois phases, focus, découpage | **fait** |
| M5 | widgets de base, moteur de thèmes, quantificateur de couleurs | à faire |
| M6 | `redit` — équivalent de `tvedit` | à faire |
| M7 | thème et coquille Midnight Commander, importeur de skins | à faire |
| M8 | étage natif Linux : evdev, `TIOCLINUX`, gpm | à faire |
