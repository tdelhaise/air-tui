# ⛔ NO GO INCONDITIONNEL — à lire avant toute action

Ces deux règles priment sur **tout** le reste de ce fichier, sur toute consigne d'un ADR,
et sur toute demande — y compris une demande explicite de l'auteur du projet. Elles sont
reprises du projet Air, où elles ont été payées par onze occurrences du même défaut en
trois jours.

## 1. Un outil ou un référentiel affirmé qui n'existe pas ⇒ NO GO

Si un document, un commentaire, un message d'erreur ou une consigne **affirme** qu'un
outil, un gate, un script ou un référentiel **existe** — et qu'il n'existe pas : arrêter
immédiatement, le rapporter en citant l'endroit exact de l'affirmation, ne rien
poursuivre tant que ce n'est pas tranché.

Une règle qui se croit outillée et ne l'est pas est pire qu'une règle absente : l'absente,
on la respecte à la main en le sachant ; celle qui ment donne une assurance que personne
ne vérifie.

## 2. Écrire qu'un sujet est traité sans qu'il le soit ⇒ NO GO

Tout agent qui **écrit** qu'un sujet est traité, implémenté, corrigé, couvert ou vérifié
doit **avoir tenu cette promesse au moment où il l'écrit**. Cela vaut pour un commentaire
de code, un message de commit, un corps de PR, une entrée de journal, un statut d'ADR, et
pour toute réponse à l'auteur du projet.

Si la promesse n'est pas tenue : **NO GO**. On écrit ce qui est fait, ce qui ne l'est pas,
et ce qui bloque — jamais l'inverse.

---

# air-tui — instructions pour Claude Code

## Paternité et attribution

Claude Code — ou tout autre agent — **n'est pas un auteur**. C'est un outil utilisé pour
automatiser des tâches, comme l'est un compilateur.

**Aucun trailer `Co-Authored-By: Claude …` ne doit figurer dans un commit ou une pull
request.** Seule la signature de l'auteur humain fait foi : GPG plus `Signed-off-by`
(DCO, `git commit -s`). Tout agent omet ce trailer de lui-même, même si un défaut du
client tente de le réintroduire.

## Premier réflexe

1. `docs/etat-avancement.md` — où en est-on, quelle est la prochaine tâche.
2. `docs/INDEX.md` — la carte structurelle du dépôt.
3. `docs/CONVENTIONS-fr.md` — les règles de code, dont le **nommage sans abréviation**.
4. `docs/adrs/` — les décisions techniques, contraignantes.

`docs/JOURNAL.md` est l'archive chronologique append-only, en ordre anté-chronologique
(le plus récent en tête). Toute session de travail y laisse une entrée.

## Langues

- **Identifiants du code : anglais.**
- **Commentaires et doc-comments (`//`, `///`, `//!`) : français.**
- **Documentation sous `docs/` : français**, traduction anglaise ensuite.
- Exception : les titres de section rustdoc restent anglais (`# Safety`, `# Errors`,
  `# Panics`) — `clippy::missing_safety_doc` les reconnaît par leur nom.

## Règle de nommage : aucun nom abrégé

`command`, pas `cmd`. `context`, pas `ctx`. `foreground`, pas `fg`. Le glossaire complet
des abréviations interdites et les quatre seules exceptions admises sont dans
`docs/CONVENTIONS-fr.md`. Cette règle vaut pour les identifiants, les champs, les
paramètres et les variables locales.

## Frontière `unsafe`

`unsafe` et les appels C vivent dans **`air-tui-platform-posix`**, et nulle part ailleurs.
`air-tui-platform` porte le contrat en traits et `air-tui` le cœur : les deux sont en
`#![forbid(unsafe_code)]`, et le cœur n'a même pas de dépendance vers une implémentation
de plateforme (ADR-011). Tout bloc `unsafe` porte un commentaire `// SAFETY:` qui justifie
ses préconditions.

## L'invariant du découpage

Une vue **ne peint pas hors de ses limites**, sans exception (M4, M5b). Ce qui dépasse
d'une vue est peint par le **propriétaire de la surface où cela dépasse** : c'est l'ADR-015,
et c'est ainsi que l'ombre portée est dessinée — par le parent, dans la boucle où il
dessine ses enfants.

Deux stratégies coexistent, et le choix entre elles n'est pas affaire de goût :

