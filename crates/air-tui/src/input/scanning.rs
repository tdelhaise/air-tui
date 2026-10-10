// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les briques de balayage du décodeur — des fonctions **totales**, chacune testable
//! isolément.
//!
//! Elles existent sous cette forme pour la raison qu'expose l'ADR-012 : une étape
//! partielle écrite en ligne dans le décodeur laisserait une branche inatteignable, donc
//! impossible à couvrir. Extraites et totales, elles se testent sur tout leur domaine —
//! parfois exhaustivement, le domaine d'un octet étant petit.

use crate::input::event::AirModifiers;

/// Le nombre d'octets d'une séquence UTF-8 dont le premier octet est `leading_byte`.
///
/// Rend `None` quand l'octet ne peut pas commencer une séquence : un octet de
/// continuation (`0x80..=0xBF`), un début surlong (`0xC0`, `0xC1`), ou un début hors du
/// domaine d'Unicode (`0xF5..=0xFF`).
pub const fn utf8_sequence_length(leading_byte: u8) -> Option<usize> {
    match leading_byte {
        0x00..=0x7F => Some(1),
        0xC2..=0xDF => Some(2),
        0xE0..=0xEF => Some(3),
        0xF0..=0xF4 => Some(4),
        _ => None,
    }
}

/// Lit un paramètre décimal de séquence de contrôle.
///
/// Un paramètre **vide** vaut `Some(None)` : c'est la valeur par défaut, que la norme
/// autorise et que les terminaux emploient (`CSI ; 5 A`). Un paramètre qui n'est pas
/// décimal, ou qui déborde d'un `u16`, rend `None` — c'est une séquence malformée, pas une
/// valeur par défaut, et le décodeur doit les distinguer.
pub fn parse_parameter(bytes: &[u8]) -> Option<Option<u16>> {
    if bytes.is_empty() {
        return Some(None);
    }
    let mut accumulated: u16 = 0;
    for byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => u16::from(byte.wrapping_sub(b'0')),
            _ => return None,
        };
        accumulated = accumulated.checked_mul(10)?.checked_add(digit)?;
    }
    Some(Some(accumulated))
}

/// Traduit le paramètre de modificateurs d'xterm en modificateurs nommés.
///
/// La convention est contre-intuitive et mérite d'être écrite : le paramètre vaut le
/// masque **plus un**. `1` signifie donc « aucun modificateur », `5` signifie
/// `1 + 4` c'est-à-dire contrôle seul.
///
/// Un paramètre nul est traité comme `1` : certains terminaux l'émettent, et il n'y a
/// qu'une lecture raisonnable.
pub const fn modifiers_from_parameter(parameter: u16) -> AirModifiers {
    let mask = parameter.saturating_sub(1);
    AirModifiers {
        shift: mask & 0b0001 != 0,
        alternate: mask & 0b0010 != 0,
        control: mask & 0b0100 != 0,
        super_key: mask & 0b1000 != 0,
    }
}

/// L'indice de la première occurrence de `needle` dans `haystack`, s'il y en a une.
pub fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    let last_start = haystack.len().saturating_sub(needle.len());
    for start in 0..=last_start {
        if haystack.get(start..start.saturating_add(needle.len())) == Some(needle) {
            return Some(start);
        }
    }
    None
}

