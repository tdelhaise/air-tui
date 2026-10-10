// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le tampon de texte — le modèle de l'éditeur.
//!
//! # Il ne connaît ni vue, ni terminal, ni fichier
//!
//! Ce tampon ne sait pas qu'il est affiché. Il n'a pas de dimension d'écran, pas de
//! défilement, pas de couleur, et il ne lit ni n'écrit aucun fichier : l'ouverture se
//! traduit en [`AirTextAction::Load`] et l'enregistrement lit [`AirTextBuffer::to_text`].
//! C'est ce qui le rend testable sans terminal — et tous les tests de ce module le sont.
//!
//! Le défilement appartient à la **vue**, parce qu'il dépend de la dimension de la fenêtre
//! et que deux vues du même tampon doivent pouvoir défiler séparément.
//!
//! # Des lignes, et le prix de ce choix
//!
//! Le texte est un `Vec<String>`, une ligne par entrée. Ce n'est ni un tampon à trou ni
//! une corde, et ce choix se paie :
//!
//! - accéder à la ligne `n` est en O(1), et l'édition dans une ligne est en O(longueur de
//!   la ligne) — ce qui est exactement le profil d'un éditeur de texte ;
//! - insérer ou supprimer une **ligne** déplace la queue du vecteur, donc en O(nombre de
//!   lignes). Sur un fichier de dix mille lignes, c'est imperceptible ; sur un fichier de
//!   dix millions, il faudrait une corde.
//!
//! La limite est assumée et documentée plutôt que contournée par une structure que
//! personne n'aurait besoin de relire. `tvedit` faisait le même choix.
//!
//! # Le curseur est un décalage en octets, sur une frontière de grappe
//!
//! Comme dans [`crate::widgets::AirInputLine`], et pour la même raison : un décalage est
//! stable à l'insertion comme à la suppression, là où un indice de grappe demanderait de
//! recompter la ligne entière. L'invariant « le décalage est sur une frontière de grappe »
//! est tenu par les déplacements, qui passent tous par
//! [`crate::unicode::next_grapheme_boundary`] et son symétrique.
//!
//! # La colonne désirée
//!
//! Monter puis descendre doit ramener le curseur où il était, même en traversant une
//! ligne courte. Le tampon garde donc une **colonne désirée**, que les déplacements
//! verticaux lisent sans la changer et que tous les autres remettent à la colonne réelle.
//! Sans elle, un aller-retour vertical à travers une ligne vide ramène le curseur en
//! colonne zéro — ce que tout éditeur sérieux évite.

use crate::model::{AirChangeSet, AirGeneration, AirModel};
use crate::unicode::{
    grapheme_width, graphemes, next_grapheme_boundary, next_word_start, previous_grapheme_boundary,
    previous_word_start, width_between,
};

/// Une position dans le tampon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct AirTextPosition {
    /// L'indice de la ligne, compté depuis zéro.
    pub line: usize,
    /// Le décalage en octets dans cette ligne, sur une frontière de grappe.
    pub offset: usize,
}

impl AirTextPosition {
    /// Une position.
    #[must_use]
    pub const fn new(line: usize, offset: usize) -> Self {
        Self { line, offset }
    }
}

/// Ce qu'on peut demander au tampon de texte.
///
/// Toute modification et tout déplacement passent par là : l'ensemble de ce qui peut
/// arriver au texte se lit d'un coup d'œil sur cette énumération (ADR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirTextAction {
    /// Insère un caractère au curseur.
    InsertCharacter(char),
    /// Coupe la ligne au curseur.
    InsertLineBreak,
    /// Supprime la grappe qui précède le curseur, ou recolle la ligne précédente.
    DeleteBackward,
    /// Supprime la grappe qui suit le curseur, ou recolle la ligne suivante.
    DeleteForward,
    /// Supprime la ligne courante entière.
    DeleteLine,
    /// Recule d'une grappe, en passant à la ligne précédente s'il faut.
    MoveLeft,
    /// Avance d'une grappe, en passant à la ligne suivante s'il faut.
    MoveRight,
    /// Monte d'une ligne, à la colonne désirée.
    MoveUp,
    /// Descend d'une ligne, à la colonne désirée.
    MoveDown,
    /// Va au début de la ligne.
    MoveToLineStart,
    /// Va à la fin de la ligne.
    MoveToLineEnd,
    /// Va au début du mot précédent.
    MoveToPreviousWord,
    /// Va au début du mot suivant.
    MoveToNextWord,
    /// Va au tout début du texte.
    MoveToDocumentStart,
    /// Va à la toute fin du texte.
    MoveToDocumentEnd,
    /// Monte d'autant de lignes.
    MoveUpByLines(usize),
    /// Descend d'autant de lignes.
    MoveDownByLines(usize),
    /// Place le curseur à une position donnée, bornée au texte.
    MoveTo(AirTextPosition),
    /// Place le curseur sur une ligne, à une colonne **d'affichage**.
    ///
    /// C'est ce qu'un clic produit : une souris donne une colonne, jamais un décalage en
    /// octets. La résolution appartient au modèle, seul à connaître le texte de la ligne —
    /// une vue qui la ferait aurait besoin de la ligne entière alors qu'elle n'en reflète
    /// que la partie visible.
    MoveToColumn {
        /// L'indice de la ligne, borné au texte.
        line: usize,
        /// La colonne d'affichage, bornée à la fin de la ligne.
        column: usize,
    },
    /// Remplace tout le contenu — c'est l'ouverture d'un fichier.
    Load(Vec<String>),
    /// Déclare le contenu enregistré.
    MarkSaved,
}

