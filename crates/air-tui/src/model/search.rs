// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La recherche dans un texte — une **question** posée au modèle, pas une mutation.
//!
//! # Pourquoi ce n'est pas une action
//!
//! L'ADR-007 fait passer toute mutation par une [`crate::model::AirTextAction`], et la
//! recherche n'en est pas une : elle ne change ni le texte, ni le curseur, ni la
//! sélection. Elle **répond** — « voici où est la première occurrence à partir d'ici »,
//! ou rien. Ce que le contrôleur en fait ensuite passe, lui, par les actions qui existent
//! déjà : un déplacement qui efface la sélection vers le début de l'occurrence, puis un
//! déplacement qui l'étend vers sa fin.
//!
//! C'est ce partage qui laisse la **politique** au contrôleur. Reprendre au début du
//! document quand on atteint la fin, afficher « introuvable », retenir le dernier motif
//! pour une recherche suivante : aucune de ces trois décisions n'est un fait sur le texte,
//! et aucune n'a donc sa place ici. Le modèle ne sait répondre qu'à une question factuelle,
//! et c'est ce qui le rend éprouvable sans monter de contrôleur.
//!
//! # L'unité de comparaison est la grappe de graphème
//!
//! Pas l'octet, et pas le caractère. Un curseur d'air-tui se déplace par grappes depuis le
//! jalon M3, donc une occurrence qui commencerait ou finirait **au milieu** d'une grappe
//! donnerait une sélection que le curseur ne sait pas décrire. Chercher `e` dans un `é`
//! décomposé — `e` suivi de `U+0301` — ne trouve donc rien, et c'est le comportement
//! juste : ce qui est affiché est un `é`, pas un `e` suivi d'autre chose.
//!
//! # Deux limites, écrites parce qu'elles sont réelles
//!
//! - **le motif ne traverse pas les lignes.** Il vient d'un champ de saisie d'une seule
//!   ligne, qui ne peut pas produire de saut de ligne ; un motif qui en contiendrait un ne
//!   trouverait rien, puisqu'aucune ligne du tampon n'en porte ;
//! - **le repliement de casse est simple**, celui de `char::to_lowercase`, et appliqué
//!   grappe à grappe. `Ê` et `ê` se trouvent l'un l'autre ; `ß` et `SS` non, parce que le
//!   repliement complet d'Unicode change le **nombre** de grappes et qu'aucune table de ce
//!   dépôt ne le décrit. Mieux vaut une limite nommée qu'un repliement qui marche sur
//!   l'alphabet latin et surprend ailleurs.

use crate::unicode::graphemes;

/// Ce qu'on cherche, et comment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AirTextSearch {
    /// Le motif cherché, tel qu'il a été saisi.
    pub pattern: String,
    /// Vrai si la casse compte.
    ///
    /// Faux par défaut : c'est ce qu'on attend en tapant un mot dans un éditeur. Les deux
    /// valeurs sont éprouvées, et `redit` ne sait encore poser que la première — son
    /// dialogue de saisie porte un champ et aucune case à cocher.
    pub case_sensitive: bool,
}

impl AirTextSearch {
    /// Une recherche insensible à la casse.
    #[must_use]
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            case_sensitive: false,
        }
    }

    /// La même recherche, avec la casse qui compte.
    #[must_use]
    pub fn case_sensitive(mut self) -> Self {
        self.case_sensitive = true;
        self
    }

    /// Vrai si cette recherche ne peut rien trouver.
    ///
    /// Un motif vide ne se trouve **nulle part**, plutôt que partout : la seconde réponse
    /// est défendable en théorie et inutilisable en pratique, puisqu'elle sélectionnerait
    /// une occurrence de largeur nulle à chaque frappe.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pattern.is_empty()
    }
}

/// La première occurrence du motif dans une ligne, à partir d'un décalage.
///
/// Rend le couple des décalages en octets, tous deux sur une frontière de grappe. Le
/// décalage de départ n'a pas besoin d'être sur une frontière : la recherche commence à la
/// première frontière qui ne le précède pas.
pub(crate) fn find_in_line(
    line: &str,
    from: usize,
    search: &AirTextSearch,
) -> Option<(usize, usize)> {
    if search.is_empty() {
        return None;
    }
    let mut offset: usize = 0;
    for cluster in graphemes(line) {
        if offset >= from
            && let Some(end) = match_at(line, offset, search)
        {
            return Some((offset, end));
        }
        offset = offset.saturating_add(cluster.len());
    }
    None
}

