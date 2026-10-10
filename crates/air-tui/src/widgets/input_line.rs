// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le champ de saisie d'une ligne.
//!
//! # L'unité d'édition est la grappe de graphème, pas le caractère
//!
//! `é` écrit en forme décomposée est deux caractères (`U+0065 U+0301`) et une seule
//! touche effacée. `👨‍👩‍👧` est quatre caractères, dont deux liants, et une seule touche
//! effacée. Un curseur qui se déplace « d'un caractère » couperait entre une lettre et
//! son accent, et l'effacement laisserait un accent orphelin que l'utilisateur ne peut
//! plus atteindre.
//!
//! Tout se déplace donc par grappes, données par [`crate::unicode::graphemes`] (UAX #29).
//! La position du curseur est un **décalage en octets**, toujours sur une frontière de
//! grappe : un décalage est stable à l'insertion comme à la suppression, là où un indice
//! de grappe demanderait de recompter tout le texte.
//!
//! # Le curseur est celui du terminal
//!
//! Le champ ne dessine pas de curseur. Il répond à [`crate::view::AirView::cursor`], et
//! la traversée de l'arbre ne retient cette réponse que si la vue a le focus. C'est le
//! vrai curseur du terminal qui clignote, à la bonne colonne — et deux champs ne peuvent
//! pas le revendiquer en même temps.
//!
//! # Le défilement
//!
//! Un texte plus large que le champ défile horizontalement, et l'invariant est simple :
//! **le curseur est toujours visible**. Il est rétabli après chaque modification et
//! chaque déplacement, par [`AirInputLine::scroll_into_view`], et jamais au dessin : une
//! vue qui se dessine ne doit pas changer d'état.

use crate::cell::{AirAttributes, AirCell};
use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow};
use crate::input::event::{AirInputEvent, AirKey, AirMouseButton, AirMouseEventKind};
use crate::theme::AirRole;
use crate::unicode::{
    grapheme_width, graphemes, next_grapheme_boundary, previous_grapheme_boundary, width_between,
};
use crate::view::painter::AirPainter;
use crate::view::{AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewState};

/// Un champ de saisie d'une ligne.
#[derive(Debug)]
pub struct AirInputLine {
    /// L'état commun.
    state: AirViewState,
    /// Le texte saisi.
    text: String,
    /// La position du curseur, en octets, toujours sur une frontière de grappe.
    cursor: usize,
    /// Le premier octet affiché — le défilement horizontal.
    first_visible: usize,
    /// Le nombre maximal de grappes acceptées, s'il y a une limite.
    maximum_length: Option<usize>,
}

impl AirInputLine {
    /// Un champ vide, sans bordure.
    ///
    /// La bordure est retirée pour la raison d'[`crate::widgets::AirStaticText`] : un
    /// champ encadré d'une ligne n'a plus de place pour son texte. Un champ encadré
    /// s'obtient en le plaçant dans un [`crate::widgets::AirPanel`].
    #[must_use]
    pub fn new(mut state: AirViewState) -> Self {
        state.set_border(false);
        Self {
            state,
            text: String::new(),
            cursor: 0,
            first_visible: 0,
            maximum_length: None,
        }
    }

