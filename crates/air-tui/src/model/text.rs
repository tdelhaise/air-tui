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
//!
//! # La sélection est une **ancre**, pas deux bornes
//!
//! Faite au jalon M7d. Le tampon garde l'ancre, et la sélection est l'intervalle entre
//! elle et le curseur, *dans l'ordre où ils se trouvent*. Garder deux bornes déjà
//! ordonnées ferait perdre le sens du geste : sélectionner trois caractères vers la droite
//! puis remonter vers la gauche doit **réduire** la sélection, et non en commencer une
//! autre.
//!
//! La sélection a sa propre génération, distincte de celle du curseur : l'abandonner ne
//! déplace pas le curseur, et la vue doit pourtant repeindre. Les confondre laisserait une
//! surbrillance orpheline à l'écran.
//!
//! Un déplacement porte son mode — [`AirTextAction::move_by`] efface la sélection,
//! [`AirTextAction::select_by`] l'étend —, et les deux partagent le même
//! [`AirTextMotion`]. Les décliner en deux familles de variantes aurait obligé à ajouter
//! chaque déplacement futur à deux endroits.

use crate::model::history::{AirEditHistory, AirTextEdit, position_after};
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

/// Un déplacement du curseur, indépendamment de ce qu'il fait à la sélection.
///
/// Séparé de l'action pour que **le même déplacement serve aux deux modes** : avec
/// `Maj`, il étend la sélection ; sans, il l'efface. Les décliner en deux familles de
/// variantes — `MoveLeft` et `SelectLeft` — obligerait à ajouter chaque déplacement futur
/// à deux endroits, et une règle écrite deux fois finit par ne plus l'être qu'une (leçon
/// du jalon M7c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirTextMotion {
    /// Recule d'une grappe, en passant à la ligne précédente s'il faut.
    Left,
    /// Avance d'une grappe, en passant à la ligne suivante s'il faut.
    Right,
    /// Monte d'une ligne, à la colonne désirée.
    Up,
    /// Descend d'une ligne, à la colonne désirée.
    Down,
    /// Va au début de la ligne.
    LineStart,
    /// Va à la fin de la ligne.
    LineEnd,
    /// Va au début du mot précédent.
    PreviousWord,
    /// Va au début du mot suivant.
    NextWord,
    /// Va au tout début du texte.
    DocumentStart,
    /// Va à la toute fin du texte.
    DocumentEnd,
    /// Monte d'autant de lignes.
    UpByLines(usize),
    /// Descend d'autant de lignes.
    DownByLines(usize),
    /// Va à une position donnée, bornée au texte.
    To(AirTextPosition),
    /// Va sur une ligne, à une colonne **d'affichage**.
    ///
    /// C'est ce qu'un clic produit : une souris donne une colonne, jamais un décalage en
    /// octets. La résolution appartient au modèle, seul à connaître le texte de la ligne —
    /// une vue qui la ferait aurait besoin de la ligne entière alors qu'elle n'en reflète
    /// que la partie visible.
    ToColumn {
        /// L'indice de la ligne, borné au texte.
        line: usize,
        /// La colonne d'affichage, bornée à la fin de la ligne.
        column: usize,
    },
}

/// Ce qu'on peut demander au tampon de texte.
///
/// Toute modification et tout déplacement passent par là : l'ensemble de ce qui peut
/// arriver au texte se lit d'un coup d'œil sur cette énumération (ADR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirTextAction {
    /// Insère un caractère au curseur, en remplaçant la sélection s'il y en a une.
    InsertCharacter(char),
    /// Insère un texte au curseur, en remplaçant la sélection s'il y en a une.
    ///
    /// C'est par là que passent le collage encadré du terminal et le presse-papiers
    /// interne : un collage n'est pas une suite de frappes, et le traiter comme telle
    /// ferait avancer la génération une fois par caractère.
    InsertText(String),
    /// Coupe la ligne au curseur, en remplaçant la sélection s'il y en a une.
    InsertLineBreak,
    /// Supprime la sélection, ou la grappe qui précède le curseur, ou recolle la ligne
    /// précédente.
    DeleteBackward,
    /// Supprime la sélection, ou la grappe qui suit le curseur, ou recolle la ligne
    /// suivante.
    DeleteForward,
    /// Supprime la ligne courante entière.
    DeleteLine,
    /// Déplace le curseur, en étendant ou en effaçant la sélection.
    ///
    /// Voir [`AirTextAction::move_by`] et [`AirTextAction::select_by`], qui nomment les
    /// deux valeurs du drapeau — un booléen anonyme au site d'appel ne dirait pas laquelle
    /// étend.
    Move {
        /// Le déplacement demandé.
        motion: AirTextMotion,
        /// Vrai pour étendre la sélection depuis son ancre, faux pour l'effacer.
        extend: bool,
    },
    /// Sélectionne tout le texte, et porte le curseur à la fin.
    SelectAll,
    /// Sélectionne le segment de texte qui couvre le curseur.
    ///
    /// Le **segment** et non « le mot », parce que c'est ce que l'UAX #29 découpe : un mot,
    /// mais aussi une suite d'espaces ou un signe de ponctuation. Un double clic sur un
    /// espace doit sélectionner les espaces, et non rester sans effet visible.
    SelectSegment,
    /// Sélectionne la ligne entière où se trouve le curseur, saut de ligne compris.
    SelectLine,
    /// Abandonne la sélection sans toucher au texte ni au curseur.
    ClearSelection,
    /// Supprime le texte sélectionné, s'il y en a.
    DeleteSelection,
    /// Remplace tout le contenu — c'est l'ouverture d'un fichier.
    Load(Vec<String>),
    /// Déclare le contenu enregistré.
    MarkSaved,
    /// Annule le dernier geste d'édition, et replace le curseur où il était.
    ///
    /// Un **geste**, et non une frappe : taper `abc` s'annule d'un coup, un collage aussi.
    /// La règle de fusion est dans l'ADR-014, et ses bords sont écrits parce qu'ils sont
    /// arbitraires.
    Undo,
    /// Rétablit le dernier geste annulé.
    ///
    /// Toute édition nouvelle vide la pile de rétablissement : rejouer un remplacement sur
    /// un texte qui n'est plus celui qu'il décrivait n'aurait aucun sens.
    Redo,
}

impl AirTextAction {
    /// Un déplacement qui **efface** la sélection : une flèche nue.
    #[must_use]
    pub const fn move_by(motion: AirTextMotion) -> Self {
        Self::Move {
            motion,
            extend: false,
        }
    }

