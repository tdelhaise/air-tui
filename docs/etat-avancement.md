# État d'avancement d'air-tui

Document de suivi **vivant** : où en est-on, quelle est la prochaine tâche. Il se met à
jour à chaque session. L'historique figé est dans [`JOURNAL.md`](JOURNAL.md).

**Dernière mise à jour : 2026-10-10** (jalon M7i — plusieurs skins compilés ; les deux
derniers gates de conventions ; jalon M7h — l'annulation dans `redit`).

## Dépôt

<https://github.com/tdelhaise/air-tui> — public, MPL-2.0, branche `main` protégée : toute
modification passe par une pull request, y compris pour les administrateurs. Les commits
sont signés par la clef `5C6F8A2E20E58061` (`thierry.delhaise@air-desktop.org`).

Une **intégration continue** existe depuis M6b (`.github/workflows/ci.yml`) : sur chaque
PR, `fmt`, `clippy -D warnings`, les essais, `build --locked`, `check-naming`,
`check-versions` sur Linux `x86_64` et macOS `aarch64`, plus `deny check`,
`check-codec-coverage`, les tables Unicode à l'octet et la conformité des segmentations.

Ces trois contrôles sont **exigés à la fusion** : `main` les porte en
`required_status_checks`, et `enforce_admins` est actif, donc la règle vaut aussi pour
l'auteur du dépôt. `strict` est à `false` délibérément — une branche n'a pas à être
rebasée sur `main` pour être fusionnée, seulement à voir ses contrôles passer.

Elle ne remplace pas la liste locale de `CLAUDE.md` : un coureur n'a pas de terminal de
contrôle, donc **aucun essai en pseudo-terminal** n'y tourne, et le fuzzing exige nightly.

## Où en est-on

Jalons **M0 à M7i terminés**. Le dépôt porte ses conventions, ses ADR fondateurs, le socle
de rendu, le décodeur d'entrée, l'arbre de vues, treize widgets, la **couche MVC** de
l'ADR-007, la hiérarchie d'objets complète, deux thèmes dont un **compilé**, et deux
**applications** : `redit` ouvre un fichier, l'édite et l'enregistre ; `rmc` parcourt deux
arborescences côte à côte.

Depuis M7g, les **trente-neuf** skins livrés avec `mc` 4.8.33 sont importables. Ils
étaient neuf.

Ce qui manque pour que ce soit un framework complet : l'étage d'entrée natif Linux (M8),
une liste à plusieurs colonnes, et le **défilement d'un groupe** d'items plus long que sa
place.

### Fait

- **M7b — les listes, et la coquille `rmc` à deux panneaux** :
  - **Quatre rôles de liste dans la palette** — `ListNormal`, `ListSelected`,
    `ListMarked`, `ListMarkedSelected` —, première extension depuis qu'un second thème
    existe. C'est ce qui a éprouvé le moteur de thèmes plutôt que de le décrire : ajouter
    un rôle demande une valeur dans chacun des deux thèmes, et celle du thème compilé vient
    d'une clef du skin que la table d'`xtask` désigne, non d'une valeur écrite à la main
    dans l'artefact. `mc` nomme les quatre (`[core] _default_`, `selected`, `marked`,
    `markselect`) ; Turbo Vision n'en nomme que trois dans `cpListViewer`, et le quatrième
    est composé.
  - **Deux pièges de transcription, écrits plutôt que subis** : Borland appelle *Focused*
    l'item sous le curseur et *Selected* un item marqué, là où air-tui emploie `Selected`
    pour ce qui est sous le curseur — comme `MenuSelected` le fait depuis M5 ; et le
    décompte des indices de `cpAppColor` ne se fait pas à la main. C'est
    `examples/tvdemo/tvdemo2.cpp` qui nomme ses propres entrées — « List normal » pour 57,
    « List focused » pour 58, « List selected » pour 59 —, et les six entrées des boutons
    transcrites à M5 portent les mêmes numéros que ce fichier leur donne. Le décompte est
    donc validé par une source indépendante du calcul.
  - **`AirListBox`** tient son curseur et son défilement, mais ne marque rien : elle
    produit une `AirListRequest::ToggleMark` que le contrôleur applique, et reçoit en
    retour l'état de marquage. Marquer un fichier est un fait que l'application garde, pas
    une affaire d'affichage. Elle ne connaît pas son ascenseur et l'ascenseur ne la connaît
    pas : le contrôleur les accorde en deux lignes, et un essai les exécute pour que la
    documentation du module ne soit pas une promesse.
  - **`rmc`** : deux panneaux, `Tab` pour passer de l'un à l'autre, `Entrée` pour
    descendre, `Inser` pour marquer, `F5` pour relire, `F10` pour sortir. Thème par défaut
    celui de `mc`, `--theme tv` pour la même coquille aux couleurs de Turbo Vision. Les
    deux moitiés que l'ADR-008 sépare existent donc séparément, et la séparation n'est plus
    une affirmation de l'ADR : c'est une ligne de commande.
  - **Trois défauts que les essais ont trouvés**, et qu'une relecture n'aurait pas vus : le
    marquage ne faisait pas avancer la génération du modèle, donc la marque était comptée
    dans le titre et invisible dans la liste ; poser le curseur sur le répertoire d'où l'on
    remonte ne peut pas se faire au moment de la commande, puisque la liste reflète encore
    le répertoire précédent — c'est devenu une intention, appliquée après le reflet ; et le
    curseur d'ascenseur empruntait `close_box_mark`, ce qui mettait un `X` au milieu de la
    glissière dans le thème de `mc`, dont le skin ferme ses fenêtres par un `X`. Le jeu de
    glyphes a donc gagné `scroll_thumb`. **Seul un second thème pouvait faire apparaître ce
    dernier** : les deux glyphes coïncident dans Turbo Vision.
  - **Mesure des trente-neuf skins : dix importables**, contre douze à M7a. Les deux perdus
    posent `bold` sur `[core] marked`, une clef que la table n'employait pas avant les
    rôles de liste : le compte baisse parce que le modèle s'est enrichi.
  - Vérifié en pseudo-terminal sur les **octets émis** : `rmc` sort sur `F10` (`exit:0`) et
    tient sans elle (`timeout`) ; `Tab`, flèche bas puis `Inser` donnent
    `38;5;11;48;5;6` — jaune sur cyan, l'item marqué sous le curseur du skin de `mc` — et
    le titre du panneau affiche « 1 marqué ».

