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
| `crates/air-tui-platform-posix` | implémentation POSIX — le seul `unsafe`, le seul C | 0.1.1 |
| `crates/air-tui` | le cœur, dépend uniquement du contrat | 0.6.0 |
| `crates/air-tui-platform-air` | implémentation `linux-air` | **à écrire** |
| `fuzz/` | cibles `cargo-fuzz`, espace de travail séparé (nightly) | `ansi_decoder` |
| `xtask/` | outillage du dépôt, en Rust | génération des tables Unicode |
| `tools/` | outillage qui ne peut pas être en Rust sans un appel C de plus | `pty-drive.py` |

## Les modules d'`air-tui`

| Module | Rôle | Jalon |
|---|---|---|
| `geometry` | colonnes, lignes, points, dimensions, rectangles | M0 |
| `cell` | couleur, attributs, cellule, continuation d'un glyphe large | M0 |
| `surface` | la grille, le différentiel, le suivi des zones salies | M0 |
| `terminal` | `AirTerminal` sur le contrat : rendu, entrée, curseur, protocole clavier | M0, M2, M5b |
| `codec` | encodeur SGR, écriture décimale — à 100 % de couverture | M0 |
| `input` | décodeur ANSI, protocole Kitty, modèle de touche et de souris | M1, M2 |
| `unicode` | largeur d'affichage, graphèmes (UAX #29), tables générées | M3 |
| `view` | trait `AirView`, peintre à découpage réel, trois phases, contexte de dessin | M4, M5b |
| `command` | `AirCommandSet` — le grisage en *pull* | M4 |
| `theme` | palette par rôles, glyphes, quantificateur de couleurs | M5 |
| `widgets` | panneau, étiquette, fenêtre, bouton, barre de menu, barre de statut, champ de saisie | M5, M5b |

## Ce qui reste à produire

Les modules annoncés dans l'architecture et **non encore écrits** : `input/evdev` et
`input/vt` (M8), `mvc/` — le journal de changements d'ADR-007 —, `AirApplication`,
`AirScreen` et `AirDesktop`, le thème Midnight Commander et son importeur de skins (M7),
les applications `redit` et `rmc`, et la crate `air-tui-platform-air`.