    /// Un déplacement qui **étend** la sélection : `Maj` et une flèche.
    #[must_use]
    pub const fn select_by(motion: AirTextMotion) -> Self {
        Self::Move {
            motion,
            extend: true,
        }
    }
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
    /// L'autre borne de la sélection, posée au début de celle-ci.
    ///
    /// La sélection est l'intervalle entre l'ancre et le curseur, dans l'ordre où ils se
    /// trouvent. Garder l'ancre plutôt que deux bornes ordonnées est ce qui permet de
    /// **revenir en arrière** : sélectionner trois caractères à droite puis remonter à
    /// gauche doit réduire la sélection, pas en commencer une autre.
    anchor: Option<AirTextPosition>,
    /// La colonne d'affichage que les déplacements verticaux cherchent à retrouver.
    desired_column: usize,
    /// La génération du contenu.
    text_generation: AirGeneration,
    /// La génération de la position du curseur.
    cursor_generation: AirGeneration,
    /// La génération de la sélection.
    ///
    /// Distincte de celle du curseur : abandonner une sélection ne déplace pas le curseur,
    /// et la vue doit pourtant repeindre. Les confondre laisserait une surbrillance à
    /// l'écran après son abandon.
    selection_generation: AirGeneration,
    /// Vrai si le contenu a changé depuis le dernier enregistrement.
    ///
    /// Dérivé de l'historique plutôt que posé à la main : annuler jusqu'au point
    /// d'enregistrement doit l'éteindre, et un drapeau que seule une édition allume y
    /// resterait allumé (ADR-014).
    modified: bool,
    /// Le journal d'annulation.
    history: AirEditHistory,
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
            anchor: None,
            desired_column: 0,
            text_generation: AirGeneration::new(),
            cursor_generation: AirGeneration::new(),
            selection_generation: AirGeneration::new(),
            modified: false,
            history: AirEditHistory::new(),
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

    /// La génération de la sélection.
    #[must_use]
    pub const fn selection_generation(&self) -> AirGeneration {
        self.selection_generation
    }

    /// Les bornes de la sélection, **ordonnées**, ou `None` s'il n'y en a pas.
    ///
    /// Une sélection vide — l'ancre posée exactement sur le curseur — rend `None` : il n'y
    /// a rien à peindre et rien à supprimer, et la distinguer obligerait chaque lecteur à
    /// traiter un cas sans effet.
    #[must_use]
    pub fn selection(&self) -> Option<(AirTextPosition, AirTextPosition)> {
        let anchor = self.anchor?;
        if anchor == self.cursor {
            return None;
        }
        if anchor <= self.cursor {
            Some((anchor, self.cursor))
        } else {
            Some((self.cursor, anchor))
        }
    }

