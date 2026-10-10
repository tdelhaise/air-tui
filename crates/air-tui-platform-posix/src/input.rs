// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'attente d'une entrée, avec délai.

use core::ffi::c_int;
use core::time::Duration;
use std::os::fd::{AsRawFd, BorrowedFd};

use air_tui_platform::{InputReadiness, PlatformError};

use crate::errno::Errno;
use crate::ffi::{self, PollDescriptor};

/// Attend qu'une entrée soit disponible sur le descripteur.
///
/// `timeout` à `None` attend indéfiniment.
///
/// Les trois issues normales sont distinguées **par le type**, et aucune n'est confondue
/// avec une autre : c'est la leçon du gate `check-poll-results` d'Air — prendre « j'ai
/// fini d'attendre » pour « l'événement est arrivé » est un défaut qui ne se voit pas à la
/// relecture. Le champ `revents` est donc consulté, et non seulement le compte rendu par
/// `poll`.
///
/// # Errors
///
/// - [`Errno::INTERRUPTED`] quand un signal a interrompu l'attente. **La couche 0 ne
///   retente jamais** (ADR-021 §2) : c'est à l'appelant de décider s'il reprend l'attente
///   ou s'il va d'abord relire la taille du terminal, ce qui est le bon comportement après
///   un redimensionnement.
/// - [`Errno::BAD_FILE_DESCRIPTOR`] quand `poll` rend `POLLNVAL`, c'est-à-dire quand le
///   descripteur n'est pas surveillable.
/// - [`Errno::INPUT_OUTPUT`] quand `poll` rend `POLLERR`.
pub fn wait_for_input(
    file_descriptor: BorrowedFd<'_>,
    timeout: Option<Duration>,
) -> Result<InputReadiness, PlatformError> {
    let timeout_milliseconds = match timeout {
        // `poll` prend un délai négatif pour « indéfiniment ».
        None => -1,
        Some(duration) => {
            // Conversion lossy explicitement bornée : un délai qui dépasse la capacité du
            // paramètre est ramené au maximum représentable, ce qui reste une attente bien
            // plus longue que tout usage d'interface.
            c_int::try_from(duration.as_millis()).unwrap_or(c_int::MAX)
        }
    };

    let mut watched = PollDescriptor {
        file_descriptor: file_descriptor.as_raw_fd(),
        requested_events: ffi::POLLIN,
        returned_events: 0,
    };

    // SAFETY: `watched` est une allocation locale vivante pour la durée de l'appel, et on
    // annonce exactement un descripteur, ce qui correspond à la taille du tampon pointé.
    let ready_count =
        unsafe { ffi::poll(core::ptr::from_mut(&mut watched), 1, timeout_milliseconds) };

    if ready_count == -1 {
        return Err(Errno::last().into());
    }
    if ready_count == 0 {
        return Ok(InputReadiness::TimedOut);
    }
    classify(watched.returned_events).map_err(PlatformError::from)
}

/// Traduit un champ `revents` en issue typée.
///
/// L'ordre des contrôles n'est pas indifférent : `POLLNVAL` et `POLLERR` sont des
/// conditions d'erreur et priment, `POLLIN` prime sur `POLLHUP` parce qu'un flux qui
/// raccroche peut encore avoir des octets en attente qu'il serait faux de perdre.
fn classify(returned_events: core::ffi::c_short) -> Result<InputReadiness, Errno> {
    if returned_events & ffi::POLLNVAL != 0 {
        return Err(Errno::BAD_FILE_DESCRIPTOR);
    }
    if returned_events & ffi::POLLERR != 0 {
        return Err(Errno::INPUT_OUTPUT);
    }
    if returned_events & ffi::POLLIN != 0 {
        return Ok(InputReadiness::Ready);
    }
    if returned_events & ffi::POLLHUP != 0 {
        return Ok(InputReadiness::Closed);
    }
    // `poll` a signalé le descripteur pour une raison qu'on n'a pas demandée et qui n'est
    // ni une erreur ni une fermeture : rien à lire, donc rien d'autre à dire qu'une
    // expiration.
    Ok(InputReadiness::TimedOut)
}

#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::io::Write;
    use std::os::fd::AsFd;

    use air_tui_platform::{InputReadiness, PlatformErrorKind};

    use super::{classify, wait_for_input};
    use crate::errno::Errno;
    use crate::ffi;

    #[test]
    fn un_tube_portant_des_octets_est_lisible() {
        let (reader, mut writer) = std::io::pipe().expect("création du tube");
        writer.write_all(b"x").expect("écriture dans le tube");
        let readiness = wait_for_input(reader.as_fd(), Some(Duration::from_millis(50)))
            .expect("poll sur un tube garni");
        assert_eq!(readiness, InputReadiness::Ready);
    }

    #[test]
    fn un_tube_vide_expire() {
        // Un tube sans écrivain actif ne devient pas lisible : l'attente doit expirer, et
        // cette expiration doit être une issue normale, pas une erreur.
        let (reader, _writer) = std::io::pipe().expect("création du tube");
        let readiness = wait_for_input(reader.as_fd(), Some(Duration::from_millis(10)))
            .expect("poll sur un tube vide");
        assert_eq!(readiness, InputReadiness::TimedOut);
    }

    #[test]
    fn la_classification_fait_primer_les_erreurs() {
        assert_eq!(
            classify(ffi::POLLNVAL | ffi::POLLIN).err(),
            Some(Errno::BAD_FILE_DESCRIPTOR)
        );
        assert_eq!(
            classify(ffi::POLLERR | ffi::POLLIN).err(),
            Some(Errno::INPUT_OUTPUT)
        );
    }

    #[test]
    fn la_classification_fait_primer_les_donnees_sur_le_raccrochage() {
        // Un flux qui raccroche peut encore porter des octets : les perdre serait un
        // défaut silencieux.
        assert_eq!(
            classify(ffi::POLLIN | ffi::POLLHUP),
            Ok(InputReadiness::Ready)
        );
        assert_eq!(classify(ffi::POLLHUP), Ok(InputReadiness::Closed));
    }

    #[test]
    fn la_classification_d_un_signalement_inattendu_vaut_expiration() {
        assert_eq!(classify(0), Ok(InputReadiness::TimedOut));
    }

    /// Divergence constatée, pas supposée : le `poll` de macOS ne sait pas surveiller
    /// `/dev/null` et rend `POLLNVAL` (0x0020), là où Linux rend `POLLIN`.
    ///
    /// Ce test existe pour que la connaissance ne se reperde pas : c'est le contrôle du
    /// champ `revents` qui l'a révélée au premier lancement, et non le compte rendu par
    /// `poll`, qui valait un dans les deux cas.
    #[test]
    #[cfg(target_os = "macos")]
    fn sur_macos_poll_refuse_dev_null() {
        let file = std::fs::File::open("/dev/null").expect("ouverture de /dev/null");
        assert_eq!(
            wait_for_input(file.as_fd(), Some(Duration::from_millis(0)))
                .err()
                .map(air_tui_platform::PlatformError::kind),
            Some(PlatformErrorKind::InvalidDevice)
        );
    }

    /// Le pendant Linux de la divergence ci-dessus.
    #[test]
    #[cfg(target_os = "linux")]
    fn sur_linux_dev_null_est_immediatement_lisible() {
        let file = std::fs::File::open("/dev/null").expect("ouverture de /dev/null");
        assert_eq!(
            wait_for_input(file.as_fd(), Some(Duration::from_millis(0))),
            Ok(InputReadiness::Ready)
        );
    }
}
