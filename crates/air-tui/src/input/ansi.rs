// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le décodeur de séquences ANSI d'entrée.
//!
//! C'est un codec au sens de l'ADR-012 : il est tenu à 100 % de couverture et il porte une
//! cible de fuzzing, parce qu'il reçoit par définition des octets qu'il n'a pas choisis.
//!
//! # Ce qu'il refuse de faire
//!
//! **Il ne devine jamais.** Trois comportements en découlent, et ils sont distincts :
//!
//! - un préfixe **incomplet** consomme zéro octet et ne produit rien. L'appelant rappellera
//!   avec la suite. C'est indispensable sur un terminal, où une séquence peut être coupée
//!   entre deux lectures ;
//! - une séquence **malformée** est une erreur. La tolérance est la faille (Principe 11
//!   d'Air) ;
//! - une séquence **bien formée mais non reconnue** est consommée sans produire
//!   d'événement. Ce n'est pas de la tolérance : un terminal émet des réponses qu'on ne
//!   lui a pas demandées, et les refuser bloquerait le flux sur la première.
//!
//! # L'ambiguïté de l'échappement
//!
//! Un `ESC` isolé est indiscernable du début d'une séquence : seul le temps les sépare. Le
//! décodeur, qui est pur, ne connaît pas le temps — il rend donc « incomplet ». C'est à la
//! boucle d'événements, qui sait qu'un délai s'est écoulé sans nouvel octet, d'appeler
//! [`AirAnsiDecoder::isolated_escape_event`].
//!
//! # Ce qu'il ne couvre pas encore
//!
//! Le protocole clavier Kitty, donc les relâchements de touche et les combinaisons que le
//! codage historique ne peut pas exprimer — `Ctrl+Shift+S` reste indiscernable de `Ctrl+S`
//! (jalon M2, ADR-003). Le double clic, qui demande une horloge et n'a donc pas sa place
//! dans un décodeur pur.

use air_tui_platform::Decoder;

use crate::geometry::{AirColumn, AirPoint, AirRow};
use crate::input::event::{
    AirInputEvent, AirKey, AirKeyEvent, AirModifiers, AirMouseButton, AirMouseEvent,
    AirMouseEventKind,
};
use crate::input::kitty::{AirKeyboardFlags, key_for_code, key_state_from_event_type};
use crate::input::scanning::{
    AirParameterField, find_subslice, modifiers_from_parameter, parse_parameter_field,
    partial_suffix_length, utf8_sequence_length,
};

/// La séquence qui ouvre un collage encadré.
const PASTE_START_PARAMETER: u32 = 200;
/// La séquence qui ferme un collage encadré.
const PASTE_END_SEQUENCE: &[u8] = b"\x1b[201~";
/// Le paramètre de fermeture d'un collage encadré, rencontré hors collage.
const PASTE_END_PARAMETER: u32 = 201;

/// Le premier paramètre d'une séquence `modifyOtherKeys` niveau 2.
const MODIFY_OTHER_KEYS_PARAMETER: u32 = 27;

/// Nombre maximal d'octets de paramètres d'une séquence de contrôle.
///
/// Les séquences qu'un terminal émet réellement en comptent moins de vingt. La borne est
/// là pour qu'un flux hostile ne fasse pas croître un balayage indéfiniment : le Principe
/// 11 d'Air demande une disposition fixe et bornée sur un chemin d'entrée.
const MAXIMUM_PARAMETER_BYTES: usize = 64;

/// Taille maximale d'un collage encadré.
///
/// Un mégaoctet de texte collé est déjà démesuré pour une interface texte. La borne
/// empêche un flux hostile de faire croître un tampon sans fin.
const MAXIMUM_PASTE_BYTES: usize = 1 << 20;

/// Ce qui peut rendre un flux indécodable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirDecodeError {
    /// Les octets ne forment pas de l'UTF-8 valide.
    InvalidUtf8,
    /// La séquence de contrôle ne respecte pas la structure attendue.
    MalformedSequence,
    /// Un collage encadré dépasse la taille admise.
    PasteTooLarge,
}

impl core::fmt::Display for AirDecodeError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let described = match self {
            Self::InvalidUtf8 => "octets qui ne forment pas de l'UTF-8 valide",
            Self::MalformedSequence => "séquence de contrôle malformée",
            Self::PasteTooLarge => "collage encadré trop volumineux",
        };
        formatter.write_str(described)
    }
}

impl core::error::Error for AirDecodeError {}

/// Les paramètres numériques d'une séquence de contrôle.
///
/// Un tableau de taille fixe plutôt qu'un `Vec` : le décodage d'une frappe ne doit pas
/// allouer (ADR-021 §4 d'Air).
#[derive(Debug, Clone, Copy, Default)]
struct Parameters {
    /// Le premier champ : le numéro de touche, ou la valeur de navigation.
    first: AirParameterField,
    /// Le deuxième champ : les modificateurs pour une touche, la colonne pour la souris.
    second: AirParameterField,
    /// Le troisième champ : le texte associé, ou la ligne pour la souris.
    third: AirParameterField,
    /// Combien de champs ont été rencontrés, retenus ou non.
    count: usize,
}

impl Parameters {
    /// Le champ d'indice `index`.
    ///
    /// Des champs nommés plutôt qu'un tableau indexé : trois champs suffisent aux
    /// séquences traitées, et l'indexation d'un tableau est un accès qui peut paniquer —
    /// ce que le Principe 3 d'Air refuse sur un chemin d'entrée.
    const fn field(&self, index: usize) -> AirParameterField {
        match index {
            0 => self.first,
            1 => self.second,
            2 => self.third,
            _ => AirParameterField {
                primary: None,
                first_sub: None,
                second_sub: None,
            },
        }
    }

    /// La valeur principale du champ d'indice `index`, ou `fallback` s'il est absent.
    const fn value_or(&self, index: usize, fallback: u32) -> u32 {
        match self.field(index).primary {
            Some(value) => value,
            None => fallback,
        }
    }

    /// Le premier sous-paramètre du champ d'indice `index`, ou `fallback` s'il est absent.
    const fn first_sub_or(&self, index: usize, fallback: u32) -> u32 {
        match self.field(index).first_sub {
            Some(value) => value,
            None => fallback,
        }
    }

    /// Le deuxième sous-paramètre du champ d'indice `index`, sans valeur de repli.
    const fn second_sub(&self, index: usize) -> Option<u32> {
        self.field(index).second_sub
    }

    /// Retient un champ de plus.
    ///
    /// Au-delà du troisième, le champ est compté mais pas conservé : les séquences
    /// traitées n'en emploient pas davantage.
    const fn push(&mut self, field: AirParameterField) {
        match self.count {
            0 => self.first = field,
            1 => self.second = field,
            2 => self.third = field,
            _ => {}
        }
        self.count = self.count.saturating_add(1);
    }
}

/// Découpe et lit les champs d'une séquence de contrôle, sous-paramètres compris.
fn parse_parameters(bytes: &[u8]) -> Result<Parameters, AirDecodeError> {
    let mut parameters = Parameters::default();
    if bytes.is_empty() {
        return Ok(parameters);
    }
    for raw_field in bytes.split(|byte| *byte == b';') {
        let Some(field) = parse_parameter_field(raw_field) else {
            return Err(AirDecodeError::MalformedSequence);
        };
        parameters.push(field);
    }
    Ok(parameters)
}

/// Le premier caractère d'un texte.
///
/// Rend le caractère nul pour un texte vide, cas inatteignable depuis les appelants — qui
/// n'invoquent cette fonction qu'après un décodage UTF-8 réussi sur au moins un octet — et
/// couvert par un test direct (ADR-012).
fn first_character(text: &str) -> char {
    text.chars().next().unwrap_or('\u{0}')
}

