// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le bureau — la zone des fenêtres, et son fond.
//!
//! # Ce qu'il ajoute à un groupe
//!
//! Un [`crate::view::AirGroup`] tient déjà une liste d'enfants en ordre de profondeur. Le
//! bureau y ajoute deux choses, et seulement deux :
//!
//! - un **fond** dessiné avec le glyphe de remplissage du thème — le `░` de Turbo Vision ;
//! - la **cascade** et la **mosaïque**, qui ne sont pas des opérations de groupe mais des
//!   dispositions de fenêtres.
//!
//! La gestion de la liste elle-même est partagée avec le groupe, par les fonctions de
//! [`crate::view::group`] : trois conteneurs la cherchent, et la recopier dans chacun
//! garantirait qu'ils divergent.

use crate::geometry::{AirPoint, AirRectangle};
use crate::theme::AirRole;
use crate::view::group::{bring_child_to_front, focus_child, focused_child, position_of};
use crate::view::painter::AirPainter;
use crate::view::{AirDrawContext, AirView, AirViewId, AirViewState};

/// La zone des fenêtres.
pub struct AirDesktop {
    /// L'état commun.
    state: AirViewState,
    /// Les fenêtres, de l'arrière vers l'avant.
    windows: Vec<Box<dyn AirView>>,
}

impl AirDesktop {
    /// Un bureau vide, sans bordure.
    ///
    /// Le bureau est le fond, pas une fenêtre : un cadre autour de lui encadrerait l'écran.
    #[must_use]
    pub fn new(mut state: AirViewState) -> Self {
        state.set_border(false);
        Self {
            state,
            windows: Vec::new(),
        }
    }

    /// Ajoute une fenêtre au premier plan et rend son identifiant.
    pub fn insert(&mut self, window: Box<dyn AirView>) -> AirViewId {
        let identifier = window.state().identifier();
        self.windows.push(window);
        identifier
    }

    /// Retire la fenêtre d'identifiant donné, si elle est là.
    pub fn remove(&mut self, identifier: AirViewId) -> Option<Box<dyn AirView>> {
        let position = position_of(&self.windows, identifier)?;
        Some(self.windows.remove(position))
    }

    /// Le nombre de fenêtres.
    #[must_use]
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Vrai si le bureau est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Amène une fenêtre au premier plan.
    pub fn bring_to_front(&mut self, identifier: AirViewId) -> bool {
        bring_child_to_front(&mut self.windows, identifier)
    }

    /// Donne le focus à une fenêtre et le retire aux autres.
    pub fn focus(&mut self, identifier: AirViewId) -> bool {
        focus_child(&mut self.windows, identifier)
    }

    /// L'identifiant de la fenêtre qui a le focus.
    #[must_use]
    pub fn focused(&self) -> Option<AirViewId> {
        focused_child(&self.windows)
    }

    /// La fenêtre d'identifiant donné.
    #[must_use]
    pub fn window(&self, identifier: AirViewId) -> Option<&dyn AirView> {
        let position = position_of(&self.windows, identifier)?;
        self.windows.get(position).map(Box::as_ref)
    }

    /// La fenêtre d'identifiant donné, en écriture.
    pub fn window_mut(&mut self, identifier: AirViewId) -> Option<&mut (dyn AirView + 'static)> {
        let position = position_of(&self.windows, identifier)?;
        self.windows.get_mut(position).map(Box::as_mut)
    }

    /// La fenêtre au premier plan, en écriture.
    pub fn front_window_mut(&mut self) -> Option<&mut (dyn AirView + 'static)> {
        self.windows.last_mut().map(Box::as_mut)
    }

