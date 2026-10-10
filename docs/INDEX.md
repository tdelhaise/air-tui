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
| `crates/air-tui-platform` | le contrat : traits, types portables | socle M0 |
| `crates/air-tui-platform-posix` | implémentation POSIX — le seul `unsafe`, le seul C | socle M0 |
| `crates/air-tui` | le cœur, dépend uniquement du contrat | socle M0 |
| `crates/air-tui-platform-air` | implémentation `linux-air` | **à écrire** |
| `fuzz/` | cibles `cargo-fuzz`, espace de travail séparé (nightly) | `ansi_decoder` |
| `xtask/` | outillage du dépôt, en Rust | génération des tables Unicode |

## Ce qui reste à produire

Les modules annoncés dans l'architecture et **non encore écrits** : `input/evdev` et
`input/vt` (M8), `mvc/`, (tables UCD générées), `view/` (arbre, trois phases,
focus, découpage, bordures), `widgets/`, `theme/` au-delà du minimum de M0, `mvc/`, les
applications d'exemple `redit` et `rmc`, la crate `air-tui-platform-air`, et `xtask/`.
