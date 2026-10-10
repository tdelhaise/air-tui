// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le protocole clavier Kitty — négociation et sémantique.
//!
//! # Pourquoi il est de tier-1
//!
//! C'est le **seul** mécanisme qui livre les modificateurs complets à travers un pty,
//! donc en SSH et sur macOS (ADR-003). Sans lui, le codage historique ne sait pas
//! distinguer `Ctrl+Shift+S` de `Ctrl+S` : les deux produisent le même octet `0x13`. Il
//! n'est donc pas une amélioration différable, c'est ce qui rend tenable la deuxième
//! exigence du cahier des charges.
//!
//! Il apporte aussi deux choses qu'aucun autre étage terminal ne donne : les
//! **relâchements de touche**, et la **touche de la disposition de base** — de quoi faire
//! fonctionner `Alt+F` pour « Fichier » sur un clavier AZERTY comme sur un QWERTY, ce qui
//! est exactement le problème qu'un framework à menus doit résoudre.
//!
//! # La négociation, et pourquoi elle ne repose pas sur un délai
//!
//! On envoie l'interrogation `CSI ? u` **suivie** d'une demande d'attributs de
//! périphérique `CSI c`. Les terminaux répondent dans l'ordre : si la réponse Kitty
//! arrive, elle arrive **avant** celle des attributs. Observer les attributs sans avoir vu
//! de réponse Kitty est donc une conclusion négative **certaine**, et non l'expiration
//! d'un délai choisi au hasard.
//!
//! Le délai reste là en dernier recours, pour le terminal qui ne répond à rien : il ne
//! doit jamais geler le lancement.
//!
//! # Ce que la table des touches fonctionnelles couvre
//!
//! Les touches dont le numéro est certain : tabulation, entrée, échappement, retour
//! arrière, les caractères imprimables, et `F13` à `F35`. Le pavé numérique, les touches
//! multimédia et les touches modificatrices seules **ne sont pas** dans la table : leurs
//! numéros n'ont pas été vérifiés contre un terminal réel, et une séquence hors table est
//! consommée sans produire d'événement — ce qui est le comportement correct, pas une
//! dégradation.

use crate::codec::decimal::push_decimal;
use crate::input::event::{AirKey, AirKeyState};

/// Le premier numéro de touche fonctionnelle de `F13`.
const FIRST_HIGH_FUNCTION_KEY_CODE: u32 = 57_376;
/// Le dernier numéro de touche fonctionnelle, celui de `F35`.
const LAST_HIGH_FUNCTION_KEY_CODE: u32 = 57_398;
/// Le rang de `F13` dans la numérotation d'air-tui.
const FIRST_HIGH_FUNCTION_KEY_RANK: u32 = 13;

/// Le protocole d'entrée clavier en vigueur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AirKeyboardProtocol {
    /// Le codage historique. `Ctrl+Shift+X` y est indiscernable de `Ctrl+X`.
    #[default]
    Legacy,
    /// `modifyOtherKeys` niveau 2, d'xterm. Distingue davantage de combinaisons que le
    /// codage historique, mais ne rapporte ni relâchement ni disposition de base.
    ModifyOtherKeys,
    /// Le protocole Kitty. Modificateurs complets, types d'événement, disposition de base.
    Kitty,
}

/// Les fonctions du protocole Kitty qu'on demande au terminal.
///
/// Une structure de booléens nommés, comme ailleurs dans le projet : le masque de bits
/// n'apparaît qu'au moment d'écrire la séquence.
// Cinq booléens, et c'est la bonne forme : chacun correspond à un bit que le terminal
// comprend séparément. Le masque n'existe qu'au moment d'écrire la séquence, et c'est
// `to_parameter` qui le fabrique — un seul endroit à relire.
#[allow(
    clippy::struct_excessive_bools,
    reason = "un booléen par fonction du protocole"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AirKeyboardFlags {
    /// Lever l'ambiguïté des séquences d'échappement.
    pub disambiguate_escape_codes: bool,
    /// Rapporter les types d'événement : enfoncement, répétition, relâchement.
    pub report_event_types: bool,
    /// Rapporter les touches de remplacement, dont celle de la disposition de base.
    pub report_alternate_keys: bool,
    /// Rapporter toutes les touches sous forme de séquence d'échappement.
    pub report_all_keys_as_escape_codes: bool,
    /// Rapporter le texte associé à la frappe.
    pub report_associated_text: bool,
}

