// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La segmentation en mots de l'annexe 29 d'Unicode.
//!
//! # Pourquoi ce n'est pas « couper aux espaces »
//!
//! Un éditeur doit déplacer le curseur de mot en mot, et la règle naïve — couper sur les
//! espaces — se trompe partout où le texte n'est pas anglais :
//!
//! - `l'enfant` et `can't` sont **un** seul segment chacun : une apostrophe entre deux
//!   lettres ne coupe pas (`WB6` et `WB7`), alors qu'une coupure sur la ponctuation les
//!   découperait en trois ;
//! - `3,14` est **un** seul segment : la virgule décimale ne coupe pas un nombre, là où
//!   la même virgule dans `a,b` coupe (règles `WB11` et `WB12`) ;
//! - `日本語` est **trois** segments sans un seul espace, parce que chaque idéogramme est
//!   une unité de déplacement ;
//! - `カタカナ` est **un** segment, à l'inverse des idéogrammes qui le précèdent dans
//!   cette liste (`WB13`).
//!
//! Ces quatre comportements ne sortent pas d'une heuristique : ce sont les règles `WB1` à
//! `WB999` de l'annexe 29, appliquées dans leur ordre, sur la propriété `Word_Break`
//! d'une table générée depuis le référentiel (ADR-006).
//!
//! # Deux segmentations, deux usages
//!
//! La segmentation en **grappes** ([`crate::unicode::graphemes`]) donne l'unité
//! d'effacement et de déplacement d'une touche. La segmentation en **mots** donne l'unité
//! de `Ctrl`+flèche. Elles sont indépendantes et ne se déduisent pas l'une de l'autre.
//!
//! # Le repliement de la règle `WB4`, et pourquoi il complique tout
//!
//! `WB4` dit qu'un caractère de prolongement, de formatage ou une jointure **disparaît**
//! derrière le caractère qui le précède : `a<ZWJ>b` se décide comme `ab`. Les règles
//! suivantes ne raisonnent donc pas sur le caractère précédent, mais sur le dernier
//! caractère **non ignorable** — et certaines, comme `WB7`, sur celui d'avant encore.
//!
//! D'où l'état porté par [`WordState`] : deux propriétés repliées en arrière, la propriété
//! brute du caractère précédent — nécessaire aux règles `WB3` à `WB3d`, qui s'appliquent
//! avant le repliement —, et le compte d'indicateurs régionaux consécutifs. Le
//! regard en avant, lui, est borné : un caractère non ignorable, pas plus.

use crate::unicode::tables::WORD_BREAK;
use crate::unicode::{is_extended_pictographic, value_in_table};

/// La propriété de coupure de mot d'un caractère, au sens de l'annexe 29 d'Unicode.
///
/// Les noms sont écrits en entier : la norme abrège `ALetter`, `MidNumLet`, `WSegSpace` et
/// `ZWJ`, la règle de nommage d'air-tui ne l'admet pas (ADR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirWordBreak {
    /// Aucune propriété particulière — dont les idéogrammes, que l'annexe découpe un à un.
    Other,
    /// Retour chariot.
    CarriageReturn,
    /// Saut de ligne.
    LineFeed,
    /// Autre caractère de passage à la ligne.
    Newline,
    /// Marque combinante, qui prolonge le caractère précédent.
    Extend,
    /// Jointure de largeur nulle.
    ZeroWidthJoiner,
    /// Indicateur régional — deux de suite forment un drapeau.
    RegionalIndicator,
    /// Caractère de formatage, invisible et sans effet sur la coupure.
    Format,
    /// Katakana, que l'annexe garde en un seul segment.
    Katakana,
    /// Lettre hébraïque, qui a ses propres règles de guillemet.
    HebrewLetter,
    /// Lettre alphabétique ordinaire.
    AlphabeticLetter,
    /// Apostrophe simple.
    SingleQuote,
    /// Guillemet double.
    DoubleQuote,
    /// Caractère médian admis entre deux chiffres **ou** entre deux lettres.
    MiddleNumericOrLetter,
    /// Caractère médian admis entre deux lettres.
    MiddleLetter,
    /// Caractère médian admis entre deux chiffres.
    MiddleNumeric,
    /// Chiffre.
    Numeric,
    /// Caractère qui relie un mot au suivant, comme le souligné.
    ExtendNumericOrLetter,
    /// Espace de segmentation, que l'annexe garde groupé avec ses voisins.
    WordSegmentationSpace,
}