    /// Vrai s'il y a une sélection non vide.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    /// Le texte sélectionné, ou `None` s'il n'y a pas de sélection.
    ///
    /// Les lignes sont jointes par un saut de ligne, comme [`AirTextBuffer::to_text`] :
    /// c'est ce qu'un presse-papiers doit recevoir pour que le coller rende le découpage.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection()?;
        if start.line == end.line {
            let line = self.line(start.line)?;
            return line.get(start.offset..end.offset).map(str::to_owned);
        }
        let mut collected = String::new();
        if let Some(first) = self
            .line(start.line)
            .and_then(|line| line.get(start.offset..))
        {
            collected.push_str(first);
        }
        for index in start.line.saturating_add(1)..end.line {
            collected.push('\n');
            if let Some(line) = self.line(index) {
                collected.push_str(line);
            }
        }
        collected.push('\n');
        if let Some(last) = self.line(end.line).and_then(|line| line.get(..end.offset)) {
            collected.push_str(last);
        }
        Some(collected)
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
        self.anchor = None;
        self.desired_column = 0;
        self.text_generation.advance();
        self.cursor_generation.advance();
        self.selection_generation.advance();
        self.modified = true;
    }

    /// Pose l'ancre sur le curseur si elle n'y est pas déjà.
    fn begin_selection(&mut self) {
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
            self.selection_generation.advance();
        }
    }

    /// Abandonne la sélection, et dit si quelque chose a changé.
    fn clear_selection(&mut self) -> bool {
        if self.anchor.take().is_some() {
            self.selection_generation.advance();
            return true;
        }
        false
    }

    /// Le texte entre deux positions, sauts de ligne compris.
    ///
    /// Les deux positions sont supposées ordonnées et valides ; une position hors du
    /// tampon rend ce qui s'y trouve, c'est-à-dire rien, plutôt que de paniquer.
    fn text_between(&self, start: AirTextPosition, end: AirTextPosition) -> String {
        if start.line == end.line {
            return self
                .line(start.line)
                .and_then(|line| line.get(start.offset..end.offset))
                .unwrap_or_default()
                .to_owned();
        }
        let mut collected = String::new();
        if let Some(first) = self
            .line(start.line)
            .and_then(|line| line.get(start.offset..))
        {
            collected.push_str(first);
        }
        for index in start.line.saturating_add(1)..end.line {
            collected.push('\n');
            if let Some(line) = self.line(index) {
                collected.push_str(line);
            }
        }
        collected.push('\n');
        if let Some(last) = self.line(end.line).and_then(|line| line.get(..end.offset)) {
            collected.push_str(last);
        }
        collected
    }

    /// Remplace le texte entre deux positions, et consigne de quoi faire l'inverse.
    ///
    /// **C'est la seule mutation du texte de ce module** (ADR-014). Les sept opérations
    /// d'édition — frappe, collage, saut de ligne, deux suppressions de grappe,
    /// suppression de ligne, suppression de sélection — s'y ramènent chacune à un triplet
    /// `(start, end, inserted)`. Sept chirurgies de lignes écrites à la main seraient sept
    /// inverses à tenir d'accord ; il n'y en a qu'un.
    ///
    /// Rend la position qui suit le texte inséré, c'est-à-dire où le curseur doit aller.
    fn splice(
        &mut self,
        start: AirTextPosition,
        end: AirTextPosition,
        inserted: &str,
        changes: &mut AirChangeSet<AirTextChange>,
    ) -> AirTextPosition {
        let removed = self.text_between(start, end);
        if removed.is_empty() && inserted.is_empty() {
            // Rien à faire, et surtout rien à journaliser : un pas d'annulation vide se
            // ferait annuler sans que l'écran bouge.
            return start;
        }
        let entry = AirTextEdit {
            start,
            removed,
            inserted: inserted.to_owned(),
            cursor_before: self.cursor,
            anchor_before: self.anchor,
        };
        let reached = self.rewrite(start, end, inserted, changes);
        self.history.record(entry);
        self.edited(changes);
        reached
    }

    /// La chirurgie de lignes elle-même, sans journal ni générations.
    ///
    /// Séparée de [`Self::splice`] parce que l'annulation l'emploie **sans** consigner une
    /// entrée de plus : annuler n'est pas une édition nouvelle, c'est le même
    /// remplacement joué à l'envers.
    fn rewrite(
        &mut self,
        start: AirTextPosition,
        end: AirTextPosition,
        inserted: &str,
        changes: &mut AirChangeSet<AirTextChange>,
    ) -> AirTextPosition {
        let before = self.lines.len();
        let tail = self
            .line(end.line)
            .and_then(|line| line.get(end.offset..))
            .unwrap_or_default()
            .to_owned();
        let Some(line) = self.lines.get_mut(start.line) else {
            // Inatteignable : une position vient toujours du tampon lui-même.
            return start;
        };
        if start.offset > line.len() || !line.is_char_boundary(start.offset) {
            // Inatteignable : l'invariant du curseur l'interdit. Écrire quand même
            // corromprait l'UTF-8, donc on refuse plutôt que de paniquer.
            return start;
        }
        line.truncate(start.offset);
        if end.line > start.line {
            self.lines.drain(start.line.saturating_add(1)..=end.line);
        }
        let mut pieces = inserted.split('\n');
        if let Some(first) = pieces.next()
            && let Some(line) = self.lines.get_mut(start.line)
        {
            line.push_str(first);
        }
        let mut at = start.line;
        for piece in pieces {
            at = at.saturating_add(1);
            self.lines.insert(at, piece.to_owned());
        }
        let reached = self.line_length(at);
        if let Some(line) = self.lines.get_mut(at) {
            line.push_str(&tail);
        }
        if self.lines.len() != before {
            changes.record_once(AirTextChange::LineCountChanged);
        }
        AirTextPosition::new(at, reached)
    }

    /// Supprime le texte sélectionné et place le curseur à son début.
    ///
    /// Rend vrai si quelque chose a été supprimé. C'est l'opération qu'une frappe, un
    /// collage et un retour arrière partagent : taper sur une sélection la remplace, et
    /// trois chemins qui l'écriraient chacun finiraient par diverger.
    fn delete_selection(&mut self, changes: &mut AirChangeSet<AirTextChange>) -> bool {
        let Some((start, end)) = self.selection() else {
            // Pas de sélection : il reste à abandonner une ancre posée mais vide.
            self.clear_selection();
            return false;
        };
        self.history.break_run();
        self.cursor = self.splice(start, end, "", changes);
        self.clear_selection();
        self.moved(false);
        true
    }

    /// Écrit un texte au curseur, en remplaçant la sélection s'il y en a une.
    ///
    /// **Un seul remplacement**, et c'est ce qui fait que taper sur une sélection est un
    /// seul pas d'annulation. Supprimer puis insérer en ferait deux, et l'utilisateur
    /// devrait annuler deux fois un geste qu'il n'a fait qu'une — c'est le défaut que les
    /// essais de l'annulation ont rendu visible.
    fn write_over_selection(&mut self, inserted: &str, changes: &mut AirChangeSet<AirTextChange>) {
        let (start, end) = self.selection().unwrap_or((self.cursor, self.cursor));
        self.cursor = self.splice(start, end, inserted, changes);
        self.clear_selection();
        self.moved(false);
    }

    /// Insère un texte au curseur, sauts de ligne compris.
    fn insert_text(&mut self, inserted: &str, changes: &mut AirChangeSet<AirTextChange>) {
        // Un collage est **un** pas d'annulation, pas un par caractère : il coupe donc le
        // geste de frappe en cours, avant comme après.
        self.history.break_run();
        self.write_over_selection(inserted, changes);
        self.history.break_run();
    }

    /// Sélectionne le segment qui couvre le curseur.
    fn select_segment(&mut self) {
        let line = self.cursor.line;
        let Some(text) = self.line(line) else {
            return;
        };
        let (start, end) = crate::unicode::segment_at(text, self.cursor.offset);
        self.anchor = Some(AirTextPosition::new(line, start));
        self.cursor = AirTextPosition::new(line, end);
        self.selection_generation.advance();
        self.moved(false);
    }

    /// Sélectionne la ligne du curseur, saut de ligne compris.
    ///
    /// L'ancre se pose au début de la ligne et le curseur au début de la **suivante**
    /// quand il y en a une : c'est ce qui fait qu'un triple clic suivi d'une suppression
    /// retire la ligne entière, et non une ligne vide laissée derrière. Sur la dernière
    /// ligne, il n'y a pas de suivante, et le curseur va à sa fin.
    fn select_line(&mut self) {
        let line = self.cursor.line;
        self.anchor = Some(AirTextPosition::new(line, 0));
        let next = line.saturating_add(1);
        self.cursor = if next <= self.last_line() {
            AirTextPosition::new(next, 0)
        } else {
            AirTextPosition::new(line, self.line_length(line))
        };
        self.selection_generation.advance();
        self.moved(false);
    }

    /// Note un déplacement du curseur et met la colonne désirée à jour.
    fn moved(&mut self, keep_desired_column: bool) {
        if !keep_desired_column {
            self.desired_column = self.cursor_column();
        }
        self.cursor_generation.advance();
    }

    /// Note une modification du contenu, et relit le drapeau de modification.
    ///
    /// Le drapeau n'est **pas** posé à vrai : il est dérivé de l'historique, parce
    /// qu'annuler jusqu'au point d'enregistrement doit l'éteindre (ADR-014). Un drapeau
    /// que seule une édition allume y resterait allumé sur un texte revenu à son état
    /// enregistré.
    fn edited(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        self.text_generation.advance();
        self.refresh_modified(changes);
    }

    /// Relit le drapeau de modification depuis l'historique, et consigne s'il bascule.
    fn refresh_modified(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        let modified = !self.history.is_at_saved_point();
        if modified != self.modified {
            self.modified = modified;
            changes.record_once(AirTextChange::ModifiedChanged);
        }
    }

    /// Vrai s'il y a un geste à annuler.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Vrai s'il y a un geste à rétablir.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Le nombre de gestes annulables.
    ///
    /// Exposé parce que c'est ce qui rend la **fusion** observable : taper trois lettres
    /// doit donner un pas, pas trois.
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.history.depth()
    }

    /// Annule le dernier geste, et rend vrai si quelque chose a été annulé.
    fn undo(&mut self, changes: &mut AirChangeSet<AirTextChange>) -> bool {
        let Some(entry) = self.history.undo() else {
            return false;
        };
        let end = position_after(entry.start, &entry.inserted);
        self.rewrite(entry.start, end, &entry.removed, changes);
        self.restore(entry.cursor_before, entry.anchor_before, changes);
        true
    }

    /// Rétablit le dernier geste annulé, et rend vrai si quelque chose a été rétabli.
    fn redo(&mut self, changes: &mut AirChangeSet<AirTextChange>) -> bool {
        let Some(entry) = self.history.redo() else {
            return false;
        };
        let end = position_after(entry.start, &entry.removed);
        let reached = self.rewrite(entry.start, end, &entry.inserted, changes);
        // Rétablir replace le curseur **après** le texte rejoué, et non là où il était
        // avant le geste : c'est là que l'utilisateur l'attend, comme après la frappe.
        self.restore(reached, None, changes);
        true
    }

    /// Replace le curseur et l'ancre, et relit les drapeaux.
    ///
    /// La génération de sélection avance sans condition : annuler peut faire réapparaître
    /// une sélection comme en faire disparaître une, et la vue doit repeindre dans les
    /// deux cas.
    fn restore(
        &mut self,
        cursor: AirTextPosition,
        anchor: Option<AirTextPosition>,
        changes: &mut AirChangeSet<AirTextChange>,
    ) {
        self.cursor = self.clamped(cursor);
        self.anchor = anchor.map(|position| self.clamped(position));
        self.selection_generation.advance();
        self.text_generation.advance();
        self.refresh_modified(changes);
        self.moved(false);
    }

    /// Une position ramenée dans le tampon.
    ///
    /// Une position journalisée décrit le tampon d'avant le remplacement : après l'avoir
    /// joué à l'envers, elle y est de nouveau valide — sauf si la borne du journal a
    /// oublié une entrée intermédiaire, et c'est le seul cas où cette borne se voit.
    fn clamped(&self, position: AirTextPosition) -> AirTextPosition {
        let line = position.line.min(self.last_line());
        let length = self.line_length(line);
        let offset = if position.offset <= length
            && self
                .line(line)
                .is_some_and(|text| text.is_char_boundary(position.offset))
        {
            position.offset
        } else {
            length
        };
        AirTextPosition::new(line, offset)
    }

    /// Insère un caractère au curseur, en remplaçant la sélection s'il y en a une.
    ///
    /// Le curseur avance de la longueur insérée, pas d'une grappe : le caractère inséré
    /// peut se joindre à la grappe qui précède — un accent combinant — et la frontière
    /// suivante serait alors derrière lui. C'est [`Self::splice`] qui le place.
    fn insert_character(&mut self, character: char, changes: &mut AirChangeSet<AirTextChange>) {
        let mut text = String::new();
        text.push(character);
        self.write_over_selection(&text, changes);
    }

    /// Coupe la ligne courante au curseur.
    ///
    /// Une touche `Entrée` **coupe le geste** d'annulation : un paragraphe est une
    /// frontière, et fondre la ligne suivante dans la précédente ferait annuler les deux
    /// d'un coup (ADR-014).
    fn insert_line_break(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        self.history.break_run();
        self.write_over_selection("\n", changes);
        self.history.break_run();
    }

    /// Supprime la grappe qui précède le curseur, ou recolle la ligne précédente.
    fn delete_backward(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        let end = self.cursor;
        let start = if end.offset > 0 {
            let offset = self.line(end.line).map_or(end.offset, |line| {
                previous_grapheme_boundary(line, end.offset)
            });
            AirTextPosition::new(end.line, offset)
        } else {
            let Some(previous) = end.line.checked_sub(1) else {
                // Début du texte : il n'y a rien à supprimer, et ce n'est pas une erreur.
                return;
            };
            AirTextPosition::new(previous, self.line_length(previous))
        };
        self.cursor = self.splice(start, end, "", changes);
        self.moved(false);
    }

    /// Supprime la grappe qui suit le curseur, ou recolle la ligne suivante.
    ///
    /// Elle **coupe le geste** : une suite de `Suppr` ne se fond pas, parce que le curseur
    /// ne bouge pas et que deux entrées voisines y commenceraient au même endroit — la
    /// règle de fusion de l'ADR-014 ne saurait pas les distinguer d'une insertion.
    fn delete_forward(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        let start = self.cursor;
        let end = if start.offset < self.line_length(start.line) {
            let offset = self.line(start.line).map_or(start.offset, |line| {
                next_grapheme_boundary(line, start.offset)
            });
            AirTextPosition::new(start.line, offset)
        } else {
            let next = start.line.saturating_add(1);
            if next >= self.lines.len() {
                // Fin du texte : rien à recoller.
                return;
            }
            AirTextPosition::new(next, 0)
        };
        self.history.break_run();
        self.cursor = self.splice(start, end, "", changes);
        self.history.break_run();
    }

    /// Supprime la ligne courante.
    ///
    /// Trois plages, et le choix de la bonne est tout l'intérêt : une ligne supprimée doit
    /// emporter **un** saut de ligne, et un seul.
    ///
    /// - une ligne suivie d'une autre va de son début au début de la suivante ;
    /// - la **dernière** ligne va de la fin de la précédente à la sienne, parce que le
    ///   saut de ligne à retirer est devant elle et non derrière. Ne pas le prendre
    ///   laisserait une ligne vide là où la ligne était ;
    /// - la ligne **unique** est vidée sans être retirée : l'invariant « au moins une
    ///   ligne » vaut aussi pour la suppression.
    fn delete_line(&mut self, changes: &mut AirChangeSet<AirTextChange>) {
        self.history.break_run();
        let index = self.cursor.line.min(self.last_line());
        let next = index.saturating_add(1);
        let (start, end) = if next <= self.last_line() {
            (
                AirTextPosition::new(index, 0),
                AirTextPosition::new(next, 0),
            )
        } else if let Some(previous) = index.checked_sub(1) {
            (
                AirTextPosition::new(previous, self.line_length(previous)),
                AirTextPosition::new(index, self.line_length(index)),
            )
        } else {
            (
                AirTextPosition::new(0, 0),
                AirTextPosition::new(0, self.line_length(0)),
            )
        };
        self.splice(start, end, "", changes);
        // Le curseur se pose au début de la ligne qui a pris la place de celle qu'on
        // vient de retirer — la dernière, quand c'est la dernière qui est partie.
        self.cursor = AirTextPosition::new(index.min(self.last_line()), 0);
        self.history.break_run();
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

impl AirTextBuffer {
    /// Applique un déplacement, sans rien décider de la sélection.
    ///
    /// Séparée d'`apply` pour que les deux modes — étendre et effacer — partagent le même
    /// code de déplacement : c'est la raison d'être d'[`AirTextMotion`].
    fn apply_motion(&mut self, motion: AirTextMotion) {
        match motion {
            AirTextMotion::Left => self.move_left(),
            AirTextMotion::Right => self.move_right(),
            AirTextMotion::Up => self.move_to_line(self.cursor.line.saturating_sub(1)),
            AirTextMotion::Down => self.move_to_line(self.cursor.line.saturating_add(1)),
            AirTextMotion::LineStart => {
                self.cursor.offset = 0;
                self.moved(false);
            }
            AirTextMotion::LineEnd => {
                self.cursor.offset = self.line_length(self.cursor.line);
                self.moved(false);
            }
            AirTextMotion::PreviousWord => self.move_to_previous_word(),
            AirTextMotion::NextWord => self.move_to_next_word(),
            AirTextMotion::DocumentStart => {
                self.cursor = AirTextPosition::default();
                self.moved(false);
            }
            AirTextMotion::DocumentEnd => {
                let line = self.last_line();
                self.cursor = AirTextPosition::new(line, self.line_length(line));
                self.moved(false);
            }
            AirTextMotion::UpByLines(count) => {
                self.move_to_line(self.cursor.line.saturating_sub(count));
            }
            AirTextMotion::DownByLines(count) => {
                self.move_to_line(self.cursor.line.saturating_add(count));
            }
            AirTextMotion::To(position) => self.move_to(position),
            AirTextMotion::ToColumn { line, column } => {
                let line = line.min(self.last_line());
                let offset = match self.line(line) {
                    Some(text) => Self::offset_for_column(text, column),
                    None => 0,
                };
                self.cursor = AirTextPosition::new(line, offset);
                self.moved(false);
            }
        }
    }
}

impl AirModel for AirTextBuffer {
    type Action = AirTextAction;
    type Change = AirTextChange;

    fn apply(&mut self, action: Self::Action, changes: &mut AirChangeSet<Self::Change>) {
        match action {
            // Les trois insertions remplacent la sélection **dans le même
            // remplacement** : taper sur une sélection est un geste, donc un pas
            // d'annulation (ADR-014).
            AirTextAction::InsertCharacter(character) => {
                self.insert_character(character, changes);
            }
            AirTextAction::InsertText(inserted) => self.insert_text(&inserted, changes),
            AirTextAction::InsertLineBreak => self.insert_line_break(changes),
            AirTextAction::DeleteBackward => {
                // Sur une sélection, le retour arrière la supprime **et s'arrête** : y
                // ajouter la grappe précédente mangerait un caractère de plus que ce que
                // l'utilisateur voyait surligné.
                if !self.delete_selection(changes) {
                    self.delete_backward(changes);
                }
            }
            AirTextAction::DeleteForward => {
                if !self.delete_selection(changes) {
                    self.delete_forward(changes);
                }
            }
            AirTextAction::DeleteLine => {
                self.clear_selection();
                self.delete_line(changes);
            }
            AirTextAction::Move { motion, extend } => {
                if extend {
                    self.begin_selection();
                } else {
                    self.clear_selection();
                }
                self.apply_motion(motion);
            }
            AirTextAction::SelectAll => {
                self.anchor = Some(AirTextPosition::default());
                let line = self.last_line();
                self.cursor = AirTextPosition::new(line, self.line_length(line));
                self.selection_generation.advance();
                self.moved(false);
            }
            AirTextAction::SelectSegment => self.select_segment(),
            AirTextAction::SelectLine => self.select_line(),
            AirTextAction::ClearSelection => {
                self.clear_selection();
            }
            AirTextAction::DeleteSelection => {
                self.delete_selection(changes);
            }
            AirTextAction::Load(lines) => {
                self.replace_lines(lines);
                // Un chargement oublie l'historique : annuler au-delà d'une ouverture de
                // fichier ferait réapparaître le contenu du précédent (ADR-014).
                self.history.clear();
                self.modified = false;
                changes.record_once(AirTextChange::ContentReplaced);
                changes.record_once(AirTextChange::LineCountChanged);
                changes.record_once(AirTextChange::ModifiedChanged);
            }
            AirTextAction::MarkSaved => {
                self.history.mark_saved();
                self.refresh_modified(changes);
            }
            AirTextAction::Undo => {
                self.undo(changes);
            }
            AirTextAction::Redo => {
                self.redo(changes);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AirTextAction, AirTextBuffer, AirTextChange, AirTextMotion, AirTextPosition};
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

    // ------------------------------------------------- annulation (ADR-014)

    #[test]
    fn annuler_une_frappe_rend_le_texte_et_le_curseur() {
        let mut buffer = AirTextBuffer::from_text("une");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::LineEnd)],
        );
        type_text(&mut buffer, " chose");
        assert_eq!(buffer.to_text(), "une chose");
        assert_eq!(buffer.undo_depth(), 1, "six frappes, un seul geste");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "une");
        assert_eq!(
            buffer.cursor(),
            AirTextPosition::new(0, 3),
            "le regard revient avant la première lettre tapée"
        );
    }

    #[test]
    fn un_deplacement_coupe_le_geste_de_frappe() {
        let mut buffer = AirTextBuffer::new();
        type_text(&mut buffer, "ab");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::LineStart)],
        );
        type_text(&mut buffer, "z");
        assert_eq!(buffer.to_text(), "zab");
        assert_eq!(buffer.undo_depth(), 2);
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "ab");
    }

    #[test]
    fn un_collage_s_annule_d_un_seul_pas() {
        let mut buffer = AirTextBuffer::from_text("début");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::InsertText(" une\ndeux\ntrois".to_owned()),
            ],
        );
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.undo_depth(), 1);
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "début");
        assert_eq!(buffer.line_count(), 1);
    }

    #[test]
    fn un_saut_de_ligne_est_une_frontiere_d_annulation() {
        // Un paragraphe est une frontière : annuler ne doit pas emporter la ligne d'avant.
        let mut buffer = AirTextBuffer::new();
        type_text(&mut buffer, "une");
        apply_all(&mut buffer, vec![AirTextAction::InsertLineBreak]);
        type_text(&mut buffer, "deux");
        assert_eq!(buffer.undo_depth(), 3, "frappe, saut, frappe");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "une\n");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "une");
    }

    #[test]
    fn une_serie_de_retours_arriere_s_annule_d_un_seul_pas() {
        let mut buffer = AirTextBuffer::from_text("abcdef");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::DeleteBackward,
                AirTextAction::DeleteBackward,
                AirTextAction::DeleteBackward,
            ],
        );
        assert_eq!(buffer.to_text(), "abc");
        assert_eq!(buffer.undo_depth(), 1);
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "abcdef");
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 6));
    }

    #[test]
    fn annuler_une_suppression_de_selection_rend_la_selection() {
        // La moitié de ce qu'on attend d'une annulation est que le regard revienne : ici,
        // ce qui revient est la surbrillance elle-même.
        let mut buffer = AirTextBuffer::from_text("une belle journée");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineStart),
                AirTextAction::select_by(AirTextMotion::NextWord),
                AirTextAction::DeleteSelection,
            ],
        );
        assert_ne!(buffer.to_text(), "une belle journée");
        assert!(!buffer.has_selection());
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "une belle journée");
        assert!(buffer.has_selection(), "la sélection doit réapparaître");
        assert_eq!(buffer.selected_text().as_deref(), Some("une "));
    }

    #[test]
    fn annuler_une_suppression_de_ligne_la_rend_entiere() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::DownByLines(1)),
                AirTextAction::DeleteLine,
            ],
        );
        assert_eq!(buffer.to_text(), "une\ntrois");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "une\ndeux\ntrois");
        assert_eq!(buffer.line_count(), 3);
    }

    #[test]
    fn annuler_la_suppression_de_la_derniere_ligne_la_rend_aussi() {
        // Le cas qui a trouvé un défaut pendant la refonte : la plage d'une dernière
        // ligne part de la **fin de la précédente**, pour emporter le saut de ligne qui
        // est devant elle et non derrière.
        let mut buffer = AirTextBuffer::from_text("une\ndeux");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::DocumentEnd),
                AirTextAction::DeleteLine,
            ],
        );
        assert_eq!(buffer.lines(), ["une"]);
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.lines(), ["une", "deux"]);
    }

    #[test]
    fn retablir_rejoue_le_geste_annule() {
        let mut buffer = AirTextBuffer::new();
        type_text(&mut buffer, "abc");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "");
        assert!(buffer.can_redo());
        apply_all(&mut buffer, vec![AirTextAction::Redo]);
        assert_eq!(buffer.to_text(), "abc");
        assert_eq!(
            buffer.cursor(),
            AirTextPosition::new(0, 3),
            "le curseur se pose après le texte rejoué, comme après la frappe"
        );
    }

    #[test]
    fn une_frappe_apres_une_annulation_interdit_le_retablissement() {
        let mut buffer = AirTextBuffer::new();
        type_text(&mut buffer, "abc");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert!(buffer.can_redo());
        type_text(&mut buffer, "z");
        assert!(!buffer.can_redo(), "le texte décrit n'existe plus");
    }

    #[test]
    fn annuler_tout_eteint_le_drapeau_de_modification() {
        // Un drapeau que seule une édition allume resterait allumé sur un texte revenu à
        // son état enregistré, et le titre de `redit` garderait son étoile.
        let mut buffer = AirTextBuffer::from_text("origine");
        apply_all(&mut buffer, vec![AirTextAction::MarkSaved]);
        assert!(!buffer.is_modified());
        type_text(&mut buffer, "x");
        assert!(buffer.is_modified());
        let changes = apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert!(!buffer.is_modified(), "le texte est revenu à l'enregistré");
        assert!(
            changes.contains(&AirTextChange::ModifiedChanged),
            "le basculement doit être consigné : {:?}",
            changes.as_slice()
        );
    }

    #[test]
    fn un_chargement_oublie_l_historique() {
        // Annuler au-delà d'une ouverture ferait réapparaître le fichier précédent.
        let mut buffer = AirTextBuffer::new();
        type_text(&mut buffer, "brouillon");
        assert!(buffer.can_undo());
        apply_all(
            &mut buffer,
            vec![AirTextAction::Load(vec!["une".to_owned()])],
        );
        assert!(!buffer.can_undo());
        assert!(!buffer.can_redo());
        assert!(!buffer.is_modified());
    }

    #[test]
    fn annuler_sans_rien_a_annuler_ne_fait_rien() {
        let mut buffer = AirTextBuffer::from_text("une");
        let before = buffer.text_generation();
        let changes = apply_all(&mut buffer, vec![AirTextAction::Undo, AirTextAction::Redo]);
        assert_eq!(buffer.to_text(), "une");
        assert_eq!(buffer.text_generation(), before, "rien n'a bougé");
        assert!(changes.is_empty());
    }

    #[test]
    fn un_deplacement_seul_ne_remplit_pas_le_journal() {
        // Le journal ne porte que des remplacements : un déplacement n'en est pas un, et
        // un pas d'annulation vide se ferait annuler sans que l'écran bouge.
        let mut buffer = AirTextBuffer::from_text("une\ndeux");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::Right),
                AirTextAction::move_by(AirTextMotion::Down),
                AirTextAction::SelectAll,
                AirTextAction::ClearSelection,
            ],
        );
        assert!(!buffer.can_undo());
        assert_eq!(buffer.undo_depth(), 0);
    }

    #[test]
    fn annuler_une_frappe_sur_une_selection_rend_le_texte_remplace() {
        // Deux gestes en un : la sélection est supprimée **et** le caractère inséré. Les
        // deux doivent revenir, et dans le bon ordre.
        let mut buffer = AirTextBuffer::from_text("une belle journée");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineStart),
                AirTextAction::select_by(AirTextMotion::NextWord),
                AirTextAction::InsertCharacter('X'),
            ],
        );
        assert_eq!(buffer.to_text(), "Xbelle journée");
        assert_eq!(buffer.undo_depth(), 1, "un geste, un pas");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "une belle journée");
        assert!(
            buffer.has_selection(),
            "la sélection remplacée doit réapparaître"
        );
    }

    #[test]
    fn annuler_une_suppression_avant_rend_le_caractere() {
        let mut buffer = AirTextBuffer::from_text("abc");
        apply_all(&mut buffer, vec![AirTextAction::DeleteForward]);
        assert_eq!(buffer.to_text(), "bc");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "abc");
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
    }

    #[test]
    fn annuler_une_grappe_composee_la_rend_entiere() {
        // Une grappe de plusieurs points de code s'efface d'une touche : son annulation
        // doit la rendre entière, et non son premier point de code.
        let mut buffer = AirTextBuffer::from_text("e\u{301}a");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineStart),
                AirTextAction::DeleteForward,
            ],
        );
        assert_eq!(buffer.to_text(), "a");
        apply_all(&mut buffer, vec![AirTextAction::Undo]);
        assert_eq!(buffer.to_text(), "e\u{301}a");
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
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::Right)],
        );
        assert_eq!(buffer.text_generation(), text);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 1));
    }

    #[test]
    fn une_grappe_s_efface_d_une_seule_touche() {
        // `e` plus accent combinant : deux caractères, une grappe, une touche.
        let mut buffer = AirTextBuffer::from_text("cafe\u{301}");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::DeleteBackward,
            ],
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
                AirTextAction::move_by(AirTextMotion::Right),
                AirTextAction::move_by(AirTextMotion::Right),
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
            vec![
                AirTextAction::move_by(AirTextMotion::Down),
                AirTextAction::DeleteBackward,
            ],
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
            vec![
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::DeleteForward,
            ],
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
                AirTextAction::move_by(AirTextMotion::DocumentEnd),
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
            vec![
                AirTextAction::move_by(AirTextMotion::Down),
                AirTextAction::DeleteLine,
            ],
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
            vec![
                AirTextAction::move_by(AirTextMotion::DocumentEnd),
                AirTextAction::DeleteLine,
            ],
        );
        assert_eq!(buffer.lines(), ["une"]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
    }

    #[test]
    fn la_fleche_gauche_remonte_a_la_fin_de_la_ligne_precedente() {
        let mut buffer = AirTextBuffer::from_text("abc\nd");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::Down)],
        );
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::Left)],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 3));
    }

    #[test]
    fn la_fleche_droite_descend_au_debut_de_la_ligne_suivante() {
        let mut buffer = AirTextBuffer::from_text("a\nb");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::move_by(AirTextMotion::Right),
            ],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 0));
    }

    #[test]
    fn les_fleches_aux_bords_du_texte_ne_font_rien() {
        let mut buffer = AirTextBuffer::from_text("a");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::Left)],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::DocumentEnd),
                AirTextAction::move_by(AirTextMotion::Right),
            ],
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
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::move_by(AirTextMotion::Down),
                AirTextAction::move_by(AirTextMotion::Down),
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
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::move_by(AirTextMotion::Down),
                // La ligne du bas est courte : le curseur s'arrête à sa fin.
                AirTextAction::move_by(AirTextMotion::Left),
                AirTextAction::move_by(AirTextMotion::Up),
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
            vec![
                AirTextAction::move_by(AirTextMotion::Right),
                AirTextAction::move_by(AirTextMotion::Down),
            ],
        );
        assert_eq!(buffer.cursor_column(), 2);
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 2));
    }

    #[test]
    fn les_deplacements_par_page_bornent_au_texte() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::DownByLines(100))],
        );
        assert_eq!(buffer.cursor().line, 2);
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::UpByLines(100))],
        );
        assert_eq!(buffer.cursor().line, 0);
    }

    #[test]
    fn le_deplacement_par_mot_suit_l_annexe_29() {
        let mut buffer = AirTextBuffer::from_text("le chat, noir");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::NextWord)],
        );
        assert_eq!(buffer.cursor().offset, 3);
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::NextWord)],
        );
        assert_eq!(buffer.cursor().offset, 9);
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::PreviousWord)],
        );
        assert_eq!(buffer.cursor().offset, 3);
    }

    #[test]
    fn le_mot_suivant_passe_a_la_ligne_quand_la_ligne_est_finie() {
        let mut buffer = AirTextBuffer::from_text("un\ndeux");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::move_by(AirTextMotion::NextWord),
            ],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 0));
    }

    #[test]
    fn le_mot_precedent_remonte_a_la_fin_de_la_ligne_precedente() {
        let mut buffer = AirTextBuffer::from_text("un\ndeux");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::Down),
                AirTextAction::move_by(AirTextMotion::PreviousWord),
            ],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 2));
    }

    #[test]
    fn le_mot_aux_bords_du_texte_ne_bouge_pas() {
        let mut buffer = AirTextBuffer::from_text("un");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::PreviousWord)],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::DocumentEnd),
                AirTextAction::move_by(AirTextMotion::NextWord),
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
            vec![AirTextAction::move_by(AirTextMotion::To(
                AirTextPosition::new(0, 2),
            ))],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 0));
    }

    #[test]
    fn un_clic_place_le_curseur_a_la_colonne_d_affichage() {
        // L'idéogramme occupe deux colonnes : la colonne 2 tombe sur le `a` qui le suit.
        let mut buffer = AirTextBuffer::from_text("x\n\u{6F22}ab");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::ToColumn {
                line: 1,
                column: 2,
            })],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 3));
    }

    #[test]
    fn un_clic_au_dela_d_une_ligne_place_le_curseur_a_sa_fin() {
        let mut buffer = AirTextBuffer::from_text("ab");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::ToColumn {
                line: 99,
                column: 99,
            })],
        );
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 2));
    }

    #[test]
    fn une_position_hors_du_texte_est_bornee() {
        let mut buffer = AirTextBuffer::from_text("ab\ncd");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::To(
                AirTextPosition::new(99, 99),
            ))],
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
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::LineEnd)],
        );
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

    // ------------------------------------------------------- la sélection

    /// Sélectionne vers la droite d'autant de grappes, depuis la position courante.
    fn select_right(buffer: &mut AirTextBuffer, count: usize) {
        apply_all(
            buffer,
            (0..count)
                .map(|_| AirTextAction::select_by(AirTextMotion::Right))
                .collect(),
        );
    }

    #[test]
    fn une_fleche_nue_n_ouvre_pas_de_selection() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::Right)],
        );
        assert_eq!(buffer.selection(), None);
        assert!(!buffer.has_selection());
        assert_eq!(buffer.selected_text(), None);
    }

    #[test]
    fn maj_et_une_fleche_ouvrent_une_selection() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 2);
        assert_eq!(
            buffer.selection(),
            Some((AirTextPosition::new(0, 0), AirTextPosition::new(0, 2)))
        );
        assert_eq!(buffer.selected_text().as_deref(), Some("ab"));
    }

    #[test]
    fn revenir_en_arriere_reduit_la_selection() {
        // C'est la raison d'être de l'ancre : garder deux bornes déjà ordonnées ferait
        // repartir une sélection au lieu de la réduire.
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 3);
        apply_all(
            &mut buffer,
            vec![AirTextAction::select_by(AirTextMotion::Left)],
        );
        assert_eq!(buffer.selected_text().as_deref(), Some("ab"));
    }

    #[test]
    fn une_selection_vide_ne_compte_pas() {
        // Aller à droite puis revenir ramène le curseur sur l'ancre : il n'y a rien à
        // peindre, et l'exposer obligerait chaque lecteur à traiter un cas sans effet.
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 1);
        apply_all(
            &mut buffer,
            vec![AirTextAction::select_by(AirTextMotion::Left)],
        );
        assert_eq!(buffer.selection(), None);
    }

    #[test]
    fn une_selection_vers_la_gauche_rend_des_bornes_ordonnees() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::LineEnd),
                AirTextAction::select_by(AirTextMotion::Left),
                AirTextAction::select_by(AirTextMotion::Left),
            ],
        );
        assert_eq!(
            buffer.selection(),
            Some((AirTextPosition::new(0, 2), AirTextPosition::new(0, 4)))
        );
        assert_eq!(buffer.selected_text().as_deref(), Some("cd"));
    }

    #[test]
    fn une_selection_sur_plusieurs_lignes_porte_les_sauts_de_ligne() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::Right),
                AirTextAction::select_by(AirTextMotion::Down),
                AirTextAction::select_by(AirTextMotion::Down),
            ],
        );
        assert_eq!(buffer.selected_text().as_deref(), Some("ne\ndeux\nt"));
    }

    #[test]
    fn un_deplacement_nu_efface_la_selection() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 2);
        apply_all(
            &mut buffer,
            vec![AirTextAction::move_by(AirTextMotion::Right)],
        );
        assert_eq!(buffer.selection(), None);
    }

    #[test]
    fn taper_sur_une_selection_la_remplace() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 2);
        type_text(&mut buffer, "X");
        assert_eq!(buffer.line(0), Some("Xcd"));
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 1));
        assert_eq!(buffer.selection(), None);
    }

    #[test]
    fn le_retour_arriere_sur_une_selection_ne_mange_rien_de_plus() {
        // Le piège : supprimer la sélection **puis** la grappe précédente retirerait un
        // caractère que l'utilisateur ne voyait pas surligné.
        let mut buffer = AirTextBuffer::from_text("abcd");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::Right),
                AirTextAction::select_by(AirTextMotion::Right),
                AirTextAction::DeleteBackward,
            ],
        );
        assert_eq!(buffer.line(0), Some("acd"));
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 1));
    }

    #[test]
    fn la_touche_supprime_sur_une_selection_la_supprime() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 2);
        apply_all(&mut buffer, vec![AirTextAction::DeleteForward]);
        assert_eq!(buffer.line(0), Some("cd"));
    }

    #[test]
    fn supprimer_une_selection_de_plusieurs_lignes_recolle_les_bords() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::Right),
                AirTextAction::select_by(AirTextMotion::Down),
                AirTextAction::select_by(AirTextMotion::Down),
            ],
        );
        let changes = apply_all(&mut buffer, vec![AirTextAction::DeleteSelection]);
        assert_eq!(buffer.lines(), ["urois"]);
        assert_eq!(buffer.line_count(), 1);
        assert!(changes.contains(&AirTextChange::LineCountChanged));
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 1));
    }

    #[test]
    fn tout_selectionner_porte_le_curseur_a_la_fin() {
        let mut buffer = AirTextBuffer::from_text("ab\ncd");
        apply_all(&mut buffer, vec![AirTextAction::SelectAll]);
        assert_eq!(buffer.selected_text().as_deref(), Some("ab\ncd"));
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 2));
    }

    #[test]
    fn abandonner_la_selection_ne_deplace_pas_le_curseur() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 2);
        let before = buffer.cursor();
        let generation = buffer.selection_generation();
        apply_all(&mut buffer, vec![AirTextAction::ClearSelection]);
        assert_eq!(buffer.cursor(), before);
        assert_eq!(buffer.selection(), None);
        assert_ne!(
            buffer.selection_generation().value(),
            generation.value(),
            "la vue ne saurait pas qu'il faut effacer la surbrillance"
        );
    }

    #[test]
    fn abandonner_une_selection_absente_ne_fait_rien_avancer() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        let generation = buffer.selection_generation();
        apply_all(&mut buffer, vec![AirTextAction::ClearSelection]);
        assert_eq!(buffer.selection_generation().value(), generation.value());
    }

    #[test]
    fn charger_un_contenu_abandonne_la_selection() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 2);
        apply_all(
            &mut buffer,
            vec![AirTextAction::Load(vec!["neuf".to_owned()])],
        );
        assert_eq!(buffer.selection(), None);
    }

    // ------------------------------------------------ l'insertion de texte

    #[test]
    fn un_texte_insere_porte_ses_sauts_de_ligne() {
        let mut buffer = AirTextBuffer::from_text("ad");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::Right),
                AirTextAction::InsertText("b\nc".to_owned()),
            ],
        );
        assert_eq!(buffer.lines(), ["ab", "cd"]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(1, 1));
    }

    #[test]
    fn un_texte_insere_sans_saut_reste_sur_la_ligne() {
        let mut buffer = AirTextBuffer::from_text("ad");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::Right),
                AirTextAction::InsertText("bc".to_owned()),
            ],
        );
        assert_eq!(buffer.lines(), ["abcd"]);
        assert_eq!(buffer.cursor(), AirTextPosition::new(0, 3));
    }

    #[test]
    fn un_texte_insere_remplace_la_selection() {
        let mut buffer = AirTextBuffer::from_text("abcd");
        select_right(&mut buffer, 2);
        apply_all(
            &mut buffer,
            vec![AirTextAction::InsertText("XY".to_owned())],
        );
        assert_eq!(buffer.lines(), ["XYcd"]);
        assert_eq!(buffer.selection(), None);
    }

    #[test]
    fn un_texte_insere_fait_avancer_la_generation_une_seule_fois() {
        // Un collage n'est pas une suite de frappes : le traiter comme telle ferait
        // repousser le reflet une fois par caractère.
        let mut buffer = AirTextBuffer::new();
        let before = buffer.text_generation().value();
        apply_all(
            &mut buffer,
            vec![AirTextAction::InsertText("abcdef".to_owned())],
        );
        assert_eq!(buffer.text_generation().value(), before.saturating_add(1));
    }

    #[test]
    fn un_texte_vide_insere_ne_change_rien_au_contenu() {
        let mut buffer = AirTextBuffer::from_text("ab");
        apply_all(&mut buffer, vec![AirTextAction::InsertText(String::new())]);
        assert_eq!(buffer.lines(), ["ab"]);
    }

    // -------------------------------------- le segment et la ligne entière

    #[test]
    fn selectionner_le_segment_prend_le_mot_sous_le_curseur() {
        let mut buffer = AirTextBuffer::from_text("une belle journée");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::ToColumn { line: 0, column: 6 }),
                AirTextAction::SelectSegment,
            ],
        );
        assert_eq!(buffer.selected_text().as_deref(), Some("belle"));
    }

    #[test]
    fn selectionner_le_segment_sur_un_espace_prend_les_espaces() {
        // Rendre « rien » laisserait le double clic sans effet visible.
        let mut buffer = AirTextBuffer::from_text("une  belle");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::ToColumn { line: 0, column: 3 }),
                AirTextAction::SelectSegment,
            ],
        );
        assert_eq!(buffer.selected_text().as_deref(), Some("  "));
    }

    #[test]
    fn selectionner_la_ligne_prend_son_saut_de_ligne() {
        // L'ancre au début de la ligne, le curseur au début de la **suivante** : c'est ce
        // qui fait qu'une suppression retire la ligne entière, sans laisser une ligne vide.
        let mut buffer = AirTextBuffer::from_text("une\ndeux\ntrois");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::Down),
                AirTextAction::SelectLine,
            ],
        );
        assert_eq!(buffer.selected_text().as_deref(), Some("deux\n"));

        apply_all(&mut buffer, vec![AirTextAction::DeleteSelection]);
        assert_eq!(buffer.lines(), ["une", "trois"]);
    }

    #[test]
    fn selectionner_la_derniere_ligne_s_arrete_a_sa_fin() {
        let mut buffer = AirTextBuffer::from_text("une\ndeux");
        apply_all(
            &mut buffer,
            vec![
                AirTextAction::move_by(AirTextMotion::DocumentEnd),
                AirTextAction::SelectLine,
            ],
        );
        assert_eq!(buffer.selected_text().as_deref(), Some("deux"));
    }

    #[test]
    fn selectionner_le_segment_d_une_ligne_vide_ne_selectionne_rien() {
        let mut buffer = AirTextBuffer::new();
        apply_all(&mut buffer, vec![AirTextAction::SelectSegment]);
        assert_eq!(buffer.selection(), None);
    }
}
