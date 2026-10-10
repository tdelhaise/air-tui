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

Aucun gate `cargo xtask` n'est écrit à ce jour. Les contrôles listés dans
`docs/CONVENTIONS-fr.md` sous « à outiller » sont à exécuter **à la main**. Ne pas écrire
qu'ils tournent avant qu'ils tournent (NO GO n° 1).

## Contrôles à passer en local avant de pousser

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
```

Et, quand la modification touche un codec (ADR-012) :

```sh
cargo llvm-cov --summary-only            # 100 % attendu sur codec/ et input/
cd fuzz && cargo +nightly fuzz run ansi_decoder -- -max_total_time=60
```

Le fuzzing exige nightly ; `fuzz/` est un espace de travail séparé pour cette raison, et
le dépôt reste pinné sur stable.

Et, quand la modification touche la couche 0 ou la boucle d'événements, **vérifier dans un
vrai pseudo-terminal** — compiler ne prouve rien pour une application qui a besoin d'un
terminal :

```sh
cargo build --example frame
tools/pty-drive.py ./target/debug/examples/frame 800 1b66 500 1b5b32317e   # doit sortir
tools/pty-drive.py ./target/debug/examples/frame 600 1b66                  # doit tenir
```

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
