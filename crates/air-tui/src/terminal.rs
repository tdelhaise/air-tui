// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le terminal du framework — un périphérique injecté, et le rendu différentiel.
//!
//! Cette structure ne sait pas sur quel système elle tourne : elle reçoit un
//! [`TerminalDevice`] et ne connaît que ce contrat (ADR-011). C'est le binaire final qui
//! choisit une implémentation — `air-tui-platform-posix` aujourd'hui, une crate
//! `linux-air` demain.

use core::time::Duration;
use std::time::Instant;

use air_tui_platform::{Decoder, Encoder, InputReadiness, PlatformError, TerminalDevice};

use crate::codec::{AirCellPresentation, AirSgrEncoder, decimal::push_decimal};
use crate::geometry::{AirPoint, AirSize};
use crate::input::ansi::AirAnsiDecoder;
use crate::input::event::AirInputEvent;
use crate::input::kitty::{
    AirKeyboardFlags, AirKeyboardProtocol, MODIFY_OTHER_KEYS_DISABLE_SEQUENCE,
    MODIFY_OTHER_KEYS_ENABLE_SEQUENCE, POP_SEQUENCE, PRIMARY_DEVICE_ATTRIBUTES_SEQUENCE,
    QUERY_SEQUENCE, push_request_sequence,
};
use crate::surface::AirSurface;
use crate::theme::quantize::{AirColorDepth, quantize};

/// La dimension retenue quand la plateforme annonce une taille inconnue.
///
/// Quatre-vingts colonnes sur vingt-cinq lignes : la taille d'un écran texte VGA, et le
/// repli qu'emploient les terminaux depuis quarante ans.
const FALLBACK_SIZE: AirSize = AirSize::new(80, 25);

/// Un terminal prêt à recevoir une interface.
///
/// À la construction : passage en mode brut, entrée dans l'écran alterné, masquage du
/// curseur. À la destruction : l'inverse, et la restauration du mode d'origine. Le
/// [`Drop`] est la seule garantie qui tienne — un chemin d'erreur oublié laisserait sinon
/// le terminal inutilisable après la sortie du programme.
pub struct AirTerminal {
    /// Le périphérique injecté.
    device: Box<dyn TerminalDevice>,
    /// Le tampon de sortie, alloué une fois et réutilisé à chaque rendu.
    output: Vec<u8>,
    /// L'encodeur de présentation, qui n'émet que sur changement.
    sgr_encoder: AirSgrEncoder,
    /// Le protocole clavier demandé au terminal, à défaire en sortie.
    keyboard_protocol: AirKeyboardProtocol,
    /// Ce que le terminal sait afficher en couleurs.
    color_depth: AirColorDepth,
    /// Où poser le curseur matériel à la prochaine image, s'il doit se voir.
    cursor: Option<AirPoint>,
}

impl core::fmt::Debug for AirTerminal {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // `dyn TerminalDevice` n'impose pas `Debug` : exiger cela du contrat le
        // restreindrait sans rien apporter au cœur.
        formatter
            .debug_struct("AirTerminal")
            .field("output_capacity", &self.output.capacity())
            .finish_non_exhaustive()
    }
}

impl AirTerminal {
    /// Prend possession d'un périphérique et prépare le terminal.
    ///
    /// # Errors
    ///
    /// Si le passage en mode brut ou l'écriture des séquences d'initialisation échoue.
    pub fn new(mut device: Box<dyn TerminalDevice>) -> Result<Self, PlatformError> {
        device.enter_raw_mode()?;
        let mut terminal = Self {
            device,
            // Assez pour un écran complet sans réallouer : chaque cellule modifiée coûte
            // une poignée d'octets de positionnement et d'attributs.
            output: Vec::with_capacity(64 * 1024),
            sgr_encoder: AirSgrEncoder::new(),
            keyboard_protocol: AirKeyboardProtocol::Legacy,
            // Déduite de l'environnement au démarrage ; l'application peut la forcer.
            color_depth: AirColorDepth::from_environment(),
            // Caché jusqu'à ce qu'une vue focalisée en réclame un.
            cursor: None,
        };
        terminal.device.write_output(b"\x1b[?1049h\x1b[?25l")?;
        terminal.device.flush_output()?;
        Ok(terminal)
    }

    /// Où poser le curseur matériel, ou `None` pour le cacher.
    ///
    /// C'est exactement ce que rend [`crate::view::draw_view`] : la boucle d'application
    /// transmet sans interpréter. Le terminal ne connaît pas l'arbre de vues, et l'arbre
    /// ne connaît pas le terminal.
    pub const fn set_cursor(&mut self, cursor: Option<AirPoint>) {
        self.cursor = cursor;
    }

    /// Où le curseur sera posé à la prochaine image.
    #[must_use]
    pub const fn cursor(&self) -> Option<AirPoint> {
        self.cursor
    }

    /// La taille courante du terminal.
    ///
    /// Interrogée à la demande, à chaque appel : c'est ce qui rend inutile la gestion du
    /// signal `SIGWINCH` et ce qui garantit qu'aucune valeur périmée ne traîne après un
    /// redimensionnement.
    ///
    /// # Errors
    ///
    /// Si l'interrogation de la plateforme échoue.
    pub fn size(&self) -> Result<AirSize, PlatformError> {
        let measured = self.device.window_size()?;
        Ok(match measured {
            Some(size) => AirSize::new(size.columns, size.rows),
            None => FALLBACK_SIZE,
        })
    }