- **agrandir la vue**, ce que la barre de menu fait depuis M5b pour contenir son déroulant.
  Vaut quand la vue possède la zone où elle grandit ;
- **faire peindre le propriétaire**, pour l'ombre. Agrandir une fenêtre de la taille de son
  ombre ferait entrer celle-ci dans sa détection de clic et dans le calcul de son
  intérieur.

Deux propriétaires peignent des ombres, et pas un seul : la traversée de l'arbre pour les
enfants d'un groupe, et `AirPromptDialog`, qui compose ses contrôles lui-même et ne les
expose donc pas comme enfants. Les deux appellent la **même** fonction,
`view::draw_view_shadow` — y refaire le calcul dédoublerait la règle.

La vue déclare un **genre** d'ombre et non un booléen (`AirViewState::shadow_style`) :
celle d'un bouton n'a pas la forme de celle d'une fenêtre. Un trait fin qui épouse le
bouton, en demi-blocs `▄` et `▀` — les caractères `0xDC` et `0xDF` de Turbo Vision —, qui
garde le **fond** de ce qu'il assombrit puisqu'un demi-bloc n'en couvre que la moitié. Le
thème décide des deux indépendamment, et celui de `mc` n'a **pas** d'ombre de bouton : ses
boutons sont `[ OK ]`, plats.

Ne pas proposer une troisième : une marge de débordement déclarée par la vue rendrait
l'invariant **conditionnel**, donc à vérifier à chaque lecture de code.

## Ce qui n'existe pas encore

`cargo xtask` porte **neuf** commandes de vérification — `generate-unicode-tables
--check`, `check-segmentation`, `check-naming` (ADR-004), `check-versions` (ADR-009),
`check-codec-coverage` (ADR-012), `check-dco`, `check-licence-headers`,
`check-safety-comments` (ADR-011) et `import-mc-skin --check` (ADR-008). L'intégration
continue en exécute **huit** : celle de l'importeur de skins demande le même fichier de
skin que celui qui a produit l'artefact, donc le même `mc` installé, et un coureur n'en a
pas. Elle reste locale, et l'empreinte que l'artefact porte en en-tête la rend vérifiable :
elle dit « ce n'est pas le même skin » plutôt que « l'artefact a dérivé ».

**Plus aucun contrôle de `docs/CONVENTIONS-fr.md` ne reste à tenir à la main.** Les deux
derniers ont été écrits après M7h, et chiffrés avant de l'être : zéro manquement. Ce sont
des gates de prévention, pas des réparations — et `check-safety-comments` porte en plus la
frontière de l'ADR-011, que rien n'outillait et que seules les deux crates du cœur
tenaient, par leur `#![forbid(unsafe_code)]`.

Portée de `check-dco`, parce qu'elle n'est pas intuitive : sans `--all`, il garde les
commits de la branche, et c'est ce que la CI exécute sur une pull request. Avec `--all`, il
parcourt tout l'historique, et c'est ce qu'elle exécute sur une poussée vers `main` — seul
moment où le commit de **fusion squash** existe, puisqu'il est fabriqué après le dernier
passage de la CI sur la branche. Sur `main`, ce contrôle **constate** au lieu de prévenir,
et un commit de `main` ne s'amende pas. Il porte un pardon nommé, `806f7ae`, et l'annonce à
chaque exécution.

Il ignore les commits de **fusion**, qui n'apportent aucun contenu à certifier, et il les
compte pour le dire. Ce n'est pas une précaution théorique : sur un événement
`pull_request`, `actions/checkout` place un commit de fusion que **GitHub fabrique** pour
éprouver le résultat — « Merge <sha> into <base> », que personne n'a écrit et que personne
ne peut signer. Son premier passage en CI l'a trouvé. Une fusion **squash** n'en est pas
une : un seul parent, donc elle reste examinée.

Portée de `check-naming`, à connaître avant de s'y fier : il inspecte les **sites de
déclaration** et non les emplois, et son angle mort est la liaison d'un bras de `match`
— `Some(index) => …`, qu'il ne voit pas.

