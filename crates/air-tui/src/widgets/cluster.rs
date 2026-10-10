// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les cases à cocher et les boutons radio.
//!
//! # Un seul type pour les deux, comme Turbo Vision
//!
//! `TCheckBoxes` et `TRadioButtons` dérivent tous deux de `TCluster` et ne diffèrent que
//! par trois choses : la **marque** qu'ils dessinent, ce qu'un **appui** fait, et si un
//! déplacement du curseur emporte la sélection. Le reste — la disposition en lignes, la
//! navigation aux flèches, les lettres d'accès rapide, le clic — est identique.
//!
//! Sans héritage, deux types séparés auraient dupliqué ce reste, et une règle écrite deux
//! fois finit par ne plus l'être qu'une : c'est la leçon des jalons M7c et M7d. D'où un
//! seul [`AirCluster`], un [`AirClusterKind`] qui porte la différence, et deux
//! constructeurs nommés — [`AirCluster::check_boxes`] et [`AirCluster::radio_buttons`] —
//! pour que le site d'appel dise lequel il construit.
//!
//! # Ce que les deux font différemment
//!
//! | | case à cocher | bouton radio |
//! |---|---|---|
//! | marque | `[X]` | `(•)` |
//! | plusieurs cochés | oui | **non**, un seul |
//! | flèche haut/bas | déplace le curseur | déplace **et** sélectionne |
//! | `Espace` | bascule l'item | sélectionne l'item |
//!
//! Le déplacement qui sélectionne vient de `TRadioButtons::movedTo`, qui affecte `value` :
//! un groupe à choix unique où le curseur se promènerait sans rien choisir obligerait à
//! appuyer après chaque flèche, pour un geste qui n'a qu'une issue possible.
//!
//! # La bordure
//!
//! Retirée dans le constructeur, pour la raison des autres petits contrôles (ADR-010) : un
//! groupe encadré n'a plus la place de ses libellés. Un groupe encadré s'obtient en le
//! plaçant dans un [`crate::widgets::AirPanel`], qui porte un titre — c'est ainsi que les
//! dialogues de Turbo Vision présentent leurs options.

use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
use crate::theme::{AirClusterGlyphs, AirRole};
use crate::view::painter::AirPainter;
use crate::view::{AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewState};
use crate::widgets::hotkey::split_hotkey;

/// Ce qu'un groupe d'items cochables est.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirClusterKind {
    /// Des cases à cocher : chacune indépendante, plusieurs cochées à la fois.
    CheckBoxes,
    /// Des boutons radio : un seul coché, et le déplacement du curseur choisit.
    RadioButtons,
}

/// Un item d'un groupe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirClusterItem {
    /// Le libellé, avec sa lettre d'accès rapide entre `~` s'il en a une.
    pub label: String,
    /// Vrai si l'item est coché.
    pub checked: bool,
}

impl AirClusterItem {
    /// Un item décoché.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            checked: false,
        }
    }

    /// Le même item, coché.
    #[must_use]
    pub const fn checked(mut self) -> Self {
        self.checked = true;
        self
    }
}

/// Un groupe de cases à cocher ou de boutons radio.
#[derive(Debug)]
pub struct AirCluster {
    /// L'état commun.
    state: AirViewState,
    /// Ce que ce groupe est.
    kind: AirClusterKind,
    /// Les items, un par ligne.
    items: Vec<AirClusterItem>,
    /// L'item sous le curseur.
    focused_item: usize,
}

impl AirCluster {
    /// Un groupe de cases à cocher, sans bordure et sans item.
    #[must_use]
    pub fn check_boxes(state: AirViewState) -> Self {
        Self::new(state, AirClusterKind::CheckBoxes)
    }

    /// Un groupe de boutons radio, sans bordure et sans item.
    ///
    /// Aucun item n'est coché au départ. Un groupe radio sans choix est un état légitime —
    /// « rien n'est encore décidé » —, et en cocher un d'office déciderait à la place de
    /// l'utilisateur. L'appelant qui veut un défaut le pose avec
    /// [`AirCluster::with_item`] et [`AirClusterItem::checked`].
    #[must_use]
    pub fn radio_buttons(state: AirViewState) -> Self {
        Self::new(state, AirClusterKind::RadioButtons)
    }