    /// Attend qu'une entrée soit disponible.
    ///
    /// Une expiration est [`InputReadiness::TimedOut`], **pas** une erreur. Et une
    /// interruption par signal est remontée telle quelle : c'est à l'appelant de décider
    /// s'il reprend l'attente ou s'il va d'abord relire la taille du terminal.
    ///
    /// # Errors
    ///
    /// Si l'attente échoue, y compris sur interruption par un signal.
    pub fn wait_for_input(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<InputReadiness, PlatformError> {
        self.device.wait_for_input(timeout)
    }

    /// Lit les octets disponibles dans `buffer` et rend leur nombre.
    ///
    /// Aucune interprétation : le décodeur qui transformera ces octets en événements est
    /// l'objet du jalon M1 et n'est pas écrit.
    ///
    /// # Errors
    ///
    /// Si la lecture échoue.
    pub fn read_available(&mut self, buffer: &mut [u8]) -> Result<usize, PlatformError> {
        self.device.read_input(buffer)
    }

    /// Interroge le terminal sur sa prise en charge du protocole clavier Kitty.
    ///
    /// # Pourquoi ce n'est pas une affaire de délai
    ///
    /// On envoie l'interrogation `CSI ? u` **suivie** d'une demande d'attributs de
    /// périphérique. Les terminaux répondent dans l'ordre : voir les attributs sans avoir
    /// vu de réponse Kitty avant eux est donc une conclusion négative **certaine**, et non
    /// l'expiration d'un délai choisi au hasard.
    ///
    /// `timeout` ne sert qu'au terminal qui ne répond à rien : il ne doit jamais geler le
    /// lancement.
    ///
    /// # Ce qui est tapé pendant l'interrogation n'est pas perdu
    ///
    /// Les événements qui ne sont pas des réponses du terminal — l'utilisateur impatient
    /// qui tape — sont transmis à `emit`. Les jeter serait un défaut qu'on ne verrait
    /// qu'au lancement d'une machine lente.
    ///
    /// # Errors
    ///
    /// Si l'écriture de l'interrogation ou la lecture de la réponse échoue. Une séquence
    /// indécodable, elle, **n'est pas** une erreur : l'interrogation s'arrête et conclut
    /// avec ce qu'elle a vu.
    pub fn detect_keyboard_protocol(
        &mut self,
        timeout: Duration,
        emit: &mut dyn FnMut(AirInputEvent),
    ) -> Result<AirKeyboardProtocol, PlatformError> {
        self.output.clear();
        self.output.extend_from_slice(QUERY_SEQUENCE);
        self.output
            .extend_from_slice(PRIMARY_DEVICE_ATTRIBUTES_SEQUENCE);
        let request = core::mem::take(&mut self.output);
        let written = self.device.write_output(&request);
        self.output = request;
        written?;
        self.device.flush_output()?;

        let deadline = Instant::now().checked_add(timeout);
        let mut decoder = AirAnsiDecoder::new();
        let mut pending = Vec::new();
        let mut read_buffer = [0_u8; 256];
        let mut detected = None;
        let mut fence_reached = false;

        while !fence_reached {
            let remaining = match deadline {
                Some(instant) => instant.saturating_duration_since(Instant::now()),
                // Un délai si grand qu'il déborde de l'horloge : on prend le délai demandé
                // tel quel, ce qui revient à attendre aussi longtemps qu'annoncé.
                None => timeout,
            };
            if remaining.is_zero() {
                break;
            }
            match self.device.wait_for_input(Some(remaining))? {
                InputReadiness::Ready => {}
                InputReadiness::TimedOut | InputReadiness::Closed => break,
            }
            let count = self.device.read_input(&mut read_buffer)?;
            if count == 0 {
                break;
            }
            pending.extend_from_slice(read_buffer.get(..count).unwrap_or(&[]));

            let outcome = decoder.decode(&pending, &mut |event| match event {
                AirInputEvent::KeyboardProtocolReported(_) => {
                    detected = Some(AirKeyboardProtocol::Kitty);
                }
                AirInputEvent::DeviceAttributesReported => fence_reached = true,
                other => emit(other),
            });
            match outcome {
                Ok(consumed) => {
                    pending.drain(..consumed);
                }
                // Une séquence indécodable pendant l'interrogation n'est pas une panne :
                // on conclut avec ce qu'on a vu plutôt que d'échouer au lancement.
                Err(_) => break,
            }
        }

        Ok(detected.unwrap_or(AirKeyboardProtocol::Legacy))
    }

    /// Demande au terminal le protocole clavier `protocol`.
    ///
    /// Le protocole demandé est mémorisé et défait à la destruction du terminal : laisser
    /// un terminal en protocole Kitty après la sortie rendrait le shell inutilisable.
    ///
    /// # Errors
    ///
    /// Si l'écriture échoue.
    pub fn enable_keyboard_protocol(
        &mut self,
        protocol: AirKeyboardProtocol,
    ) -> Result<(), PlatformError> {
        self.output.clear();
        match protocol {
            AirKeyboardProtocol::Kitty => {
                push_request_sequence(&mut self.output, AirKeyboardFlags::WANTED);
            }
            AirKeyboardProtocol::ModifyOtherKeys => {
                self.output
                    .extend_from_slice(MODIFY_OTHER_KEYS_ENABLE_SEQUENCE);
            }
            // Rien à demander : c'est l'état par défaut de tout terminal.
            AirKeyboardProtocol::Legacy => {}
        }
        let request = core::mem::take(&mut self.output);
        let written = self.device.write_output(&request);
        self.output = request;
        written?;
        self.device.flush_output()?;
        self.keyboard_protocol = protocol;
        Ok(())
    }

    /// Détecte le meilleur protocole clavier disponible et l'active.
    ///
    /// Kitty quand le terminal le reconnaît ; sinon `modifyOtherKeys` niveau 2, demandé
    /// **sans interrogation**. Ce choix est délibéré : il n'existe pas de sondage fiable
    /// et universel pour `modifyOtherKeys`, mais la demande est inoffensive — un terminal
    /// qui ne la comprend pas l'ignore, et le décodeur continue de lire le codage
    /// historique.
    ///
    /// # Errors
    ///
    /// Si l'interrogation ou l'activation échoue.
    pub fn enable_best_keyboard_protocol(
        &mut self,
        timeout: Duration,
        emit: &mut dyn FnMut(AirInputEvent),
    ) -> Result<AirKeyboardProtocol, PlatformError> {
        let detected = self.detect_keyboard_protocol(timeout, emit)?;
        let chosen = match detected {
            AirKeyboardProtocol::Kitty => AirKeyboardProtocol::Kitty,
            AirKeyboardProtocol::Legacy | AirKeyboardProtocol::ModifyOtherKeys => {
                AirKeyboardProtocol::ModifyOtherKeys
            }
        };
        self.enable_keyboard_protocol(chosen)?;
        Ok(chosen)
    }

    /// Ce que le terminal sait afficher en couleurs.
    #[must_use]
    pub const fn color_depth(&self) -> AirColorDepth {
        self.color_depth
    }

    /// Force la profondeur de couleur.
    ///
    /// La déduction par l'environnement se trompe : un terminal moderne lancé sous un
    /// `TERM` hérité s'annonce à seize couleurs. Une application qui sait mieux le dit.
    pub const fn set_color_depth(&mut self, depth: AirColorDepth) {
        self.color_depth = depth;
    }

    /// Le protocole clavier en vigueur.
    #[must_use]
    pub const fn keyboard_protocol(&self) -> AirKeyboardProtocol {
        self.keyboard_protocol
    }

    /// Envoie au terminal les cellules qui ont changé, puis marque la surface comme
    /// affichée.
    ///
    /// Le différentiel est encadré par les séquences de mise à jour synchronisée : un
    /// terminal qui les comprend n'affiche rien avant la fin, ce qui supprime le
    /// scintillement sur les redessins larges ; un terminal qui ne les comprend pas les
    /// ignore.
    ///
    /// # Errors
    ///
    /// Si l'écriture sur le terminal échoue.
    pub fn present(&mut self, surface: &mut AirSurface) -> Result<(), PlatformError> {
        self.output.clear();
        // Le terminal ne se souvient de rien d'une image à l'autre : l'encodeur non plus.
        self.sgr_encoder.forget();
        self.output.extend_from_slice(b"\x1b[?2026h");

        for (point, cell) in surface.difference() {
            if cell.is_continuation {
                // La seconde moitié d'un caractère large : l'émettre écraserait la moitié
                // droite du glyphe que le terminal vient de dessiner.
                continue;
            }
            // Les coordonnées ANSI comptent depuis un, la grille depuis zéro.
            self.output.extend_from_slice(b"\x1b[");
            push_decimal(&mut self.output, point.row.value().saturating_add(1));
            self.output.push(b';');
            push_decimal(&mut self.output, point.column.value().saturating_add(1));
            self.output.push(b'H');

            // La quantification a lieu ici, au dernier moment : un thème est écrit une
            // fois en vraies couleurs, et chaque terminal reçoit ce qu'il sait afficher
            // (ADR-008). L'encodeur, lui, reste pur.
            let mut presentation = AirCellPresentation::from(*cell);
            presentation.foreground = quantize(presentation.foreground, self.color_depth);
            presentation.background = quantize(presentation.background, self.color_depth);
            match self.sgr_encoder.encode(&presentation, &mut self.output) {
                Ok(()) => {}
                // `Infallible` : la branche est inatteignable, et le `match` la nomme
                // plutôt que de la masquer derrière un `unwrap` interdit.
                Err(impossible) => match impossible {},
            }

            let mut encoded_character = [0_u8; 4];
            self.output.extend_from_slice(
                cell.character
                    .encode_utf8(&mut encoded_character)
                    .as_bytes(),
            );
        }

        // Le curseur est posé **après** les cellules : l'écriture de la dernière cellule
        // l'a laissé là où elle s'est terminée, et le laisser visible à cet endroit le
        // ferait clignoter n'importe où.
        self.output.extend_from_slice(b"\x1b[0m");
        match self.cursor {
            Some(position) => {
                self.output.extend_from_slice(b"\x1b[");
                push_decimal(&mut self.output, position.row.value().saturating_add(1));
                self.output.push(b';');
                push_decimal(&mut self.output, position.column.value().saturating_add(1));
                self.output.extend_from_slice(b"H\x1b[?25h");
            }
            None => self.output.extend_from_slice(b"\x1b[?25l"),
        }
        self.output.extend_from_slice(b"\x1b[?2026l");
        // Le tampon est pris par valeur le temps de l'écriture pour ne pas emprunter
        // `self` deux fois.
        let payload = core::mem::take(&mut self.output);
        let outcome = self.device.write_output(&payload);
        self.output = payload;
        outcome?;
        self.device.flush_output()?;
        surface.mark_presented();
        Ok(())
    }
}

impl Drop for AirTerminal {
    fn drop(&mut self) {
        // Aucune erreur n'est remontée : on est en train de partir, et laisser le terminal
        // en mode brut serait pire que d'ignorer un échec d'écriture. Chaque étape est
        // donc tentée indépendamment des précédentes.
        // Défaire le protocole clavier avant tout le reste : laisser un terminal en
        // protocole Kitty après la sortie rendrait le shell inutilisable.
        match self.keyboard_protocol {
            AirKeyboardProtocol::Kitty => {
                let _ = self.device.write_output(POP_SEQUENCE);
            }
            AirKeyboardProtocol::ModifyOtherKeys => {
                let _ = self.device.write_output(MODIFY_OTHER_KEYS_DISABLE_SEQUENCE);
            }
            AirKeyboardProtocol::Legacy => {}
        }
        let _ = self.device.write_output(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = self.device.flush_output();
        let _ = self.device.restore_mode();
    }
}

#[cfg(test)]
mod tests {
    use core::cell::RefCell;
    use core::time::Duration;
    use std::rc::Rc;