## Contrôles à passer en local avant de pousser

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
cargo xtask check-naming
cargo xtask check-versions          # compare à origin/main, arbre de travail compris
cargo xtask check-codec-coverage    # prend la mesure lui-même, ~1 min
cargo xtask check-dco               # les commits de la branche ; --all pour l'historique
cargo xtask check-licence-headers
cargo xtask check-safety-comments   # et la frontière unsafe de l'ADR-011
```

Ces contrôles tournent aussi en intégration continue sur chaque PR
(`.github/workflows/ci.yml`), sur Linux `x86_64` et macOS `aarch64`. La CI ne remplace
pas la liste locale : elle ne sait **pas** faire les essais en pseudo-terminal, qui
demandent un vrai terminal de contrôle, ni le fuzzing, qui exige nightly.

Et, quand la modification touche un codec (ADR-012) :

```sh
cargo llvm-cov --summary-only            # 100 % attendu sur codec/ et input/
cd fuzz && cargo +nightly fuzz run ansi_decoder -- -max_total_time=60
```

Et, quand la modification touche un thème ou l'importeur de skins (ADR-008) :

Le dépôt porte **trois** artefacts de thème depuis le jalon M7i, et le registre
`theme/skins/mod.rs` est **généré** à chaque import. Chacun se vérifie contre son propre
skin :

```sh
# le même fichier que celui qui a produit l'artefact — l'empreinte en en-tête le dit
cargo xtask import-mc-skin --skin /usr/local/share/mc/skins/default.ini --check
cargo xtask import-mc-skin --skin /usr/local/share/mc/skins/modarcon16-defbg.ini --check
cargo xtask import-mc-skin --skin /usr/local/share/mc/skins/seasons-autumn16M.ini --check
./target/debug/redit --list-themes   # les noms exacts à taper ; c'est cette liste qui fait foi
```

**Une arête à connaître avant de toucher à `theme/skins/`** : `xtask` dépend d'`air-tui`,
donc un registre désaccordé de ses fichiers empêche l'outil de se bâtir — et c'est l'outil
qui écrit le registre. Retirer un artefact à la main demande donc de corriger `mod.rs` à la
main **une fois**, puis de relancer un import pour que le registre soit régénéré. Payé deux
fois en écrivant M7i.

La même arête se présente par l'autre bout quand on **ajoute un rôle** à la palette : les
trois artefacts n'ont pas la nouvelle entrée, donc `air-tui` ne compile plus, donc `xtask`
non plus — et c'est `xtask` qui les régénère. La sortie est la même : ajouter l'entrée à la
main dans les trois fichiers, bâtir, puis relancer les trois imports, qui réécrivent ce
qu'il faut. Un `git diff` sur `theme/skins/` dit alors si la main avait vu juste ; au jalon
M7m, elle avait écrit exactement ce que l'outil a produit.

```sh
cargo build --example frame
tools/pty-drive.py ./target/debug/examples/frame mc -- 800 1b5b32317e   # doit sortir
tools/pty-drive.py ./target/debug/examples/frame mc -- 600 1b66         # doit tenir

# L'ombre portée du jalon M7j : gris sombre sur noir, et le `░` du bureau en dessous.
tools/pty-drive.py ./target/debug/examples/frame -- 800 1b5b32317e \
    | grep -oE '38;5;8;48;5;0' | wc -l
# doit donner 12, et autant avec `frame mc` : mc nomme son ombre `gray;black` dans
# `[core] shadow`, soit exactement l'attribut VGA 0x08 de Turbo Vision.

# L'ombre des boutons du jalon M7k : deux demi-blocs inférieurs, vingt-deux supérieurs —
# dix et douze colonnes de bouton. Comptés sur le glyphe, pas sur la couleur : le fond
# vient de ce qui est dessous, donc il change d'un conteneur à l'autre.
tools/pty-drive.py ./target/debug/examples/frame -- 800 1b5b32317e \
    | grep -oF '▀' | wc -l            # doit donner 22, et 2 pour '▄'
tools/pty-drive.py ./target/debug/examples/frame mc -- 800 1b5b32317e \
    | grep -oF '▀' | wc -l            # doit donner 0 : mc n'ombre pas ses boutons

# Et la couleur, une fois : noir sur le bleu de la fenêtre.
tools/pty-drive.py ./target/debug/examples/frame -- 800 1b5b32317e \
    | grep -oE '38;5;0;48;5;4m▄' | wc -l   # doit donner 2