    /// Dispose les fenêtres en cascade, décalées d'une cellule.
    ///
    /// Chaque fenêtre garde sa dimension et reçoit une origine décalée ; la dernière
    /// insérée est la plus avancée, donc celle qu'on voit entière. Une fenêtre qui
    /// sortirait du bureau est ramenée dans ses limites plutôt que découpée — un titre
    /// invisible n'aide personne.
    pub fn cascade(&mut self) {
        let Some(area) = self.state.content_bounds() else {
            return;
        };
        for (index, window) in self.windows.iter_mut().enumerate() {
            let step = u16::try_from(index).unwrap_or(u16::MAX);
            let size = window.state().bounds().size;
            let column = step.min(area.size.columns.saturating_sub(size.columns));
            let row = step.min(area.size.rows.saturating_sub(size.rows));
            window.state_mut().set_bounds(AirRectangle::from_parts(
                column,
                row,
                size.columns,
                size.rows,
            ));
        }
    }

    /// Dispose les fenêtres en colonnes de largeur égale.
    ///
    /// La dernière colonne reçoit le reste de la division : sans cela, une largeur non
    /// divisible laisserait une bande de bureau à droite.
    pub fn tile(&mut self) {
        let Some(area) = self.state.content_bounds() else {
            return;
        };
        let count = self.windows.len();
        let Ok(count_columns) = u16::try_from(count) else {
            return;
        };
        if count_columns == 0 {
            return;
        }
        let width = area.size.columns.checked_div(count_columns).unwrap_or(0);
        if width == 0 {
            // Moins d'une colonne par fenêtre : la mosaïque n'a pas de sens, et en
            // produire une de largeur nulle cacherait tout.
            return;
        }
        for (index, window) in self.windows.iter_mut().enumerate() {
            let position = u16::try_from(index).unwrap_or(u16::MAX);
            let column = width.saturating_mul(position);
            let is_last = index.saturating_add(1) == count;
            let columns = if is_last {
                area.size.columns.saturating_sub(column)
            } else {
                width
            };
            window.state_mut().set_bounds(AirRectangle::from_parts(
                column,
                0,
                columns,
                area.size.rows,
            ));
        }
    }
}

impl AirView for AirDesktop {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let colors = context.theme.palette.color_pair(AirRole::DesktopBackground);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::new(AirPoint::ORIGIN, size),
            colors.cell(context.theme.glyphs.desktop_fill),
        );
    }

    fn children(&self) -> &[Box<dyn AirView>] {
        &self.windows
    }

    fn children_mut(&mut self) -> &mut [Box<dyn AirView>] {
        &mut self.windows
    }
}

impl core::fmt::Debug for AirDesktop {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // `dyn AirView` n'impose pas `Debug` : l'exiger du trait le restreindrait sans
        // rien apporter.
        formatter
            .debug_struct("AirDesktop")
            .field("identifier", &self.state.identifier())
            .field("windows", &self.windows.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::AirDesktop;
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::surface::AirSurface;
    use crate::theme::AirTheme;
    use crate::view::painter::AirPainter;
    use crate::view::{AirDrawContext, AirView, AirViewId, AirViewState, draw_view};
    use crate::widgets::AirWindow;

    fn identifier(value: u32) -> AirViewId {
        AirViewId::new(value).expect("un identifiant non nul")
    }

    fn desktop(columns: u16, rows: u16) -> AirDesktop {
        AirDesktop::new(AirViewState::new(
            identifier(1),
            AirRectangle::from_parts(0, 0, columns, rows),
        ))
    }

    fn window(value: u32, columns: u16, rows: u16) -> Box<dyn AirView> {
        Box::new(AirWindow::new(AirViewState::new(
            identifier(value),
            AirRectangle::from_parts(0, 0, columns, rows),
        )))
    }

    fn render(view: &dyn AirView, size: AirSize) -> String {
        let mut surface = AirSurface::new(size);
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(view, &mut painter, &AirDrawContext::new(&theme, &commands));
        }
        let mut lines: Vec<String> = Vec::new();
        for row in 0..size.rows {
            let mut line = String::new();
            for column in 0..size.columns {
                let point = AirPoint::new(AirColumn::new(column), AirRow::new(row));
                if let Some(cell) = surface.cell(point) {
                    line.push(cell.character);
                }
            }
            lines.push(line);
        }
        lines.join("\n")
    }

