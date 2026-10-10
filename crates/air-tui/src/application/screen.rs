// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'écran — le chrome plein écran d'une application.
//!
//! # Il existe pour rendre une erreur impossible
//!
//! Au jalon M5b, la barre de menu devait être insérée **en dernier**, sous peine d'être
//! recouverte par le bureau au dessin comme au routage. C'était écrit dans la
//! documentation, et une contrainte qui ne tient que par la documentation est une
//! contrainte qu'on finit par violer.
//!
//! [`AirScreen`] range ses trois pièces lui-même, dans le seul ordre qui marche, quel que
//! soit l'ordre des appels :
//!
//! ```text
//! enfants = [ bureau, barre de statut, barre de menu ]
//!             arrière ──────────────────────▶ avant
//! ```
//!
//! Le bureau est au fond parce qu'il est le fond ; la barre de menu est devant parce que
//! son menu déroulant doit recouvrir ce qui est derrière. On ne peut plus se tromper.
//!
//! # Et il porte la disposition
//!
//! [`AirScreen::resize`] place la barre de menu sur la première ligne, la barre de statut
//! sur la dernière, et donne au bureau ce qui reste. L'application n'a plus à recalculer
//! trois rectangles à chaque changement de dimension de terminal — ce que l'exemple du
//! jalon M5b faisait à la main.
//!
//! # Un ajout par rapport à Turbo Vision
//!
//! Turbo Vision n'avait qu'un seul chrome d'application, figé. Un écran est ici un objet
//! que [`crate::application::AirApplication`] **empile** : un écran d'accueil, un
//! assistant à étapes, et surtout la cohabitation de deux chromes radicalement différents
//! — `redit` en style Turbo Vision, `rmc` en style Midnight Commander — sur le même
//! moteur.

use crate::geometry::{AirRectangle, AirSize};
use crate::view::painter::AirPainter;
use crate::view::{AirDrawContext, AirView, AirViewId, AirViewState};

/// Le chrome plein écran d'une application.
pub struct AirScreen {
    /// L'état commun.
    state: AirViewState,
    /// Les enfants, dans l'ordre imposé : bureau, barre de statut, barre de menu.
    children: Vec<Box<dyn AirView>>,
    /// Vrai si un bureau a été posé.
    has_desktop: bool,
    /// Vrai si une barre de statut a été posée.
    has_status_line: bool,
    /// Vrai si une barre de menu a été posée.
    has_menu_bar: bool,
}

impl AirScreen {
    /// Un écran vide, sans bordure.
    ///
    /// L'écran est l'écran, pas une fenêtre : un cadre autour de lui encadrerait le
    /// terminal.
    #[must_use]
    pub fn new(mut state: AirViewState) -> Self {
        state.set_border(false);
        Self {
            state,
            children: Vec::new(),
            has_desktop: false,
            has_status_line: false,
            has_menu_bar: false,
        }
    }

    /// L'indice du bureau dans la liste des enfants.
    fn desktop_index(&self) -> Option<usize> {
        if self.has_desktop { Some(0) } else { None }
    }

    /// L'indice de la barre de statut.
    fn status_line_index(&self) -> Option<usize> {
        if !self.has_status_line {
            return None;
        }
        Some(usize::from(self.has_desktop))
    }

    /// L'indice de la barre de menu : toujours le dernier.
    fn menu_bar_index(&self) -> Option<usize> {
        if !self.has_menu_bar {
            return None;
        }
        self.children.len().checked_sub(1)
    }

    /// Pose le bureau, qui reçoit le focus.
    ///
    /// Le bureau est **au fond** de la pile d'enfants : c'est le fond. Il reçoit le focus
    /// parce que c'est par lui que les frappes atteignent la fenêtre active ; sans cela, la
    /// phase du focus ne trouverait personne.
    pub fn set_desktop(&mut self, mut desktop: Box<dyn AirView>) {
        desktop.state_mut().set_focused(true);
        if let Some(index) = self.desktop_index() {
            if let Some(slot) = self.children.get_mut(index) {
                *slot = desktop;
            }
        } else {
            self.children.insert(0, desktop);
            self.has_desktop = true;
        }
        self.relayout();
    }

