// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Unicode — largeur d'affichage et segmentation en graphèmes.
//!
//! # Pourquoi ce module existe
//!
//! `std` fournit `char`, `str` et la validation UTF-8, mais **pas** les deux propriétés
//! dont une interface texte ne peut pas se passer (ADR-006) :
//!
//! - la **largeur d'affichage**. Sans elle, tout alignement casse dès le premier
//!   idéogramme : un `漢` occupe deux colonnes là où un `a` en occupe une, et une bordure
//!   de fenêtre calculée en caractères se décale ;
//! - la **segmentation en graphèmes**. Sans elle, le curseur d'un éditeur coupe les
//!   séquences à jointure de largeur nulle : `Backspace` démembre un `👨‍👩‍👧‍👦`
//!   en quatre personnages au lieu de le supprimer.
//!
//! L'écosystème résout cela avec deux crates ; l'ADR-005 les exclut. Les tables sont donc
//! **générées depuis le référentiel Unicode et committées**, par
//! `cargo xtask generate-unicode-tables`.
//!
//! # La largeur d'une grappe, et pourquoi elle n'est pas une somme
//!
//! [`display_width`] rend la largeur d'un **caractère**. Mais un terminal n'affiche pas
//! des caractères, il affiche des grappes : `👨‍👩‍👧‍👦` est une seule grappe de sept
//! points de code, et elle occupe deux colonnes — pas huit.
//!
//! [`grapheme_width`] rend donc le **maximum** des largeurs de la grappe, et non leur
//! somme. C'est ce que font les terminaux modernes, et c'est la seule définition qui
//! garde le curseur et le rendu d'accord entre eux.

mod tables;
mod words;

use core::cmp::Ordering;

pub use tables::UNICODE_VERSION;
pub use words::{
    AirWordBreak, AirWords, is_word, next_word_start, previous_word_start, word_break, words,
};

use tables::{
    EXTENDED_PICTOGRAPHIC, GRAPHEME_BREAK, INDIC_CONSONANT, INDIC_EXTEND, INDIC_LINKER, WIDE,
    ZERO_WIDTH,
};

/// La propriété de coupure de graphème d'un caractère, au sens de l'annexe 29 d'Unicode.
///
/// Les noms sont écrits en entier : la norme abrège `L`, `LV`, `LVT` et `ZWJ`, la règle de
/// nommage d'air-tui ne l'admet pas (ADR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirGraphemeBreak {
    /// Aucune propriété particulière — le cas de la grande majorité des caractères.
    Other,
    /// Retour chariot.
    CarriageReturn,
    /// Saut de ligne.
    LineFeed,
    /// Caractère de commande.
    Control,
    /// Marque combinante, qui prolonge la grappe précédente.
    Extend,
    /// Jointure de largeur nulle, qui soude deux pictogrammes.
    ZeroWidthJoiner,
    /// Indicateur régional — deux de suite forment un drapeau.
    RegionalIndicator,
    /// Caractère qui se préfixe au suivant.
    Prepend,
    /// Marque d'espacement, qui prolonge la grappe précédente en occupant une colonne.
    SpacingMark,
    /// Jamo initial du hangûl.
    HangulLeading,
    /// Jamo médian du hangûl.
    HangulVowel,
    /// Jamo final du hangûl.
    HangulTrailing,
    /// Syllabe hangûl initiale et médiane.
    HangulLeadingVowel,
    /// Syllabe hangûl complète.
    HangulLeadingVowelTrailing,
}

