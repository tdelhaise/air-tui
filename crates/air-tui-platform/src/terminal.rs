// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le contrat du périphérique terminal.

use core::time::Duration;

use crate::error::PlatformError;

/// La taille utile du terminal, en cellules de texte.
///
/// Les deux dimensions sont non nulles par contrat : une plateforme qui ne connaît pas sa
/// taille rend `None`, elle ne rend pas une dimension nulle (ADR-021 §1 d'Air).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    /// Nombre de colonnes.
    pub columns: u16,
    /// Nombre de lignes.
    pub rows: u16,
}

/// Ce qu'a conclu une attente d'entrée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputReadiness {
    /// Des octets sont disponibles en lecture.
    ///
    /// Une fin de flux compte comme disponible : la lecture qui suit rendra zéro octet.
    Ready,
    /// Le délai a expiré sans qu'aucun octet n'arrive.
    TimedOut,
    /// Le périphérique a disparu et plus aucune donnée n'arrivera.
    ///
    /// Une boucle d'événements doit s'arrêter, pas réessayer.
    Closed,
}

/// Ce dont air-tui a besoin d'un terminal, exprimé sans rien supposer du système.
///
/// Le trait est **object-safe** : le cœur du framework tient un
/// `Box<dyn TerminalDevice>` et ne porte donc aucun paramètre de type jusque dans l'arbre
/// de vues. C'est ce qui permet d'avoir, demain, une implémentation `linux-air` sans
/// toucher une ligne du cœur.
///
/// # Ce que le contrat n'impose pas
///
/// Il ne parle ni de descripteur de fichier, ni de `termios`, ni de `poll` : ces notions
/// n'existent pas sur toutes les cibles. Une implémentation est libre de s'appuyer sur un
/// pty POSIX, sur un service d'un système en Rust intégral, ou sur un tampon en mémoire
/// pour les tests.
pub trait TerminalDevice {
    /// La taille courante, ou `None` si la plateforme ne la connaît pas encore.
    ///
    /// Appelée à chaque tour de boucle : interroger à la demande évite de maintenir une
    /// valeur qui se périme après un redimensionnement.
    ///
    /// # Errors
    ///
    /// Si l'interrogation échoue.
    fn window_size(&self) -> Result<Option<WindowSize>, PlatformError>;

    /// Place le terminal dans l'état où air-tui peut travailler : pas d'écho, pas de mise
    /// en tampon par ligne, pas d'interprétation des caractères de contrôle.
    ///
    /// # Errors
    ///
    /// Si le périphérique n'est pas un terminal, ou si le changement d'état échoue.
    fn enter_raw_mode(&mut self) -> Result<(), PlatformError>;

    /// Rend au terminal l'état qu'il avait avant [`Self::enter_raw_mode`].
    ///
    /// # Errors
    ///
    /// Si la restauration échoue.
    fn restore_mode(&mut self) -> Result<(), PlatformError>;

    /// Attend qu'une entrée soit disponible. `timeout` à `None` attend indéfiniment.
    ///
    /// # Errors
    ///
    /// Avec [`crate::PlatformErrorKind::Interrupted`] quand un signal a interrompu
    /// l'attente. **Aucune implémentation ne retente d'elle-même.**
    fn wait_for_input(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<InputReadiness, PlatformError>;

    /// Lit les octets disponibles et rend leur nombre. Aucune interprétation.
    ///
    /// # Errors
    ///
    /// Si la lecture échoue.
    fn read_input(&mut self, buffer: &mut [u8]) -> Result<usize, PlatformError>;

    /// Écrit des octets sur le terminal.
    ///
    /// # Errors
    ///
    /// Si l'écriture échoue.
    fn write_output(&mut self, bytes: &[u8]) -> Result<(), PlatformError>;

    /// Pousse vers le terminal ce qui serait encore en tampon.
    ///
    /// # Errors
    ///
    /// Si la purge échoue.
    fn flush_output(&mut self) -> Result<(), PlatformError>;
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::{InputReadiness, TerminalDevice, WindowSize};
    use crate::error::{PlatformError, PlatformErrorKind};

    /// Un périphérique de test : prouve que le contrat est implémentable sans aucun
    /// appel système, ce qui est exactement ce qu'on attend de lui.
    struct MemoryTerminal {
        size: Option<WindowSize>,
        raw: bool,
        pending_input: Vec<u8>,
        written: Vec<u8>,
    }

    impl TerminalDevice for MemoryTerminal {
        fn window_size(&self) -> Result<Option<WindowSize>, PlatformError> {
            Ok(self.size)
        }

        fn enter_raw_mode(&mut self) -> Result<(), PlatformError> {
            self.raw = true;
            Ok(())
        }

        fn restore_mode(&mut self) -> Result<(), PlatformError> {
            self.raw = false;
            Ok(())
        }

        fn wait_for_input(
            &mut self,
            _timeout: Option<Duration>,
        ) -> Result<InputReadiness, PlatformError> {
            if self.pending_input.is_empty() {
                Ok(InputReadiness::TimedOut)
            } else {
                Ok(InputReadiness::Ready)
            }
        }

        fn read_input(&mut self, buffer: &mut [u8]) -> Result<usize, PlatformError> {
            let count = self.pending_input.len().min(buffer.len());
            let (taken, remaining) = self.pending_input.split_at(count);
            let Some(slot) = buffer.get_mut(..count) else {
                return Err(PlatformError::new(PlatformErrorKind::Other));
            };
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

    fn memory_terminal() -> MemoryTerminal {
        MemoryTerminal {
            size: Some(WindowSize {
                columns: 80,
                rows: 25,
            }),
            raw: false,
            pending_input: Vec::new(),
            written: Vec::new(),
        }
    }

    #[test]
    fn le_contrat_est_implementable_sans_appel_systeme() {
        let mut device = memory_terminal();
        assert!(!device.raw);
        device.enter_raw_mode().expect("passage en mode brut");
        assert!(device.raw);
        device.restore_mode().expect("restauration");
        assert!(!device.raw);
    }

    #[test]
    fn le_contrat_est_object_safe() {
        // La compilation de cette ligne est le test : un trait non object-safe ne
        // passerait pas, et le cœur du framework tient précisément un `Box<dyn …>`.
        let boxed: Box<dyn TerminalDevice> = Box::new(memory_terminal());
        assert_eq!(
            boxed.window_size().expect("taille"),
            Some(WindowSize {
                columns: 80,
                rows: 25
            })
        );
    }

    #[test]
    fn une_attente_sans_entree_expire() {
        let mut device = memory_terminal();
        assert_eq!(
            device.wait_for_input(Some(Duration::from_millis(0))),
            Ok(InputReadiness::TimedOut)
        );
    }

    #[test]
    fn la_lecture_rend_les_octets_en_attente() {
        let mut device = memory_terminal();
        device.pending_input = b"abc".to_vec();
        assert_eq!(device.wait_for_input(None), Ok(InputReadiness::Ready));
        let mut buffer = [0_u8; 2];
        assert_eq!(device.read_input(&mut buffer).expect("lecture"), 2);
        assert_eq!(&buffer, b"ab");
        assert_eq!(device.read_input(&mut buffer).expect("lecture"), 1);
    }

    #[test]
    fn l_ecriture_accumule_les_octets() {
        let mut device = memory_terminal();
        device.write_output(b"\x1b[0m").expect("écriture");
        device.flush_output().expect("purge");
        assert_eq!(device.written, b"\x1b[0m");
    }
}
