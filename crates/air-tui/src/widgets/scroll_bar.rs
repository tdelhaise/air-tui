// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'ascenseur — la position dans un contenu plus grand que sa fenêtre.
//!
//! # Il ne défile rien
//!
//! Un ascenseur ne connaît pas le contenu qu'il représente, et il ne le déplace pas. Il
//! affiche trois nombres — où on est, combien on voit, combien il y a — et il **demande**
//! une nouvelle position quand on le clique. C'est l'appelant qui décide si la demande est
//! satisfaite, par [`AirScrollBar::take_request`].
//!
//! Ce n'est pas une précaution de style : l'ascenseur d'un éditeur et celui d'une liste
//! n'appliquent pas la même borne, et c'est le propriétaire du contenu qui le sait.
//!
//! # Le glissement du curseur n'est pas là
//!
//! Attraper le curseur et le faire glisser demande de suivre un bouton **maintenu** entre
//! deux événements, donc un état « en cours de glissement » et une notion de capture de la
//! souris que l'arbre de vues n'a pas encore. Les flèches, les pages et la molette
//! couvrent tout le reste. C'est une absence, pas un oubli.

use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
use crate::input::event::{AirInputEvent, AirMouseButton, AirMouseEventKind};
use crate::theme::AirRole;
use crate::view::painter::AirPainter;
use crate::view::{AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewState};

/// Le sens d'un ascenseur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AirOrientation {
    /// De haut en bas — le long du bord droit d'une fenêtre.
    #[default]
    Vertical,
    /// De gauche à droite — le long de son bord inférieur.
    Horizontal,
}

/// Un ascenseur.
#[derive(Debug)]
pub struct AirScrollBar {
    /// L'état commun.
    state: AirViewState,
    /// Le sens.
    orientation: AirOrientation,
    /// La première unité visible.
    position: usize,
    /// Le nombre d'unités visibles à la fois.
    visible: usize,
    /// Le nombre total d'unités.
    total: usize,
    /// La position demandée par le dernier geste, pas encore relevée.
    request: Option<usize>,
}

impl AirScrollBar {
    /// Un ascenseur, sans bordure.
    ///
    /// Un ascenseur encadré n'aurait plus de place pour sa glissière : la bordure est donc
    /// retirée dans le constructeur, comme pour les autres petits contrôles (ADR-010).
    #[must_use]
    pub fn new(mut state: AirViewState, orientation: AirOrientation) -> Self {
        state.set_border(false);
        Self {
            state,
            orientation,
            position: 0,
            visible: 1,
            total: 1,
            request: None,
        }
    }

    /// Le sens de l'ascenseur.
    #[must_use]
    pub const fn orientation(&self) -> AirOrientation {
        self.orientation
    }

    /// Déclare ce que l'ascenseur représente.
    ///
    /// `visible` et `total` sont ramenés à au moins un : un contenu de zéro unité n'a pas
    /// de position, et diviser par zéro pour placer un curseur serait une panique dans un
    /// chemin de dessin.
    pub fn set_range(&mut self, position: usize, visible: usize, total: usize) {
        self.total = total.max(1);
        self.visible = visible.max(1).min(self.total);
        self.position = position.min(self.maximum_position());
    }