    #[test]
    fn un_bureau_neuf_est_vide_et_sans_bordure() {
        let desktop = desktop(20, 10);
        assert!(desktop.is_empty());
        assert_eq!(desktop.len(), 0);
        assert!(!desktop.state().has_border());
    }

    #[test]
    fn le_fond_est_le_glyphe_de_remplissage_du_theme() {
        let desktop = desktop(4, 2);
        assert_eq!(render(&desktop, AirSize::new(4, 2)), "░░░░\n░░░░");
    }

    #[test]
    fn une_fenetre_inseree_recouvre_le_fond() {
        let mut desktop = desktop(6, 3);
        desktop.insert(window(2, 4, 3));
        let image = render(&desktop, AirSize::new(6, 3));
        assert_eq!(image, "┌──┐░░\n│  │░░\n└──┘░░");
    }

    #[test]
    fn la_derniere_inseree_est_au_premier_plan() {
        let mut desktop = desktop(20, 10);
        desktop.insert(window(2, 4, 4));
        desktop.insert(window(3, 4, 4));
        assert_eq!(
            desktop
                .children()
                .last()
                .map(|child| child.state().identifier()),
            Some(identifier(3))
        );
        assert!(desktop.bring_to_front(identifier(2)));
        assert_eq!(
            desktop
                .children()
                .last()
                .map(|child| child.state().identifier()),
            Some(identifier(2))
        );
    }

    #[test]
    fn un_identifiant_inconnu_ne_passe_pas_au_premier_plan() {
        let mut desktop = desktop(20, 10);
        desktop.insert(window(2, 4, 4));
        assert!(!desktop.bring_to_front(identifier(99)));
    }

    #[test]
    fn le_focus_est_exclusif_entre_fenetres() {
        let mut desktop = desktop(20, 10);
        desktop.insert(window(2, 4, 4));
        desktop.insert(window(3, 4, 4));
        assert!(desktop.focus(identifier(3)));
        assert_eq!(desktop.focused(), Some(identifier(3)));
        assert!(desktop.focus(identifier(2)));
        assert_eq!(desktop.focused(), Some(identifier(2)));
        assert!(!desktop.focus(identifier(99)));
        // Un identifiant inconnu retire le focus à tout le monde plutôt que d'en laisser
        // deux : c'est le comportement de `focus_child`, et il est préférable à un focus
        // qui se dédouble.
        assert_eq!(desktop.focused(), None);
    }

    #[test]
    fn une_fenetre_se_retrouve_et_se_retire_par_son_identifiant() {
        let mut desktop = desktop(20, 10);
        let first = desktop.insert(window(2, 4, 4));
        desktop.insert(window(3, 4, 4));
        assert!(desktop.window(first).is_some());
        assert!(desktop.window_mut(first).is_some());
        assert!(desktop.remove(first).is_some());
        assert!(desktop.window(first).is_none());
        assert!(desktop.remove(first).is_none());
        assert_eq!(desktop.len(), 1);
    }

    #[test]
    fn la_fenetre_de_devant_est_la_derniere() {
        let mut desktop = desktop(20, 10);
        desktop.insert(window(2, 4, 4));
        desktop.insert(window(3, 4, 4));
        assert_eq!(
            desktop
                .front_window_mut()
                .map(|window| window.state().identifier()),
            Some(identifier(3))
        );
    }

    #[test]
    fn un_bureau_vide_n_a_pas_de_fenetre_de_devant() {
        let mut desktop = desktop(20, 10);
        assert!(desktop.front_window_mut().is_none());
    }