/// La propriété de coupure de mot d'un caractère.
#[must_use]
pub fn word_break(character: char) -> AirWordBreak {
    value_in_table(WORD_BREAK, u32::from(character), AirWordBreak::Other)
}

/// Vrai pour `AHLetter` : une lettre alphabétique ou hébraïque.
///
/// L'annexe emploie ce raccourci dans six règles ; l'écrire une fois évite de répéter la
/// disjonction et de se tromper sur l'une d'elles.
const fn is_alphabetic_or_hebrew(property: AirWordBreak) -> bool {
    matches!(
        property,
        AirWordBreak::AlphabeticLetter | AirWordBreak::HebrewLetter
    )
}

/// Vrai pour `MidNumLetQ` : un médian mixte ou une apostrophe.
const fn is_middle_mixed_or_quote(property: AirWordBreak) -> bool {
    matches!(
        property,
        AirWordBreak::MiddleNumericOrLetter | AirWordBreak::SingleQuote
    )
}

/// Vrai pour les caractères que la règle `WB4` replie derrière leur prédécesseur.
const fn is_ignorable(property: AirWordBreak) -> bool {
    matches!(
        property,
        AirWordBreak::Extend | AirWordBreak::Format | AirWordBreak::ZeroWidthJoiner
    )
}

/// Vrai pour les trois propriétés de passage à la ligne, que `WB3a` et `WB3b` isolent.
const fn is_newline_like(property: AirWordBreak) -> bool {
    matches!(
        property,
        AirWordBreak::CarriageReturn | AirWordBreak::LineFeed | AirWordBreak::Newline
    )
}

/// L'état accumulé pendant le parcours, tel que les règles le réclament.
#[derive(Debug, Clone, Copy)]
struct WordState {
    /// La propriété du caractère précédent, **sans repliement** — pour `WB3` à `WB3d`.
    previous: AirWordBreak,
    /// La propriété du dernier caractère non ignorable, ou `None` au début du texte.
    folded_previous: Option<AirWordBreak>,
    /// Celle du non ignorable d'avant — `WB7`, `WB7c` et `WB11` la réclament.
    folded_previous_previous: Option<AirWordBreak>,
    /// Combien d'indicateurs régionaux consécutifs ont été consommés.
    regional_indicator_run: usize,
}

impl WordState {
    /// L'état au début d'un texte : rien n'a été vu.
    const fn start() -> Self {
        Self {
            previous: AirWordBreak::Other,
            folded_previous: None,
            folded_previous_previous: None,
            regional_indicator_run: 0,
        }
    }

    /// Met l'état à jour pour un caractère qui vient d'être consommé.
    ///
    /// Le repliement de `WB4` est **conditionnel** : la règle exclut explicitement le
    /// début de texte et les passages à la ligne. Une marque combinante placée juste
    /// après un saut de ligne ne se replie donc derrière rien — elle devient elle-même le
    /// caractère de référence, et forme un segment à elle seule.
    fn consume(&mut self, property: AirWordBreak) {
        let foldable = match self.folded_previous {
            Some(previous) => !is_newline_like(previous),
            None => false,
        };
        let folded_away = is_ignorable(property) && foldable;
        if !folded_away {
            self.folded_previous_previous = self.folded_previous;
            self.folded_previous = Some(property);
        }
        self.regional_indicator_run = if property == AirWordBreak::RegionalIndicator {
            self.regional_indicator_run.saturating_add(1)
        } else if folded_away {
            self.regional_indicator_run
        } else {
            0
        };
        self.previous = property;
    }
}

/// La propriété du premier caractère non ignorable qui suit `current` dans `tail`.
///
/// `tail` commence **par** `current`. Rend `None` à la fin du texte. C'est tout le regard
/// en avant dont les règles ont besoin : `WB6`, `WB7b` et `WB12` ne regardent qu'un
/// caractère au-delà de la frontière examinée.
fn folded_after(tail: &str, current: char) -> Option<AirWordBreak> {
    let rest = tail.get(current.len_utf8()..)?;
    rest.chars()
        .map(word_break)
        .find(|property| !is_ignorable(*property))
}

