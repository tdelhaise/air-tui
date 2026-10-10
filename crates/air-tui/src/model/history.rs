// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le journal d'annulation — un historique de **remplacements**, pas d'instantanés.
//!
//! # Pourquoi des remplacements
//!
//! L'ADR-014 tranche : un pas d'annulation est « entre ces deux positions, il y avait ce
//! texte, il y a maintenant celui-là ». Son inverse est le même remplacement, les deux
//! textes échangés — et rien d'autre à calculer.
//!
//! Un instantané du tampon coûterait la taille du document à **chaque pas** ; une entrée
//! de journal coûte la longueur du texte qui change, donc un caractère pour une frappe.
//! Le prix de ce choix est qu'il n'est juste que si **une seule** mutation produit les
//! entrées, et c'est ce que `AirTextBuffer::splice` garantit.
//!
//! # Pourquoi une action n'est pas journalisable
//!
//! `AirTextAction::DeleteBackward` est une **intention** : elle supprime une sélection, ou
//! une grappe, ou un saut de ligne, selon l'état. Une intention ne s'inverse pas sans
//! reconstituer l'état d'avant. Un effet, si.

use super::text::AirTextPosition;

/// Le nombre de pas d'annulation que le journal retient.
///
/// Un journal non borné est une fuite lente dans une session longue. La borne est une
/// capacité qu'on annonce plutôt qu'une mémoire qu'on promet : au-delà, la plus ancienne
/// entrée est oubliée, et le point d'enregistrement avec elle s'il tombe en dehors.
pub(crate) const UNDO_DEPTH_LIMIT: usize = 256;

/// Un remplacement, et de quoi replacer le regard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AirTextEdit {
    /// Là où le remplacement commence.
    pub start: AirTextPosition,
    /// Ce qui s'y trouvait.
    pub removed: String,
    /// Ce qui s'y trouve.
    pub inserted: String,
    /// Où était le curseur avant ce remplacement.
    pub cursor_before: AirTextPosition,
    /// Où était l'ancre de sélection avant ce remplacement.
    pub anchor_before: Option<AirTextPosition>,
}

impl AirTextEdit {
    /// Vrai si ce remplacement n'ajoute que du texte.
    fn is_pure_insertion(&self) -> bool {
        self.removed.is_empty()
    }

    /// Vrai si ce remplacement ne retire que du texte.
    fn is_pure_deletion(&self) -> bool {
        self.inserted.is_empty()
    }

    /// La position qui suit le texte inséré.
    fn end_of_inserted(&self) -> AirTextPosition {
        position_after(self.start, &self.inserted)
    }

    /// La position qui suit le texte retiré, dans le tampon d'**avant**.
    fn end_of_removed(&self) -> AirTextPosition {
        position_after(self.start, &self.removed)
    }
}

/// La position atteinte après avoir écrit un texte à une position.
///
/// Sans saut de ligne, la colonne avance de la longueur en octets. Avec, la ligne avance
/// du nombre de sauts et la colonne repart de ce qui suit le dernier.
pub(crate) fn position_after(start: AirTextPosition, text: &str) -> AirTextPosition {
    match text.rfind('\n') {
        None => AirTextPosition::new(start.line, start.offset.saturating_add(text.len())),
        Some(index) => {
            let breaks = text.matches('\n').count();
            let after = text.len().saturating_sub(index).saturating_sub(1);
            AirTextPosition::new(start.line.saturating_add(breaks), after)
        }
    }
}

/// L'historique d'un tampon : ce qui peut être annulé, et ce qui peut être rétabli.
#[derive(Debug, Clone, Default)]
pub(crate) struct AirEditHistory {
    /// Les remplacements faits, du plus ancien au plus récent.
    done: Vec<AirTextEdit>,
    /// Les remplacements annulés, le dernier annulé en queue.
    undone: Vec<AirTextEdit>,
    /// Vrai quand le geste en cours est terminé, donc qu'une entrée nouvelle ne s'y fond
    /// pas.
    run_broken: bool,
    /// Le nombre de remplacements faits au dernier enregistrement.
    ///
    /// `None` veut dire « l'état enregistré n'est plus joignable » : soit rien n'a été
    /// enregistré, soit l'entrée qui y menait est tombée hors de la borne.
    saved_depth: Option<usize>,
}

