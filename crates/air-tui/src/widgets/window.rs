// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La fenêtre — un conteneur à titre et case de fermeture.

use crate::cell::AirCell;
use crate::command::{AirCommand, CLOSE};
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
use crate::input::event::{AirInputEvent, AirMouseButton, AirMouseEventKind};
use crate::theme::AirRole;
use crate::view::painter::AirPainter;
use crate::view::{
    AirBorderStyle, AirContext, AirDrawContext, AirEvent, AirGroup, AirOutcome, AirView, AirViewId,
    AirViewState,
};

/// La colonne où commence la case de fermeture, dans les coordonnées de la fenêtre.
///
/// Deux cellules après l'angle : c'est la position de Turbo Vision, et elle laisse une
/// cellule de trait entre l'angle et le crochet.
const CLOSE_BOX_COLUMN: u16 = 2;
/// La largeur de la case de fermeture : `[`, la marque, `]`.
const CLOSE_BOX_WIDTH: u16 = 3;
/// La largeur minimale pour qu'une case de fermeture tienne sans toucher l'angle opposé.
const MINIMUM_WIDTH_FOR_CLOSE_BOX: u16 = 7;

/// Un conteneur bordé à titre, avec une case de fermeture cliquable.
///
/// Il **contient** un [`AirGroup`] au lieu d'en hériter (ADR-011).
pub struct AirWindow {
    /// Les enfants et l'état commun.
    group: AirGroup,
    /// Le titre inscrit sur la bordure.
    title: Option<String>,
    /// Vrai si la case de fermeture est présente.
    closable: bool,
    /// La commande émise par la case de fermeture.
    close_command: AirCommand,
}

impl AirWindow {
    /// Une fenêtre vide, bordée en trait simple, avec sa case de fermeture.
    #[must_use]
    pub const fn new(state: AirViewState) -> Self {
        Self {
            group: AirGroup::new(state),
            title: None,
            closable: true,
            close_command: CLOSE,
        }
    }

    /// La même fenêtre, avec un titre.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// La même fenêtre, en cadre double — la forme d'un dialogue.
    #[must_use]
    pub fn as_dialog(mut self) -> Self {
        self.group
            .state_mut()
            .set_border_style(AirBorderStyle::Double);
        self
    }

    /// La même fenêtre, sans case de fermeture.
    #[must_use]
    pub const fn without_close_box(mut self) -> Self {
        self.closable = false;
        self
    }

    /// La même fenêtre, avec une autre commande de fermeture.
    ///
    /// Utile quand plusieurs fenêtres coexistent et que l'application doit savoir
    /// laquelle se ferme.
    #[must_use]
    pub const fn with_close_command(mut self, command: AirCommand) -> Self {
        self.close_command = command;
        self
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

    /// Vrai si la case de fermeture est dessinée.
    ///
    /// Elle demande sept colonnes : deux angles, une cellule de trait, trois pour la case,
    /// et une de trait avant l'angle opposé. En dessous, la dessiner mangerait le cadre.
    #[must_use]
    pub fn shows_close_box(&self) -> bool {
        self.closable && self.state().bounds().size.columns >= MINIMUM_WIDTH_FOR_CLOSE_BOX
    }

    /// Le rôle de couleur du cadre, selon le style et le focus.
    fn frame_icon_role(&self) -> AirRole {
        match self.group.state().border_style() {
            AirBorderStyle::Single => AirRole::WindowFrameIcon,
            AirBorderStyle::Double => AirRole::DialogFrameIcon,
        }
    }

    /// Vrai si la position, en coordonnées de la fenêtre, tombe sur la case de fermeture.
    fn hits_close_box(&self, position: AirPoint) -> bool {
        if !self.shows_close_box() || position.row.value() != 0 {
            return false;
        }
        let first = CLOSE_BOX_COLUMN;
        let last = first.saturating_add(CLOSE_BOX_WIDTH).saturating_sub(1);
        let column = position.column.value();
        column >= first && column <= last
    }
}

impl AirView for AirWindow {
    fn state(&self) -> &AirViewState {
        self.group.state()
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        self.group.state_mut()
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let role = match self.group.state().border_style() {
            AirBorderStyle::Single => AirRole::WindowInterior,
            AirBorderStyle::Double => AirRole::DialogInterior,
        };
        let colors = context.theme.palette.color_pair(role);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::new(AirPoint::ORIGIN, size),
            AirCell::new(' ', colors.foreground, colors.background),
        );
    }