/// L'événement produit par un octet de commande C0, ou par le caractère de suppression.
///
/// Le dernier bras est inatteignable depuis les appelants, qui ne passent que des octets
/// inférieurs à `0x20` ou égaux à `0x7F` ; un test le couvre directement.
#[allow(
    clippy::match_same_arms,
    reason = "des octets distincts peuvent désigner la même touche sans être le même cas"
)]
const fn control_key_event(byte: u8) -> AirKeyEvent {
    let control = AirModifiers::NONE.with_control();
    let none = AirModifiers::NONE;
    let (logical, modifiers) = match byte {
        0x00 => (AirKey::Character(' '), control),
        0x01 => (AirKey::Character('a'), control),
        0x02 => (AirKey::Character('b'), control),
        0x03 => (AirKey::Character('c'), control),
        0x04 => (AirKey::Character('d'), control),
        0x05 => (AirKey::Character('e'), control),
        0x06 => (AirKey::Character('f'), control),
        0x07 => (AirKey::Character('g'), control),
        0x08 => (AirKey::Backspace, none),
        0x09 => (AirKey::Tab, none),
        0x0A => (AirKey::Enter, none),
        0x0B => (AirKey::Character('k'), control),
        0x0C => (AirKey::Character('l'), control),
        0x0D => (AirKey::Enter, none),
        0x0E => (AirKey::Character('n'), control),
        0x0F => (AirKey::Character('o'), control),
        0x10 => (AirKey::Character('p'), control),
        0x11 => (AirKey::Character('q'), control),
        0x12 => (AirKey::Character('r'), control),
        0x13 => (AirKey::Character('s'), control),
        0x14 => (AirKey::Character('t'), control),
        0x15 => (AirKey::Character('u'), control),
        0x16 => (AirKey::Character('v'), control),
        0x17 => (AirKey::Character('w'), control),
        0x18 => (AirKey::Character('x'), control),
        0x19 => (AirKey::Character('y'), control),
        0x1A => (AirKey::Character('z'), control),
        0x1B => (AirKey::Escape, none),
        0x1C => (AirKey::Character('\\'), control),
        0x1D => (AirKey::Character(']'), control),
        0x1E => (AirKey::Character('^'), control),
        0x1F => (AirKey::Character('_'), control),
        0x7F => (AirKey::Backspace, none),
        _ => (AirKey::Character('\u{0}'), none),
    };
    AirKeyEvent {
        logical,
        modifiers,
        state: crate::input::event::AirKeyState::Press,
        physical: None,
        base_layout: None,
    }
}

/// La touche de navigation ou de fonction désignée par un paramètre de séquence `~`.
const fn key_for_tilde_parameter(parameter: u32) -> Option<AirKey> {
    match parameter {
        1 | 7 => Some(AirKey::Home),
        2 => Some(AirKey::Insert),
        3 => Some(AirKey::Delete),
        4 | 8 => Some(AirKey::End),
        5 => Some(AirKey::PageUp),
        6 => Some(AirKey::PageDown),
        11 => Some(AirKey::Function(1)),
        12 => Some(AirKey::Function(2)),
        13 => Some(AirKey::Function(3)),
        14 => Some(AirKey::Function(4)),
        15 => Some(AirKey::Function(5)),
        17 => Some(AirKey::Function(6)),
        18 => Some(AirKey::Function(7)),
        19 => Some(AirKey::Function(8)),
        20 => Some(AirKey::Function(9)),
        21 => Some(AirKey::Function(10)),
        23 => Some(AirKey::Function(11)),
        24 => Some(AirKey::Function(12)),
        25 => Some(AirKey::Function(13)),
        26 => Some(AirKey::Function(14)),
        28 => Some(AirKey::Function(15)),
        29 => Some(AirKey::Function(16)),
        31 => Some(AirKey::Function(17)),
        32 => Some(AirKey::Function(18)),
        33 => Some(AirKey::Function(19)),
        34 => Some(AirKey::Function(20)),
        _ => None,
    }
}

/// La touche désignée par l'octet final d'une séquence, hors séquences `~`.
const fn key_for_final_byte(final_byte: u8) -> Option<AirKey> {
    match final_byte {
        b'A' => Some(AirKey::Up),
        b'B' => Some(AirKey::Down),
        b'C' => Some(AirKey::Right),
        b'D' => Some(AirKey::Left),
        b'E' => Some(AirKey::Character('5')),
        b'F' => Some(AirKey::End),
        b'H' => Some(AirKey::Home),
        b'P' => Some(AirKey::Function(1)),
        b'Q' => Some(AirKey::Function(2)),
        b'R' => Some(AirKey::Function(3)),
        b'S' => Some(AirKey::Function(4)),
        _ => None,
    }
}

/// Les modificateurs encodés dans le code de bouton d'un événement de souris SGR.
const fn mouse_modifiers(code: u32) -> AirModifiers {
    AirModifiers {
        shift: code & 0b0000_0100 != 0,
        alternate: code & 0b0000_1000 != 0,
        control: code & 0b0001_0000 != 0,
        super_key: false,
    }
}

/// Le bouton désigné par les deux bits de poids faible d'un code de souris.
const fn mouse_button(code: u32) -> Option<AirMouseButton> {
    match code & 0b11 {
        0 => Some(AirMouseButton::Left),
        1 => Some(AirMouseButton::Middle),
        2 => Some(AirMouseButton::Right),
        _ => None,
    }
}

/// Le genre d'événement de souris, ou `None` si le code n'en décrit aucun de sensé.
///
/// Un enfoncement sans bouton n'a pas de sens : plutôt que d'inventer un bouton gauche,
/// on ne produit pas d'événement.
const fn mouse_kind(code: u32, is_release: bool) -> Option<AirMouseEventKind> {
    let button = mouse_button(code);
    if code & 0b0100_0000 != 0 {
        return match code & 0b11 {
            0 => Some(AirMouseEventKind::WheelUp),
            1 => Some(AirMouseEventKind::WheelDown),
            2 => Some(AirMouseEventKind::WheelLeft),
            _ => Some(AirMouseEventKind::WheelRight),
        };
    }
    if is_release {
        return Some(AirMouseEventKind::Released(button));
    }
    if code & 0b0010_0000 != 0 {
        return Some(AirMouseEventKind::Moved(button));
    }
    match button {
        Some(pressed) => Some(AirMouseEventKind::Pressed(pressed)),
        None => None,
    }
}

/// Le décodeur de séquences ANSI d'entrée.
///
/// Il porte un seul état : le tampon d'un collage encadré en cours. Tout le reste du
/// décodage est sans mémoire.
#[derive(Debug, Default)]
pub struct AirAnsiDecoder {
    /// Le texte collé accumulé, quand un collage encadré est ouvert.
    paste_buffer: Option<Vec<u8>>,
}

impl AirAnsiDecoder {
    /// Un décodeur neuf.
    #[must_use]
    pub const fn new() -> Self {
        Self { paste_buffer: None }
    }

    /// Vrai si un collage encadré est en cours.
    ///
    /// Utile à une boucle d'événements qui voudrait suspendre le traitement des raccourcis
    /// pendant un collage.
    #[must_use]
    pub const fn is_pasting(&self) -> bool {
        self.paste_buffer.is_some()
    }

    /// L'événement à produire quand un `ESC` isolé n'a reçu aucune suite dans le délai
    /// imparti.
    ///
    /// Le décodeur ne connaît pas le temps : il rend « incomplet » sur un `ESC` seul, et
    /// c'est la boucle d'événements qui tranche. Cette fonction existe pour que la
    /// décision soit prise à un seul endroit du code appelant.
    #[must_use]
    pub const fn isolated_escape_event() -> AirInputEvent {
        AirInputEvent::Key(AirKeyEvent::press(AirKey::Escape))
    }

    /// Décode au plus un élément.
    ///
    /// Rend zéro quand le préfixe est incomplet ou l'entrée vide.
    fn decode_once(
        &mut self,
        input: &[u8],
        emit: &mut dyn FnMut(AirInputEvent),
    ) -> Result<usize, AirDecodeError> {
        // Le tampon est pris par valeur : le décodage d'un collage n'a alors plus aucune
        // branche « tampon absent » à traiter, donc aucune branche inatteignable (ADR-012).
        if let Some(buffer) = self.paste_buffer.take() {
            return self.decode_paste_content(buffer, input, emit);
        }
        let Some(leading) = input.first() else {
            return Ok(0);
        };
        match *leading {
            0x1B => self.decode_escape(input, emit),
            byte if byte < 0x20 || byte == 0x7F => {
                emit(AirInputEvent::Key(control_key_event(byte)));
                Ok(1)
            }
            leading_byte => decode_text(leading_byte, input, emit),
        }
    }

    /// Décode ce qui suit un `ESC`.
    fn decode_escape(
        &mut self,
        input: &[u8],
        emit: &mut dyn FnMut(AirInputEvent),
    ) -> Result<usize, AirDecodeError> {
        match input.get(1) {
            // `ESC` seul : indiscernable du début d'une séquence, donc incomplet.
            None => Ok(0),
            Some(b'[') => self.decode_control_sequence(input, emit),
            Some(b'O') => Ok(decode_single_shift_three(input, emit)),
            Some(following) => decode_alternate(*following, input, emit),
        }
    }