/// Compare un point de code à une plage, pour la recherche binaire.
const fn compare_range(first: u32, last: u32, code: u32) -> Ordering {
    if code < first {
        Ordering::Greater
    } else if code > last {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

/// Vrai si le point de code appartient à l'une des plages.
fn is_in_ranges(ranges: &[(u32, u32)], code: u32) -> bool {
    ranges
        .binary_search_by(|(first, last)| compare_range(*first, *last, code))
        .is_ok()
}

/// La valeur inscrite dans une table de propriété énumérée, ou `default` hors des plages.
///
/// Générique sur la valeur : les deux segmentations de l'annexe 29 — graphèmes et mots —
/// ont la même forme de table et la même recherche binaire. L'écrire deux fois
/// garantirait qu'elles divergent.
fn value_in_table<Value: Copy>(table: &[(u32, u32, Value)], code: u32, default: Value) -> Value {
    let found = table
        .binary_search_by(|(first, last, _)| compare_range(*first, *last, code))
        .ok()
        .and_then(|index| table.get(index));
    match found {
        Some((_, _, value)) => *value,
        None => default,
    }
}

/// La propriété de coupure de graphème d'un caractère.
#[must_use]
pub fn grapheme_break(character: char) -> AirGraphemeBreak {
    value_in_table(
        GRAPHEME_BREAK,
        u32::from(character),
        AirGraphemeBreak::Other,
    )
}

/// Vrai si le caractère est pictographique au sens étendu.
///
/// Sert à la règle `GB11` de l'annexe 29 : deux pictogrammes soudés par une jointure de
/// largeur nulle forment une seule grappe.
#[must_use]
pub fn is_extended_pictographic(character: char) -> bool {
    is_in_ranges(EXTENDED_PICTOGRAPHIC, u32::from(character))
}

/// La classe d'un caractère pour la coupure conjointe indienne.
///
/// Elle porte la règle `GB9c` de l'annexe 29 : en devanagari et dans les écritures
/// voisines, un lieur soude ce qui le suit à ce qui le précède — `क्ष` est un seul signe à
/// l'écran, donc une seule grappe. Sans cette règle, le curseur coupe en son milieu.
///
/// # La règle a changé en Unicode 18.0.0
///
/// La révision 49 de l'annexe l'a réécrite en
/// `\p{InCB=Linker} \p{InCB=Extend}* × \p{InCB=Consonant}` : l'exigence d'une consonne
/// **avant** le lieur a disparu. Un virama en tête de texte soude donc désormais la
/// consonne qui le suit. Ce dépôt portait la formulation antérieure, et dix cas du fichier
/// d'essais officiel l'ont montré — voir `cargo xtask check-segmentation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirIndicConjunctBreak {
    /// Aucun rôle dans une conjointe.
    None,
    /// Une consonne.
    Consonant,
    /// Une marque qui prolonge sans lier.
    Extend,
    /// Un lieur — le virama et ses équivalents.
    Linker,
}

/// La classe de coupure conjointe indienne d'un caractère.
#[must_use]
pub fn indic_conjunct_break(character: char) -> AirIndicConjunctBreak {
    let code = u32::from(character);
    if is_in_ranges(INDIC_LINKER, code) {
        return AirIndicConjunctBreak::Linker;
    }
    if is_in_ranges(INDIC_EXTEND, code) {
        return AirIndicConjunctBreak::Extend;
    }
    if is_in_ranges(INDIC_CONSONANT, code) {
        return AirIndicConjunctBreak::Consonant;
    }
    AirIndicConjunctBreak::None
}

/// Le nombre de colonnes qu'occupe un caractère : zéro, une, ou deux.
///
/// Les caractères de commande rendent zéro : les compter serait décaler tout ce qui suit.
#[must_use]
pub fn display_width(character: char) -> usize {
    let code = u32::from(character);
    if character.is_control() {
        return 0;
    }
    if is_in_ranges(ZERO_WIDTH, code) {
        return 0;
    }
    if is_in_ranges(WIDE, code) {
        return 2;
    }
    1
}

/// Le nombre de colonnes qu'occupe une grappe de graphème.
///
/// Le **maximum** des largeurs de ses caractères, pas leur somme — voir la note du module.
#[must_use]
pub fn grapheme_width(cluster: &str) -> usize {
    cluster.chars().map(display_width).max().unwrap_or(0)
}

/// Le nombre de colonnes qu'occupe un texte.
#[must_use]
pub fn text_width(text: &str) -> usize {
    graphemes(text).map(grapheme_width).sum()
}

/// L'état de la règle `GB11` : a-t-on vu un pictogramme, puis des marques, puis une
/// jointure ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PictographicState {
    /// Rien de pictographique en cours.
    None,
    /// Un pictogramme vient d'être consommé, éventuellement suivi de marques.
    Pictographic,
    /// Un pictogramme, des marques, puis une jointure de largeur nulle.
    PictographicThenJoiner,
}

