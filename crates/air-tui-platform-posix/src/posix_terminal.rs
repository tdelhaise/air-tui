// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le terminal POSIX, ouvert sur `/dev/tty`.

use core::time::Duration;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, BorrowedFd};

use air_tui_platform::{
    InputReadiness, PlatformError, PlatformErrorKind, TerminalDevice, WindowSize,
};

use crate::errno::Errno;
use crate::input::wait_for_input;
use crate::terminal_mode::{TerminalMode, enter_raw_mode, is_terminal, restore_mode};
use crate::window_size::terminal_window_size;

/// Un terminal POSIX.
///
/// Ouvert sur `/dev/tty` et non sur l'entrée standard : l'application garde ainsi la
/// possibilité de voir ses flux redirigés par le shell sans que son interface en souffre.
///
/// Le mode d'origine est restauré par [`Drop`]. C'est la seule garantie qui tienne : un
/// chemin d'erreur oublié laisserait sinon le terminal inutilisable après la sortie du
/// programme.
#[derive(Debug)]
pub struct PosixTerminal {
    /// Le terminal ouvert.
    teletype: File,
    /// Le mode à restaurer en sortie, présent dès que le mode brut a été activé.
    original_mode: Option<TerminalMode>,
}

impl PosixTerminal {
    /// Ouvre `/dev/tty`.
    ///
    /// Le mode brut n'est **pas** activé ici : c'est
    /// [`TerminalDevice::enter_raw_mode`] qui le fait, pour que l'ouverture et le
    /// changement d'état restent deux décisions distinctes.
    ///
    /// # Errors
    ///
    /// Si `/dev/tty` n'est pas ouvrable, ou s'il ne désigne pas un terminal.
    pub fn open() -> Result<Self, PlatformError> {
        let teletype = File::options()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .map_err(|error| platform_error_from_io(&error))?;
        if !is_terminal(teletype.as_fd()) {
            return Err(PlatformError::new(PlatformErrorKind::NotATerminal));
        }
        Ok(Self {
            teletype,
            original_mode: None,
        })
    }

    /// Le descripteur, pour les enveloppes de ce module.
    fn file_descriptor(&self) -> BorrowedFd<'_> {
        self.teletype.as_fd()
    }
}

impl TerminalDevice for PosixTerminal {
    fn window_size(&self) -> Result<Option<WindowSize>, PlatformError> {
        let measured = terminal_window_size(self.file_descriptor())?;
        Ok(measured.map(|size| WindowSize {
            columns: size.columns,
            rows: size.rows,
        }))
    }

    fn enter_raw_mode(&mut self) -> Result<(), PlatformError> {
        // Le premier passage conserve le mode d'origine. Un second appel ne doit pas
        // l'écraser avec un mode déjà brut, sinon la restauration ne rendrait rien.
        let previous = enter_raw_mode(self.file_descriptor())?;
        if self.original_mode.is_none() {
            self.original_mode = Some(previous);
        }
        Ok(())
    }

    fn restore_mode(&mut self) -> Result<(), PlatformError> {
        match self.original_mode.take() {
            Some(mode) => {
                restore_mode(self.file_descriptor(), &mode)?;
                Ok(())
            }
            // Rien à restaurer n'est pas une erreur : le terminal est déjà dans son état
            // d'origine.
            None => Ok(()),
        }
    }

    fn wait_for_input(
        &mut self,
        timeout: Option<Duration>,
    ) -> Result<InputReadiness, PlatformError> {
        wait_for_input(self.file_descriptor(), timeout)
    }

    fn read_input(&mut self, buffer: &mut [u8]) -> Result<usize, PlatformError> {
        self.teletype
            .read(buffer)
            .map_err(|error| platform_error_from_io(&error))
    }

    fn write_output(&mut self, bytes: &[u8]) -> Result<(), PlatformError> {
        self.teletype
            .write_all(bytes)
            .map_err(|error| platform_error_from_io(&error))
    }

    fn flush_output(&mut self) -> Result<(), PlatformError> {
        self.teletype
            .flush()
            .map_err(|error| platform_error_from_io(&error))
    }
}

impl Drop for PosixTerminal {
    fn drop(&mut self) {
        // Aucune erreur n'est remontée : on est en train de partir, et laisser le terminal
        // en mode brut serait pire que d'ignorer un échec.
        if let Some(mode) = self.original_mode.take() {
            let _ = restore_mode(self.teletype.as_fd(), &mode);
        }
    }
}

/// Traduit une erreur de `std::io` en erreur de plateforme.
///
/// Le code natif est conservé pour le diagnostic, mais c'est la nature qui porte la
/// décision — elle a le même sens sur toutes les cibles.
fn platform_error_from_io(error: &std::io::Error) -> PlatformError {
    match error.raw_os_error().and_then(core::num::NonZeroI32::new) {
        Some(code) => Errno::from_code_value(code).into(),
        None => PlatformError::new(PlatformErrorKind::Other),
    }
}

#[cfg(test)]
mod tests {
    use super::platform_error_from_io;
    use air_tui_platform::PlatformErrorKind;

    #[test]
    fn une_erreur_io_sans_code_natif_devient_other() {
        let error = std::io::Error::other("sans code natif");
        assert_eq!(
            platform_error_from_io(&error).kind(),
            PlatformErrorKind::Other
        );
    }

    #[test]
    fn une_erreur_io_avec_code_natif_conserve_sa_nature() {
        let error = std::io::Error::from_raw_os_error(4);
        let converted = platform_error_from_io(&error);
        assert_eq!(converted.kind(), PlatformErrorKind::Interrupted);
        assert_eq!(
            converted.native_code().map(core::num::NonZeroI32::get),
            Some(4)
        );
    }

    #[test]
    fn ouvrir_un_terminal_echoue_proprement_sans_tty() {
        // Sous un harnais de test sans terminal de contrôle, `/dev/tty` n'est pas
        // ouvrable. Le point vérifié est qu'on rend une erreur typée, pas une panique.
        if let Err(error) = super::PosixTerminal::open() {
            assert_ne!(error.kind(), PlatformErrorKind::Unsupported);
        }
    }
}
