// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le terminal POSIX, sur les descripteurs standards ou sur `/dev/tty`.
//!
//! # Pourquoi les descripteurs standards sont préférés
//!
//! Ouvrir `/dev/tty` paraît plus propre : l'interface survit alors à une redirection des
//! flux par le shell. C'est ce que faisait ce module, et **c'est inutilisable sur macOS**.
//!
//! Sur macOS, `poll` sur un descripteur ouvert sur `/dev/tty` rend `POLLNVAL` — le
//! descripteur est déclaré non surveillable. Mesuré en dehors d'air-tui, le tableau est
//! net :
//!
//! | | macOS | Linux |
//! |---|---|---|
//! | `poll` sur `/dev/tty` | **`POLLNVAL`** | normal |
//! | `poll` sur le descripteur 0 du même terminal | normal | normal |
//! | `select` sur `/dev/tty` | normal | normal |
//!
//! `/dev/tty` n'est pas un périphérique ordinaire : c'est un alias du terminal de contrôle
//! résolu à l'ouverture, et son pilote macOS ne prend pas en charge le mécanisme que `poll`
//! utilise. La conséquence était qu'`AirTerminal::wait_for_input` échouait au premier appel
//! et que toute application sortait aussitôt.
//!
//! Chaque bout est donc choisi séparément : le **descripteur standard** quand c'en est un
//! terminal, `/dev/tty` sinon. C'est ce que font ncurses et Turbo Vision, et cela traite du
//! même coup le cas mixte — `programme > fichier` garde l'entrée sur le descripteur 0, donc
//! une attente qui fonctionne, et ne passe par `/dev/tty` que pour écrire.
//!
//! Il reste un cas où `/dev/tty` sert à l'**entrée** : quand le descripteur 0 n'est pas un
//! terminal, par exemple `programme < fichier`. Sur macOS, l'attente y échouera toujours,
//! et c'est assumé — lire les frappes du terminal quand l'entrée standard a été
//! délibérément redirigée est une demande contradictoire.

use core::ffi::c_int;
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

/// Le descripteur de l'entrée standard.
const STANDARD_INPUT: c_int = 0;
/// Le descripteur de la sortie standard.
const STANDARD_OUTPUT: c_int = 1;

/// Par où passe un bout du terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endpoint {
    /// Le descripteur standard — 0 en entrée, 1 en sortie. **Non possédé** : le refermer
    /// priverait le reste du programme de son terminal.
    Standard,
    /// `/dev/tty`, ouvert et possédé par ce terminal.
    Teletype,
}

/// Par où passer pour un descripteur standard donné.
///
/// Un descripteur standard qui désigne un terminal est pris tel quel ; sinon il a été
/// redirigé, et il faut `/dev/tty`.
fn endpoint_for(descriptor: BorrowedFd<'_>) -> Endpoint {
    if is_terminal(descriptor) {
        Endpoint::Standard
    } else {
        Endpoint::Teletype
    }
}

/// Un terminal POSIX.
///
/// Le mode d'origine est restauré par [`Drop`]. C'est la seule garantie qui tienne : un
/// chemin d'erreur oublié laisserait sinon le terminal inutilisable après la sortie du
/// programme.
#[derive(Debug)]
pub struct PosixTerminal {
    /// `/dev/tty`, ouvert seulement si au moins un bout en a besoin.
    teletype: Option<File>,
    /// Par où arrive l'entrée.
    input: Endpoint,
    /// Par où part la sortie.
    output: Endpoint,
    /// Le mode à restaurer en sortie, présent dès que le mode brut a été activé.
    original_mode: Option<TerminalMode>,
}