/// L'état de la règle `GB9c` : a-t-on vu un lieur, puis des prolongements ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConjunctState {
    /// Rien de conjoint en cours.
    None,
    /// Un lieur, éventuellement suivi de marques de prolongement conjointes.
    LinkerThenExtenders,
}

/// L'état accumulé pendant le parcours d'une grappe.
#[derive(Debug, Clone, Copy)]
struct ClusterState {
    /// Où en est la règle `GB11`.
    pictographic: PictographicState,
    /// Où en est la règle `GB9c`.
    conjunct: ConjunctState,
    /// Combien d'indicateurs régionaux consécutifs ont été consommés.
    regional_indicator_run: usize,
}

impl ClusterState {
    /// L'état après consommation du premier caractère d'une grappe.
    fn start(character: char) -> Self {
        let mut state = Self {
            pictographic: PictographicState::None,
            conjunct: ConjunctState::None,
            regional_indicator_run: 0,
        };
        state.consume(character);
        state
    }

    /// Met l'état à jour pour un caractère qui vient d'être consommé.
    fn consume(&mut self, character: char) {
        let property = grapheme_break(character);
        self.pictographic = if is_extended_pictographic(character) {
            PictographicState::Pictographic
        } else {
            match (self.pictographic, property) {
                (PictographicState::Pictographic, AirGraphemeBreak::Extend) => {
                    PictographicState::Pictographic
                }
                (PictographicState::Pictographic, AirGraphemeBreak::ZeroWidthJoiner) => {
                    PictographicState::PictographicThenJoiner
                }
                _ => PictographicState::None,
            }
        };
        // Un lieur ouvre toujours la conjointe, une marque de prolongement la conserve, et
        // tout le reste — une consonne comprise — la referme. Une consonne qui conserverait
        // l'état ferait souder deux consonnes séparées par un lieur **déjà consommé** deux
        // fois de suite.
        self.conjunct = match (self.conjunct, indic_conjunct_break(character)) {
            (_, AirIndicConjunctBreak::Linker)
            | (ConjunctState::LinkerThenExtenders, AirIndicConjunctBreak::Extend) => {
                ConjunctState::LinkerThenExtenders
            }
            _ => ConjunctState::None,
        };
        self.regional_indicator_run = if property == AirGraphemeBreak::RegionalIndicator {
            self.regional_indicator_run.saturating_add(1)
        } else {
            0
        };
    }
}

/// Vrai s'il faut couper entre `previous` et `current`.
///
/// L'ordre des règles est celui de l'annexe 29, et il n'est pas indifférent : `GB9` doit
/// passer avant `GB999`, sinon toute marque combinante ouvrirait une grappe.
fn breaks_between(previous: char, current: char, state: ClusterState) -> bool {
    let before = grapheme_break(previous);
    let after = grapheme_break(current);

    // `GB3` : un retour chariot et un saut de ligne ne se séparent pas.
    if before == AirGraphemeBreak::CarriageReturn && after == AirGraphemeBreak::LineFeed {
        return false;
    }
    // `GB4` et `GB5` : une commande coupe de part et d'autre.
    if matches!(
        before,
        AirGraphemeBreak::Control | AirGraphemeBreak::CarriageReturn | AirGraphemeBreak::LineFeed
    ) {
        return true;
    }
    if matches!(
        after,
        AirGraphemeBreak::Control | AirGraphemeBreak::CarriageReturn | AirGraphemeBreak::LineFeed
    ) {
        return true;
    }
    // `GB6`, `GB7`, `GB8` : les syllabes du hangûl restent entières. Un seul filtre sur le
    // couple, qui se relit comme la table de l'annexe au lieu de trois conditions.
    let hangul_continues = matches!(
        (before, after),
        (
            AirGraphemeBreak::HangulLeading,
            AirGraphemeBreak::HangulLeading
                | AirGraphemeBreak::HangulVowel
                | AirGraphemeBreak::HangulLeadingVowel
                | AirGraphemeBreak::HangulLeadingVowelTrailing
        ) | (
            AirGraphemeBreak::HangulLeadingVowel | AirGraphemeBreak::HangulVowel,
            AirGraphemeBreak::HangulVowel | AirGraphemeBreak::HangulTrailing
        ) | (
            AirGraphemeBreak::HangulLeadingVowelTrailing | AirGraphemeBreak::HangulTrailing,
            AirGraphemeBreak::HangulTrailing
        )
    );
    if hangul_continues {
        return false;
    }
    // `GB9` et `GB9a` : une marque prolonge la grappe.
    if matches!(
        after,
        AirGraphemeBreak::Extend
            | AirGraphemeBreak::ZeroWidthJoiner
            | AirGraphemeBreak::SpacingMark
    ) {
        return false;
    }
    // `GB9b` : un préfixe s'attache au suivant.
    if before == AirGraphemeBreak::Prepend {
        return false;
    }
    // `GB11` : deux pictogrammes soudés par une jointure ne font qu'une grappe.
    if state.pictographic == PictographicState::PictographicThenJoiner
        && is_extended_pictographic(current)
    {
        return false;
    }
    // `GB9c` : un lieur, des prolongements, puis une consonne forment une conjointe.
    if state.conjunct == ConjunctState::LinkerThenExtenders
        && indic_conjunct_break(current) == AirIndicConjunctBreak::Consonant
    {
        return false;
    }
    // `GB12` et `GB13` : les indicateurs régionaux vont par deux.
    if before == AirGraphemeBreak::RegionalIndicator
        && after == AirGraphemeBreak::RegionalIndicator
        && state.regional_indicator_run % 2 == 1
    {
        return false;
    }
    // `GB999` : partout ailleurs, on coupe.
    true
}