/// Vrai s'il faut couper avant `current`.
///
/// L'ordre des règles est celui de l'annexe, et il n'est pas indifférent : `WB4` doit
/// passer avant `WB5`, sinon une marque combinante couperait un mot en deux ; et `WB3a`
/// doit passer avant `WB4`, sinon une marque placée après un retour chariot se replierait
/// derrière lui.
#[allow(
    clippy::too_many_lines,
    reason = "une règle de l'annexe par bloc, dans son ordre — les regrouper les rendrait invérifiables"
)]
fn breaks_before(
    state: &WordState,
    current: AirWordBreak,
    current_is_pictographic: bool,
    next: Option<AirWordBreak>,
) -> bool {
    let Some(folded) = state.folded_previous else {
        // `WB1` : on coupe au début du texte. L'itérateur ne passe pas par là, mais une
        // réponse muette serait pire qu'une réponse juste.
        return true;
    };
    let previous = state.previous;

    // `WB3` : un retour chariot et un saut de ligne ne se séparent pas.
    if previous == AirWordBreak::CarriageReturn && current == AirWordBreak::LineFeed {
        return false;
    }
    // `WB3a` et `WB3b` : un passage à la ligne coupe de part et d'autre.
    if is_newline_like(previous) || is_newline_like(current) {
        return true;
    }
    // `WB3c` : une jointure soude ce qui la suit, s'il est pictographique.
    if previous == AirWordBreak::ZeroWidthJoiner && current_is_pictographic {
        return false;
    }
    // `WB3d` : deux espaces de segmentation restent ensemble.
    if previous == AirWordBreak::WordSegmentationSpace
        && current == AirWordBreak::WordSegmentationSpace
    {
        return false;
    }
    // `WB4` : un prolongement, un formatage ou une jointure se replie derrière le
    // caractère précédent — donc aucune coupure devant lui.
    if is_ignorable(current) {
        return false;
    }

    // À partir d'ici, les règles raisonnent sur la suite **repliée**.
    let before_before = state.folded_previous_previous;

    // `WB5` : deux lettres restent ensemble.
    if is_alphabetic_or_hebrew(folded) && is_alphabetic_or_hebrew(current) {
        return false;
    }
    // `WB6` et `WB7` : un médian entre deux lettres ne coupe ni devant ni derrière. C'est
    // ce qui garde `can't` et `l'enfant` entiers.
    if is_alphabetic_or_hebrew(folded)
        && (current == AirWordBreak::MiddleLetter || is_middle_mixed_or_quote(current))
        && next.is_some_and(is_alphabetic_or_hebrew)
    {
        return false;
    }
    if (folded == AirWordBreak::MiddleLetter || is_middle_mixed_or_quote(folded))
        && is_alphabetic_or_hebrew(current)
        && before_before.is_some_and(is_alphabetic_or_hebrew)
    {
        return false;
    }
    // `WB7a`, `WB7b` et `WB7c` : les guillemets de l'hébreu.
    if folded == AirWordBreak::HebrewLetter && current == AirWordBreak::SingleQuote {
        return false;
    }
    if folded == AirWordBreak::HebrewLetter
        && current == AirWordBreak::DoubleQuote
        && next == Some(AirWordBreak::HebrewLetter)
    {
        return false;
    }
    if folded == AirWordBreak::DoubleQuote
        && current == AirWordBreak::HebrewLetter
        && before_before == Some(AirWordBreak::HebrewLetter)
    {
        return false;
    }
    // `WB8` à `WB10` : chiffres et lettres se mêlent sans couper.
    if folded == AirWordBreak::Numeric && current == AirWordBreak::Numeric {
        return false;
    }
    if is_alphabetic_or_hebrew(folded) && current == AirWordBreak::Numeric {
        return false;
    }
    if folded == AirWordBreak::Numeric && is_alphabetic_or_hebrew(current) {
        return false;
    }
    // `WB11` et `WB12` : un médian numérique entre deux chiffres ne coupe pas. C'est ce
    // qui garde `3,14` entier là où `a,b` se coupe.
    if (folded == AirWordBreak::MiddleNumeric || is_middle_mixed_or_quote(folded))
        && current == AirWordBreak::Numeric
        && before_before == Some(AirWordBreak::Numeric)
    {
        return false;
    }
    if folded == AirWordBreak::Numeric
        && (current == AirWordBreak::MiddleNumeric || is_middle_mixed_or_quote(current))
        && next == Some(AirWordBreak::Numeric)
    {
        return false;
    }
    // `WB13` : les katakana restent groupés.
    if folded == AirWordBreak::Katakana && current == AirWordBreak::Katakana {
        return false;
    }
    // `WB13a` et `WB13b` : le liant — le souligné — s'attache des deux côtés.
    let joinable = |property: AirWordBreak| {
        is_alphabetic_or_hebrew(property)
            || matches!(property, AirWordBreak::Numeric | AirWordBreak::Katakana)
    };
    if (joinable(folded) || folded == AirWordBreak::ExtendNumericOrLetter)
        && current == AirWordBreak::ExtendNumericOrLetter
    {
        return false;
    }
    if folded == AirWordBreak::ExtendNumericOrLetter && joinable(current) {
        return false;
    }
    // `WB15` et `WB16` : les indicateurs régionaux vont par deux.
    if folded == AirWordBreak::RegionalIndicator
        && current == AirWordBreak::RegionalIndicator
        && state.regional_indicator_run % 2 == 1
    {
        return false;
    }
    // `WB999` : partout ailleurs, on coupe.
    true
}