impl PosixTerminal {
    /// Ouvre le terminal : descripteurs standards quand ce sont des terminaux, `/dev/tty`
    /// sinon.
    ///
    /// Le mode brut n'est **pas** activé ici : c'est
    /// [`TerminalDevice::enter_raw_mode`] qui le fait, pour que l'ouverture et le
    /// changement d'état restent deux décisions distinctes.
    ///
    /// # Errors
    ///
    /// Quand un bout n'est pas un terminal standard et que `/dev/tty` n'est pas ouvrable,
    /// ou qu'il ne désigne pas un terminal.
    pub fn open() -> Result<Self, PlatformError> {
        // SAFETY: les descripteurs standards sont ouverts par le processus appelant pour
        // toute sa durée de vie, et l'emprunt ne sert qu'à les interroger. Aucune
        // fermeture n'a lieu : `BorrowedFd` ne possède rien.
        let standard_input = unsafe { BorrowedFd::borrow_raw(STANDARD_INPUT) };
        // SAFETY: même raisonnement que pour l'entrée standard.
        let standard_output = unsafe { BorrowedFd::borrow_raw(STANDARD_OUTPUT) };

        let input = endpoint_for(standard_input);
        let output = endpoint_for(standard_output);

        let teletype = if input == Endpoint::Teletype || output == Endpoint::Teletype {
            let opened = File::options()
                .read(true)
                .write(true)
                .open("/dev/tty")
                .map_err(|error| platform_error_from_io(&error))?;
            if !is_terminal(opened.as_fd()) {
                return Err(PlatformError::new(PlatformErrorKind::NotATerminal));
            }
            Some(opened)
        } else {
            None
        };

        Ok(Self {
            teletype,
            input,
            output,
            original_mode: None,
        })
    }

    /// Le descripteur de `/dev/tty`, ou une erreur s'il n'a pas été ouvert.
    ///
    /// Le cas ne se produit pas : `open` ouvre `/dev/tty` dès qu'un bout le réclame. Il est
    /// rendu en erreur plutôt que masqué par un `unwrap`, parce qu'une invariante tenue par
    /// construction est tout de même une invariante.
    fn teletype_descriptor(&self) -> Result<BorrowedFd<'_>, PlatformError> {
        match self.teletype.as_ref() {
            Some(file) => Ok(file.as_fd()),
            None => Err(PlatformError::new(PlatformErrorKind::InvalidDevice)),
        }
    }

    /// Le descripteur d'entrée — celui que `poll`, `ioctl` et `termios` prennent.
    fn input_descriptor(&self) -> Result<BorrowedFd<'_>, PlatformError> {
        match self.input {
            // SAFETY: `open` a vérifié que ce descripteur désigne un terminal, et il reste
            // ouvert aussi longtemps que le processus. L'emprunt ne possède rien.
            Endpoint::Standard => Ok(unsafe { BorrowedFd::borrow_raw(STANDARD_INPUT) }),
            Endpoint::Teletype => self.teletype_descriptor(),
        }
    }
}

impl TerminalDevice for PosixTerminal {
    fn window_size(&self) -> Result<Option<WindowSize>, PlatformError> {
        let measured = terminal_window_size(self.input_descriptor()?)?;
        Ok(measured.map(|size| WindowSize {
            columns: size.columns,
            rows: size.rows,
        }))
    }

    fn enter_raw_mode(&mut self) -> Result<(), PlatformError> {
        // Le premier passage conserve le mode d'origine. Un second appel ne doit pas
        // l'écraser avec un mode déjà brut, sinon la restauration ne rendrait rien.
        let previous = enter_raw_mode(self.input_descriptor()?)?;
        if self.original_mode.is_none() {
            self.original_mode = Some(previous);
        }
        Ok(())
    }