/// La fin de l'occurrence si le motif commence exactement à ce décalage.
fn match_at(line: &str, offset: usize, search: &AirTextSearch) -> Option<usize> {
    let mut remaining = line.get(offset..)?;
    let mut consumed = offset;
    for wanted in graphemes(&search.pattern) {
        let found = graphemes(remaining).next()?;
        if !same_cluster(found, wanted, search.case_sensitive) {
            return None;
        }
        consumed = consumed.saturating_add(found.len());
        remaining = remaining.get(found.len()..)?;
    }
    Some(consumed)
}

/// Vrai si deux grappes sont la même, à la règle de casse près.
///
/// L'égalité d'octets d'abord : c'est le cas courant, et il ne demande aucun repliement.
fn same_cluster(found: &str, wanted: &str, case_sensitive: bool) -> bool {
    if found == wanted {
        return true;
    }
    if case_sensitive {
        return false;
    }
    found
        .chars()
        .flat_map(char::to_lowercase)
        .eq(wanted.chars().flat_map(char::to_lowercase))
}

#[cfg(test)]
mod tests {
    use super::{AirTextSearch, find_in_line};

    #[test]
    fn un_motif_se_trouve_et_rend_ses_deux_bouts() {
        let found = find_in_line("une belle journee", 0, &AirTextSearch::new("belle"));
        assert_eq!(found, Some((4, 9)));
    }

    #[test]
    fn la_recherche_part_du_decalage_donne() {
        // Deux occurrences : la seconde est celle qu'on demande.
        let search = AirTextSearch::new("ab");
        assert_eq!(find_in_line("ab ab", 0, &search), Some((0, 2)));
        assert_eq!(find_in_line("ab ab", 1, &search), Some((3, 5)));
        assert_eq!(find_in_line("ab ab", 4, &search), None);
    }

    #[test]
    fn un_motif_absent_ne_se_trouve_pas() {
        assert_eq!(find_in_line("abc", 0, &AirTextSearch::new("abd")), None);
    }

    #[test]
    fn un_motif_plus_long_que_la_fin_de_ligne_ne_se_trouve_pas() {
        // La comparaison s'arrête faute de grappe, et non en débordant.
        assert_eq!(find_in_line("abc", 0, &AirTextSearch::new("abcd")), None);
    }

    #[test]
    fn un_motif_vide_ne_se_trouve_nulle_part() {
        // Plutôt que partout : une occurrence de largeur nulle serait inutilisable.
        assert!(AirTextSearch::new("").is_empty());
        assert_eq!(find_in_line("abc", 0, &AirTextSearch::new("")), None);
    }

    #[test]
    fn la_casse_est_ignoree_par_defaut_et_respectee_sur_demande() {
        let line = "Belle";
        assert_eq!(
            find_in_line(line, 0, &AirTextSearch::new("belle")),
            Some((0, 5))
        );
        assert_eq!(
            find_in_line(line, 0, &AirTextSearch::new("belle").case_sensitive()),
            None
        );
    }

    #[test]
    fn le_repliement_de_casse_passe_les_accents() {
        // `char::to_lowercase`, donc Unicode et pas seulement l'ASCII.
        // Cinq octets et non six : `É` en fait deux, `T` un seul.
        assert_eq!(
            find_in_line("ÉTÉ", 0, &AirTextSearch::new("été")),
            Some((0, 5))
        );
    }

    #[test]
    fn une_occurrence_ne_commence_pas_au_milieu_d_une_grappe() {
        // « é » décomposé : `e` suivi de U+0301. Chercher `e` ne trouve rien, parce que la
        // grappe affichée est un `é` — et qu'une sélection à l'intérieur d'une grappe est
        // une position que le curseur ne sait pas décrire.
        // « état » et non « étendu » : ce mot-là porte un second `e`, que la recherche
        // trouverait à juste titre — l'essai aurait échoué en accusant la bonne règle.
        let line = "e\u{301}tat";
        assert_eq!(find_in_line(line, 0, &AirTextSearch::new("e")), None);
        assert_eq!(
            find_in_line(line, 0, &AirTextSearch::new("e\u{301}")),
            Some((0, 3))
        );
    }

    #[test]
    fn une_occurrence_ne_finit_pas_au_milieu_d_une_grappe() {
        // Le pendant du précédent, et il n'est pas redondant : ici le motif **commence**
        // bien sur une frontière, et c'est sa fin qui tomberait dans la grappe suivante.
        let line = "ae\u{301}";
        assert_eq!(find_in_line(line, 0, &AirTextSearch::new("ae")), None);
    }

    #[test]
    fn un_motif_qui_traverse_les_lignes_ne_se_trouve_pas() {
        // Limite nommée : aucune ligne du tampon ne porte de saut de ligne.
        assert_eq!(find_in_line("abc", 0, &AirTextSearch::new("ab\nc")), None);
    }
}