    /// Pose la barre de statut, en **post-traitement**.
    ///
    /// La phase est imposée ici et non laissée à l'appelant : une barre de statut qui
    /// verrait les touches avant la vue focalisée les lui volerait.
    pub fn set_status_line(&mut self, mut status_line: Box<dyn AirView>) {
        status_line.state_mut().set_post_process(true);
        if let Some(index) = self.status_line_index() {
            if let Some(slot) = self.children.get_mut(index) {
                *slot = status_line;
            }
        } else {
            let index = usize::from(self.has_desktop);
            self.children.insert(index, status_line);
            self.has_status_line = true;
        }
        self.relayout();
    }

    /// Pose la barre de menu, en **pré-traitement** et au premier plan.
    ///
    /// Les deux sont imposés, et pour deux raisons différentes : le pré-traitement pour
    /// qu'`Alt`+lettre parvienne à la barre avant le champ de saisie qui a le focus, et le
    /// premier plan pour que le menu déroulant recouvre ce qui est derrière.
    pub fn set_menu_bar(&mut self, mut menu_bar: Box<dyn AirView>) {
        menu_bar.state_mut().set_pre_process(true);
        if let Some(index) = self.menu_bar_index() {
            if let Some(slot) = self.children.get_mut(index) {
                *slot = menu_bar;
            }
        } else {
            self.children.push(menu_bar);
            self.has_menu_bar = true;
        }
        self.relayout();
    }

    /// Le bureau, en écriture.
    pub fn desktop_mut(&mut self) -> Option<&mut (dyn AirView + 'static)> {
        let index = self.desktop_index()?;
        self.children.get_mut(index).map(Box::as_mut)
    }

    /// Le bureau.
    #[must_use]
    pub fn desktop(&self) -> Option<&dyn AirView> {
        let index = self.desktop_index()?;
        self.children.get(index).map(Box::as_ref)
    }

    /// La barre de statut, en écriture.
    pub fn status_line_mut(&mut self) -> Option<&mut (dyn AirView + 'static)> {
        let index = self.status_line_index()?;
        self.children.get_mut(index).map(Box::as_mut)
    }

    /// La barre de menu, en écriture.
    pub fn menu_bar_mut(&mut self) -> Option<&mut (dyn AirView + 'static)> {
        let index = self.menu_bar_index()?;
        self.children.get_mut(index).map(Box::as_mut)
    }

    /// L'identifiant du bureau, s'il y en a un.
    #[must_use]
    pub fn desktop_identifier(&self) -> Option<AirViewId> {
        self.desktop().map(|desktop| desktop.state().identifier())
    }

    /// La dimension de l'écran.
    #[must_use]
    pub fn size(&self) -> AirSize {
        self.state.bounds().size
    }

    /// Change la dimension de l'écran et replace ses trois pièces.
    pub fn resize(&mut self, size: AirSize) {
        self.state
            .set_bounds(AirRectangle::from_parts(0, 0, size.columns, size.rows));
        self.relayout();
    }

    /// Replace les trois pièces dans la dimension courante.
    ///
    /// La barre de menu mange la première ligne, la barre de statut la dernière, et le
    /// bureau prend ce qui reste. Un écran trop petit pour les trois donne un bureau de
    /// hauteur nulle plutôt qu'un chevauchement : une barre à moitié recouverte est pire
    /// qu'un bureau absent.
    fn relayout(&mut self) {
        let size = self.state.bounds().size;
        let top = u16::from(self.has_menu_bar);
        let bottom = u16::from(self.has_status_line);
        let desktop_rows = size.rows.saturating_sub(top).saturating_sub(bottom);

        if let Some(index) = self.desktop_index()
            && let Some(desktop) = self.children.get_mut(index)
        {
            desktop.state_mut().set_bounds(AirRectangle::from_parts(
                0,
                top,
                size.columns,
                desktop_rows,
            ));
        }
        if let Some(index) = self.status_line_index()
            && let Some(status_line) = self.children.get_mut(index)
        {
            let row = size.rows.saturating_sub(1);
            status_line
                .state_mut()
                .set_bounds(AirRectangle::from_parts(0, row, size.columns, 1));
        }
        if let Some(index) = self.menu_bar_index()
            && let Some(menu_bar) = self.children.get_mut(index)
        {
            menu_bar
                .state_mut()
                .set_bounds(AirRectangle::from_parts(0, 0, size.columns, 1));
        }
    }
}

impl AirView for AirScreen {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        // Un écran n'a pas de contenu propre : ses trois pièces couvrent tout, et la
        // traversée les dessine après cet appel.
        let _ = (painter, context);
    }

    fn children(&self) -> &[Box<dyn AirView>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [Box<dyn AirView>] {
        &mut self.children
    }
}

