// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La liste — des items, un curseur, et l'ascenseur qu'elle pilote.
//!
//! # Le reflet est complet, et c'est un choix
//!
//! [`AirEditorView`](crate::widgets::AirEditorView) ne garde que les lignes visibles d'un
//! tampon, parce qu'un fichier peut faire des millions de lignes. Une liste garde **tous**
//! ses items : un répertoire, une liste de polices, un historique tiennent en mémoire, et
//! le reflet partiel coûterait ici une complication sans contrepartie.
//!
//! Ce qui ne change pas, c'est le sens de la flèche : la vue ne porte aucune référence
//! vers le modèle, et c'est le contrôleur qui lui pousse les items par
//! [`AirListBox::set_items`] à l'étape 3 du tour de boucle de l'ADR-007.
//!
//! # Le curseur appartient à la vue, le marquage au modèle
//!
//! Se déplacer dans une liste ne change rien à ce qu'elle montre : le curseur et le
//! défilement sont donc ici, comme le défilement de la vue d'édition est dans la vue
//! d'édition. **Marquer** un item est autre chose — c'est un fait que l'application garde,
//! qui survit à la fermeture de la liste et qui commande ce qu'une touche de fonction fera
//! ensuite. La vue ne marque donc rien : elle produit une
//! [`AirListRequest::ToggleMark`] que le contrôleur applique, et reçoit en retour
//! l'état de marquage dans [`AirListItem::marked`].
//!
//! # Elle ne défile pas l'ascenseur, et l'ascenseur ne la défile pas
//!
//! Les deux sont indépendants, et c'est le contrôleur qui les accorde en deux lignes :
//!
//! ```text
//! bar.set_range(list.top_item(), list.visible_rows(), list.item_count());
//! if let Some(position) = bar.take_request() { list.scroll_to(position); }
//! ```
//!
//! Donner à la liste une référence vers son ascenseur ferait exactement l'arête arrière
//! que l'ADR-007 refuse au modèle.
//!
//! # Ce qui n'est pas là
//!
//! Le **double-clic** : le modèle d'entrée ne le rapporte pas, parce que le distinguer de
//! deux clics demande un horodatage que le décodeur ne porte pas (jalon M1). Un clic
//! déplace donc le curseur, et c'est `Entrée` qui active. L'absence est ici plutôt que
//! remplacée par une approximation : activer au clic sur l'item déjà pointé surprendrait.
//!
//! Les **colonnes multiples** : `mc` sait afficher un répertoire sur deux ou trois
//! colonnes, et `TListViewer` portait un `numCols`. Une colonne suffit à un panneau de
//! fichiers, et la seconde demande de décider ce que `PageDown` veut dire.

use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
use crate::theme::{AirColorPair, AirRole};
use crate::unicode::{grapheme_width, graphemes};
use crate::view::painter::AirPainter;
use crate::view::{AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewState};

/// Un item de liste, tel que le contrôleur le pousse.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AirListItem {
    /// Le libellé, écrit à partir du bord gauche.
    pub text: String,
    /// Une précision écrite contre le bord droit — une taille, une date, un compte.
    ///
    /// Vide quand il n'y en a pas. Elle a la priorité sur le libellé : c'est le libellé
    /// qui est tronqué quand les deux ne tiennent pas, parce qu'une taille coupée en deux
    /// ne veut plus rien dire.
    pub detail: String,
    /// Vrai quand l'application tient cet item pour marqué.
    pub marked: bool,
}

impl AirListItem {
    /// Un item qui ne porte qu'un libellé.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            detail: String::new(),
            marked: false,
        }
    }

    /// Le même item, avec sa précision de droite.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// Le même item, marqué.
    #[must_use]
    pub const fn marked(mut self) -> Self {
        self.marked = true;
        self
    }
}

/// Ce qu'une liste demande à son contrôleur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirListRequest {
    /// `Entrée` sur l'item d'indice donné — l'ouvrir, le choisir, y descendre.
    Activate(usize),
    /// `Inser` sur l'item d'indice donné — le marquer, ou le démarquer.
    ToggleMark(usize),
}