    /// Le même champ, avec un texte initial et le curseur à la fin.
    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.set_text(text);
        self
    }

    /// Le même champ, avec une longueur maximale en grappes.
    #[must_use]
    pub fn with_maximum_length(mut self, maximum_length: usize) -> Self {
        self.maximum_length = Some(maximum_length);
        self.truncate_to_maximum();
        self
    }

    /// Le texte saisi.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Remplace le texte et place le curseur à la fin.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.truncate_to_maximum();
        self.cursor = self.text.len();
        self.scroll_into_view();
    }

    /// La position du curseur, en octets dans le texte.
    #[must_use]
    pub const fn cursor_offset(&self) -> usize {
        self.cursor
    }

    /// Le nombre maximal de grappes acceptées, s'il y a une limite.
    #[must_use]
    pub const fn maximum_length(&self) -> Option<usize> {
        self.maximum_length
    }

    /// Le nombre de grappes du texte.
    #[must_use]
    pub fn length(&self) -> usize {
        graphemes(&self.text).count()
    }

    /// Le premier octet affiché — le défilement horizontal.
    #[must_use]
    pub const fn first_visible_offset(&self) -> usize {
        self.first_visible
    }

    /// Coupe le texte à la longueur maximale, sur une frontière de grappe.
    fn truncate_to_maximum(&mut self) {
        let Some(maximum) = self.maximum_length else {
            return;
        };
        let mut offset: usize = 0;
        let mut kept: usize = 0;
        for cluster in graphemes(&self.text) {
            if kept >= maximum {
                break;
            }
            offset = offset.saturating_add(cluster.len());
            kept = kept.saturating_add(1);
        }
        self.text.truncate(offset);
        self.cursor = self.cursor.min(self.text.len());
    }

    /// Le décalage de la frontière de grappe qui précède `offset`.
    ///
    /// Délégué à [`crate::unicode`] : l'éditeur du jalon M6 a besoin du même pas, et deux
    /// implémentations de la même règle garantissent qu'elles divergent.
    fn previous_boundary(&self, offset: usize) -> usize {
        previous_grapheme_boundary(&self.text, offset)
    }

    /// Le décalage de la frontière de grappe qui suit `offset`.
    fn next_boundary(&self, offset: usize) -> usize {
        next_grapheme_boundary(&self.text, offset)
    }

    /// La largeur affichée de la tranche de texte comprise entre deux décalages.
    fn width_between(&self, first: usize, last: usize) -> usize {
        width_between(&self.text, first, last)
    }

    /// La colonne du curseur dans la zone de contenu, défilement compris.
    #[must_use]
    pub fn cursor_column(&self) -> u16 {
        let width = self.width_between(self.first_visible, self.cursor);
        u16::try_from(width).unwrap_or(u16::MAX)
    }

    /// Ramène le curseur dans la zone visible en ajustant le défilement.
    ///
    /// Appelé après chaque modification et chaque déplacement, jamais au dessin : une vue
    /// qui se dessine ne doit pas changer d'état, sinon le rendu dépend de l'ordre des
    /// appels.
    fn scroll_into_view(&mut self) {
        let columns = self.state.bounds().size.columns;
        if columns == 0 {
            self.first_visible = self.cursor;
            return;
        }
        // Le curseur est avant la fenêtre : la fenêtre recule jusqu'à lui.
        if self.cursor < self.first_visible {
            self.first_visible = self.cursor;
            return;
        }
        // Le curseur est après la fenêtre : elle avance d'une grappe à la fois, jusqu'à
        // ce qu'il rentre. Une grappe large peut en coûter deux colonnes, et avancer par
        // colonnes couperait au milieu d'un glyphe.
        //
        // La dernière colonne est réservée au curseur lui-même : sans cela, taper en
        // bout de champ le placerait hors de la vue.
        let usable = usize::from(columns.saturating_sub(1));
        while self.width_between(self.first_visible, self.cursor) > usable
            && self.first_visible < self.cursor
        {
            self.first_visible = self.next_boundary(self.first_visible);
        }
    }

    /// Insère un caractère au curseur. Rend `false` si la longueur maximale l'interdit.
    pub fn insert(&mut self, character: char) -> bool {
        if let Some(maximum) = self.maximum_length
            && self.length() >= maximum
        {
            return false;
        }
        self.text.insert(self.cursor, character);
        // Le curseur avance de la longueur insérée, pas d'une grappe : le caractère
        // inséré peut se joindre à la grappe qui précède — un accent combinant, par
        // exemple — et la frontière suivante serait alors derrière lui.
        self.cursor = self.cursor.saturating_add(character.len_utf8());
        self.scroll_into_view();
        true
    }

    /// Supprime la grappe qui précède le curseur. Rend `false` s'il n'y en a pas.
    pub fn delete_backward(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let start = self.previous_boundary(self.cursor);
        self.text.replace_range(start..self.cursor, "");
        self.cursor = start;
        self.scroll_into_view();
        true
    }

    /// Supprime la grappe qui suit le curseur. Rend `false` s'il n'y en a pas.
    pub fn delete_forward(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        let end = self.next_boundary(self.cursor);
        self.text.replace_range(self.cursor..end, "");
        self.scroll_into_view();
        true
    }

    /// Déplace le curseur d'une grappe vers la gauche. Rend `false` s'il est au début.
    pub fn move_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor = self.previous_boundary(self.cursor);
        self.scroll_into_view();
        true
    }

    /// Déplace le curseur d'une grappe vers la droite. Rend `false` s'il est à la fin.
    pub fn move_right(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        self.cursor = self.next_boundary(self.cursor);
        self.scroll_into_view();
        true
    }

    /// Place le curseur au début du texte.
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
        self.scroll_into_view();
    }

    /// Place le curseur à la fin du texte.
    pub fn move_to_end(&mut self) {
        self.cursor = self.text.len();
        self.scroll_into_view();
    }

    /// Place le curseur sur la grappe affichée à la colonne donnée.
    ///
    /// Une colonne au delà du texte place le curseur à la fin : c'est ce qu'un clic dans
    /// le vide d'un champ doit faire.
    pub fn move_to_column(&mut self, column: u16) {
        let wanted = usize::from(column);
        let mut offset = self.first_visible;
        let mut walked: usize = 0;
        let Some(tail) = self.text.get(self.first_visible..) else {
            return;
        };
        for cluster in graphemes(tail) {
            let width = grapheme_width(cluster);
            // La grappe couvre la colonne visée dès que la colonne tombe avant sa fin :
            // un glyphe large se sélectionne par sa moitié droite comme par sa gauche.
            if wanted < walked.saturating_add(width) {
                break;
            }
            walked = walked.saturating_add(width);
            offset = offset.saturating_add(cluster.len());
        }
        self.cursor = offset;
        self.scroll_into_view();
    }
}

