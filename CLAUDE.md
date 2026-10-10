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

## Ce qui n'existe pas encore

`cargo xtask` porte **six** commandes de vérification — `generate-unicode-tables --check`,
`check-segmentation`, `check-naming` (ADR-004), `check-versions` (ADR-009),
`check-codec-coverage` (ADR-012) et `import-mc-skin --check` (ADR-008). L'intégration
continue en exécute **cinq** : celle de l'importeur de skins demande le même fichier de
skin que celui qui a produit l'artefact, donc le même `mc` installé, et un coureur n'en a
pas. Elle reste locale, et l'empreinte que l'artefact porte en en-tête la rend
vérifiable : elle dit « ce n'est pas le même skin » plutôt que « l'artefact a dérivé ».

Trois contrôles de `docs/CONVENTIONS-fr.md` restent **à écrire**, donc à tenir à la main :
`check-licence-headers`, `check-safety-comments` et `check-dco`. Ne pas écrire qu'ils
tournent avant qu'ils tournent (NO GO n° 1).

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

```sh
# le même fichier que celui qui a produit l'artefact — l'empreinte en en-tête le dit
cargo xtask import-mc-skin --skin /usr/local/share/mc/skins/default.ini --check
cargo build --example frame
tools/pty-drive.py ./target/debug/examples/frame mc -- 800 1b5b32317e   # doit sortir
tools/pty-drive.py ./target/debug/examples/frame mc -- 600 1b66         # doit tenir
```

Le chemin du skin dépend de l'installation : `/usr/local/share/mc/skins/` sous Homebrew,
`/usr/share/mc/skins/` sous Debian et Ubuntu. Le thème se vérifie sur les **octets émis**
et non à l'œil : la barre de menu passe de `38;5;0;48;5;7` à `38;5;15;48;5;6`, et le
bureau du `░` à l'espace.

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

cargo build -p rmc
mkdir -p /tmp/rmc-essai/sous-repertoire && printf 'abcd' > /tmp/rmc-essai/quatre.txt
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- 700 1b5b32317e      # doit sortir
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- 600 09              # doit tenir
# Tab, flèche bas, Inser, F10 : le panneau de droite devient actif, un item est marqué
tools/pty-drive.py ./target/debug/rmc /tmp/rmc-essai -- 500 09 300 1b5b42 300 1b5b327e 700 1b5b32317e
```

La sortie de `rmc` se lit en retirant les séquences d'échappement : les noms de fichiers
sont écrits cellule par cellule, donc `grep quatre.txt` sur la sortie brute ne trouve
rien alors que l'écran les porte. Ce qui se vérifie sur les octets, c'est la **couleur** :
`38;5;11;48;5;6` est l'item marqué sous le curseur du skin de `mc`, et son absence après
un `Inser` dirait que le marquage ne traverse pas le modèle.

Attention aux codes : `1b5b31327e` est `F2`, `1b5b31337e` est `F3`, `1b5b32317e` est
`F10`. Se tromper de touche donne un faux négatif — `F3` ouvre un dialogue modal qui
consomme le `F10` qui suit, et le verdict devient `timeout` alors que tout fonctionne.

Les **deux** essais comptent : sans l'essai de contrôle, un programme qui sort pour une
toute autre raison donne le même verdict que celui qui obéit à la touche. C'est ainsi que
le `POLLNVAL` de `/dev/tty` sur macOS est resté caché jusqu'au jalon M5b.

Sur la couverture, deux exigences de portée différente, à ne pas confondre :

- **100 % sur les codecs**, régions comprises — c'est ADR-012, et c'est tenu aujourd'hui
  sur `codec/` et `input/` ;
- un objectif élevé ailleurs, **non atteint** : `air-tui-platform-posix` demande un vrai
  terminal pour être couvert. Ne pas annoncer un seuil global qui n'existe pas.

## Git

- Trunk-based. `main` protégée : pas de push direct, tout passe par une PR.
- `git commit -s` obligatoire (DCO). Signature GPG sur les commits de `main`.
- Conventional Commits : `feat`, `fix`, `docs`, `chore`, `test`, `refactor`.
- Jamais de force-push sur `main`.