- **M7a — le thème Midnight Commander, et l'importeur qui le produit** :
  - **`cargo xtask import-mc-skin`** compile un `.ini` de skin en artefact Rust committé.
    L'ADR-008 l'exige ainsi : « les thèmes sont compilés, pas analysés au runtime ».
    Conséquence vérifiable plutôt que promise — `air-tui` n'embarque aucun analyseur de
    `.ini` et n'ouvre aucun fichier pour afficher un thème.
  - **La table de correspondance est annotée** : vingt-cinq rôles transcrits d'une seule
    clef du skin, trois composés de deux clefs faute d'être nommés par `mc`, et quatorze
    correspondances qui portent écrite la règle ayant demandé un choix. `mc` nomme ses
    couleurs par élément d'interface, air-tui par rôle sémantique : les deux ensembles ne
    se recouvrent pas entièrement, et ce qui ne se recouvre pas est écrit.
  - **Deux écarts entre le manuel de `mc` et les fichiers qu'il livre**, tranchés en faveur
    des fichiers : le manuel nomme les jonctions `centertop`/`centerbottom`/`centermiddle`
    là où les 39 skins écrivent `topmiddle`/`bottommiddle`/`cross`, et il annonce `color16`
    à `color255` là où les skins emploient aussi `color0` à `color15`, plus de deux mille
    fois.
  - **Un défaut de mon analyseur trouvé par la mesure, pas par la relecture** : je
    traitais `#` comme un début de commentaire, et les 39 skins portent 192 valeurs
    contenant un `#` — toutes des vraies couleurs, aucune un commentaire. Quatre skins se
    vidaient donc de leurs couleurs en annonçant « «  » n'est pas une couleur connue ».
    Corrigé, et `AirColor::Rgb` les exprime.
  - **Mesuré sur les 39 skins livrés : douze sont importables.** Les vingt-sept autres sont
    refusés pour deux limites du modèle d'air-tui, chiffrées donc décidables : dix-huit
    posent un **attribut** sur un rôle employé, que `AirColorPair` ne porte pas ; neuf
    emploient `default`, le fond transparent de `mc`, qu'`AirColor` ne sait pas exprimer.
  - **Le décodeur de skins est à 100 % de régions, de fonctions et de lignes**, et inscrit
    au gate `check-codec-coverage` — septième codec. La règle a eu l'effet qu'ADR-012
    annonce : les branches défensives inatteignables ont été **supprimées** par le typage,
    pas couvertes. Les trois chiffres d'une couleur du cube et les six d'une vraie couleur
    se reconnaissent par motif de tranche, et la valeur d'un chiffre hexadécimal vient
    d'une table plutôt que d'un `to_digit` suivi d'une conversion infaillible.
  - **`--theme mc` dans `redit` et `mc` dans l'exemple `frame`** : les deux lancements
    dessinent exactement les mêmes vues. Vérifié sur les **octets émis** dans un
    pseudo-terminal — la barre de menu passe de `38;5;0;48;5;7` à `38;5;15;48;5;6`, la
    barre de statut de `48;5;7` à `48;5;6`, et le bureau du `░` à l'espace.
  - **Une empreinte du skin dans l'artefact**, pour que `--check` distingue deux échecs que
    le même symptôme recouvre : « ce n'est pas le même skin » et « l'artefact a dérivé de
    son skin ».
- **M6b — les trois contrôles de conventions, et l'intégration continue qui les exige** :
  - **`cargo xtask check-naming`** (ADR-004) : un balayage lexical qui écarte
    commentaires, chaînes et littéraux, relève les **sites de déclaration** et refuse le
    glossaire des abréviations. Les quatre exceptions sont tenues : les noms imposés par
    `std`, et les corps de bloc `extern "C"` exemptés entiers. 6866 noms inspectés dans 64
    fichiers, **aucune abréviation** — la règle tenue à la main l'était bien.
  - **`cargo xtask check-versions`** (ADR-009) : les crates touchées sur la plage, l'arbre
    de travail joint à la plage, et la **cohérence des dépendances par chemin**. Ce dernier
    contrôle attrape ce que cargo laisse passer : une version déclarée qui a dérivé sans
    devenir incompatible.
  - **`cargo xtask check-codec-coverage`** (ADR-012) : les six codecs à 100 % de régions,
    de fonctions **et** de lignes. Un codec absent du relevé est un manquement, pas un
    succès — sans quoi un fichier renommé sortirait de la règle en silence.
  - Les trois sont **éprouvés sur un refus**, pas seulement sur un succès : un gate qui dit
    toujours oui est pire qu'un gate absent. 49 essais nouveaux, dont ceux des deux pièges
    de la lecture par sous-chaîne (`rust-version` qui répondrait pour `version`,
    `notcovered` qui répondrait pour `covered`).
  - **Ce que la mesure a tranché, et qui est écrit dans le gate** : deux modules nommés
    `codec` ne sont **pas** soumis à la règle. `codec/mod.rs` est une façade sans une ligne
    exécutable, que `llvm-cov` ne relève pas du tout. `air-tui-platform/src/codec.rs` porte
    le contrat — deux traits sans méthode par défaut — et ses six régions manquantes sur 90
    sont les fermetures de deux essais où le décodeur ne doit **rien** produire, donc où la
    fermeture ne peut pas être appelée.
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
- **M6 vérifié sur les trois machines** : macOS `aarch64`, hydrogen (`x86_64`, Ubuntu
  26.04, noyau 7.0) et neon (`aarch64`, Ubuntu 26.04, Raspberry Pi). Sur chacune :
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  **845 tests** au vert, `cargo deny check`, `generate-unicode-tables --check` octet pour
  octet, `check-segmentation` sans écart, et les **quatre essais en pseudo-terminal** —
  `frame` et `redit`, chacun avec son essai de contrôle. L'enregistrement de `redit` est
  vérifié sur le contenu du fichier, pas sur le code de sortie : les trois machines
  rendent `ABCorigine`.
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

### Fait au jalon M7c — le type d'événement de touche

Le jalon M7b avait consigné un défaut : « le filtrage des relâchements de touche n'est fait
que par `AirListBox` ». En l'instruisant, il s'est avéré **plus large que décrit**, et deux
affirmations de M7b se sont révélées fausses. Elles sont corrigées ici plutôt que tues.

- **Le décodeur ignorait le type d'événement sur l'encodage historique.** Le protocole
  Kitty place le type en sous-paramètre du champ des modificateurs, et cette règle vaut
  pour *toutes* les formes. Le décodeur ne la lisait que sur la forme `CSI … u` : le type
  était donc décodé là où il ne sert qu'aux caractères, et **ignoré là où il désigne les
  touches dont les widgets vivent** — flèches, `Début`, `Fin`, `Inser`, `Suppr`, `F1` à
  `F20`. Mesuré avant correction : `\x1b[;1:3B` — un relâchement de flèche bas — ressortait
  en **enfoncement**.
- **Donc le filtre que portait `AirListBox` ne protégeait presque rien**, et son
  commentaire — « les traiter ferait descendre le curseur de deux items par appui sur la
  flèche » — décrivait un effet qu'il ne prévenait pas : la flèche relâchée descendait le
  curseur malgré lui, puisqu'elle n'arrivait jamais marquée comme un relâchement. Le
  commentaire a été réécrit, et le filtre retiré du widget.
- **La forme réellement émise omet le numéro de touche.** `kitty` écrit `CSI ; 1 : 3 B`, le
  champ vide, parce que la spécification autorise à taire le numéro quand il vaut un.
  Vérifié dans `kitty/key_encoding.py`, qui n'écrit le numéro que s'il diffère de un ; la
  page du protocole ne donne que la règle générale et **aucun exemple de flèche**, ce qui
  aurait laissé une inférence là où il fallait une source.