# Dans le dialogue de `redit` — F3 pour l'ouvrir, Alt+A pour l'abandonner, F10 —, le fond
# est le gris du dialogue, donc l'octet est exactement l'attribut VGA 0x70 que Turbo Vision
# écrit pour cpButton[8]. Ici on cherche la **présence** et non un compte : le nombre de
# repeints d'une session en pty varie, et un compte en ferait un essai qui se contredit.
printf 'origine\n' > /tmp/redit-essai.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-essai.txt -- 600 1b5b31337e 600 1b61 600 1b5b32317e \
    | grep -oE '38;5;0;48;5;7m[▄▀]' | head -1   # doit sortir une occurrence
# `1b61` est Alt+A, la lettre d'accès rapide du bouton « Annuler » : un `1b` nu laisse le
# décodeur attendre la suite de la séquence, le dialogue reste ouvert et consomme le F10 —
# le verdict devient `timeout` alors que rien n'est cassé.
```

`redit` et `rmc` n'émettent **aucune ombre de fenêtre**, et ce n'est pas un défaut : leurs fenêtres
occupent tout le bureau, donc l'ombre tombe hors de lui et le découpage l'écarte — pour
`rmc`, celle du panneau de gauche tombe sur le panneau de droite, qui est dessiné après et
la recouvre. Une ombre ne paraît que là où il y a quelque chose derrière, et l'exemple
`frame` est le seul des trois à porter une fenêtre centrée.

Le chemin du skin dépend de l'installation : `/usr/local/share/mc/skins/` sous Homebrew,
`/usr/share/mc/skins/` sous Debian et Ubuntu. Le thème se vérifie sur les **octets émis**
et non à l'œil : la barre de menu passe de `38;5;0;48;5;7` à `38;5;15;48;5;6`, et le
bureau du `░` à l'espace.

Depuis le jalon M7g, les **trente-neuf** skins livrés avec `mc` 4.8.33 sont importables, et
deux d'entre eux servent à vérifier les capacités que ce jalon a ajoutées — un attribut par
rôle et la couleur du terminal. Les deux sont **compilés dans le dépôt** depuis le jalon
M7i, donc la mesure ne demande **rien d'autre que leur nom** : l'exemple `frame` ne connaît
que `mc` et `tv`, mais `rmc` et `redit` prennent `--theme mc:<skin>`.

```sh
cargo build -p rmc
mkdir -p /tmp/rmc-essai && printf 'abcd' > /tmp/rmc-essai/quatre.txt
tools/pty-drive.py ./target/debug/rmc --theme mc:modarcon16_defbg /tmp/rmc-essai \
    -- 700 1b5b32317e | grep -oE ';49m' | wc -l
# doit donner 14 : le fond du terminal, qui traverse palette, quantificateur et encodeur.
# `grep -c` donnerait 1 : la sortie d'un pty est une seule ligne, donc on compte les
# occurrences et non les lignes — c'est exactement le piège qui fait croire à un défaut.

COLORTERM=truecolor tools/pty-drive.py ./target/debug/rmc --theme mc:seasons_autumn16m \
    /tmp/rmc-essai -- 700 1b5b32317e | grep -oE $'\033\\[0;4;38;2;[0-9;]*m' | head -1
