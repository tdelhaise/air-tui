// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le panneau — un conteneur bordé, à titre optionnel.

use crate::theme::{AirRole, AirTheme};
use crate::view::painter::AirPainter;
use crate::view::{AirContext, AirEvent, AirGroup, AirOutcome, AirView, AirViewId, AirViewState};

/// Un conteneur bordé qui porte un titre.
///
/// Il **contient** un [`AirGroup`] au lieu d'en hériter (ADR-011) : tout ce qui relève du
/// rangement des enfants est délégué, et le panneau n'ajoute que son titre et son fond.
pub struct AirPanel {
    /// Les enfants et l'état commun.
    group: AirGroup,
    /// Le titre inscrit sur la bordure.
    title: Option<String>,
}

impl AirPanel {
    /// Un panneau vide, bordé — c'est un conteneur, il garde le défaut d'ADR-010.
    #[must_use]
    pub const fn new(state: AirViewState) -> Self {
        Self {
            group: AirGroup::new(state),
            title: None,
        }
    }

    /// Le même panneau, avec un titre.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Change le titre.
    pub fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }

    /// Ajoute une vue au premier plan et rend son identifiant.
    pub fn insert(&mut self, view: Box<dyn AirView>) -> AirViewId {
        self.group.insert(view)
    }

    /// Le groupe sous-jacent, pour le focus et l'ordre de profondeur.
    #[must_use]
    pub const fn group(&self) -> &AirGroup {
        &self.group
    }

    /// Le groupe sous-jacent, en écriture.
    pub const fn group_mut(&mut self) -> &mut AirGroup {
        &mut self.group
    }
}

impl AirView for AirPanel {
    fn state(&self) -> &AirViewState {
        self.group.state()
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        self.group.state_mut()
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, theme: &AirTheme) {
        let colors = theme.palette.color_pair(AirRole::WindowInterior);
        let size = painter.size();
        painter.fill_rectangle(
            crate::geometry::AirRectangle::new(crate::geometry::AirPoint::ORIGIN, size),
            crate::cell::AirCell::new(' ', colors.foreground, colors.background),
        );
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        self.group.handle_event(event, context)
    }

    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    fn children(&self) -> &[Box<dyn AirView>] {
        self.group.children()
    }

    fn children_mut(&mut self) -> &mut [Box<dyn AirView>] {
        self.group.children_mut()
    }
}

impl core::fmt::Debug for AirPanel {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AirPanel")
            .field("title", &self.title)
            .field("group", &self.group)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::AirPanel;
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::AirInputEvent;
    use crate::surface::AirSurface;
    use crate::theme::AirTheme;
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirContext, AirEvent, AirOutcome, AirView, AirViewId, AirViewState, dispatch_event,
        draw_view,
    };
    use crate::widgets::AirStaticText;

    fn identifier(value: u32) -> AirViewId {
        AirViewId::new(value).expect("un identifiant non nul")
    }

    fn state(value: u32, bounds: AirRectangle) -> AirViewState {
        AirViewState::new(identifier(value), bounds)
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
    fn un_panneau_garde_la_bordure_par_defaut() {
        // C'est un conteneur : il garde le défaut d'ADR-010, contrairement aux petits
        // contrôles.
        let panel = AirPanel::new(state(1, AirRectangle::from_parts(0, 0, 10, 5)));
        assert!(panel.state().has_border());
        assert_eq!(panel.title(), None);
    }

    #[test]
    fn un_titre_se_pose_et_se_retire() {
        let mut panel =
            AirPanel::new(state(1, AirRectangle::from_parts(0, 0, 10, 5))).with_title("Air");
        assert_eq!(panel.title(), Some("Air"));
        panel.set_title(None);
        assert_eq!(panel.title(), None);
    }

    #[test]
    fn un_panneau_delegue_son_etat_et_ses_enfants_au_groupe() {
        let mut panel = AirPanel::new(state(1, AirRectangle::from_parts(0, 0, 10, 5)));
        assert_eq!(panel.state().identifier(), identifier(1));
        panel.state_mut().set_focused(true);
        assert!(panel.state().is_focused());
        assert!(panel.group().is_empty());

        let inserted = panel.insert(Box::new(AirStaticText::new(
            state(2, AirRectangle::from_parts(0, 0, 4, 1)),
            "abc",
        )));
        assert_eq!(inserted, identifier(2));
        assert_eq!(panel.children().len(), 1);
        assert_eq!(panel.children_mut().len(), 1);
        assert_eq!(panel.group().len(), 1);
        assert!(panel.group_mut().focus(identifier(2)));
    }

    #[test]
    fn un_panneau_dessine_son_cadre_son_titre_et_son_contenu() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(11, 4);
        let mut surface = AirSurface::new(size);
        let mut panel =
            AirPanel::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size))).with_title("Air");
        panel.insert(Box::new(AirStaticText::new(
            state(2, AirRectangle::from_parts(0, 0, 9, 1)),
            "bonjour",
        )));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(&panel, &mut painter, &theme);
        }
        let lines = rendered(&surface, size);
        assert_eq!(lines.first().map(String::as_str), Some("┌── Air ──┐"));
        assert_eq!(lines.get(1).map(String::as_str), Some("│bonjour  │"));
        assert_eq!(lines.get(3).map(String::as_str), Some("└─────────┘"));
    }

    #[test]
    fn l_affichage_d_un_panneau_mentionne_son_titre() {
        let panel =
            AirPanel::new(state(1, AirRectangle::from_parts(0, 0, 4, 4))).with_title("Essai");
        let text = format!("{panel:?}");
        assert!(text.contains("Essai"), "rendu : {text}");
    }

    #[test]
    fn un_panneau_transmet_les_evenements_au_groupe() {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut panel = AirPanel::new(state(1, AirRectangle::from_parts(0, 0, 10, 5)));
        let mut event = AirEvent::new(AirInputEvent::FocusGained);
        // Un panneau n'a rien à dire d'un événement : il délègue, et le délégué ignore.
        assert_eq!(
            dispatch_event(&mut panel, &mut event, &mut context),
            AirOutcome::Ignored
        );
    }
}