/// Le parcours des segments de mot d'un texte.
///
/// L'état des règles est porté par l'itérateur et traverse les frontières : `WB7` regarde
/// deux caractères en arrière, donc au-delà du segment qu'elle vient de fermer.
#[derive(Debug, Clone)]
pub struct AirWords<'text> {
    /// Le texte entier — les règles regardent de part et d'autre d'une frontière.
    text: &'text str,
    /// Où commence le prochain segment, en octets.
    offset: usize,
    /// L'état des règles au point `offset`.
    state: WordState,
}

impl<'text> Iterator for AirWords<'text> {
    type Item = &'text str;

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.offset;
        let first = self.text.get(start..)?.chars().next()?;
        // Le premier caractère d'un segment est consommé sans décision : la frontière qui
        // le précède vient d'être posée, ou c'est le début du texte (`WB1`).
        self.state.consume(word_break(first));
        self.offset = start.saturating_add(first.len_utf8());

        while let Some(tail) = self.text.get(self.offset..) {
            let Some(current) = tail.chars().next() else {
                break;
            };
            let property = word_break(current);
            if breaks_before(
                &self.state,
                property,
                is_extended_pictographic(current),
                folded_after(tail, current),
            ) {
                break;
            }
            self.state.consume(property);
            self.offset = self.offset.saturating_add(current.len_utf8());
        }
        self.text.get(start..self.offset)
    }
}

/// Parcourt les segments de mot d'un texte.
///
/// Les segments sont **contigus** et couvrent tout le texte : un espace, une ponctuation
/// et un mot sont chacun un segment. [`is_word`] distingue les mots du reste.
#[must_use]
pub fn words(text: &str) -> AirWords<'_> {
    AirWords {
        text,
        offset: 0,
        state: WordState::start(),
    }
}

/// Vrai si le segment est un **mot** et non une séparation.
///
/// Les propriétés de lettre, de chiffre, de katakana et de liant en font un mot. Pour le
/// reste — la propriété `Other`, qui couvre les idéogrammes autant que la ponctuation —
/// c'est `char::is_alphanumeric` de `std` qui tranche : il porte déjà la catégorie
/// générale d'Unicode, et la réimporter serait une table de plus à tenir à jour.
#[must_use]
pub fn is_word(segment: &str) -> bool {
    segment.chars().any(|character| {
        matches!(
            word_break(character),
            AirWordBreak::AlphabeticLetter
                | AirWordBreak::HebrewLetter
                | AirWordBreak::Numeric
                | AirWordBreak::Katakana
                | AirWordBreak::ExtendNumericOrLetter
        ) || character.is_alphanumeric()
    })
}

/// Le décalage du début du mot qui suit `offset`, ou la fin du texte.
///
/// C'est `Ctrl`+flèche droite : on saute les séparations et on s'arrête au début du mot
/// suivant, jamais à sa fin — c'est le geste de Turbo Vision.
#[must_use]
pub fn next_word_start(text: &str, offset: usize) -> usize {
    let mut walked: usize = 0;
    for segment in words(text) {
        if walked > offset && is_word(segment) {
            return walked;
        }
        walked = walked.saturating_add(segment.len());
    }
    text.len()
}