impl AirKeyboardFlags {
    /// Ce dont air-tui a besoin : tout sauf le report systématique de toutes les touches.
    ///
    /// Le report systématique transforme la moindre frappe en séquence, y compris les
    /// caractères ordinaires, ce qui n'apporte rien à un framework qui sait déjà les
    /// décoder — et complique le collage.
    pub const WANTED: Self = Self {
        disambiguate_escape_codes: true,
        report_event_types: true,
        report_alternate_keys: true,
        report_all_keys_as_escape_codes: false,
        report_associated_text: true,
    };

    /// Le masque de bits que le terminal attend.
    #[must_use]
    pub const fn to_parameter(self) -> u32 {
        let mut parameter = 0;
        if self.disambiguate_escape_codes {
            parameter |= 0b0000_0001;
        }
        if self.report_event_types {
            parameter |= 0b0000_0010;
        }
        if self.report_alternate_keys {
            parameter |= 0b0000_0100;
        }
        if self.report_all_keys_as_escape_codes {
            parameter |= 0b0000_1000;
        }
        if self.report_associated_text {
            parameter |= 0b0001_0000;
        }
        parameter
    }

    /// Les fonctions décrites par un masque de bits, tel qu'un terminal le rapporte.
    #[must_use]
    pub const fn from_parameter(parameter: u32) -> Self {
        Self {
            disambiguate_escape_codes: parameter & 0b0000_0001 != 0,
            report_event_types: parameter & 0b0000_0010 != 0,
            report_alternate_keys: parameter & 0b0000_0100 != 0,
            report_all_keys_as_escape_codes: parameter & 0b0000_1000 != 0,
            report_associated_text: parameter & 0b0001_0000 != 0,
        }
    }

    /// Vrai si aucune fonction n'est active.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.to_parameter() == 0
    }
}

/// L'interrogation du protocole en vigueur : `CSI ? u`.
pub const QUERY_SEQUENCE: &[u8] = b"\x1b[?u";

/// La demande d'attributs de périphérique : `CSI c`.
///
/// Envoyée juste après l'interrogation, elle sert de **jalon** : sa réponse clôt la série,
/// donc l'absence de réponse Kitty avant elle est une conclusion négative certaine.
pub const PRIMARY_DEVICE_ATTRIBUTES_SEQUENCE: &[u8] = b"\x1b[c";

/// Le retrait des fonctions demandées : `CSI < u`.
pub const POP_SEQUENCE: &[u8] = b"\x1b[<u";

/// L'activation de `modifyOtherKeys` niveau 2.
pub const MODIFY_OTHER_KEYS_ENABLE_SEQUENCE: &[u8] = b"\x1b[>4;2m";

/// La désactivation de `modifyOtherKeys`.
pub const MODIFY_OTHER_KEYS_DISABLE_SEQUENCE: &[u8] = b"\x1b[>4;0m";

/// Écrit dans `output` la séquence qui demande les fonctions `flags`.
pub fn push_request_sequence(output: &mut Vec<u8>, flags: AirKeyboardFlags) {
    output.extend_from_slice(b"\x1b[>");
    // Le masque tient dans cinq bits ; la conversion ne peut pas perdre d'information, et
    // la borne est là pour ne pas écrire un nombre tronqué si un drapeau s'ajoutait un
    // jour sans que cette ligne soit relue.
    push_decimal(
        output,
        u16::try_from(flags.to_parameter()).unwrap_or(u16::MAX),
    );
    output.push(b'u');
}

/// L'état de touche décrit par un type d'événement Kitty.
///
/// Un type absent ou inconnu vaut un enfoncement : c'est la valeur par défaut du
/// protocole, et la seule lecture qui ne perde pas la frappe.
pub const fn key_state_from_event_type(event_type: u32) -> AirKeyState {
    match event_type {
        2 => AirKeyState::Repeat,
        3 => AirKeyState::Release,
        _ => AirKeyState::Press,
    }
}