- **Les relâchements sont écartés à la distribution**, en un seul endroit
  (`view::dispatch_event`), avant la descente dans l'arbre — et non dans chaque widget.
  Douze widgets qui répètent une règle sont douze endroits où elle peut manquer, et c'est
  exactement ce que M7b avait livré. L'information n'est pas détruite : une boucle
  d'application tient l'événement d'entrée avant de le distribuer, donc qui a besoin des
  relâchements les a encore.
- **Une répétition passe, et c'est éprouvé.** Maintenir une flèche doit répéter le
  déplacement : un filtre qui écarterait « tout ce qui n'est pas un enfoncement » serait le
  défaut symétrique, moins visible et plus pénible. Les deux moitiés de la règle portent
  chacune leur essai, dans le décodeur et dans la distribution, parce qu'un filtre trop
  large passerait l'un et casserait l'autre.
- **L'essai en pseudo-terminal est possible**, contre ce que M7b avait écrit (« aucun essai
  ne le voit, il faut un terminal Kitty réel pour le constater »). Il suffit d'**émettre
  les octets** du relâchement : le décodeur décode ce qu'il reçoit, sans égard pour ce que
  le programme a négocié. Les deux essais exigés par `CLAUDE.md` ont été faits, et ils sont
  décisifs : avant correction, quatre repeints, la marque apparue au troisième et
  **disparue au quatrième**, avec `4 o` au titre — la flèche relâchée avait descendu le
  curseur et le relâchement d'`Inser` avait démarqué. Après, trois repeints, la marque dans
  le dernier, `2 o` au titre.
- **1064 essais** (dix de plus), les **7 codecs toujours à 100 %** de régions, de fonctions
  et de lignes, et le fuzzing rejoué sur le décodeur touché : **282 946 exécutions en
  61 secondes, aucun plantage**.
- Une leçon d'outillage, sur la couverture : l'aide d'essai écrite d'abord portait un bras
  `panic!` jamais pris, soit **deux régions non couvertes** et un codec en défaut d'ADR-012.
  Réécrite en aide **constructrice** — elle fabrique l'événement attendu au lieu
  d'extraire celui qu'on a reçu —, elle n'a plus de branche morte. Le code d'essai compte
  dans la mesure, et c'est voulu.

### Fait au jalon M7d — la sélection de texte, et le presse-papiers

- **La sélection est une ancre, pas deux bornes.** La sélection est l'intervalle entre
  l'ancre et le curseur, *dans l'ordre où ils se trouvent*. Garder deux bornes déjà
  ordonnées ferait perdre le sens du geste : sélectionner trois caractères vers la droite
  puis remonter vers la gauche doit **réduire** la sélection, et non en commencer une
  autre. C'est éprouvé par un essai qui vaut pour les deux widgets.
- **Les quatorze actions `Move*` sont devenues `Move { motion, extend }`**, avec
  `AirTextMotion` pour le déplacement et deux constructeurs qui nomment les deux valeurs du
  drapeau — `move_by` efface la sélection, `select_by` l'étend. Les décliner en deux
  familles de variantes aurait obligé à ajouter chaque déplacement futur **à deux
  endroits**, et le jalon M7c venait de montrer ce que coûte une règle écrite deux fois.
  C'est une rupture d'API, assumée en `y` (ADR-009).
- **Trois rôles de palette, portée à trente-cinq** : `EditorSelected`, `InputLineText`,
  `InputLineSelected`. Les valeurs de Turbo Vision sont confirmées par une source
  indépendante du calcul, comme à M7b — `tvdemo2.cpp` nomme lui-même « Highlighted text »
  (14), « Input normal » (50) et « Input selected » (51). Et la transcription a **validé
  l'existant** : le texte non sélectionné de l'éditeur n'a pas besoin d'un rôle propre,
  parce que `cpEditor[1]` est déjà `WindowText`, transcrit à M5. La vue peignait juste
  depuis le début, sans qu'on sache pourquoi.
- **Les gestes, au clavier et à la souris** : `Maj`+flèches, `Maj`+`Début`, `Maj`+`Fin`,
  `Maj`+clic, le glisser, `Ctrl`+`A`. Le glisser n'a demandé **aucun état** dans les vues :
  l'appui efface la sélection, donc le premier déplacement pose l'ancre là où le bouton est
  tombé. Garder le point d'appui dans la vue l'aurait dédoublé avec l'ancre.
- **Une répétition du piège de M7c, évitée** : la surbrillance se vérifie sur les
  **couleurs** de la surface rendue, jamais sur les caractères. Un essai qui ne regarderait
  que le texte passerait alors que rien n'est surligné.
- **Le presse-papiers est au contrôleur**, ni au modèle ni à la vue, parce qu'il est
  partagé entre les deux widgets éditables. Les vues le **demandent** par trois commandes —
  `COPY`, `CUT`, `PASTE` — sans jamais le toucher, comme un ascenseur demande une position
  sans rien défiler. `AirClipboard` refuse un texte vide : couper une sélection vide ne doit
  pas détruire une copie que l'utilisateur croyait sûre.
- **Le collage encadré du terminal est enfin consommé.** Le décodeur produisait
  `AirInputEvent::Paste` depuis le jalon M1 et **personne ne le lisait** : la séquence était
  décodée, puis jetée. Les deux widgets éditables le traitent désormais, par la même action
  `InsertText` que le presse-papiers interne — un collage n'est pas une suite de frappes, et
  le traiter comme telle ferait avancer la génération une fois par caractère.
- **Le grisage en *pull* a servi une seconde fois sans rien ajouter** : « Copier » et
  « Couper » grisés sans sélection, « Coller » grisé sur un presse-papiers vide.
- **Trois attendus d'essais faux, et le code juste à chaque fois.** Le surlignage après un
  défilement horizontal (la fenêtre montre les colonnes 5 à 8, donc `gh` tombe en positions
  1 et 2) ; et coller juste après avoir copié, qui **remplace** la sélection au lieu de la
  dupliquer, parce que copier ne désélectionne pas — ce qui est le comportement correct.
  L'essai a été scindé en deux, chacun disant ce qu'il vérifie.
- **1124 essais** (soixante de plus), les 7 codecs toujours à 100 %, et neuf skins
  importables sur trente-neuf — un de moins qu'à M7b, pour une raison nouvelle au décompte
  et consignée dans l'ADR-008.

### Fait au jalon M7e — le double et le triple clic

- **La décision qui restait à prendre est prise, et elle était de placement.** Un double
  clic n'est pas une séquence d'octets : c'est **deux séquences et un délai**. Le décodeur
  est pur, et c'est cette pureté qui permet au fuzzing de vérifier qu'un découpage
  arbitraire de l'entrée produit la même suite d'événements — y mettre une horloge la
  détruirait. La synthèse vit donc dans la **boucle d'application**, seul endroit qui voit
  tous les événements avant l'arbre et qui connaît déjà le temps. Ni dans les widgets, où
  ce serait la même règle écrite douze fois (leçon de M7c, appliquée avant de la repayer),
  ni dans `dispatch_event`, qui est une fonction pure et doit le rester.
