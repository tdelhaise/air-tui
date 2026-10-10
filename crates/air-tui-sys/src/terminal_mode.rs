// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Le mode du terminal : lecture, passage en mode brut, restauration.
//!
//! Aucune constante de drapeau n'apparaît ici : `cfmakeraw` compose le mode brut, et
//! l'état sauvegardé est un tampon opaque. Voir la note d'ABI de [`crate::ffi`].

use std::os::fd::{AsRawFd, BorrowedFd};

use crate::errno::Errno;
use crate::ffi::{self, TermiosBuffer};

/// Un état du terminal, conservé pour pouvoir le restaurer.
///
/// Opaque par construction : le projet n'inspecte jamais ses champs, il se contente de le
/// rendre tel quel au système.
#[derive(Clone, Copy)]
pub struct TerminalMode {
    /// L'état rendu par `tcgetattr`, jamais inspecté par le projet.
    saved: TermiosBuffer,
}

impl core::fmt::Debug for TerminalMode {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Le contenu n'a pas de sens hors du système : on n'en affiche pas les octets.
        formatter.write_str("TerminalMode(opaque)")
    }
}

/// Indique si le descripteur désigne un terminal.
///
/// Appelé avant toute tentative de passage en mode brut : basculer un fichier ordinaire
/// ou un tube en mode brut n'a pas de sens, et l'échec serait plus loin et moins clair.
#[must_use]
pub fn is_terminal(file_descriptor: BorrowedFd<'_>) -> bool {
    // SAFETY: `isatty` ne lit que le descripteur passé, n'écrit nulle part, et
    // `BorrowedFd` garantit que le descripteur est ouvert pour la durée de l'appel.
    let answer = unsafe { ffi::isatty(file_descriptor.as_raw_fd()) };
    answer == 1
}

/// Lit le mode courant du terminal.
///
/// # Errors
///
/// Rend l'`errno` posé par `tcgetattr` — `ENOTTY` si le descripteur n'est pas un
/// terminal, `EBADF` s'il n'est pas ouvert.
pub fn current_mode(file_descriptor: BorrowedFd<'_>) -> Result<TerminalMode, Errno> {
    let mut buffer = TermiosBuffer::zeroed();
    // SAFETY: `buffer` est une allocation locale vivante pour la durée de l'appel, d'au
    // moins `sizeof(struct termios)` octets sur les deux plateformes (voir
    // `TERMIOS_BUFFER_SIZE`), et correctement alignée. `tcgetattr` n'écrit pas au-delà.
    let outcome = unsafe { ffi::tcgetattr(file_descriptor.as_raw_fd(), buffer.as_mut_pointer()) };
    if outcome == -1 {
        return Err(Errno::last());
    }
    Ok(TerminalMode { saved: buffer })
}

/// Passe le terminal en mode brut et rend **le mode précédent**.
///
/// Le mode rendu est celui qu'il faudra réappliquer pour restaurer le terminal : la valeur
/// n'est donc pas décorative, la perdre revient à laisser le terminal inutilisable après
/// la sortie du programme.
///
/// # Errors
///
/// `ENOTTY` si le descripteur n'est pas un terminal — le contrôle est fait ici plutôt que
/// laissé à `tcsetattr`, pour que l'erreur désigne la cause. Sinon, l'`errno` posé par
/// `tcgetattr` ou `tcsetattr`.
pub fn enter_raw_mode(file_descriptor: BorrowedFd<'_>) -> Result<TerminalMode, Errno> {
    if !is_terminal(file_descriptor) {
        return Err(Errno::NOT_A_TERMINAL);
    }

    let previous = current_mode(file_descriptor)?;
    let mut raw = previous.saved;

    // SAFETY: `raw` est une copie locale vivante pour la durée de l'appel, de taille et
    // d'alignement suffisants. `cfmakeraw` ne fait que modifier les champs de drapeaux de
    // la structure qu'on lui désigne.
    unsafe { ffi::cfmakeraw(raw.as_mut_pointer()) };

    // SAFETY: `raw` a été rempli par `tcgetattr` puis `cfmakeraw`, donc il contient une
    // `struct termios` valide. `tcsetattr` ne fait que la lire.
    let outcome =
        unsafe { ffi::tcsetattr(file_descriptor.as_raw_fd(), ffi::TCSANOW, raw.as_pointer()) };
    if outcome == -1 {
        return Err(Errno::last());
    }
    Ok(previous)
}

/// Réapplique un mode sauvegardé.
///
/// # Errors
///
/// Rend l'`errno` posé par `tcsetattr`.
pub fn restore_mode(file_descriptor: BorrowedFd<'_>, mode: &TerminalMode) -> Result<(), Errno> {
    // SAFETY: `mode.saved` provient d'un `tcgetattr` réussi, donc contient une
    // `struct termios` valide, et vit pour la durée de l'appel. `tcsetattr` ne fait que
    // la lire.
    let outcome = unsafe {
        ffi::tcsetattr(
            file_descriptor.as_raw_fd(),
            ffi::TCSANOW,
            mode.saved.as_pointer(),
        )
    };
    if outcome == -1 {
        return Err(Errno::last());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsFd;

    use super::{current_mode, enter_raw_mode, is_terminal};
    use crate::errno::Errno;

    #[test]
    fn un_fichier_ordinaire_n_est_pas_un_terminal() {
        let file = std::fs::File::open("/dev/null").expect("ouverture de /dev/null");
        assert!(!is_terminal(file.as_fd()));
    }

    #[test]
    fn lire_le_mode_d_un_non_terminal_echoue() {
        let file = std::fs::File::open("/dev/null").expect("ouverture de /dev/null");
        assert!(current_mode(file.as_fd()).is_err());
    }

    #[test]
    fn passer_un_non_terminal_en_mode_brut_rend_not_a_terminal() {
        let file = std::fs::File::open("/dev/null").expect("ouverture de /dev/null");
        // `TerminalMode` est opaque et n'implémente pas `PartialEq` : on compare
        // l'erreur, qui est la seule chose qui nous intéresse ici.
        assert_eq!(
            enter_raw_mode(file.as_fd()).err(),
            Some(Errno::NOT_A_TERMINAL)
        );
    }
}