    fn draw_frame_decorations(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        if !self.shows_close_box() {
            return;
        }
        let frame_role = match self.group.state().border_style() {
            AirBorderStyle::Single => {
                if self.state().is_focused() {
                    AirRole::WindowFrameActive
                } else {
                    AirRole::WindowFrameInactive
                }
            }
            AirBorderStyle::Double => {
                if self.state().is_focused() {
                    AirRole::DialogFrameActive
                } else {
                    AirRole::DialogFrameInactive
                }
            }
        };
        let frame = context.theme.palette.color_pair(frame_role);
        let mark = context.theme.palette.color_pair(self.frame_icon_role());
        let glyphs = context.theme.glyphs;
        for (offset, character, colors) in [
            (0_u16, glyphs.close_box_left, frame),
            (1, glyphs.close_box_mark, mark),
            (2, glyphs.close_box_right, frame),
        ] {
            painter.set_cell(
                AirPoint::new(
                    AirColumn::new(CLOSE_BOX_COLUMN.saturating_add(offset)),
                    AirRow::FIRST,
                ),
                AirCell::new(character, colors.foreground, colors.background),
            );
        }
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        if let Some(AirInputEvent::Mouse(mouse)) = event.input() {
            let is_press = matches!(mouse.kind, AirMouseEventKind::Pressed(AirMouseButton::Left));
            if is_press && self.hits_close_box(mouse.position) {
                if !context.commands.is_enabled(self.close_command) {
                    // La commande est grisée : le clic est consommé pour que personne
                    // d'autre ne l'interprète, mais rien n'est émis.
                    event.consume();
                    return AirOutcome::Consumed;
                }
                event.consume();
                return AirOutcome::Command(self.close_command);
            }
        }
        AirOutcome::Ignored
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

impl core::fmt::Debug for AirWindow {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AirWindow")
            .field("title", &self.title)
            .field("closable", &self.closable)
            .field("close_command", &self.close_command)
            .field("group", &self.group)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::AirWindow;
    use crate::command::{AirCommand, AirCommandSet, CLOSE};
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirModifiers, AirMouseButton, AirMouseEvent, AirMouseEventKind,
    };
    use crate::surface::AirSurface;
    use crate::theme::{AirRole, AirTheme};
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirBorderStyle, AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId,
        AirViewState, dispatch_event, draw_view,
    };
    use crate::widgets::AirStaticText;

