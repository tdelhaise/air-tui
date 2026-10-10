// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! La taille de la fenêtre du terminal, en cellules.

use std::os::fd::{AsRawFd, BorrowedFd};

use crate::errno::Errno;
use crate::ffi::{self, Winsize};

/// La taille utile du terminal, en cellules de texte.
///
/// Les deux dimensions sont garanties non nulles : le noyau emploie zéro comme sentinelle
/// de « taille inconnue », et [`terminal_window_size`] la traduit en `None` plutôt que de
/// la propager (ADR-021 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSize {
    /// Nombre de colonnes.
    pub columns: u16,
    /// Nombre de lignes.
    pub rows: u16,
}

/// Interroge le terminal sur sa taille.
///
/// Rend `Ok(None)` quand le noyau annonce une dimension nulle, ce qui arrive sur un pty
/// dont aucune taille n'a encore été négociée. L'appelant choisit alors sa valeur de
/// repli ; la couche 0 ne lui impose pas de deviner à partir d'un zéro.
///
/// Cette fonction est faite pour être appelée **à chaque tour de boucle** : interroger le
/// système à la demande évite de maintenir une variable globale qui se désynchronise après
/// un redimensionnement, et rend inutile la gestion du signal `SIGWINCH`.
///
/// # Errors
///
/// Rend l'`errno` posé par `ioctl` — `ENOTTY` si le descripteur n'est pas un terminal.
pub fn terminal_window_size(file_descriptor: BorrowedFd<'_>) -> Result<Option<WindowSize>, Errno> {
    let mut measured = Winsize::default();
    // SAFETY: `measured` est une allocation locale vivante pour la durée de l'appel, de
    // la taille exacte de `struct winsize` (vérifiée par un test), et le pointeur est
    // passé en argument variadique conformément au prototype C de `ioctl`.
    let outcome = unsafe {
        ffi::ioctl(
            file_descriptor.as_raw_fd(),
            ffi::TIOCGWINSZ,
            core::ptr::from_mut(&mut measured),
        )
    };
    if outcome == -1 {
        return Err(Errno::last());
    }
    if measured.columns == 0 || measured.rows == 0 {
        return Ok(None);
    }
    Ok(Some(WindowSize {
        columns: measured.columns,
        rows: measured.rows,
    }))
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsFd;

    use super::terminal_window_size;

    #[test]
    fn interroger_un_non_terminal_echoue() {
        let file = std::fs::File::open("/dev/null").expect("ouverture de /dev/null");
        assert!(terminal_window_size(file.as_fd()).is_err());
    }
}