    #[test]
    fn la_cascade_decale_chaque_fenetre_d_une_cellule() {
        let mut desktop = desktop(20, 10);
        desktop.insert(window(2, 6, 4));
        desktop.insert(window(3, 6, 4));
        desktop.insert(window(4, 6, 4));
        desktop.cascade();
        let origins: Vec<(u16, u16)> = desktop
            .children()
            .iter()
            .map(|child| {
                let bounds = child.state().bounds();
                (bounds.left(), bounds.top())
            })
            .collect();
        assert_eq!(origins, vec![(0, 0), (1, 1), (2, 2)]);
    }

    #[test]
    fn la_cascade_ramene_une_fenetre_qui_sortirait_du_bureau() {
        let mut desktop = desktop(8, 5);
        desktop.insert(window(2, 8, 5));
        desktop.insert(window(3, 8, 5));
        desktop.cascade();
        let origins: Vec<(u16, u16)> = desktop
            .children()
            .iter()
            .map(|child| {
                let bounds = child.state().bounds();
                (bounds.left(), bounds.top())
            })
            .collect();
        // La seconde ne peut pas se décaler : elle occupe déjà tout le bureau.
        assert_eq!(origins, vec![(0, 0), (0, 0)]);
    }

    #[test]
    fn la_mosaique_partage_la_largeur() {
        let mut desktop = desktop(21, 6);
        desktop.insert(window(2, 4, 4));
        desktop.insert(window(3, 4, 4));
        desktop.tile();
        let bounds: Vec<(u16, u16)> = desktop
            .children()
            .iter()
            .map(|child| {
                let rectangle = child.state().bounds();
                (rectangle.left(), rectangle.size.columns)
            })
            .collect();
        // Vingt-et-une colonnes pour deux : dix chacune, et la dernière prend le reste.
        assert_eq!(bounds, vec![(0, 10), (10, 11)]);
    }

    #[test]
    fn la_mosaique_occupe_toute_la_hauteur() {
        let mut desktop = desktop(10, 6);
        desktop.insert(window(2, 4, 2));
        desktop.tile();
        let first = desktop
            .children()
            .first()
            .map(|child| child.state().bounds().size);
        assert_eq!(first, Some(AirSize::new(10, 6)));
    }

    #[test]
    fn une_mosaique_sans_place_ne_fait_rien() {
        // Moins d'une colonne par fenêtre : une mosaïque de largeur nulle cacherait tout.
        let mut desktop = desktop(2, 4);
        desktop.insert(window(2, 2, 4));
        desktop.insert(window(3, 2, 4));
        desktop.insert(window(4, 2, 4));
        desktop.tile();
        let widths: Vec<u16> = desktop
            .children()
            .iter()
            .map(|child| child.state().bounds().size.columns)
            .collect();
        assert_eq!(widths, vec![2, 2, 2]);
    }

    #[test]
    fn une_disposition_sur_un_bureau_vide_ne_panique_pas() {
        let mut desktop = desktop(20, 10);
        desktop.cascade();
        desktop.tile();
        assert!(desktop.is_empty());
    }

    #[test]
    fn un_bureau_sans_zone_de_contenu_ne_dispose_rien() {
        let mut desktop = AirDesktop::new(AirViewState::new(
            identifier(1),
            AirRectangle::from_parts(0, 0, 0, 0),
        ));
        desktop.insert(window(2, 4, 4));
        desktop.cascade();
        desktop.tile();
        assert_eq!(
            desktop
                .children()
                .first()
                .map(|child| child.state().bounds().size),
            Some(AirSize::new(4, 4))
        );
    }

    #[test]
    fn un_bureau_s_affiche_avec_son_compte_de_fenetres() {
        let mut desktop = desktop(20, 10);
        desktop.insert(window(2, 4, 4));
        let text = format!("{desktop:?}");
        assert!(text.contains("AirDesktop"), "rendu : {text}");
        assert!(text.contains("windows: 1"), "rendu : {text}");
    }
}