    /// La première unité visible.
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }

    /// Le nombre d'unités visibles.
    #[must_use]
    pub const fn visible(&self) -> usize {
        self.visible
    }

    /// Le nombre total d'unités.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.total
    }

    /// La position la plus avancée qui laisse la fenêtre pleine.
    #[must_use]
    pub fn maximum_position(&self) -> usize {
        self.total.saturating_sub(self.visible)
    }

    /// Relève la position demandée, et l'oublie.
    ///
    /// Rend `None` quand aucun geste n'a eu lieu depuis le dernier relevé. C'est le même
    /// contrat que les intentions de la vue d'édition : la vue produit une demande, le
    /// contrôleur la consomme.
    pub const fn take_request(&mut self) -> Option<usize> {
        self.request.take()
    }

    /// Demande une position, bornée.
    fn request_position(&mut self, position: usize) {
        self.request = Some(position.min(self.maximum_position()));
    }

    /// La longueur de la glissière, flèches comprises.
    fn length(&self) -> u16 {
        let size = self.state.bounds().size;
        match self.orientation {
            AirOrientation::Vertical => size.rows,
            AirOrientation::Horizontal => size.columns,
        }
    }

    /// La longueur utile, entre les deux flèches.
    fn track_length(&self) -> u16 {
        self.length().saturating_sub(2)
    }

    /// La position du curseur sur la glissière, en cellules depuis la première flèche.
    ///
    /// Rend `None` quand la glissière n'a pas de place — un ascenseur de deux cellules
    /// n'est que ses deux flèches.
    fn thumb_offset(&self) -> Option<u16> {
        let track = self.track_length();
        if track == 0 {
            return None;
        }
        let maximum = self.maximum_position();
        if maximum == 0 {
            // Tout est visible : le curseur est en tête, et il n'y a nulle part à aller.
            return Some(0);
        }
        let last_cell = usize::from(track.saturating_sub(1));
        // Produit puis division, dans cet ordre : diviser d'abord perdrait toute la
        // précision sur un contenu long.
        let scaled = self
            .position
            .saturating_mul(last_cell)
            .checked_div(maximum)
            .unwrap_or(0);
        u16::try_from(scaled).ok().map(|offset| offset.min(track))
    }

    /// Traite un clic, et rend `true` si une demande a été produite.
    fn handle_press(&mut self, position: AirPoint) -> bool {
        let along = match self.orientation {
            AirOrientation::Vertical => position.row.value(),
            AirOrientation::Horizontal => position.column.value(),
        };
        let length = self.length();
        if length == 0 || along >= length {
            return false;
        }
        // La première flèche recule d'une unité, la dernière avance d'une unité.
        if along == 0 {
            self.request_position(self.position.saturating_sub(1));
            return true;
        }
        if along == length.saturating_sub(1) {
            self.request_position(self.position.saturating_add(1));
            return true;
        }
        let Some(thumb) = self.thumb_offset() else {
            return false;
        };
        let on_track = along.saturating_sub(1);
        if on_track < thumb {
            self.request_position(self.position.saturating_sub(self.visible));
            return true;
        }
        if on_track > thumb {
            self.request_position(self.position.saturating_add(self.visible));
            return true;
        }
        // Sur le curseur lui-même : il n'y a rien à faire sans glissement, et déplacer
        // d'une unité serait une surprise.
        false
    }
}

impl AirView for AirScrollBar {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let palette = &context.theme.palette;
        let page = palette.color_pair(AirRole::ScrollBarPage);
        let controls = palette.color_pair(AirRole::ScrollBarControls);
        let glyphs = context.theme.glyphs;
        let size = painter.size();
        let length = self.length();
        if length == 0 {
            return;
        }

        let put = |painter: &mut AirPainter<'_>, along: u16, character: char, inverted: bool| {
            let colors = if inverted { controls } else { page };
            let point = match self.orientation {
                AirOrientation::Vertical => AirPoint::new(AirColumn::FIRST, AirRow::new(along)),
                AirOrientation::Horizontal => AirPoint::new(AirColumn::new(along), AirRow::FIRST),
            };
            painter.set_cell(point, colors.cell(character));
        };

        // La glissière d'abord, pour que les flèches et le curseur s'y posent dessus.
        painter.fill_rectangle(
            AirRectangle::new(AirPoint::ORIGIN, size),
            page.cell(context.theme.glyphs.desktop_fill),
        );
        let (first, last) = match self.orientation {
            AirOrientation::Vertical => (glyphs.scroll_arrow_up, glyphs.scroll_arrow_down),
            // Les flèches horizontales ne sont pas dans le jeu de glyphes du thème : les
            // ajouter pour un seul usage reviendrait à faire porter au thème un détail
            // que Turbo Vision écrivait aussi en dur.
            AirOrientation::Horizontal => ('\u{25C4}', '\u{25BA}'),
        };
        put(painter, 0, first, true);
        put(painter, length.saturating_sub(1), last, true);
        if let Some(thumb) = self.thumb_offset() {
            put(painter, thumb.saturating_add(1), glyphs.scroll_thumb, true);
        }
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        let _ = context;
        if !self.state.is_enabled() {
            return AirOutcome::Ignored;
        }
        let Some(AirInputEvent::Mouse(mouse)) = event.input() else {
            return AirOutcome::Ignored;
        };
        let position = mouse.position;
        let produced = match mouse.kind {
            AirMouseEventKind::Pressed(AirMouseButton::Left) => self.handle_press(position),
            AirMouseEventKind::WheelUp => {
                self.request_position(self.position.saturating_sub(1));
                true
            }
            AirMouseEventKind::WheelDown => {
                self.request_position(self.position.saturating_add(1));
                true
            }
            _ => return AirOutcome::Ignored,
        };
        if !produced {
            // Un clic sur le curseur est bien pour l'ascenseur : il est consommé, sinon il
            // irait au contenu qui est derrière.
            event.consume();
            return AirOutcome::Consumed;
        }
        event.consume();
        AirOutcome::Consumed
    }
}