- **Le seuil vient de Turbo Vision, il n'est pas inventé.** `TEventQueue::doubleDelay` vaut
  8 ticks de l'horloge BIOS à 18,2 Hz, soit **440 ms**, et les trois conditions de
  `tevent.cpp` sont reprises telles quelles : le **même bouton**, **exactement la même
  cellule**, et le délai. La condition de position est stricte chez Borland — pas de
  tolérance d'une cellule — et elle le reste ici, une cellule de terminal étant assez
  grande pour qu'un double clic y tienne.
- **Le triple clic venait avec**, parce que c'est le même compteur : Borland le gère
  (`meTripleClick`), et un quatrième clic rapide reste un triple chez lui comme ici.
- **Le suiveur ne tient pas d'horloge** : l'instant lui est donné, et la mesure d'écart
  aussi. C'est ce qui rend la règle vérifiable par **treize essais déterministes**, là où un
  `Instant::now()` à l'intérieur aurait demandé de faire dormir un test — donc un test lent
  et instable. Les essais emploient un compteur de millisecondes ; la boucle, un vrai
  instant.
- **L'événement s'ajoute, il ne remplace pas.** Un double clic produit `Pressed` **puis**
  `DoubleClicked` : l'ordre compte, parce que c'est l'enfoncement qui place le curseur et le
  clic multiple qui étend ensuite la sélection. Et un widget qui ignore le geste garde le
  comportement du simple clic au lieu de ne plus rien recevoir — la rétrocompatibilité est
  obtenue par construction, pas par précaution.
- **Un redimensionnement coupe l'enchaînement.** La cellule d'avant ne désigne plus la même
  chose, et enchaîner dessus ferait un double clic sur un autre widget.
- **Trois usages, et le troisième n'est pas une sélection** : le double clic sélectionne le
  segment sous le pointeur et le triple la ligne entière dans la vue d'édition ; dans un
  champ d'une ligne, le triple vaut « tout sélectionner » ; et dans une **liste**, le double
  clic **ouvre** l'item, comme dans Midnight Commander. Le même geste, trois sens selon le
  widget — ce qui est la raison pour laquelle la boucle le synthétise sans décider quoi en
  faire.
- **Le segment et non « le mot »** : `unicode::segment_at` rend les bornes du segment au
  sens de l'UAX #29, donc un mot, mais aussi une suite d'espaces ou un signe de ponctuation.
  Un double clic sur un espace sélectionne les espaces, parce que rendre « rien » laisserait
  le geste sans effet visible.
- **Le triple clic prend le saut de ligne** : l'ancre au début de la ligne, le curseur au
  début de la **suivante**. C'est ce qui fait qu'une suppression retire la ligne entière,
  sans laisser une ligne vide derrière.
- **Vérifié en pseudo-terminal avec son contrôle**, et c'est ce qui prouve la chaîne
  complète — séquence SGR, décodeur, synthèse, distribution, intention, modèle,
  enregistrement : deux clics sur « belle » puis `Ctrl`+`X` donnent « une  journee », et un
  **seul** clic laisse le texte intact.
- **1161 essais** (trente-sept de plus), 7 codecs toujours à 100 %.
- Un point de catégorie, écrit pour qu'on n'y voie pas un oubli : `input/clicks.rs` n'est
  **pas** soumis à la règle des 100 % de l'ADR-012. Ce qui fait un codec est de convertir un
  flux, pas de vivre sous `input/` — et ce fichier agrège des événements avec une horloge
  sans en convertir aucun. La note est dans la liste de `xtask`.

### Fait au jalon M7f — les cases à cocher et les boutons radio

- **Un seul type pour les deux, comme Turbo Vision.** `TCheckBoxes` et `TRadioButtons`
  dérivent tous deux de `TCluster` et ne diffèrent que par trois choses : la **marque**
  dessinée, ce qu'un **appui** fait, et si un déplacement du curseur emporte la sélection.
  Le reste — la disposition en lignes, la navigation aux flèches, les lettres d'accès
  rapide, le clic — est identique. Sans héritage, deux types séparés auraient dupliqué ce
  reste, et une règle écrite deux fois finit par ne plus l'être qu'une : c'est la leçon de
  M7c et M7d. D'où un `AirCluster`, un `AirClusterKind` qui porte la différence, et deux
  constructeurs nommés pour que le site d'appel dise lequel il construit.
- **Le déplacement qui choisit vient de Borland, pas d'une préférence.**
  `TRadioButtons::movedTo` affecte `value` : dans un groupe à choix unique, la flèche
  sélectionne. Un curseur qui s'y promènerait sans rien choisir obligerait à appuyer après
  chaque flèche, pour un geste qui n'a qu'une issue possible. Dans un groupe de cases, la
  flèche ne fait que déplacer — et les deux comportements portent chacun leur essai.
- **Trois rôles de palette, portée à trente-huit**, et la méthode des jalons précédents a
  resservi : `cpCluster "\x10\x11\x12…"` donne `cpAppColor[47..49]`, que `tvdemo2.cpp`
  nomme lui-même « Cluster normal », « Cluster selected » et « Cluster shortcut ». Au
  passage, cette lecture de la table est recoupée par les transcriptions de M7b : les
  valeurs des listes concordent à l'octet.
- **Six glyphes qui ne viennent pas du skin, et c'est écrit comme tel.** `[`, `X`, `]`,
  `(`, `•`, `)` : **aucun** des trente-neuf skins de `mc` ne porte de clef pour eux, ce qui
  a été vérifié par un balayage et non supposé. Ils restent donc ceux du thème par défaut,
  et ce n'est pas une transcription — ce que l'artefact généré dit maintenant.
- **Un décompte faux trouvé et corrigé, hérité de M7b.** Le commentaire de l'artefact et la
  doc de `AirGlyphSet` annonçaient « cinq glyphes » hors skin et en énuméraient **six** :
  `scroll_thumb` avait été ajouté à M7b sans que la phrase suive. Le compte passe à douze,
  et il est juste aux trois endroits qui le portent.
- **Une généralisation de l'ADR-008 démentie par la mesure.** Il affirmait qu'« un rôle de
  plus est une clef de plus à savoir exprimer » ; ce jalon ajoute trois rôles et le compte
  des skins importables **ne bouge pas** — toujours neuf sur trente-neuf —, parce que les
  trois se branchent sur `[dialog] _default_`, `dfocus` et `dhotnormal`, déjà lues. Ce qui
  fait baisser le compte est une **clef** nouvelle, pas un rôle nouveau. Corrigé dans l'ADR.
- **L'invariant du choix unique vaut aussi à la construction** : un item coché ajouté à un
  groupe radio décoche les autres. Sans cela, un groupe mal monté afficherait deux points,
  et le premier appui en effacerait un sans qu'on sache lequel.
- **Deux états légitimes, écrits plutôt que corrigés** : un groupe radio neuf n'a **aucun**
  choix, parce que « rien n'est encore décidé » est un état réel et qu'en cocher un d'office
  déciderait à la place de l'utilisateur ; et décocher le choix courant est accepté, parce
  que le refuser obligerait l'appelant à reconstruire le groupe pour y revenir.
