// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Les déclarations `extern "C"` — la totalité de la surface C du projet.
//!
//! Six symboles, tous présents dans la bibliothèque C que `std` lie déjà : glibc sur
//! `*-linux-gnu`, libSystem sur `*-apple-darwin`. Aucune entrée dans `Cargo.toml`
//! (ADR-005).
//!
//! Ce module est `pub(crate)` : rien d'ici ne sort de la crate. Chaque symbole reçoit son
//! enveloppe typée dans un module voisin (ADR-021 §3).
//!
//! # Deux pièges d'ABI, traités explicitement
//!
//! ## `ioctl` est variadique
//!
//! Sur l'ABI arm64 d'Apple, les arguments **variadiques** passent par la pile là où les
//! arguments fixes passent par registre. Déclarer `ioctl` en non-variadique « marche »
//! sur `x86_64` et casse silencieusement sur Apple Silicon. La déclaration est donc
//! variadique, et les appelants passent le pointeur en argument variadique.
//!
//! ## `struct termios` n'a pas la même forme sur les deux plateformes
//!
//! `tcflag_t` est 32 bits et `NCCS` vaut 32 sur Linux ; 64 bits et 20 sur macOS. Plutôt
//! que de recopier deux définitions de champs — et deux jeux de constantes de drapeaux —
//! depuis une mémoire faillible, [`TermiosBuffer`] est un **tampon opaque
//! sur-dimensionné** que seul le noyau remplit.
//!
//! Le code ne lit aucun champ : il enchaîne `tcgetattr` → `cfmakeraw` → `tcsetattr` et
//! conserve une copie intacte pour la restauration. Aucune constante de drapeau
//! n'apparaît dans le projet, donc aucune ne peut être fausse.

use core::ffi::{c_int, c_short, c_ulong};

/// Taille du tampon opaque qui tient une `struct termios`.
///
/// Mesures de référence : 60 octets sur Linux `x86_64`, 72 sur macOS. La marge est
/// délibérée — `tcgetattr` écrit `sizeof(struct termios)` octets, et un tampon plus grand
/// que nécessaire est sûr, alors qu'un tampon trop petit serait un débordement.
const TERMIOS_BUFFER_SIZE: usize = 128;

/// Le tampon doit couvrir la plus grande des deux tailles connues, 72 octets sur macOS.
/// Vérifié à la compilation : un tampon trop petit serait un débordement à l'exécution.
const _: () = assert!(TERMIOS_BUFFER_SIZE >= 72);

/// Une `struct termios` traitée comme une boîte noire.
///
/// L'alignement de 8 couvre le membre le plus contraignant des deux plateformes
/// (`tcflag_t` et `speed_t` sur 64 bits chez Apple).
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub(crate) struct TermiosBuffer([u8; TERMIOS_BUFFER_SIZE]);

impl TermiosBuffer {
    /// Un tampon nul, prêt à être rempli par `tcgetattr`.
    pub(crate) const fn zeroed() -> Self {
        Self([0; TERMIOS_BUFFER_SIZE])
    }

    /// Le pointeur mutable à passer aux appels C.
    pub(crate) fn as_mut_pointer(&mut self) -> *mut Self {
        core::ptr::from_mut(self)
    }

    /// Le pointeur constant à passer aux appels C.
    pub(crate) fn as_pointer(&self) -> *const Self {
        core::ptr::from_ref(self)
    }
}

/// La structure que remplit `ioctl(TIOCGWINSZ)`.
///
/// Identique sur Linux et sur macOS : quatre entiers de seize bits.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct Winsize {
    /// Nombre de lignes.
    pub(crate) rows: u16,
    /// Nombre de colonnes.
    pub(crate) columns: u16,
    /// Largeur en pixels, quand le terminal la connaît — zéro sinon.
    pub(crate) horizontal_pixels: u16,
    /// Hauteur en pixels, quand le terminal la connaît — zéro sinon.
    pub(crate) vertical_pixels: u16,
}

/// Un descripteur surveillé par `poll`.
#[repr(C)]
pub(crate) struct PollDescriptor {
    /// Le descripteur surveillé.
    pub(crate) file_descriptor: c_int,
    /// Les événements demandés, en masque de bits.
    pub(crate) requested_events: c_short,
    /// Les événements survenus, remplis par le noyau.
    pub(crate) returned_events: c_short,
}

/// `POLLIN` — des données sont disponibles en lecture.
pub(crate) const POLLIN: c_short = 0x0001;

