// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Fuzzing du décodeur de séquences ANSI.
//!
//! Le Principe 1 d'Air impose le fuzzing de toute API qui accepte des données externes, et
//! l'ADR-012 le redit pour les décodeurs : un décodeur reçoit par définition des octets
//! qu'il n'a pas choisis.
//!
//! Trois invariants sont vérifiés, au-delà de l'absence de panique :
//!
//! 1. le nombre d'octets consommés ne dépasse jamais la taille de l'entrée ;
//! 2. après un décodage réussi, redécoder le reste ne progresse plus — la boucle du
//!    décodeur ne laisse rien de décodable derrière elle ;
//! 3. découper la même entrée en deux lectures produit la même suite d'événements que de
//!    la fournir d'un seul morceau. C'est la propriété qui compte vraiment sur un
//!    terminal, où une séquence arrive coupée en deux sans prévenir.

#![no_main]

use air_tui::input::AirAnsiDecoder;
use air_tui_platform::Decoder;
use libfuzzer_sys::fuzz_target;

/// Décode une entrée d'un seul morceau et rend les événements, ou `None` sur erreur.
fn decode_whole(input: &[u8]) -> Option<(usize, Vec<String>)> {
    let mut decoder = AirAnsiDecoder::new();
    let mut collected = Vec::new();
    let consumed = decoder
        .decode(input, &mut |event| collected.push(format!("{event:?}")))
        .ok()?;
    Some((consumed, collected))
}

/// Décode la même entrée en deux morceaux, coupés à `split`.
fn decode_in_two_parts(input: &[u8], split: usize) -> Option<Vec<String>> {
    let (first, second) = input.split_at(split.min(input.len()));
    let mut decoder = AirAnsiDecoder::new();
    let mut collected = Vec::new();
    let consumed = decoder
        .decode(first, &mut |event| collected.push(format!("{event:?}")))
        .ok()?;
    // Ce que le premier morceau n'a pas consommé est repris en tête du second.
    let mut remainder = first.get(consumed..)?.to_vec();
    remainder.extend_from_slice(second);
    decoder
        .decode(&remainder, &mut |event| collected.push(format!("{event:?}")))
        .ok()?;
    Some(collected)
}

fuzz_target!(|input: &[u8]| {
    let Some((consumed, whole)) = decode_whole(input) else {
        // Une entrée refusée est une issue normale : le décodeur ne devine pas.
        return;
    };

    // 1. Le décodeur ne peut pas consommer plus qu'on ne lui a donné.
    assert!(
        consumed <= input.len(),
        "consommé {consumed} pour une entrée de {}",
        input.len()
    );

    // 2. Le reste ne doit plus rien contenir de décodable.
    if let Some(remainder) = input.get(consumed..) {
        let mut decoder = AirAnsiDecoder::new();
        if let Ok(again) = decoder.decode(remainder, &mut |_| {}) {
            assert_eq!(again, 0, "un reste décodable a été laissé derrière");
        }
    }

    // 3. Le découpage ne change pas le résultat. On n'essaie qu'une coupure par entrée :
    //    le fuzzer explore les autres de lui-même en faisant varier la taille.
    if !input.is_empty() {
        let split = input.len() / 2;
        if let Some(in_parts) = decode_in_two_parts(input, split) {
            assert_eq!(
                whole, in_parts,
                "le découpage à l'octet {split} change le résultat"
            );
        }
    }
});