- **Le curseur ne se montre que dans un groupe qui a le focus.** Deux groupes côte à côte
  qui marqueraient chacun un item laisseraient croire à deux curseurs — et l'exemple `frame`
  les met précisément côte à côte, ce qui est la seule disposition qui le vérifie à l'œil.
- **Vérifié sur les octets émis, dans les deux thèmes** : `[X]`, `[ ]`, `(•)` et `( )`
  apparaissent dans la sortie de `frame` comme dans celle de `frame mc`, et la couleur du
  curseur de groupe du skin de `mc` (`dfocus`) y est présente.
- **1191 essais** (trente de plus), 7 codecs toujours à 100 %.
- Une extraction imposée par clippy, et le découpage est juste : `emit` dépassait cent
  lignes, et l'émission du jeu de glyphes en est sortie. Les deux sorties ne partagent que
  le tampon qu'elles remplissent.

### Fait au jalon M7g — les deux limites du thème, et une troisième qui n'en était pas une

- **Un attribut par rôle.** `AirColorPair` porte `attributes` en plus de ses deux
  couleurs : un rôle décrit une **présentation**, pas seulement un couple. Le thème Turbo
  Vision n'en pose aucun — les seize couleurs du VGA portent déjà la distinction
  vive/sombre dans l'index lui-même —, et c'est exactement ce qui rend le défaut de ce
  jalon silencieux.
- **Le chemin de l'attribut jusqu'à la grille, et aucun autre.**
  `AirColorPair::cell(character)` et `AirPainter::draw_text_with_colors(origin, text,
  colors)`. Les vingt-trois appels de widget et les vingt constructions de cellule qui
  passaient `colors.foreground, colors.background, AirAttributes::NONE` sont réécrits :
  ce geste était précisément celui qui laissait l'attribut au bord du chemin. `draw_text`
  garde ses cinq paramètres pour ce qui ne vient pas d'un rôle.
- **Un test de garde, vérifié par son contrôle.** Ni l'un ni l'autre des deux thèmes livrés
  ne pose d'attribut : rien ne montrerait un site fautif pendant le développement. Un test
  peint donc les douze widgets avec une palette dont les trente-huit rôles sont gras et
  refuse la moindre cellule qui ne l'est pas. Rétabli sur un seul site, il échoue en
  nommant le widget, la cellule et sa position — c'est ce qui prouve qu'il n'est pas vide.
- **`AirColor::TerminalDefault`**, émise en SGR par `39` et `49` — les deux paramètres qui
  *annulent* une couleur au lieu d'en poser une. Elle traverse les trois profondeurs du
  quantificateur **inchangée**, et c'est la seule dans ce cas : la quantification choisit la
  plus proche voisine d'une couleur connue, et celle-ci est celle que l'utilisateur a réglée
  dans son terminal. La remplacer par un index effacerait ce qu'elle demande.
- **`AirAttributes` gagne `blink`**, cinquième et dernier attribut que `mc` sait écrire.
  Aucun des trente-neuf skins ne l'emploie — mesuré —, et il est là pour que la
  correspondance soit **totale** : un test confronte les cinq noms de l'analyseur aux cinq
  champs de la structure, ce qui rend le refus « cet attribut n'a pas d'expression »
  démontrablement inatteignable au lieu de supposé tel.
- **Neuf skins importables sur trente-neuf, puis seize, puis trente-neuf.** Les deux
  limites levées n'en ont rendu que **seize** importables, là où l'ADR-008 en annonçait
  trente-huit. Vingt-trois restaient refusés pour une **clef absente** : dix-huit n'écrivent
  pas `[editor] editframeactive`, quatre n'ont pas de `[editor] _default_`, `gotar.ini` n'a
  pas de `[core] inputmark`.
- **Le décompte de l'ADR était faux par construction**, et c'est la leçon du jalon.
  L'importeur s'arrête à la **première** cause de refus qu'il rencontre : le décompte
  comptait donc une cause par skin, alors que la plupart en avaient deux. Une mesure qui
  compte des causes exclusives là où elles se cumulent ne se corrige pas en raffinant le
  décompte — elle se corrige en levant une limite et en recomptant.
- **La troisième cause n'était pas une limite du modèle, mais un défaut de l'analyseur.**
  L'ADR rangeait la clef absente parmi « les limites du modèle d'air-tui ou les lacunes du
  skin », et écrivait que « `mc` en applique un [défaut] ; air-tui préférerait qu'on décide
  lequel ». Il n'y avait rien à décider : le manuel de `mc` décrit la cascade en deux
  endroits — `[core] _default_` est « used in all other sections if they not contain color
  definitions », et le `_default_` de chaque section est « used [core]._default_ if not
  specified ». Une clef absente hérite donc de sa section puis de `[core]`. Aucun des
  vingt-trois skins n'est mal formé ; `mc` les affiche tous.
- **Une limite attribuée au modèle est une hypothèse** tant qu'elle n'a pas été levée. Les
  deux premières l'étaient vraiment ; la troisième était une lacune rangée du mauvais côté,
  et il a suffi de lire le manuel pour s'en apercevoir.
- **Un rôle composé retient les attributs de la clef qui donne l'avant-plan.** Les trois
  attributs que les skins emploient portent sur le dessin du caractère, et c'est
  l'avant-plan qui le décrit ; prendre ceux du fond mettrait en gras un texte dont le skin
  ne dit rien.
- **L'artefact committé est inchangé, empreinte comprise** (`0xefc735864405630c`) : le skin
  par défaut ne pose aucun attribut et n'emploie pas la couleur du terminal, donc la forme
  courte `pair(f, b)` le décrit toujours en entier. Les quatre fonctions d'aide émises se
  décident désormais **séparément** — un attribut suffit à envoyer un couple d'index sur la
  forme longue, qui emploie `index` sans qu'aucune vraie couleur paraisse, et émettre `rgb`
  avec elle laisserait du code mort que `clippy -D warnings` refuse.
- **Vérifié sur les octets émis, de bout en bout, sur deux skins jusqu'ici refusés** :
  `modarcon16-defbg` fait apparaître `;49m` vingt fois — le fond du terminal — et `[0;1;`
  quatre fois ; `seasons-autumn16M` fait apparaître `[0;4;38;2;0;0;0;48;2;255;201;12m`,
  c'est-à-dire un souligné sur une vraie couleur, pour la lettre d'accès rapide d'un menu.
  Les quatre skins essayés ont été compilés **et bâtis** avant que l'artefact soit rétabli.
- **1203 essais** (douze de plus), 7 codecs toujours à 100 %.
- **Cinq décomptes faux corrigés**, tous hérités de M7d et M7f : « trente-deux rôles » dans
  la doc du module `theme`, dans celle de `from_parts` et dans celle de `ROLE_SOURCES`,
  « huit assiettes » là où il y en a onze, et « quinze correspondances annotées » dans
  l'ADR-008 là où l'outil en compte seize depuis deux jalons. Un décompte écrit dans un
  commentaire ne se recompte jamais tout seul.
