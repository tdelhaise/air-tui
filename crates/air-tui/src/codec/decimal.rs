// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'écriture d'un entier en chiffres ASCII.
//!
//! # Pourquoi ne pas employer `write!`
//!
//! Trois raisons, dans l'ordre d'importance : `write!` sur un `Vec<u8>` rend un
//! `Result<(), std::io::Error>` qui ne peut jamais être une erreur, ce qui obligerait soit
//! à un `unwrap` — interdit — soit à propager une erreur impossible jusqu'au sommet ; il
//! traîne la machinerie de formatage dans le chemin le plus chaud du rendu ; et il est
//! plus lent qu'une division sur un `u16`.
//!
//! # Pourquoi ces deux fonctions auxiliaires
//!
//! La règle des codecs impose 100 % de couverture (ADR-012), **branches comprises**. Une
//! étape infaillible écrite en ligne — un `unwrap_or` sur une conversion qui ne peut pas
//! échouer, un `if let` sur un index toujours valide — laisse une branche que rien ne peut
//! atteindre, donc impossible à couvrir.
//!
//! Chaque étape partielle est donc extraite dans une fonction **totale**, dont le cas
//! dégénéré est inatteignable depuis l'appelant mais directement testable. La couverture
//! cesse d'être une pénitence et redevient ce qu'elle doit être : la preuve que chaque
//! chemin a été pensé.

/// Les puissances de dix qui couvrent le domaine d'un `u16`.
const POWERS_OF_TEN: [u16; 5] = [10_000, 1_000, 100, 10, 1];

/// Le chiffre de `value` à la position décimale `power`.
///
/// `power` est une puissance de dix non nulle. Le cas `power == 0` est inatteignable
/// depuis [`push_decimal`], qui n'emploie que [`POWERS_OF_TEN`] ; il rend zéro plutôt que
/// de paniquer, et un test le couvre directement.
const fn digit_at(value: u16, power: u16) -> u16 {
    match value.checked_div(power) {
        // Le reste par dix existe toujours : dix n'est pas nul, et `checked_rem`
        // n'aurait ici qu'une branche que rien ne pourrait atteindre — donc impossible à
        // couvrir, ce qu'ADR-012 interdit sur un codec.
        #[allow(
            clippy::arithmetic_side_effects,
            reason = "reste par un littéral non nul : l'opération est totale"
        )]
        Some(quotient) => quotient % 10,
        None => 0,
    }
}

/// Le caractère ASCII d'un chiffre décimal.
///
/// Les valeurs hors de `0..=9` sont inatteignables depuis [`push_decimal`], où l'argument
/// est un reste de division par dix ; elles rendent `'0'`, et un test couvre ce cas.
#[allow(
    clippy::match_same_arms,
    reason = "le zéro légitime et le cas dégénéré partagent leur résultat mais pas leur sens"
)]
const fn digit_to_ascii(digit: u16) -> u8 {
    match digit {
        0 => b'0',
        1 => b'1',
        2 => b'2',
        3 => b'3',
        4 => b'4',
        5 => b'5',
        6 => b'6',
        7 => b'7',
        8 => b'8',
        9 => b'9',
        _ => b'0',
    }
}

/// Ajoute à `output` les chiffres décimaux de `value`, sans séparateur ni signe.
///
/// Zéro s'écrit `0`, et aucun zéro de tête n'est émis. L'opération ne peut pas échouer.
pub fn push_decimal(output: &mut Vec<u8>, value: u16) {
    let mut emitting = false;
    for power in POWERS_OF_TEN {
        let digit = digit_at(value, power);
        // Le chiffre des unités est toujours émis : c'est ce qui fait écrire « 0 » plutôt
        // que rien pour une valeur nulle.
        if digit != 0 || emitting || power == 1 {
            output.push(digit_to_ascii(digit));
            emitting = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{digit_at, digit_to_ascii, push_decimal};

    fn rendered(value: u16) -> String {
        let mut output = Vec::new();
        push_decimal(&mut output, value);
        String::from_utf8(output).expect("des chiffres ASCII sont de l'UTF-8 valide")
    }

    #[test]
    fn zero_s_ecrit_avec_un_seul_chiffre() {
        assert_eq!(rendered(0), "0");
    }

    #[test]
    fn un_chiffre_unique_est_rendu_sans_zero_de_tete() {
        assert_eq!(rendered(7), "7");
    }

    #[test]
    fn les_chiffres_sont_dans_le_bon_ordre() {
        assert_eq!(rendered(1234), "1234");
    }

    #[test]
    fn un_zero_interne_est_conserve() {
        assert_eq!(rendered(1002), "1002");
    }

    #[test]
    fn la_valeur_maximale_est_rendue_en_entier() {
        assert_eq!(rendered(u16::MAX), "65535");
    }

    #[test]
    fn l_ecriture_ajoute_sans_vider_le_tampon() {
        let mut output = b"\x1b[".to_vec();
        push_decimal(&mut output, 42);
        assert_eq!(output, b"\x1b[42");
    }

    #[test]
    fn toutes_les_valeurs_coincident_avec_le_formatage_standard() {
        // Vérification exhaustive : le domaine est petit, donc on ne se contente pas d'un
        // échantillon.
        for value in 0..=u16::MAX {
            assert_eq!(rendered(value), value.to_string(), "pour {value}");
        }
    }

    #[test]
    fn digit_at_extrait_le_chiffre_de_la_position_demandee() {
        assert_eq!(digit_at(1234, 1000), 1);
        assert_eq!(digit_at(1234, 100), 2);
        assert_eq!(digit_at(1234, 10), 3);
        assert_eq!(digit_at(1234, 1), 4);
    }

    #[test]
    fn digit_at_rend_zero_pour_une_puissance_nulle() {
        // Inatteignable depuis `push_decimal` ; couvert ici pour que la branche existe
        // dans la mesure plutôt que d'être un angle mort.
        assert_eq!(digit_at(1234, 0), 0);
    }

    #[test]
    fn digit_to_ascii_couvre_les_dix_chiffres() {
        for digit in 0..=9_u16 {
            let expected = char::from_digit(u32::from(digit), 10)
                .expect("un chiffre décimal a toujours une écriture");
            assert_eq!(digit_to_ascii(digit), u8::try_from(expected).unwrap_or(0));
        }
    }

    #[test]
    fn digit_to_ascii_rend_zero_hors_domaine() {
        // Même raison que ci-dessus : la branche dégénérée est testée, pas tolérée.
        assert_eq!(digit_to_ascii(10), b'0');
        assert_eq!(digit_to_ascii(u16::MAX), b'0');
    }
}