/// Détache la première grappe de `text` du reste.
///
/// Rend la grappe et ce qui la suit. La coupure se fait sur un décalage venu de
/// `char_indices`, donc sur une frontière de caractère par construction — ce qui évite à
/// la fois une option inatteignable et une arithmétique d'octets à la main.
fn split_first_cluster(text: &str) -> (&str, &str) {
    let mut indices = text.char_indices();
    let Some((_, first)) = indices.next() else {
        return ("", "");
    };
    let mut state = ClusterState::start(first);
    let mut previous = first;

    for (offset, current) in indices {
        if breaks_between(previous, current, state) {
            return text.split_at(offset);
        }
        state.consume(current);
        previous = current;
    }
    (text, "")
}

/// Le parcours des grappes de graphème d'un texte.
#[derive(Debug, Clone)]
pub struct AirGraphemes<'text> {
    /// Ce qu'il reste à parcourir.
    ///
    /// Une tranche plutôt qu'un indice : il n'y a rien à désynchroniser, et rien à
    /// reborner.
    remaining: &'text str,
}

impl<'text> Iterator for AirGraphemes<'text> {
    type Item = &'text str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let (cluster, rest) = split_first_cluster(self.remaining);
        self.remaining = rest;
        Some(cluster)
    }
}

/// Parcourt les grappes de graphème d'un texte.
///
/// C'est l'unité par laquelle un curseur se déplace, et par laquelle une suppression
/// opère : un caractère n'est pas une unité d'édition.
#[must_use]
pub fn graphemes(text: &str) -> AirGraphemes<'_> {
    AirGraphemes { remaining: text }
}

/// Le décalage de la frontière de grappe qui **précède** `offset`.
///
/// Un décalage déjà sur une frontière recule d'une grappe ; un décalage à zéro y reste.
/// C'est le pas de `Retour arrière` et de la flèche gauche : il porte à lui seul la
/// garantie qu'aucune touche ne coupe un `é` décomposé ni un émoji à jointures.
#[must_use]
pub fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    let mut boundary: usize = 0;
    let mut walked: usize = 0;
    for cluster in graphemes(text) {
        let next = walked.saturating_add(cluster.len());
        if next >= offset {
            return walked;
        }
        boundary = next;
        walked = next;
    }
    boundary
}

/// Le décalage de la frontière de grappe qui **suit** `offset`.
///
/// Un décalage à la fin du texte y reste.
#[must_use]
pub fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    let mut walked: usize = 0;
    for cluster in graphemes(text) {
        let next = walked.saturating_add(cluster.len());
        if next > offset {
            return next;
        }
        walked = next;
    }
    text.len()
}

