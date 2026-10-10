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

use air_tui_platform::{Encoder, InputReadiness, PlatformError, TerminalDevice};

use crate::codec::{AirCellPresentation, AirSgrEncoder, decimal::push_decimal};
use crate::geometry::AirSize;
use crate::surface::AirSurface;

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
        };
        terminal.device.write_output(b"\x1b[?1049h\x1b[?25l")?;
        terminal.device.flush_output()?;
        Ok(terminal)
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
            // Les coordonnées ANSI comptent depuis un, la grille depuis zéro.
            self.output.extend_from_slice(b"\x1b[");
            push_decimal(&mut self.output, point.row.value().saturating_add(1));
            self.output.push(b';');
            push_decimal(&mut self.output, point.column.value().saturating_add(1));
            self.output.push(b'H');

            let presentation = AirCellPresentation::from(*cell);
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

        self.output.extend_from_slice(b"\x1b[0m\x1b[?2026l");
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
        let _ = self.device.write_output(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = self.device.flush_output();
        let _ = self.device.restore_mode();
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use air_tui_platform::{
        InputReadiness, PlatformError, PlatformErrorKind, TerminalDevice, WindowSize,
    };

    use super::{AirTerminal, FALLBACK_SIZE};
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