impl AirView for AirInputLine {
    fn state(&self) -> &AirViewState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AirViewState {
        &mut self.state
    }

    fn draw_content(&self, painter: &mut AirPainter<'_>, context: &AirDrawContext<'_>) {
        let palette = &context.theme.palette;
        let role = if self.state.is_enabled() {
            AirRole::WindowText
        } else {
            AirRole::ButtonDisabled
        };
        let colors = palette.color_pair(role);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::from_parts(0, 0, size.columns, 1),
            AirCell::new(' ', colors.foreground, colors.background),
        );
        let Some(visible) = self.text.get(self.first_visible..) else {
            return;
        };
        painter.draw_text(
            AirPoint::new(AirColumn::FIRST, AirRow::FIRST),
            visible,
            colors.foreground,
            colors.background,
            AirAttributes::NONE,
        );
    }

    fn cursor(&self) -> Option<AirPoint> {
        if !self.state.is_enabled() {
            return None;
        }
        Some(AirPoint::new(
            AirColumn::new(self.cursor_column()),
            AirRow::FIRST,
        ))
    }

    fn handle_event(&mut self, event: &mut AirEvent, context: &mut AirContext<'_>) -> AirOutcome {
        let _ = context;
        if !self.state.is_enabled() {
            return AirOutcome::Ignored;
        }
        let Some(input) = event.input() else {
            return AirOutcome::Ignored;
        };
        match input {
            AirInputEvent::Mouse(mouse) => {
                if !matches!(mouse.kind, AirMouseEventKind::Pressed(AirMouseButton::Left)) {
                    return AirOutcome::Ignored;
                }
                self.move_to_column(mouse.position.column.value());
                event.consume();
                AirOutcome::Consumed
            }
            AirInputEvent::Key(key) => {
                // Une frappe avec `Ctrl` ou `Alt` ne se saisit pas : elle appartient aux
                // raccourcis, et la barre de menu ou l'application en décide.
                if key.modifiers.control || key.modifiers.alternate {
                    return AirOutcome::Ignored;
                }
                let handled = match key.logical {
                    // Le caractère littéral, pas la touche de base : sur un clavier
                    // français, `AltGr`+`0` doit saisir `@` et non `à`.
                    AirKey::Character(character) => self.insert(character),
                    AirKey::Backspace => self.delete_backward(),
                    AirKey::Delete => self.delete_forward(),
                    AirKey::Left => self.move_left(),
                    AirKey::Right => self.move_right(),
                    AirKey::Home => {
                        self.move_to_start();
                        true
                    }
                    AirKey::End => {
                        self.move_to_end();
                        true
                    }
                    _ => return AirOutcome::Ignored,
                };
                // Consommé même quand rien n'a bougé : `Retour arrière` au début du champ
                // est une frappe qui appartient au champ, et la laisser remonter la
                // ferait agir ailleurs.
                let _ = handled;
                event.consume();
                AirOutcome::Consumed
            }
            _ => AirOutcome::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AirInputLine;
    use crate::command::AirCommandSet;
    use crate::geometry::{AirColumn, AirPoint, AirRectangle, AirRow, AirSize};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirModifiers, AirMouseButton, AirMouseEvent,
        AirMouseEventKind,
    };
    use crate::surface::AirSurface;
    use crate::theme::AirTheme;
    use crate::view::painter::AirPainter;
    use crate::view::{
        AirContext, AirDrawContext, AirEvent, AirOutcome, AirView, AirViewId, AirViewState,
        dispatch_event, draw_view,
    };

    fn field(columns: u16) -> AirInputLine {
        AirInputLine::new(AirViewState::new(
            AirViewId::FIRST,
            AirRectangle::from_parts(0, 0, columns, 1),
        ))
    }

    fn point(column: u16, row: u16) -> AirPoint {
        AirPoint::new(AirColumn::new(column), AirRow::new(row))
    }

    fn key(logical: AirKey) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(AirKeyEvent::press(logical)))
    }

    fn key_with(logical: AirKey, modifiers: AirModifiers) -> AirEvent {
        AirEvent::new(AirInputEvent::Key(
            AirKeyEvent::press(logical).with_modifiers(modifiers),
        ))
    }

    fn click(column: u16) -> AirEvent {
        AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(column, 0),
            kind: AirMouseEventKind::Pressed(AirMouseButton::Left),
            modifiers: AirModifiers::NONE,
        }))
    }

    fn dispatch(input_line: &mut AirInputLine, event: &mut AirEvent) -> AirOutcome {
        let mut commands = AirCommandSet::new();
        let mut context = AirContext {
            commands: &mut commands,
        };
        dispatch_event(input_line, event, &mut context)
    }

    fn type_text(input_line: &mut AirInputLine, text: &str) {
        for character in text.chars() {
            let mut event = key(AirKey::Character(character));
            dispatch(input_line, &mut event);
        }
    }

    /// Dessine le champ et rend la ligne obtenue, avec la position absolue du curseur.
    fn render(input_line: &AirInputLine, columns: u16) -> (String, Option<AirPoint>) {
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        let size = AirSize::new(columns, 1);
        let mut surface = AirSurface::new(size);
        let cursor = {
            let mut painter = AirPainter::new(&mut surface);
            let Some(mut own) = painter.child(input_line.state().bounds()) else {
                panic!("le champ doit tenir dans la surface");
            };
            draw_view(
                input_line,
                &mut own,
                &AirDrawContext::new(&theme, &commands),
            )
        };
        let line = (0..columns)
            .map(|column| {
                surface
                    .cell(point(column, 0))
                    .map_or(' ', |found| found.character)
            })
            .collect();
        (line, cursor)
    }

    #[test]
    fn un_champ_neuf_est_vide_et_sans_bordure() {
        // ADR-010 pose la bordure par défaut ; un champ d'une ligne n'a pas la place.
        let input_line = field(10);
        assert!(!input_line.state().has_border());
        assert_eq!(input_line.text(), "");
        assert_eq!(input_line.cursor_offset(), 0);
        assert_eq!(input_line.length(), 0);
        assert_eq!(input_line.maximum_length(), None);
        assert_eq!(input_line.first_visible_offset(), 0);
    }

    #[test]
    fn un_texte_initial_place_le_curseur_a_la_fin() {
        let input_line = field(10).with_text("abc");
        assert_eq!(input_line.text(), "abc");
        assert_eq!(input_line.cursor_offset(), 3);
        assert_eq!(input_line.cursor_column(), 3);
    }

    #[test]
    fn la_saisie_insere_au_curseur() {
        let mut input_line = field(20);
        type_text(&mut input_line, "abc");
        assert_eq!(input_line.text(), "abc");
        input_line.move_to_start();
        type_text(&mut input_line, "X");
        assert_eq!(input_line.text(), "Xabc");
        assert_eq!(input_line.cursor_offset(), 1);
    }

    #[test]
    fn la_saisie_consomme_la_frappe() {
        // Sinon elle remonterait à la barre de statut ou à l'application.
        let mut input_line = field(20);
        let mut event = key(AirKey::Character('a'));
        assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Consumed);
        assert!(event.is_consumed());
    }

    #[test]
    fn une_frappe_avec_ctrl_ou_alt_n_est_pas_saisie() {
        // Elle appartient aux raccourcis : la saisir écrirait une lettre pour un raccourci.
        let mut input_line = field(20);
        for modifiers in [
            AirModifiers::NONE.with_control(),
            AirModifiers::NONE.with_alternate(),
        ] {
            let mut event = key_with(AirKey::Character('s'), modifiers);
            assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Ignored);
            assert!(!event.is_consumed());
        }
        assert_eq!(input_line.text(), "");
    }

    #[test]
    fn une_touche_inconnue_n_est_pas_consommee() {
        let mut input_line = field(20);
        let mut event = key(AirKey::Function(2));
        assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Ignored);
        assert!(!event.is_consumed());
    }

    #[test]
    fn un_champ_desactive_ne_saisit_rien_et_n_a_pas_de_curseur() {
        let mut input_line = field(20).with_text("abc");
        input_line.state_mut().set_enabled(false);
        let mut event = key(AirKey::Character('d'));
        assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Ignored);
        assert_eq!(input_line.text(), "abc");
        assert_eq!(input_line.cursor(), None);
    }

    #[test]
    fn le_retour_arriere_efface_la_grappe_entiere() {
        // « é » décomposé est deux caractères et une seule touche effacée : effacer un
        // caractère laisserait un accent orphelin.
        let mut input_line = field(20).with_text("abe\u{0301}");
        assert_eq!(input_line.length(), 3);
        assert!(input_line.delete_backward());
        assert_eq!(input_line.text(), "ab");
        assert_eq!(input_line.cursor_offset(), 2);
    }

    #[test]
    fn le_retour_arriere_au_debut_ne_fait_rien_mais_consomme() {
        let mut input_line = field(20).with_text("ab");
        input_line.move_to_start();
        assert!(!input_line.delete_backward());
        assert_eq!(input_line.text(), "ab");
        // La frappe appartient au champ : la laisser remonter la ferait agir ailleurs.
        let mut event = key(AirKey::Backspace);
        assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Consumed);
    }

    #[test]
    fn la_suppression_avant_efface_la_grappe_suivante() {
        let mut input_line = field(20).with_text("e\u{0301}ab");
        input_line.move_to_start();
        assert!(input_line.delete_forward());
        assert_eq!(input_line.text(), "ab");
        assert_eq!(input_line.cursor_offset(), 0);
        input_line.move_to_end();
        assert!(!input_line.delete_forward());
    }

    #[test]
    fn les_fleches_se_deplacent_par_grappes() {
        // Trois grappes, dont une de deux caractères et une de quatre.
        let mut input_line = field(20).with_text("ae\u{0301}\u{1F468}");
        assert_eq!(input_line.length(), 3);
        input_line.move_to_start();
        assert_eq!(input_line.cursor_offset(), 0);
        assert!(input_line.move_right());
        assert_eq!(input_line.cursor_offset(), 1);
        assert!(input_line.move_right());
        assert_eq!(input_line.cursor_offset(), 4);
        assert!(input_line.move_right());
        assert_eq!(input_line.cursor_offset(), 8);
        assert!(!input_line.move_right());
        assert!(input_line.move_left());
        assert_eq!(input_line.cursor_offset(), 4);
        assert!(input_line.move_left());
        assert_eq!(input_line.cursor_offset(), 1);
        assert!(input_line.move_left());
        assert_eq!(input_line.cursor_offset(), 0);
        assert!(!input_line.move_left());
    }

    #[test]
    fn les_touches_de_deplacement_passent_par_l_evenement() {
        let mut input_line = field(20).with_text("abc");
        for (logical, expected) in [
            (AirKey::Home, 0),
            (AirKey::Right, 1),
            (AirKey::End, 3),
            (AirKey::Left, 2),
        ] {
            let mut event = key(logical);
            assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Consumed);
            assert_eq!(input_line.cursor_offset(), expected);
        }
        let mut event = key(AirKey::Delete);
        assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Consumed);
        assert_eq!(input_line.text(), "ab");
    }

    #[test]
    fn la_colonne_du_curseur_compte_les_largeurs_affichees() {
        // Un idéogramme occupe deux colonnes : compter les grappes décalerait le curseur.
        let input_line = field(20).with_text("a漢b");
        assert_eq!(input_line.length(), 3);
        assert_eq!(input_line.cursor_column(), 4);
    }

    #[test]
    fn la_longueur_maximale_refuse_la_saisie_de_trop() {
        let mut input_line = field(20).with_maximum_length(3);
        assert_eq!(input_line.maximum_length(), Some(3));
        type_text(&mut input_line, "abcd");
        assert_eq!(input_line.text(), "abc");
        assert!(!input_line.insert('d'));
    }

    #[test]
    fn la_longueur_maximale_coupe_un_texte_trop_long_sur_une_grappe() {
        // La coupe est sur une frontière de grappe : couper « é » décomposé en deux
        // laisserait un accent orphelin dans le champ.
        let input_line = field(20).with_text("ae\u{0301}bc").with_maximum_length(2);
        assert_eq!(input_line.text(), "ae\u{0301}");
        assert_eq!(input_line.length(), 2);
    }

    #[test]
    fn la_longueur_maximale_vaut_aussi_pour_set_text() {
        let mut input_line = field(20).with_maximum_length(2);
        input_line.set_text("abcd");
        assert_eq!(input_line.text(), "ab");
        assert_eq!(input_line.cursor_offset(), 2);
    }

    #[test]
    fn le_texte_defile_pour_garder_le_curseur_visible() {
        // Six colonnes, cinq utilisables : la sixième est réservée au curseur, sans quoi
        // taper en bout de champ le placerait hors de la vue.
        let mut input_line = field(6);
        type_text(&mut input_line, "abcdefgh");
        assert_eq!(input_line.text(), "abcdefgh");
        let (line, cursor) = render(&input_line, 6);
        assert_eq!(line, "defgh ");
        assert_eq!(input_line.cursor_column(), 5);
        // Le curseur n'est rendu que si la vue a le focus.
        assert_eq!(cursor, None);
        // Revenir au début ramène la fenêtre avec lui.
        input_line.move_to_start();
        assert_eq!(input_line.first_visible_offset(), 0);
        assert_eq!(render(&input_line, 6).0, "abcdef");
        assert_eq!(input_line.cursor_column(), 0);
    }

    #[test]
    fn le_defilement_avance_par_grappes_et_non_par_colonnes() {
        // Avec un idéogramme à deux colonnes, avancer d'une colonne couperait le glyphe.
        let mut input_line = field(5);
        type_text(&mut input_line, "漢漢漢");
        // Le texte fait six colonnes, quatre sont utilisables : la première grappe sort.
        assert_eq!(input_line.first_visible_offset(), 3);
        assert_eq!(input_line.cursor_column(), 4);
    }

    #[test]
    fn un_champ_de_largeur_nulle_ne_boucle_pas() {
        // Cas dégénéré : aucune colonne utilisable. Il doit se terminer, pas tourner.
        let mut input_line = AirInputLine::new(AirViewState::new(
            AirViewId::FIRST,
            AirRectangle::from_parts(0, 0, 0, 1),
        ));
        type_text(&mut input_line, "abc");
        assert_eq!(input_line.text(), "abc");
        assert_eq!(input_line.first_visible_offset(), 3);
        assert_eq!(input_line.cursor_column(), 0);
    }

    #[test]
    fn un_clic_place_le_curseur_sur_la_grappe_visee() {
        let mut input_line = field(20).with_text("a漢b");
        // Colonne 0 : la première grappe.
        let mut event = click(0);
        assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Consumed);
        assert_eq!(input_line.cursor_offset(), 0);
        // Colonnes 1 et 2 : les deux moitiés de l'idéogramme désignent la même grappe.
        let mut event = click(1);
        dispatch(&mut input_line, &mut event);
        assert_eq!(input_line.cursor_offset(), 1);
        let mut event = click(2);
        dispatch(&mut input_line, &mut event);
        assert_eq!(input_line.cursor_offset(), 1);
        // Colonne 3 : la grappe qui suit.
        let mut event = click(3);
        dispatch(&mut input_line, &mut event);
        assert_eq!(input_line.cursor_offset(), 4);
    }

    #[test]
    fn un_clic_dans_le_vide_place_le_curseur_a_la_fin() {
        let mut input_line = field(20).with_text("abc");
        let mut event = click(15);
        dispatch(&mut input_line, &mut event);
        assert_eq!(input_line.cursor_offset(), 3);
    }

    #[test]
    fn un_clic_tient_compte_du_defilement() {
        let mut input_line = field(6);
        type_text(&mut input_line, "abcdefgh");
        assert_eq!(input_line.first_visible_offset(), 3);
        // La colonne 0 affiche « d », quatrième grappe du texte.
        input_line.move_to_column(0);
        assert_eq!(input_line.cursor_offset(), 3);
    }

    #[test]
    fn un_relachement_de_souris_est_ignore() {
        let mut input_line = field(20);
        let mut event = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(1, 0),
            kind: AirMouseEventKind::Released(Some(AirMouseButton::Left)),
            modifiers: AirModifiers::NONE,
        }));
        assert_eq!(dispatch(&mut input_line, &mut event), AirOutcome::Ignored);
    }

    #[test]
    fn le_curseur_n_est_rendu_que_pour_la_vue_focalisee() {
        // C'est la traversée qui applique la règle, pas la vue : le champ répond toujours.
        let mut input_line = field(10).with_text("abc");
        assert_eq!(input_line.cursor(), Some(point(3, 0)));
        let (_, without_focus) = render(&input_line, 10);
        assert_eq!(without_focus, None);
        input_line.state_mut().set_focused(true);
        let (_, with_focus) = render(&input_line, 10);
        assert_eq!(with_focus, Some(point(3, 0)));
    }
}