/// La largeur affichée de la tranche de texte comprise entre deux décalages.
///
/// Une tranche hors du texte, ou à l'envers, vaut zéro colonne : le calcul d'une colonne
/// de curseur ne doit pas pouvoir échouer.
#[must_use]
pub fn width_between(text: &str, first: usize, last: usize) -> usize {
    match text.get(first..last) {
        Some(slice) => text_width(slice),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AirGraphemeBreak, AirIndicConjunctBreak, UNICODE_VERSION, display_width, grapheme_break,
        grapheme_width, graphemes, indic_conjunct_break, is_extended_pictographic, is_in_ranges,
        next_grapheme_boundary, previous_grapheme_boundary, split_first_cluster, text_width,
        value_in_table, width_between,
    };

    fn clusters(text: &str) -> Vec<&str> {
        graphemes(text).collect()
    }

    #[test]
    fn la_version_des_tables_est_inscrite() {
        // Pour qu'on sache toujours de quelle édition du référentiel les tables viennent.
        assert_eq!(UNICODE_VERSION, "18.0.0");
    }

    // ------------------------------------------------------ largeurs

    #[test]
    fn un_caractere_latin_occupe_une_colonne() {
        assert_eq!(display_width('a'), 1);
        assert_eq!(display_width('é'), 1);
        assert_eq!(display_width(' '), 1);
    }

    #[test]
    fn une_commande_n_occupe_aucune_colonne() {
        // Les compter décalerait tout ce qui suit.
        assert_eq!(display_width('\n'), 0);
        assert_eq!(display_width('\u{0}'), 0);
        assert_eq!(display_width('\u{7F}'), 0);
    }

    #[test]
    fn une_marque_combinante_n_occupe_aucune_colonne() {
        // U+0301 : accent aigu combinant.
        assert_eq!(display_width('\u{301}'), 0);
    }

    #[test]
    fn un_ideogramme_occupe_deux_colonnes() {
        assert_eq!(display_width('漢'), 2);
        assert_eq!(display_width('字'), 2);
        // Pleine chasse : le `Ａ` latin large.
        assert_eq!(display_width('\u{FF21}'), 2);
    }

    #[test]
    fn un_emoji_occupe_deux_colonnes() {
        assert_eq!(display_width('🦀'), 2);
        assert_eq!(display_width('😀'), 2);
    }

    #[test]
    fn un_symbole_a_presentation_textuelle_reste_sur_une_colonne() {
        // `☺` est pictographique au sens étendu, mais sa présentation par défaut est
        // textuelle : le compter comme large décalerait les bordures.
        assert!(is_extended_pictographic('\u{263A}'));
        assert_eq!(display_width('\u{263A}'), 1);
    }

    #[test]
    fn la_largeur_d_une_grappe_est_un_maximum_et_non_une_somme() {
        // `e` plus un accent combinant : une colonne, pas deux.
        assert_eq!(grapheme_width("e\u{301}"), 1);
        // Une famille soudée par des jointures : deux colonnes, pas huit.
        assert_eq!(grapheme_width("👨\u{200D}👩\u{200D}👧\u{200D}👦"), 2);
    }

    #[test]
    fn une_grappe_entierement_de_largeur_nulle_n_occupe_rien() {
        assert_eq!(grapheme_width("\u{301}"), 0);
        assert_eq!(grapheme_width(""), 0);
    }

    #[test]
    fn la_largeur_d_un_texte_somme_ses_grappes() {
        assert_eq!(text_width(""), 0);
        assert_eq!(text_width("abc"), 3);
        assert_eq!(text_width("漢字"), 4);
        assert_eq!(text_width("a漢b"), 4);
        // Le `e` accentué par composition compte pour une colonne.
        assert_eq!(text_width("cafe\u{301}"), 4);
        // Une famille entière compte pour deux.
        assert_eq!(text_width("👨\u{200D}👩\u{200D}👧\u{200D}👦"), 2);
    }

    // ------------------------------------------------------ graphèmes

    #[test]
    fn un_texte_vide_n_a_aucune_grappe() {
        assert_eq!(clusters(""), Vec::<&str>::new());
    }

    #[test]
    fn des_caracteres_latins_font_une_grappe_chacun() {
        assert_eq!(clusters("abc"), vec!["a", "b", "c"]);
    }

    #[test]
    fn une_marque_combinante_reste_avec_sa_base() {
        // `GB9`. C'est la règle qui empêche `Backspace` de laisser un accent orphelin.
        assert_eq!(clusters("e\u{301}f"), vec!["e\u{301}", "f"]);
    }

    #[test]
    fn plusieurs_marques_s_accumulent_dans_la_meme_grappe() {
        assert_eq!(clusters("e\u{301}\u{308}"), vec!["e\u{301}\u{308}"]);
    }

    #[test]
    fn un_retour_chariot_et_un_saut_de_ligne_ne_se_separent_pas() {
        // `GB3`.
        assert_eq!(clusters("\r\n"), vec!["\r\n"]);
        assert_eq!(clusters("a\r\nb"), vec!["a", "\r\n", "b"]);
    }

    #[test]
    fn un_saut_de_ligne_seul_est_sa_propre_grappe() {
        // `GB4` et `GB5`.
        assert_eq!(clusters("\n\n"), vec!["\n", "\n"]);
        assert_eq!(clusters("a\nb"), vec!["a", "\n", "b"]);
    }

    #[test]
    fn deux_indicateurs_regionaux_forment_un_drapeau() {
        // `GB12` et `GB13`. Le drapeau français : U+1F1EB U+1F1F7.
        let flag = "\u{1F1EB}\u{1F1F7}";
        assert_eq!(clusters(flag), vec![flag]);
    }

    #[test]
    fn quatre_indicateurs_regionaux_forment_deux_drapeaux() {
        // C'est tout l'objet de la parité : sans elle, quatre indicateurs feraient un
        // seul bloc et les deux drapeaux se confondraient.
        let two = "\u{1F1EB}\u{1F1F7}\u{1F1E9}\u{1F1EA}";
        assert_eq!(
            clusters(two),
            vec!["\u{1F1EB}\u{1F1F7}", "\u{1F1E9}\u{1F1EA}"]
        );
    }

    #[test]
    fn trois_indicateurs_regionaux_font_un_drapeau_et_un_reste() {
        let three = "\u{1F1EB}\u{1F1F7}\u{1F1E9}";
        assert_eq!(clusters(three), vec!["\u{1F1EB}\u{1F1F7}", "\u{1F1E9}"]);
    }

    #[test]
    fn une_famille_soudee_par_des_jointures_est_une_seule_grappe() {
        // `GB11`. Sans elle, `Backspace` démembrerait la famille en quatre personnages.
        let family = "👨\u{200D}👩\u{200D}👧\u{200D}👦";
        assert_eq!(clusters(family), vec![family]);
    }

    #[test]
    fn un_pictogramme_avec_modificateur_et_jointure_tient_ensemble() {
        // Une main avec teinte de peau, soudée : pictogramme, marque, jointure,
        // pictogramme.
        let sequence = "👍\u{1F3FD}\u{200D}👍";
        assert_eq!(clusters(sequence), vec![sequence]);
    }

    #[test]
    fn une_jointure_entre_deux_caracteres_non_pictographiques_ne_les_soude_pas() {
        // `GB9` attache la jointure au `a`, mais `GB11` ne s'applique pas : le `b` ouvre une
        // grappe. Souder ici serait inventer une règle.
        assert_eq!(clusters("a\u{200D}b"), vec!["a\u{200D}", "b"]);
    }

    #[test]
    fn une_syllabe_hangul_decomposee_est_une_seule_grappe() {
        // `GB6`, `GB7`, `GB8` : jamo initial, médian, final.
        let syllable = "\u{1112}\u{1161}\u{11AB}";
        assert_eq!(clusters(syllable), vec![syllable]);
    }

    #[test]
    fn une_syllabe_hangul_complete_suivie_d_un_final_tient_ensemble() {
        // `한` précomposé, de propriété LVT, suivi d'un jamo final.
        let sequence = "\u{D55C}\u{11AB}";
        assert_eq!(clusters(sequence), vec![sequence]);
    }

    #[test]
    fn deux_jamo_initiaux_restent_dans_la_meme_grappe() {
        let sequence = "\u{1112}\u{1112}";
        assert_eq!(clusters(sequence), vec![sequence]);
    }

    #[test]
    fn une_conjointe_devanagari_est_une_seule_grappe() {
        // `GB9c` : consonne, lieur, consonne. `क्ष` s'affiche comme un seul signe, donc le
        // curseur doit le traverser d'un coup.
        let conjunct = "\u{915}\u{94D}\u{937}";
        assert_eq!(clusters(conjunct), vec![conjunct]);
    }

    #[test]
    fn un_lieur_sans_consonne_devant_soude_quand_meme() {
        // La révision 49 de l'annexe, livrée avec Unicode 18.0.0, a retiré de `GB9c`
        // l'exigence d'une consonne avant le lieur. Ce cas est le n° 179 du fichier
        // d'essais officiel, et il échouait avant cette correction.
        let sequence = "\u{94D}\u{915}";
        assert_eq!(clusters(sequence), vec![sequence]);
    }

    #[test]
    fn un_lieur_precede_d_une_lettre_latine_soude_aussi() {
        // Cas n° 859 du fichier d'essais : `a`, virama, `त`. La consonne du début n'a plus
        // à être indienne — elle n'a plus à exister.
        let sequence = "a\u{94D}\u{924}";
        assert_eq!(clusters(sequence), vec![sequence]);
    }

    #[test]
    fn deux_consonnes_devanagari_sans_lieur_font_deux_grappes() {
        // C'est le lieur qui fait la conjointe, pas la proximité.
        assert_eq!(clusters("\u{915}\u{937}"), vec!["\u{915}", "\u{937}"]);
    }

    #[test]
    fn un_prefixe_s_attache_au_caractere_suivant() {
        // `GB9b` : le signe de nombre arabe U+0600.
        assert_eq!(clusters("\u{600}1"), vec!["\u{600}1"]);
    }

    #[test]
    fn une_marque_d_espacement_prolonge_la_grappe() {
        // `GB9a` : la voyelle devanagari U+093E occupe une colonne mais ne coupe pas.
        let sequence = "\u{915}\u{93E}";
        assert_eq!(clusters(sequence), vec![sequence]);
    }

    #[test]
    fn un_texte_melange_est_segmente_correctement() {
        let text = "a漢\u{1F1EB}\u{1F1F7}e\u{301}";
        assert_eq!(
            clusters(text),
            vec!["a", "漢", "\u{1F1EB}\u{1F1F7}", "e\u{301}"]
        );
    }

    #[test]
    fn la_concatenation_des_grappes_rend_le_texte_d_origine() {
        // L'invariant qui compte : la segmentation ne perd ni ne duplique rien.
        for text in [
            "",
            "abc",
            "e\u{301}f",
            "\r\n",
            "👨\u{200D}👩\u{200D}👧\u{200D}👦",
            "\u{1F1EB}\u{1F1F7}\u{1F1E9}\u{1F1EA}",
            "\u{915}\u{94D}\u{937}",
            "a\u{200D}b",
            "\u{600}1",
        ] {
            let rebuilt: String = graphemes(text).collect();
            assert_eq!(rebuilt, text, "segmentation lossy pour {text:?}");
        }
    }

    #[test]
    fn detacher_la_premiere_grappe_d_un_texte_vide_rend_deux_vides() {
        assert_eq!(split_first_cluster(""), ("", ""));
    }

    #[test]
    fn detacher_la_premiere_grappe_rend_la_grappe_et_le_reste() {
        assert_eq!(split_first_cluster("e\u{301}fg"), ("e\u{301}", "fg"));
        assert_eq!(split_first_cluster("a"), ("a", ""));
    }

    // ------------------------------------------- propriétés unitaires

    #[test]
    fn les_proprietes_de_coupure_sont_lues_dans_la_table() {
        assert_eq!(grapheme_break('a'), AirGraphemeBreak::Other);
        assert_eq!(grapheme_break('\r'), AirGraphemeBreak::CarriageReturn);
        assert_eq!(grapheme_break('\n'), AirGraphemeBreak::LineFeed);
        assert_eq!(grapheme_break('\u{0}'), AirGraphemeBreak::Control);
        assert_eq!(grapheme_break('\u{301}'), AirGraphemeBreak::Extend);
        assert_eq!(
            grapheme_break('\u{200D}'),
            AirGraphemeBreak::ZeroWidthJoiner
        );
        assert_eq!(
            grapheme_break('\u{1F1EB}'),
            AirGraphemeBreak::RegionalIndicator
        );
        assert_eq!(grapheme_break('\u{600}'), AirGraphemeBreak::Prepend);
        assert_eq!(grapheme_break('\u{93E}'), AirGraphemeBreak::SpacingMark);
        assert_eq!(grapheme_break('\u{1112}'), AirGraphemeBreak::HangulLeading);
        assert_eq!(grapheme_break('\u{1161}'), AirGraphemeBreak::HangulVowel);
        assert_eq!(grapheme_break('\u{11AB}'), AirGraphemeBreak::HangulTrailing);
        assert_eq!(
            grapheme_break('\u{D55C}'),
            AirGraphemeBreak::HangulLeadingVowelTrailing
        );
        assert_eq!(
            grapheme_break('\u{AC00}'),
            AirGraphemeBreak::HangulLeadingVowel
        );
    }

    #[test]
    fn la_classe_conjointe_indienne_est_lue_dans_les_trois_tables() {
        assert_eq!(
            indic_conjunct_break('\u{915}'),
            AirIndicConjunctBreak::Consonant
        );
        assert_eq!(
            indic_conjunct_break('\u{94D}'),
            AirIndicConjunctBreak::Linker
        );
        assert_eq!(
            indic_conjunct_break('\u{301}'),
            AirIndicConjunctBreak::Extend
        );
        assert_eq!(indic_conjunct_break('a'), AirIndicConjunctBreak::None);
    }

    #[test]
    fn le_caractere_pictographique_est_reconnu() {
        assert!(is_extended_pictographic('🦀'));
        assert!(!is_extended_pictographic('a'));
    }

    #[test]
    fn une_table_vide_ne_contient_aucun_point_de_code() {
        // Couvre la branche d'absence, que les tables réelles n'atteignent jamais pour un
        // point de code valide (ADR-012).
        assert!(!is_in_ranges(&[], 0x41));
        assert_eq!(
            value_in_table(&[], 0x41, AirGraphemeBreak::Other),
            AirGraphemeBreak::Other
        );
    }

    #[test]
    fn les_frontieres_de_grappe_encadrent_un_decalage() {
        // `a`, `e` + accent combinant, émoji : 1 + 3 + 4 octets.
        let text = "ae\u{301}\u{1F468}";
        assert_eq!(next_grapheme_boundary(text, 0), 1);
        assert_eq!(next_grapheme_boundary(text, 1), 4);
        assert_eq!(next_grapheme_boundary(text, 4), 8);
        assert_eq!(next_grapheme_boundary(text, 8), 8);
        assert_eq!(previous_grapheme_boundary(text, 8), 4);
        assert_eq!(previous_grapheme_boundary(text, 4), 1);
        assert_eq!(previous_grapheme_boundary(text, 1), 0);
        assert_eq!(previous_grapheme_boundary(text, 0), 0);
    }

    #[test]
    fn la_largeur_entre_deux_decalages_compte_les_colonnes() {
        let text = "a\u{6F22}b";
        // L'idéogramme occupe deux colonnes à lui seul.
        assert_eq!(width_between(text, 0, text.len()), 4);
        assert_eq!(width_between(text, 1, 4), 2);
    }

    #[test]
    fn une_tranche_impossible_vaut_zero_colonne() {
        // Un décalage hors du texte, et un intervalle à l'envers : ni l'un ni l'autre ne
        // doit faire échouer le calcul d'une colonne de curseur.
        assert_eq!(width_between("abc", 0, 99), 0);
        assert_eq!(width_between("abc", 2, 1), 0);
    }

    #[test]
    fn le_parcours_des_grappes_est_clonable_et_affichable() {
        let iterator = graphemes("ab");
        let cloned = iterator.clone();
        assert_eq!(cloned.count(), 2);
        assert!(format!("{:?}", graphemes("ab")).contains("AirGraphemes"));
    }
}
