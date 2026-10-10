# Index du dépôt air-tui

Carte structurelle : ce qui existe, où le trouver, ce qui reste à produire.
L'**avancement** vit dans [`etat-avancement.md`](etat-avancement.md), l'**historique** dans
[`JOURNAL.md`](JOURNAL.md).

## Racine

| Fichier | Rôle |
|---|---|
| `README.md` | présentation publique du projet |
| `AGENTS.md` | conventions de travail pour tout agent |
| `CLAUDE.md` | les mêmes, plus le workflow local |
| `Cargo.toml` | espace de travail, lints partagés |
| `rust-toolchain.toml` | toolchain pinnée et datée (ADR-025) |
| `deny.toml` | bornes de licences et de sources |
| `LICENSE` | MPL-2.0 |
| `NOTICE` | attribution des données Unicode incorporées |
| `.cargo/config.toml` | l'alias `cargo xtask` |
| `.github/workflows/ci.yml` | l'intégration continue : les contrôles exigés sur chaque PR |

## Documentation

| Fichier | Rôle |
|---|---|
| `docs/ARCHITECTURE-fr.md` | l'architecture d'ensemble |
| `docs/CONVENTIONS-fr.md` | conventions de code, nommage, exceptions |
| `docs/etat-avancement.md` | document de suivi vivant |
| `docs/JOURNAL.md` | archive append-only, anté-chronologique |
| `docs/adrs/` | décisions techniques, contraignantes |
| `docs/adrs/statuts.toml` | avancement de chaque ADR |

## Crates

| Crate | Rôle | État |
|---|---|---|
| `crates/air-tui-platform` | le contrat : traits, types portables | 0.1.0 |
| `crates/air-tui-platform-posix` | implémentation POSIX — le seul `unsafe`, le seul C | 0.2.0 |
| `crates/air-tui` | le cœur, dépend uniquement du contrat | 0.11.0 |
| `crates/redit` | l'éditeur de texte — le seul endroit qui choisit une plateforme | 0.2.3 |
| `crates/rmc` | la coquille à deux panneaux — même règle pour la plateforme | 0.1.5 |
| `crates/air-tui-platform-air` | implémentation `linux-air` | **à écrire** |
| `fuzz/` | cibles `cargo-fuzz`, espace de travail séparé (nightly) | `ansi_decoder` |
| `xtask/` | outillage du dépôt, en Rust | 0.4.0 — tables Unicode, conformité des segmentations, les trois contrôles de conventions, et l'importeur de skins MC |
| `tools/` | outillage qui ne peut pas être en Rust sans un appel C de plus | `pty-drive.py` |

## Les modules d'`air-tui`

| Module | Rôle | Jalon |
|---|---|---|
| `geometry` | colonnes, lignes, points, dimensions, rectangles | M0 |
| `cell` | couleur — index, RGB, **défaut du terminal** —, attributs, cellule, continuation d'un glyphe large | M0, M7g |
| `surface` | la grille, le différentiel, le suivi des zones salies | M0 |
| `terminal` | `AirTerminal` sur le contrat : rendu, entrée, curseur, protocole clavier | M0, M2, M5b |
| `codec` | encodeur SGR, écriture décimale — à 100 % de couverture | M0 |
| `input` | décodeur ANSI, protocole Kitty, modèle de touche et de souris, **synthèse des clics multiples** | M1, M2, M7e |
| `unicode` | largeur d'affichage, graphèmes **et mots** (UAX #29), tables générées | M3, M6 |
| `view` | trait `AirView`, peintre à découpage réel, trois phases, contexte de dessin, accès typé | M4, M5b, M6 |
| `command` | `AirCommandSet` — le grisage en *pull* | M4 |
| `clipboard` | `AirClipboard` — le presse-papiers interne, possédé par le contrôleur | M7d |
| `theme` | palette par rôles (trente-huit, **avec attributs**), glyphes, quantificateur, **deux thèmes** | M5, M7, M7g |
| `theme::skins` | les thèmes compilés depuis un skin de Midnight Commander — **généré** | M7 |
| `widgets` | panneau, étiquette, fenêtre, bouton, barre de menu, barre de statut, champ de saisie, ascenseur, vue d'édition, dialogue de saisie, liste, **cases à cocher et boutons radio** | M5, M5b, M6, M7b, M7f |
| `model` | `AirModel`, `AirChangeSet`, `AirGeneration`, `AirTextBuffer` et sa **sélection** — la couche MVC | M6, M7d |
| `application` | `AirApplication`, `AirScreen`, `AirDesktop` — la boucle et le chrome | M6 |

## Les modules de `xtask`

| Module | Rôle |
|---|---|
| `main` | la ligne de commande : sept sous-commandes |
| `ucd` | la lecture des fichiers du référentiel Unicode |
| `unicode_tables` | la génération du fichier de tables, et sa vérification à l'octet |
| `segmentation_conformance` | la confrontation aux fichiers d'essais de l'annexe 29 |
| `naming` | `check-naming` — le balayage lexical des sites de déclaration (ADR-004) |
| `mc_skin` | le décodeur des skins de Midnight Commander — à 100 % de couverture (ADR-012) |
| `mc_theme` | la table de correspondance vers les rôles d'air-tui, et l'émetteur de l'artefact |
| `versions` | `check-versions` — l'incrément par crate et la dérive des dépendances (ADR-009) |
| `codec_coverage` | `check-codec-coverage` — les 100 % exigés des codecs (ADR-012) |

## Les modules de `rmc`

| Module | Rôle |
|---|---|
| `main` | la ligne de commande et la boucle |
| `directory` | le modèle d'un panneau : un répertoire, ses entrées, ce qui y est marqué |
| `shell` | le contrôleur : deux panneaux, le focus, les quatre étapes de l'ADR-007 |

## Les modules de `redit`

| Module | Rôle |
|---|---|
| `main` | la ligne de commande et la boucle |
| `document` | le tampon, son chemin, et les deux accès au disque |
| `editor` | le contrôleur : l'arbre, les quatre étapes de l'ADR-007, les commandes |

## Ce qui reste à produire

Les modules annoncés dans l'architecture et **non encore écrits** : `input/evdev` et
`input/vt` (M8), `AirDecorations` — la troisième assiette du triplet de l'ADR-008, que
l'ombre portée demande et que le découpage du peintre interdit aujourd'hui —, et la crate
`air-tui-platform-air`.

Côté widgets, les absences mesurées : une **liste à plusieurs colonnes**, et le
**défilement d'un groupe** d'items plus long que sa place. Les cases à cocher, les boutons
radio et la sélection de texte sont faits — M7f et M7d.