/// Une liste d'items à une colonne.
#[derive(Debug)]
pub struct AirListBox {
    /// L'état commun.
    state: AirViewState,
    /// Le reflet complet des items.
    items: Vec<AirListItem>,
    /// Le premier item visible.
    top_item: usize,
    /// L'item sous le curseur.
    focused_item: usize,
    /// Les demandes produites et non encore relevées.
    requests: Vec<AirListRequest>,
}

impl AirListBox {
    /// Une liste vide, sans bordure.
    ///
    /// La bordure est retirée pour la raison d'[`AirEditorView`](crate::widgets::AirEditorView) :
    /// une liste est le contenu d'un panneau qui porte déjà son cadre, et deux cadres
    /// emboîtés mangeraient deux items (ADR-010).
    #[must_use]
    pub fn new(mut state: AirViewState) -> Self {
        state.set_border(false);
        Self {
            state,
            items: Vec::new(),
            top_item: 0,
            focused_item: 0,
            requests: Vec::new(),
        }
    }

    /// Remplace le reflet.
    ///
    /// Le curseur et le défilement sont ramenés dans le nouveau contenu : une liste qui
    /// rétrécit ne garde pas un curseur au-delà de son dernier item.
    pub fn set_items(&mut self, items: Vec<AirListItem>) {
        self.items = items;
        self.focused_item = self.focused_item.min(self.last_item());
        self.scroll_to(self.top_item);
    }

    /// Les items reflétés.
    #[must_use]
    pub fn items(&self) -> &[AirListItem] {
        &self.items
    }

    /// Le nombre d'items.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Vrai quand la liste est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// L'indice de l'item sous le curseur.
    ///
    /// Vaut zéro sur une liste vide, où il ne désigne rien : [`Self::focused`] rend alors
    /// `None`, et c'est elle qu'il faut interroger pour agir.
    #[must_use]
    pub const fn focused_item(&self) -> usize {
        self.focused_item
    }

    /// L'item sous le curseur, s'il y en a un.
    #[must_use]
    pub fn focused(&self) -> Option<&AirListItem> {
        self.items.get(self.focused_item)
    }

    /// Le premier item visible.
    #[must_use]
    pub const fn top_item(&self) -> usize {
        self.top_item
    }

    /// Le nombre d'items que la liste montre à la fois.
    ///
    /// Au moins un : une liste d'aucune ligne visible ferait diviser par zéro le calcul
    /// des pages, et une page d'aucun item n'avancerait jamais.
    #[must_use]
    pub fn visible_rows(&self) -> usize {
        let rows = self
            .state
            .content_bounds()
            .map_or(0, |bounds| bounds.size.rows);
        usize::from(rows).max(1)
    }

    /// Le dernier indice valable, ou zéro sur une liste vide.
    fn last_item(&self) -> usize {
        self.items.len().saturating_sub(1)
    }

    /// Le plus grand premier item visible qui laisse la fenêtre pleine.
    fn maximum_top(&self) -> usize {
        self.items.len().saturating_sub(self.visible_rows())
    }

    /// Place le premier item visible, borné.
    pub fn scroll_to(&mut self, top_item: usize) {
        self.top_item = top_item.min(self.maximum_top());
    }

    /// Place le curseur et le ramène dans la fenêtre.
    pub fn focus_item(&mut self, index: usize) {
        self.focused_item = index.min(self.last_item());
        self.reveal();
    }

    /// Fait défiler le minimum pour que le curseur soit visible.
    fn reveal(&mut self) {
        let rows = self.visible_rows();
        if self.focused_item < self.top_item {
            self.top_item = self.focused_item;
            return;
        }
        let last_visible = self.top_item.saturating_add(rows.saturating_sub(1));
        if self.focused_item > last_visible {
            self.top_item = self
                .focused_item
                .saturating_add(1)
                .saturating_sub(rows)
                .min(self.maximum_top());
        }
    }