/// Le décalage du début du mot qui précède `offset`, ou zéro.
#[must_use]
pub fn previous_word_start(text: &str, offset: usize) -> usize {
    let mut walked: usize = 0;
    let mut found: usize = 0;
    for segment in words(text) {
        if walked >= offset {
            break;
        }
        if is_word(segment) {
            found = walked;
        }
        walked = walked.saturating_add(segment.len());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{AirWordBreak, is_word, next_word_start, previous_word_start, word_break, words};

    fn segments(text: &str) -> Vec<&str> {
        words(text).collect()
    }

    #[test]
    fn les_segments_couvrent_le_texte_sans_trou_ni_recouvrement() {
        let text = "Le chat, 3,14 fois.";
        assert_eq!(segments(text).concat(), text);
    }

    #[test]
    fn un_texte_vide_ne_rend_aucun_segment() {
        assert_eq!(segments(""), [] as [&str; 0]);
    }

    #[test]
    fn deux_mots_separes_par_un_espace_font_trois_segments() {
        assert_eq!(segments("le chat"), vec!["le", " ", "chat"]);
    }

    #[test]
    fn une_apostrophe_entre_deux_lettres_ne_coupe_pas() {
        // `WB6` et `WB7` : c'est le cas qui rend la coupure naïve inacceptable en anglais.
        assert_eq!(segments("can't"), vec!["can't"]);
    }

    #[test]
    fn une_apostrophe_francaise_ne_coupe_pas_non_plus() {
        // Même paire de règles qu'en anglais : une seule lettre suffit de chaque côté du
        // médian. Une coupure sur la ponctuation donnerait trois segments ici.
        assert_eq!(segments("l'enfant"), vec!["l'enfant"]);
    }

    #[test]
    fn une_virgule_decimale_ne_coupe_pas_un_nombre() {
        // `WB11` et `WB12`.
        assert_eq!(segments("3,14"), vec!["3,14"]);
    }

    #[test]
    fn la_meme_virgule_coupe_entre_deux_lettres() {
        // La virgule est `MiddleNumeric` : elle ne relie que des chiffres. C'est la preuve
        // que la propriété est lue, et non devinée.
        assert_eq!(segments("a,b"), vec!["a", ",", "b"]);
    }

    #[test]
    fn un_point_entre_deux_lettres_ne_coupe_pas() {
        // Le point est `MiddleNumericOrLetter` : il relie les deux.
        assert_eq!(segments("a.b"), vec!["a.b"]);
    }

    #[test]
    fn un_ideogramme_est_un_segment_a_lui_seul() {
        // Aucun espace, et pourtant trois unités de déplacement.
        assert_eq!(segments("日本語"), vec!["日", "本", "語"]);
    }

    #[test]
    fn les_katakana_restent_groupes() {
        // `WB13` — à l'inverse des idéogrammes.
        assert_eq!(segments("カタカナ"), vec!["カタカナ"]);
    }

    #[test]
    fn un_souligne_relie_deux_mots() {
        // `WB13a` et `WB13b` : c'est ce qui garde un identifiant entier.
        assert_eq!(segments("air_tui"), vec!["air_tui"]);
    }

    #[test]
    fn un_chiffre_collé_a_une_lettre_ne_coupe_pas() {
        // `WB9` et `WB10`.
        assert_eq!(segments("air2tui"), vec!["air2tui"]);
    }

    #[test]
    fn un_retour_chariot_suivi_d_un_saut_de_ligne_reste_entier() {
        // `WB3`.
        assert_eq!(segments("a\r\nb"), vec!["a", "\r\n", "b"]);
    }

    #[test]
    fn un_saut_de_ligne_seul_coupe_des_deux_cotes() {
        // `WB3a` et `WB3b`.
        assert_eq!(segments("a\nb"), vec!["a", "\n", "b"]);
    }

    #[test]
    fn une_marque_combinante_ne_coupe_pas_un_mot() {
        // `WB4` : `e` suivi de l'accent aigu combinant reste dans le mot.
        assert_eq!(segments("cafe\u{301}"), vec!["cafe\u{301}"]);
    }

    #[test]
    fn une_marque_combinante_apres_un_saut_de_ligne_est_un_segment_a_elle_seule() {
        // `WB4` exclut les passages à la ligne : la marque ne se replie derrière rien,
        // donc elle ouvre son propre segment — et rien ne la rattache à la lettre qui
        // suit, puisque `WB4` ne replie que vers l'arrière.
        assert_eq!(segments("\n\u{301}a"), vec!["\n", "\u{301}", "a"]);
    }

    #[test]
    fn deux_drapeaux_font_deux_segments() {
        // `WB15` et `WB16` : les indicateurs régionaux vont par deux, donc quatre
        // indicateurs font deux segments et non un ni quatre.
        assert_eq!(segments("\u{1F1EB}\u{1F1F7}\u{1F1E9}\u{1F1EA}").len(), 2);
    }

    #[test]
    fn trois_indicateurs_regionaux_font_deux_segments() {
        // Le troisième est seul : il ouvre un segment.
        assert_eq!(segments("\u{1F1EB}\u{1F1F7}\u{1F1E9}").len(), 2);
    }

    #[test]
    fn une_jointure_soude_deux_pictogrammes() {
        // `WB3c`.
        assert_eq!(segments("\u{1F468}\u{200D}\u{1F469}").len(), 1);
    }

    #[test]
    fn les_espaces_consecutifs_restent_groupes() {
        // `WB3d`.
        assert_eq!(segments("a   b"), vec!["a", "   ", "b"]);
    }

    #[test]
    fn un_guillemet_hebraique_entre_deux_lettres_ne_coupe_pas() {
        // `WB7b` et `WB7c` : la règle existe pour l'hébreu seul, et elle est testée pour
        // qu'une relecture ne la prenne pas pour du bruit.
        assert_eq!(segments("\u{05D0}\u{0022}\u{05D1}").len(), 1);
    }

    #[test]
    fn un_mot_se_distingue_d_une_separation() {
        assert!(is_word("chat"));
        assert!(is_word("3,14"));
        assert!(is_word("日"));
        assert!(!is_word(" "));
        assert!(!is_word(","));
        assert!(!is_word("\n"));
    }

    #[test]
    fn le_mot_suivant_saute_la_ponctuation() {
        let text = "le chat, noir";
        assert_eq!(next_word_start(text, 0), 3);
        assert_eq!(next_word_start(text, 3), 9);
    }

    #[test]
    fn le_mot_suivant_du_dernier_mot_est_la_fin_du_texte() {
        let text = "le chat";
        assert_eq!(next_word_start(text, 3), text.len());
    }

    #[test]
    fn le_mot_precedent_recule_jusqu_au_debut_du_mot() {
        let text = "le chat, noir";
        assert_eq!(previous_word_start(text, 13), 9);
        assert_eq!(previous_word_start(text, 9), 3);
        assert_eq!(previous_word_start(text, 3), 0);
        assert_eq!(previous_word_start(text, 0), 0);
    }

    #[test]
    fn le_mot_precedent_depuis_le_milieu_d_un_mot_rend_son_debut() {
        let text = "le chat";
        assert_eq!(previous_word_start(text, 5), 3);
    }

    #[test]
    fn les_proprietes_lues_sont_celles_du_referentiel() {
        assert_eq!(word_break('a'), AirWordBreak::AlphabeticLetter);
        assert_eq!(word_break('3'), AirWordBreak::Numeric);
        assert_eq!(word_break(','), AirWordBreak::MiddleNumeric);
        assert_eq!(word_break('.'), AirWordBreak::MiddleNumericOrLetter);
        assert_eq!(word_break(':'), AirWordBreak::MiddleLetter);
        assert_eq!(word_break('\''), AirWordBreak::SingleQuote);
        assert_eq!(word_break('"'), AirWordBreak::DoubleQuote);
        assert_eq!(word_break('_'), AirWordBreak::ExtendNumericOrLetter);
        assert_eq!(word_break(' '), AirWordBreak::WordSegmentationSpace);
        assert_eq!(word_break('\r'), AirWordBreak::CarriageReturn);
        assert_eq!(word_break('\n'), AirWordBreak::LineFeed);
        assert_eq!(word_break('\u{0B}'), AirWordBreak::Newline);
        assert_eq!(word_break('\u{200D}'), AirWordBreak::ZeroWidthJoiner);
        assert_eq!(word_break('\u{0301}'), AirWordBreak::Extend);
        assert_eq!(word_break('\u{00AD}'), AirWordBreak::Format);
        assert_eq!(word_break('\u{1F1EB}'), AirWordBreak::RegionalIndicator);
        assert_eq!(word_break('カ'), AirWordBreak::Katakana);
        assert_eq!(word_break('\u{05D0}'), AirWordBreak::HebrewLetter);
        assert_eq!(word_break('日'), AirWordBreak::Other);
    }
}
