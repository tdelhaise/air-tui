// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La palette — des rôles sémantiques vers des couleurs.
//!
//! Le choix d'indexer par **rôle** et non par la chaîne de remappage à 63 octets de
//! Borland est posé par l'ADR-008 : la fidélité du thème par défaut s'obtient en
//! instanciant les valeurs de Borland dans ce modèle, pas en reproduisant son mécanisme.

use crate::cell::AirColor;

/// Un rôle sémantique de l'interface.
///
/// L'énumération est volontairement courte : elle ne couvre que ce que l'exemple du jalon
/// M0 dessine. Les rôles manquants — ascenseurs, items de menu, boutons, champs de saisie
/// — viendront avec les widgets qui les emploient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirRole {
    /// Le fond du bureau et son motif.
    DesktopBackground,
    /// Le cadre d'une fenêtre active.
    WindowFrameActive,
    /// Le cadre d'une fenêtre inactive.
    WindowFrameInactive,
    /// Le titre d'une fenêtre.
    WindowTitle,
    /// L'intérieur d'une fenêtre.
    WindowInterior,
    /// La marque de la case de fermeture.
    CloseBoxMark,
    /// La barre de menu.
    MenuBar,
    /// La lettre d'accès rapide d'un item de menu.
    MenuHotkey,
    /// La barre de statut.
    StatusLine,
    /// La lettre d'accès rapide d'un item de barre de statut.
    StatusHotkey,
}

/// Un couple de couleurs : caractère et fond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirColorPair {
    /// La couleur du caractère.
    pub foreground: AirColor,
    /// La couleur du fond.
    pub background: AirColor,
}

impl AirColorPair {
    /// Construit un couple de couleurs.
    #[must_use]
    pub const fn new(foreground: AirColor, background: AirColor) -> Self {
        Self {
            foreground,
            background,
        }
    }
}

/// La palette d'un thème.
#[derive(Debug, Clone)]
pub struct AirPalette {
    /// Le fond du bureau et son motif.
    desktop_background: AirColorPair,
    /// Le cadre d'une fenêtre active.
    window_frame_active: AirColorPair,
    /// Le cadre d'une fenêtre inactive.
    window_frame_inactive: AirColorPair,
    /// Le titre d'une fenêtre.
    window_title: AirColorPair,
    /// L'intérieur d'une fenêtre.
    window_interior: AirColorPair,
    /// La marque de la case de fermeture.
    close_box_mark: AirColorPair,
    /// La barre de menu.
    menu_bar: AirColorPair,
    /// La lettre d'accès rapide d'un item de menu.
    menu_hotkey: AirColorPair,
    /// La barre de statut.
    status_line: AirColorPair,
    /// La lettre d'accès rapide d'un item de barre de statut.
    status_hotkey: AirColorPair,
}

impl AirPalette {
    /// La palette de Turbo Vision.
    ///
    /// Les couples retenus sont ceux que l'on reconnaît d'emblée : gris clair sur bleu
    /// pour le bureau, noir sur gris clair pour la barre de menu, noir sur cyan pour la
    /// barre de statut, et la marque de fermeture en vert vif.
    ///
    /// La **fidélité exacte** à l'ensemble des entrées de Borland est l'objet du jalon M5
    /// (ADR-008) : ce qui est ici est juste pour ces dix rôles, pas exhaustif.
    #[must_use]
    pub const fn turbo_vision() -> Self {
        Self {
            desktop_background: AirColorPair::new(AirColor::LIGHT_GRAY, AirColor::BLUE),
            window_frame_active: AirColorPair::new(AirColor::WHITE, AirColor::BLUE),
            window_frame_inactive: AirColorPair::new(AirColor::LIGHT_GRAY, AirColor::BLUE),
            window_title: AirColorPair::new(AirColor::WHITE, AirColor::BLUE),
            window_interior: AirColorPair::new(AirColor::LIGHT_GRAY, AirColor::BLUE),
            close_box_mark: AirColorPair::new(AirColor::BRIGHT_GREEN, AirColor::BLUE),
            menu_bar: AirColorPair::new(AirColor::BLACK, AirColor::LIGHT_GRAY),
            menu_hotkey: AirColorPair::new(AirColor::RED, AirColor::LIGHT_GRAY),
            status_line: AirColorPair::new(AirColor::BLACK, AirColor::CYAN),
            status_hotkey: AirColorPair::new(AirColor::WHITE, AirColor::CYAN),
        }
    }

    /// Les couleurs d'un rôle.
    #[must_use]
    pub const fn color_pair(&self, role: AirRole) -> AirColorPair {
        match role {
            AirRole::DesktopBackground => self.desktop_background,
            AirRole::WindowFrameActive => self.window_frame_active,
            AirRole::WindowFrameInactive => self.window_frame_inactive,
            AirRole::WindowTitle => self.window_title,
            AirRole::WindowInterior => self.window_interior,
            AirRole::CloseBoxMark => self.close_box_mark,
            AirRole::MenuBar => self.menu_bar,
            AirRole::MenuHotkey => self.menu_hotkey,
            AirRole::StatusLine => self.status_line,
            AirRole::StatusHotkey => self.status_hotkey,
        }
    }
}

impl Default for AirPalette {
    fn default() -> Self {
        Self::turbo_vision()
    }
}

#[cfg(test)]
mod tests {
    use super::{AirPalette, AirRole};
    use crate::cell::AirColor;

    #[test]
    fn le_bureau_est_gris_clair_sur_bleu() {
        let pair = AirPalette::turbo_vision().color_pair(AirRole::DesktopBackground);
        assert_eq!(pair.foreground, AirColor::LIGHT_GRAY);
        assert_eq!(pair.background, AirColor::BLUE);
    }

    #[test]
    fn la_barre_de_statut_est_noir_sur_cyan() {
        let pair = AirPalette::turbo_vision().color_pair(AirRole::StatusLine);
        assert_eq!(pair.foreground, AirColor::BLACK);
        assert_eq!(pair.background, AirColor::CYAN);
    }

    #[test]
    fn la_marque_de_fermeture_est_vert_vif() {
        let pair = AirPalette::turbo_vision().color_pair(AirRole::CloseBoxMark);
        assert_eq!(pair.foreground, AirColor::BRIGHT_GREEN);
    }
}