    /// Le constructeur commun.
    fn new(mut state: AirViewState, kind: AirClusterKind) -> Self {
        state.set_border(false);
        Self {
            state,
            kind,
            items: Vec::new(),
            focused_item: 0,
        }
    }

    /// Le même groupe, avec un item de plus.
    #[must_use]
    pub fn with_item(mut self, item: AirClusterItem) -> Self {
        self.push(item);
        self
    }

    /// Ajoute un item.
    pub fn push(&mut self, item: AirClusterItem) {
        let checked = item.checked;
        self.items.push(item);
        // Un item coché ajouté à un groupe radio décoche les autres : l'invariant « un
        // seul coché » vaut aussi à la construction, sinon un groupe mal monté afficherait
        // deux points et le premier appui en effacerait un sans qu'on sache lequel.
        if checked {
            let last = self.items.len().saturating_sub(1);
            self.enforce_single_choice(last);
        }
    }

    /// Ce que ce groupe est.
    #[must_use]
    pub const fn kind(&self) -> AirClusterKind {
        self.kind
    }

    /// Les items.
    #[must_use]
    pub fn items(&self) -> &[AirClusterItem] {
        &self.items
    }

    /// Le nombre d'items.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Vrai si le groupe n'a aucun item.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// L'item sous le curseur.
    #[must_use]
    pub const fn focused_item(&self) -> usize {
        self.focused_item
    }

    /// Vrai si l'item d'indice `index` est coché. Faux hors bornes.
    #[must_use]
    pub fn is_checked(&self, index: usize) -> bool {
        self.items.get(index).is_some_and(|item| item.checked)
    }

    /// L'unique item coché d'un groupe radio, s'il y en a un.
    ///
    /// Rend toujours `None` pour un groupe de cases à cocher : la question n'a pas de sens
    /// là où plusieurs réponses sont possibles, et rendre « le premier coché » inviterait à
    /// employer un groupe de cases comme un groupe radio.
    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        if self.kind != AirClusterKind::RadioButtons {
            return None;
        }
        self.items.iter().position(|item| item.checked)
    }

    /// Les indices des items cochés, dans l'ordre.
    #[must_use]
    pub fn checked_items(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| item.checked.then_some(index))
            .collect()
    }

    /// Coche ou décoche un item, en tenant l'invariant d'un groupe radio.
    ///
    /// Décocher le choix d'un groupe radio est accepté : c'est le retour à « rien n'est
    /// décidé », et le refuser obligerait l'appelant à reconstruire le groupe pour y
    /// revenir.
    pub fn set_checked(&mut self, index: usize, checked: bool) {
        let Some(item) = self.items.get_mut(index) else {
            return;
        };
        item.checked = checked;
        if checked {
            self.enforce_single_choice(index);
        }
    }

    /// Place le curseur sur un item, borné au dernier.
    ///
    /// Dans un groupe radio, cela **choisit** aussi : c'est `TRadioButtons::movedTo`.
    pub fn focus_item(&mut self, index: usize) {
        if self.items.is_empty() {
            self.focused_item = 0;
            return;
        }
        self.focused_item = index.min(self.items.len().saturating_sub(1));
        if self.kind == AirClusterKind::RadioButtons {
            self.set_checked(self.focused_item, true);
        }
    }

    /// Bascule l'item sous le curseur — ou le choisit, dans un groupe radio.
    pub fn press_focused(&mut self) {
        let index = self.focused_item;
        match self.kind {
            AirClusterKind::CheckBoxes => {
                let checked = self.is_checked(index);
                self.set_checked(index, !checked);
            }
            AirClusterKind::RadioButtons => self.set_checked(index, true),
        }
    }

    /// Dans un groupe radio, décoche tout sauf `kept`.
    fn enforce_single_choice(&mut self, kept: usize) {
        if self.kind != AirClusterKind::RadioButtons {
            return;
        }
        for (index, item) in self.items.iter_mut().enumerate() {
            if index != kept {
                item.checked = false;
            }
        }
    }

    /// Les glyphes de ce genre de groupe, dans un thème.
    const fn glyphs(&self, theme: &crate::theme::AirTheme) -> AirClusterGlyphs {
        match self.kind {
            AirClusterKind::CheckBoxes => theme.glyphs.check_box,
            AirClusterKind::RadioButtons => theme.glyphs.radio_button,
        }
    }

    /// L'item dont le libellé porte cette lettre d'accès rapide.
    fn item_for_hotkey(&self, character: char) -> Option<usize> {
        let wanted = character.to_ascii_lowercase();
        self.items
            .iter()
            .position(|item| split_hotkey(&item.label).hotkey_character() == Some(wanted))
    }

    /// Traite une frappe, et dit si elle a servi.
    fn handle_key(&mut self, key: AirKey) -> bool {
        match key {
            AirKey::Up => {
                self.focus_item(self.focused_item.saturating_sub(1));
                true
            }
            AirKey::Down => {
                self.focus_item(self.focused_item.saturating_add(1));
                true
            }
            AirKey::Home => {
                self.focus_item(0);
                true
            }
            AirKey::End => {
                self.focus_item(self.items.len().saturating_sub(1));
                true
            }
            AirKey::Character(' ') => {
                self.press_focused();
                true
            }
            AirKey::Character(character) => {
                // La lettre d'accès rapide place le curseur **et** appuie : c'est un
                // raccourci, et il serait inutile s'il fallait encore une frappe.
                let Some(index) = self.item_for_hotkey(character) else {
                    return false;
                };
                self.focus_item(index);
                self.press_focused();
                true
            }
            _ => false,
        }
    }
}

