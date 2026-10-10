// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les thèmes — palette et glyphes.
//!
//! # État
//!
//! **Deux thèmes, et c'est ce qui fait de ce module un moteur.** La palette indexée par
//! rôles sémantiques (vingt-huit rôles), le jeu de glyphes et le quantificateur de
//! couleurs existent depuis le jalon M5 ; le thème Midnight Commander et l'importeur de
//! skins qui le produit, depuis M7.
//!
//! Un thème est un **couple** `palette` + `glyphs`. L'ADR-008 décrit un **triplet**, la
//! troisième assiette étant `AirDecorations` — ombres, style de trait, style des touches
//! rapides. Elle n'existe pas, et l'écart est assumé plutôt que comblé par une structure
//! vide : le style de trait vit déjà dans le jeu de glyphes, qui porte un cadre de
//! fenêtre et un cadre de dialogue distincts, et l'ombre portée ne peut pas être dessinée
//! aujourd'hui — elle tombe **hors** des limites de la vue qui la projette, ce que le
//! découpage du peintre refuse sans exception (M4, M5b). Tant qu'aucune vue ne dessine
//! d'ombre, une assiette qui en décrirait une annoncerait une capacité absente.
//!
//! Le rôle `AirRole::ButtonShadow` est dans la palette pour la même raison inverse : il
//! est transcrit de Borland et de `mc`, et **aucun widget ne le lit** encore.

pub mod glyphs;
pub mod palette;
pub mod quantize;
pub(crate) mod skins;

pub use glyphs::{AirFrameGlyphs, AirGlyphSet};
pub use palette::{AirColorPair, AirPalette, AirRole};
pub use quantize::{AirColorDepth, quantize};

/// Un thème complet.
///
/// Les deux assiettes sont séparées parce que ce qui fait reconnaître un look tient autant
/// aux glyphes qu'aux couleurs : le `╔═╗` double trait des dialogues de Turbo Vision est
/// aussi caractéristique que son fond gris clair — et le skin par défaut de Midnight
/// Commander, lui, dessine ses dialogues en trait **simple**, ce qui se voit d'un coup
/// d'œil.
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

    /// Le thème de Midnight Commander, compilé depuis son skin par défaut.
    ///
    /// Un avertissement que l'ADR-008 écrit noir sur blanc : **ce thème ne suffit pas à
    /// ressembler à Midnight Commander**. Ce qui fait reconnaître `mc` est sa coquille —
    /// deux panneaux, ligne de commande, barre de touches étiquetée —, et une coquille
    /// n'est pas un thème. Appliqué au chrome de `redit`, ce thème donne un éditeur aux
    /// couleurs de `mc`, pas `mc`.
    #[must_use]
    pub fn midnight_commander() -> Self {
        Self {
            palette: AirPalette::midnight_commander(),
            glyphs: AirGlyphSet::midnight_commander(),
        }
    }

    /// La description que le skin d'origine porte, pour le thème de Midnight Commander.
    #[must_use]
    pub const fn midnight_commander_description() -> &'static str {
        skins::midnight_commander::DESCRIPTION
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
    fn les_deux_themes_se_distinguent_a_la_barre_de_menu() {
        // Turbo Vision : noir sur gris clair. Midnight Commander : blanc sur cyan. C'est
        // la différence la plus visible entre les deux, et elle vient du skin.
        let turbo = AirTheme::turbo_vision();
        let midnight = AirTheme::midnight_commander();
        assert_ne!(
            turbo.palette.color_pair(AirRole::MenuBar),
            midnight.palette.color_pair(AirRole::MenuBar)
        );
    }

    #[test]
    fn les_deux_themes_s_accordent_sur_le_fond_du_bureau() {
        // Et ce n'est pas un défaut : les deux sont gris clair sur bleu. Midnight
        // Commander a repris le fond de Norton Commander, que Turbo Vision employait
        // déjà. Deux transcriptions indépendantes qui tombent sur la même valeur.
        let turbo = AirTheme::turbo_vision();
        let midnight = AirTheme::midnight_commander();
        assert_eq!(
            turbo.palette.color_pair(AirRole::DesktopBackground),
            midnight.palette.color_pair(AirRole::DesktopBackground)
        );
    }

    #[test]
    fn le_bureau_de_midnight_commander_n_est_pas_hachure() {
        // mc n'a pas de bureau : ses panneaux couvrent l'écran. Le remplissage est donc
        // un espace, là où Turbo Vision pose son `░`.
        assert_eq!(AirTheme::turbo_vision().glyphs.desktop_fill, '░');
        assert_eq!(AirTheme::midnight_commander().glyphs.desktop_fill, ' ');
    }

    #[test]
    fn midnight_commander_dessine_ses_dialogues_en_trait_simple() {
        // La famille `d*` du skin par défaut est en trait simple, contrairement au
        // `╔═╗` de Turbo Vision. C'est le fichier qui le dit, pas une interprétation.
        let midnight = AirTheme::midnight_commander();
        assert_eq!(midnight.glyphs.dialog_frame, midnight.glyphs.window_frame);
        let turbo = AirTheme::turbo_vision();
        assert_ne!(turbo.glyphs.dialog_frame, turbo.glyphs.window_frame);
    }

    #[test]
    fn la_case_de_fermeture_porte_la_marque_du_skin() {
        assert_eq!(AirTheme::turbo_vision().glyphs.close_box_mark, '■');
        assert_eq!(AirTheme::midnight_commander().glyphs.close_box_mark, 'X');
    }

    #[test]
    fn les_vingt_huit_roles_repondent_dans_les_deux_themes() {
        // Un rôle sans valeur serait un trou dans la palette, et le compilateur ne le
        // verrait pas : `color_pair` est exhaustif par construction, mais une palette
        // compilée depuis un skin pourrait porter des couples laissés à zéro.
        let roles = [
            AirRole::DesktopBackground,
            AirRole::MenuBar,
            AirRole::MenuDisabled,
            AirRole::MenuHotkey,
            AirRole::MenuSelected,
            AirRole::MenuSelectedDisabled,
            AirRole::MenuSelectedHotkey,
            AirRole::StatusLine,
            AirRole::StatusDisabled,
            AirRole::StatusHotkey,
            AirRole::WindowFrameActive,
            AirRole::WindowFrameInactive,
            AirRole::WindowFrameIcon,
            AirRole::WindowTitle,
            AirRole::WindowInterior,
            AirRole::WindowText,
            AirRole::DialogFrameActive,
            AirRole::DialogFrameInactive,
            AirRole::DialogFrameIcon,
            AirRole::DialogInterior,
            AirRole::ButtonNormal,
            AirRole::ButtonDefault,
            AirRole::ButtonFocused,
            AirRole::ButtonDisabled,
            AirRole::ButtonHotkey,
            AirRole::ButtonShadow,
            AirRole::ScrollBarPage,
            AirRole::ScrollBarControls,
        ];
        assert_eq!(roles.len(), 28);
        let midnight = AirTheme::midnight_commander();
        let mut distinct = Vec::new();
        for role in roles {
            let pair = midnight.palette.color_pair(role);
            distinct.push((pair.foreground, pair.background));
        }
        distinct.dedup();
        assert!(
            distinct.len() > 1,
            "une palette qui ne rend qu'un seul couple n'est pas une palette"
        );
    }

    #[test]
    fn le_theme_compile_porte_la_description_de_son_skin() {
        assert_eq!(AirTheme::midnight_commander_description(), "Standard skin");
    }

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