    /// Relève les demandes produites, et les oublie.
    pub fn drain_requests(&mut self) -> std::vec::Drain<'_, AirListRequest> {
        self.requests.drain(..)
    }

    /// Vrai quand une demande attend d'être relevée.
    #[must_use]
    pub fn has_pending_requests(&self) -> bool {
        !self.requests.is_empty()
    }

    /// Les couleurs d'un item, selon qu'il est marqué et sous le curseur.
    ///
    /// Le curseur ne se voit que sur une liste **focalisée** : c'est ce que font `mc` et
    /// `TListViewer`, et c'est ce qui permet de savoir du premier coup d'œil lequel des
    /// deux panneaux reçoit les touches.
    fn colors(&self, index: usize, palette: &crate::theme::AirPalette) -> AirColorPair {
        let marked = self.items.get(index).is_some_and(|item| item.marked);
        let selected = self.state.is_focused() && index == self.focused_item;
        let role = match (marked, selected) {
            (true, true) => AirRole::ListMarkedSelected,
            (true, false) => AirRole::ListMarked,
            (false, true) => AirRole::ListSelected,
            (false, false) => AirRole::ListNormal,
        };
        palette.color_pair(role)
    }

    /// Traite une touche, et rend vrai quand elle a servi.
    fn handle_key(&mut self, key: AirKey) -> bool {
        let rows = self.visible_rows();
        match key {
            AirKey::Up => self.focus_item(self.focused_item.saturating_sub(1)),
            AirKey::Down => self.focus_item(self.focused_item.saturating_add(1)),
            AirKey::PageUp => self.focus_item(self.focused_item.saturating_sub(rows)),
            AirKey::PageDown => self.focus_item(self.focused_item.saturating_add(rows)),
            AirKey::Home => self.focus_item(0),
            AirKey::End => self.focus_item(self.last_item()),
            AirKey::Enter => {
                if self.items.is_empty() {
                    return false;
                }
                self.requests
                    .push(AirListRequest::Activate(self.focused_item));
            }
            AirKey::Insert => {
                if self.items.is_empty() {
                    return false;
                }
                // `mc` avance d'un item après avoir marqué : marquer une suite de
                // fichiers se fait alors en pressant `Inser` autant de fois.
                self.requests
                    .push(AirListRequest::ToggleMark(self.focused_item));
                self.focus_item(self.focused_item.saturating_add(1));
            }
            _ => return false,
        }
        true
    }

    /// Traite un geste de souris, et rend vrai quand il a servi.
    fn handle_mouse(&mut self, kind: AirMouseEventKind, row: u16) -> bool {
        match kind {
            AirMouseEventKind::Pressed(AirMouseButton::Left) => {
                let index = self.top_item.saturating_add(usize::from(row));
                if index >= self.items.len() {
                    // Un clic sous le dernier item ne déplace rien : il est consommé
                    // parce qu'il est bien pour la liste, mais il ne désigne pas d'item.
                    return true;
                }
                self.focus_item(index);
                true
            }
            // Le double clic **ouvre**, comme dans Midnight Commander et dans tout
            // gestionnaire de fichiers. L'enfoncement qui le précède a déjà posé le
            // curseur, donc la demande porte sur le bon item sans qu'il faille le
            // recalculer ici.
            AirMouseEventKind::DoubleClicked(AirMouseButton::Left) => {
                let index = self.focused_item;
                if index >= self.items.len() {
                    return true;
                }
                self.requests.push(AirListRequest::Activate(index));
                true
            }
            AirMouseEventKind::WheelUp => {
                self.scroll_to(self.top_item.saturating_sub(3));
                true
            }
            AirMouseEventKind::WheelDown => {
                self.scroll_to(self.top_item.saturating_add(3));
                true
            }
            _ => false,
        }
    }
}

/// La plus longue tranche de tête dont la largeur d'affichage tient dans `columns`.
///
/// Le parcours est par grappes de graphème (ADR-006) : une grappe large qui ne tient pas
/// entière est écartée, parce qu'en afficher la moitié produirait un glyphe tronqué.
fn fitted(text: &str, columns: usize) -> &str {
    let mut offset: usize = 0;
    let mut walked: usize = 0;
    for cluster in graphemes(text) {
        let width = grapheme_width(cluster);
        if walked.saturating_add(width) > columns {
            break;
        }
        walked = walked.saturating_add(width);
        offset = offset.saturating_add(cluster.len());
    }
    text.get(..offset).unwrap_or(text)
}