/// La touche d'air-tui désignée par un numéro de touche Kitty.
///
/// Rend `None` pour un numéro hors de la table — pavé numérique, touches multimédia,
/// touches modificatrices seules —, auquel cas la séquence est consommée sans produire
/// d'événement.
pub fn key_for_code(code: u32) -> Option<AirKey> {
    match code {
        9 => Some(AirKey::Tab),
        13 => Some(AirKey::Enter),
        27 => Some(AirKey::Escape),
        127 => Some(AirKey::Backspace),
        FIRST_HIGH_FUNCTION_KEY_CODE..=LAST_HIGH_FUNCTION_KEY_CODE => {
            let offset = code.saturating_sub(FIRST_HIGH_FUNCTION_KEY_CODE);
            let rank = FIRST_HIGH_FUNCTION_KEY_RANK.saturating_add(offset);
            u8::try_from(rank).ok().map(AirKey::Function)
        }
        _ => character_key_for_code(code),
    }
}

/// Le premier point de code de la zone à usage privé du plan de base.
const FIRST_PRIVATE_USE_CODE: u32 = 0xE000;
/// Le dernier point de code de la zone à usage privé du plan de base.
const LAST_PRIVATE_USE_CODE: u32 = 0xF8FF;

/// La touche caractère désignée par un point de code, s'il en désigne une.
///
/// Deux domaines sont écartés, pour deux raisons distinctes :
///
/// - les **commandes C0** : le protocole rapporte la touche de base, donc `0x03` y
///   désignerait la touche `c` et non « fin de texte » ;
/// - la **zone à usage privé** du plan de base, `U+E000` à `U+F8FF` : c'est là que Kitty
///   loge ses numéros de touches fonctionnelles. Sans cette exclusion, un numéro hors de
///   la table d'air-tui — pavé numérique, multimédia, touche modificatrice seule —
///   deviendrait un caractère imprimable fantôme que l'application insérerait dans un
///   champ de saisie. Défaut trouvé par le test `un_numero_hors_table_ne_donne_aucune_touche`.
fn character_key_for_code(code: u32) -> Option<AirKey> {
    if code < 0x20 {
        return None;
    }
    if (FIRST_PRIVATE_USE_CODE..=LAST_PRIVATE_USE_CODE).contains(&code) {
        return None;
    }
    char::from_u32(code).map(AirKey::Character)
}

#[cfg(test)]
mod tests {
    use super::{
        AirKeyboardFlags, AirKeyboardProtocol, character_key_for_code, key_for_code,
        key_state_from_event_type, push_request_sequence,
    };
    use crate::input::event::{AirKey, AirKeyState};

    #[test]
    fn le_protocole_par_defaut_est_le_codage_historique() {
        assert_eq!(AirKeyboardProtocol::default(), AirKeyboardProtocol::Legacy);
    }

    #[test]
    fn les_fonctions_voulues_excluent_le_report_systematique() {
        let wanted = AirKeyboardFlags::WANTED;
        assert!(wanted.disambiguate_escape_codes);
        assert!(wanted.report_event_types);
        assert!(wanted.report_alternate_keys);
        assert!(!wanted.report_all_keys_as_escape_codes);
        assert!(wanted.report_associated_text);
        // 1 + 2 + 4 + 16
        assert_eq!(wanted.to_parameter(), 23);
    }

    #[test]
    fn le_masque_et_les_fonctions_sont_reciproques() {
        for parameter in 0..32_u32 {
            let flags = AirKeyboardFlags::from_parameter(parameter);
            assert_eq!(
                flags.to_parameter(),
                parameter,
                "pour le masque {parameter}"
            );
        }
    }

    #[test]
    fn un_masque_nul_ne_decrit_aucune_fonction() {
        assert!(AirKeyboardFlags::from_parameter(0).is_empty());
        assert!(AirKeyboardFlags::default().is_empty());
        assert!(!AirKeyboardFlags::WANTED.is_empty());
    }