/// Ce que le tampon rapporte avoir changé.
///
/// Rien de ce qui est fréquent n'est ici : une frappe ne consigne rien, elle fait avancer
/// un compteur de génération. Le journal ne porte que les faits **structurels** qu'une
/// réaction a besoin de distinguer (ADR-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirTextChange {
    /// Le contenu entier a été remplacé.
    ContentReplaced,
    /// Le nombre de lignes a changé.
    LineCountChanged,
    /// Le drapeau de modification a basculé.
    ModifiedChanged,
}

/// Le tampon de texte.
///
/// Invariant : il y a **toujours au moins une ligne**, même vide. Un tampon sans ligne
/// obligerait chaque lecteur à traiter un cas qui n'a pas de sens à l'écran, et le curseur
/// n'aurait nulle part où se trouver.
#[derive(Debug, Clone)]
pub struct AirTextBuffer {
    /// Les lignes, sans leur caractère de fin.
    lines: Vec<String>,
    /// La position du curseur.
    cursor: AirTextPosition,
    /// La colonne d'affichage que les déplacements verticaux cherchent à retrouver.
    desired_column: usize,
    /// La génération du contenu.
    text_generation: AirGeneration,
    /// La génération de la position du curseur.
    cursor_generation: AirGeneration,
    /// Vrai si le contenu a changé depuis le dernier enregistrement.
    modified: bool,
}

