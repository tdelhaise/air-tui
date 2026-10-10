// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le code d'erreur du système, en type non nul.

use core::fmt;
use core::num::NonZeroI32;

/// Un numéro d'erreur du système, garanti non nul.
///
/// La non-nullité est structurelle : `errno` vaut zéro quand il n'y a pas d'erreur, donc
/// un `Errno` existant **est** une erreur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Errno(NonZeroI32);

impl Errno {
    /// `EINTR` — appel interrompu par un signal.
    ///
    /// Exposé parce que c'est le seul code que la couche 1 doit savoir distinguer : la
    /// couche 0 ne retente jamais, c'est l'appelant qui décide s'il reprend l'attente ou
    /// s'il va d'abord relire la taille du terminal.
    pub const INTERRUPTED: Self = Self::from_code(4);

    /// `EINVAL` — argument invalide. Sert de repli quand l'appel a échoué sans poser
    /// `errno`, ce qui ne devrait pas arriver mais ne doit pas pour autant produire un
    /// `Errno` nul.
    pub const INVALID_ARGUMENT: Self = Self::from_code(22);

    /// `EIO` — erreur d'entrée-sortie.
    pub const INPUT_OUTPUT: Self = Self::from_code(5);

    /// `EBADF` — le descripteur n'est pas ouvert, ou n'est pas surveillable.
    pub const BAD_FILE_DESCRIPTOR: Self = Self::from_code(9);

    /// `ENOTTY` — le descripteur ne désigne pas un terminal.
    pub const NOT_A_TERMINAL: Self = Self::from_code(25);

    /// Construit un `Errno` depuis un code littéral non nul.
    ///
    /// # Panics
    ///
    /// Si `code` vaut zéro. Réservé aux constantes de ce module, dont les valeurs sont
    /// littérales : la panique est donc inatteignable à l'exécution.
    const fn from_code(code: i32) -> Self {
        match NonZeroI32::new(code) {
            Some(non_zero) => Self(non_zero),
            None => panic!("un code errno littéral ne peut pas être nul"),
        }
    }

    /// Le code brut, non nul.
    pub const fn code(self) -> NonZeroI32 {
        self.0
    }

    /// Lit l'`errno` courant du fil d'exécution.
    ///
    /// À n'appeler qu'immédiatement après un appel système ayant échoué. Si `errno` est
    /// nul malgré l'échec, rend [`Self::INVALID_ARGUMENT`] plutôt que de fabriquer un
    /// zéro impossible.
    pub(crate) fn last() -> Self {
        match std::io::Error::last_os_error()
            .raw_os_error()
            .and_then(NonZeroI32::new)
        {
            Some(non_zero) => Self(non_zero),
            None => Self::INVALID_ARGUMENT,
        }
    }
}

impl fmt::Display for Errno {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `std::io::Error` sait traduire un code en message du système ; on s'appuie
        // dessus plutôt que d'embarquer une table de messages.
        let described = std::io::Error::from_raw_os_error(self.0.get());
        write!(formatter, "{described} (errno {})", self.0)
    }
}

impl core::error::Error for Errno {}

impl From<Errno> for std::io::Error {
    fn from(errno: Errno) -> Self {
        Self::from_raw_os_error(errno.0.get())
    }
}

#[cfg(test)]
mod tests {
    use super::Errno;

    #[test]
    fn interrupted_porte_le_code_quatre() {
        assert_eq!(Errno::INTERRUPTED.code().get(), 4);
    }

    #[test]
    fn invalid_argument_porte_le_code_vingt_deux() {
        assert_eq!(Errno::INVALID_ARGUMENT.code().get(), 22);
    }

    #[test]
    fn affichage_contient_le_code_numerique() {
        let rendered = Errno::INTERRUPTED.to_string();
        assert!(rendered.contains('4'), "rendu inattendu : {rendered}");
    }

    #[test]
    fn conversion_vers_io_error_preserve_le_code() {
        let converted = std::io::Error::from(Errno::INTERRUPTED);
        assert_eq!(converted.raw_os_error(), Some(4));
    }
}
