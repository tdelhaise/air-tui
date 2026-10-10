// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les caractères de dessin d'un thème.

/// Les six caractères d'un style de cadre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirFrameGlyphs {
    /// Angle supérieur gauche.
    pub top_left: char,
    /// Angle supérieur droit.
    pub top_right: char,
    /// Angle inférieur gauche.
    pub bottom_left: char,
    /// Angle inférieur droit.
    pub bottom_right: char,
    /// Segment horizontal.
    pub horizontal: char,
    /// Segment vertical.
    pub vertical: char,
}

impl AirFrameGlyphs {
    /// Le cadre simple trait, employé par les fenêtres de Turbo Vision.
    pub const SINGLE: Self = Self {
        top_left: '┌',
        top_right: '┐',
        bottom_left: '└',
        bottom_right: '┘',
        horizontal: '─',
        vertical: '│',
    };

    /// Le cadre double trait, employé par les dialogues de Turbo Vision.
    pub const DOUBLE: Self = Self {
        top_left: '╔',
        top_right: '╗',
        bottom_left: '╚',
        bottom_right: '╝',
        horizontal: '═',
        vertical: '║',
    };
}

/// Le jeu de glyphes d'un thème.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AirGlyphSet {
    /// Le cadre des fenêtres ordinaires.
    pub window_frame: AirFrameGlyphs,
    /// Le cadre des dialogues.
    pub dialog_frame: AirFrameGlyphs,
    /// Le caractère de remplissage du bureau.
    pub desktop_fill: char,
    /// Le crochet ouvrant de la case de fermeture.
    pub close_box_left: char,
    /// La marque de la case de fermeture.
    pub close_box_mark: char,
    /// Le crochet fermant de la case de fermeture.
    pub close_box_right: char,
    /// La flèche d'ascenseur vers le haut.
    pub scroll_arrow_up: char,
    /// La flèche d'ascenseur vers le bas.
    pub scroll_arrow_down: char,
}

impl AirGlyphSet {
    /// Le jeu de glyphes de Turbo Vision.
    ///
    /// Les valeurs sont celles de la page de code 437 d'origine, transposées en Unicode :
    /// le remplissage du bureau est le `░` (0xB0), la marque de fermeture le `■` (0xFE).
    #[must_use]
    pub const fn turbo_vision() -> Self {
        Self {
            window_frame: AirFrameGlyphs::SINGLE,
            dialog_frame: AirFrameGlyphs::DOUBLE,
            desktop_fill: '░',
            close_box_left: '[',
            close_box_mark: '■',
            close_box_right: ']',
            scroll_arrow_up: '▲',
            scroll_arrow_down: '▼',
        }
    }
}

impl Default for AirGlyphSet {
    fn default() -> Self {
        Self::turbo_vision()
    }
}

#[cfg(test)]
mod tests {
    use super::{AirFrameGlyphs, AirGlyphSet};

    #[test]
    fn le_cadre_de_dialogue_est_en_double_trait() {
        assert_eq!(
            AirGlyphSet::turbo_vision().dialog_frame,
            AirFrameGlyphs::DOUBLE
        );
    }

    #[test]
    fn le_cadre_de_fenetre_est_en_simple_trait() {
        assert_eq!(
            AirGlyphSet::turbo_vision().window_frame,
            AirFrameGlyphs::SINGLE
        );
    }

    #[test]
    fn la_case_de_fermeture_est_entre_crochets() {
        let glyphs = AirGlyphSet::turbo_vision();
        assert_eq!(glyphs.close_box_left, '[');
        assert_eq!(glyphs.close_box_mark, '■');
        assert_eq!(glyphs.close_box_right, ']');
    }

    #[test]
    fn les_fleches_d_ascenseur_sont_des_triangles_pleins() {
        let glyphs = AirGlyphSet::turbo_vision();
        assert_eq!(glyphs.scroll_arrow_up, '▲');
        assert_eq!(glyphs.scroll_arrow_down, '▼');
    }

    #[test]
    fn le_jeu_par_defaut_est_celui_de_turbo_vision() {
        assert_eq!(AirGlyphSet::default(), AirGlyphSet::turbo_vision());
    }

    #[test]
    fn les_quatre_angles_d_un_cadre_sont_distincts() {
        for glyphs in [AirFrameGlyphs::SINGLE, AirFrameGlyphs::DOUBLE] {
            let mut corners = [
                glyphs.top_left,
                glyphs.top_right,
                glyphs.bottom_left,
                glyphs.bottom_right,
            ];
            corners.sort_unstable();
            let before = corners.len();
            let mut unique = corners.to_vec();
            unique.dedup();
            assert_eq!(
                unique.len(),
                before,
                "deux angles identiques dans {glyphs:?}"
            );
            assert_ne!(glyphs.horizontal, glyphs.vertical);
        }
    }

    #[test]
    fn le_bureau_est_rempli_de_hachures_legeres() {
        assert_eq!(AirGlyphSet::turbo_vision().desktop_fill, '░');
    }
}