impl Default for AirTextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl AirTextBuffer {
    /// Un tampon d'une seule ligne vide.
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: AirTextPosition::default(),
            desired_column: 0,
            text_generation: AirGeneration::new(),
            cursor_generation: AirGeneration::new(),
            modified: false,
        }
    }

    /// Un tampon chargé d'un texte, coupé sur les sauts de ligne.
    ///
    /// Les fins de ligne `\r\n` sont acceptées : le `\r` est retiré. Il n'est pas
    /// mémorisé, donc l'enregistrement écrit des fins de ligne Unix — c'est une limite
    /// connue, pas une conversion silencieuse qu'on ignorerait.
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let lines: Vec<String> = text
            .split('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
            .collect();
        Self::from_lines(lines)
    }

    /// Un tampon chargé de lignes.
    ///
    /// Une liste vide donne un tampon d'une ligne vide, pour tenir l'invariant.
    #[must_use]
    pub fn from_lines(lines: Vec<String>) -> Self {
        let mut buffer = Self::new();
        buffer.replace_lines(lines);
        buffer.modified = false;
        buffer
    }

    /// Les lignes du tampon.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// La ligne d'indice donné.
    #[must_use]
    pub fn line(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(String::as_str)
    }

    /// Le nombre de lignes — toujours au moins une.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// La position du curseur.
    #[must_use]
    pub const fn cursor(&self) -> AirTextPosition {
        self.cursor
    }

    /// La colonne d'affichage du curseur, glyphes larges compris.
    #[must_use]
    pub fn cursor_column(&self) -> usize {
        let Some(line) = self.line(self.cursor.line) else {
            return 0;
        };
        width_between(line, 0, self.cursor.offset)
    }

    /// Vrai si le contenu a changé depuis le dernier enregistrement.
    #[must_use]
    pub const fn is_modified(&self) -> bool {
        self.modified
    }

    /// La génération du contenu.
    #[must_use]
    pub const fn text_generation(&self) -> AirGeneration {
        self.text_generation
    }

    /// La génération de la position du curseur.
    #[must_use]
    pub const fn cursor_generation(&self) -> AirGeneration {
        self.cursor_generation
    }

    /// Le texte entier, lignes jointes par un saut de ligne.
    ///
    /// C'est ce que l'enregistrement écrit. Aucun saut final n'est ajouté : ce qui a été
    /// chargé est ce qui est rendu.
    #[must_use]
    pub fn to_text(&self) -> String {
        self.lines.join("\n")
    }

    /// Le décalage, dans une ligne, de la grappe qui couvre une colonne d'affichage.
    ///
    /// Une colonne au delà de la ligne rend la fin de la ligne. C'est ce qui place le
    /// curseur après un déplacement vertical, et après un clic.
    fn offset_for_column(line: &str, column: usize) -> usize {
        let mut offset: usize = 0;
        let mut walked: usize = 0;
        for cluster in graphemes(line) {
            if walked >= column {
                return offset;
            }
            walked = walked.saturating_add(grapheme_width(cluster));
            offset = offset.saturating_add(cluster.len());
        }
        offset
    }

    /// L'indice de la dernière ligne.
    fn last_line(&self) -> usize {
        self.lines.len().saturating_sub(1)
    }

    /// La longueur en octets d'une ligne, ou zéro.
    fn line_length(&self, index: usize) -> usize {
        self.line(index).map_or(0, str::len)
    }

    /// Remplace toutes les lignes et ramène le curseur au début.
    fn replace_lines(&mut self, lines: Vec<String>) {
        self.lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
        self.cursor = AirTextPosition::default();
        self.desired_column = 0;
        self.text_generation.advance();
        self.cursor_generation.advance();
        self.modified = true;
    }

    /// Note un déplacement du curseur et met la colonne désirée à jour.
    fn moved(&mut self, keep_desired_column: bool) {
        if !keep_desired_column {
            self.desired_column = self.cursor_column();
        }
        self.cursor_generation.advance();
    }

    /// Note une modification du contenu.
    fn edited(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        self.text_generation.advance();
        if !self.modified {
            self.modified = true;
            changes.record_once(AirTextChange::ModifiedChanged);
        }
    }

    /// Insère un caractère au curseur.
    fn insert_character(&mut self, character: char, changes: &mut AirChangeSet<AirTextChange>) {
        let offset = self.cursor.offset;
        let Some(line) = self.lines.get_mut(self.cursor.line) else {
            return;
        };
        if offset > line.len() || !line.is_char_boundary(offset) {
            // Inatteignable : l'invariant du curseur l'interdit. Insérer quand même
            // corromprait l'UTF-8, donc on refuse plutôt que de paniquer.
            return;
        }
        line.insert(offset, character);
        // Le curseur avance de la longueur insérée, pas d'une grappe : le caractère inséré
        // peut se joindre à la grappe qui précède — un accent combinant — et la frontière
        // suivante serait alors derrière lui.
        self.cursor.offset = offset.saturating_add(character.len_utf8());
        self.edited(changes);
        self.moved(false);
    }

    /// Coupe la ligne courante au curseur.
    fn insert_line_break(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        let offset = self.cursor.offset;
        let index = self.cursor.line;
        let Some(line) = self.lines.get_mut(index) else {
            return;
        };
        if offset > line.len() || !line.is_char_boundary(offset) {
            return;
        }
        let tail = line.split_off(offset);
        let next = index.saturating_add(1);
        self.lines.insert(next, tail);
        self.cursor = AirTextPosition::new(next, 0);
        self.edited(changes);
        changes.record_once(AirTextChange::LineCountChanged);
        self.moved(false);
    }

    /// Supprime la grappe qui précède le curseur, ou recolle la ligne précédente.
    fn delete_backward(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        if self.cursor.offset > 0 {
            let offset = self.cursor.offset;
            let Some(line) = self.lines.get_mut(self.cursor.line) else {
                return;
            };
            let start = previous_grapheme_boundary(line, offset);
            line.replace_range(start..offset, "");
            self.cursor.offset = start;
            self.edited(changes);
            self.moved(false);
            return;
        }
        let Some(previous) = self.cursor.line.checked_sub(1) else {
            // Début du texte : il n'y a rien à supprimer, et ce n'est pas une erreur.
            return;
        };
        let removed = self.lines.remove(self.cursor.line);
        let join_offset = self.line_length(previous);
        if let Some(line) = self.lines.get_mut(previous) {
            line.push_str(&removed);
        }
        self.cursor = AirTextPosition::new(previous, join_offset);
        self.edited(changes);
        changes.record_once(AirTextChange::LineCountChanged);
        self.moved(false);
    }

    /// Supprime la grappe qui suit le curseur, ou recolle la ligne suivante.
    fn delete_forward(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        let offset = self.cursor.offset;
        if offset < self.line_length(self.cursor.line) {
            let Some(line) = self.lines.get_mut(self.cursor.line) else {
                return;
            };
            let end = next_grapheme_boundary(line, offset);
            line.replace_range(offset..end, "");
            self.edited(changes);
            return;
        }
        let next = self.cursor.line.saturating_add(1);
        if next >= self.lines.len() {
            // Fin du texte : rien à recoller.
            return;
        }
        let removed = self.lines.remove(next);
        if let Some(line) = self.lines.get_mut(self.cursor.line) {
            line.push_str(&removed);
        }
        self.edited(changes);
        changes.record_once(AirTextChange::LineCountChanged);
    }

    /// Supprime la ligne courante.
    ///
    /// La dernière ligne n'est pas retirée mais vidée : l'invariant « au moins une ligne »
    /// vaut aussi pour la suppression.
    fn delete_line(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        if self.lines.len() <= 1 {
            let was_empty = self.line_length(0) == 0;
            if let Some(line) = self.lines.get_mut(0) {
                line.clear();
            }
            self.cursor = AirTextPosition::default();
            if !was_empty {
                self.edited(changes);
            }
            self.moved(false);
            return;
        }
        let index = self.cursor.line.min(self.last_line());
        self.lines.remove(index);
        let line = index.min(self.last_line());
        self.cursor = AirTextPosition::new(line, 0);
        self.edited(changes);
        changes.record_once(AirTextChange::LineCountChanged);
        self.moved(false);
    }

    /// Recule d'une grappe, en changeant de ligne s'il faut.
    fn move_left(&mut self) {
        if self.cursor.offset > 0 {
            if let Some(line) = self.line(self.cursor.line) {
                self.cursor.offset = previous_grapheme_boundary(line, self.cursor.offset);
            }
            self.moved(false);
            return;
        }
        let Some(previous) = self.cursor.line.checked_sub(1) else {
            return;
        };
        self.cursor = AirTextPosition::new(previous, self.line_length(previous));
        self.moved(false);
    }

    /// Avance d'une grappe, en changeant de ligne s'il faut.
    fn move_right(&mut self) {
        if self.cursor.offset < self.line_length(self.cursor.line) {
            if let Some(line) = self.line(self.cursor.line) {
                self.cursor.offset = next_grapheme_boundary(line, self.cursor.offset);
            }
            self.moved(false);
            return;
        }
        let next = self.cursor.line.saturating_add(1);
        if next >= self.lines.len() {
            return;
        }
        self.cursor = AirTextPosition::new(next, 0);
        self.moved(false);
    }

    /// Va à la ligne donnée, à la colonne désirée.
    fn move_to_line(&mut self, line: usize) {
        let line = line.min(self.last_line());
        let offset = match self.line(line) {
            Some(text) => Self::offset_for_column(text, self.desired_column),
            None => 0,
        };
        self.cursor = AirTextPosition::new(line, offset);
        self.moved(true);
    }

    /// Va au début du mot précédent, en changeant de ligne s'il faut.
    fn move_to_previous_word(&mut self) {
        if self.cursor.offset == 0 {
            let Some(previous) = self.cursor.line.checked_sub(1) else {
                return;
            };
            self.cursor = AirTextPosition::new(previous, self.line_length(previous));
            self.moved(false);
            return;
        }
        if let Some(line) = self.line(self.cursor.line) {
            self.cursor.offset = previous_word_start(line, self.cursor.offset);
        }
        self.moved(false);
    }

    /// Va au début du mot suivant, en changeant de ligne s'il faut.
    fn move_to_next_word(&mut self) {
        let length = self.line_length(self.cursor.line);
        if self.cursor.offset >= length {
            let next = self.cursor.line.saturating_add(1);
            if next >= self.lines.len() {
                return;
            }
            self.cursor = AirTextPosition::new(next, 0);
            self.moved(false);
            return;
        }
        if let Some(line) = self.line(self.cursor.line) {
            self.cursor.offset = next_word_start(line, self.cursor.offset);
        }
        self.moved(false);
    }

    /// Place le curseur à une position, bornée au texte et ramenée sur une frontière.
    ///
    /// Un décalage au milieu d'une grappe est **reculé** sur la frontière qui précède :
    /// une position venue d'un clic ne doit pas pouvoir casser l'invariant.
    fn move_to(&mut self, position: AirTextPosition) {
        let line = position.line.min(self.last_line());
        let offset = match self.line(line) {
            Some(text) => {
                let bounded = position.offset.min(text.len());
                if bounded == text.len() {
                    bounded
                } else {
                    let forward = next_grapheme_boundary(text, bounded);
                    if forward == bounded {
                        bounded
                    } else {
                        previous_grapheme_boundary(text, forward)
                    }
                }
            }
            None => 0,
        };
        self.cursor = AirTextPosition::new(line, offset);
        self.moved(false);
    }
}