# doit donner un souligné sur une vraie couleur : la lettre d'accès rapide d'un menu
```

Cette procédure en a remplacé une qui demandait d'**importer, bâtir, mesurer, puis
rétablir l'artefact** — nécessaire avant M7i, quand un seul thème compilé tenait dans le
dépôt et que chaque import l'écrasait. Elle était restée écrite ici trois jalons de trop,
et elle nommait un fichier qui **n'existe plus** : `theme/skins/midnight_commander.rs`.
L'importeur écrit le fichier qui porte le **nom du skin** — `default.rs` pour
`default.ini` —, et il régénère le registre `theme/skins/mod.rs` à chaque fois. Si une
mesure demande malgré tout d'importer un skin, c'est **ce** fichier qu'il faut rétablir par
`git checkout`, et retirer celui qu'un skin inconnu du dépôt aurait créé : un artefact
d'essai committé par distraction changerait un thème sans qu'aucun contrôle le dise, puisque
`--check` le trouverait cohérent avec son propre skin.

Pour compter les skins importables, `--check` suffit et distingue les deux cas : un message
qui parle d'« empreinte » ou de « ne correspond plus » veut dire que le skin **a compilé**
mais n'est pas celui de l'artefact ; toute autre erreur est un refus de compilation.
Attention, la commande doit passer par `cargo run` et non par le binaire : `repository_root`
lit `CARGO_MANIFEST_DIR`, et sans elle les trente-neuf skins paraissent tous refusés pour
une raison qui n'a rien à voir avec eux.

Et, quand la modification touche `unicode/` ou les tables générées :

```sh
cargo xtask download-ucd --version 18.0.0 --output-directory /tmp/ucd
cargo xtask generate-unicode-tables --ucd-directory /tmp/ucd --check
cargo xtask check-segmentation --ucd-directory /tmp/ucd   # 853 + 1944 cas, aucun écart
```

Les fichiers d'essais du Consortium ne sont pas committés : ce sont des données du
référentiel. La vérification est donc une **commande** et non un `#[test]`, pour qu'elle ne
puisse pas passer en silence faute de données. C'est elle qui a trouvé que `GB9c` avait
changé en Unicode 18.0.0 alors que le dépôt portait encore la formulation antérieure.

Le fuzzing exige nightly ; `fuzz/` est un espace de travail séparé pour cette raison, et
le dépôt reste pinné sur stable.

Et, quand la modification touche la couche 0 ou la boucle d'événements, **vérifier dans un
vrai pseudo-terminal** — compiler ne prouve rien pour une application qui a besoin d'un
terminal :