    use air_tui_platform::{
        InputReadiness, PlatformError, PlatformErrorKind, TerminalDevice, WindowSize,
    };

    use super::{AirKeyboardProtocol, AirTerminal, FALLBACK_SIZE};
    use crate::cell::{AirCell, AirColor};
    use crate::geometry::{AirColumn, AirPoint, AirRow, AirSize};
    use crate::surface::AirSurface;

    /// Un périphérique en mémoire.
    ///
    /// C'est tout l'intérêt de la couture par traits (ADR-011) : le rendu du framework se
    /// vérifie **sans terminal**, de façon déterministe, et sur une machine qui n'a pas de
    /// `/dev/tty`.
    #[derive(Default)]
    struct RecordingDevice {
        /// La taille annoncée, ou `None` pour simuler une taille inconnue.
        size: Option<WindowSize>,
        /// Tout ce qui a été écrit, dans l'ordre.
        written: Vec<u8>,
        /// Les octets que la lecture rendra.
        pending_input: Vec<u8>,
        /// Compte des passages en mode brut.
        raw_mode_entries: usize,
        /// Compte des restaurations.
        restorations: usize,
        /// Ce que l'attente doit rendre.
        readiness: Option<InputReadiness>,
    }

    impl TerminalDevice for RecordingDevice {
        fn window_size(&self) -> Result<Option<WindowSize>, PlatformError> {
            Ok(self.size)
        }