impl AirModel for AirTextBuffer {
    type Action = AirTextAction;
    type Change = AirTextChange;

    fn apply(&mut self, action: Self::Action, changes: &mut AirChangeSet<Self::Change>) {
        match action {
            AirTextAction::InsertCharacter(character) => self.insert_character(character, changes),
            AirTextAction::InsertLineBreak => self.insert_line_break(changes),
            AirTextAction::DeleteBackward => self.delete_backward(changes),
            AirTextAction::DeleteForward => self.delete_forward(changes),
            AirTextAction::DeleteLine => self.delete_line(changes),
            AirTextAction::MoveLeft => self.move_left(),
            AirTextAction::MoveRight => self.move_right(),
            AirTextAction::MoveUp => {
                self.move_to_line(self.cursor.line.saturating_sub(1));
            }
            AirTextAction::MoveDown => {
                self.move_to_line(self.cursor.line.saturating_add(1));
            }
            AirTextAction::MoveToLineStart => {
                self.cursor.offset = 0;
                self.moved(false);
            }
            AirTextAction::MoveToLineEnd => {
                self.cursor.offset = self.line_length(self.cursor.line);
                self.moved(false);
            }
            AirTextAction::MoveToPreviousWord => self.move_to_previous_word(),
            AirTextAction::MoveToNextWord => self.move_to_next_word(),
            AirTextAction::MoveToDocumentStart => {
                self.cursor = AirTextPosition::default();
                self.moved(false);
            }
            AirTextAction::MoveToDocumentEnd => {
                let line = self.last_line();
                self.cursor = AirTextPosition::new(line, self.line_length(line));
                self.moved(false);
            }
            AirTextAction::MoveUpByLines(count) => {
                self.move_to_line(self.cursor.line.saturating_sub(count));
            }
            AirTextAction::MoveDownByLines(count) => {
                self.move_to_line(self.cursor.line.saturating_add(count));
            }
            AirTextAction::MoveTo(position) => self.move_to(position),
            AirTextAction::MoveToColumn { line, column } => {
                let line = line.min(self.last_line());
                let offset = match self.line(line) {
                    Some(text) => Self::offset_for_column(text, column),
                    None => 0,
                };
                self.cursor = AirTextPosition::new(line, offset);
                self.moved(false);
            }
            AirTextAction::Load(lines) => {
                self.replace_lines(lines);
                self.modified = false;
                changes.record_once(AirTextChange::ContentReplaced);
                changes.record_once(AirTextChange::LineCountChanged);
                changes.record_once(AirTextChange::ModifiedChanged);
            }
            AirTextAction::MarkSaved => {
                if self.modified {
                    self.modified = false;
                    changes.record_once(AirTextChange::ModifiedChanged);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AirTextAction, AirTextBuffer, AirTextChange, AirTextPosition};
    use crate::model::{AirChangeSet, AirModel};

    /// Applique une suite d'actions et rend le journal.
    fn apply_all(
        buffer: &mut AirTextBuffer,
        actions: Vec<AirTextAction>,
    ) -> AirChangeSet<AirTextChange> {
        let mut changes = AirChangeSet::new();
        for action in actions {
            buffer.apply(action, &mut changes);
        }
        changes
    }

    /// Insère un texte caractère par caractère, comme le ferait une frappe.
    fn type_text(buffer: &mut AirTextBuffer, text: &str) {
        apply_all(
            buffer,
            text.chars().map(AirTextAction::InsertCharacter).collect(),
        );
    }

    #[test]
    fn un_tampon_neuf_a_une_ligne_vide() {
        let buffer = AirTextBuffer::new();
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(buffer.line(0), Some(""));
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
        assert!(!buffer.is_modified());
    }

    #[test]
    fn un_tampon_sans_ligne_en_recoit_une() {
        // L'invariant vaut aussi pour un chargement vide.
        let buffer = AirTextBuffer::from_lines(Vec::new());
        assert_eq!(buffer.line_count(), 1);
    }

    #[test]
    fn un_texte_se_coupe_sur_les_sauts_de_ligne() {
        let buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        assert_eq!(buffer.lines(), ["une", "deux", "trois"]);
        assert!(!buffer.is_modified());
    }

    #[test]
    fn les_fins_de_ligne_windows_sont_acceptees() {
        let buffer = AirTextBuffer::from_text("une\r\ndeux\r\n");
        assert_eq!(buffer.lines(), ["une", "deux", ""]);
    }

    #[test]
    fn le_texte_rendu_est_celui_qui_a_ete_charge() {
        let buffer = AirTextBuffer::from_text("une\ndeux");
        assert_eq!(buffer.to_text(), "une\ndeux");
    }

    #[test]
    fn une_frappe_marque_le_tampon_modifie_une_seule_fois() {
        let mut buffer = AirTextBuffer::new();
        let mut changes = AirChangeSet::new();
        buffer.apply(AirTextAction::InsertCharacter('a'), &mut changes);
        buffer.apply(AirTextAction::InsertCharacter('b'), &mut changes);
        assert_eq!(buffer.line(0), Some("ab"));
        assert!(buffer.is_modified());
        // Le drapeau ne bascule qu'une fois : le journal ne porte pas la frappe, il porte
        // le changement d'état.
        assert_eq!(changes.as_slice(), [AirTextChange::ModifiedChanged]);
    }

    #[test]
    fn une_frappe_fait_avancer_les_deux_generations() {
        let mut buffer = AirTextBuffer::new();
        let text = buffer.text_generation();
        let cursor = buffer.cursor_generation();
        type_text(&mut buffer, "a");
        assert_ne!(buffer.text_generation(), text);
        assert_ne!(buffer.cursor_generation(), cursor);
    }

    #[test]
    fn un_deplacement_ne_touche_pas_la_generation_du_texte() {
        // C'est la raison d'être de deux compteurs : une vue qui ne dépend que du texte ne
        // se redessine pas parce que le curseur a bougé.
        let mut buffer = AirTextBuffer::from_text("abc");
        let text = buffer.text_generation();
        apply_all(&mut buffer, vec![AirTextAction::MoveRight]);
        assert_eq!(buffer.text_generation(), text);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 1));
    }

    #[test]
    fn une_grappe_s_efface_d_une_seule_touche() {
        // `e` plus accent combinant : deux caractères, une grappe, une touche.
        let mut buffer = AirTextBuffer::from_text("cafe\u{301}");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveToLineEnd, AirTextAction::DeleteBackward],
        );
        assert_eq!(buffer.line(0), Some("caf"));
    }

    #[test]
    fn une_suppression_avant_efface_la_grappe_suivante() {
        let mut buffer = AirTextBuffer::from_text("e\u{301}a");
        apply_all(&mut buffer, vec![AirTextAction::DeleteForward]);
        assert_eq!(buffer.line(0), Some("a"));
    }

    #[test]
    fn un_saut_de_ligne_coupe_la_ligne_au_curseur() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        let changes = apply_all(
            &mut buffer,
            vec![
                AirTextAction::MoveRight,
                AirTextAction::MoveRight,
                AirTextAction::InsertLineBreak,
            ],
        );
        assert_eq!(buffer.lines(), ["ab", "cd"]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 0));
        assert!(changes.contains(&AirTextChange::LineCountChanged));
    }

    #[test]
    fn une_suppression_en_tete_de_ligne_recolle_la_precedente() {
        let mut buffer = AirTextBuffer::from_text("ab\ncd");
        let changes = apply_all(
            &mut buffer,
            vec![AirTextAction::MoveDown, AirTextAction::DeleteBackward],
        );
        assert_eq!(buffer.lines(), ["abcd"]);
        // Le curseur est à la soudure, pas au début.
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 2));
        assert!(changes.contains(&AirTextChange::LineCountChanged));
    }

    #[test]
    fn une_suppression_en_fin_de_ligne_recolle_la_suivante() {
        let mut buffer = AirTextBuffer::from_text("ab\ncd");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveToLineEnd, AirTextAction::DeleteForward],
        );
        assert_eq!(buffer.lines(), ["abcd"]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 2));
    }

    #[test]
    fn une_suppression_au_tout_debut_ne_fait_rien() {
        let mut buffer = AirTextBuffer::from_text("ab");
        let changes = apply_all(&mut buffer, vec![AirTextAction::DeleteBackward]);
        assert_eq!(buffer.lines(), ["ab"]);
        assert!(!buffer.is_modified());
        assert!(changes.is_empty());
    }

    #[test]
    fn une_suppression_a_la_toute_fin_ne_fait_rien() {
        let mut buffer = AirTextBuffer::from_text("ab");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::MoveToDocumentEnd,
                AirTextAction::DeleteForward,
            ],
        );
        assert_eq!(buffer.lines(), ["ab"]);
        assert!(!buffer.is_modified());
    }

    #[test]
    fn supprimer_une_ligne_la_retire() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        let changes = apply_all(
            &mut buffer,
            vec![AirTextAction::MoveDown, AirTextAction::DeleteLine],
        );
        assert_eq!(buffer.lines(), ["une", "trois"]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 0));
        assert!(changes.contains(&AirTextChange::LineCountChanged));
    }

    #[test]
    fn supprimer_la_derniere_ligne_la_vide_sans_la_retirer() {
        let mut buffer = AirTextBuffer::from_text("seule");
        apply_all(&mut buffer, vec![AirTextAction::DeleteLine]);
        assert_eq!(buffer.lines(), [""]);
        assert!(buffer.is_modified());
    }

    #[test]
    fn supprimer_la_derniere_ligne_deja_vide_ne_modifie_rien() {
        let mut buffer = AirTextBuffer::new();
        apply_all(&mut buffer, vec![AirTextAction::DeleteLine]);
        assert_eq!(buffer.lines(), [""]);
        assert!(!buffer.is_modified());
    }

    #[test]
    fn supprimer_la_ligne_du_bas_remonte_le_curseur() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveToDocumentEnd, AirTextAction::DeleteLine],
        );
        assert_eq!(buffer.lines(), ["une"]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
    }

    #[test]
    fn la_fleche_gauche_remonte_a_la_fin_de_la_ligne_precedente() {
        let mut buffer = AirTextBuffer::from_text("abc\nd");
        apply_all(&mut buffer, vec![AirTextAction::MoveDown]);
        apply_all(&mut buffer, vec![AirTextAction::MoveLeft]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 3));
    }

    #[test]
    fn la_fleche_droite_descend_au_debut_de_la_ligne_suivante() {
        let mut buffer = AirTextBuffer::from_text("a\nb");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveToLineEnd, AirTextAction::MoveRight],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 0));
    }

    #[test]
    fn les_fleches_aux_bords_du_texte_ne_font_rien() {
        let mut buffer = AirTextBuffer::from_text("a");
        apply_all(&mut buffer, vec![AirTextAction::MoveLeft]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveToDocumentEnd, AirTextAction::MoveRight],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 1));
    }

    #[test]
    fn un_aller_retour_vertical_retrouve_la_colonne() {
        // C'est la colonne désirée : sans elle, la ligne vide du milieu ramènerait le
        // curseur en colonne zéro et l'aller-retour ne serait pas neutre.
        let mut buffer = AirTextBuffer::from_text("abcdef\n\nabcdef");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::MoveToLineEnd,
                AirTextAction::MoveDown,
                AirTextAction::MoveDown,
            ],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(2, 6));
    }

    #[test]
    fn un_deplacement_horizontal_remet_la_colonne_desiree() {
        let mut buffer = AirTextBuffer::from_text("abcdef\nab");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::MoveToLineEnd,
                AirTextAction::MoveDown,
                // La ligne du bas est courte : le curseur s'arrête à sa fin.
                AirTextAction::MoveLeft,
                AirTextAction::MoveUp,
            ],
        );
        // La colonne désirée est devenue 1, pas 6.
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 1));
    }

    #[test]
    fn un_deplacement_vertical_tient_compte_des_glyphes_larges() {
        // L'idéogramme occupe deux colonnes : la colonne désirée 2 tombe après lui.
        let mut buffer = AirTextBuffer::from_text("\u{6F22}ab\nxyz");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveRight, AirTextAction::MoveDown],
        );
        assert_eq!(buffer.cursor_column(), 2);
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 2));
    }

    #[test]
    fn les_deplacements_par_page_bornent_au_texte() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        apply_all(&mut buffer, vec![AirTextAction::MoveDownByLines(100)]);
        assert_eq!(buffer.cursor().line, 2);
        apply_all(&mut buffer, vec![AirTextAction::MoveUpByLines(100)]);
        assert_eq!(buffer.cursor().line, 0);
    }

    #[test]
    fn le_deplacement_par_mot_suit_l_annexe_29() {
        let mut buffer = AirTextBuffer::from_text("le chat, noir");
        apply_all(&mut buffer, vec![AirTextAction::MoveToNextWord]);
        assert_eq!(buffer.cursor().offset, 3);
        apply_all(&mut buffer, vec![AirTextAction::MoveToNextWord]);
        assert_eq!(buffer.cursor().offset, 9);
        apply_all(&mut buffer, vec![AirTextAction::MoveToPreviousWord]);
        assert_eq!(buffer.cursor().offset, 3);
    }

    #[test]
    fn le_mot_suivant_passe_a_la_ligne_quand_la_ligne_est_finie() {
        let mut buffer = AirTextBuffer::from_text("un\ndeux");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveToLineEnd, AirTextAction::MoveToNextWord],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 0));
    }

    #[test]
    fn le_mot_precedent_remonte_a_la_fin_de_la_ligne_precedente() {
        let mut buffer = AirTextBuffer::from_text("un\ndeux");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveDown, AirTextAction::MoveToPreviousWord],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 2));
    }

    #[test]
    fn le_mot_aux_bords_du_texte_ne_bouge_pas() {
        let mut buffer = AirTextBuffer::from_text("un");
        apply_all(&mut buffer, vec![AirTextAction::MoveToPreviousWord]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::MoveToDocumentEnd,
                AirTextAction::MoveToNextWord,
            ],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 2));
    }

    #[test]
    fn une_position_au_milieu_d_une_grappe_est_ramenee_sur_sa_frontiere() {
        // Le clic d'une souris, ou une position calculée à côté, ne doit pas pouvoir
        // casser l'invariant du curseur.
        let mut buffer = AirTextBuffer::from_text("e\u{301}a");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveTo(AirTextPosition::new(0, 2))],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
    }

    #[test]
    fn un_clic_place_le_curseur_a_la_colonne_d_affichage() {
        // L'idéogramme occupe deux colonnes : la colonne 2 tombe sur le `a` qui le suit.
        let mut buffer = AirTextBuffer::from_text("x\n\u{6F22}ab");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveToColumn { line: 1, column: 2 }],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 3));
    }

    #[test]
    fn un_clic_au_dela_d_une_ligne_place_le_curseur_a_sa_fin() {
        let mut buffer = AirTextBuffer::from_text("ab");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveToColumn {
                line: 99,
                column: 99,
            }],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 2));
    }

    #[test]
    fn une_position_hors_du_texte_est_bornee() {
        let mut buffer = AirTextBuffer::from_text("ab\ncd");
        apply_all(
            &mut buffer,
            vec![AirTextAction::MoveTo(AirTextPosition::new(99, 99))],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 2));
    }

    #[test]
    fn un_chargement_remplace_tout_et_remet_le_drapeau() {
        let mut buffer = AirTextBuffer::from_text("ancien");
        type_text(&mut buffer, "x");
        assert!(buffer.is_modified());
        let changes = apply_all(
            &mut buffer,
            vec![AirTextAction::Load(vec![
                "neuf".to_owned(),
                "deux".to_owned(),
            ])],
        );
        assert_eq!(buffer.lines(), ["neuf", "deux"]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
        assert!(!buffer.is_modified());
        assert!(changes.contains(&AirTextChange::ContentReplaced));
        assert!(changes.contains(&AirTextChange::LineCountChanged));
    }

    #[test]
    fn un_enregistrement_eteint_le_drapeau_une_seule_fois() {
        let mut buffer = AirTextBuffer::new();
        type_text(&mut buffer, "a");
        let changes = apply_all(
            &mut buffer,
            vec![AirTextAction::MarkSaved, AirTextAction::MarkSaved],
        );
        assert!(!buffer.is_modified());
        assert_eq!(changes.as_slice(), [AirTextChange::ModifiedChanged]);
    }

    #[test]
    fn le_tampon_par_defaut_est_le_tampon_neuf() {
        let buffer = AirTextBuffer::default();
        assert_eq!(buffer.line_count(), 1);
    }

    #[test]
    fn la_colonne_du_curseur_compte_les_colonnes_et_non_les_octets() {
        let mut buffer = AirTextBuffer::from_text("\u{6F22}a");
        apply_all(&mut buffer, vec![AirTextAction::MoveToLineEnd]);
        // Trois octets pour l'idéogramme, deux colonnes, plus une pour le `a`.
        assert_eq!(buffer.cursor().offset, 4);
        assert_eq!(buffer.cursor_column(), 3);
    }

    #[test]
    fn un_tampon_se_clone_sans_partager_son_etat() {
        let mut buffer = AirTextBuffer::from_text("ab");
        let copy = buffer.clone();
        type_text(&mut buffer, "c");
        assert_eq!(copy.line(0), Some("ab"));
    }
}
