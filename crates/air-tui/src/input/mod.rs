// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'entrée — le modèle d'événements et le décodeur de séquences ANSI.
//!
//! # L'étage terminal
//!
//! Ce module couvre l'étage **terminal** de l'ADR-003 : des octets arrivant d'un pty, donc
//! aucun code matériel et des modificateurs limités à ce que le codage historique sait
//! exprimer. C'est l'étage de tier-1, celui qui fonctionne en SSH et sur macOS.
//!
//! L'étage **natif** — `evdev` et `TIOCLINUX` sur la console Linux, seul à livrer de vrais
//! codes matériels — viendra au jalon M8, derrière la même forme d'événement.

pub mod ansi;
pub mod clicks;
pub mod event;
pub mod kitty;
pub mod scanning;

pub use ansi::{AirAnsiDecoder, AirDecodeError};
pub use clicks::{AirClickCount, AirClickTracker, DEFAULT_MULTIPLE_CLICK_DELAY};
pub use event::{
    AirInputEvent, AirKey, AirKeyEvent, AirKeyState, AirModifiers, AirMouseButton, AirMouseEvent,
    AirMouseEventKind, AirScanCode,
};
pub use kitty::{AirKeyboardFlags, AirKeyboardProtocol};
pub use scanning::AirParameterField;