/// La largeur d'affichage d'un texte, en colonnes.
fn width_of(text: &str) -> usize {
    graphemes(text).map(grapheme_width).sum()
}

impl AirView for AirListBox {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let palette = &context.theme.palette;
        let size = painter.size();
        let columns = usize::from(size.columns);
        for row in 0..size.rows {
            let index = self.top_item.saturating_add(usize::from(row));
            let colors = self.colors(index, palette);
            let line = AirRectangle::from_parts(0, row, size.columns, 1);
            painter.fill_rectangle(line, colors.cell(' '));
            let Some(item) = self.items.get(index) else {
                // Sous le dernier item, la liste n'écrit rien : le fond ordinaire est
                // déjà posé, et inventer un contenu serait un mensonge visuel.
                continue;
            };
            let detail_width = width_of(&item.detail);
            // La précision passe avant le libellé : c'est le libellé qui cède la place.
            let text_columns = if detail_width == 0 {
                columns
            } else {
                columns.saturating_sub(detail_width.saturating_add(1))
            };
            let text = fitted(&item.text, text_columns);
            painter.draw_text_with_colors(
                AirPoint::new(AirColumn::FIRST, AirRow::new(row)),
                text,
                colors,
            );
            if detail_width > 0 && detail_width <= columns {
                let start = columns.saturating_sub(detail_width);
                let column = u16::try_from(start).unwrap_or(u16::MAX);
                painter.draw_text_with_colors(
                    AirPoint::new(AirColumn::new(column), AirRow::new(row)),
                    &item.detail,
                    colors,
                );
            }
        }
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        let _ = context;
        if !self.state.is_enabled() {
            return AirOutcome::Ignored;
        }
        let Some(input) = event.input() else {
            return AirOutcome::Ignored;
        };
        let served = match input {
            AirInputEvent::Key(key) => {
                // Un modificateur change le sens d'une touche : `Ctrl`+`Début` n'est pas
                // `Début`, et la liste ne doit pas prendre ce qu'elle ne traite pas.
                if key.modifiers.control || key.modifiers.alternate {
                    return AirOutcome::Ignored;
                }
                // Les relâchements de touche ne parviennent pas ici : la distribution
                // les écarte avant la descente (`crate::view::dispatch_event`). Ce widget
                // portait ce filtre au jalon M7b, seul du dépôt à le porter — et il ne
                // protégeait presque rien, le décodeur rendant alors un enfoncement pour
                // tout relâchement de flèche.
                self.handle_key(key.logical)
            }
            AirInputEvent::Mouse(mouse) => {
                self.handle_mouse(mouse.kind, mouse.position.row.value())
            }
            _ => return AirOutcome::Ignored,
        };
        if !served {
            return AirOutcome::Ignored;
        }
        event.consume();
        AirOutcome::Consumed
    }

    fn resize(&mut self, size: crate::geometry::AirSize) {
        let bounds = self.state.bounds();
        self.state
            .set_bounds(AirRectangle::new(bounds.origin, size));
        // La fenêtre a changé de hauteur : le curseur doit y rentrer, et le défilement
        // ne doit pas laisser de lignes vides sous le dernier item.
        self.scroll_to(self.top_item);
        self.reveal();
    }
}