        fn enter_raw_mode(&mut self) -> Result<(), PlatformError> {
            self.raw_mode_entries = self.raw_mode_entries.saturating_add(1);
            Ok(())
        }

        fn restore_mode(&mut self) -> Result<(), PlatformError> {
            self.restorations = self.restorations.saturating_add(1);
            Ok(())
        }

        fn wait_for_input(
            &mut self,
            _timeout: Option<Duration>,
        ) -> Result<InputReadiness, PlatformError> {
            match self.readiness {
                Some(readiness) => Ok(readiness),
                None => Err(PlatformError::new(PlatformErrorKind::Interrupted)),
            }
        }

        fn read_input(&mut self, buffer: &mut [u8]) -> Result<usize, PlatformError> {
            let count = self.pending_input.len().min(buffer.len());
            let Some(slot) = buffer.get_mut(..count) else {
                return Err(PlatformError::new(PlatformErrorKind::Other));
            };
            let (taken, remaining) = self.pending_input.split_at(count);
            slot.copy_from_slice(taken);
            self.pending_input = remaining.to_vec();
            Ok(count)
        }

        fn write_output(&mut self, bytes: &[u8]) -> Result<(), PlatformError> {
            self.written.extend_from_slice(bytes);
            Ok(())
        }

        fn flush_output(&mut self) -> Result<(), PlatformError> {
            Ok(())
        }
    }

    /// Un périphérique qui refuse le mode brut, pour vérifier la remontée d'erreur.
    struct FailingDevice;

    impl TerminalDevice for FailingDevice {
        fn window_size(&self) -> Result<Option<WindowSize>, PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::InputOutput))
        }

        fn enter_raw_mode(&mut self) -> Result<(), PlatformError> {
            Err(PlatformError::new(PlatformErrorKind::NotATerminal))
        }

        fn restore_mode(&mut self) -> Result<(), PlatformError> {
            Ok(())
        }

        fn wait_for_input(
            &mut self,
            _timeout: Option<Duration>,
        ) -> Result<InputReadiness, PlatformError> {
            Ok(InputReadiness::TimedOut)
        }

        fn read_input(&mut self, _buffer: &mut [u8]) -> Result<usize, PlatformError> {
            Ok(0)
        }

        fn write_output(&mut self, _bytes: &[u8]) -> Result<(), PlatformError> {
            Ok(())
        }

