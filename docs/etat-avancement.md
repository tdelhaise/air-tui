# État d'avancement d'air-tui

Document de suivi **vivant** : où en est-on, quelle est la prochaine tâche. Il se met à
jour à chaque session. L'historique figé est dans [`JOURNAL.md`](JOURNAL.md).

**Dernière mise à jour : 2026-10-10** (jalon M6 — `redit`, la couche MVC, la hiérarchie
d'objets, et la segmentation en mots).

## Dépôt

<https://github.com/tdelhaise/air-tui> — public, MPL-2.0, branche `main` protégée : toute
modification passe par une pull request, y compris pour les administrateurs. Les commits
sont signés par la clef `5C6F8A2E20E58061` (`thierry.delhaise@air-desktop.org`).

**Aucune CI n'existe à ce jour**, donc aucun contrôle n'est requis à la fusion : les
vérifications de `CLAUDE.md` sont à exécuter en local avant d'ouvrir une PR.

## Où en est-on

Jalons **M0 à M6 terminés**. Le dépôt porte ses conventions, ses ADR fondateurs, le socle
de rendu, le décodeur d'entrée, l'arbre de vues, onze widgets, la **couche MVC** de
l'ADR-007, la hiérarchie d'objets complète, et une **application** : `redit` ouvre un
fichier, l'édite et l'enregistre.

Ce qui manque pour que ce soit un framework complet : la **sélection** de texte — donc le
copier-coller interne —, les listes et les cases à cocher, le thème Midnight Commander et
la coquille `rmc` (M7), et l'étage d'entrée natif Linux (M8).

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
- **M5 — moteur de thèmes et premiers widgets** :
  - **palette transcrite de Borland**, pas approchée : les valeurs viennent de
    `cpAppColor` et `cpGrayDialog` de `magiblot/tvision`, chaîne par chaîne, avec l'octet
    d'origine en commentaire. Vingt-huit rôles.
  - **deux corrections de fidélité** que la transcription a révélées : les indices VGA ne
    sont pas les indices ANSI — le bleu et le rouge y sont échangés —, et la barre de
    statut est noir sur gris clair comme le menu, pas sur cyan comme les premières valeurs
    de ce projet le disaient.
  - **quantificateur** de couleurs, requis par ADR-008 : vraies couleurs, deux cent
    cinquante-six, seize. Déduit de `COLORTERM` et `TERM`, forçable, appliqué au dernier
    moment dans le rendu. Idempotent, et testé comme tel.
  - **`AirWindow`** avec sa case de fermeture `[■]` cliquable, en cadre simple ou double, et
    **`AirButton`** avec sa lettre d'accès rapide en notation `~X~`, activable au clic, par
    `Entrée`/`Espace` au focus, ou par `Alt`+lettre de loin.
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
- **Le défaut bloquant de macOS corrigé** : `poll` sur un descripteur ouvert sur
  `/dev/tty` y rend `POLLNVAL`, donc `wait_for_input` échouait au premier appel et toute
  application sortait aussitôt. `PosixTerminal` choisit désormais chaque bout séparément —
  le descripteur standard quand c'en est un terminal, `/dev/tty` sinon — ce qui est aussi
  ce que font ncurses et Turbo Vision. Vérifié dans un pseudo-terminal sur les deux
  plateformes, essai de contrôle compris.
- **`tools/pty-drive.py`** : le harnais qui a trouvé ce défaut, versionné pour que la
  vérification soit reproductible. Un essai manuel non reproductible est un essai perdu.
- **M6 — `redit`, et les quatre pièces que l'éditeur a demandées** :
  - **la couche modèle de l'ADR-007** (`air-tui/src/model/`) : `AirModel` à une seule
    méthode, `AirChangeSet` pour les faits **structurels**, `AirGeneration` pour l'état
    **continu**, et `AirTextBuffer` — vingt-deux actions qui sont l'inventaire exhaustif
    de ce qui peut arriver au texte. Le tampon ne lit ni n'écrit aucun fichier et ignore
    qu'il est affiché : ses tests tournent sans terminal.
  - **`AirEditorView`**, la vue qui ne tient **pas** son modèle : ni `&`, ni `Rc`, ni
    pointeur. Elle garde un reflet que le contrôleur lui pousse, et accumule des
    intentions qu'il relève. Les compteurs de génération y font un travail mesurable —
    `synchronize` compare quatre valeurs et sort, la plupart du temps.
  - **`AirApplication`, `AirScreen`, `AirDesktop`** : la hiérarchie que
    `ARCHITECTURE-fr.md` annonçait comme cible depuis le premier jour. L'écran range ses
    quatre pièces lui-même, dans le seul ordre qui marche — la contrainte « la barre de
    menu en dernier » ne tient plus par la documentation. Il porte la disposition, impose
    les phases, et porte la **modalité**.
  - **`AirScrollBar` et `AirPromptDialog`** : l'ascenseur ne défile rien, il **demande**
    une position ; le dialogue pose une question et rend une réponse, en modal.
  - **`redit`** (`crates/redit/`) : les quatre mécanismes de l'ADR-007 en service dans une
    vraie application, `Document` qui porte le disque que le modèle refuse de porter, et
    une ligne de commande à trois formes.
- **La segmentation en mots de l'annexe 29**, la pièce de M3 qui attendait ce jalon :
  règles `WB1` à `WB999`, repliement de `WB4`, regards en arrière de `WB7`, `WB7c` et
  `WB11`. C'est ce qui garde `3,14` et `can't` entiers là où une coupure sur la ponctuation
  les casserait, et ce qui découpe `日本語` en trois unités sans un seul espace.
- **`cargo xtask check-segmentation`**, et ce qu'il a trouvé. Il confronte les **deux**
  segmentations aux fichiers d'essais du Consortium : `GraphemeBreakTest.txt` 853 cas,
  `WordBreakTest.txt` 1944 cas, aucun écart aujourd'hui. Au premier lancement, il
  rapportait **dix écarts** sur la segmentation en grappes du jalon M3, annoncée comme
  faite : la révision 49 de l'annexe, livrée avec Unicode 18.0.0, a réécrit `GB9c` et
  retiré l'exigence d'une consonne **avant** le lieur. Les tables venaient déjà de 18.0.0,
  les règles non.
- **Une erreur de conception corrigée en cours de jalon** : `AirApplication` possédait
  d'abord l'arbre et une pile d'écrans. C'était intenable — une fois la vue d'édition
  confiée à l'application, plus personne ne pouvait la synchroniser. La racine est
  désormais **passée en paramètre**, et la pile appartient à l'appelant. `AirView` hérite
  de `Any` pour que `find_mut` rende une vue avec son type (ADR-013).
- **M5b — les trois widgets qui demandaient une mécanique de plus** :
  - **`AirMenuBar`** à menus déroulants, avec `AirMenu` et `AirMenuItem`, séparateurs
    compris. Ouverture par `Alt`+lettre ou au clic, flèches verticales qui **sautent les
    séparateurs**, flèches horizontales qui changent de menu en anneau, `Échap` qui
    referme, `Entrée` qui émet. En **pré-traitement**.
  - **`AirStatusLine`** et ses `AirStatusItem` : des raccourcis publiés, activables à la
    touche et au clic. En **post-traitement**, pour ne pas voler une touche de fonction à
    la vue qui a le focus.
  - **`AirInputLine`** : saisie, suppression et déplacement **par grappes de graphème** —
    un `é` décomposé ou un émoji à jointures s'effacent d'une touche. Défilement
    horizontal à invariant « le curseur est toujours visible », clic qui positionne le
    curseur en tenant compte des glyphes larges et du défilement.
- **La surimpression annoncée pour M5b n'était pas nécessaire**, et c'est la découverte du
  jalon. Le dessin parcourt les enfants de l'arrière vers l'avant et le routage positionnel
  de l'avant vers l'arrière : une vue au premier plan recouvre déjà ses sœurs et reçoit
  déjà les clics avant elles. La barre de menu **grandit** quand elle s'ouvre, son menu
  reste dans ses propres limites, et le découpage par vue ne souffre aucune exception. La
  contrainte qui en découle est réelle et documentée : la barre doit être insérée **en
  dernier**.
- **Le curseur matériel**, qui manquait et sans lequel aucun champ de saisie n'est
  utilisable. `AirView::cursor` répond en coordonnées de contenu, `draw_view` rend la
  position **absolue** du curseur de la vue focalisée la plus profonde, et
  `AirTerminal::set_cursor` le pose après les cellules. Une vue non focalisée n'en publie
  pas, et un curseur hors du découpage n'en est pas un.
- **`AirDrawContext`**, qui corrige un défaut de fond de M5 : `ARCHITECTURE-fr.md`
  affirmait que chaque vue interroge le jeu de commandes **au moment où elle se dessine**,
  or le chemin de dessin ne recevait que le thème. Le grisage d'un item de menu reposait
  sur une fonction qui rendait toujours `true`. Le contexte de dessin porte désormais le
  thème **et** le jeu de commandes, et le grisage comme le refus d'activer lisent la même
  source — ils ne peuvent plus diverger.
- **Deux widgets de base** : `AirPanel`, conteneur bordé à titre, et `AirStaticText`, qui
  retire la bordure dans son constructeur comme le veut le corollaire d'ADR-010.
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
- Couverture mesurée au 2026-10-10, après M6 : **95,35 %** de régions sur tout l'espace de
  travail. Les six modules d'entrée et de codec restent à **100 %** de régions, de
  fonctions et de lignes (`input/ansi.rs`, `input/event.rs`, `input/scanning.rs`,
  `input/kitty.rs`, `codec/sgr.rs`, `codec/decimal.rs`) — c'est l'exigence d'ADR-012, et
  elle est tenue. Le reste d'`air-tui` est entre 97 et 100 % selon le module ; les trois
  points bas sont `terminal.rs` à 94,4 % (branches d'erreur de périphérique),
  `redit/src/main.rs` à 52,9 % (la boucle demande un terminal) et `xtask` entre 24 et 81 %
  (outillage hors exécution du framework).
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

### Défauts ouverts

*(aucun à ce jour)*

### Pas fait — et à ne pas annoncer comme fait

- Le double clic : il demande une horloge, donc il n'a pas sa place dans un décodeur pur.
- La table des touches fonctionnelles de Kitty au-delà des numéros certains : pavé
  numérique, touches multimédia, touches modificatrices seules. Hors table, donc consommées
  sans événement — et la table a été écrite d'après la spécification, **pas vérifiée contre
  un terminal réel**.
- `rmc`, et le **thème Midnight Commander** avec son importeur de skins (M7).
- Les **listes, cases à cocher et boutons radio**.
- La **sélection** de texte, dans la vue d'édition comme dans le champ de saisie :
  `Maj`+flèche, le copier-coller interne, la surbrillance, et le double-clic qui en
  dépend. Les deux n'ont qu'un curseur, pas d'étendue sélectionnée. C'est la plus grosse
  absence de `redit`.
- Dans `redit` : pas de **recherche**, pas d'**annulation**, pas de **tabulation** — elle
  demande de choisir des taquets et de les appliquer au rendu comme au calcul de colonne —,
  et pas de seconde fenêtre. Le bureau sait en tenir plusieurs, rien ne les crée.
- Le **glissement** du curseur d'un ascenseur : il demande de suivre un bouton maintenu
  entre deux événements, donc une capture de souris que l'arbre de vues n'a pas. Les
  flèches, les pages et la molette couvrent le reste.
- Le **parcours du focus** par `Tab` à l'intérieur d'un dialogue : il demanderait une
  chaîne de focus dans une vue sans enfants, donc un second mécanisme de focus à côté de
  celui de l'arbre.
- `AirBinding`, la part MVVM que l'ADR-007 borne aux formulaires. `AirPromptDialog`
  possède ses contrôles directement, ce que l'ADR prévoit ; un dialogue à dix champs
  validés la demanderait.
- Couverture de tests : **100 % tenu sur les codecs** (ADR-012) et élevé sur `air-tui`,
  mais `air-tui-platform-posix` demande un vrai terminal et reste bas.
- Les gates `check-naming` (ADR-004), `check-versions` (ADR-009) et
  `check-codec-coverage` (ADR-012). `cargo xtask` porte désormais **deux** commandes — la
  génération des tables Unicode et la conformité des segmentations — mais pas ces trois
  contrôles, qui restent manuels.
- La couverture de `xtask` : 34 % à 81 % selon le module. C'est de l'outillage hors
  exécution du framework, qu'ADR-012 ne couvre pas — mais ce n'est pas une raison de le
  laisser là.
- Toute CI. `main` n'exige donc aucun contrôle à la fusion, faute d'en avoir un à exiger.
- `air-tui-platform-air`, l'implémentation pour la cible `linux-air`.
- Les cibles `cargo-fuzz` des décodeurs à venir (Kitty, graphèmes Unicode).
- *(plus rien ici : la dette de vérification Linux est soldée, voir ci-dessous)*

## Prochaine tâche

**M7 — le thème Midnight Commander et la coquille `rmc`.** Le moteur de thèmes existe
depuis M5 et n'a encore qu'un thème : la preuve qu'il est bien un moteur reste à faire. Le
jalon demande une seconde palette, un second jeu de glyphes, l'importeur de skins MC en
`xtask`, et une coquille à deux panneaux — qui exercera `AirScreen` comme chrome
commutable, ce que `redit` n'exerce pas.

Il demandera probablement les **listes**, qu'un panneau de fichiers ne peut pas éviter, et
avec elles les ascenseurs qu'elles pilotent.

Une dette restante : remonter la couverture de `air-tui-platform-posix` par des tests sous
pty. Et deux vérifications encore absentes : la **console Linux native**, inaccessible par
SSH et dont dépend tout l'étage natif du jalon M8 ; et le protocole Kitty **contre un vrai
terminal** — il est décodé et négocié d'après la spécification, et les tests le pilotent
avec un périphérique en mémoire, ce qui prouve le code mais pas la table des numéros de
touches fonctionnelles.

## Jalons

| Jalon | Contenu | État |
|---|---|---|
| M0 | socle : couche 0, surface, différentiel, rendu ANSI, exemple | **fait** |
| M1 | décodeur ANSI, souris SGR, collage encadré, focus | **fait** |
| M2 | protocole clavier Kitty, repli `modifyOtherKeys` | **fait** |
| M3 | tables Unicode : largeur, graphèmes, et mots au jalon M6 | **fait** |
| M4 | arbre de vues, trois phases, focus, découpage | **fait** |
| M5 | moteur de thèmes, quantificateur, fenêtre et bouton | **fait** |
| M5b | barre de menu à menus déroulants, barre de statut, champ de saisie, curseur | **fait** |
| M6 | `redit` — équivalent de `tvedit`, couche MVC, hiérarchie d'objets | **fait** |
| M7 | thème et coquille Midnight Commander, importeur de skins | à faire |
| M8 | étage natif Linux : evdev, `TIOCLINUX`, gpm | à faire |
