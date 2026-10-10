// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! air-tui — un framework d'interface en mode texte, sans aucune dépendance.
//!
//! # État
//!
//! **Préliminaire.** Seul le socle du jalon M0 existe : la géométrie, la cellule, la
//! surface différentielle, le terminal en RAII et un thème minimal. L'arbre de vues, les
//! widgets, le parseur d'entrée et la couche MVC ne sont **pas** écrits — voir
//! `docs/etat-avancement.md`.
//!
//! # Architecture
//!
//! Cette crate est le cœur du framework. Elle est en `#![forbid(unsafe_code)]` et ne
//! dépend **que** du contrat `air-tui-platform` : elle n'a aucune dépendance vers une
//! implémentation de plateforme, et ne peut donc pas atteindre le système, même par
//! accident (ADR-002, ADR-011). C'est le binaire final qui choisit un
//! [`TerminalDevice`] et l'injecte.
//!
//! # Conventions de nommage
//!
//! Les types du modèle d'objets public portent le préfixe `Air` — `AirWindow`,
//! `AirView`, `AirRectangle` — à la manière de Qt (`QWidget`) ou de Cocoa (`NSWindow`),
//! et aucun identifiant n'est abrégé : `column`, jamais `col` ; `foreground`, jamais
//! `fg` (ADR-004).

#![forbid(unsafe_code)]
// `expect` et `unwrap` sont interdits en production (lints de l'espace de travail) et
// légitimes dans un test, où une précondition non tenue doit faire échouer le test
// bruyamment.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod cell;
pub mod codec;
pub mod geometry;
pub mod surface;
pub mod terminal;
pub mod theme;

pub use air_tui_platform::{InputReadiness, PlatformError, PlatformErrorKind, TerminalDevice};
pub use cell::{AirAttributes, AirCell, AirColor};
pub use codec::{AirCellPresentation, AirSgrEncoder};
pub use geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
pub use surface::AirSurface;
pub use terminal::AirTerminal;
pub use theme::AirTheme;
