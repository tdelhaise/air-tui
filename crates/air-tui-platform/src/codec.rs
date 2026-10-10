// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les codecs — la seule porte entre un flux d'octets et des éléments typés.
//!
//! # La règle
//!
//! Toute conversion d'un flux de données passe par un codec, et **tout codec est tenu à
//! 100 % de couverture de code** (ADR-012). Ce n'est pas une préférence de style : un
//! codec est le point où des octets venus de l'extérieur acquièrent un sens, donc le
//! point où une branche non testée devient une interprétation fausse que personne ne
//! verra.
//!
//! Un codec est un [`Decoder`], un [`Encoder`], ou les deux. Beaucoup n'ont besoin que
//! d'un sens — l'encodeur de séquences SGR n'a aucune raison de savoir les relire — et
//! forcer un stub inutile ne ferait qu'ajouter du code non couvert.
//!
//! # Ce que la forme garantit
//!
//! [`Decoder::decode`] rend le nombre d'octets **consommés**, et un préfixe incomplet
//! n'en consomme aucun. C'est ce qui rend un décodeur utilisable sur un flux qui arrive
//! en morceaux — le cas normal d'un terminal, où une séquence d'échappement peut être
//! coupée en deux lectures.
//!
//! Les éléments produits sont poussés dans une fermeture plutôt que rendus dans un
//! conteneur : une frappe ne doit pas provoquer d'allocation (ADR-021 §4 d'Air).

/// Transforme un flux d'octets en éléments typés.
pub trait Decoder {
    /// Ce que le décodeur produit.
    type Item;
    /// Ce qui peut échouer.
    type Error;

    /// Consomme un préfixe de `input` et pousse les éléments reconnus dans `emit`.
    ///
    /// Rend le nombre d'octets consommés. Les règles sont contraignantes, et c'est ce qui
    /// rend un décodeur composable :
    ///
    /// - un préfixe **incomplet** consomme zéro octet et ne produit rien : l'appelant
    ///   rappellera avec davantage de données ;
    /// - un préfixe **invalide** est une erreur, jamais une devinette. La tolérance est la
    ///   faille (Principe 11 d'Air) ;
    /// - le nombre rendu ne dépasse jamais `input.len()`.
    ///
    /// # Errors
    ///
    /// Quand le préfixe est invalide et ne peut pas devenir valide en recevant la suite.
    fn decode(
        &mut self,
        input: &[u8],
        emit: &mut dyn FnMut(Self::Item),
    ) -> Result<usize, Self::Error>;
}

/// Transforme des éléments typés en octets.
pub trait Encoder {
    /// Ce que l'encodeur accepte.
    type Item;
    /// Ce qui peut échouer.
    type Error;

    /// Ajoute à `output` la représentation en octets de `item`.
    ///
    /// L'encodeur **ajoute** et ne vide jamais `output` : c'est l'appelant qui possède le
    /// tampon et le réutilise d'un rendu à l'autre, sans réallouer.
    ///
    /// # Errors
    ///
    /// Quand l'élément n'a pas de représentation valide.
    fn encode(&mut self, item: &Self::Item, output: &mut Vec<u8>) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{Decoder, Encoder};

    /// Un codec minimal, à titre de vérification du contrat : il reconnaît les chiffres
    /// ASCII et refuse tout le reste.
    struct DigitCodec;

    #[derive(Debug, PartialEq, Eq)]
    struct NotADigit(u8);

    impl Decoder for DigitCodec {
        type Item = u8;
        type Error = NotADigit;

        fn decode(
            &mut self,
            input: &[u8],
            emit: &mut dyn FnMut(Self::Item),
        ) -> Result<usize, Self::Error> {
            let Some(first) = input.first() else {
                // Préfixe incomplet : rien consommé, rien produit.
                return Ok(0);
            };
            if first.is_ascii_digit() {
                emit(first.wrapping_sub(b'0'));
                Ok(1)
            } else {
                Err(NotADigit(*first))
            }
        }
    }

    impl Encoder for DigitCodec {
        type Item = u8;
        type Error = NotADigit;

        fn encode(&mut self, item: &Self::Item, output: &mut Vec<u8>) -> Result<(), Self::Error> {
            if *item > 9 {
                return Err(NotADigit(*item));
            }
            output.push(item.wrapping_add(b'0'));
            Ok(())
        }
    }

    #[test]
    fn un_prefixe_vide_ne_consomme_rien() {
        let mut collected = Vec::new();
        let consumed = DigitCodec
            .decode(b"", &mut |item| collected.push(item))
            .expect("un flux vide n'est pas une erreur");
        assert_eq!(consumed, 0);
        assert_eq!(collected, Vec::<u8>::new());
    }

    #[test]
    fn un_element_reconnu_est_consomme_et_produit() {
        let mut collected = Vec::new();
        let consumed = DigitCodec
            .decode(b"7abc", &mut |item| collected.push(item))
            .expect("un chiffre est valide");
        assert_eq!(consumed, 1);
        assert_eq!(collected, vec![7]);
    }

    #[test]
    fn un_prefixe_invalide_est_une_erreur_et_non_une_devinette() {
        let mut collected = Vec::new();
        let outcome = DigitCodec.decode(b"x", &mut |item| collected.push(item));
        assert_eq!(outcome, Err(NotADigit(b'x')));
        assert_eq!(collected, Vec::<u8>::new());
    }

    #[test]
    fn l_encodeur_ajoute_sans_vider_le_tampon() {
        let mut output = b"deja la".to_vec();
        DigitCodec.encode(&3, &mut output).expect("3 est encodable");
        assert_eq!(output, b"deja la3");
    }

    #[test]
    fn l_encodeur_refuse_un_element_hors_domaine() {
        let mut output = Vec::new();
        assert_eq!(DigitCodec.encode(&42, &mut output), Err(NotADigit(42)));
        assert_eq!(output, Vec::<u8>::new());
    }
}