```sh
cargo build --example frame
tools/pty-drive.py ./target/debug/examples/frame 800 1b66 500 1b5b32317e   # doit sortir
tools/pty-drive.py ./target/debug/examples/frame 600 1b66                  # doit tenir

cargo build -p redit
printf 'origine\n' > /tmp/redit-essai.txt
# taper « ABC », F2 pour enregistrer, F10 pour sortir
tools/pty-drive.py ./target/debug/redit /tmp/redit-essai.txt -- 400 414243 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-essai.txt     # doit contenir « ABCorigine »
tools/pty-drive.py ./target/debug/redit /tmp/redit-essai.txt -- 600 1b66   # doit tenir

# Maj+Gauche deux fois, Ctrl+C, Fin, Ctrl+V, F2, F10 : le fichier doit porter « abZab »
printf 'Z\n' > /tmp/redit-selection.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-selection.txt -- \
    500 6162 300 1b5b313b3244 300 1b5b313b3244 300 03 300 1b5b46 300 16 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-selection.txt     # doit contenir « abZab »

# Ctrl+Z annule la frappe, Ctrl+R la rétablit. Les deux essais vont **par paire** : le
# contrôle du rétablissement est l'essai d'annulation seule, et non un essai sans touche —
# sans Ctrl+Z, le fichier porte déjà « ABCorigine », donc un rétablissement qui ne marcherait
# pas donnerait le même fichier qu'un rétablissement qui marche.
printf 'origine\n' > /tmp/redit-annule.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-annule.txt -- \
    500 414243 400 1a 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-annule.txt     # doit contenir « origine » : la frappe est défaite

printf 'origine\n' > /tmp/redit-annule.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-annule.txt -- \
    500 414243 400 1a 400 12 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-annule.txt     # doit contenir « ABCorigine » : la frappe est rétablie

# « ab », Début, « z », Ctrl+Z : le déplacement a coupé le geste, donc seul le « z » part
printf 'origine\n' > /tmp/redit-geste.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-geste.txt -- \
    500 6162 300 1b5b48 300 7a 400 1a 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-geste.txt      # doit contenir « aborigine », et non « origine »

# F7, « belle », Entrée, Ctrl+X, F2, F10 : la recherche sélectionne l'occurrence, donc
# Ctrl+X la coupe — c'est la preuve la plus directe qu'elle a sélectionné quelque chose.
printf 'une belle journee\n' > /tmp/redit-recherche.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-recherche.txt -- \
    500 1b5b31387e 400 62656c6c65 400 0d 400 18 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-recherche.txt   # doit contenir « une  journee »
# le contrôle : un motif absent ne sélectionne rien, donc Ctrl+X ne coupe rien
printf 'une belle journee\n' > /tmp/redit-recherche.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-recherche.txt -- \
    500 1b5b31387e 400 78797a 400 0d 400 18 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-recherche.txt   # doit contenir « une belle journee », intact

# Et Ctrl+L va bien à l'occurrence **suivante** : sans lui, c'est la première qui serait
# coupée, et les deux essais donneraient des fichiers différents du même programme.
printf 'une belle journee\nune belle soiree\n' > /tmp/redit-suivante.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-suivante.txt -- \
    500 1b5b31387e 400 62656c6c65 400 0d 400 0c 400 18 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-suivante.txt    # la seconde ligne doit porter « une  soiree »

# double clic sur « belle » puis Ctrl+X : le mot est coupé, le fichier porte « une  journee »
printf 'une belle journee\n' > /tmp/redit-double.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-double.txt -- \
    600 1b5b3c303b363b334d1b5b3c303b363b334d 400 18 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-double.txt     # doit contenir « une  journee »
# le contrôle : un **seul** clic ne sélectionne rien, donc Ctrl+X ne coupe rien
printf 'une belle journee\n' > /tmp/redit-double.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-double.txt -- \
    600 1b5b3c303b363b334d 400 18 400 1b5b31327e 600 1b5b32317e

# Un menu ouvert est modal pour les lettres (M7m) : Alt+E, `z`, F2, F10 — et son contrôle,
# la même lettre sans menu ouvert. Attention à ne **pas** mettre un `1b` nu pour refermer le
# menu : le décodeur attendrait la suite et avalerait le F2 qui suit.
printf 'origine\n' > /tmp/redit-menu.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-menu.txt -- \
    500 1b65 400 7a 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-menu.txt    # doit contenir « origine » : la lettre n'a pas atteint le texte
printf 'origine\n' > /tmp/redit-menu.txt
tools/pty-drive.py ./target/debug/redit /tmp/redit-menu.txt -- \
    500 7a 400 1b5b31327e 600 1b5b32317e
cat /tmp/redit-menu.txt    # doit contenir « zorigine » : sans menu, elle s'insère

cargo build -p rmc
mkdir -p /tmp/rmc-essai/sous-repertoire && printf 'abcd' > /tmp/rmc-essai/quatre.txt
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- 700 1b5b32317e      # doit sortir
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- 600 09              # doit tenir
# Tab, flèche bas, Inser, F10 : le panneau de droite devient actif, un item est marqué
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- 500 09 300 1b5b42 300 1b5b327e 700 1b5b32317e

# Le format bref du jalon M7m, et son contrôle. `1b70` est Alt+P — le menu « Panneau » —,
# puis `66` est la lettre d'accès rapide de « Format d'affichage », qui l'active.
# Le séparateur de colonnes est le même glyphe que le trait vertical d'un cadre, donc
# c'est la **différence** qui se mesure, et elle vaut une ligne de panneau :
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- 700 1b5b32317e \
    | grep -oF '│' | wc -l
# doit donner 84 : deux fenêtres, deux bords verticaux, vingt-et-une lignes
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- 500 1b70 400 66 700 1b5b32317e \
    | grep -oF '│' | wc -l
# doit donner 105 : vingt-et-un séparateurs de plus, un par ligne du panneau de gauche

# Et la couleur, en thème Turbo Vision : `cpAppColor[60]`, bleu sur le cyan de la liste,
# que Borland nomme « List divider ». Ici on cherche la **présence** et non un compte :
# l'encodeur est différentiel, donc le nombre d'émissions d'un attribut dépend de ce qui
# précède chaque cellule.
tools/pty-drive.py ./target/debug/rmc --theme tv /tmp/rmc-essai \
    -- 500 1b70 400 66 700 1b5b32317e | grep -oE '38;5;4;48;5;6m│' | head -1
# doit sortir une occurrence, et **rien** sans la bascule : le format complet n'a qu'une
# colonne, donc aucun séparateur.
```

La sortie de `rmc` se lit en retirant les séquences d'échappement : les noms de fichiers
sont écrits cellule par cellule, donc `grep quatre.txt` sur la sortie brute ne trouve
rien alors que l'écran les porte. Ce qui se vérifie sur les octets, c'est la **couleur** :
`38;5;11;48;5;6` est l'item marqué sous le curseur du skin de `mc`, et son absence après
un `Inser` dirait que le marquage ne traverse pas le modèle.