    /// Décode une séquence `ESC [ … final`.
    fn decode_control_sequence(
        &mut self,
        input: &[u8],
        emit: &mut dyn FnMut(AirInputEvent),
    ) -> Result<usize, AirDecodeError> {
        let body = input.get(2..).unwrap_or(&[]);
        let mut index = 0_usize;
        let final_byte;
        loop {
            let Some(byte) = body.get(index) else {
                // La séquence n'est pas terminée : on attend la suite.
                return Ok(0);
            };
            if (0x40..=0x7E).contains(byte) {
                final_byte = *byte;
                break;
            }
            if !(0x20..=0x3F).contains(byte) {
                return Err(AirDecodeError::MalformedSequence);
            }
            index = index.saturating_add(1);
            if index >= MAXIMUM_PARAMETER_BYTES {
                return Err(AirDecodeError::MalformedSequence);
            }
        }
        let parameter_bytes = body.get(..index).unwrap_or(&[]);
        // `ESC`, `[`, les paramètres, et l'octet final.
        let consumed = index.saturating_add(3);

        let (private_prefix, parameter_bytes) = split_private_prefix(parameter_bytes);
        if private_prefix == Some(b'<') {
            decode_mouse(parameter_bytes, final_byte, emit)?;
            return Ok(consumed);
        }

        let parameters = parse_parameters(parameter_bytes)?;

        if private_prefix == Some(b'?') {
            decode_terminal_report(&parameters, final_byte, emit);
            return Ok(consumed);
        }
        if private_prefix.is_some() {
            // Un préfixe privé qu'on ne reconnaît pas : la séquence est bien formée, elle
            // est consommée, et on n'invente pas ce qu'elle voulait dire.
            return Ok(consumed);
        }

        let modifiers = modifiers_from_parameter(parameters.value_or(1, 1));

        match final_byte {
            b'u' => decode_kitty_key(&parameters, emit),
            b'~' => {
                self.decode_tilde_sequence(&parameters, modifiers, emit);
            }
            b'Z' => {
                emit(AirInputEvent::Key(
                    AirKeyEvent::press(AirKey::Tab).with_modifiers(AirModifiers::NONE.with_shift()),
                ));
            }
            b'I' => emit(AirInputEvent::FocusGained),
            b'O' => emit(AirInputEvent::FocusLost),
            other => {
                if let Some(key) = key_for_final_byte(other) {
                    emit(AirInputEvent::Key(
                        AirKeyEvent::press(key).with_modifiers(modifiers),
                    ));
                }
                // Sinon : séquence bien formée, non reconnue. Consommée, sans événement.
            }
        }
        Ok(consumed)
    }

    /// Décode une séquence terminée par `~`, dont les collages encadrés.
    fn decode_tilde_sequence(
        &mut self,
        parameters: &Parameters,
        modifiers: AirModifiers,
        emit: &mut dyn FnMut(AirInputEvent),
    ) {
        let parameter = parameters.value_or(0, 0);
        if parameter == PASTE_START_PARAMETER {
            self.paste_buffer = Some(Vec::new());
            return;
        }
        if parameter == PASTE_END_PARAMETER {
            // Fermeture rencontrée hors collage : rien à fermer, rien à signaler.
            return;
        }
        if parameter == MODIFY_OTHER_KEYS_PARAMETER {
            // `CSI 27 ; modificateurs ; numéro ~` — la forme de `modifyOtherKeys`
            // niveau 2, le repli d'xterm et de VTE quand le protocole Kitty est absent.
            if let Some(logical) = key_for_code(parameters.value_or(2, 0)) {
                emit(AirInputEvent::Key(
                    AirKeyEvent::press(logical).with_modifiers(modifiers),
                ));
            }
            return;
        }
        if let Some(key) = key_for_tilde_parameter(parameter) {
            emit(AirInputEvent::Key(
                AirKeyEvent::press(key).with_modifiers(modifiers),
            ));
        }
    }

    /// Accumule le contenu d'un collage encadré jusqu'à son terminateur.
    ///
    /// `buffer` est pris par valeur et rendu au décodeur seulement si le collage continue.
    /// Une erreur **referme** le collage : le flux est rompu, et garder un tampon ouvert
    /// ferait traiter la suite du flux comme du texte collé.
    fn decode_paste_content(
        &mut self,
        mut buffer: Vec<u8>,
        input: &[u8],
        emit: &mut dyn FnMut(AirInputEvent),
    ) -> Result<usize, AirDecodeError> {
        if let Some(index) = find_subslice(input, PASTE_END_SEQUENCE) {
            let content = input.get(..index).unwrap_or(&[]);
            if buffer.len().saturating_add(content.len()) > MAXIMUM_PASTE_BYTES {
                return Err(AirDecodeError::PasteTooLarge);
            }
            buffer.extend_from_slice(content);
            let text = String::from_utf8(buffer).map_err(|_| AirDecodeError::InvalidUtf8)?;
            emit(AirInputEvent::Paste(text));
            return Ok(index.saturating_add(PASTE_END_SEQUENCE.len()));
        }
        // Pas de terminateur en vue : on consomme tout, en gardant de côté ce qui pourrait
        // être un terminateur coupé entre deux morceaux de lecture.
        let kept = partial_suffix_length(input, PASTE_END_SEQUENCE);
        let taken = input.len().saturating_sub(kept);
        let content = input.get(..taken).unwrap_or(&[]);
        if buffer.len().saturating_add(content.len()) > MAXIMUM_PASTE_BYTES {
            return Err(AirDecodeError::PasteTooLarge);
        }
        buffer.extend_from_slice(content);
        self.paste_buffer = Some(buffer);
        Ok(taken)
    }
}

/// Décode un caractère UTF-8.
///
/// `leading` est l'octet de tête, que l'appelant connaît déjà : le passer évite une branche
/// « entrée vide » que rien ne peut atteindre, donc impossible à couvrir (ADR-012).
fn decode_text(
    leading: u8,
    input: &[u8],
    emit: &mut dyn FnMut(AirInputEvent),
) -> Result<usize, AirDecodeError> {
    let Some(length) = utf8_sequence_length(leading) else {
        return Err(AirDecodeError::InvalidUtf8);
    };
    let Some(sequence) = input.get(..length) else {
        // Caractère coupé entre deux lectures : on attend la suite.
        return Ok(0);
    };
    let text = core::str::from_utf8(sequence).map_err(|_| AirDecodeError::InvalidUtf8)?;
    emit(AirInputEvent::Key(AirKeyEvent::press(AirKey::Character(
        first_character(text),
    ))));
    Ok(length)
}

/// Décode `ESC O final` — la forme « SS3 », employée par les touches de fonction du pavé
/// d'application.
fn decode_single_shift_three(input: &[u8], emit: &mut dyn FnMut(AirInputEvent)) -> usize {
    let Some(final_byte) = input.get(2) else {
        return 0;
    };
    if let Some(key) = key_for_final_byte(*final_byte) {
        emit(AirInputEvent::Key(AirKeyEvent::press(key)));
    }
    // Séquence non reconnue : consommée sans événement.
    3
}

/// Décode `ESC` suivi d'autre chose que `[` ou `O` — la combinaison avec la touche
/// alternative.
fn decode_alternate(
    leading: u8,
    input: &[u8],
    emit: &mut dyn FnMut(AirInputEvent),
) -> Result<usize, AirDecodeError> {
    let rest = input.get(1..).unwrap_or(&[]);
    if leading < 0x20 || leading == 0x7F {
        let base = control_key_event(leading);
        let mut modifiers = base.modifiers;
        modifiers.alternate = true;
        emit(AirInputEvent::Key(base.with_modifiers(modifiers)));
        return Ok(2);
    }
    let Some(length) = utf8_sequence_length(leading) else {
        return Err(AirDecodeError::InvalidUtf8);
    };
    let Some(sequence) = rest.get(..length) else {
        return Ok(0);
    };
    let text = core::str::from_utf8(sequence).map_err(|_| AirDecodeError::InvalidUtf8)?;
    emit(AirInputEvent::Key(
        AirKeyEvent::press(AirKey::Character(first_character(text)))
            .with_modifiers(AirModifiers::NONE.with_alternate()),
    ));
    Ok(length.saturating_add(1))
}

/// Détache l'octet de préfixe privé d'une suite de paramètres.
///
/// La norme réserve `0x3C` à `0x3F` — `<`, `=`, `>`, `?` — au premier rang des paramètres
/// pour marquer une séquence privée. Les trois formes qui nous concernent sont `<` pour la
/// souris SGR, `?` pour une réponse du terminal, et `>` pour une demande.
fn split_private_prefix(parameter_bytes: &[u8]) -> (Option<u8>, &[u8]) {
    match parameter_bytes.first() {
        Some(first) if (0x3C..=0x3F).contains(first) => {
            (Some(*first), parameter_bytes.get(1..).unwrap_or(&[]))
        }
        _ => (None, parameter_bytes),
    }
}