- **Deux affirmations fausses corrigées dans `cell.rs`** : le doc-comment d'`AirColor`
  annonçait le quantificateur « prévu au jalon M5 et **n'est pas écrit** », celui
  d'`AirCell` les tables UCD de même. Les deux existent depuis M5 et M3.

### Fait au jalon M7i — plusieurs skins compilés, et le choix par un nom

- **La limite que M7g avait laissée est levée.** Les trente-neuf skins étaient importables,
  mais pas **ensemble** : importer le second effaçait le premier. `theme/skins/mod.rs` est
  devenu un **registre généré**, et `AirTheme::from_command_line_name` résout trois
  formes — `tv`, `mc`, et `mc:<nom>` pour un skin compilé.
- **L'ADR-008 n'est pas touché, et c'est le point** : rien n'est lu à l'exécution. Le choix
  est une clef dans une table construite à la compilation, pas un fichier ouvert au
  démarrage. L'espace de noms `mc:` dit d'où vient un thème.
- **Le registre est généré parce que deux listes disent la même chose** : la liste des
  modules et la table des noms divergeraient si on les tenait à la main. Les deux sortent
  du même balayage du répertoire, à chaque import.
- **Trois skins repris, et le chiffre des trente-neuf écrit à côté de la décision.** Les
  trente-neuf coûtent 395 Kio de source générée et deux dixièmes de seconde de compilation
  — 1,03 s contre 1,26 s. Ce n'est donc pas le coût technique qui tranche : reprendre
  trente-neuf skins d'un projet sous GPLv3 dans un dépôt sous MPL-2.0 est une décision de
  l'auteur, et l'argument du `NOTICE` lui appartient. Les trois choisis le sont pour ce
  qu'ils démontrent : `default`, `modarcon16-defbg` (la couleur du terminal, quinze rôles)
  et `seasons-autumn16M` (vraies couleurs et attributs).
- **La résolution d'un nom vit dans `air-tui`**, et non dans les deux applications :
  `ThemeChoice` y était écrit deux fois, avec des défauts différents.
- **Une régression introduite et rattrapée en relisant** : le remplacement de
  `ThemeChoice` par un nom donnait `tv` pour défaut aux deux applications, alors que `rmc`
  s'ouvre aux couleurs de `mc` — ce que son propre doc-comment disait. Aucun essai ne
  l'affirmait autrement que par une comparaison à `ThemeChoice::default()`, qui
  disparaissait avec le type ; celui qui le dit maintenant compare à `DEFAULT_THEME`.
- **Le nom d'un thème est le nom du skin transposé en identifiant Rust**, minuscules
  comprises : `seasons-autumn16M.ini` donne `mc:seasons_autumn16m`. `clippy -D warnings`
  refuse `non_snake_case`, ce qui s'est vu à la compilation. `--list-themes` donne les noms
  exacts et fait foi.
- **Une arête du dépôt, payée deux fois** : `xtask` dépend d'`air-tui`, donc un registre
  désaccordé de ses fichiers empêche l'outil de se bâtir — et c'est l'outil qui écrit le
  registre. Retirer un artefact demande de corriger `mod.rs` à la main une fois. C'est le
  prix d'un espace de travail unique (ADR-005), écrit dans `CLAUDE.md`.
- **Vérifié sur les octets émis, les quatre thèmes dans le même binaire** : `tv` donne
  `38;5;0;48;5;7`, `mc:default` `38;5;0;48;5;6`, `mc:modarcon16_defbg` `38;5;15;48;5;2`
  avec seize `;49m`, et `mc:seasons_autumn16m` `38;2;0;0;0;48;2;255;201;12` avec trois
  soulignés. Un seul exécutable, quatre apparences.
- **1296 essais**, 7 codecs toujours à 100 %.

### Fait après M7h — les deux derniers gates de conventions

- **Les six contrôles de `docs/CONVENTIONS-fr.md` sont écrits et exécutés en CI.** Plus
  aucun ne reste à tenir à la main, et la section « À outiller » du document a disparu.
- **Chiffrés avant d'être écrits, et le chiffre est zéro** : soixante-dix-neuf fichiers
  portent tous l'avis MPL, onze blocs `unsafe` portent tous leur `// SAFETY:`. Ce sont des
  gates de **prévention**, pas des réparations, et le dire est la moitié de leur valeur :
  leur intérêt est qu'un fichier **nouveau** est le moment où une de ces deux règles
  s'oublie, et c'est aussi le moment où personne ne relit les soixante-dix-huit autres.
- **`check-safety-comments` porte un apport que personne n'avait demandé, et c'est le seul
  réel** : la **frontière** de l'ADR-011 — « `unsafe` vit dans `air-tui-platform-posix`, et
  nulle part ailleurs ». Rien ne l'outillait. Les deux crates du cœur la tiennent par leur
  `#![forbid(unsafe_code)]`, mais `redit`, `rmc`, `xtask` et les cibles de fuzzing ne le
  portent pas : un `unsafe` y aurait compilé sans un mot.
- **« Un bloc `unsafe` » n'a pas une seule réponse syntaxique**, et c'était la vraie
  question de conception. Cinq formes portent le mot-clé ; seule `unsafe { }` exige un
  `// SAFETY:`, parce que seule elle affirme au compilateur quelque chose qu'il ne vérifie
  pas. `unsafe extern` est une **déclaration** que l'édition 2024 oblige à marquer, et
  `unsafe fn`, `unsafe impl`, `unsafe trait` reportent l'engagement sur l'appelant, qui le
  documente par une section rustdoc. Les quatre sont **comptées et annoncées** à chaque
  exécution plutôt qu'exemptées en silence.
- **Le mot-clé est cherché par balayage lexical, et on peut montrer pourquoi** :
  `xtask/src/naming.rs` porte deux fois le mot `unsafe` dans des **chaînes de
  caractères** — sa liste de mots-clés, et un source d'essai. Un contrôle fondé sur `grep`
  déclarerait donc `xtask` en infraction, et on apprendrait à l'ignorer. Le balayage est
  celui de `check-naming`, partagé plutôt que réécrit : deux scanners donneraient deux
  comportements dont un seul serait éprouvé.
- **Le marqueur est cherché sur trois lignes au-dessus du bloc**, et non une : une
  justification tient rarement sur une ligne. La borne existe pour qu'un marqueur d'un
  autre bloc ne couvre pas celui-ci, et les deux bords portent leur essai.
- **Vérifié par trois contrôles négatifs** : un avis retiré d'un fichier, un `// SAFETY:`
  retiré d'un bloc, et un `unsafe { }` ajouté dans `redit`. Les trois sont attrapés et
  nommés avec leur fichier et leur ligne, puis rétablis.
- Vingt-et-un essais, dont un qui confronte `licence.rs` à lui-même : le contrôle porte
  l'avis qu'il exige.

### Fait au jalon M7h — l'annulation, et la mutation unique qu'elle a imposée

- **Un journal de remplacements, pas des instantanés** (ADR-014). Un instantané du tampon
  coûterait la taille du document à chaque frappe ; une entrée de journal coûte la longueur
  du texte qui change. Le prix du choix est que le journal n'est juste que si chaque
  mutation produit son inverse exact.
