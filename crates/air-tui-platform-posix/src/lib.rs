// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Implémentation POSIX du contrat de plateforme — l'interface avec le système, et **rien
//! d'autre**.
//!
//! Cette crate est le seul endroit du projet où `unsafe` et les appels à la bibliothèque
//! C sont autorisés (ADR-002, ADR-011). `air-tui-platform` est en
//! `#![forbid(unsafe_code)]` et `air-tui` ne dépend même pas de cette crate : la frontière
//! n'est pas une discipline, c'est le graphe de dépendances.
//!
//! # Ce qu'elle fournit
//!
//! [`PosixTerminal`], qui implémente [`air_tui_platform::TerminalDevice`] au-dessus de six
//! symboles de la bibliothèque C, chacun derrière une **enveloppe dédiée typée** — jamais
//! de `ioctl(descripteur, requête, …)` générique, conformément à ADR-021 §3 du projet
//! Air :
//!
//! | Enveloppe | Symbole C |
//! |---|---|
//! | [`is_terminal`] | `isatty` |
//! | [`current_mode`], [`enter_raw_mode`], [`restore_mode`] | `tcgetattr`, `cfmakeraw`, `tcsetattr` |
//! | [`terminal_window_size`] | `ioctl` avec `TIOCGWINSZ` |
//! | [`wait_for_input`] | `poll` |
//!
//! Trois besoins sont volontairement couverts par `std` plutôt que par du FFI :
//! l'ouverture de `/dev/tty` (`std::fs::File`), la lecture et l'écriture
//! (`std::io::Read` et `std::io::Write`), et `errno`
//! (`std::io::Error::last_os_error`).
//!
//! # Conventions respectées
//!
//! - `Option<T>` plutôt qu'une sentinelle du noyau : une taille de fenêtre nulle devient
//!   `Ok(None)`, pas `Ok(WindowSize { columns: 0, rows: 0 })`.
//! - **EINTR est remonté à l'appelant**, jamais retenté automatiquement. La boucle
//!   d'événements écrit son propre retry — ce qui lui permet, sur un redimensionnement,
//!   d'aller relire la taille au lieu de retenter l'attente.
//! - Aucune allocation sur le chemin chaud.

// `expect` et `unwrap` sont interdits en production (lints de l'espace de travail) et
// légitimes dans un test, où une précondition non tenue doit faire échouer le test
// bruyamment.
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

mod errno;
mod ffi;
mod input;
mod posix_terminal;
mod terminal_mode;
mod window_size;

pub use errno::Errno;
pub use input::wait_for_input;
pub use posix_terminal::PosixTerminal;
pub use terminal_mode::{TerminalMode, current_mode, enter_raw_mode, is_terminal, restore_mode};
pub use window_size::terminal_window_size;