/// Décode une réponse du terminal, reconnaissable à son préfixe `?`.
fn decode_terminal_report(
    parameters: &Parameters,
    final_byte: u8,
    emit: &mut dyn FnMut(AirInputEvent),
) {
    match final_byte {
        // `CSI ? <masque> u` : les fonctions du protocole clavier Kitty en vigueur. Son
        // arrivée **est** la preuve que le protocole est pris en charge.
        b'u' => emit(AirInputEvent::KeyboardProtocolReported(
            AirKeyboardFlags::from_parameter(parameters.value_or(0, 0)),
        )),
        // `CSI ? … c` : les attributs de périphérique. Ils servent de jalon à la
        // négociation, leur contenu ne nous apprend rien d'utile.
        b'c' => emit(AirInputEvent::DeviceAttributesReported),
        // Une autre réponse : consommée sans événement.
        _ => {}
    }
}

/// Décode une frappe au format du protocole clavier Kitty : `CSI … u`.
///
/// La forme complète est
/// `CSI touche : décalée : disposition-de-base ; modificateurs : type ; texte u`.
fn decode_kitty_key(parameters: &Parameters, emit: &mut dyn FnMut(AirInputEvent)) {
    let Some(key_from_code) = key_for_code(parameters.value_or(0, 0)) else {
        // Numéro hors de la table : pavé numérique, multimédia, touche modificatrice
        // seule. Consommé sans événement plutôt que deviné.
        return;
    };
    // Le texte associé, quand le terminal le rapporte, est ce que l'utilisateur a
    // réellement produit : il prime sur le numéro de touche, qui ignore les
    // compositions et AltGr.
    let logical = match parameters.field(2).primary.and_then(key_for_code) {
        Some(from_text) => from_text,
        None => key_from_code,
    };
    emit(AirInputEvent::Key(AirKeyEvent {
        logical,
        modifiers: modifiers_from_parameter(parameters.value_or(1, 1)),
        state: key_state_from_event_type(parameters.first_sub_or(1, 1)),
        physical: None,
        base_layout: parameters.second_sub(0).and_then(key_for_code),
    }));
}

/// Ramène une coordonnée de terminal, comptée depuis un, à un indice de grille compté
/// depuis zéro.
fn narrow_coordinate(value: u32) -> u16 {
    let from_zero = value.saturating_sub(1);
    u16::try_from(from_zero).unwrap_or(u16::MAX)
}

/// Décode un événement de souris au format SGR 1006.
fn decode_mouse(
    parameter_bytes: &[u8],
    final_byte: u8,
    emit: &mut dyn FnMut(AirInputEvent),
) -> Result<(), AirDecodeError> {
    let parameters = parse_parameters(parameter_bytes)?;
    let code = parameters.value_or(0, 0);
    // Le terminal compte depuis un ; la grille depuis zéro. La conversion est bornée : une
    // grille de plus de 65 535 colonnes n'existe pas, et tronquer serait pire que saturer.
    let column = narrow_coordinate(parameters.value_or(1, 1));
    let row = narrow_coordinate(parameters.value_or(2, 1));
    let is_release = final_byte == b'm';
    if let Some(kind) = mouse_kind(code, is_release) {
        emit(AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            kind,
            modifiers: mouse_modifiers(code),
        }));
    }
    // Un code qui ne décrit aucun événement sensé est consommé sans rien produire.
    Ok(())
}

impl Decoder for AirAnsiDecoder {
    type Item = AirInputEvent;
    type Error = AirDecodeError;

    fn decode(
        &mut self,
        input: &[u8],
        emit: &mut dyn FnMut(Self::Item),
    ) -> Result<usize, Self::Error> {
        let mut consumed = 0_usize;
        loop {
            let remaining = input.get(consumed..).unwrap_or(&[]);
            let progress = self.decode_once(remaining, emit)?;
            if progress == 0 {
                return Ok(consumed);
            }
            consumed = consumed.saturating_add(progress);
        }
    }
}

#[cfg(test)]
mod tests {
    use air_tui_platform::Decoder;

    use super::{
        AirAnsiDecoder, AirDecodeError, AirKeyboardFlags, MAXIMUM_PASTE_BYTES, Parameters,
        control_key_event, first_character, key_for_final_byte, key_for_tilde_parameter,
        mouse_button, mouse_kind, mouse_modifiers, narrow_coordinate, parse_parameters,
        split_private_prefix,
    };
    use crate::geometry::{AirColumn, AirPoint, AirRow};
    use crate::input::event::{
        AirInputEvent, AirKey, AirKeyEvent, AirKeyState, AirModifiers, AirMouseButton,
        AirMouseEvent, AirMouseEventKind,
    };

    /// Décode une entrée complète et rend les octets consommés avec les événements.
    fn decode(input: &[u8]) -> (usize, Vec<AirInputEvent>) {
        let mut decoder = AirAnsiDecoder::new();
        decode_with(&mut decoder, input)
    }

    fn decode_with(decoder: &mut AirAnsiDecoder, input: &[u8]) -> (usize, Vec<AirInputEvent>) {
        let mut collected = Vec::new();
        let consumed = decoder
            .decode(input, &mut |event| collected.push(event))
            .expect("décodage attendu valide");
        (consumed, collected)
    }

    /// Décode une entrée attendue en échec, et rend l'erreur avec ce qui avait été émis
    /// avant elle.
    ///
    /// Tous les tests d'erreur passent par cette unique fermeture : trois fermetures
    /// `|_| {}` distinctes laisseraient trois corps jamais exécutés, donc trois régions
    /// non couvertes (ADR-012).
    fn decode_failing(
        decoder: &mut AirAnsiDecoder,
        input: &[u8],
    ) -> (AirDecodeError, Vec<AirInputEvent>) {
        let mut collected = Vec::new();
        let error = decoder
            .decode(input, &mut |event| collected.push(event))
            .expect_err("une erreur était attendue");
        (error, collected)
    }

    fn error_of(input: &[u8]) -> AirDecodeError {
        decode_failing(&mut AirAnsiDecoder::new(), input).0
    }

    fn key(logical: AirKey) -> AirInputEvent {
        AirInputEvent::Key(AirKeyEvent::press(logical))
    }

    fn key_with(logical: AirKey, modifiers: AirModifiers) -> AirInputEvent {
        AirInputEvent::Key(AirKeyEvent::press(logical).with_modifiers(modifiers))
    }

    // ---------------------------------------------------------------- texte

    #[test]
    fn une_entree_vide_ne_consomme_rien() {
        assert_eq!(decode(b""), (0, Vec::new()));
    }

    #[test]
    fn un_caractere_ascii_est_decode() {
        assert_eq!(decode(b"a"), (1, vec![key(AirKey::Character('a'))]));
    }