#[cfg(test)]
mod tests {
    use super::{AirOrientation, AirScrollBar};
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{AirInputEvent, AirMouseButton, AirMouseEvent, AirMouseEventKind};
    use crate::input::event::{AirKey, AirKeyEvent, AirModifiers};
    use crate::surface::AirSurface;
    use crate::theme::AirTheme;
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId, AirViewState,
        draw_view,
    };

    fn vertical(rows: u16) -> AirScrollBar {
        AirScrollBar::new(
            AirViewState::new(AirViewId::FIRST, AirRectangle::from_parts(0, 0, 1, rows)),
            AirOrientation::Vertical,
        )
    }

    fn horizontal(columns: u16) -> AirScrollBar {
        AirScrollBar::new(
            AirViewState::new(AirViewId::FIRST, AirRectangle::from_parts(0, 0, columns, 1)),
            AirOrientation::Horizontal,
        )
    }

    fn click(bar: &mut AirScrollBar, column: u16, row: u16) -> AirOutcome {
        send(
            bar,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            column,
            row,
        )
    }

    fn send(bar: &mut AirScrollBar, kind: AirMouseEventKind, column: u16, row: u16) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            kind,
            modifiers: AirModifiers::default(),
        }));
        bar.handle_event(&mut event, &mut context)
    }

    fn render(bar: &AirScrollBar, size: AirSize) -> String {
        let mut surface = AirSurface::new(size);
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(bar, &mut painter, &AirDrawContext::new(&theme, &commands));
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
    fn un_ascenseur_neuf_est_sans_bordure() {
        assert!(!vertical(10).state().has_border());
    }

    #[test]
    fn une_plage_vide_est_ramenee_a_une_unite() {
        // Sans cette borne, placer le curseur diviserait par zéro.
        let mut bar = vertical(10);
        bar.set_range(0, 0, 0);
        assert_eq!(bar.total(), 1);
        assert_eq!(bar.visible(), 1);
        assert_eq!(bar.maximum_position(), 0);
    }

    #[test]
    fn la_fenetre_ne_depasse_pas_le_contenu() {
        let mut bar = vertical(10);
        bar.set_range(0, 50, 10);
        assert_eq!(bar.visible(), 10);
    }

    #[test]
    fn la_position_est_bornee_a_son_maximum() {
        let mut bar = vertical(10);
        bar.set_range(90, 10, 50);
        assert_eq!(bar.position(), 40);
    }

    #[test]
    fn la_premiere_fleche_recule_d_une_unite() {
        let mut bar = vertical(10);
        bar.set_range(5, 3, 20);
        assert_eq!(click(&mut bar, 0, 0), AirOutcome::Consumed);
        assert_eq!(bar.take_request(), Some(4));
    }

    #[test]
    fn la_derniere_fleche_avance_d_une_unite() {
        let mut bar = vertical(10);
        bar.set_range(5, 3, 20);
        click(&mut bar, 0, 9);
        assert_eq!(bar.take_request(), Some(6));
    }

    #[test]
    fn un_clic_au_dessus_du_curseur_recule_d_une_page() {
        let mut bar = vertical(10);
        bar.set_range(10, 4, 40);
        click(&mut bar, 0, 1);
        assert_eq!(bar.take_request(), Some(6));
    }

    #[test]
    fn un_clic_au_dessous_du_curseur_avance_d_une_page() {
        let mut bar = vertical(10);
        bar.set_range(10, 4, 40);
        click(&mut bar, 0, 8);
        assert_eq!(bar.take_request(), Some(14));
    }

    #[test]
    fn un_clic_sur_le_curseur_ne_demande_rien_mais_consomme() {
        let mut bar = vertical(10);
        bar.set_range(0, 4, 40);
        // Le curseur est en tête de glissière, donc en ligne 1.
        assert_eq!(click(&mut bar, 0, 1), AirOutcome::Consumed);
        assert_eq!(bar.take_request(), None);
    }

    #[test]
    fn une_demande_relevee_n_est_pas_rendue_deux_fois() {
        let mut bar = vertical(10);
        bar.set_range(5, 3, 20);
        click(&mut bar, 0, 0);
        assert_eq!(bar.take_request(), Some(4));
        assert_eq!(bar.take_request(), None);
    }

    #[test]
    fn une_demande_est_bornee_au_maximum() {
        let mut bar = vertical(10);
        bar.set_range(39, 1, 40);
        click(&mut bar, 0, 9);
        assert_eq!(bar.take_request(), Some(39));
    }

    #[test]
    fn la_molette_demande_une_unite_dans_chaque_sens() {
        let mut bar = vertical(10);
        bar.set_range(5, 3, 20);
        send(&mut bar, AirMouseEventKind::WheelDown, 0, 5);
        assert_eq!(bar.take_request(), Some(6));
        send(&mut bar, AirMouseEventKind::WheelUp, 0, 5);
        assert_eq!(bar.take_request(), Some(4));
    }

    #[test]
    fn un_ascenseur_horizontal_lit_la_colonne_et_non_la_ligne() {
        let mut bar = horizontal(10);
        bar.set_range(5, 3, 20);
        click(&mut bar, 9, 0);
        assert_eq!(bar.take_request(), Some(6));
    }

    #[test]
    fn un_ascenseur_desactive_ignore_la_souris() {
        let mut bar = vertical(10);
        bar.state_mut().set_enabled(false);
        assert_eq!(click(&mut bar, 0, 0), AirOutcome::Ignored);
        assert_eq!(bar.take_request(), None);
    }

    #[test]
    fn un_evenement_de_clavier_est_ignore() {
        let mut bar = vertical(10);
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent::press(AirKey::Down)));
        assert_eq!(
            bar.handle_event(&mut event, &mut context),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn un_clic_hors_de_la_glissiere_ne_demande_rien() {
        let mut bar = vertical(4);
        bar.set_range(0, 1, 10);
        click(&mut bar, 0, 9);
        assert_eq!(bar.take_request(), None);
    }

    #[test]
    fn un_ascenseur_de_deux_cellules_n_a_que_ses_fleches() {
        let mut bar = vertical(2);
        bar.set_range(5, 1, 10);
        assert_eq!(bar.thumb_offset(), None);
        // Les deux cellules restent les deux flèches, et elles répondent.
        click(&mut bar, 0, 1);
        assert_eq!(bar.take_request(), Some(6));
    }

    #[test]
    fn le_curseur_est_en_tete_quand_la_position_est_en_tete() {
        let mut bar = vertical(6);
        bar.set_range(0, 2, 20);
        let image = render(&bar, AirSize::new(1, 6));
        assert_eq!(image, "▲\n■\n░\n░\n░\n▼");
    }

    #[test]
    fn le_curseur_est_en_queue_quand_la_position_est_au_maximum() {
        let mut bar = vertical(6);
        bar.set_range(18, 2, 20);
        let image = render(&bar, AirSize::new(1, 6));
        assert_eq!(image, "▲\n░\n░\n░\n■\n▼");
    }

    #[test]
    fn un_contenu_entierement_visible_garde_le_curseur_en_tete() {
        let mut bar = vertical(5);
        bar.set_range(0, 10, 10);
        let image = render(&bar, AirSize::new(1, 5));
        assert_eq!(image, "▲\n■\n░\n░\n▼");
    }

    #[test]
    fn un_ascenseur_horizontal_se_dessine_couche() {
        let mut bar = horizontal(6);
        bar.set_range(0, 2, 20);
        let image = render(&bar, AirSize::new(6, 1));
        assert_eq!(image, "◄■░░░►");
    }

    #[test]
    fn un_ascenseur_de_zero_cellule_ne_dessine_rien() {
        let bar = AirScrollBar::new(
            AirViewState::new(AirViewId::FIRST, AirRectangle::from_parts(0, 0, 1, 0)),
            AirOrientation::Vertical,
        );
        // Ni panique, ni cellule : la vue n'a pas de place.
        assert_eq!(render(&bar, AirSize::new(1, 1)), " ");
    }

    #[test]
    fn le_sens_est_celui_qu_on_a_demande() {
        assert_eq!(vertical(3).orientation(), AirOrientation::Vertical);
        assert_eq!(horizontal(3).orientation(), AirOrientation::Horizontal);
        assert_eq!(AirOrientation::default(), AirOrientation::Vertical);
    }
}