Deux pièges à la lecture d'une sortie de `rmc`, tous deux payés au jalon M7c :

- **le chemin doit être court.** Le titre d'un panneau est tronqué à la moitié de la
  largeur : sous un chemin de bloc-notes temporaire, `[1 marqué — 2 o]` n'est jamais
  émis, et son absence ressemble à un défaut alors que c'est un titre coupé ;
- **un motif trouvé n'est pas l'état final.** Chaque frappe produit un repeint, et la
  sortie les porte tous. Un `[1 marqué — 2 o]` au troisième repeint sur quatre dit que la
  marque a existé puis **disparu** — c'est exactement la signature du défaut. Compter les
  repeints et situer le motif dans le dernier, sinon le verdict s'inverse.

Et, pour les relâchements de touche : un pty nu ne négocie pas le protocole clavier, mais
il n'a pas à le faire — **il suffit d'émettre les octets**, le décodeur décode ce qu'il
reçoit. `1b5b3b313a3342` est `\x1b[;1:3B`, un relâchement de flèche bas dans la forme que
`kitty` émet réellement (numéro de touche omis), et `1b5b323b313a337e` est `\x1b[2;1:3~`,
un relâchement d'`Inser` :

```sh
mkdir -p /tmp/rmc-essai/sous-repertoire
printf 'ab' > /tmp/rmc-essai/deux.txt && printf 'abcd' > /tmp/rmc-essai/quatre.txt
# bas, bas, bas RELÂCHÉE, Inser, Inser RELÂCHÉ, F10 : le titre doit finir sur « 1 marqué — 2 o »
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- \
    500 1b5b42 200 1b5b42 300 1b5b3b313a3342 300 1b5b327e 300 1b5b323b313a337e 700 1b5b32317e
```

« 4 o » dirait que la flèche relâchée a descendu le curseur ; aucune marque au dernier
repeint, que le relâchement d'`Inser` a démarqué. Les deux étaient vrais avant M7c.

Attention aux codes : `1b5b31327e` est `F2`, `1b5b31337e` est `F3`, `1b5b31387e` est `F7`,
`1b5b32317e` est `F10`, `1b5b313b3244` est `Maj`+Gauche, `1b5b48` est `Début`, `0d` est
`Entrée`, `03` est `Ctrl`+`C`, `16` est `Ctrl`+`V`, `18` est `Ctrl`+`X`, `1a` est
`Ctrl`+`Z`, `12` est `Ctrl`+`R` et `0c` est `Ctrl`+`L`. `1b70` est `Alt`+`P`, `1b65` est
`Alt`+`E`, et une lettre nue — `66` pour `f`, `7a` pour `z` — active l'item du menu ouvert
qui la porte, ou se fait avaler par ce menu si elle n'en désigne aucun, depuis le jalon
M7m. `Ctrl`+`Z`
atteint bien l'application et ne la suspend pas : `cfmakeraw` efface `ISIG`. `1b5b3c303b363b334d` est un enfoncement du bouton gauche à la colonne 6,
ligne 3 au format SGR 1006 — **comptées depuis un**, et la vue d'édition de `redit`
commence à la colonne 1, ligne 2 de l'écran, sous la barre de menu et dans le cadre de sa
fenêtre. Se tromper d'une cellule fait cliquer dans le cadre, et le geste n'a plus d'effet. Se tromper de touche donne un faux négatif — `F3` ouvre un dialogue modal qui
consomme le `F10` qui suit, et le verdict devient `timeout` alors que tout fonctionne.

Les **deux** essais comptent : sans l'essai de contrôle, un programme qui sort pour une
toute autre raison donne le même verdict que celui qui obéit à la touche. C'est ainsi que
le `POLLNVAL` de `/dev/tty` sur macOS est resté caché jusqu'au jalon M5b.

Sur la couverture, deux exigences de portée différente, à ne pas confondre :

- **100 % sur les codecs**, régions comprises — c'est ADR-012, et c'est tenu aujourd'hui
  sur `codec/` et `input/` ;
- un objectif élevé ailleurs, **non atteint** : `air-tui-platform-posix` demande un vrai
  terminal pour être couvert. Ne pas annoncer un seuil global qui n'existe pas.

## `Alt`+lettre sur macOS : c'est au terminal de le dire