- **Et c'est ce prix qui a payé : sept chirurgies de lignes ramenées à une.** Le tampon
  portait sept écritures à la main — frappe, collage, saut de ligne, deux suppressions de
  grappe, suppression de ligne, suppression de sélection —, chacune découpant les lignes à
  sa façon. Elles se ramènent toutes à `splice(début, fin, texte)`, qui fait la chirurgie,
  avance les générations et consigne l'entrée. L'annulation n'est donc pas une couche posée
  à côté du tampon : c'est une propriété de son **unique** point de mutation.
- **Le geste est l'unité, et ses bords sont écrits parce qu'ils sont arbitraires.** Une
  insertion se fond dans la précédente si elle commence là où l'autre finissait ; un retour
  arrière de même, vers la gauche. Tout le reste coupe : déplacement, changement de
  sélection, saut de ligne, collage, suppression de sélection ou de ligne, enregistrement,
  chargement. Un collage est donc **un** pas, et `Entrée` une frontière.
- **Un défaut trouvé par les essais de l'annulation elle-même** : taper sur une sélection
  faisait deux pas — une suppression, puis une insertion —, et l'utilisateur aurait dû
  annuler deux fois un geste qu'il n'avait fait qu'une. Les trois insertions remplacent
  désormais la sélection dans le **même** remplacement, et annuler rend la sélection.
- **Un second défaut trouvé par un essai existant pendant la refonte** : la plage d'une
  **dernière** ligne supprimée part de la fin de la précédente, parce que le saut de ligne
  à emporter est devant elle et non derrière. L'écriture d'origine retirait la ligne du
  vecteur, ce qui masquait la question. Le filet de plus de cent essais du tampon a attrapé
  l'écart au premier passage.
- **Le drapeau de modification est dérivé de l'historique**, et non posé à la main :
  annuler jusqu'au point d'enregistrement éteint l'étoile du titre. Un drapeau que seule une
  édition allume y resterait allumé sur un texte revenu à son état enregistré. Le point
  d'enregistrement suit la borne du journal, et devient **injoignable** si l'entrée qui y
  menait est oubliée — c'est le seul endroit où la borne se voit, et il porte son essai.
- **Journal borné à deux cent cinquante-six pas**, la plus ancienne entrée oubliée au-delà.
  Un journal non borné est une fuite lente dans une session longue ; une borne est une
  capacité qu'on annonce plutôt qu'une mémoire qu'on promet.
- **`Ctrl`+`Z` et `Ctrl`+`R`, le second choisi après vérification et non de mémoire.**
  `Ctrl`+`Y` est déjà liée à la suppression de ligne de Turbo Vision, et la reprendre
  casserait un geste transcrit ; `Ctrl`+`Maj`+`Z` est indistinguable de `Ctrl`+`Z` sur un
  terminal qui ne négocie pas le protocole Kitty, donc le rétablissement marcherait sur
  certains terminaux et pas sur d'autres. `Ctrl`+`Z` n'interrompt pas le processus parce
  que `cfmakeraw` efface `ISIG` — vérifié dans la crate POSIX, pas supposé.
- **Aucun changement structurel ajouté à l'ADR-007** : le grisage des deux commandes est en
  *pull*, lu au dessin comme celui de `COPY` depuis M7d. Les deux touches sortent par le
  canal des **intentions**, pas des commandes, parce que l'historique est dans le modèle que
  la vue adresse déjà ; les commandes `UNDO` et `REDO` n'existent que pour la barre de menu.
- **1263 essais** (soixante de plus), 7 codecs toujours à 100 %.
- **Vérifié en pseudo-terminal, par paires** : `ABC` puis `Ctrl`+`Z` rend « origine » ;
  `ABC`, `Ctrl`+`Z`, `Ctrl`+`R` rend « ABCorigine ». Le contrôle du rétablissement est
  l'essai d'annulation seule, et non un essai sans touche — sans `Ctrl`+`Z` le fichier porte
  déjà « ABCorigine », donc un rétablissement muet donnerait le même fichier qu'un
  rétablissement qui marche. Un troisième essai montre la coupure du geste : « ab », `Début`,
  « z », `Ctrl`+`Z` rend « aborigine ».

### Fait après M7g — le gate `check-dco`

- **Le troisième des trois contrôles « à outiller » est écrit.** `cargo xtask check-dco`
  refuse un commit sans `Signed-off-by`, et tout trailer qui ferait d'un agent un auteur :
  `Co-Authored-By` nommant `claude` ou `anthropic`, et `Claude-Session`. Il en restait deux
  à écrire à ce moment-là, `check-licence-headers` et `check-safety-comments` : les deux
  l'ont été depuis.
- **Deux portées, et la distinction est le cœur du contrôle.** Sans `--all`, il garde les
  commits de la branche, ce qu'une pull request vérifie avant la fusion. Avec `--all`, il
  parcourt tout l'historique — et c'est le seul moment où le commit de **fusion squash**
  existe, puisqu'il est fabriqué après le dernier passage de la CI sur la branche. La CI
  exécute la première portée sur une pull request et la seconde sur une poussée vers `main`.
- **Sur `main`, ce contrôle constate au lieu de prévenir**, et il faut le dire plutôt que
  laisser croire le contraire : un commit de `main` ne s'amende pas, le force-push y étant
  interdit sans exception. C'est tout ce qu'un dépôt ainsi protégé peut obtenir, et c'est
  déjà le passage du silence au rouge.
- **Un pardon, nommé et motivé** : `806f7ae`, la fusion du jalon M7g, le seul commit de
  `main` sans DCO. Le compte rendu l'annonce à **chaque** exécution, succès compris. Un
  contrôle qui taît ce qu'il ne regarde pas donne la même assurance qu'un contrôle absent,
  et restreindre sa portée pour qu'il passe aurait été le défaut qu'il existe pour empêcher.
- **Trois décisions tranchées par la mesure, pas par l'intuition** : l'adresse du
  `Signed-off-by` n'est **pas** confrontée à celle de l'auteur, parce que dix-neuf des
  vingt-trois commits de `main` sont des fusions squash que GitHub signe de
  `…@users.noreply.github.com` ; un co-auteur **humain** reste légitime, aucune règle du
  dépôt ne l'interdisant ; et la casse d'une clef de trailer ne décide pas, puisque
  `git interpret-trailers` les compare sans elle.
- **La lecture se fait d'un seul appel à `git log`**, séparateurs U+001E et U+001F. Un
  message qui en contiendrait un ferait rendre trois champs à un enregistrement : le cas est
  refusé **en le nommant** plutôt que mal lu.
- **Les commits de fusion sont comptés et ignorés**, et c'est le premier passage en
  intégration continue qui l'a imposé : sur un événement `pull_request`, `actions/checkout`
  ne place pas la tête de la branche mais un commit de fusion que **GitHub fabrique** pour
  éprouver le résultat — « Merge <sha> into <base> ». Personne ne l'a écrit, personne ne
  peut le signer, et il était dans la plage. Un commit à plus d'un parent n'apporte aucun
  contenu à certifier, ce qui est aussi la raison pour laquelle `git rebase --signoff` ne
  signe pas les fusions. Une fusion **squash** n'en est pas une — un seul parent —, donc
  les dix-neuf de `main` restent examinées.