/// `POLLERR` — condition d'erreur sur le descripteur.
pub(crate) const POLLERR: c_short = 0x0008;

/// `POLLHUP` — l'autre extrémité a raccroché.
pub(crate) const POLLHUP: c_short = 0x0010;

/// `POLLNVAL` — requête invalide : le descripteur n'est pas surveillable.
///
/// macOS le rend pour les périphériques caractère que son `poll` ne sait pas surveiller,
/// `/dev/null` compris, là où Linux rend `POLLIN`. La divergence est réelle et constatée
/// (voir les tests de `input`), pas supposée.
pub(crate) const POLLNVAL: c_short = 0x0020;

/// `TCSANOW` — appliquer le changement de mode immédiatement. Zéro sur les deux
/// plateformes.
pub(crate) const TCSANOW: c_int = 0;

/// `TIOCGWINSZ` — lire la taille de la fenêtre du terminal.
///
/// Les deux valeurs diffèrent parce que Linux emploie un numéro de requête plat là où les
/// BSD encodent la direction et la taille : chez Apple, `_IOR('t', 104, struct winsize)`
/// se développe en `0x4008_7468`.
#[cfg(target_os = "linux")]
pub(crate) const TIOCGWINSZ: c_ulong = 0x5413;
/// `TIOCGWINSZ` — voir la variante Linux pour l'explication du codage.
#[cfg(target_os = "macos")]
pub(crate) const TIOCGWINSZ: c_ulong = 0x4008_7468;

/// Le type du second paramètre de `poll` : `nfds_t`.
///
/// `unsigned long` dans la glibc, `unsigned int` chez Apple. C'est exactement le genre de
/// divergence que cette couche existe pour absorber.
#[cfg(target_os = "linux")]
pub(crate) type NumberOfDescriptors = core::ffi::c_ulong;
/// Le type du second paramètre de `poll` — voir la variante Linux.
#[cfg(target_os = "macos")]
pub(crate) type NumberOfDescriptors = core::ffi::c_uint;

unsafe extern "C" {
    /// Lit l'état courant du terminal dans `termios_pointer`.
    pub(crate) fn tcgetattr(file_descriptor: c_int, termios_pointer: *mut TermiosBuffer) -> c_int;

    /// Applique l'état `termios_pointer` au terminal.
    pub(crate) fn tcsetattr(
        file_descriptor: c_int,
        optional_actions: c_int,
        termios_pointer: *const TermiosBuffer,
    ) -> c_int;

    /// Compose le mode brut dans `termios_pointer`.
    ///
    /// Employé précisément pour ne pas réécrire à la main les masques `ICANON`, `ECHO`,
    /// `ISIG`, `IEXTEN`, `IXON`, `ICRNL` et `OPOST`, dont les valeurs diffèrent entre
    /// Linux et macOS.
    pub(crate) fn cfmakeraw(termios_pointer: *mut TermiosBuffer);

    /// Appel système multiplexé — **variadique**, voir la note d'ABI du module.
    pub(crate) fn ioctl(file_descriptor: c_int, request: c_ulong, ...) -> c_int;

    /// Attend qu'un des descripteurs soit prêt, ou que le délai expire.
    pub(crate) fn poll(
        descriptors: *mut PollDescriptor,
        count: NumberOfDescriptors,
        timeout_milliseconds: c_int,
    ) -> c_int;

    /// Indique si le descripteur désigne un terminal.
    pub(crate) fn isatty(file_descriptor: c_int) -> c_int;
}

#[cfg(test)]
mod tests {
    use super::{TERMIOS_BUFFER_SIZE, TermiosBuffer, Winsize};

    #[test]
    fn le_tampon_termios_a_la_taille_et_l_alignement_annonces() {
        // La borne inférieure est vérifiée à la compilation ; ici on contrôle que le
        // `repr(C, align(8))` produit bien ce que les appels C attendent.
        assert_eq!(size_of::<TermiosBuffer>(), TERMIOS_BUFFER_SIZE);
        assert_eq!(align_of::<TermiosBuffer>(), 8);
    }

    #[test]
    fn winsize_fait_huit_octets() {
        assert_eq!(size_of::<Winsize>(), 8);
    }

    #[test]
    fn le_tampon_nul_est_entierement_nul() {
        let buffer = TermiosBuffer::zeroed();
        assert!(buffer.0.iter().all(|byte| *byte == 0));
    }
}