    fn restore_mode(&mut self) -> Result<(), PlatformError> {
        match self.original_mode.take() {
            Some(mode) => {
                restore_mode(self.input_descriptor()?, &mode)?;
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
        wait_for_input(self.input_descriptor()?, timeout)
    }

    fn read_input(&mut self, buffer: &mut [u8]) -> Result<usize, PlatformError> {
        match self.input {
            Endpoint::Standard => std::io::stdin()
                .read(buffer)
                .map_err(|error| platform_error_from_io(&error)),
            Endpoint::Teletype => match self.teletype.as_ref() {
                // `Read` est implémenté pour `&File` : la lecture n'a pas besoin d'un
                // emprunt exclusif, et le décalage de fichier n'a aucun sens sur un
                // terminal.
                Some(mut file) => file
                    .read(buffer)
                    .map_err(|error| platform_error_from_io(&error)),
                None => Err(PlatformError::new(PlatformErrorKind::InvalidDevice)),
            },
        }
    }

    fn write_output(&mut self, bytes: &[u8]) -> Result<(), PlatformError> {
        match self.output {
            Endpoint::Standard => std::io::stdout()
                .write_all(bytes)
                .map_err(|error| platform_error_from_io(&error)),
            Endpoint::Teletype => match self.teletype.as_ref() {
                Some(mut file) => file
                    .write_all(bytes)
                    .map_err(|error| platform_error_from_io(&error)),
                None => Err(PlatformError::new(PlatformErrorKind::InvalidDevice)),
            },
        }
    }

    fn flush_output(&mut self) -> Result<(), PlatformError> {
        // La sortie standard de `std` est tamponnée par ligne : une séquence ANSI, qui ne
        // porte pas de fin de ligne, y resterait sans ce vidage explicite.
        match self.output {
            Endpoint::Standard => std::io::stdout()
                .flush()
                .map_err(|error| platform_error_from_io(&error)),
            Endpoint::Teletype => match self.teletype.as_ref() {
                Some(mut file) => file.flush().map_err(|error| platform_error_from_io(&error)),
                None => Err(PlatformError::new(PlatformErrorKind::InvalidDevice)),
            },
        }
    }
}

impl Drop for PosixTerminal {
    fn drop(&mut self) {
        // Aucune erreur n'est remontée : on est en train de partir, et laisser le terminal
        // en mode brut serait pire que d'ignorer un échec. Le descripteur est relu ici
        // plutôt que conservé, parce qu'un emprunt ne peut pas survivre à la structure.
        let Some(mode) = self.original_mode.take() else {
            return;
        };
        let descriptor = match self.input {
            // SAFETY: le descripteur standard est resté ouvert pour toute la durée du
            // processus, et l'emprunt ne possède rien.
            Endpoint::Standard => Some(unsafe { BorrowedFd::borrow_raw(STANDARD_INPUT) }),
            Endpoint::Teletype => self.teletype.as_ref().map(AsFd::as_fd),
        };
        if let Some(descriptor) = descriptor {
            let _ = restore_mode(descriptor, &mode);
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
    use std::fs::File;
    use std::os::fd::AsFd;

    use air_tui_platform::{PlatformErrorKind, TerminalDevice};

    use super::{Endpoint, PosixTerminal, endpoint_for, platform_error_from_io};

    /// Un terminal dont le bout réclame `/dev/tty` alors qu'il n'a pas été ouvert.
    ///
    /// L'état ne se produit pas par `open`, qui ouvre `/dev/tty` dès qu'un bout le
    /// réclame. Il se construit ici pour vérifier que l'invariante est rendue en erreur et
    /// non supposée : une invariante tenue par construction est tout de même une
    /// invariante, et un jour la construction change.
    fn without_teletype() -> PosixTerminal {
        PosixTerminal {
            teletype: None,
            input: Endpoint::Teletype,
            output: Endpoint::Teletype,
            original_mode: None,
        }
    }

    #[test]
    fn un_descripteur_qui_n_est_pas_un_terminal_renvoie_vers_dev_tty() {
        // Le cas réel : `programme > fichier` redirige la sortie standard.
        let redirected = File::create(std::env::temp_dir().join("air-tui-endpoint-test"))
            .expect("un fichier temporaire");
        assert_eq!(endpoint_for(redirected.as_fd()), Endpoint::Teletype);
    }

    #[test]
    fn un_bout_sur_dev_tty_non_ouvert_est_une_erreur_et_non_une_supposition() {
        let mut terminal = without_teletype();
        assert_eq!(
            terminal.input_descriptor().map(|_| ()).unwrap_err().kind(),
            PlatformErrorKind::InvalidDevice
        );
        assert_eq!(
            terminal.window_size().map(|_| ()).unwrap_err().kind(),
            PlatformErrorKind::InvalidDevice
        );
        assert_eq!(
            terminal.enter_raw_mode().unwrap_err().kind(),
            PlatformErrorKind::InvalidDevice
        );
        let mut buffer = [0_u8; 4];
        assert_eq!(
            terminal
                .read_input(&mut buffer)
                .map(|_| ())
                .unwrap_err()
                .kind(),
            PlatformErrorKind::InvalidDevice
        );
        assert_eq!(
            terminal.write_output(b"x").unwrap_err().kind(),
            PlatformErrorKind::InvalidDevice
        );
        assert_eq!(
            terminal.flush_output().unwrap_err().kind(),
            PlatformErrorKind::InvalidDevice
        );
    }

    #[test]
    fn sans_mode_conserve_la_restauration_ne_fait_rien() {
        let mut terminal = without_teletype();
        assert!(terminal.restore_mode().is_ok());
        // Et la libération d'un terminal sans mode conservé ne doit pas échouer non plus.
        drop(terminal);
    }

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