    #[test]
    fn plusieurs_caracteres_sont_decodes_en_un_appel() {
        let (consumed, events) = decode(b"abc");
        assert_eq!(consumed, 3);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn un_caractere_accentue_est_decode_en_un_evenement() {
        assert_eq!(
            decode("é".as_bytes()),
            (2, vec![key(AirKey::Character('é'))])
        );
    }

    #[test]
    fn un_ideogramme_et_un_emoji_sont_decodes() {
        assert_eq!(
            decode("漢".as_bytes()),
            (3, vec![key(AirKey::Character('漢'))])
        );
        assert_eq!(
            decode("🦀".as_bytes()),
            (4, vec![key(AirKey::Character('🦀'))])
        );
    }

    #[test]
    fn un_caractere_coupe_entre_deux_lectures_ne_consomme_rien() {
        // Premier octet d'un « é » sans sa continuation : incomplet, pas une erreur.
        assert_eq!(decode(b"\xc3"), (0, Vec::new()));
        assert_eq!(decode(b"\xf0\x9f\xa6"), (0, Vec::new()));
    }

    #[test]
    fn un_octet_qui_ne_peut_pas_commencer_un_caractere_est_une_erreur() {
        assert_eq!(error_of(b"\x80"), AirDecodeError::InvalidUtf8);
        assert_eq!(error_of(b"\xc0\x80"), AirDecodeError::InvalidUtf8);
        assert_eq!(error_of(b"\xff"), AirDecodeError::InvalidUtf8);
    }

    #[test]
    fn une_continuation_invalide_est_une_erreur() {
        // 0xc3 annonce deux octets, mais 0x28 n'est pas une continuation valide.
        assert_eq!(error_of(b"\xc3\x28"), AirDecodeError::InvalidUtf8);
    }

    #[test]
    fn first_character_rend_le_caractere_nul_sur_un_texte_vide() {
        // Inatteignable depuis le décodeur ; couvert ici (ADR-012).
        assert_eq!(first_character(""), '\u{0}');
        assert_eq!(first_character("ab"), 'a');
    }

    // ------------------------------------------------------- commandes C0

    #[test]
    fn tous_les_octets_de_commande_produisent_un_evenement() {
        for byte in 0_u8..=0x1F {
            if byte == 0x1B {
                // L'échappement a son propre traitement.
                continue;
            }
            let (consumed, events) = decode(&[byte]);
            assert_eq!(consumed, 1, "pour {byte:#04x}");
            assert_eq!(events.len(), 1, "pour {byte:#04x}");
        }
    }

    #[test]
    fn les_commandes_nommees_donnent_la_bonne_touche() {
        assert_eq!(decode(b"\x09"), (1, vec![key(AirKey::Tab)]));
        assert_eq!(decode(b"\x0d"), (1, vec![key(AirKey::Enter)]));
        assert_eq!(decode(b"\x0a"), (1, vec![key(AirKey::Enter)]));
        assert_eq!(decode(b"\x08"), (1, vec![key(AirKey::Backspace)]));
        assert_eq!(decode(b"\x7f"), (1, vec![key(AirKey::Backspace)]));
    }

    #[test]
    fn les_lettres_de_controle_portent_le_modificateur_control() {
        let control = AirModifiers::NONE.with_control();
        assert_eq!(
            decode(b"\x03"),
            (1, vec![key_with(AirKey::Character('c'), control)])
        );
        assert_eq!(
            decode(b"\x01"),
            (1, vec![key_with(AirKey::Character('a'), control)])
        );
        assert_eq!(
            decode(b"\x1a"),
            (1, vec![key_with(AirKey::Character('z'), control)])
        );
        assert_eq!(
            decode(b"\x00"),
            (1, vec![key_with(AirKey::Character(' '), control)])
        );
        assert_eq!(
            decode(b"\x1f"),
            (1, vec![key_with(AirKey::Character('_'), control)])
        );
    }

    #[test]
    fn control_key_event_hors_domaine_rend_un_caractere_nul() {
        // Bras inatteignable depuis le décodeur, couvert directement (ADR-012).
        let event = control_key_event(b'A');
        assert_eq!(event.logical, AirKey::Character('\u{0}'));
        assert_eq!(event.state, AirKeyState::Press);
    }

    // ------------------------------------------------------- échappement

    #[test]
    fn un_echappement_isole_est_incomplet_et_non_une_touche() {
        // C'est le cœur de l'ambiguïté : seul le temps distingue `ESC` d'un début de
        // séquence, et le décodeur ne connaît pas le temps.
        assert_eq!(decode(b"\x1b"), (0, Vec::new()));
    }

    #[test]
    fn l_evenement_d_echappement_isole_est_fourni_a_part() {
        assert_eq!(AirAnsiDecoder::isolated_escape_event(), key(AirKey::Escape));
    }

    #[test]
    fn deux_echappements_donnent_un_echappement_alternatif() {
        assert_eq!(
            decode(b"\x1b\x1b"),
            (
                2,
                vec![key_with(
                    AirKey::Escape,
                    AirModifiers::NONE.with_alternate()
                )]
            )
        );
    }

    #[test]
    fn echappement_plus_lettre_donne_la_touche_alternative() {
        assert_eq!(
            decode(b"\x1ba"),
            (
                2,
                vec![key_with(
                    AirKey::Character('a'),
                    AirModifiers::NONE.with_alternate()
                )]
            )
        );
    }

    #[test]
    fn echappement_plus_caractere_multioctet() {
        let mut input = vec![0x1b];
        input.extend_from_slice("é".as_bytes());
        assert_eq!(
            decode(&input),
            (
                3,
                vec![key_with(
                    AirKey::Character('é'),
                    AirModifiers::NONE.with_alternate()
                )]
            )
        );
    }

    #[test]
    fn echappement_plus_caractere_coupe_ne_consomme_rien() {
        assert_eq!(decode(b"\x1b\xc3"), (0, Vec::new()));
    }

    #[test]
    fn echappement_plus_octet_invalide_est_une_erreur() {
        assert_eq!(error_of(b"\x1b\x80"), AirDecodeError::InvalidUtf8);
    }

    #[test]
    fn echappement_plus_continuation_invalide_est_une_erreur() {
        assert_eq!(error_of(b"\x1b\xc3\x28"), AirDecodeError::InvalidUtf8);
    }

    #[test]
    fn echappement_plus_commande_cumule_control_et_alternative() {
        let (consumed, events) = decode(b"\x1b\x03");
        assert_eq!(consumed, 2);
        let expected = AirModifiers::NONE.with_control().with_alternate();
        assert_eq!(events, vec![key_with(AirKey::Character('c'), expected)]);
    }

    // ------------------------------------------------------------- SS3

    #[test]
    fn ss3_donne_les_quatre_premieres_touches_de_fonction() {
        assert_eq!(decode(b"\x1bOP"), (3, vec![key(AirKey::Function(1))]));
        assert_eq!(decode(b"\x1bOQ"), (3, vec![key(AirKey::Function(2))]));
        assert_eq!(decode(b"\x1bOR"), (3, vec![key(AirKey::Function(3))]));
        assert_eq!(decode(b"\x1bOS"), (3, vec![key(AirKey::Function(4))]));
    }

    #[test]
    fn ss3_donne_aussi_les_fleches_du_pave_d_application() {
        assert_eq!(decode(b"\x1bOA"), (3, vec![key(AirKey::Up)]));
        assert_eq!(decode(b"\x1bOH"), (3, vec![key(AirKey::Home)]));
    }

    #[test]
    fn ss3_incomplet_ne_consomme_rien() {
        assert_eq!(decode(b"\x1bO"), (0, Vec::new()));
    }

    #[test]
    fn ss3_non_reconnu_est_consomme_sans_evenement() {
        assert_eq!(decode(b"\x1bOZ"), (3, Vec::new()));
    }

    // ------------------------------------------------- séquences CSI

    #[test]
    fn une_sequence_csi_incomplete_ne_consomme_rien() {
        assert_eq!(decode(b"\x1b["), (0, Vec::new()));
        assert_eq!(decode(b"\x1b[1"), (0, Vec::new()));
        assert_eq!(decode(b"\x1b[1;5"), (0, Vec::new()));
    }

    #[test]
    fn les_fleches_et_la_navigation_sont_decodees() {
        assert_eq!(decode(b"\x1b[A"), (3, vec![key(AirKey::Up)]));
        assert_eq!(decode(b"\x1b[B"), (3, vec![key(AirKey::Down)]));
        assert_eq!(decode(b"\x1b[C"), (3, vec![key(AirKey::Right)]));
        assert_eq!(decode(b"\x1b[D"), (3, vec![key(AirKey::Left)]));
        assert_eq!(decode(b"\x1b[H"), (3, vec![key(AirKey::Home)]));
        assert_eq!(decode(b"\x1b[F"), (3, vec![key(AirKey::End)]));
    }

    #[test]
    fn les_modificateurs_de_sequence_sont_appliques() {
        // `CSI 1 ; 5 A` : le paramètre vaut le masque plus un, donc 5 signifie contrôle.
        assert_eq!(
            decode(b"\x1b[1;5A"),
            (
                6,
                vec![key_with(AirKey::Up, AirModifiers::NONE.with_control())]
            )
        );
        assert_eq!(
            decode(b"\x1b[1;2C"),
            (
                6,
                vec![key_with(AirKey::Right, AirModifiers::NONE.with_shift())]
            )
        );
    }

    #[test]
    fn la_tabulation_arriere_est_une_tabulation_avec_majuscule() {
        // Il n'y a pas de variante `BackTab` : c'est `Tab` plus `shift`.
        assert_eq!(
            decode(b"\x1b[Z"),
            (
                3,
                vec![key_with(AirKey::Tab, AirModifiers::NONE.with_shift())]
            )
        );
    }

    #[test]
    fn les_evenements_de_focus_sont_decodes() {
        assert_eq!(decode(b"\x1b[I"), (3, vec![AirInputEvent::FocusGained]));
        assert_eq!(decode(b"\x1b[O"), (3, vec![AirInputEvent::FocusLost]));
    }

    #[test]
    fn les_sequences_tilde_donnent_navigation_et_fonctions() {
        assert_eq!(decode(b"\x1b[2~"), (4, vec![key(AirKey::Insert)]));
        assert_eq!(decode(b"\x1b[3~"), (4, vec![key(AirKey::Delete)]));
        assert_eq!(decode(b"\x1b[5~"), (4, vec![key(AirKey::PageUp)]));
        assert_eq!(decode(b"\x1b[6~"), (4, vec![key(AirKey::PageDown)]));
        assert_eq!(decode(b"\x1b[15~"), (5, vec![key(AirKey::Function(5))]));
        assert_eq!(decode(b"\x1b[24~"), (5, vec![key(AirKey::Function(12))]));
        assert_eq!(decode(b"\x1b[34~"), (5, vec![key(AirKey::Function(20))]));
    }

    #[test]
    fn une_sequence_tilde_avec_modificateur() {
        assert_eq!(
            decode(b"\x1b[3;5~"),
            (
                6,
                vec![key_with(AirKey::Delete, AirModifiers::NONE.with_control())]
            )
        );
    }

    #[test]
    fn une_sequence_bien_formee_mais_non_reconnue_est_consommee_sans_evenement() {
        // Un terminal émet des réponses qu'on ne lui a pas demandées : les refuser
        // bloquerait le flux sur la première.
        assert_eq!(decode(b"\x1b[99~"), (5, Vec::new()));
        assert_eq!(decode(b"\x1b[1;2x"), (6, Vec::new()));
        // Un préfixe privé qu'on ne reconnaît pas.
        assert_eq!(decode(b"\x1b[=1A"), (5, Vec::new()));
    }

    #[test]
    fn un_octet_interdit_dans_une_sequence_est_une_erreur() {
        // 0x07 n'est ni un paramètre, ni un intermédiaire, ni un octet final.
        assert_eq!(error_of(b"\x1b[\x07"), AirDecodeError::MalformedSequence);
    }

    #[test]
    fn un_parametre_non_numerique_est_une_erreur() {
        // `>` est dans la plage des paramètres mais n'est ni un chiffre, ni un séparateur.
        // Hors première position il ne peut pas être un préfixe privé : c'est donc une
        // séquence malformée.
        assert_eq!(error_of(b"\x1b[1>A"), AirDecodeError::MalformedSequence);
    }

    #[test]
    fn un_deux_points_est_un_separateur_et_non_une_erreur() {
        // Il l'était avant le protocole Kitty, qui s'en sert pour ses sous-paramètres.
        assert_eq!(decode(b"\x1b[1:2A"), (6, vec![key(AirKey::Up)]));
    }

    #[test]
    fn un_parametre_qui_deborde_est_une_erreur() {
        // La borne est celle d'un u32 depuis que Kitty transmet des points de code.
        assert_eq!(
            error_of(b"\x1b[99999999999A"),
            AirDecodeError::MalformedSequence
        );
    }

    #[test]
    fn une_sequence_aux_parametres_demesures_est_une_erreur() {
        let mut input = b"\x1b[".to_vec();
        input.extend(core::iter::repeat_n(b'1', 100));
        input.push(b'A');
        assert_eq!(error_of(&input), AirDecodeError::MalformedSequence);
    }

    #[test]
    fn les_parametres_au_dela_du_troisieme_sont_valides_mais_ignores() {
        // Validés — une séquence malformée reste une erreur — mais pas conservés.
        assert_eq!(
            decode(b"\x1b[1;5;3;4;9;9A"),
            (
                14,
                vec![key_with(AirKey::Up, AirModifiers::NONE.with_control())]
            )
        );
        assert_eq!(
            error_of(b"\x1b[1;5;3;4;9;>A"),
            AirDecodeError::MalformedSequence
        );
    }

    #[test]
    fn un_parametre_vide_vaut_sa_valeur_par_defaut() {
        // `CSI ; 5 A` : premier paramètre absent, modificateur présent.
        assert_eq!(
            decode(b"\x1b[;5A"),
            (
                5,
                vec![key_with(AirKey::Up, AirModifiers::NONE.with_control())]
            )
        );
    }

    // ----------------------------------------------------------- souris

    fn mouse(column: u16, row: u16, kind: AirMouseEventKind) -> AirInputEvent {
        AirInputEvent::Mouse(AirMouseEvent {
            position: AirPoint::new(AirColumn::new(column), AirRow::new(row)),
            kind,
            modifiers: AirModifiers::NONE,
        })
    }

    #[test]
    fn un_enfoncement_de_souris_est_decode_en_coordonnees_depuis_zero() {
        // Le terminal compte depuis un ; la grille depuis zéro.
        assert_eq!(
            decode(b"\x1b[<0;10;20M"),
            (
                11,
                vec![mouse(
                    9,
                    19,
                    AirMouseEventKind::Pressed(AirMouseButton::Left)
                )]
            )
        );
    }

    #[test]
    fn les_trois_boutons_sont_distingues() {
        assert_eq!(
            decode(b"\x1b[<1;1;1M"),
            (
                9,
                vec![mouse(
                    0,
                    0,
                    AirMouseEventKind::Pressed(AirMouseButton::Middle)
                )]
            )
        );
        assert_eq!(
            decode(b"\x1b[<2;1;1M"),
            (
                9,
                vec![mouse(
                    0,
                    0,
                    AirMouseEventKind::Pressed(AirMouseButton::Right)
                )]
            )
        );
    }

    #[test]
    fn un_relachement_est_distingue_par_l_octet_final() {
        assert_eq!(
            decode(b"\x1b[<0;1;1m"),
            (
                9,
                vec![mouse(
                    0,
                    0,
                    AirMouseEventKind::Released(Some(AirMouseButton::Left))
                )]
            )
        );
    }

    #[test]
    fn un_relachement_sans_bouton_reste_un_relachement() {
        // L'encodage permet de ne pas dire quel bouton : inventer une réponse serait une
        // devinette.
        assert_eq!(
            decode(b"\x1b[<3;1;1m"),
            (9, vec![mouse(0, 0, AirMouseEventKind::Released(None))])
        );
    }

    #[test]
    fn les_quatre_directions_de_molette_sont_decodees() {
        assert_eq!(
            decode(b"\x1b[<64;1;1M"),
            (10, vec![mouse(0, 0, AirMouseEventKind::WheelUp)])
        );
        assert_eq!(
            decode(b"\x1b[<65;1;1M"),
            (10, vec![mouse(0, 0, AirMouseEventKind::WheelDown)])
        );
        assert_eq!(
            decode(b"\x1b[<66;1;1M"),
            (10, vec![mouse(0, 0, AirMouseEventKind::WheelLeft)])
        );
        assert_eq!(
            decode(b"\x1b[<67;1;1M"),
            (10, vec![mouse(0, 0, AirMouseEventKind::WheelRight)])
        );
    }

    #[test]
    fn un_deplacement_est_decode_avec_le_bouton_maintenu() {
        assert_eq!(
            decode(b"\x1b[<32;4;5M"),
            (
                10,
                vec![mouse(
                    3,
                    4,
                    AirMouseEventKind::Moved(Some(AirMouseButton::Left))
                )]
            )
        );
        assert_eq!(
            decode(b"\x1b[<35;4;5M"),
            (10, vec![mouse(3, 4, AirMouseEventKind::Moved(None))])
        );
    }

    #[test]
    fn un_enfoncement_sans_bouton_ne_produit_aucun_evenement() {
        // Le code 3 hors molette, hors déplacement et hors relâchement ne décrit rien de
        // sensé : la séquence est consommée, mais rien n'est inventé.
        assert_eq!(decode(b"\x1b[<3;1;1M"), (9, Vec::new()));
    }

    #[test]
    fn les_modificateurs_de_souris_sont_decodes() {
        // 20 = 16 (contrôle) + 4 (majuscule) + 0 (bouton gauche)
        let (consumed, events) = decode(b"\x1b[<20;1;1M");
        assert_eq!(consumed, 10);
        assert!(matches!(
            events.first(),
            Some(AirInputEvent::Mouse(event))
                if event.modifiers.control
                    && event.modifiers.shift
                    && !event.modifiers.alternate
        ));
    }

    #[test]
    fn un_parametre_de_souris_malforme_est_une_erreur() {
        assert_eq!(error_of(b"\x1b[<0;>;1M"), AirDecodeError::MalformedSequence);
    }

    #[test]
    fn des_parametres_de_souris_absents_prennent_leurs_valeurs_par_defaut() {
        // Sans coordonnées, le terminal est censé compter depuis un : on retombe sur
        // l'origine, et non sur une position négative.
        assert_eq!(
            decode(b"\x1b[<0M"),
            (
                5,
                vec![mouse(
                    0,
                    0,
                    AirMouseEventKind::Pressed(AirMouseButton::Left)
                )]
            )
        );
    }

    // ---------------------------------------------------------- collage

    #[test]
    fn un_collage_encadre_est_livre_en_un_seul_evenement() {
        let (consumed, events) = decode(b"\x1b[200~bonjour\x1b[201~");
        assert_eq!(consumed, 6 + 7 + 6);
        assert_eq!(events, vec![AirInputEvent::Paste("bonjour".to_owned())]);
    }

    #[test]
    fn un_collage_vide_est_livre_comme_tel() {
        let (_, events) = decode(b"\x1b[200~\x1b[201~");
        assert_eq!(events, vec![AirInputEvent::Paste(String::new())]);
    }

    #[test]
    fn un_collage_accentue_traverse_intact() {
        let (_, events) = decode("\x1b[200~été\x1b[201~".as_bytes());
        assert_eq!(events, vec![AirInputEvent::Paste("été".to_owned())]);
    }

    #[test]
    fn un_collage_reparti_sur_plusieurs_lectures_est_recolle() {
        let mut decoder = AirAnsiDecoder::new();
        let (_, first) = decode_with(&mut decoder, b"\x1b[200~bon");
        assert_eq!(first, Vec::new());
        assert!(decoder.is_pasting());
        let (_, second) = decode_with(&mut decoder, b"jour\x1b[201~");
        assert_eq!(second, vec![AirInputEvent::Paste("bonjour".to_owned())]);
        assert!(!decoder.is_pasting());
    }

    #[test]
    fn un_terminateur_coupe_en_deux_lectures_n_est_pas_perdu() {
        // C'est le piège du collage : consommer naïvement tout le morceau avalerait le
        // début du terminateur, et le collage ne se fermerait jamais.
        let mut decoder = AirAnsiDecoder::new();
        let (consumed, _) = decode_with(&mut decoder, b"\x1b[200~texte\x1b[20");
        // Les quatre octets du terminateur partiel sont conservés pour la suite.
        assert_eq!(consumed, 6 + 5);
        let (_, events) = decode_with(&mut decoder, b"\x1b[201~");
        assert_eq!(events, vec![AirInputEvent::Paste("texte".to_owned())]);
    }

    #[test]
    fn un_collage_non_utf8_est_une_erreur() {
        let mut input = b"\x1b[200~".to_vec();
        input.push(0xFF);
        input.extend_from_slice(b"\x1b[201~");
        assert_eq!(error_of(&input), AirDecodeError::InvalidUtf8);
    }

    #[test]
    fn un_collage_demesure_est_refuse_sur_le_morceau_terminal() {
        let mut decoder = AirAnsiDecoder::new();
        let _ = decode_with(&mut decoder, b"\x1b[200~");
        let oversized = vec![b'x'; MAXIMUM_PASTE_BYTES + 1];
        let mut input = oversized;
        input.extend_from_slice(b"\x1b[201~");
        assert_eq!(
            decode_failing(&mut decoder, &input).0,
            AirDecodeError::PasteTooLarge
        );
    }

    #[test]
    fn un_collage_demesure_est_refuse_sur_un_morceau_intermediaire() {
        let mut decoder = AirAnsiDecoder::new();
        let _ = decode_with(&mut decoder, b"\x1b[200~");
        let oversized = vec![b'x'; MAXIMUM_PASTE_BYTES + 1];
        assert_eq!(
            decode_failing(&mut decoder, &oversized).0,
            AirDecodeError::PasteTooLarge
        );
    }

    #[test]
    fn une_fermeture_de_collage_hors_collage_est_consommee_sans_evenement() {
        assert_eq!(decode(b"\x1b[201~"), (6, Vec::new()));
    }

    #[test]
    fn un_decodeur_neuf_ne_colle_pas() {
        assert!(!AirAnsiDecoder::new().is_pasting());
        assert!(!AirAnsiDecoder::default().is_pasting());
    }

    // -------------------------------------------------- flux composites

    #[test]
    fn un_flux_melange_est_decode_dans_l_ordre() {
        let (consumed, events) = decode(b"a\x1b[Ab\x1b[<0;1;1M\x0d");
        assert_eq!(consumed, 1 + 3 + 1 + 9 + 1);
        assert_eq!(events.len(), 5);
        assert_eq!(events.first(), Some(&key(AirKey::Character('a'))));
        assert_eq!(events.get(1), Some(&key(AirKey::Up)));
        assert_eq!(events.get(4), Some(&key(AirKey::Enter)));
    }

    #[test]
    fn les_evenements_precedant_une_erreur_sont_conserves() {
        // L'erreur interrompt le décodage, mais ne rétracte pas ce qui a déjà été
        // produit : l'appelant garde les touches valides qui précédaient.
        let (error, events) = decode_failing(&mut AirAnsiDecoder::new(), b"ab\xff");
        assert_eq!(error, AirDecodeError::InvalidUtf8);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn un_flux_qui_finit_par_un_prefixe_incomplet_consomme_le_reste() {
        let (consumed, events) = decode(b"ab\x1b[");
        assert_eq!(consumed, 2);
        assert_eq!(events.len(), 2);
    }

    // ------------------------------------------- protocole clavier Kitty

    #[test]
    fn une_frappe_kitty_simple_est_decodee() {
        assert_eq!(decode(b"\x1b[97u"), (5, vec![key(AirKey::Character('a'))]));
    }

    #[test]
    fn une_frappe_kitty_porte_ses_modificateurs() {
        assert_eq!(
            decode(b"\x1b[97;5u"),
            (
                7,
                vec![key_with(
                    AirKey::Character('a'),
                    AirModifiers::NONE.with_control()
                )]
            )
        );
    }

    #[test]
    fn kitty_distingue_ce_que_le_codage_historique_ne_sait_pas_exprimer() {
        // C'est la raison d'être du jalon : sur un pty sans Kitty, `Ctrl+Shift+S` et
        // `Ctrl+S` produisent le même octet 0x13. Ici ils sont distincts.
        let control_only = decode(b"\x1b[115;5u").1;
        let control_shift = decode(b"\x1b[115;6u").1;
        assert_ne!(control_only, control_shift);
        assert_eq!(
            control_shift,
            vec![key_with(
                AirKey::Character('s'),
                AirModifiers::NONE.with_control().with_shift()
            )]
        );
    }

    #[test]
    fn les_trois_types_d_evenement_sont_rapportes() {
        // `matches!` plutôt qu'une fermeture avec un bras par défaut : ce bras serait
        // hors d'atteinte, donc une région non couverte (ADR-012).
        for (input, expected) in [
            (&b"\x1b[97;1:1u"[..], AirKeyState::Press),
            (&b"\x1b[97;1:2u"[..], AirKeyState::Repeat),
            (&b"\x1b[97;1:3u"[..], AirKeyState::Release),
        ] {
            let (_, events) = decode(input);
            assert!(
                matches!(events.first(), Some(AirInputEvent::Key(event)) if event.state == expected),
                "état attendu {expected:?} pour {input:?}"
            );
        }
    }

    #[test]
    fn la_disposition_de_base_est_rapportee_et_sert_aux_raccourcis() {
        // Sur un clavier cyrillique, la touche du `a` produit `ф`. Un raccourci doit
        // continuer de fonctionner : c'est à cela que sert la disposition de base.
        let (consumed, events) = decode("\x1b[1092:0:97;3u".as_bytes());
        assert_eq!(consumed, 14);
        assert!(matches!(
            events.first(),
            Some(AirInputEvent::Key(event))
                if event.logical == AirKey::Character('ф')
                    && event.base_layout == Some(AirKey::Character('a'))
                    && event.shortcut_key() == AirKey::Character('a')
                    && event.modifiers.alternate
        ));
    }

    #[test]
    fn le_texte_associe_prime_sur_le_numero_de_touche() {
        // La touche de base est `e`, mais la composition a produit `é` : c'est `é` que
        // l'application doit insérer.
        let (_, events) = decode(b"\x1b[101;1;233u");
        assert_eq!(events, vec![key(AirKey::Character('é'))]);
    }

    #[test]
    fn un_point_de_code_hors_du_plan_de_base_est_decode() {
        // Prouve l'élargissement des paramètres en u32 : 128512 ne tient pas dans un u16.
        assert_eq!(
            decode(b"\x1b[128512u"),
            (9, vec![key(AirKey::Character('\u{1F600}'))])
        );
    }

    #[test]
    fn les_touches_nommees_de_kitty_sont_decodees() {
        assert_eq!(decode(b"\x1b[27u"), (5, vec![key(AirKey::Escape)]));
        assert_eq!(decode(b"\x1b[13u"), (5, vec![key(AirKey::Enter)]));
        assert_eq!(decode(b"\x1b[9u"), (4, vec![key(AirKey::Tab)]));
        assert_eq!(decode(b"\x1b[127u"), (6, vec![key(AirKey::Backspace)]));
        assert_eq!(decode(b"\x1b[57376u"), (8, vec![key(AirKey::Function(13))]));
    }

    #[test]
    fn un_numero_de_touche_hors_table_est_consomme_sans_evenement() {
        // Pavé numérique et touches modificatrices seules : hors table. Les laisser
        // passer comme caractères de la zone à usage privé insérerait des glyphes
        // fantômes dans les champs de saisie.
        assert_eq!(decode(b"\x1b[57399u"), (8, Vec::new()));
        assert_eq!(decode(b"\x1b[57441u"), (8, Vec::new()));
    }

    #[test]
    fn un_texte_associe_hors_table_retombe_sur_le_numero_de_touche() {
        // Le troisième champ ne décrit pas un caractère : la touche de base reste.
        assert_eq!(
            decode(b"\x1b[97;1;3u"),
            (9, vec![key(AirKey::Character('a'))])
        );
    }

    // ------------------------------------------ réponses du terminal

    #[test]
    fn la_reponse_du_protocole_clavier_est_decodee() {
        let (consumed, events) = decode(b"\x1b[?23u");
        assert_eq!(consumed, 6);
        assert_eq!(
            events,
            vec![AirInputEvent::KeyboardProtocolReported(
                AirKeyboardFlags::WANTED
            )]
        );
    }

    #[test]
    fn une_reponse_de_protocole_vide_reste_une_reponse() {
        // C'est son arrivée qui prouve la prise en charge, pas son contenu.
        let (_, events) = decode(b"\x1b[?0u");
        assert_eq!(
            events,
            vec![AirInputEvent::KeyboardProtocolReported(
                AirKeyboardFlags::default()
            )]
        );
    }

    #[test]
    fn les_attributs_de_peripherique_servent_de_jalon() {
        assert_eq!(
            decode(b"\x1b[?62;1;6c"),
            (10, vec![AirInputEvent::DeviceAttributesReported])
        );
    }

    #[test]
    fn une_autre_reponse_du_terminal_est_consommee_sans_evenement() {
        assert_eq!(decode(b"\x1b[?1;2R"), (7, Vec::new()));
    }

    #[test]
    fn split_private_prefix_detache_les_quatre_prefixes_reserves() {
        assert_eq!(split_private_prefix(b"<0;1;1"), (Some(b'<'), &b"0;1;1"[..]));
        assert_eq!(split_private_prefix(b"=1"), (Some(b'='), &b"1"[..]));
        assert_eq!(split_private_prefix(b">4;2"), (Some(b'>'), &b"4;2"[..]));
        assert_eq!(split_private_prefix(b"?23"), (Some(b'?'), &b"23"[..]));
        assert_eq!(split_private_prefix(b"1;5"), (None, &b"1;5"[..]));
        assert_eq!(split_private_prefix(b""), (None, &b""[..]));
    }

    // ----------------------------------------------- modifyOtherKeys

    #[test]
    fn modify_other_keys_est_decode() {
        // Le repli d'xterm et de VTE : `CSI 27 ; modificateurs ; numéro ~`.
        assert_eq!(
            decode(b"\x1b[27;5;115~"),
            (
                11,
                vec![key_with(
                    AirKey::Character('s'),
                    AirModifiers::NONE.with_control()
                )]
            )
        );
    }

    #[test]
    fn modify_other_keys_distingue_aussi_control_shift() {
        assert_eq!(
            decode(b"\x1b[27;6;83~"),
            (
                10,
                vec![key_with(
                    AirKey::Character('S'),
                    AirModifiers::NONE.with_control().with_shift()
                )]
            )
        );
    }

    #[test]
    fn modify_other_keys_hors_table_est_consomme_sans_evenement() {
        assert_eq!(decode(b"\x1b[27;1;57399~"), (13, Vec::new()));
    }

    // --------------------------------------- bornes et cas dégénérés

    #[test]
    fn une_coordonnee_de_souris_demesuree_sature_au_lieu_de_tronquer() {
        // Tronquer ferait atterrir le clic à une position arbitraire ; saturer le place au
        // bord, ce qui est faux mais pas absurde.
        assert_eq!(
            decode(b"\x1b[<0;70000;1M"),
            (
                13,
                vec![mouse(
                    u16::MAX,
                    0,
                    AirMouseEventKind::Pressed(AirMouseButton::Left)
                )]
            )
        );
    }

    #[test]
    fn narrow_coordinate_ramene_depuis_un_et_sature() {
        assert_eq!(narrow_coordinate(0), 0);
        assert_eq!(narrow_coordinate(1), 0);
        assert_eq!(narrow_coordinate(10), 9);
        assert_eq!(narrow_coordinate(70_000), u16::MAX);
    }

    #[test]
    fn les_champs_au_dela_du_troisieme_sont_vides() {
        let parameters = parse_parameters(b"1:2;3:4;5:6;7:8").expect("paramètres valides");
        assert_eq!(parameters.value_or(3, 99), 99);
        assert_eq!(parameters.first_sub_or(3, 99), 99);
        assert_eq!(parameters.second_sub(3), None);
    }

    #[test]
    fn les_sous_parametres_absents_prennent_leur_repli() {
        let parameters = parse_parameters(b"97").expect("paramètres valides");
        assert_eq!(parameters.first_sub_or(0, 7), 7);
        assert_eq!(parameters.second_sub(0), None);
    }

    // ---------------------------------------------- fonctions auxiliaires

    #[test]
    fn value_or_rend_le_repli_au_dela_des_parametres_lus() {
        let parameters = Parameters::default();
        assert_eq!(parameters.value_or(0, 7), 7);
        assert_eq!(parameters.value_or(9, 7), 7);
    }

    #[test]
    fn value_or_rend_le_repli_pour_un_parametre_vide() {
        let parameters = parse_parameters(b";5").expect("paramètres valides");
        assert_eq!(parameters.value_or(0, 42), 42);
        assert_eq!(parameters.value_or(1, 0), 5);
    }

    #[test]
    fn parse_parameters_d_une_entree_vide_ne_lit_rien() {
        let parameters = parse_parameters(b"").expect("entrée vide valide");
        assert_eq!(parameters.value_or(0, 3), 3);
    }

    #[test]
    fn key_for_tilde_parameter_couvre_les_valeurs_connues_et_refuse_les_autres() {
        assert_eq!(key_for_tilde_parameter(1), Some(AirKey::Home));
        assert_eq!(key_for_tilde_parameter(7), Some(AirKey::Home));
        assert_eq!(key_for_tilde_parameter(4), Some(AirKey::End));
        assert_eq!(key_for_tilde_parameter(8), Some(AirKey::End));
        for parameter in [
            11_u32, 12, 13, 14, 17, 18, 19, 20, 21, 23, 25, 26, 28, 29, 31, 32, 33,
        ] {
            assert!(
                key_for_tilde_parameter(parameter).is_some(),
                "paramètre {parameter}"
            );
        }
        for parameter in [0_u32, 9, 10, 16, 22, 30, 35, 999] {
            assert_eq!(
                key_for_tilde_parameter(parameter),
                None,
                "paramètre {parameter}"
            );
        }
    }

    #[test]
    fn key_for_final_byte_couvre_les_octets_connus() {
        assert_eq!(key_for_final_byte(b'E'), Some(AirKey::Character('5')));
        assert_eq!(key_for_final_byte(b'G'), None);
    }

    #[test]
    fn mouse_button_refuse_le_code_sans_bouton() {
        assert_eq!(mouse_button(0), Some(AirMouseButton::Left));
        assert_eq!(mouse_button(3), None);
    }

    #[test]
    fn mouse_modifiers_lit_les_trois_bits() {
        let all = mouse_modifiers(4 | 8 | 16);
        assert!(all.shift && all.alternate && all.control);
        assert!(!all.super_key);
        assert!(mouse_modifiers(0).is_empty());
    }

    #[test]
    fn mouse_kind_ne_rend_rien_pour_un_code_sans_sens() {
        assert_eq!(mouse_kind(3, false), None);
    }

    #[test]
    fn l_affichage_des_erreurs_distingue_les_trois_causes() {
        let rendered = [
            AirDecodeError::InvalidUtf8.to_string(),
            AirDecodeError::MalformedSequence.to_string(),
            AirDecodeError::PasteTooLarge.to_string(),
        ];
        let mut sorted = rendered.to_vec();
        sorted.sort_unstable();
        let before = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), before, "deux causes partagent un libellé");
    }
}