impl AirView for AirCluster {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let palette = &context.theme.palette;
        let normal = palette.color_pair(AirRole::ClusterNormal);
        let selected = palette.color_pair(AirRole::ClusterSelected);
        let hotkey = palette.color_pair(AirRole::ClusterHotkey);
        let glyphs = self.glyphs(context.theme);
        let size = painter.size();
        painter.fill_rectangle(AirRectangle::new(AirPoint::ORIGIN, size), normal.cell(' '));
        let focused_view = self.state.is_focused();
        for (index, item) in self.items.iter().enumerate() {
            let Ok(row) = u16::try_from(index) else {
                break;
            };
            if row >= size.rows {
                break;
            }
            // Le curseur ne se montre que dans un groupe qui a le focus : deux groupes
            // côte à côte qui marqueraient chacun un item laisseraient croire à deux
            // curseurs.
            let colors = if focused_view && index == self.focused_item {
                selected
            } else {
                normal
            };
            let mark = if item.checked { glyphs.mark } else { ' ' };
            let box_text = format!("{}{}{} ", glyphs.left, mark, glyphs.right);
            let row = AirRow::new(row);
            let mut column = painter.draw_text_with_colors(
                AirPoint::new(AirColumn::FIRST, row),
                &box_text,
                colors,
            );
            let label = split_hotkey(&item.label);
            for (text, pair) in [
                (label.before, colors),
                (label.hotkey.unwrap_or(""), hotkey),
                (label.after, colors),
            ] {
                if text.is_empty() {
                    continue;
                }
                column = painter.draw_text_with_colors(AirPoint::new(column, row), text, pair);
            }
        }
    }

    fn cursor(&self) -> Option<AirPoint> {
        if !self.state.is_focused() || self.items.is_empty() {
            return None;
        }
        // Le curseur du terminal se pose **dans** la case, à la place de la marque : c'est
        // ce que fait Turbo Vision, et c'est ce qui montre où un `Espace` agira.
        let row = u16::try_from(self.focused_item).ok()?;
        Some(AirPoint::new(AirColumn::new(1), AirRow::new(row)))
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        let _ = context;
        if !self.state.is_enabled() || self.items.is_empty() {
            return AirOutcome::Ignored;
        }
        let Some(input) = event.input() else {
            return AirOutcome::Ignored;
        };
        let served = match input {
            AirInputEvent::Key(key) => {
                // Un modificateur change le sens d'une touche, et `Alt`+lettre appartient
                // aux raccourcis : la barre de menu en décide.
                if key.modifiers.control || key.modifiers.alternate {
                    return AirOutcome::Ignored;
                }
                self.handle_key(key.logical)
            }
            AirInputEvent::Mouse(mouse) => match mouse.kind {
                AirMouseEventKind::Pressed(AirMouseButton::Left) => {
                    let index = usize::from(mouse.position.row.value());
                    if index >= self.items.len() {
                        // Un clic sous le dernier item est consommé — il est bien pour ce
                        // groupe — mais il ne désigne rien.
                        true
                    } else {
                        self.focus_item(index);
                        self.press_focused();
                        true
                    }
                }
                _ => false,
            },
            _ => return AirOutcome::Ignored,
        };
        if !served {
            return AirOutcome::Ignored;
        }
        event.consume();
        AirOutcome::Consumed
    }
}

