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
//!
//! # La sélection est **locale**, et c'est voulu
//!
//! Faite au jalon M7d. Là où [`crate::widgets::AirEditorView`] lit la sélection dans son
//! tampon, ce champ la tient lui-même : il tient déjà son texte, et lui donner un modèle
//! pour une ligne serait payer l'ADR-007 sans rien en retirer.
//!
//! Même ancre que le tampon, plutôt que deux bornes ordonnées — c'est ce qui permet de
//! réduire une sélection en revenant en arrière —, mêmes gestes (`Maj`+flèches,
//! `Maj`+`Début`, `Maj`+`Fin`, `Maj`+clic, glisser, `Ctrl`+`A`), et les trois commandes de
//! presse-papiers **demandées** sans jamais le toucher.
//!
//! Un texte collé perd ses sauts de ligne : un champ d'une ligne ne peut pas les porter.
//! Les garder écrirait un caractère de contrôle dans le texte ; refuser le collage entier
//! perdrait la saisie.

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
    /// L'autre borne de la sélection, posée au début de celle-ci.
    ///
    /// Locale au widget, là où la vue d'édition la lit dans son tampon : un champ de
    /// saisie tient son texte lui-même, et lui donner un modèle pour une ligne serait
    /// payer l'ADR-007 sans rien en retirer.
    anchor: Option<usize>,
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
            anchor: None,
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
        self.set_maximum_length(Some(maximum_length));
        self
    }

    /// Change la longueur maximale, et coupe le texte s'il la dépasse.
    ///
    /// `None` retire la limite. La coupure se fait sur une frontière de grappe : réduire
    /// la limite ne peut pas laisser un accent orphelin.
    pub fn set_maximum_length(&mut self, maximum_length: Option<usize>) {
        self.maximum_length = maximum_length;
        self.truncate_to_maximum();
        self.scroll_into_view();
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

    /// Les bornes de la sélection, **ordonnées**, ou `None` s'il n'y en a pas.
    ///
    /// Une sélection vide rend `None`, comme dans le tampon de texte : il n'y a rien à
    /// peindre et rien à supprimer.
    #[must_use]
    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        if anchor == self.cursor {
            return None;
        }
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// Vrai s'il y a une sélection non vide.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    /// Le texte sélectionné, ou `None` s'il n'y a pas de sélection.
    #[must_use]
    pub fn selected_text(&self) -> Option<&str> {
        let (start, end) = self.selection()?;
        self.text.get(start..end)
    }

    /// Sélectionne tout le texte et porte le curseur à la fin.
    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.text.len();
        self.scroll_into_view();
    }

    /// Sélectionne le segment de texte qui couvre le curseur.
    ///
    /// Le **segment** au sens de l'UAX #29 : un mot, mais aussi une suite d'espaces ou un
    /// signe de ponctuation. Un double clic sur un espace doit sélectionner les espaces,
    /// et non rester sans effet visible.
    pub fn select_segment(&mut self) {
        let (start, end) = crate::unicode::segment_at(&self.text, self.cursor);
        self.anchor = Some(start);
        self.cursor = end;
        self.scroll_into_view();
    }

    /// Abandonne la sélection, sans toucher au texte ni au curseur.
    pub const fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// Supprime le texte sélectionné. Rend `false` s'il n'y en avait pas.
    pub fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection() else {
            self.anchor = None;
            return false;
        };
        self.text.replace_range(start..end, "");
        self.cursor = start;
        self.anchor = None;
        self.scroll_into_view();
        true
    }

    /// Insère un texte au curseur, en remplaçant la sélection s'il y en a une.
    ///
    /// Les sauts de ligne sont **retirés** : un champ d'une ligne ne peut pas les porter,
    /// et un collage multiligne arrive par le presse-papiers comme par le terminal. Les
    /// garder écrirait un caractère de contrôle dans le texte, les refuser perdrait le
    /// collage entier — les retirer est ce que fait tout champ d'une ligne.
    pub fn insert_text(&mut self, inserted: &str) -> bool {
        self.delete_selection();
        let mut accepted = false;
        for character in inserted
            .chars()
            .filter(|character| *character != '\n' && *character != '\r')
        {
            if !self.insert(character) {
                break;
            }
            accepted = true;
        }
        accepted
    }

    /// Pose l'ancre sur le curseur si elle n'y est pas déjà.
    const fn begin_selection(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
    }

    /// Insère un caractère au curseur. Rend `false` si la longueur maximale l'interdit.
    ///
    /// La sélection n'est **pas** remplacée ici : c'est [`AirInputLine::insert_text`] et le
    /// traitement des frappes qui s'en chargent. Une insertion d'un caractère reste donc
    /// l'opération élémentaire, employée par les deux.
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

    /// Déplace le curseur d'une grappe vers la gauche, et **abandonne la sélection**.
    ///
    /// C'est la flèche nue. Son pendant qui étend est [`AirInputLine::select_left`] : deux
    /// méthodes nommées plutôt qu'un drapeau booléen, qui ne dirait rien au site d'appel.
    pub fn move_left(&mut self) -> bool {
        self.anchor = None;
        self.move_cursor_left()
    }

    /// Déplace le curseur d'une grappe vers la droite, et abandonne la sélection.
    pub fn move_right(&mut self) -> bool {
        self.anchor = None;
        self.move_cursor_right()
    }

    /// Place le curseur au début du texte, et abandonne la sélection.
    pub fn move_to_start(&mut self) {
        self.anchor = None;
        self.cursor = 0;
        self.scroll_into_view();
    }

    /// Place le curseur à la fin du texte, et abandonne la sélection.
    pub fn move_to_end(&mut self) {
        self.anchor = None;
        self.cursor = self.text.len();
        self.scroll_into_view();
    }

    /// Recule le curseur d'une grappe, sans rien décider de la sélection.
    fn move_cursor_left(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        self.cursor = self.previous_boundary(self.cursor);
        self.scroll_into_view();
        true
    }

    /// Avance le curseur d'une grappe, sans rien décider de la sélection.
    fn move_cursor_right(&mut self) -> bool {
        if self.cursor >= self.text.len() {
            return false;
        }
        self.cursor = self.next_boundary(self.cursor);
        self.scroll_into_view();
        true
    }

    /// Étend la sélection d'une grappe vers la gauche. Rend `false` si le curseur est au
    /// début.
    pub fn select_left(&mut self) -> bool {
        self.begin_selection();
        self.move_cursor_left()
    }

    /// Étend la sélection d'une grappe vers la droite. Rend `false` si le curseur est à la
    /// fin.
    pub fn select_right(&mut self) -> bool {
        self.begin_selection();
        self.move_cursor_right()
    }

    /// Étend la sélection jusqu'au début du texte.
    pub fn select_to_start(&mut self) {
        self.begin_selection();
        self.cursor = 0;
        self.scroll_into_view();
    }

    /// Étend la sélection jusqu'à la fin du texte.
    pub fn select_to_end(&mut self) {
        self.begin_selection();
        self.cursor = self.text.len();
        self.scroll_into_view();
    }

    /// Étend la sélection jusqu'à la grappe affichée à la colonne donnée.
    pub fn select_to_column(&mut self, column: u16) {
        self.begin_selection();
        self.place_cursor_at_column(column);
    }

    /// Place le curseur sur la grappe affichée à la colonne donnée.
    ///
    /// Une colonne au delà du texte place le curseur à la fin : c'est ce qu'un clic dans
    /// le vide d'un champ doit faire.
    pub fn move_to_column(&mut self, column: u16) {
        self.anchor = None;
        self.place_cursor_at_column(column);
    }

    /// Place le curseur à une colonne, sans rien décider de la sélection.
    fn place_cursor_at_column(&mut self, column: u16) {
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
            AirRole::InputLineText
        } else {
            AirRole::ButtonDisabled
        };
        let colors = palette.color_pair(role);
        let marked = palette.color_pair(AirRole::InputLineSelected);
        let size = painter.size();
        painter.fill_rectangle(
            AirRectangle::from_parts(0, 0, size.columns, 1),
            AirCell::new(' ', colors.foreground, colors.background),
        );
        let Some(visible) = self.text.get(self.first_visible..) else {
            return;
        };
        let Some((start, end)) = self.selection() else {
            painter.draw_text(
                AirPoint::new(AirColumn::FIRST, AirRow::FIRST),
                visible,
                colors.foreground,
                colors.background,
                AirAttributes::NONE,
            );
            return;
        };
        // Grappe par grappe, parce que la couleur change en cours de ligne. Le décalage
        // est en octets et non en colonnes : le champ garde son texte entier, donc les
        // bornes de la sélection s'y comparent directement — contrairement à la vue
        // d'édition, qui ne garde que les colonnes visibles.
        let mut offset = self.first_visible;
        let mut column: u16 = 0;
        for cluster in graphemes(visible) {
            if column >= size.columns {
                break;
            }
            let inside = offset >= start && offset < end;
            let pair = if inside { marked } else { colors };
            painter.draw_text(
                AirPoint::new(AirColumn::new(column), AirRow::FIRST),
                cluster,
                pair.foreground,
                pair.background,
                AirAttributes::NONE,
            );
            offset = offset.saturating_add(cluster.len());
            column = column.saturating_add(u16::try_from(grapheme_width(cluster)).unwrap_or(1));
        }
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
                match mouse.kind {
                    AirMouseEventKind::Pressed(AirMouseButton::Left) => {
                        if mouse.modifiers.shift {
                            self.select_to_column(mouse.position.column.value());
                        } else {
                            self.move_to_column(mouse.position.column.value());
                        }
                    }
                    // Le glisser sélectionne, comme dans la vue d'édition, et sans garder
                    // le point d'appui : l'ancre posée par le premier déplacement le tient.
                    AirMouseEventKind::Moved(Some(AirMouseButton::Left)) => {
                        self.select_to_column(mouse.position.column.value());
                    }
                    // Le double clic suit son enfoncement, qui a déjà posé le curseur.
                    AirMouseEventKind::DoubleClicked(AirMouseButton::Left) => {
                        self.select_segment();
                    }
                    // Un champ d'une ligne n'a qu'une ligne à prendre : le triple clic y
                    // vaut « tout sélectionner », ce qui est ce que fait tout champ.
                    AirMouseEventKind::TripleClicked(AirMouseButton::Left) => {
                        self.select_all();
                    }
                    _ => return AirOutcome::Ignored,
                }
                event.consume();
                AirOutcome::Consumed
            }
            AirInputEvent::Key(key) => {
                // Une frappe avec `Ctrl` ou `Alt` ne se saisit pas : elle appartient aux
                // raccourcis, et la barre de menu ou l'application en décide. `Ctrl`+`A`
                // fait exception, parce que sélectionner tout n'a d'effet que dans le
                // champ qui a le focus.
                if key.modifiers.alternate {
                    return AirOutcome::Ignored;
                }
                if key.modifiers.control {
                    if matches!(key.logical, AirKey::Character('a' | 'A')) {
                        self.select_all();
                        event.consume();
                        return AirOutcome::Consumed;
                    }
                    // Les trois gestes du presse-papiers sortent par le canal des
                    // commandes : le presse-papiers est au contrôleur, pas au champ.
                    let command = match key.logical {
                        AirKey::Character('c' | 'C') => crate::command::COPY,
                        AirKey::Character('x' | 'X') => crate::command::CUT,
                        AirKey::Character('v' | 'V') => crate::command::PASTE,
                        _ => return AirOutcome::Ignored,
                    };
                    event.consume();
                    return AirOutcome::Command(command);
                }
                let shift = key.modifiers.shift;
                let handled = match key.logical {
                    // Le caractère littéral, pas la touche de base : sur un clavier
                    // français, `AltGr`+`0` doit saisir `@` et non `à`.
                    AirKey::Character(character) => {
                        // Taper sur une sélection la remplace, comme dans l'éditeur.
                        self.delete_selection();
                        self.insert(character)
                    }
                    // Sur une sélection, le retour arrière et la touche de suppression la
                    // suppriment et **s'arrêtent** : y ajouter une grappe retirerait un
                    // caractère que l'utilisateur ne voyait pas surligné.
                    AirKey::Backspace => self.delete_selection() || self.delete_backward(),
                    AirKey::Delete => self.delete_selection() || self.delete_forward(),
                    AirKey::Left if shift => self.select_left(),
                    AirKey::Right if shift => self.select_right(),
                    AirKey::Left => self.move_left(),
                    AirKey::Right => self.move_right(),
                    AirKey::Home if shift => {
                        self.select_to_start();
                        true
                    }
                    AirKey::End if shift => {
                        self.select_to_end();
                        true
                    }
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
            // Le collage encadré du terminal, décodé depuis M1 et jusqu'ici jeté.
            AirInputEvent::Paste(pasted) => {
                self.insert_text(pasted);
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
    fn changer_la_longueur_maximale_coupe_ou_libere() {
        let mut input_line = field(20).with_text("abcdef");
        input_line.set_maximum_length(Some(3));
        assert_eq!(input_line.text(), "abc");
        assert_eq!(input_line.cursor_offset(), 3);
        input_line.set_maximum_length(None);
        assert_eq!(input_line.maximum_length(), None);
        type_text(&mut input_line, "xyz");
        assert_eq!(input_line.text(), "abcxyz");
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

    // ------------------------------------------------------- la sélection

    /// Les fonds de chaque colonne du champ, sous le thème Turbo Vision.
    fn backgrounds(input_line: &AirInputLine, columns: u16) -> Vec<Option<AirColor>> {
        let theme = AirTheme::turbo_vision();
        let commands = AirCommandSet::new();
        let size = AirSize::new(columns, 1);
        let mut surface = AirSurface::new(size);
        {
            let mut painter = AirPainter::new(&mut surface);
            let Some(mut own) = painter.child(input_line.state().bounds()) else {
                panic!("le champ doit tenir dans la surface");
            };
            draw_view(
                input_line,
                &mut own,
                &AirDrawContext::new(&theme, &commands),
            );
        }
        (0..columns)
            .map(|column| surface.cell(point(column, 0)).map(|cell| cell.background))
            .collect()
    }

    /// Le fond du texte sélectionné et celui du texte ordinaire, sous Turbo Vision.
    fn selection_backgrounds() -> (AirColor, AirColor) {
        let theme = AirTheme::turbo_vision();
        (
            theme
                .palette
                .color_pair(AirRole::InputLineSelected)
                .background,
            theme.palette.color_pair(AirRole::InputLineText).background,
        )
    }

    fn shift() -> AirModifiers {
        AirModifiers {
            shift: true,
            ..AirModifiers::NONE
        }
    }

    #[test]
    fn maj_et_une_fleche_selectionnent_dans_le_champ() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcd");
        dispatch(&mut input_line, &mut key(AirKey::Home));
        dispatch(&mut input_line, &mut key_with(AirKey::Right, shift()));
        dispatch(&mut input_line, &mut key_with(AirKey::Right, shift()));
        assert_eq!(input_line.selected_text(), Some("ab"));
    }

    #[test]
    fn le_texte_selectionne_du_champ_est_surligne() {
        let (marked, normal) = selection_backgrounds();
        let mut input_line = field(4);
        type_text(&mut input_line, "abcd");
        dispatch(&mut input_line, &mut key(AirKey::Home));
        dispatch(&mut input_line, &mut key_with(AirKey::Right, shift()));
        dispatch(&mut input_line, &mut key_with(AirKey::Right, shift()));
        assert_eq!(
            backgrounds(&input_line, 4),
            vec![Some(marked), Some(marked), Some(normal), Some(normal)]
        );
    }

    #[test]
    fn un_champ_sans_selection_n_a_aucun_surlignage() {
        let (_, normal) = selection_backgrounds();
        let mut input_line = field(4);
        type_text(&mut input_line, "abcd");
        assert!(
            backgrounds(&input_line, 4)
                .iter()
                .all(|background| *background == Some(normal))
        );
    }

    #[test]
    fn une_fleche_nue_abandonne_la_selection_du_champ() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcd");
        dispatch(&mut input_line, &mut key(AirKey::Home));
        dispatch(&mut input_line, &mut key_with(AirKey::Right, shift()));
        assert!(input_line.has_selection());
        dispatch(&mut input_line, &mut key(AirKey::Right));
        assert_eq!(input_line.selection(), None);
    }

    #[test]
    fn maj_et_fin_selectionnent_jusqu_au_bout() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcd");
        dispatch(&mut input_line, &mut key(AirKey::Home));
        dispatch(&mut input_line, &mut key_with(AirKey::End, shift()));
        assert_eq!(input_line.selected_text(), Some("abcd"));
    }

    #[test]
    fn maj_et_debut_selectionnent_jusqu_au_commencement() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcd");
        dispatch(&mut input_line, &mut key_with(AirKey::Home, shift()));
        assert_eq!(input_line.selected_text(), Some("abcd"));
    }

    #[test]
    fn taper_sur_une_selection_la_remplace_dans_le_champ() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcd");
        input_line.select_all();
        type_text(&mut input_line, "X");
        assert_eq!(input_line.text(), "X");
    }

    #[test]
    fn le_retour_arriere_sur_une_selection_du_champ_ne_mange_rien_de_plus() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcd");
        dispatch(&mut input_line, &mut key(AirKey::Home));
        dispatch(&mut input_line, &mut key(AirKey::Right));
        dispatch(&mut input_line, &mut key_with(AirKey::Right, shift()));
        dispatch(&mut input_line, &mut key(AirKey::Backspace));
        assert_eq!(input_line.text(), "acd");
        assert_eq!(input_line.cursor_offset(), 1);
    }

    #[test]
    fn ctrl_a_selectionne_tout_le_champ() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcd");
        let outcome = dispatch(
            &mut input_line,
            &mut key_with(
                AirKey::Character('a'),
                AirModifiers {
                    control: true,
                    ..AirModifiers::NONE
                },
            ),
        );
        assert_eq!(outcome, AirOutcome::Consumed);
        assert_eq!(input_line.selected_text(), Some("abcd"));
    }

    #[test]
    fn une_autre_frappe_avec_controle_reste_aux_raccourcis() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcd");
        let outcome = dispatch(
            &mut input_line,
            &mut key_with(
                AirKey::Character('s'),
                AirModifiers {
                    control: true,
                    ..AirModifiers::NONE
                },
            ),
        );
        assert_eq!(outcome, AirOutcome::Ignored);
        assert_eq!(input_line.text(), "abcd");
    }

    #[test]
    fn glisser_la_souris_selectionne_dans_le_champ() {
        let mut input_line = field(10);
        type_text(&mut input_line, "abcdef");
        dispatch(&mut input_line, &mut click(1));
        let mut dragged = AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(4, 0),
            kind: AirMouseEventKind::Moved(Some(AirMouseButton::Left)),
            modifiers: AirModifiers::NONE,
        }));
        dispatch(&mut input_line, &mut dragged);
        assert_eq!(input_line.selected_text(), Some("bcd"));
    }

    #[test]
    fn un_texte_colle_perd_ses_sauts_de_ligne() {
        // Un champ d'une ligne ne peut pas les porter. Les garder écrirait un caractère de
        // contrôle dans le texte ; refuser le collage entier perdrait la saisie.
        let mut input_line = field(20);
        assert!(input_line.insert_text("ab\ncd"));
        assert_eq!(input_line.text(), "abcd");
    }

    #[test]
    fn un_texte_colle_remplace_la_selection_du_champ() {
        let mut input_line = field(20);
        type_text(&mut input_line, "abcd");
        input_line.select_all();
        assert!(input_line.insert_text("XY"));
        assert_eq!(input_line.text(), "XY");
    }

    #[test]
    fn un_texte_colle_s_arrete_a_la_longueur_maximale() {
        let mut input_line = field(20).with_maximum_length(3);
        assert!(input_line.insert_text("abcdef"));
        assert_eq!(input_line.text(), "abc");
    }

    // ------------------------------------------------- le clic multiple

    fn mouse_event(kind: AirMouseEventKind, column: u16) -> AirEvent {
        AirEvent::new(AirInputEvent::Mouse(AirMouseEvent {
            position: point(column, 0),
            kind,
            modifiers: AirModifiers::NONE,
        }))
    }

    #[test]
    fn un_double_clic_selectionne_le_mot_du_champ() {
        let mut input_line = field(20);
        type_text(&mut input_line, "une belle");
        dispatch(&mut input_line, &mut click(6));
        dispatch(
            &mut input_line,
            &mut mouse_event(AirMouseEventKind::DoubleClicked(AirMouseButton::Left), 6),
        );
        assert_eq!(input_line.selected_text(), Some("belle"));
    }

    #[test]
    fn un_double_clic_sur_un_espace_selectionne_les_espaces() {
        let mut input_line = field(20);
        type_text(&mut input_line, "une  belle");
        dispatch(&mut input_line, &mut click(3));
        dispatch(
            &mut input_line,
            &mut mouse_event(AirMouseEventKind::DoubleClicked(AirMouseButton::Left), 3),
        );
        assert_eq!(input_line.selected_text(), Some("  "));
    }

    #[test]
    fn un_triple_clic_selectionne_tout_le_champ() {
        // Un champ d'une ligne n'a qu'une ligne à prendre.
        let mut input_line = field(20);
        type_text(&mut input_line, "une belle");
        dispatch(&mut input_line, &mut click(6));
        dispatch(
            &mut input_line,
            &mut mouse_event(AirMouseEventKind::TripleClicked(AirMouseButton::Left), 6),
        );
        assert_eq!(input_line.selected_text(), Some("une belle"));
    }
}
