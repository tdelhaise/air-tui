// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le texte statique — une ligne de texte, sans bordure.

use crate::geometry::{AirColumn, AirPoint, AirRow};
use crate::theme::AirRole;
use crate::view::painter::AirPainter;
use crate::view::{AirDrawContext, AirView, AirViewState};

/// Une ligne de texte qui ne réagit à rien.
///
/// **Sans bordure** : c'est un petit contrôle, et l'encadrer ne laisserait plus de place
/// au texte (ADR-010). Le constructeur retire donc le défaut, explicitement.
pub struct AirStaticText {
    /// L'état commun.
    state: AirViewState,
    /// Le texte affiché.
    text: String,
    /// Le rôle de couleur employé.
    role: AirRole,
}

impl AirStaticText {
    /// Une étiquette, sans bordure, aux couleurs de l'intérieur d'une fenêtre.
    #[must_use]
    pub fn new(mut state: AirViewState, text: impl Into<String>) -> Self {
        // Un petit contrôle retire la bordure par défaut : l'encadrer ne laisserait plus
        // de place au texte.
        state.set_border(false);
        Self {
            state,
            text: text.into(),
            role: AirRole::WindowInterior,
        }
    }

    /// La même étiquette, à un autre rôle de couleur.
    #[must_use]
    pub const fn with_role(mut self, role: AirRole) -> Self {
        self.role = role;
        self
    }

    /// Le texte affiché.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Change le texte.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl AirView for AirStaticText {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let colors = context.theme.palette.color_pair(self.role);
        painter.draw_text_with_colors(
            AirPoint::new(AirColumn::FIRST, AirRow::FIRST),
            &self.text,
            colors,
        );
    }
}

impl core::fmt::Debug for AirStaticText {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AirStaticText")
            .field("text", &self.text)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::AirStaticText;
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::surface::AirSurface;
    use crate::theme::{AirRole, AirTheme};
    use crate::view::painter::AirPainter;
    use crate::view::{AirDrawContext, AirView, AirViewId, AirViewState, draw_view};

    fn state(value: u32, bounds: AirRectangle) -> AirViewState {
        AirViewState::new(
            AirViewId::new(value).expect("un identifiant non nul"),
            bounds,
        )
    }

    fn rendered(surface: &AirSurface, size: AirSize) -> Vec<String> {
        (0..size.rows)
            .map(|row| {
                (0..size.columns)
                    .map(|column| {
                        surface
                            .cell(AirPoint::new(AirColumn::new(column), AirRow::new(row)))
                            .map_or(' ', |found| found.character)
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn une_etiquette_retire_la_bordure_dans_son_constructeur() {
        // Le corollaire d'ADR-010 : un petit contrôle encadré n'a plus de place pour son
        // texte, et le retrait doit être explicite plutôt que subi.
        let label = AirStaticText::new(state(1, AirRectangle::from_parts(0, 0, 6, 1)), "abc");
        assert!(!label.state().has_border());
    }

    #[test]
    fn le_texte_se_lit_et_se_remplace() {
        let mut label = AirStaticText::new(state(1, AirRectangle::from_parts(0, 0, 6, 1)), "abc");
        assert_eq!(label.text(), "abc");
        label.set_text("def");
        assert_eq!(label.text(), "def");
        label.state_mut().set_visible(false);
        assert!(!label.state().is_visible());
    }

    #[test]
    fn une_etiquette_ecrit_son_texte_depuis_l_origine() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(6, 1);
        let mut surface = AirSurface::new(size);
        let label = AirStaticText::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)), "abc");
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &label,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        assert_eq!(rendered(&surface, size), vec!["abc   ".to_owned()]);
    }

    #[test]
    fn un_texte_plus_long_que_la_vue_est_decoupe() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(3, 1);
        let mut surface = AirSurface::new(size);
        let label = AirStaticText::new(
            state(1, AirRectangle::new(AirPoint::ORIGIN, size)),
            "abcdefgh",
        );
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &label,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        assert_eq!(rendered(&surface, size), vec!["abc".to_owned()]);
    }

    #[test]
    fn le_role_de_couleur_est_celui_demande() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(4, 1);
        let mut surface = AirSurface::new(size);
        let label = AirStaticText::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)), "a")
            .with_role(AirRole::StatusLine);
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &label,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let expected = theme.palette.color_pair(AirRole::StatusLine);
        assert_eq!(
            surface
                .cell(AirPoint::new(AirColumn::FIRST, AirRow::FIRST))
                .map(|found| found.background),
            Some(expected.background)
        );
    }

    #[test]
    fn l_affichage_mentionne_le_texte() {
        let label = AirStaticText::new(state(1, AirRectangle::from_parts(0, 0, 4, 1)), "coucou");
        let text = format!("{label:?}");
        assert!(text.contains("coucou"), "rendu : {text}");
    }
}