impl AirEditHistory {
    /// Un historique vide, au point d'enregistrement.
    pub(crate) fn new() -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            run_broken: true,
            saved_depth: Some(0),
        }
    }

    /// Termine le geste en cours : l'entrée suivante ne se fondra pas dans la précédente.
    pub(crate) const fn break_run(&mut self) {
        self.run_broken = true;
    }

    /// Consigne un remplacement, en le fondant dans le précédent si c'est le même geste.
    ///
    /// Toute édition nouvelle **vide** la pile de rétablissement : rétablir après avoir
    /// édité rejouerait un remplacement sur un texte qui n'est plus celui qu'il décrivait.
    pub(crate) fn record(&mut self, entry: AirTextEdit) {
        self.undone.clear();
        if !self.run_broken
            && let Some(last) = self.done.last_mut()
            && merge(last, &entry)
        {
            return;
        }
        self.run_broken = false;
        self.done.push(entry);
        if self.done.len() > UNDO_DEPTH_LIMIT {
            self.done.remove(0);
            // Le point d'enregistrement se décale avec la pile, et devient injoignable
            // quand c'est lui qui est tombé.
            self.saved_depth = self.saved_depth.and_then(|depth| depth.checked_sub(1));
        }
    }

    /// Retire le dernier remplacement fait, pour qu'il soit joué à l'envers.
    pub(crate) fn undo(&mut self) -> Option<AirTextEdit> {
        let entry = self.done.pop()?;
        self.undone.push(entry.clone());
        self.run_broken = true;
        Some(entry)
    }

    /// Reprend le dernier remplacement annulé, pour qu'il soit rejoué à l'endroit.
    pub(crate) fn redo(&mut self) -> Option<AirTextEdit> {
        let entry = self.undone.pop()?;
        self.done.push(entry.clone());
        self.run_broken = true;
        Some(entry)
    }

    /// Vrai s'il y a quelque chose à annuler.
    pub(crate) fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    /// Vrai s'il y a quelque chose à rétablir.
    pub(crate) fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// Déclare l'état courant enregistré.
    pub(crate) const fn mark_saved(&mut self) {
        self.saved_depth = Some(self.done.len());
        self.run_broken = true;
    }

    /// Vrai si le texte est celui du dernier enregistrement.
    ///
    /// C'est ce qui fait qu'annuler jusqu'au bout **efface** l'étoile du titre : un
    /// drapeau que seule une édition met à vrai resterait allumé sur un texte revenu à
    /// son état enregistré.
    pub(crate) fn is_at_saved_point(&self) -> bool {
        self.saved_depth == Some(self.done.len())
    }

    /// Oublie tout, et déclare l'état courant enregistré.
    ///
    /// C'est ce qu'un chargement demande : annuler au-delà d'une ouverture de fichier
    /// ferait réapparaître le contenu du fichier précédent, ce qui n'a pas de sens pour
    /// l'utilisateur.
    pub(crate) fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
        self.run_broken = true;
        self.saved_depth = Some(0);
    }

    /// Le nombre de pas annulables, pour les essais et le compte rendu.
    pub(crate) fn depth(&self) -> usize {
        self.done.len()
    }
}