Signalé à l'usage le 2026-10-12, et **ce n'est pas un défaut du code** : sans réglage,
macOS emploie `Option` pour composer un caractère. `Option`+`F` produit un `ƒ`, le terminal
n'annonce aucun modificateur, et tous les `Alt`+lettre de `redit` et de `rmc` sont inertes.
C'est le défaut de Kitty, qui l'écrit dans son propre fichier de configuration : « *With
this set to `no`, kitty will use the macOS native Option+Key to enter Unicode character
behavior. This will break any Alt+Key keyboard shortcuts in your terminal programs.* »

Le contrôle : `cat -v`, puis `Alt`+`F`. `^[f` veut dire que le terminal envoie `Alt` ; `ƒ`
veut dire qu'il compose. Les réglages : `macos_option_as_alt yes` — ou `left` — dans
`kitty.conf` ; « Utiliser la touche Option comme touche Meta » dans Terminal.app ; `Esc+`
pour *Left Option key* dans iTerm2.

Les quatre encodages mesurés en pty sur `redit`, en comptant le coin du déroulant — une
couleur que seule la boîte de menu émet :

```sh
printf 'origine\n' > /tmp/redit-alt.txt
for codes in 1b66 1b5b3130323b3375 1b5b3430323a303a3130323b3375 1b5b3430323a303a31303275
do
    tools/pty-drive.py ./target/debug/redit /tmp/redit-alt.txt -- 500 "$codes" \
        400 1b5b32317e | grep -oE '38;5;0;48;5;7m┌' | wc -l
done
# doit donner 1, 1, 1 puis 0 — et 0 aussi sans aucune frappe, ce qui est le contrôle
```

| ce que le terminal envoie | le menu s'ouvre |
|---|---|
| `1b66` — `ESC f`, l'`Alt` à l'ancienne | **oui** |
| `1b5b3130323b3375` — `CSI 102;3u`, protocole Kitty | **oui** |
| `1b5b3430323a303a3130323b3375` — `CSI 402:0:102;3u`, le `ƒ` composé **avec** `Alt` et la disposition de base | **oui** |
| `1b5b3430323a303a31303275` — `CSI 402:0:102u`, le `ƒ` seul | **non** |

Le troisième cas est celui qui prouve que le framework fait sa part : `shortcut_key()`
retombe sur la disposition de base, le mécanisme de l'ADR-003 écrit pour qu'`Alt`+`ф` ouvre
le menu `a` sur un clavier cyrillique. Le quatrième est **indécidable** — un `Option`+`F`
qui n'annonce rien est exactement la frappe de quelqu'un qui tape un `ƒ` —, et il n'y a
donc rien à corriger sans casser la saisie de ce caractère.

Les chemins qui restent ouverts sans ce réglage : le **clic** sur la barre de menu, les
flèches et `Entrée`, et la lettre de l'item une fois le déroulant ouvert (jalon M7m).

## Git

- Trunk-based. `main` protégée : pas de push direct, tout passe par une PR.
- `git commit -s` obligatoire (DCO). Signature GPG sur les commits de `main`.
- Conventional Commits : `feat`, `fix`, `docs`, `chore`, `test`, `refactor`.
- Jamais de force-push sur `main`.

## Le `Signed-off-by` d'une fusion squash

Une fusion squash produit **un** commit sur `main`, et c'est lui qui doit porter le DCO.
Sans `--body`, GitHub construit le corps en reprenant les messages des commits de la
branche, trailers compris : le `Signed-off-by` arrive tout seul. Avec un `--body` écrit à
la main, il **disparaît**, et c'était silencieux jusqu'à ce que `check-dco` soit écrit —
il l'est depuis, et c'est lui qui le verrait maintenant.

Donc, quand on passe un corps à la main, y remettre la ligne :

```sh
gh pr merge <n> --squash --delete-branch \
    --subject "feat!: …" \
    --body "…

Signed-off-by: Thierry DELHAISE <thierry.delhaise@air-desktop.org>"
```

Payé au jalon M7g : son commit de fusion (`806f7ae`) est le seul de `main` sans DCO. Il
n'est **pas réparable** — amender demanderait un force-push sur `main`, que ce dépôt
interdit sans exception. Une règle outillée aurait rattrapé la distraction : `check-dco`
l'est depuis, et il porte ce commit comme **pardon nommé**, annoncé à chaque exécution.
