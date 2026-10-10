// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! L'erreur de plateforme — portable, et sans supposer qu'un `errno` existe.

use core::fmt;
use core::num::NonZeroI32;

/// La nature d'une erreur de plateforme.
///
/// L'énumération est délibérément courte et fermée sur ce que le cœur du framework doit
/// savoir **distinguer pour agir**. Tout le reste tombe dans [`PlatformErrorKind::Other`] :
/// une erreur qu'on ne sait pas traiter différemment n'a pas besoin d'une variante.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformErrorKind {
    /// L'appel a été interrompu par un signal.
    ///
    /// La seule variante que la boucle d'événements doit reconnaître : elle reprend
    /// l'attente, ou va d'abord relire la taille du terminal. Aucune couche ne retente
    /// à sa place (ADR-021 §2 d'Air).
    Interrupted,
    /// Le périphérique visé n'est pas un terminal.
    NotATerminal,
    /// Le descripteur n'est pas valide, ou n'est pas surveillable.
    InvalidDevice,
    /// Erreur d'entrée-sortie sur le périphérique.
    InputOutput,
    /// L'opération n'existe pas sur cette plateforme.
    ///
    /// Attendue sur `linux-air` pour ce qui n'a pas de sens hors d'un pty POSIX.
    Unsupported,
    /// Une erreur que le cœur ne traite pas de façon particulière.
    Other,
}

/// Une erreur venue de la plateforme.
///
/// Le code natif est conservé quand il existe, pour le diagnostic, mais il n'est **pas**
/// le moyen de décider : le cœur du framework branche sur [`PlatformErrorKind`], qui a le
/// même sens partout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformError {
    /// Ce que l'appelant doit savoir pour agir.
    kind: PlatformErrorKind,
    /// Le code natif, quand la plateforme en fournit un.
    native_code: Option<NonZeroI32>,
}

impl PlatformError {
    /// Construit une erreur sans code natif.
    #[must_use]
    pub const fn new(kind: PlatformErrorKind) -> Self {
        Self {
            kind,
            native_code: None,
        }
    }

    /// Construit une erreur en conservant le code natif de la plateforme.
    #[must_use]
    pub const fn with_native_code(kind: PlatformErrorKind, native_code: NonZeroI32) -> Self {
        Self {
            kind,
            native_code: Some(native_code),
        }
    }

    /// La nature de l'erreur — ce sur quoi l'appelant branche.
    #[must_use]
    pub const fn kind(self) -> PlatformErrorKind {
        self.kind
    }

    /// Le code natif, pour le diagnostic seulement.
    #[must_use]
    pub const fn native_code(self) -> Option<NonZeroI32> {
        self.native_code
    }

    /// Vrai si l'appel a été interrompu par un signal.
    ///
    /// Raccourci du cas que toute boucle d'événements doit traiter.
    #[must_use]
    pub const fn is_interrupted(self) -> bool {
        matches!(self.kind, PlatformErrorKind::Interrupted)
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let described = match self.kind {
            PlatformErrorKind::Interrupted => "appel interrompu par un signal",
            PlatformErrorKind::NotATerminal => "le périphérique n'est pas un terminal",
            PlatformErrorKind::InvalidDevice => "périphérique invalide ou non surveillable",
            PlatformErrorKind::InputOutput => "erreur d'entrée-sortie",
            PlatformErrorKind::Unsupported => "opération absente de cette plateforme",
            PlatformErrorKind::Other => "erreur de plateforme",
        };
        match self.native_code {
            Some(code) => write!(formatter, "{described} (code natif {code})"),
            None => formatter.write_str(described),
        }
    }
}

impl core::error::Error for PlatformError {}

#[cfg(test)]
mod tests {
    use core::num::NonZeroI32;

    use super::{PlatformError, PlatformErrorKind};

    #[test]
    fn une_erreur_sans_code_natif_n_en_expose_aucun() {
        let error = PlatformError::new(PlatformErrorKind::NotATerminal);
        assert_eq!(error.kind(), PlatformErrorKind::NotATerminal);
        assert_eq!(error.native_code(), None);
    }

    #[test]
    fn une_erreur_avec_code_natif_le_conserve() {
        let code = NonZeroI32::new(4).expect("4 est non nul");
        let error = PlatformError::with_native_code(PlatformErrorKind::Interrupted, code);
        assert_eq!(error.native_code(), Some(code));
    }

    #[test]
    fn is_interrupted_ne_vaut_que_pour_l_interruption() {
        assert!(PlatformError::new(PlatformErrorKind::Interrupted).is_interrupted());
        assert!(!PlatformError::new(PlatformErrorKind::InputOutput).is_interrupted());
    }

    #[test]
    fn l_affichage_mentionne_le_code_natif_quand_il_existe() {
        let code = NonZeroI32::new(25).expect("25 est non nul");
        let with_code = PlatformError::with_native_code(PlatformErrorKind::NotATerminal, code);
        assert!(with_code.to_string().contains("25"));

        let without_code = PlatformError::new(PlatformErrorKind::NotATerminal);
        assert!(!without_code.to_string().contains("25"));
    }

    #[test]
    fn chaque_nature_a_un_libelle_propre() {
        let kinds = [
            PlatformErrorKind::Interrupted,
            PlatformErrorKind::NotATerminal,
            PlatformErrorKind::InvalidDevice,
            PlatformErrorKind::InputOutput,
            PlatformErrorKind::Unsupported,
            PlatformErrorKind::Other,
        ];
        let mut rendered: Vec<String> = kinds
            .iter()
            .map(|kind| PlatformError::new(*kind).to_string())
            .collect();
        rendered.sort_unstable();
        let count_before = rendered.len();
        rendered.dedup();
        assert_eq!(
            rendered.len(),
            count_before,
            "deux natures partagent un libellé"
        );
    }
}