    #[test]
    fn la_sequence_de_demande_porte_le_masque() {
        let mut output = Vec::new();
        push_request_sequence(&mut output, AirKeyboardFlags::WANTED);
        assert_eq!(output, b"\x1b[>23u");
    }

    #[test]
    fn la_sequence_de_demande_d_un_masque_nul_est_ecrite_explicitement() {
        let mut output = Vec::new();
        push_request_sequence(&mut output, AirKeyboardFlags::default());
        assert_eq!(output, b"\x1b[>0u");
    }

    #[test]
    fn les_types_d_evenement_donnent_les_trois_etats() {
        assert_eq!(key_state_from_event_type(1), AirKeyState::Press);
        assert_eq!(key_state_from_event_type(2), AirKeyState::Repeat);
        assert_eq!(key_state_from_event_type(3), AirKeyState::Release);
    }

    #[test]
    fn un_type_d_evenement_inconnu_vaut_un_enfoncement() {
        // La seule lecture qui ne perde pas la frappe.
        assert_eq!(key_state_from_event_type(0), AirKeyState::Press);
        assert_eq!(key_state_from_event_type(99), AirKeyState::Press);
    }

    #[test]
    fn les_touches_nommees_sont_dans_la_table() {
        assert_eq!(key_for_code(9), Some(AirKey::Tab));
        assert_eq!(key_for_code(13), Some(AirKey::Enter));
        assert_eq!(key_for_code(27), Some(AirKey::Escape));
        assert_eq!(key_for_code(127), Some(AirKey::Backspace));
    }

    #[test]
    fn les_caracteres_imprimables_deviennent_des_touches_caractere() {
        assert_eq!(key_for_code(97), Some(AirKey::Character('a')));
        assert_eq!(key_for_code(32), Some(AirKey::Character(' ')));
        assert_eq!(key_for_code(0xE9), Some(AirKey::Character('é')));
        assert_eq!(key_for_code(0x1F980), Some(AirKey::Character('🦀')));
    }

    #[test]
    fn les_commandes_c0_ne_sont_pas_des_touches_caractere() {
        // Le protocole rapporte la touche de base : 3 désignerait `c`, pas « fin de texte ».
        assert_eq!(character_key_for_code(3), None);
        assert_eq!(character_key_for_code(0x1F), None);
    }

    #[test]
    fn un_point_de_code_invalide_ne_donne_aucune_touche() {
        // Les substituts UTF-16 ne sont pas des caractères.
        assert_eq!(key_for_code(0xD800), None);
        assert_eq!(key_for_code(0x11_0000), None);
    }

    #[test]
    fn les_touches_de_fonction_hautes_sont_numerotees_de_treize_a_trente_cinq() {
        assert_eq!(key_for_code(57_376), Some(AirKey::Function(13)));
        assert_eq!(key_for_code(57_377), Some(AirKey::Function(14)));
        assert_eq!(key_for_code(57_398), Some(AirKey::Function(35)));
    }

    #[test]
    fn la_zone_a_usage_prive_n_est_jamais_un_caractere() {
        // Kitty y loge ses numéros de touches fonctionnelles : les laisser passer comme
        // caractères ferait insérer des glyphes fantômes dans les champs de saisie.
        assert_eq!(character_key_for_code(0xE000), None);
        assert_eq!(character_key_for_code(0xF8FF), None);
        // Juste en dehors, de part et d'autre, ce sont de vrais caractères.
        assert!(character_key_for_code(0xDFFF).is_none(), "substitut UTF-16");
        assert_eq!(
            character_key_for_code(0xF900),
            Some(AirKey::Character('\u{F900}'))
        );
    }

    #[test]
    fn un_numero_hors_table_ne_donne_aucune_touche() {
        // Pavé numérique, multimédia, touches modificatrices seules : hors table, donc
        // consommés sans événement. Leurs numéros n'ont pas été vérifiés contre un
        // terminal réel, et deviner serait pire que se taire.
        assert_eq!(key_for_code(57_399), None);
        assert_eq!(key_for_code(57_441), None);
        assert_eq!(key_for_code(57_300), None);
    }
}
