// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les codecs d'air-tui.
//!
//! Toute conversion d'un flux de données passe par un codec, et **tout codec est tenu à
//! 100 % de couverture de code** (ADR-012). Les traits [`air_tui_platform::Decoder`] et
//! [`air_tui_platform::Encoder`] portent le contrat.
//!
//! # Ce qui vit ici
//!
//! - [`sgr`] — l'encodage de la présentation d'une cellule en séquence SGR.
//!
//! # Ce qui viendra
//!
//! Le décodeur de séquences ANSI d'entrée (jalon M1), le décodeur du protocole clavier
//! Kitty (M2), et le décodage des graphèmes Unicode (M3). Chacun sera un codec, avec sa
//! cible `cargo-fuzz` — un décodeur reçoit des octets hostiles par définition.

pub mod decimal;
pub mod sgr;

pub use sgr::{AirCellPresentation, AirSgrEncoder};