impl core::fmt::Debug for AirScreen {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AirScreen")
            .field("identifier", &self.state.identifier())
            .field("size", &self.state.bounds().size)
            .field("desktop", &self.has_desktop)
            .field("status_line", &self.has_status_line)
            .field("menu_bar", &self.has_menu_bar)
            .field("children", &self.children.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::AirScreen;
    use crate::application::AirDesktop;
    use crate::command::{AirCommandSet, QUIT};
    use crate::geometry::{AirRectangle, AirSize};
    use crate::input::event::AirKey;
    use crate::view::{AirView, AirViewId, AirViewState};
    use crate::widgets::{AirMenu, AirMenuBar, AirMenuItem, AirStatusItem, AirStatusLine};

    fn identifier(value: u32) -> AirViewId {
        AirViewId::new(value).expect("un identifiant non nul")
    }

    fn screen(columns: u16, rows: u16) -> AirScreen {
        AirScreen::new(AirViewState::new(
            identifier(1),
            AirRectangle::from_parts(0, 0, columns, rows),
        ))
    }

    fn desktop() -> Box<dyn AirView> {
        Box::new(AirDesktop::new(AirViewState::new(
            identifier(2),
            AirRectangle::from_parts(0, 0, 1, 1),
        )))
    }

    fn menu_bar() -> Box<dyn AirView> {
        Box::new(
            AirMenuBar::new(AirViewState::new(
                identifier(3),
                AirRectangle::from_parts(0, 0, 1, 1),
            ))
            .with_menu(AirMenu::new("~F~ile").with_item(AirMenuItem::new("E~x~it", QUIT))),
        )
    }

    fn status_line() -> Box<dyn AirView> {
        Box::new(
            AirStatusLine::new(AirViewState::new(
                identifier(4),
                AirRectangle::from_parts(0, 0, 1, 1),
            ))
            .with_item(AirStatusItem::new("~F10~ Exit", AirKey::Function(10), QUIT)),
        )
    }

    fn identifiers(screen: &AirScreen) -> Vec<u32> {
        screen
            .children()
            .iter()
            .map(|child| child.state().identifier().value())
            .collect()
    }

    #[test]
    fn un_ecran_neuf_est_vide_et_sans_bordure() {
        let screen = screen(80, 25);
        assert_eq!(screen.children().len(), 0);
        assert!(!screen.state().has_border());
        assert_eq!(screen.size(), AirSize::new(80, 25));
    }

    #[test]
    fn la_barre_de_menu_est_en_dernier_quel_que_soit_l_ordre_des_appels() {
        // C'est la raison d'être de cet objet : au jalon M5b, cette contrainte ne tenait
        // que par la documentation.
        let mut first = screen(80, 25);
        first.set_menu_bar(menu_bar());
        first.set_desktop(desktop());
        first.set_status_line(status_line());
        assert_eq!(identifiers(&first), vec![2, 4, 3]);

        let mut second = screen(80, 25);
        second.set_desktop(desktop());
        second.set_status_line(status_line());
        second.set_menu_bar(menu_bar());
        assert_eq!(identifiers(&second), vec![2, 4, 3]);
    }

    #[test]
    fn la_barre_de_menu_est_en_pre_traitement_et_la_barre_de_statut_en_post() {
        let mut screen = screen(80, 25);
        screen.set_menu_bar(menu_bar());
        screen.set_status_line(status_line());
        let menu = screen.menu_bar_mut().expect("une barre de menu");
        assert!(menu.state().is_pre_process());
        assert!(!menu.state().is_post_process());
        let status = screen.status_line_mut().expect("une barre de statut");
        assert!(status.state().is_post_process());
        assert!(!status.state().is_pre_process());
    }

    #[test]
    fn le_bureau_recoit_le_focus() {
        // Sans lui, la phase du focus ne trouverait personne et aucune frappe n'atteindrait
        // la fenêtre active.
        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        assert!(
            screen
                .desktop_mut()
                .expect("un bureau")
                .state()
                .is_focused()
        );
    }

    #[test]
    fn la_disposition_donne_une_ligne_a_chaque_barre_et_le_reste_au_bureau() {
        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        screen.set_status_line(status_line());
        screen.set_menu_bar(menu_bar());
        let bounds: Vec<AirRectangle> = screen
            .children()
            .iter()
            .map(|child| child.state().bounds())
            .collect();
        assert_eq!(
            bounds,
            vec![
                AirRectangle::from_parts(0, 1, 80, 23),
                AirRectangle::from_parts(0, 24, 80, 1),
                AirRectangle::from_parts(0, 0, 80, 1),
            ]
        );
    }

    #[test]
    fn un_bureau_seul_occupe_tout_l_ecran() {
        let mut screen = screen(40, 10);
        screen.set_desktop(desktop());
        assert_eq!(
            screen.desktop().map(|view| view.state().bounds()),
            Some(AirRectangle::from_parts(0, 0, 40, 10))
        );
    }

    #[test]
    fn un_redimensionnement_replace_les_trois_pieces() {
        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        screen.set_status_line(status_line());
        screen.set_menu_bar(menu_bar());
        screen.resize(AirSize::new(100, 40));
        assert_eq!(screen.size(), AirSize::new(100, 40));
        let bounds: Vec<AirRectangle> = screen
            .children()
            .iter()
            .map(|child| child.state().bounds())
            .collect();
        assert_eq!(
            bounds,
            vec![
                AirRectangle::from_parts(0, 1, 100, 38),
                AirRectangle::from_parts(0, 39, 100, 1),
                AirRectangle::from_parts(0, 0, 100, 1),
            ]
        );
    }

    #[test]
    fn un_ecran_trop_petit_donne_un_bureau_de_hauteur_nulle() {
        // Une barre à moitié recouverte est pire qu'un bureau absent.
        let mut screen = screen(20, 2);
        screen.set_desktop(desktop());
        screen.set_status_line(status_line());
        screen.set_menu_bar(menu_bar());
        assert_eq!(
            screen.desktop().map(|view| view.state().bounds().size.rows),
            Some(0)
        );
    }

    #[test]
    fn reposer_une_piece_la_remplace_sans_la_dupliquer() {
        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        screen.set_menu_bar(menu_bar());
        screen.set_desktop(Box::new(AirDesktop::new(AirViewState::new(
            identifier(9),
            AirRectangle::from_parts(0, 0, 1, 1),
        ))));
        assert_eq!(identifiers(&screen), vec![9, 3]);
    }

    #[test]
    fn reposer_la_barre_de_menu_la_laisse_en_dernier() {
        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        screen.set_menu_bar(menu_bar());
        screen.set_menu_bar(Box::new(AirMenuBar::new(AirViewState::new(
            identifier(8),
            AirRectangle::from_parts(0, 0, 1, 1),
        ))));
        assert_eq!(identifiers(&screen), vec![2, 8]);
    }

    #[test]
    fn reposer_la_barre_de_statut_la_laisse_au_milieu() {
        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        screen.set_status_line(status_line());
        screen.set_menu_bar(menu_bar());
        screen.set_status_line(Box::new(AirStatusLine::new(AirViewState::new(
            identifier(7),
            AirRectangle::from_parts(0, 0, 1, 1),
        ))));
        assert_eq!(identifiers(&screen), vec![2, 7, 3]);
    }

    #[test]
    fn une_piece_absente_ne_rend_rien() {
        let mut screen = screen(80, 25);
        assert!(screen.desktop().is_none());
        assert!(screen.desktop_mut().is_none());
        assert!(screen.status_line_mut().is_none());
        assert!(screen.menu_bar_mut().is_none());
        assert!(screen.desktop_identifier().is_none());
    }

    #[test]
    fn l_identifiant_du_bureau_est_celui_qu_on_a_pose() {
        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        assert_eq!(screen.desktop_identifier(), Some(identifier(2)));
    }

    #[test]
    fn un_ecran_s_affiche_avec_ses_trois_drapeaux() {
        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        let text = format!("{screen:?}");
        assert!(text.contains("AirScreen"), "rendu : {text}");
        assert!(text.contains("desktop: true"), "rendu : {text}");
        assert!(text.contains("menu_bar: false"), "rendu : {text}");
    }

    #[test]
    fn un_ecran_route_les_evenements_a_ses_enfants() {
        use crate::input::event::{AirInputEvent, AirKeyEvent};
        use crate::view::{AirContext, AirEvent, AirOutcome, dispatch_event};

        let mut screen = screen(80, 25);
        screen.set_desktop(desktop());
        screen.set_status_line(status_line());
        screen.set_menu_bar(menu_bar());
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        // `F10` est publiée par la barre de statut, qui est en post-traitement : elle la
        // reçoit parce que personne devant elle ne l'a prise.
        let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent::press(AirKey::Function(10))));
        assert_eq!(
            dispatch_event(&mut screen, &mut event, &mut context),
            AirOutcome::Command(QUIT)
        );
    }
}
