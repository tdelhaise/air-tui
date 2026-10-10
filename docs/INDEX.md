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

| Crate | Couche | État |
|---|---|---|
| `crates/air-tui-sys` | 0 — `unsafe` et appels C | squelette M0 |
| `crates/air-tui` | 1 — Rust pur | squelette M0 |

## Ce qui reste à produire

Les modules annoncés dans l'architecture et **non encore écrits** : `input/` (parseur ANSI,
Kitty, evdev, VT), `unicode/` (tables UCD générées), `view/` (arbre, trois phases, focus),
`widgets/`, `theme/` au-delà du minimum de M0, `mvc/`, les applications d'exemple
`redit` et `airmc`, et `xtask/`.