/// Fond `entry` dans `last` si les deux forment un geste continu.
///
/// Les deux seuls cas, et l'ADR-014 dit pourquoi ce sont les deux :
///
/// - une insertion qui commence là où la précédente finissait — une frappe qui continue ;
/// - une suppression arrière qui finit là où la précédente commençait — un retour arrière
///   qui continue.
///
/// Le curseur d'avant retenu est celui de la **première** entrée : annuler un mot tapé
/// doit replacer le regard avant la première lettre.
fn merge(last: &mut AirTextEdit, entry: &AirTextEdit) -> bool {
    if last.is_pure_insertion()
        && entry.is_pure_insertion()
        && last.end_of_inserted() == entry.start
    {
        last.inserted.push_str(&entry.inserted);
        return true;
    }
    if last.is_pure_deletion() && entry.is_pure_deletion() && entry.end_of_removed() == last.start {
        let mut joined = entry.removed.clone();
        joined.push_str(&last.removed);
        last.removed = joined;
        last.start = entry.start;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{AirEditHistory, AirTextEdit, UNDO_DEPTH_LIMIT, position_after};
    use crate::model::text::AirTextPosition;

    /// Un remplacement, écrit court pour que les essais se lisent.
    fn edit(line: usize, offset: usize, removed: &str, inserted: &str) -> AirTextEdit {
        AirTextEdit {
            start: AirTextPosition::new(line, offset),
            removed: removed.to_owned(),
            inserted: inserted.to_owned(),
            cursor_before: AirTextPosition::new(line, offset),
            anchor_before: None,
        }
    }

    #[test]
    fn une_position_avance_en_colonne_sans_saut_de_ligne() {
        let start = AirTextPosition::new(2, 3);
        assert_eq!(position_after(start, "abc"), AirTextPosition::new(2, 6));
        assert_eq!(position_after(start, ""), start);
    }

    #[test]
    fn une_position_change_de_ligne_a_chaque_saut() {
        let start = AirTextPosition::new(2, 3);
        assert_eq!(position_after(start, "\n"), AirTextPosition::new(3, 0));
        assert_eq!(position_after(start, "a\nbc"), AirTextPosition::new(3, 2));
        assert_eq!(
            position_after(start, "a\nb\nc"),
            AirTextPosition::new(4, 1),
            "deux sauts, deux lignes"
        );
    }

    #[test]
    fn une_position_compte_les_octets_et_non_les_caracteres() {
        // Les décalages du tampon sont en octets : un « é » en vaut deux.
        let start = AirTextPosition::new(0, 0);
        assert_eq!(position_after(start, "é"), AirTextPosition::new(0, 2));
    }

    #[test]
    fn un_historique_neuf_n_annule_rien_et_est_au_point_d_enregistrement() {
        let history = AirEditHistory::new();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert!(history.is_at_saved_point());
        assert_eq!(history.depth(), 0);
    }

    #[test]
    fn deux_insertions_qui_se_suivent_se_fondent() {
        // C'est la règle du geste : taper `ab` fait **un** pas d'annulation.
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        history.record(edit(0, 1, "", "b"));
        assert_eq!(history.depth(), 1);
        let entry = history.undo().expect("un pas");
        assert_eq!(entry.inserted, "ab");
        assert_eq!(entry.start, AirTextPosition::new(0, 0));
    }

    #[test]
    fn une_insertion_ailleurs_ne_se_fond_pas() {
        // Le curseur a sauté : les deux frappes ne forment pas un geste.
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        history.record(edit(0, 5, "", "b"));
        assert_eq!(history.depth(), 2);
    }

    #[test]
    fn un_geste_coupe_separe_deux_insertions_contigues() {
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        history.break_run();
        history.record(edit(0, 1, "", "b"));
        assert_eq!(history.depth(), 2, "la coupure doit séparer");
    }

    #[test]
    fn deux_retours_arriere_qui_se_suivent_se_fondent() {
        // Le retour arrière avance **vers la gauche** : la seconde suppression finit là
        // où la première commençait, et le texte retiré se recolle dans cet ordre.
        let mut history = AirEditHistory::new();
        history.record(edit(0, 2, "b", ""));
        history.record(edit(0, 1, "a", ""));
        assert_eq!(history.depth(), 1);
        let entry = history.undo().expect("un pas");
        assert_eq!(entry.removed, "ab", "l'ordre du texte doit être rétabli");
        assert_eq!(entry.start, AirTextPosition::new(0, 1));
    }

    #[test]
    fn le_curseur_retenu_est_celui_du_debut_du_geste() {
        // Annuler un mot tapé doit replacer le regard avant la première lettre.
        let mut history = AirEditHistory::new();
        let mut first = edit(0, 0, "", "a");
        first.cursor_before = AirTextPosition::new(0, 0);
        let mut second = edit(0, 1, "", "b");
        second.cursor_before = AirTextPosition::new(0, 1);
        history.record(first);
        history.record(second);
        let entry = history.undo().expect("un pas");
        assert_eq!(entry.cursor_before, AirTextPosition::new(0, 0));
    }

    #[test]
    fn une_insertion_et_une_suppression_ne_se_fondent_pas() {
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        history.record(edit(0, 1, "x", ""));
        assert_eq!(history.depth(), 2);
    }

    #[test]
    fn un_remplacement_ne_se_fond_dans_rien() {
        // Ni insertion pure, ni suppression pure : un collage sur une sélection.
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "ancien", "nouveau"));
        history.record(edit(0, 7, "", "a"));
        assert_eq!(history.depth(), 2);
    }

    #[test]
    fn annuler_puis_retablir_rend_la_meme_entree() {
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        let undone = history.undo().expect("un pas");
        assert!(!history.can_undo());
        assert!(history.can_redo());
        let redone = history.redo().expect("un rétablissement");
        assert_eq!(undone, redone);
        assert!(history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn une_edition_nouvelle_vide_la_pile_de_retablissement() {
        // Rejouer un remplacement sur un texte qui n'est plus celui qu'il décrivait
        // n'aurait aucun sens.
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        history.undo();
        assert!(history.can_redo());
        history.record(edit(0, 0, "", "z"));
        assert!(!history.can_redo());
    }

    #[test]
    fn le_journal_est_borne_et_oublie_le_plus_ancien() {
        let mut history = AirEditHistory::new();
        for step in 0..UNDO_DEPTH_LIMIT.saturating_add(8) {
            history.break_run();
            history.record(edit(0, step, "", "a"));
        }
        assert_eq!(history.depth(), UNDO_DEPTH_LIMIT);
    }

    #[test]
    fn le_point_d_enregistrement_devient_injoignable_quand_il_tombe_hors_de_la_borne() {
        // C'est le seul endroit où la borne se voit : le texte ne peut plus revenir à son
        // état enregistré, donc le drapeau de modification reste allumé. Mieux vaut cela
        // qu'un drapeau éteint sur un texte qui a changé.
        let mut history = AirEditHistory::new();
        history.mark_saved();
        for step in 0..UNDO_DEPTH_LIMIT.saturating_add(1) {
            history.break_run();
            history.record(edit(0, step, "", "a"));
        }
        for _ in 0..UNDO_DEPTH_LIMIT {
            history.undo();
        }
        assert_eq!(history.depth(), 0);
        assert!(
            !history.is_at_saved_point(),
            "l'entrée qui menait à l'état enregistré a été oubliée"
        );
    }

    #[test]
    fn enregistrer_pose_le_point_et_coupe_le_geste() {
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        assert!(!history.is_at_saved_point());
        history.mark_saved();
        assert!(history.is_at_saved_point());
        history.record(edit(0, 1, "", "b"));
        assert_eq!(history.depth(), 2, "l'enregistrement a coupé le geste");
        assert!(!history.is_at_saved_point());
    }

    #[test]
    fn annuler_jusqu_au_point_d_enregistrement_le_retrouve() {
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        history.mark_saved();
        history.record(edit(0, 1, "", "b"));
        history.undo();
        assert!(history.is_at_saved_point());
    }

    #[test]
    fn vider_l_historique_le_ramene_au_point_d_enregistrement() {
        let mut history = AirEditHistory::new();
        history.record(edit(0, 0, "", "a"));
        history.undo();
        history.record(edit(0, 0, "", "b"));
        history.clear();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert!(history.is_at_saved_point());
    }

    #[test]
    fn annuler_un_historique_vide_ne_rend_rien() {
        let mut history = AirEditHistory::new();
        assert_eq!(history.undo(), None);
        assert_eq!(history.redo(), None);
    }
}