- **Vérifié par ses contrôles négatifs**, sur une branche jetable : un commit sans
  signature, un `Co-Authored-By: Claude …` et un `Claude-Session:` sont chacun attrapés, la
  commande rend le code 1, et une vraie fusion `--no-ff` est bien comptée comme ignorée. Un
  gate qu'on n'a pas vu échouer ne garde rien.
- Dix-neuf essais, tous sur la partie pure : un commit y est une empreinte, un nombre de
  parents et un message, donc les règles se vérifient sans dépôt.

### Défauts ouverts

- **Le commit de fusion du jalon M7g (`806f7ae`) ne porte pas de `Signed-off-by`.** Les
  quatre commits de la branche le portaient tous ; la fusion squash a reçu un corps écrit à
  la main, et le trailer que GitHub aurait repris des commits a disparu avec lui. Ce commit
  n'est **pas réparable** : amender un commit de `main` demanderait un force-push, que ce
  dépôt interdit sans exception. `check-dco` le **pardonne nommément** et l'annonce à chaque
  exécution ; il reste donc visible au lieu d'être oublié.

### Pas fait — et à ne pas annoncer comme fait

- La table des touches fonctionnelles de Kitty au-delà des numéros certains : pavé
  numérique, touches multimédia, touches modificatrices seules. Hors table, donc consommées
  sans événement — et la table a été écrite d'après la spécification, **pas vérifiée contre
  un terminal réel**.
- Dans `rmc` : il **n'exécute aucun programme** et ne modifie aucun fichier. Ni ligne de
  commande, ni `F3` pour voir, ni `F4` pour éditer, ni copie, ni déplacement, ni
  suppression ; `Entrée` sur un fichier ne fait rien. Il ne lit pas la configuration de
  `mc`, ne connaît pas ses systèmes de fichiers virtuels, et n'affiche ni droits, ni
  propriétaire, ni date. La barre de touches ne publie que `Tab`, `F5` et `F10` : en
  annoncer dix dont sept ne feraient rien serait ce que le NO GO n° 2 interdit.
- Les **colonnes multiples** d'une liste : `mc` sait afficher un répertoire sur deux ou
  trois colonnes, et `TListViewer` portait un `numCols`. Une colonne suffit à un panneau de
  fichiers, et la seconde demande de décider ce que `Page bas` veut dire.
- **`AirDecorations`**, la troisième assiette du triplet de l'ADR-008. `AirTheme` est un
  couple. Le style de trait vit déjà dans les glyphes ; l'**ombre portée** ne peut pas être
  dessinée, parce qu'elle tombe hors des limites de la vue qui la projette et que le
  découpage du peintre ne souffre aucune exception. `AirRole::ButtonShadow` est dans la
  palette, transcrit de Borland comme de `mc`, et **aucun widget ne le lit**. Une assiette
  qui décrirait une ombre que rien ne dessine annoncerait une capacité absente.
- L'échange avec le **presse-papiers du système** : `AirClipboard` est interne au
  processus. Il ne parle ni à `pbcopy`, ni à `wl-clipboard`, ni à OSC 52 — trois mécanismes
  différents, dont le dernier est refusé par défaut par la plupart des terminaux et les
  deux premiers demandent de lancer un programme. Le couper-coller fonctionne **dans**
  l'application, et c'est tout ce qui est promis.
- Dans `redit` : pas de **recherche**, pas de **tabulation** — elle demande de choisir des
  taquets et de les appliquer au rendu comme au calcul de colonne —, et pas de seconde
  fenêtre. Le bureau sait en tenir plusieurs, rien ne les crée. L'annulation est faite
  (M7h).
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
- La couverture de `xtask` : 34 % à 81 % selon le module. C'est de l'outillage hors
  exécution du framework, qu'ADR-012 ne couvre pas — mais ce n'est pas une raison de le
  laisser là.
- Un coureur `aarch64` **Linux** : GitHub n'en offre pas gratuitement, donc cette
  architecture reste vérifiée à la main sur `neon`. C'est elle qui a prouvé la déclaration
  variadique d'`ioctl` au jalon M0.
- Les essais en **pseudo-terminal** en CI : un coureur n'a pas de terminal de contrôle.
  Ils restent dans la liste locale, où ils ont déjà trouvé le `POLLNVAL` de macOS.
- `air-tui-platform-air`, l'implémentation pour la cible `linux-air`.
- Les cibles `cargo-fuzz` des décodeurs à venir (Kitty, graphèmes Unicode).
- *(plus rien ici : la dette de vérification Linux est soldée, voir ci-dessous)*

## Prochaine tâche

**À trancher.** Le plan de jalons mène à M8 — l'étage d'entrée natif Linux —, mais M8 est
**bloqué par une vérification impossible à distance** : `evdev`, `TIOCLINUX` et `gpm`
demandent une console physique, inaccessible par SSH. Écrire du code qu'on ne peut pas
vérifier est exactement ce que ce dépôt refuse. Les candidats, du plus petit au plus gros :

1. **La peinture hors des limites d'une vue**, décision d'architecture dont dépendent
   `AirDecorations` et l'ombre portée. À trancher avant d'écrire la troisième assiette du
   triplet de l'ADR-008, pas après. C'est le dernier des candidats qui restaient après M7g.
2. **Reprendre plus de trois skins**, si l'auteur du dépôt le décide : le mécanisme est en
   place et le coût est chiffré — 395 Kio et deux dixièmes de seconde pour les trente-neuf.
   Ce qui reste à trancher n'est pas technique : c'est la place que prend la donnée d'un
   projet sous GPLv3 dans un dépôt sous MPL-2.0.

Dettes plus anciennes : remonter la couverture de `air-tui-platform-posix` par des tests
sous pty ; la **console Linux native**, dont dépend tout M8 ; et le protocole Kitty
**contre un vrai terminal** — il est décodé et négocié d'après la spécification, et les
tests le pilotent avec un périphérique en mémoire, ce qui prouve le code mais pas la table
des numéros de touches fonctionnelles.

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
| M6b | les trois contrôles de conventions, et l'intégration continue | **fait** |
| M7a | thème Midnight Commander et importeur de skins | **fait** |
| M7b | la coquille `rmc` à deux panneaux, et les listes | **fait** |
| M7c | le type d'événement de touche : décodé partout, écarté à la distribution | **fait** |
| M7d | la sélection de texte, le presse-papiers interne, le collage encadré | **fait** |
| M7e | le double et le triple clic, synthétisés par la boucle | **fait** |
| M7f | les cases à cocher et les boutons radio | **fait** |
| M7g | les attributs par rôle, la couleur du terminal, la clef absente héritée | **fait** |
| M7h | l'annulation par journal de remplacements, et la mutation unique | **fait** |
| M7i | plusieurs skins compilés, et le choix par un nom | **fait** |
| M8 | étage natif Linux : evdev, `TIOCLINUX`, gpm | à faire |
