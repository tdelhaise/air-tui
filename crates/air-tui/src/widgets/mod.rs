// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les widgets.
//!
//! # État
//!
//! Deux widgets seulement, et ils sont là pour **prouver l'arbre** plutôt que pour
//! constituer une bibliothèque : [`AirPanel`], un conteneur bordé à titre, et
//! [`AirStaticText`], une ligne de texte. Le jeu complet — boutons, champs de saisie,
//! barre de menu, barre de statut, listes — vient au jalon M5, avec le moteur de thèmes
//! qui leur donnera leurs rôles de couleur.
//!
//! # La bordure, et qui la retire
//!
//! Toute vue est bordée par défaut (ADR-010). Les **petits contrôles** la retirent dans
//! leur constructeur : [`AirStaticText`] le fait, parce qu'une étiquette encadrée n'a plus
//! de place pour son texte. Les **conteneurs** la gardent : [`AirPanel`] la garde.
//!
//! C'est un point de revue, pas une habitude : un widget qui laisse le défaut doit avoir
//! une raison.

pub mod button;
pub mod hotkey;
pub mod panel;
pub mod static_text;
pub mod window;

pub use button::AirButton;
pub use hotkey::{AirHotkeyLabel, split_hotkey};
pub use panel::AirPanel;
pub use static_text::AirStaticText;
pub use window::AirWindow;