#[cfg(test)]
mod tests {
    use super::{AirCluster, AirClusterItem, AirClusterKind};
    use crate::cell::AirColor;
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirModifiers, AirMouseButton, AirMouseEvent,
        AirMouseEventKind,
    };
    use crate::surface::AirSurface;
    use crate::theme::{AirRole, AirTheme};
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId, AirViewState,
        draw_view,
    };

    fn state(rows: u16) -> AirViewState {
        AirViewState::new(AirViewId::FIRST, AirRectangle::from_parts(0, 0, 20, rows))
    }

    /// Trois cases à cocher, dont la deuxième porte une lettre d'accès rapide.
    fn check_boxes() -> AirCluster {
        let mut cluster = AirCluster::check_boxes(state(3))
            .with_item(AirClusterItem::new("Premier"))
            .with_item(AirClusterItem::new("~D~euxième"))
            .with_item(AirClusterItem::new("Troisième"));
        cluster.state_mut().set_focused(true);
        cluster
    }

    /// Trois boutons radio, aucun choisi.
    fn radio_buttons() -> AirCluster {
        let mut cluster = AirCluster::radio_buttons(state(3))
            .with_item(AirClusterItem::new("Un"))
            .with_item(AirClusterItem::new("~D~eux"))
            .with_item(AirClusterItem::new("Trois"));
        cluster.state_mut().set_focused(true);
        cluster
    }

    fn press(cluster: &mut AirCluster, key: AirKey) -> AirOutcome {
        press_with(cluster, key, AirModifiers::default())
    }

    fn press_with(cluster: &mut AirCluster, key: AirKey, modifiers: AirModifiers) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Key(AirKeyEvent {
            modifiers,
            ..AirKeyEvent::press(key)
        }));
        cluster.handle_event(&mut event, &mut context)
    }

    fn click(cluster: &mut AirCluster, row: u16) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::new(1), AirRow::new(row)),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::default(),
        }));
        cluster.handle_event(&mut event, &mut context)
    }

    /// Dessine le groupe et rend ses lignes.
    fn render(cluster: &AirCluster, size: AirSize) -> Vec<String> {
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        let mut surface = AirSurface::new(size);
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                cluster,
                &mut painter,
                &AirDrawContext::new(&theme, &commands),
            );
        }
        (0..size.rows)
            .map(|row| {
                (0..size.columns)
                    .map(|column| {
                        surface
                            .cell(AirPoint::new(AirColumn::new(column), AirRow::new(row)))
                            .map_or(' ', |cell| cell.character)
                    })
                    .collect()
            })
            .collect()
    }

    /// Le fond de la première cellule de chaque ligne.
    fn row_backgrounds(cluster: &AirCluster, size: AirSize) -> Vec<Option<AirColor>> {
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        let mut surface = AirSurface::new(size);
        {
            let mut painter = AirPainter::new(&mut surface);
            draw_view(
                cluster,
                &mut painter,
                &AirDrawContext::new(&theme, &commands),
            );
        }
        (0..size.rows)
            .map(|row| {
                surface
                    .cell(AirPoint::new(AirColumn::FIRST, AirRow::new(row)))
                    .map(|cell| cell.background)
            })
            .collect()
    }

    // ------------------------------------------------------- la construction

    #[test]
    fn un_groupe_neuf_est_sans_bordure() {
        // ADR-010 pose la bordure par défaut ; un groupe encadré n'a plus la place de ses
        // libellés.
        assert!(!AirCluster::check_boxes(state(3)).state().has_border());
        assert!(!AirCluster::radio_buttons(state(3)).state().has_border());
    }

    #[test]
    fn un_groupe_neuf_n_a_rien_de_coche() {
        let cluster = check_boxes();
        assert_eq!(cluster.checked_items(), [] as [usize; 0]);
        assert_eq!(cluster.item_count(), 3);
        assert!(!cluster.is_empty());
    }

    #[test]
    fn un_groupe_radio_neuf_n_a_aucun_choix() {
        // « Rien n'est encore décidé » est un état légitime : en cocher un d'office
        // déciderait à la place de l'utilisateur.
        assert_eq!(radio_buttons().selected(), None);
    }

    #[test]
    fn un_groupe_de_cases_n_a_pas_de_choix_unique() {
        // La question n'a pas de sens là où plusieurs réponses sont possibles.
        let mut cluster = check_boxes();
        cluster.set_checked(1, true);
        assert_eq!(cluster.selected(), None);
        assert_eq!(cluster.kind(), AirClusterKind::CheckBoxes);
    }

    // ----------------------------------------------------------- le cochage

    #[test]
    fn plusieurs_cases_se_cochent_ensemble() {
        let mut cluster = check_boxes();
        cluster.set_checked(0, true);
        cluster.set_checked(2, true);
        assert_eq!(cluster.checked_items(), vec![0, 2]);
    }

    #[test]
    fn un_seul_bouton_radio_reste_coche() {
        let mut cluster = radio_buttons();
        cluster.set_checked(0, true);
        cluster.set_checked(2, true);
        assert_eq!(cluster.checked_items(), vec![2]);
        assert_eq!(cluster.selected(), Some(2));
    }

    #[test]
    fn un_item_coche_a_la_construction_decoche_les_autres_du_groupe_radio() {
        // L'invariant vaut aussi à la construction : un groupe mal monté afficherait deux
        // points, et le premier appui en effacerait un sans qu'on sache lequel.
        let cluster = AirCluster::radio_buttons(state(3))
            .with_item(AirClusterItem::new("Un").checked())
            .with_item(AirClusterItem::new("Deux").checked());
        assert_eq!(cluster.checked_items(), vec![1]);
    }

    #[test]
    fn decocher_le_choix_d_un_groupe_radio_est_accepte() {
        // Le retour à « rien n'est décidé ». Le refuser obligerait à reconstruire le
        // groupe pour y revenir.
        let mut cluster = radio_buttons();
        cluster.set_checked(1, true);
        cluster.set_checked(1, false);
        assert_eq!(cluster.selected(), None);
    }

    #[test]
    fn cocher_hors_bornes_ne_fait_rien() {
        let mut cluster = check_boxes();
        cluster.set_checked(99, true);
        assert_eq!(cluster.checked_items(), [] as [usize; 0]);
        assert!(!cluster.is_checked(99));
    }

    // ---------------------------------------------------------- le clavier

    #[test]
    fn les_fleches_deplacent_le_curseur_sans_cocher_une_case() {
        let mut cluster = check_boxes();
        assert_eq!(press(&mut cluster, AirKey::Down), AirOutcome::Consumed);
        assert_eq!(cluster.focused_item(), 1);
        assert_eq!(cluster.checked_items(), [] as [usize; 0]);
    }

    #[test]
    fn les_fleches_choisissent_dans_un_groupe_radio() {
        // `TRadioButtons::movedTo` affecte `value` : un curseur qui se promènerait sans
        // rien choisir obligerait à appuyer après chaque flèche, pour un geste qui n'a
        // qu'une issue.
        let mut cluster = radio_buttons();
        press(&mut cluster, AirKey::Down);
        assert_eq!(cluster.selected(), Some(1));
        press(&mut cluster, AirKey::Down);
        assert_eq!(cluster.selected(), Some(2));
    }

    #[test]
    fn l_espace_bascule_une_case() {
        let mut cluster = check_boxes();
        press(&mut cluster, AirKey::Character(' '));
        assert!(cluster.is_checked(0));
        press(&mut cluster, AirKey::Character(' '));
        assert!(!cluster.is_checked(0));
    }

    #[test]
    fn l_espace_ne_decoche_pas_un_bouton_radio() {
        // Appuyer deux fois sur le même choix le laisse choisi : le décocher ferait d'un
        // geste répété un abandon, ce qu'aucun groupe radio ne fait.
        let mut cluster = radio_buttons();
        press(&mut cluster, AirKey::Character(' '));
        press(&mut cluster, AirKey::Character(' '));
        assert_eq!(cluster.selected(), Some(0));
    }

    #[test]
    fn le_curseur_est_borne_aux_items() {
        let mut cluster = check_boxes();
        press(&mut cluster, AirKey::Up);
        assert_eq!(cluster.focused_item(), 0);
        press(&mut cluster, AirKey::End);
        assert_eq!(cluster.focused_item(), 2);
        press(&mut cluster, AirKey::Down);
        assert_eq!(cluster.focused_item(), 2);
        press(&mut cluster, AirKey::Home);
        assert_eq!(cluster.focused_item(), 0);
    }

    #[test]
    fn la_lettre_d_acces_rapide_place_le_curseur_et_appuie() {
        // Un raccourci qui demanderait encore une frappe ne servirait à rien.
        let mut cluster = check_boxes();
        press(&mut cluster, AirKey::Character('d'));
        assert_eq!(cluster.focused_item(), 1);
        assert!(cluster.is_checked(1));
    }

    #[test]
    fn la_lettre_d_acces_rapide_ignore_la_casse() {
        let mut cluster = check_boxes();
        press(&mut cluster, AirKey::Character('D'));
        assert!(cluster.is_checked(1));
    }

    #[test]
    fn une_lettre_sans_item_n_est_pas_prise() {
        // Elle doit pouvoir servir ailleurs — à un autre groupe, ou à un bouton.
        let mut cluster = check_boxes();
        assert_eq!(
            press(&mut cluster, AirKey::Character('z')),
            AirOutcome::Ignored
        );
    }

    #[test]
    fn une_frappe_modifiee_reste_aux_raccourcis() {
        let mut cluster = check_boxes();
        let modifiers = AirModifiers {
            alternate: true,
            ..AirModifiers::default()
        };
        assert_eq!(
            press_with(&mut cluster, AirKey::Character('d'), modifiers),
            AirOutcome::Ignored
        );
        assert!(!cluster.is_checked(1));
    }

    #[test]
    fn un_groupe_vide_ne_prend_rien() {
        let mut cluster = AirCluster::check_boxes(state(3));
        assert_eq!(
            press(&mut cluster, AirKey::Character(' ')),
            AirOutcome::Ignored
        );
        assert_eq!(cluster.focused_item(), 0);
    }

    #[test]
    fn un_groupe_desactive_ne_prend_rien() {
        let mut cluster = check_boxes();
        cluster.state_mut().set_enabled(false);
        assert_eq!(
            press(&mut cluster, AirKey::Character(' ')),
            AirOutcome::Ignored
        );
    }

    // ------------------------------------------------------------ la souris

    #[test]
    fn un_clic_place_le_curseur_et_appuie() {
        let mut cluster = check_boxes();
        assert_eq!(click(&mut cluster, 2), AirOutcome::Consumed);
        assert_eq!(cluster.focused_item(), 2);
        assert!(cluster.is_checked(2));
    }

    #[test]
    fn un_clic_sous_le_dernier_item_est_consomme_sans_rien_cocher() {
        let mut cluster = check_boxes();
        assert_eq!(click(&mut cluster, 5), AirOutcome::Consumed);
        assert_eq!(cluster.checked_items(), [] as [usize; 0]);
    }

    // ------------------------------------------------------------ le dessin

    #[test]
    fn une_case_cochee_porte_sa_marque() {
        let mut cluster = check_boxes();
        cluster.set_checked(1, true);
        let lines = render(&cluster, AirSize::new(20, 3));
        assert_eq!(
            lines.first().map(String::as_str),
            Some("[ ] Premier         ")
        );
        assert_eq!(
            lines.get(1).map(String::as_str),
            Some("[X] Deuxième        ")
        );
    }

    #[test]
    fn un_bouton_radio_choisi_porte_son_point() {
        let mut cluster = radio_buttons();
        cluster.set_checked(0, true);
        let lines = render(&cluster, AirSize::new(20, 3));
        assert_eq!(
            lines.first().map(String::as_str),
            Some("(•) Un              ")
        );
        assert_eq!(
            lines.get(1).map(String::as_str),
            Some("( ) Deux            ")
        );
    }

    #[test]
    fn le_marqueur_de_lettre_rapide_n_est_pas_dessine() {
        let cluster = check_boxes();
        let lines = render(&cluster, AirSize::new(20, 3));
        assert!(
            lines.get(1).is_some_and(|line| !line.contains('~')),
            "le `~` du libellé est passé à l'écran : {lines:?}"
        );
    }

    #[test]
    fn l_item_sous_le_curseur_porte_le_role_de_la_selection() {
        let theme = AirTheme::turbo_vision();
        let selected = theme
            .palette
            .color_pair(AirRole::ClusterSelected)
            .background;
        let normal = theme.palette.color_pair(AirRole::ClusterNormal).background;
        let mut cluster = check_boxes();
        cluster.focus_item(1);
        assert_eq!(
            row_backgrounds(&cluster, AirSize::new(20, 3)),
            vec![Some(normal), Some(selected), Some(normal)]
        );
    }

    #[test]
    fn un_groupe_sans_focus_ne_montre_aucun_curseur() {
        // Deux groupes côte à côte qui marqueraient chacun un item laisseraient croire à
        // deux curseurs.
        let theme = AirTheme::turbo_vision();
        let normal = theme.palette.color_pair(AirRole::ClusterNormal).background;
        let mut cluster = check_boxes();
        cluster.state_mut().set_focused(false);
        assert!(
            row_backgrounds(&cluster, AirSize::new(20, 3))
                .iter()
                .all(|background| *background == Some(normal))
        );
        assert_eq!(cluster.cursor(), None);
    }

    #[test]
    fn le_curseur_du_terminal_se_pose_dans_la_case() {
        // C'est ce qui montre où un `Espace` agira.
        let mut cluster = check_boxes();
        cluster.focus_item(2);
        assert_eq!(
            cluster.cursor(),
            Some(AirPoint::new(AirColumn::new(1), AirRow::new(2)))
        );
    }

    #[test]
    fn un_groupe_plus_court_que_ses_items_s_arrete_au_bord() {
        let cluster = check_boxes();
        let lines = render(&cluster, AirSize::new(20, 2));
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn le_theme_de_midnight_commander_emploie_la_meme_marque() {
        // Aucun skin de `mc` ne décrit de case à cocher : les glyphes restent ceux du
        // thème par défaut, et ce n'est pas une transcription.
        let theme = AirTheme::midnight_commander();
        assert_eq!(
            theme.glyphs.check_box,
            AirTheme::turbo_vision().glyphs.check_box
        );
        assert_eq!(
            theme.glyphs.radio_button,
            AirTheme::turbo_vision().glyphs.radio_button
        );
    }
}