#[cfg(test)]
mod tests {
    use super::{AirListBox, AirListItem, AirListRequest, fitted, width_of};
    use crate::cell::AirColor;
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirKeyState, AirModifiers, AirMouseButton,
        AirMouseEvent, AirMouseEventKind,
    };
    use crate::surface::AirSurface;
    use crate::theme::{AirRole, AirTheme};
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId, AirViewState,
        draw_view,
    };

    /// Une liste de `rows` lignes visibles, garnie de `count` items numérotés.
    fn list(rows: u16, count: usize) -> AirListBox {
        let mut list = AirListBox::new(AirViewState::new(
            AirViewId::FIRST,
            AirRectangle::from_parts(0, 0, 20, rows),
        ));
        list.set_items(
            (0..count)
                .map(|index| AirListItem::new(format!("item {index}")))
                .collect(),
        );
        list
    }

    fn press(list: &mut AirListBox, key: AirKey) -> AirOutcome {
        send_key(list, key, AirKeyState::Press, AirModifiers::default())
    }

    fn send_key(
        list: &mut AirListBox,
        key: AirKey,
        state: AirKeyState,
        modifiers: AirModifiers,
    ) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent {
            logical: key,
            modifiers,
            state,
            physical: None,
            base_layout: None,
        }));
        list.handle_event(&mut event, &mut context)
    }

    fn send_mouse(
        list: &mut AirListBox,
        kind: AirMouseEventKind,
        column: u16,
        row: u16,
    ) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            kind,
            modifiers: AirModifiers::default(),
        }));
        list.handle_event(&mut event, &mut context)
    }

    /// Le dessin, rendu en lignes de caractères.
    fn render(list: &AirListBox, size: AirSize) -> Vec<String> {
        let surface = painted(list, size);
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
        lines
    }

    fn painted(list: &AirListBox, size: AirSize) -> AirSurface {
        let mut surface = AirSurface::new(size);
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(list, &mut painter, &AirDrawContext::new(&theme, &commands));
        }
        surface
    }

    /// Le fond de la première cellule d'une ligne dessinée.
    ///
    /// `None` quand la ligne est hors de la surface : l'essai préfère l'absence à une
    /// couleur inventée, qui pourrait coïncider avec celle qu'il attend.
    fn background_of(list: &AirListBox, size: AirSize, row: u16) -> Option<AirColor> {
        let surface = painted(list, size);
        let point = AirPoint::new(AirColumn::FIRST, AirRow::new(row));
        surface.cell(point).map(|cell| cell.background)
    }

    #[test]
    fn une_liste_neuve_est_vide_et_sans_bordure() {
        let list = AirListBox::new(AirViewState::new(
            AirViewId::FIRST,
            AirRectangle::from_parts(0, 0, 20, 5),
        ));
        assert!(!list.state().has_border());
        assert!(list.is_empty());
        assert_eq!(list.item_count(), 0);
        assert_eq!(list.focused(), None);
    }

    #[test]
    fn le_curseur_descend_et_remonte_sans_sortir_de_la_liste() {
        let mut list = list(5, 3);
        press(&mut list, AirKey::Up);
        assert_eq!(
            list.focused_item(),
            0,
            "le curseur ne passe pas avant le premier"
        );
        press(&mut list, AirKey::Down);
        press(&mut list, AirKey::Down);
        press(&mut list, AirKey::Down);
        press(&mut list, AirKey::Down);
        assert_eq!(
            list.focused_item(),
            2,
            "le curseur ne passe pas après le dernier"
        );
    }

    #[test]
    fn le_curseur_ramene_la_fenetre_sur_lui() {
        let mut list = list(5, 20);
        press(&mut list, AirKey::End);
        assert_eq!(list.focused_item(), 19);
        // Cinq lignes visibles, vingt items : le dernier est visible quand la fenêtre
        // commence au quinzième.
        assert_eq!(list.top_item(), 15);
        press(&mut list, AirKey::Home);
        assert_eq!(list.top_item(), 0);
    }

    #[test]
    fn une_page_vaut_la_hauteur_visible() {
        let mut list = list(5, 20);
        press(&mut list, AirKey::PageDown);
        assert_eq!(list.focused_item(), 5);
        press(&mut list, AirKey::PageUp);
        assert_eq!(list.focused_item(), 0);
    }

    #[test]
    fn le_defilement_ne_laisse_pas_de_vide_sous_le_dernier_item() {
        let mut list = list(5, 8);
        list.scroll_to(100);
        assert_eq!(
            list.top_item(),
            3,
            "huit items, cinq lignes : trois au plus"
        );
    }

    #[test]
    fn un_contenu_plus_court_que_la_fenetre_ne_defile_pas() {
        let mut list = list(10, 3);
        list.scroll_to(2);
        assert_eq!(list.top_item(), 0);
    }

    #[test]
    fn entree_demande_l_ouverture_de_l_item_pointe() {
        let mut list = list(5, 3);
        press(&mut list, AirKey::Down);
        press(&mut list, AirKey::Enter);
        let requests: Vec<AirListRequest> = list.drain_requests().collect();
        assert_eq!(requests, vec![AirListRequest::Activate(1)]);
        assert!(!list.has_pending_requests(), "le relevé vide la file");
    }

    #[test]
    fn entree_sur_une_liste_vide_ne_demande_rien() {
        // L'indice zéro ne désignerait aucun item : une demande serait une invitation à
        // indexer dans le vide, et c'est le contrôleur qui paierait.
        let mut list = list(5, 0);
        assert_eq!(press(&mut list, AirKey::Enter), AirOutcome::Ignored);
        assert!(!list.has_pending_requests());
    }

    #[test]
    fn inser_demande_le_marquage_puis_descend() {
        let mut list = list(5, 3);
        press(&mut list, AirKey::Insert);
        press(&mut list, AirKey::Insert);
        let requests: Vec<AirListRequest> = list.drain_requests().collect();
        assert_eq!(
            requests,
            vec![AirListRequest::ToggleMark(0), AirListRequest::ToggleMark(1)],
            "deux appuis marquent deux items successifs, comme dans mc"
        );
        assert_eq!(list.focused_item(), 2);
    }

    #[test]
    fn la_vue_ne_marque_rien_elle_meme() {
        // Le marquage est un fait du modèle : la vue le demande et l'affiche, elle ne le
        // décide pas. Sans cette règle, deux vues du même contenu divergeraient.
        let mut list = list(5, 3);
        press(&mut list, AirKey::Insert);
        assert!(
            list.items().iter().all(|item| !item.marked),
            "la liste s'est marquée elle-même"
        );
    }

    #[test]
    fn un_relachement_de_touche_ne_descend_pas_le_curseur() {
        // La règle vit dans la distribution (jalon M7c) et non dans ce widget : l'essai
        // passe donc par `dispatch_event`, le chemin que prend une vraie frappe. Piloter
        // `handle_event` directement éprouverait un filtre que la liste ne porte plus.
        let mut list = list(5, 5);
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent {
            logical: AirKey::Down,
            modifiers: AirModifiers::default(),
            state: AirKeyState::Release,
            physical: None,
            base_layout: None,
        }));
        let outcome = crate::view::dispatch_event(&mut list, &mut event, &mut context);
        assert_eq!(outcome, AirOutcome::Ignored);
        assert_eq!(list.focused_item(), 0);
    }

    #[test]
    fn une_repetition_de_touche_descend_le_curseur() {
        // Le pendant du précédent, et il compte autant : maintenir la flèche doit
        // répéter le déplacement. Un filtre qui écarterait « tout ce qui n'est pas un
        // enfoncement » passerait l'essai du relâchement et casserait celui-ci.
        let mut list = list(5, 5);
        let outcome = send_key(
            &mut list,
            AirKey::Down,
            AirKeyState::Repeat,
            AirModifiers::default(),
        );
        assert_eq!(outcome, AirOutcome::Consumed);
        assert_eq!(list.focused_item(), 1);
    }

    #[test]
    fn une_touche_modifiee_n_est_pas_prise() {
        // `Ctrl`+`Début` n'est pas `Début` : la liste laisse passer ce qu'elle ne traite
        // pas, pour qu'un parent puisse s'en servir.
        let mut list = list(5, 20);
        press(&mut list, AirKey::End);
        let modifiers = AirModifiers {
            control: true,
            ..AirModifiers::default()
        };
        let outcome = send_key(&mut list, AirKey::Home, AirKeyState::Press, modifiers);
        assert_eq!(outcome, AirOutcome::Ignored);
        assert_eq!(list.focused_item(), 19);
    }

    #[test]
    fn un_clic_pose_le_curseur_sur_la_ligne_cliquee() {
        let mut list = list(5, 20);
        list.scroll_to(10);
        let outcome = send_mouse(
            &mut list,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            3,
            2,
        );
        assert_eq!(outcome, AirOutcome::Consumed);
        assert_eq!(list.focused_item(), 12, "troisième ligne de la fenêtre");
    }

    #[test]
    fn un_clic_sous_le_dernier_item_ne_deplace_pas_le_curseur() {
        let mut list = list(10, 3);
        send_mouse(
            &mut list,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            3,
            7,
        );
        assert_eq!(list.focused_item(), 0);
    }

    #[test]
    fn la_molette_defile_sans_deplacer_le_curseur() {
        let mut list = list(5, 40);
        send_mouse(&mut list, AirMouseEventKind::WheelDown, 0, 0);
        assert_eq!(list.top_item(), 3);
        assert_eq!(
            list.focused_item(),
            0,
            "la molette ne touche pas au curseur"
        );
        send_mouse(&mut list, AirMouseEventKind::WheelUp, 0, 0);
        assert_eq!(list.top_item(), 0);
    }

    #[test]
    fn un_contenu_plus_court_ramene_le_curseur_dedans() {
        let mut list = list(5, 20);
        press(&mut list, AirKey::End);
        assert_eq!(list.focused_item(), 19);
        list.set_items(vec![AirListItem::new("seul")]);
        assert_eq!(list.focused_item(), 0);
        assert_eq!(list.top_item(), 0);
    }

    #[test]
    fn les_items_sont_dessines_depuis_la_premiere_ligne_visible() {
        let mut list = list(3, 10);
        list.scroll_to(4);
        let lines = render(&list, AirSize::new(20, 3));
        assert_eq!(
            lines.first().map(String::as_str),
            Some("item 4              ")
        );
        assert_eq!(
            lines.get(2).map(String::as_str),
            Some("item 6              ")
        );
    }

    #[test]
    fn sous_le_dernier_item_la_liste_n_ecrit_rien() {
        let list = list(4, 2);
        let lines = render(&list, AirSize::new(20, 4));
        assert_eq!(
            lines.get(2).map(String::as_str),
            Some("                    ")
        );
        assert_eq!(
            lines.get(3).map(String::as_str),
            Some("                    ")
        );
    }

    #[test]
    fn la_precision_est_ecrite_contre_le_bord_droit() {
        let mut list = list(2, 0);
        list.set_items(vec![
            AirListItem::new("fichier").with_detail("1024"),
            AirListItem::new("autre").with_detail("7"),
        ]);
        let lines = render(&list, AirSize::new(14, 2));
        assert_eq!(lines.first().map(String::as_str), Some("fichier   1024"));
        assert_eq!(lines.get(1).map(String::as_str), Some("autre        7"));
    }

    #[test]
    fn c_est_le_libelle_qui_cede_la_place_a_la_precision() {
        // Une taille coupée en deux ne veut plus rien dire ; un nom tronqué se lit encore.
        let mut list = list(1, 0);
        list.set_items(vec![
            AirListItem::new("un-nom-de-fichier-tres-long").with_detail("4096"),
        ]);
        let lines = render(&list, AirSize::new(14, 1));
        assert_eq!(lines.first().map(String::as_str), Some("un-nom-de 4096"));
    }

    #[test]
    fn une_grappe_large_n_est_pas_coupee_en_deux() {
        // Le nom tient en six colonnes ; le septième caractère est large et ne rentre
        // pas, donc il n'est pas écrit du tout (ADR-006).
        assert_eq!(fitted("abcdef漢", 6), "abcdef");
        assert_eq!(fitted("abcdef漢", 7), "abcdef");
        assert_eq!(fitted("abcdef漢", 8), "abcdef漢");
        assert_eq!(width_of("漢字"), 4);
    }

    #[test]
    fn l_item_pointe_porte_le_role_de_la_selection() {
        let mut list = list(3, 5);
        list.state_mut().set_focused(true);
        press(&mut list, AirKey::Down);
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(20, 3);
        assert_eq!(
            background_of(&list, size, 1),
            Some(theme.palette.color_pair(AirRole::ListSelected).background)
        );
        assert_eq!(
            background_of(&list, size, 0),
            Some(theme.palette.color_pair(AirRole::ListNormal).background)
        );
    }

    #[test]
    fn une_liste_sans_focus_ne_montre_pas_de_curseur() {
        // C'est ce qui permet de savoir lequel des deux panneaux reçoit les touches.
        let mut list = list(3, 5);
        press(&mut list, AirKey::Down);
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(20, 3);
        assert_eq!(
            background_of(&list, size, 1),
            Some(theme.palette.color_pair(AirRole::ListNormal).background)
        );
    }

    #[test]
    fn un_item_marque_porte_son_role_et_le_cumule_avec_le_curseur() {
        let mut list = list(3, 0);
        list.state_mut().set_focused(true);
        list.set_items(vec![
            AirListItem::new("premier").marked(),
            AirListItem::new("second").marked(),
        ]);
        let theme = AirTheme::turbo_vision();
        let size = AirSize::new(20, 3);
        assert_eq!(
            background_of(&list, size, 0),
            Some(
                theme
                    .palette
                    .color_pair(AirRole::ListMarkedSelected)
                    .background
            )
        );
        assert_eq!(
            background_of(&list, size, 1),
            Some(theme.palette.color_pair(AirRole::ListMarked).background)
        );
    }

    #[test]
    fn un_redimensionnement_garde_le_curseur_visible() {
        let mut list = list(10, 20);
        press(&mut list, AirKey::End);
        assert_eq!(list.top_item(), 10);
        list.resize(AirSize::new(20, 4));
        assert_eq!(list.visible_rows(), 4);
        assert_eq!(list.top_item(), 16, "le curseur est resté dans la fenêtre");
        assert_eq!(list.focused_item(), 19);
    }

    #[test]
    fn une_liste_desactivee_ne_prend_rien() {
        let mut list = list(5, 5);
        list.state_mut().set_enabled(false);
        assert_eq!(press(&mut list, AirKey::Down), AirOutcome::Ignored);
        assert_eq!(list.focused_item(), 0);
    }

    #[test]
    fn la_liste_renseigne_l_ascenseur_qu_elle_ne_connait_pas() {
        use crate::widgets::{AirOrientation, AirScrollBar};
        let mut list = list(5, 40);
        let mut bar = AirScrollBar::new(
            AirViewState::new(AirViewId::FIRST, AirRectangle::from_parts(0, 0, 1, 5)),
            AirOrientation::Vertical,
        );
        press(&mut list, AirKey::PageDown);
        press(&mut list, AirKey::PageDown);
        assert_eq!(list.focused_item(), 10);
        assert_eq!(
            list.top_item(),
            6,
            "le curseur est sur la dernière ligne visible"
        );
        bar.set_range(list.top_item(), list.visible_rows(), list.item_count());
        assert_eq!(bar.position(), 6);
        assert_eq!(bar.total(), 40);
        assert_eq!(bar.visible(), 5);
        // Et en retour : l'ascenseur demande, la liste applique. Les deux lignes que la
        // documentation du module donne au contrôleur.
        send_mouse_to_bar(&mut bar);
        if let Some(position) = bar.take_request() {
            list.scroll_to(position);
        }
        assert_eq!(list.top_item(), 5, "la flèche haute a reculé d'une ligne");
        assert_eq!(
            list.focused_item(),
            10,
            "l'ascenseur déplace la fenêtre, pas le curseur"
        );
    }

    /// Un clic sur la flèche haute de l'ascenseur.
    fn send_mouse_to_bar(bar: &mut crate::widgets::AirScrollBar) {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::FIRST, AirRow::FIRST),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::default(),
        }));
        bar.handle_event(&mut event, &mut context);
    }

    #[test]
    fn un_double_clic_ouvre_l_item() {
        // Le geste de Midnight Commander et de tout gestionnaire de fichiers.
        let mut list = list(5, 10);
        send_mouse(
            &mut list,
            AirMouseEventKind::Pressed(AirMouseButton::Left),
            3,
            2,
        );
        assert_eq!(list.focused_item(), 2);
        assert!(!list.has_pending_requests(), "un simple clic n'ouvre pas");

        send_mouse(
            &mut list,
            AirMouseEventKind::DoubleClicked(AirMouseButton::Left),
            3,
            2,
        );
        let requests: Vec<AirListRequest> = list.drain_requests().collect();
        assert_eq!(requests, vec![AirListRequest::Activate(2)]);
    }

    #[test]
    fn un_double_clic_dans_le_vide_n_ouvre_rien() {
        let mut list = list(5, 0);
        send_mouse(
            &mut list,
            AirMouseEventKind::DoubleClicked(AirMouseButton::Left),
            1,
            0,
        );
        assert!(!list.has_pending_requests());
    }
}
