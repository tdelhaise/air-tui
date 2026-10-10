// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les thèmes — palette, glyphes, décorations.
//!
//! # État
//!
//! **Minimal, au service de l'exemple du jalon M0.** Le moteur décrit par l'ADR-008 — la
//! palette complète indexée par rôles, les décorations, le quantificateur de couleurs,
//! le thème Midnight Commander et l'importeur de skins — n'est **pas** écrit.
//!
//! Ce qui existe ici suffit à dessiner un cadre, une barre de menu et une barre de statut
//! aux couleurs et aux glyphes de Turbo Vision, et pose la forme que le moteur prendra :
//! un thème est un triplet, pas une palette.

pub mod glyphs;
pub mod palette;
pub mod quantize;

pub use glyphs::{AirFrameGlyphs, AirGlyphSet};
pub use palette::{AirColorPair, AirPalette, AirRole};
pub use quantize::{AirColorDepth, quantize};

/// Un thème complet.
///
/// Les trois assiettes sont séparées parce que ce qui fait reconnaître un look tient
/// autant aux glyphes qu'aux couleurs : le `╔═╗` double trait des dialogues de Turbo
/// Vision est aussi caractéristique que son fond gris clair.
#[derive(Debug, Clone)]
pub struct AirTheme {
    /// Les couleurs, indexées par rôle sémantique.
    pub palette: AirPalette,
    /// Les caractères de dessin.
    pub glyphs: AirGlyphSet,
}

impl AirTheme {
    /// Le thème par défaut : celui de Turbo Vision.
    #[must_use]
    pub fn turbo_vision() -> Self {
        Self {
            palette: AirPalette::turbo_vision(),
            glyphs: AirGlyphSet::turbo_vision(),
        }
    }
}

impl Default for AirTheme {
    fn default() -> Self {
        Self::turbo_vision()
    }
}

#[cfg(test)]
mod tests {
    use super::AirTheme;
    use crate::theme::palette::AirRole;

    #[test]
    fn le_theme_par_defaut_est_turbo_vision() {
        let theme = AirTheme::default();
        let reference = AirTheme::turbo_vision();
        assert_eq!(
            theme.palette.color_pair(AirRole::DesktopBackground),
            reference.palette.color_pair(AirRole::DesktopBackground)
        );
        assert_eq!(theme.glyphs.desktop_fill, reference.glyphs.desktop_fill);
    }
}