    fn state(value: u32, bounds: AirRectangle) -> AirViewState {
        AirViewState::new(
            AirViewId::new(value).expect("un identifiant non nul"),
            bounds,
        )
    }

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    fn click(column: u16, row: u16) -> AirEvent {
        AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(column, row),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::NONE,
        }))
    }

    fn rendered(surface: &AirSurface, size: AirSize) -> Vec<String> {
        (0..size.rows)
            .map(|row| {
                (0..size.columns)
                    .map(|column| {
                        surface
                            .cell(point(column, row))
                            .map_or(' ', |found| found.character)
                    })
                    .collect()
            })
            .collect()
    }

    fn dispatch(
        view: &mut dyn AirView,
        event: &mut AirEvent,
        commands: &mut AirCommandSet,
    ) -> AirOutcome {
        let mut context = AirContext { commands };
        dispatch_event(view, event, &mut context)
    }

    #[test]
    fn une_fenetre_neuve_est_bordee_en_trait_simple_et_fermable() {
        let window = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6)));
        assert!(window.state().has_border());
        assert_eq!(window.state().border_style(), AirBorderStyle::Single);
        assert!(window.shows_close_box());
        assert_eq!(window.title(), None);
    }

    #[test]
    fn un_dialogue_est_borde_en_trait_double() {
        let dialog = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6))).as_dialog();
        assert_eq!(dialog.state().border_style(), AirBorderStyle::Double);
    }

    #[test]
    fn la_case_de_fermeture_se_retire() {
        let window =
            AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6))).without_close_box();
        assert!(!window.shows_close_box());
    }

    #[test]
    fn une_fenetre_trop_etroite_n_affiche_pas_de_case_de_fermeture() {
        // Sept colonnes au moins : la dessiner en dessous mangerait le cadre.
        for columns in 0..7_u16 {
            let window = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, columns, 6)));
            assert!(!window.shows_close_box(), "pour {columns} colonnes");
        }
        let window = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 7, 6)));
        assert!(window.shows_close_box());
    }

    #[test]
    fn une_fenetre_dessine_son_cadre_son_titre_et_sa_case_de_fermeture() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(16, 4);
        let mut surface = AirSurface::new(size);
        let window =
            AirWindow::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size))).with_title("air");
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &window,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let lines = rendered(&surface, size);
        assert_eq!(lines.first().map(String::as_str), Some("┌─[■] air ─────┐"));
    }

    #[test]
    fn la_case_de_fermeture_survit_a_un_titre_qui_la_recouvrirait() {
        // L'ordre de dessin est cadre, titre, décorations : la case passe en dernier. La
        // précédence est voulue — la case est fonctionnelle, le titre est informatif.
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(11, 3);
        let mut surface = AirSurface::new(size);
        let window = AirWindow::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)))
            .with_title("TitreLong");
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &window,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let lines = rendered(&surface, size);
        let top = lines.first().map(String::as_str).unwrap_or_default();
        assert!(top.contains("[■]"), "la case a été recouverte : {top}");
    }

    #[test]
    fn la_marque_de_fermeture_porte_la_couleur_des_decorations() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(12, 3);
        let mut surface = AirSurface::new(size);
        let window = AirWindow::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size)));
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &window,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let expected = theme.palette.color_pair(AirRole::WindowFrameIcon);
        assert_eq!(
            surface.cell(point(3, 0)).map(|found| found.foreground),
            Some(expected.foreground),
            "la marque doit être vert vif"
        );
    }

    #[test]
    fn un_dialogue_emploie_les_couleurs_de_dialogue_pour_sa_case() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(12, 3);
        let mut surface = AirSurface::new(size);
        let dialog =
            AirWindow::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size))).as_dialog();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &dialog,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let expected = theme.palette.color_pair(AirRole::DialogFrameIcon);
        assert_eq!(
            surface.cell(point(3, 0)).map(|found| found.background),
            Some(expected.background),
            "le fond doit être gris clair, pas bleu"
        );
    }

    #[test]
    fn une_fenetre_sans_case_n_en_dessine_pas() {
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(12, 3);
        let mut surface = AirSurface::new(size);
        let window =
            AirWindow::new(state(1, AirRectangle::new(AirPoint::ORIGIN, size))).without_close_box();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                &window,
                &mut painter,
                &AirDrawContext::new(&theme, &AirCommandSet::new()),
            );
        }
        let lines = rendered(&surface, size);
        assert_eq!(lines.first().map(String::as_str), Some("┌──────────┐"));
    }

    #[test]
    fn un_clic_sur_la_case_de_fermeture_emet_la_commande() {
        let mut commands = AirCommandSet::new();
        let mut window = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6)));
        for column in 2..=4_u16 {
            let mut event = click(column, 0);
            assert_eq!(
                dispatch(&mut window, &mut event, &mut commands),
                AirOutcome::Command(CLOSE),
                "pour la colonne {column}"
            );
            assert!(event.is_consumed());
        }
    }

    #[test]
    fn un_clic_ailleurs_sur_le_cadre_ne_ferme_pas() {
        let mut commands = AirCommandSet::new();
        let mut window = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6)));
        for (column, row) in [(0_u16, 0_u16), (1, 0), (5, 0), (3, 1), (3, 5)] {
            let mut event = click(column, row);
            assert_eq!(
                dispatch(&mut window, &mut event, &mut commands),
                AirOutcome::Ignored,
                "pour ({column}, {row})"
            );
        }
    }

    #[test]
    fn une_commande_de_fermeture_grisee_consomme_le_clic_sans_rien_emettre() {
        // Consommer est délibéré : laisser le clic poursuivre ferait réagir ce qui se
        // trouve dessous, alors que l'utilisateur visait une case grisée.
        let mut commands = AirCommandSet::new();
        commands.disable(CLOSE);
        let mut window = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6)));
        let mut event = click(3, 0);
        assert_eq!(
            dispatch(&mut window, &mut event, &mut commands),
            AirOutcome::Consumed
        );
        assert!(event.is_consumed());
    }

    #[test]
    fn la_commande_de_fermeture_se_choisit() {
        let mut commands = AirCommandSet::new();
        let custom = AirCommand::new(300);
        let mut window = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6)))
            .with_close_command(custom);
        let mut event = click(3, 0);
        assert_eq!(
            dispatch(&mut window, &mut event, &mut commands),
            AirOutcome::Command(custom)
        );
    }

    #[test]
    fn une_fenetre_sans_case_ignore_le_clic_a_sa_position() {
        let mut commands = AirCommandSet::new();
        let mut window =
            AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6))).without_close_box();
        let mut event = click(3, 0);
        assert_eq!(
            dispatch(&mut window, &mut event, &mut commands),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn une_fenetre_delegue_ses_enfants_au_groupe() {
        let mut window = AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6)));
        let inserted = window.insert(Box::new(AirStaticText::new(
            state(2, AirRectangle::from_parts(0, 0, 4, 1)),
            "abc",
        )));
        assert_eq!(inserted.value(), 2);
        assert_eq!(window.children().len(), 1);
        assert_eq!(window.children_mut().len(), 1);
        assert_eq!(window.group().len(), 1);
        assert!(window.group_mut().focus(inserted));
        window.state_mut().set_focused(true);
        assert!(window.state().is_focused());
    }

    #[test]
    fn l_affichage_mentionne_le_titre_et_la_fermabilite() {
        let window =
            AirWindow::new(state(1, AirRectangle::from_parts(0, 0, 20, 6))).with_title("Essai");
        let text = format!("{window:?}");
        assert!(text.contains("Essai"), "rendu : {text}");
        assert!(text.contains("closable"), "rendu : {text}");
    }
}