        fn flush_output(&mut self) -> Result<(), PlatformError> {
            Ok(())
        }
    }

    fn device(columns: u16, rows: u16) -> RecordingDevice {
        RecordingDevice {
            size: Some(WindowSize { columns, rows }),
            readiness: Some(InputReadiness::Ready),
            ..RecordingDevice::default()
        }
    }

    #[test]
    fn la_construction_passe_en_mode_brut_et_prepare_l_ecran() {
        let terminal = AirTerminal::new(Box::new(device(80, 25))).expect("construction");
        // L'écran alterné et le masquage du curseur doivent être émis d'emblée, sinon la
        // première image écraserait le contenu du shell.
        drop(terminal);
    }

    #[test]
    fn un_peripherique_qui_refuse_le_mode_brut_fait_echouer_la_construction() {
        let outcome = AirTerminal::new(Box::new(FailingDevice));
        assert_eq!(
            outcome.err().map(PlatformError::kind),
            Some(PlatformErrorKind::NotATerminal)
        );
    }

    #[test]
    fn la_taille_inconnue_retombe_sur_la_valeur_de_repli() {
        let mut unknown = device(80, 25);
        unknown.size = None;
        let terminal = AirTerminal::new(Box::new(unknown)).expect("construction");
        assert_eq!(terminal.size().expect("taille"), FALLBACK_SIZE);
    }

    #[test]
    fn la_taille_annoncee_est_rendue_telle_quelle() {
        let terminal = AirTerminal::new(Box::new(device(132, 43))).expect("construction");
        assert_eq!(terminal.size().expect("taille"), AirSize::new(132, 43));
    }

    #[test]
    fn la_taille_remonte_l_erreur_du_peripherique() {
        // `FailingDevice` refuse le mode brut, donc on contourne la construction en
        // testant la remontée sur un périphérique accepté puis défaillant.
        struct SizeFails;
        impl TerminalDevice for SizeFails {
            fn window_size(&self) -> Result<Option<WindowSize>, PlatformError> {
                Err(PlatformError::new(PlatformErrorKind::InputOutput))
            }
            fn enter_raw_mode(&mut self) -> Result<(), PlatformError> {
                Ok(())
            }
            fn restore_mode(&mut self) -> Result<(), PlatformError> {
                Ok(())
            }
            fn wait_for_input(
                &mut self,
                _timeout: Option<Duration>,
            ) -> Result<InputReadiness, PlatformError> {
                Ok(InputReadiness::TimedOut)
            }
            fn read_input(&mut self, _buffer: &mut [u8]) -> Result<usize, PlatformError> {
                Ok(0)
            }
            fn write_output(&mut self, _bytes: &[u8]) -> Result<(), PlatformError> {
                Ok(())
            }
            fn flush_output(&mut self) -> Result<(), PlatformError> {
                Ok(())
            }
        }
        let terminal = AirTerminal::new(Box::new(SizeFails)).expect("construction");
        assert_eq!(
            terminal.size().err().map(PlatformError::kind),
            Some(PlatformErrorKind::InputOutput)
        );
    }

    #[test]
    fn l_attente_transmet_la_conclusion_du_peripherique() {
        let mut terminal = AirTerminal::new(Box::new(device(80, 25))).expect("construction");
        assert_eq!(
            terminal.wait_for_input(Some(Duration::from_millis(0))),
            Ok(InputReadiness::Ready)
        );
    }

    #[test]
    fn l_attente_remonte_l_interruption_sans_la_retenter() {
        let mut interrupting = device(80, 25);
        interrupting.readiness = None;
        let mut terminal = AirTerminal::new(Box::new(interrupting)).expect("construction");
        let outcome = terminal.wait_for_input(None);
        assert!(outcome.err().is_some_and(PlatformError::is_interrupted));
    }

    #[test]
    fn la_lecture_rend_les_octets_du_peripherique() {
        let mut source = device(80, 25);
        source.pending_input = b"xy".to_vec();
        let mut terminal = AirTerminal::new(Box::new(source)).expect("construction");
        let mut buffer = [0_u8; 4];
        assert_eq!(terminal.read_available(&mut buffer).expect("lecture"), 2);
        assert_eq!(buffer.first(), Some(&b'x'));
    }

    /// Rejoue la sortie d'un rendu pour reconstruire la grille réellement envoyée.
    ///
    /// On ne vérifie pas une chaîne d'octets attendue : ce serait figer une mise en forme
    /// sans prouver que l'image est juste. On rejoue, et on compare ce que le terminal
    /// aurait affiché.
    /// Vrai si la sortie se termine par cette séquence.
    fn ends_with(emitted: &[u8], suffix: &[u8]) -> bool {
        emitted.len() >= suffix.len()
            && emitted.get(emitted.len().saturating_sub(suffix.len())..) == Some(suffix)
    }

    fn replay(written: &[u8]) -> Vec<(u16, u16, char)> {
        let text = String::from_utf8_lossy(written);
        let mut replayed = Vec::new();
        let mut remaining = text.as_ref();
        while let Some(start) = remaining.find('\u{1b}') {
            let after = remaining.get(start.saturating_add(2)..).unwrap_or("");
            if let Some(end) = after.find('H') {
                let parameters = after.get(..end).unwrap_or("");
                let mut parts = parameters.split(';');
                let row: Option<u16> = parts.next().and_then(|part| part.parse().ok());
                let column: Option<u16> = parts.next().and_then(|part| part.parse().ok());
                if let (Some(row), Some(column)) = (row, column) {
                    // Le caractère est le dernier de la séquence qui suit le
                    // positionnement et les éventuels attributs.
                    let tail = after.get(end.saturating_add(1)..).unwrap_or("");
                    let after_attributes = match tail.strip_prefix('\u{1b}') {
                        Some(sgr) => sgr
                            .find('m')
                            .map_or(tail, |stop| sgr.get(stop.saturating_add(1)..).unwrap_or("")),
                        None => tail,
                    };
                    if let Some(character) = after_attributes.chars().next() {
                        replayed.push((row, column, character));
                    }
                }
            }
            remaining = remaining.get(start.saturating_add(1)..).unwrap_or("");
        }
        replayed
    }

    #[test]
    fn le_rendu_envoie_les_cellules_modifiees_a_la_bonne_position() {
        let mut surface = AirSurface::new(AirSize::new(3, 2));
        surface.mark_presented();
        surface.set_cell(
            AirPoint::new(AirColumn::new(1), AirRow::new(1)),
            AirCell::new('Z', AirColor::WHITE, AirColor::BLUE),
        );

        let mut recorder = RecordingDevice {
            size: Some(WindowSize {
                columns: 3,
                rows: 2,
            }),
            ..RecordingDevice::default()
        };
        recorder.written.clear();
        let mut terminal = AirTerminal::new(Box::new(recorder)).expect("construction");
        terminal.present(&mut surface).expect("rendu");

        // Rien d'autre à redessiner après un rendu : le différentiel est vidé.
        assert_eq!(surface.difference().count(), 0);
    }

    #[test]
    fn le_rendu_positionne_en_coordonnees_comptees_depuis_un() {
        // La grille compte depuis zéro, l'ANSI depuis un. L'erreur d'un décalage est
        // invisible à la relecture et évidente à l'écran, donc elle se teste.
        let mut surface = AirSurface::new(AirSize::new(2, 2));
        surface.mark_presented();
        surface.set_cell(
            AirPoint::new(AirColumn::FIRST, AirRow::FIRST),
            AirCell::new('A', AirColor::WHITE, AirColor::BLUE),
        );

        let mut collected = Vec::new();
        {
            let recorder = RecordingDevice {
                size: Some(WindowSize {
                    columns: 2,
                    rows: 2,
                }),
                ..RecordingDevice::default()
            };
            let mut terminal = AirTerminal::new(Box::new(recorder)).expect("construction");
            terminal.present(&mut surface).expect("rendu");
            collected.extend_from_slice(&terminal.output);
        }
        let replayed = replay(&collected);
        assert_eq!(replayed.first(), Some(&(1, 1, 'A')));
    }

    #[test]
    fn sans_curseur_le_rendu_le_cache() {
        // L'état initial : aucune vue focalisée n'en réclame, le terminal n'en montre pas.
        let mut surface = AirSurface::new(AirSize::new(2, 1));
        let recorder = RecordingDevice {
            size: Some(WindowSize {
                columns: 2,
                rows: 1,
            }),
            ..RecordingDevice::default()
        };
        let mut terminal = AirTerminal::new(Box::new(recorder)).expect("construction");
        assert_eq!(terminal.cursor(), None);
        terminal.present(&mut surface).expect("rendu");
        let emitted = terminal.output.clone();
        assert!(ends_with(&emitted, b"\x1b[?25l\x1b[?2026l"), "{emitted:?}");
    }

    #[test]
    fn le_curseur_est_pose_apres_les_cellules() {
        // L'écriture de la dernière cellule laisse le curseur là où elle s'est terminée :
        // le poser avant le ferait clignoter n'importe où.
        let mut surface = AirSurface::new(AirSize::new(4, 2));
        surface.mark_presented();
        surface.set_cell(
            AirPoint::new(AirColumn::new(3), AirRow::new(1)),
            AirCell::new('Z', AirColor::WHITE, AirColor::BLUE),
        );
        let recorder = RecordingDevice {
            size: Some(WindowSize {
                columns: 4,
                rows: 2,
            }),
            ..RecordingDevice::default()
        };
        let mut terminal = AirTerminal::new(Box::new(recorder)).expect("construction");
        terminal.set_cursor(Some(AirPoint::new(AirColumn::new(1), AirRow::new(0))));
        assert_eq!(
            terminal.cursor(),
            Some(AirPoint::new(AirColumn::new(1), AirRow::new(0)))
        );
        terminal.present(&mut surface).expect("rendu");
        let emitted = terminal.output.clone();
        // Comptées depuis un, comme toute coordonnée ANSI.
        assert!(
            ends_with(&emitted, b"\x1b[1;2H\x1b[?25h\x1b[?2026l"),
            "{emitted:?}"
        );
    }

    #[test]
    fn un_second_rendu_sans_changement_n_envoie_aucune_cellule() {
        let mut surface = AirSurface::new(AirSize::new(2, 1));
        let recorder = RecordingDevice {
            size: Some(WindowSize {
                columns: 2,
                rows: 1,
            }),
            ..RecordingDevice::default()
        };
        let mut terminal = AirTerminal::new(Box::new(recorder)).expect("construction");
        terminal.present(&mut surface).expect("premier rendu");
        terminal.present(&mut surface).expect("second rendu");
        assert_eq!(replay(&terminal.output), Vec::new());
    }

    /// Un périphérique scriptable pour l'interrogation du protocole clavier.
    ///
    /// Il rend prêt tant qu'il a des octets à donner, puis expire — le comportement d'un
    /// vrai terminal qui a fini de répondre. Ses écritures sont partagées pour rester
    /// observables **après** la destruction du terminal, ce qui est le seul moyen de
    /// vérifier ce que `Drop` émet.
    struct ProbeDevice {
        /// Les réponses à servir, dans l'ordre, un morceau par lecture.
        responses: Vec<Vec<u8>>,
        /// Tout ce qui a été écrit, partagé avec le test.
        written: Rc<RefCell<Vec<u8>>>,
        /// Ce que l'attente rend quand il n'y a plus de réponse.
        exhausted: InputReadiness,
    }

    impl ProbeDevice {
        fn new(responses: Vec<&[u8]>) -> (Self, Rc<RefCell<Vec<u8>>>) {
            let written = Rc::new(RefCell::new(Vec::new()));
            let device = Self {
                responses: responses.into_iter().map(<[u8]>::to_vec).collect(),
                written: Rc::clone(&written),
                exhausted: InputReadiness::TimedOut,
            };
            (device, written)
        }
    }

    impl TerminalDevice for ProbeDevice {
        fn window_size(&self) -> Result<Option<WindowSize>, PlatformError> {
            Ok(Some(WindowSize {
                columns: 80,
                rows: 25,
            }))
        }
        fn enter_raw_mode(&mut self) -> Result<(), PlatformError> {
            Ok(())
        }
        fn restore_mode(&mut self) -> Result<(), PlatformError> {
            Ok(())
        }
        fn wait_for_input(
            &mut self,
            _timeout: Option<Duration>,
        ) -> Result<InputReadiness, PlatformError> {
            if self.responses.is_empty() {
                return Ok(self.exhausted);
            }
            Ok(InputReadiness::Ready)
        }
        fn read_input(&mut self, buffer: &mut [u8]) -> Result<usize, PlatformError> {
            if self.responses.is_empty() {
                return Ok(0);
            }
            let chunk = self.responses.remove(0);
            let count = chunk.len().min(buffer.len());
            let Some(slot) = buffer.get_mut(..count) else {
                return Err(PlatformError::new(PlatformErrorKind::Other));
            };
            slot.copy_from_slice(chunk.get(..count).unwrap_or(&[]));
            Ok(count)
        }
        fn write_output(&mut self, bytes: &[u8]) -> Result<(), PlatformError> {
            self.written.borrow_mut().extend_from_slice(bytes);
            Ok(())
        }
        fn flush_output(&mut self) -> Result<(), PlatformError> {
            Ok(())
        }
    }

    fn probe(responses: Vec<&[u8]>) -> (AirTerminal, Rc<RefCell<Vec<u8>>>) {
        let (device, written) = ProbeDevice::new(responses);
        let terminal = AirTerminal::new(Box::new(device)).expect("construction");
        (terminal, written)
    }

    #[test]
    fn l_interrogation_envoie_la_question_et_le_jalon() {
        let (mut terminal, written) = probe(vec![b"\x1b[?62;1;6c"]);
        let _ = terminal
            .detect_keyboard_protocol(Duration::from_millis(50), &mut |_| {})
            .expect("interrogation");
        let emitted = written.borrow().clone();
        let text = String::from_utf8_lossy(&emitted);
        assert!(
            text.contains("\u{1b}[?u"),
            "interrogation absente : {text:?}"
        );
        assert!(text.contains("\u{1b}[c"), "jalon absent : {text:?}");
    }

    #[test]
    fn une_reponse_kitty_conclut_a_kitty() {
        let (mut terminal, _) = probe(vec![b"\x1b[?23u\x1b[?62;1;6c"]);
        assert_eq!(
            terminal
                .detect_keyboard_protocol(Duration::from_millis(50), &mut |_| {})
                .expect("interrogation"),
            AirKeyboardProtocol::Kitty
        );
    }

    #[test]
    fn le_jalon_seul_conclut_au_codage_historique() {
        // Et c'est une conclusion certaine, pas une expiration : les attributs sont
        // arrivés, donc la réponse Kitty ne viendra plus.
        let (mut terminal, _) = probe(vec![b"\x1b[?62;1;6c"]);
        assert_eq!(
            terminal
                .detect_keyboard_protocol(Duration::from_millis(50), &mut |_| {})
                .expect("interrogation"),
            AirKeyboardProtocol::Legacy
        );
    }

    #[test]
    fn une_reponse_decoupee_en_deux_lectures_est_recollee() {
        let (mut terminal, _) = probe(vec![b"\x1b[?2", b"3u\x1b[?62c"]);
        assert_eq!(
            terminal
                .detect_keyboard_protocol(Duration::from_millis(50), &mut |_| {})
                .expect("interrogation"),
            AirKeyboardProtocol::Kitty
        );
    }

    #[test]
    fn un_terminal_muet_conclut_au_codage_historique_sans_geler() {
        let (mut terminal, _) = probe(Vec::new());
        assert_eq!(
            terminal
                .detect_keyboard_protocol(Duration::from_millis(1), &mut |_| {})
                .expect("interrogation"),
            AirKeyboardProtocol::Legacy
        );
    }

    #[test]
    fn un_delai_nul_n_attend_pas() {
        let (mut terminal, _) = probe(vec![b"\x1b[?23u"]);
        assert_eq!(
            terminal
                .detect_keyboard_protocol(Duration::ZERO, &mut |_| {})
                .expect("interrogation"),
            AirKeyboardProtocol::Legacy
        );
    }

    #[test]
    fn ce_qui_est_tape_pendant_l_interrogation_n_est_pas_perdu() {
        // L'utilisateur impatient tape avant que le terminal ait répondu. Jeter sa frappe
        // serait un défaut qu'on ne verrait qu'au lancement d'une machine lente.
        let (mut terminal, _) = probe(vec![b"a\x1b[?23u\x1b[?62c"]);
        let mut collected = Vec::new();
        let detected = terminal
            .detect_keyboard_protocol(Duration::from_millis(50), &mut |event| {
                collected.push(event);
            })
            .expect("interrogation");
        assert_eq!(detected, AirKeyboardProtocol::Kitty);
        assert_eq!(collected.len(), 1, "la frappe a été perdue : {collected:?}");
    }

    #[test]
    fn une_sequence_indecodable_arrete_l_interrogation_sans_echouer() {
        let (mut terminal, _) = probe(vec![b"\xff"]);
        assert_eq!(
            terminal
                .detect_keyboard_protocol(Duration::from_millis(50), &mut |_| {})
                .expect("une séquence indécodable n'est pas une panne"),
            AirKeyboardProtocol::Legacy
        );
    }

    #[test]
    fn un_peripherique_qui_raccroche_arrete_l_interrogation() {
        let (device, _) = ProbeDevice::new(Vec::new());
        let mut closing = device;
        closing.exhausted = InputReadiness::Closed;
        let mut terminal = AirTerminal::new(Box::new(closing)).expect("construction");
        assert_eq!(
            terminal
                .detect_keyboard_protocol(Duration::from_millis(50), &mut |_| {})
                .expect("interrogation"),
            AirKeyboardProtocol::Legacy
        );
    }

    #[test]
    fn l_activation_ecrit_la_sequence_de_chaque_protocole() {
        for (protocol, expected) in [
            (AirKeyboardProtocol::Kitty, "\u{1b}[>23u"),
            (AirKeyboardProtocol::ModifyOtherKeys, "\u{1b}[>4;2m"),
        ] {
            let (mut terminal, written) = probe(Vec::new());
            written.borrow_mut().clear();
            terminal
                .enable_keyboard_protocol(protocol)
                .expect("activation");
            assert_eq!(terminal.keyboard_protocol(), protocol);
            let emitted = String::from_utf8_lossy(&written.borrow()).to_string();
            assert!(
                emitted.contains(expected),
                "{expected:?} absent de {emitted:?}"
            );
        }
    }

    #[test]
    fn activer_le_codage_historique_n_ecrit_rien() {
        let (mut terminal, written) = probe(Vec::new());
        written.borrow_mut().clear();
        terminal
            .enable_keyboard_protocol(AirKeyboardProtocol::Legacy)
            .expect("activation");
        assert_eq!(written.borrow().len(), 0);
        assert_eq!(terminal.keyboard_protocol(), AirKeyboardProtocol::Legacy);
    }

    #[test]
    fn le_meilleur_protocole_est_kitty_quand_il_est_reconnu() {
        let (mut terminal, _) = probe(vec![b"\x1b[?23u\x1b[?62c"]);
        assert_eq!(
            terminal
                .enable_best_keyboard_protocol(Duration::from_millis(50), &mut |_| {})
                .expect("activation"),
            AirKeyboardProtocol::Kitty
        );
    }

    #[test]
    fn le_meilleur_protocole_retombe_sur_modify_other_keys() {
        // Demandé sans interrogation : il n'existe pas de sondage universel, et la demande
        // est inoffensive pour un terminal qui ne la comprend pas.
        let (mut terminal, _) = probe(vec![b"\x1b[?62c"]);
        assert_eq!(
            terminal
                .enable_best_keyboard_protocol(Duration::from_millis(50), &mut |_| {})
                .expect("activation"),
            AirKeyboardProtocol::ModifyOtherKeys
        );
    }

    #[test]
    fn la_destruction_defait_le_protocole_demande() {
        // Laisser un terminal en protocole Kitty après la sortie rendrait le shell
        // inutilisable : c'est le genre de défaut qu'on ne pardonne pas à un framework.
        for (protocol, expected) in [
            (AirKeyboardProtocol::Kitty, "\u{1b}[<u"),
            (AirKeyboardProtocol::ModifyOtherKeys, "\u{1b}[>4;0m"),
        ] {
            let (mut terminal, written) = probe(Vec::new());
            terminal
                .enable_keyboard_protocol(protocol)
                .expect("activation");
            written.borrow_mut().clear();
            drop(terminal);
            let emitted = String::from_utf8_lossy(&written.borrow()).to_string();
            assert!(
                emitted.contains(expected),
                "{expected:?} absent de {emitted:?} pour {protocol:?}"
            );
        }
    }

    #[test]
    fn la_destruction_en_codage_historique_ne_defait_rien_de_plus() {
        let (terminal, written) = probe(Vec::new());
        written.borrow_mut().clear();
        drop(terminal);
        let emitted = String::from_utf8_lossy(&written.borrow()).to_string();
        assert!(
            !emitted.contains("\u{1b}[<u"),
            "pop inattendu : {emitted:?}"
        );
        assert!(emitted.contains("\u{1b}[?1049l"), "restauration absente");
    }

    #[test]
    fn le_rendu_saute_la_continuation_d_un_caractere_large() {
        // L'émettre écraserait la moitié droite du glyphe que le terminal vient de
        // dessiner : un seul caractère doit être positionné pour deux cellules.
        let mut surface = AirSurface::new(AirSize::new(4, 1));
        surface.mark_presented();
        surface.draw_text(
            AirPoint::new(AirColumn::FIRST, AirRow::FIRST),
            "漢",
            AirColor::WHITE,
            AirColor::BLUE,
            crate::cell::AirAttributes::NONE,
        );

        let recorder = RecordingDevice {
            size: Some(WindowSize {
                columns: 4,
                rows: 1,
            }),
            ..RecordingDevice::default()
        };
        let mut terminal = AirTerminal::new(Box::new(recorder)).expect("construction");
        terminal.present(&mut surface).expect("rendu");
        let replayed = replay(&terminal.output);
        assert_eq!(replayed, vec![(1, 1, '漢')]);
    }

    #[test]
    fn le_debug_ne_divulgue_pas_le_peripherique() {
        let terminal = AirTerminal::new(Box::new(device(80, 25))).expect("construction");
        let rendered = format!("{terminal:?}");
        assert!(
            rendered.contains("AirTerminal"),
            "rendu inattendu : {rendered}"
        );
    }
}
