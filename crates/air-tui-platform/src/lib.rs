// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrat de plateforme d'air-tui — des traits, et rien d'autre.
//!
//! # Pourquoi cette crate existe
//!
//! air-tui doit tourner aujourd'hui sur Linux et macOS à travers POSIX, et demain sur la
//! cible `linux-air`, qui est intégralement en Rust et dont la couche 0 n'est pas
//! appelable depuis un framework (ADR-011). Ces deux mondes n'ont rien en commun au
//! niveau des appels système.
//!
//! La réponse est une **couture par traits** : cette crate décrit ce dont air-tui a
//! besoin, sans jamais dire comment l'obtenir. Chaque plateforme fournit une crate
//! d'implémentation, et le cœur du framework ne dépend que du contrat.
//!
//! ```text
//!              air-tui              ← le cœur : ne connaît que ce contrat
//!                 │
//!        air-tui-platform           ← cette crate : des traits
//!                 ▲
//!       ┌─────────┴──────────┐
//! air-tui-platform-posix     air-tui-platform-air   ← à écrire, pour linux-air
//! (Linux glibc, macOS)       (Rust intégral)
//! ```
//!
//! Conséquence voulue : `air-tui` **ne peut pas** atteindre la couche système, même par
//! accident. Elle n'en a pas la dépendance. C'est le binaire final qui choisit une
//! implémentation et l'injecte.
//!
//! # Ce que le contrat couvre
//!
//! - [`TerminalDevice`] — le périphérique : mode brut, taille, attente, lecture, écriture.
//! - [`Decoder`] et [`Encoder`] — les codecs, seule porte autorisée entre un flux d'octets
//!   et des éléments typés (ADR-012).
//! - [`PlatformError`] — une erreur portable, qui ne suppose pas l'existence d'un `errno`.

#![forbid(unsafe_code)]
// `expect` et `unwrap` sont interdits en production (lints de l'espace de travail) et
// légitimes dans un test, où une précondition non tenue doit faire échouer le test
// bruyamment.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod codec;
pub mod error;
pub mod terminal;

pub use codec::{Decoder, Encoder};
pub use error::{PlatformError, PlatformErrorKind};
pub use terminal::{InputReadiness, TerminalDevice, WindowSize};