/// La longueur du plus long suffixe de `haystack` qui soit un préfixe **strict** de
/// `needle`.
///
/// Sert à ne pas couper un terminateur en deux : si un morceau d'entrée se termine par
/// `\x1b[20`, ces quatre octets peuvent être le début de `\x1b[201~` et doivent être
/// conservés pour le morceau suivant.
pub fn partial_suffix_length(haystack: &[u8], needle: &[u8]) -> usize {
    let maximum = haystack.len().min(needle.len().saturating_sub(1));
    let mut length = maximum;
    while length > 0 {
        let start = haystack.len().saturating_sub(length);
        if haystack.get(start..) == needle.get(..length) {
            return length;
        }
        length = length.saturating_sub(1);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{
        find_subslice, modifiers_from_parameter, parse_parameter, partial_suffix_length,
        utf8_sequence_length,
    };

    #[test]
    fn la_longueur_utf8_couvre_tout_le_domaine_d_un_octet() {
        // Exhaustif : 256 valeurs, et la référence est la table de la norme.
        for byte in 0..=u8::MAX {
            let expected = match byte {
                0x00..=0x7F => Some(1),
                0xC2..=0xDF => Some(2),
                0xE0..=0xEF => Some(3),
                0xF0..=0xF4 => Some(4),
                _ => None,
            };
            assert_eq!(
                utf8_sequence_length(byte),
                expected,
                "pour l'octet {byte:#04x}"
            );
        }
    }

    #[test]
    fn les_octets_surlongs_et_hors_domaine_sont_refuses() {
        assert_eq!(utf8_sequence_length(0xC0), None);
        assert_eq!(utf8_sequence_length(0xC1), None);
        assert_eq!(utf8_sequence_length(0x80), None);
        assert_eq!(utf8_sequence_length(0xF5), None);
        assert_eq!(utf8_sequence_length(0xFF), None);
    }

    #[test]
    fn un_parametre_vide_vaut_la_valeur_par_defaut() {
        assert_eq!(parse_parameter(b""), Some(None));
    }

    #[test]
    fn un_parametre_decimal_est_lu() {
        assert_eq!(parse_parameter(b"0"), Some(Some(0)));
        assert_eq!(parse_parameter(b"5"), Some(Some(5)));
        assert_eq!(parse_parameter(b"201"), Some(Some(201)));
        assert_eq!(parse_parameter(b"65535"), Some(Some(65535)));
    }

    #[test]
    fn un_parametre_non_decimal_est_refuse() {
        assert_eq!(parse_parameter(b"1a"), None);
        assert_eq!(parse_parameter(b"-1"), None);
        assert_eq!(parse_parameter(b" 1"), None);
    }

    #[test]
    fn un_parametre_qui_deborde_est_refuse_et_non_tronque() {
        // 65536 ne tient pas dans un u16 : le refuser est le seul comportement qui ne
        // mente pas sur ce que le terminal a envoyé.
        assert_eq!(parse_parameter(b"65536"), None);
        assert_eq!(parse_parameter(b"999999"), None);
    }

    #[test]
    fn le_parametre_de_modificateurs_vaut_le_masque_plus_un() {
        assert!(modifiers_from_parameter(1).is_empty());
        assert!(modifiers_from_parameter(2).shift);
        assert!(modifiers_from_parameter(3).alternate);
        assert!(modifiers_from_parameter(5).control);
        assert!(modifiers_from_parameter(9).super_key);
    }

    #[test]
    fn les_modificateurs_se_combinent() {
        // 1 + 1 (shift) + 2 (alternate) + 4 (control) = 8
        let combined = modifiers_from_parameter(8);
        assert!(combined.shift && combined.alternate && combined.control);
        assert!(!combined.super_key);
    }

    #[test]
    fn un_parametre_de_modificateurs_nul_vaut_aucun_modificateur() {
        assert!(modifiers_from_parameter(0).is_empty());
    }

    #[test]
    fn tous_les_masques_de_quatre_bits_sont_couverts() {
        for mask in 0..16_u16 {
            let modifiers = modifiers_from_parameter(mask.saturating_add(1));
            assert_eq!(modifiers.shift, mask & 1 != 0, "masque {mask}");
            assert_eq!(modifiers.alternate, mask & 2 != 0, "masque {mask}");
            assert_eq!(modifiers.control, mask & 4 != 0, "masque {mask}");
            assert_eq!(modifiers.super_key, mask & 8 != 0, "masque {mask}");
        }
    }

    #[test]
    fn find_subslice_trouve_la_premiere_occurrence() {
        assert_eq!(find_subslice(b"abcabc", b"abc"), Some(0));
        assert_eq!(find_subslice(b"xxabc", b"abc"), Some(2));
        assert_eq!(find_subslice(b"abc", b"abc"), Some(0));
    }

    #[test]
    fn find_subslice_rend_rien_quand_l_aiguille_est_absente() {
        assert_eq!(find_subslice(b"abc", b"xyz"), None);
        assert_eq!(find_subslice(b"ab", b"abc"), None);
        assert_eq!(find_subslice(b"", b"a"), None);
    }

    #[test]
    fn find_subslice_d_une_aiguille_vide_est_au_debut() {
        assert_eq!(find_subslice(b"abc", b""), Some(0));
    }

    #[test]
    fn partial_suffix_length_detecte_un_terminateur_coupe() {
        assert_eq!(partial_suffix_length(b"texte\x1b[20", b"\x1b[201~"), 4);
        assert_eq!(partial_suffix_length(b"texte\x1b", b"\x1b[201~"), 1);
        assert_eq!(partial_suffix_length(b"texte", b"\x1b[201~"), 0);
    }

    #[test]
    fn partial_suffix_length_ne_rend_jamais_l_aiguille_entiere() {
        // Un terminateur complet relève de `find_subslice`, pas d'un suffixe partiel :
        // rendre sa longueur entière ferait boucler l'appelant.
        assert_eq!(partial_suffix_length(b"\x1b[201~", b"\x1b[201~"), 0);
    }

    #[test]
    fn partial_suffix_length_sur_une_entree_plus_courte_que_l_aiguille() {
        assert_eq!(partial_suffix_length(b"\x1b[", b"\x1b[201~"), 2);
        assert_eq!(partial_suffix_length(b"", b"\x1b[201~"), 0);
    }
}
