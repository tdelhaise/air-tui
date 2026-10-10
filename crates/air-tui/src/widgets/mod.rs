// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les widgets.
//!
//! # État
//!
//! Huit widgets, tous colorés par les rôles du thème et jamais par une couleur écrite en
//! dur :
//!
//! - [`AirPanel`], un conteneur bordé à titre, et [`AirWindow`], une fenêtre à case de
//!   fermeture, en variante fenêtre ou dialogue ;
//! - [`AirStaticText`], une ligne de texte, et [`AirButton`], un bouton à lettre d'accès
//!   rapide et à commande ;
//! - [`AirMenuBar`] et ses [`AirMenu`]/[`AirMenuItem`], à menus déroulants ;
//! - [`AirStatusLine`] et ses [`AirStatusItem`], à raccourcis cliquables ;
//! - [`AirInputLine`], un champ de saisie d'une ligne.
//!
//! Ce qui manque encore : les listes, les ascenseurs, les cases à cocher et les boutons
//! radio, et l'éditeur de texte de `redit` (jalon M6).
//!
//! # La bordure, et qui la retire
//!
//! Toute vue est bordée par défaut (ADR-010). Les **petits contrôles** la retirent dans
//! leur constructeur : [`AirStaticText`], [`AirButton`], [`AirInputLine`], [`AirMenuBar`]
//! et [`AirStatusLine`] le font, parce qu'un contrôle d'une ligne encadré n'a plus de
//! place pour son contenu. Les **conteneurs** la gardent : [`AirPanel`] et [`AirWindow`]
//! la gardent.
//!
//! C'est un point de revue, pas une habitude : un widget qui laisse le défaut doit avoir
//! une raison.
//!
//! # Les phases, et qui les demande
//!
//! Deux widgets sortent de la phase du focus, et pour des raisons opposées :
//!
//! - [`AirMenuBar`] est en **pré-traitement**. `Alt`+lettre et `Échap` doivent lui
//!   parvenir avant le champ de saisie qui a le focus, car aucun champ n'a d'usage pour
//!   `Alt`+lettre.
//! - [`AirStatusLine`] est en **post-traitement**. Elle publie des touches de fonction :
//!   les prendre avant la vue focalisée reviendrait à les lui voler.

pub mod button;
pub mod hotkey;
pub mod input_line;
pub mod menu_bar;
pub mod panel;
pub mod static_text;
pub mod status_line;
pub mod window;

pub use button::AirButton;
pub use hotkey::{AirHotkeyLabel, split_hotkey};
pub use input_line::AirInputLine;
pub use menu_bar::{AirMenu, AirMenuBar, AirMenuItem};
pub use panel::AirPanel;
pub use static_text::AirStaticText;
pub use status_line::{AirStatusItem, AirStatusLine};
pub use window::AirWindow;
